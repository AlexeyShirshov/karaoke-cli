//! Native, schema-checked preparation of pinned PyTorch ZIP checkpoints.
//! Candle's pickle reader interprets tensor metadata; it never executes Python.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use candle_core::{pickle::PthTensors, DType, Tensor};
use candle_roformer::config::SeparatorConfig;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

pub(crate) fn convert(
    checkpoint: &Path,
    config: &Path,
    destination: &Path,
    checksum: &str,
) -> Result<(), String> {
    let result = (|| -> Result<(), String> {
        // Recheck cached sources as well: never interpret unverified pickle data.
        let mut file = File::open(checkpoint).map_err(|error| error.to_string())?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 128 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(checksum) {
            return Err(format!(
                "cached source SHA-256 mismatch; remove {} and retry",
                checkpoint.display()
            ));
        }
        log::info!(
            "preparing safetensors from {} (CPU; no Python required)",
            checkpoint.display()
        );
        let cfg = SeparatorConfig::from_yaml_file(config).map_err(|error| error.to_string())?;
        cfg.validate().map_err(|error| error.to_string())?;
        let expected: HashMap<_, _> = candle_roformer::model::expected_key_shapes(&cfg)
            .into_iter()
            .collect();
        let mut pth = PthTensors::new(checkpoint, None).map_err(|error| error.to_string())?;
        if pth.tensor_infos().is_empty() {
            for key in ["state_dict", "model"] {
                if let Ok(candidate) = PthTensors::new(checkpoint, Some(key)) {
                    if !candidate.tensor_infos().is_empty() {
                        pth = candidate;
                        break;
                    }
                }
            }
        }
        let actual = pth.tensor_infos();
        if actual.len() != expected.len() || expected.keys().any(|key| !actual.contains_key(key)) {
            return Err(format!(
                "checkpoint/config schema mismatch ({} tensors, expected {})",
                actual.len(),
                expected.len()
            ));
        }
        // Check every name, shape and dtype before allocating tensor data.
        for (name, shape) in &expected {
            let info = &actual[name];
            if info.layout.dims() != shape.as_slice() {
                return Err(format!("checkpoint shape mismatch for {name}"));
            }
            if !matches!(
                info.dtype,
                DType::F16 | DType::BF16 | DType::F32 | DType::F64
            ) {
                return Err(format!("non-floating checkpoint tensor: {name}"));
            }
        }
        let mut tensors: HashMap<String, Tensor> = HashMap::with_capacity(expected.len());
        for name in expected.keys() {
            let tensor = pth
                .get(name)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("missing checkpoint tensor: {name}"))?
                .to_dtype(DType::F32)
                .and_then(|tensor| tensor.contiguous())
                .map_err(|error| error.to_string())?;
            let values = tensor
                .flatten_all()
                .and_then(|tensor| tensor.to_vec1::<f32>())
                .map_err(|error| error.to_string())?;
            if values.iter().any(|value| !value.is_finite()) {
                return Err(format!("non-finite checkpoint tensor: {name}"));
            }
            tensors.insert(name.clone(), tensor);
        }
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        // Close the original handle before safetensors opens it for writing
        // (required by Windows file-sharing rules). TempPath still owns cleanup.
        let temporary = NamedTempFile::new_in(parent)
            .map_err(|error| error.to_string())?
            .into_temp_path();
        candle_core::safetensors::save(&tensors, &temporary).map_err(|error| error.to_string())?;
        File::options()
            .write(true)
            .open(&temporary)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
        match temporary.persist_noclobber(destination) {
            Ok(_) => {}
            Err(error)
                if error.error.kind() == std::io::ErrorKind::AlreadyExists
                    && destination.is_file() => {}
            Err(error) => return Err(error.to_string()),
        }
        log::info!("model ready: {}", destination.display());
        Ok(())
    })();
    result.map_err(|error| format!("cannot prepare {}: {error}", destination.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const PROFILE: &str = "model:
  dim: 8
  depth: 1
  stereo: true
  num_stems: 1
  num_bands: 4
  dim_head: 4
  heads: 1
  dim_freqs_in: 9
  stft_n_fft: 16
  stft_hop_length: 4
  stft_win_length: 16
  mask_estimator_depth: 2
  time_transformer_depth: 1
  freq_transformer_depth: 1
";

    // Minimal torch.save-compatible tensor ZIP, generated without Python/torch.
    fn fixture(path: &Path, wrapped: bool, value: f32, omit_tensor: bool) -> String {
        fn string(out: &mut Vec<u8>, value: &str) {
            out.push(b'X');
            out.extend((value.len() as u32).to_le_bytes());
            out.extend(value.as_bytes());
        }
        fn integer(out: &mut Vec<u8>, value: usize) {
            out.push(b'J');
            out.extend((value as i32).to_le_bytes());
        }
        fn tuple(out: &mut Vec<u8>, values: &[usize]) {
            out.push(b'(');
            for value in values {
                integer(out, *value);
            }
            out.push(b't');
        }
        let cfg = SeparatorConfig::from_yaml_str(PROFILE).unwrap();
        let mut schema = candle_roformer::model::expected_key_shapes(&cfg);
        if omit_tensor {
            schema.pop();
        }
        let mut pickle = b"\x80\x02}(".to_vec();
        if wrapped {
            string(&mut pickle, "state_dict");
            pickle.extend(b"}(");
        }
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (index, (name, shape)) in schema.iter().enumerate() {
            let elements: usize = shape.iter().product();
            string(&mut pickle, name);
            pickle.extend(b"ctorch._utils\n_rebuild_tensor_v2\n((");
            string(&mut pickle, "storage");
            pickle.extend(b"ctorch\nFloatStorage\n");
            string(&mut pickle, &index.to_string());
            string(&mut pickle, "cpu");
            integer(&mut pickle, elements);
            pickle.extend(b"tQ");
            integer(&mut pickle, 0);
            tuple(&mut pickle, shape);
            let mut strides = vec![1; shape.len()];
            for i in (0..shape.len().saturating_sub(1)).rev() {
                strides[i] = strides[i + 1] * shape[i + 1];
            }
            tuple(&mut pickle, &strides);
            pickle.extend(b"\x89ccollections\nOrderedDict\n)RtR");
            zip.start_file(format!("archive/data/{index}"), options)
                .unwrap();
            for _ in 0..elements {
                zip.write_all(&value.to_le_bytes()).unwrap();
            }
        }
        pickle.push(b'u');
        if wrapped {
            pickle.push(b'u');
        }
        pickle.push(b'.');
        zip.start_file("archive/data.pkl", options).unwrap();
        zip.write_all(&pickle).unwrap();
        zip.finish().unwrap();
        format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
    }

    #[test]
    fn native_conversion_handles_plain_and_wrapped_checkpoints() {
        for wrapped in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("source.ckpt");
            let config = dir.path().join("config.yaml");
            let destination = dir.path().join("model.safetensors");
            std::fs::write(&config, PROFILE).unwrap();
            let checksum = fixture(&source, wrapped, 0.25, false);
            convert(&source, &config, &destination, &checksum).unwrap();
            let tensors =
                candle_core::safetensors::load(&destination, &candle_core::Device::Cpu).unwrap();
            assert!(!tensors.is_empty());
            for tensor in tensors.values() {
                assert!(tensor
                    .flatten_all()
                    .unwrap()
                    .to_vec1::<f32>()
                    .unwrap()
                    .iter()
                    .all(|value| *value == 0.25));
            }
        }
    }

    #[test]
    fn native_conversion_refuses_bad_schema_and_nonfinite_tensors() {
        for (omit, value, message) in [
            (true, 0.25, "schema mismatch"),
            (false, f32::NAN, "non-finite"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("source.ckpt");
            let config = dir.path().join("config.yaml");
            let destination = dir.path().join("model.safetensors");
            std::fs::write(&config, PROFILE).unwrap();
            let checksum = fixture(&source, false, value, omit);
            let error = convert(&source, &config, &destination, &checksum).unwrap_err();
            assert!(error.contains(message), "{error}");
            assert!(!destination.exists());
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        }
    }

    #[test]
    fn invalid_cached_checkpoint_is_rejected_before_pickle_reading() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.ckpt");
        let destination = dir.path().join("model.safetensors");
        std::fs::write(&source, b"not a checkpoint").unwrap();
        let error = convert(
            &source,
            &dir.path().join("config.yaml"),
            &destination,
            &"0".repeat(64),
        )
        .unwrap_err();
        assert!(error.contains("SHA-256 mismatch"));
        assert!(!destination.exists());
    }

    #[test]
    #[ignore = "set KARAOKE_TEST_CKPT, KARAOKE_TEST_CONFIG and KARAOKE_TEST_SAFETENSORS to real assets"]
    fn native_conversion_matches_reference_weights() {
        let source = std::env::var("KARAOKE_TEST_CKPT").unwrap();
        let config = std::env::var("KARAOKE_TEST_CONFIG").unwrap();
        let reference = std::env::var("KARAOKE_TEST_SAFETENSORS").unwrap();
        let checksum = std::env::var("KARAOKE_TEST_SHA256").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("model.safetensors");
        convert(
            Path::new(&source),
            Path::new(&config),
            &destination,
            &checksum,
        )
        .unwrap();
        let converted =
            candle_core::safetensors::load(&destination, &candle_core::Device::Cpu).unwrap();
        let reference =
            candle_core::safetensors::load(reference, &candle_core::Device::Cpu).unwrap();
        assert_eq!(converted.len(), reference.len());
        for (name, tensor) in converted {
            let expected = &reference[&name];
            assert_eq!(tensor.dims(), expected.dims(), "{name}");
            assert_eq!(
                tensor.flatten_all().unwrap().to_vec1::<f32>().unwrap(),
                expected.flatten_all().unwrap().to_vec1::<f32>().unwrap(),
                "{name}"
            );
        }
    }
}
