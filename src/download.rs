//! Explicitly opted-in downloads of missing runtime-ready model assets.

use std::fs;
use std::io::{Read, Seek, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::catalog::{DownloadFormat, ModelInfo};

pub const WHISPER_SIZES: &[&str] = &[
    "tiny",
    "tiny.en",
    "base",
    "base.en",
    "small",
    "small.en",
    "medium",
    "medium.en",
    "large-v2",
    "large-v3",
];

pub struct Downloader {
    client: Client,
}

impl Downloader {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .https_only(!cfg!(test))
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(6 * 60 * 60))
            .user_agent(concat!("karaoke-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("cannot initialize model downloader: {error}"))?;
        Ok(Self { client })
    }

    pub fn separator(
        &self,
        models: &[ModelInfo],
        model_path: &Path,
        config_path: &Path,
    ) -> Result<(), String> {
        if model_path.is_file() && config_path.is_file() {
            return Ok(());
        }
        if model_path == config_path {
            return Err("model and config paths must be different".into());
        }
        let model = crate::catalog::find_model(models, &model_path.to_string_lossy())?.ok_or_else(
            || {
                format!(
                    "unknown model '{}'; use --list_models to see available names",
                    model_path.display()
                )
            },
        )?;
        let source = model.download.as_ref().ok_or_else(|| {
            format!(
                "automatic download is unavailable for {}: {}",
                model.name, model.weights
            )
        })?;
        // Validate both sources before writing either file.
        validate_source(&source.model_url, source.sha256.as_deref())?;
        validate_source(&source.config_url, source.config_sha256.as_deref())?;
        if source.format == DownloadFormat::Pytorch && source.sha256.is_none() {
            return Err("PyTorch source checkpoints require a pinned SHA-256".into());
        }
        if source.format == DownloadFormat::Pytorch
            && (model_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("safetensors")
                || model_path.with_extension("ckpt") == config_path)
        {
            return Err("converted model must use a .safetensors path distinct from its config and source .ckpt".into());
        }
        self.missing_with_target(
            config_path,
            &source.config_url,
            source.config_sha256.as_deref(),
            source.target_instrument.as_deref(),
        )?;
        if model_path.is_file() {
            return Ok(());
        }
        match source.format {
            DownloadFormat::Safetensors => {
                self.missing(model_path, &source.model_url, source.sha256.as_deref())
            }
            DownloadFormat::Pytorch => {
                let checkpoint = model_path.with_extension("ckpt");
                self.missing(&checkpoint, &source.model_url, source.sha256.as_deref())?;
                crate::checkpoint::convert(
                    &checkpoint,
                    config_path,
                    model_path,
                    source
                        .sha256
                        .as_deref()
                        .expect("validated checkpoint checksum"),
                )
            }
        }
    }

    pub fn whisper(
        &self,
        size: &str,
        config: &Path,
        tokenizer: &Path,
        weights: &Path,
    ) -> Result<(), String> {
        if !WHISPER_SIZES.contains(&size) {
            return Err(format!(
                "unsupported --whisper_size '{size}'; choose {}",
                WHISPER_SIZES.join(", ")
            ));
        }
        if config == tokenizer || config == weights || tokenizer == weights {
            return Err("Whisper asset paths must be different".into());
        }
        for (path, filename) in [
            (config, "config.json"),
            (tokenizer, "tokenizer.json"),
            (weights, "model.safetensors"),
        ] {
            self.missing(
                path,
                &format!("https://huggingface.co/openai/whisper-{size}/resolve/main/{filename}"),
                None,
            )?;
        }
        Ok(())
    }

    /// Stream into a temporary file beside the destination, then publish without
    /// overwriting. Failed or interrupted transfers are never usable model files.
    fn missing(&self, destination: &Path, url: &str, checksum: Option<&str>) -> Result<(), String> {
        self.missing_with_target(destination, url, checksum, None)
    }

    fn missing_with_target(
        &self,
        destination: &Path,
        url: &str,
        checksum: Option<&str>,
        target: Option<&str>,
    ) -> Result<(), String> {
        if destination.is_file() {
            return Ok(());
        }
        if destination.exists() {
            return Err(format!("not a file: {}", destination.display()));
        }
        validate_source(url, checksum)?;
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        log::info!("downloading model asset: {}", destination.display());
        let result = (|| -> Result<(), String> {
            let mut response = self
                .client
                .get(url)
                .send()
                .and_then(|response| response.error_for_status())
                .map_err(|error| format!("HTTP request failed: {error}"))?;
            let expected_size = response.content_length();
            let mut temporary = NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
            let mut hash = Sha256::new();
            let mut buffer = [0_u8; 128 * 1024];
            let mut received = 0_u64;
            let mut last_progress = Instant::now();
            loop {
                let count = response
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                temporary
                    .write_all(&buffer[..count])
                    .map_err(|error| error.to_string())?;
                hash.update(&buffer[..count]);
                received += count as u64;
                if last_progress.elapsed() >= Duration::from_secs(2) {
                    match expected_size {
                        Some(total) => log::info!(
                            "{}: {:.1} / {:.1} MiB",
                            destination.display(),
                            received as f64 / 1048576.0,
                            total as f64 / 1048576.0
                        ),
                        None => log::info!(
                            "{}: {:.1} MiB",
                            destination.display(),
                            received as f64 / 1048576.0
                        ),
                    }
                    last_progress = Instant::now();
                }
            }
            if received == 0 || expected_size.is_some_and(|size| size != received) {
                return Err(format!("empty or incomplete download ({received} bytes)"));
            }
            if let Some(expected) = checksum {
                let actual = format!("{:x}", hash.finalize());
                if !actual.eq_ignore_ascii_case(expected) {
                    return Err(format!(
                        "SHA-256 mismatch: expected {expected}, got {actual}"
                    ));
                }
            }
            if let Some(target) = target {
                let text =
                    fs::read_to_string(temporary.path()).map_err(|error| error.to_string())?;
                let mut profile: serde_yaml::Value =
                    serde_yaml::from_str(&text.replace("!!python/tuple", ""))
                        .map_err(|error| format!("invalid downloaded profile: {error}"))?;
                let root = profile
                    .as_mapping_mut()
                    .ok_or("profile must be a YAML mapping")?;
                let training = root
                    .entry(serde_yaml::Value::String("training".into()))
                    .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()))
                    .as_mapping_mut()
                    .ok_or("training must be a YAML mapping")?;
                training.insert(
                    serde_yaml::Value::String("target_instrument".into()),
                    serde_yaml::Value::String(target.into()),
                );
                let text = serde_yaml::to_string(&profile).map_err(|error| error.to_string())?;
                temporary
                    .as_file_mut()
                    .set_len(0)
                    .map_err(|error| error.to_string())?;
                temporary
                    .as_file_mut()
                    .rewind()
                    .map_err(|error| error.to_string())?;
                temporary
                    .write_all(text.as_bytes())
                    .map_err(|error| error.to_string())?;
            }
            temporary
                .as_file()
                .sync_all()
                .map_err(|error| error.to_string())?;
            match temporary.persist_noclobber(destination) {
                Ok(_) => {}
                // Another process may have finished the same download first.
                Err(error)
                    if error.error.kind() == std::io::ErrorKind::AlreadyExists
                        && destination.is_file() => {}
                Err(error) => return Err(error.to_string()),
            }
            log::info!("model asset ready: {}", destination.display());
            Ok(())
        })();
        result.map_err(|error| format!("cannot download {}: {error}", destination.display()))
    }
}

fn validate_source(url: &str, checksum: Option<&str>) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| format!("invalid model URL: {error}"))?;
    let test_http =
        cfg!(test) && parsed.scheme() == "http" && parsed.host_str() == Some("127.0.0.1");
    if (parsed.scheme() != "https" && !test_http)
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("model downloads require an HTTPS URL without embedded credentials".into());
    }
    if checksum.is_some_and(|value| {
        value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err("download checksum must be a 64-character SHA-256 hex string".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn serve(response: &'static str) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        });
        (url, handle)
    }

    #[test]
    fn download_checks_hash_and_preserves_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("models/model.safetensors");
        let (url, server) =
            serve("HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nmodel");
        let checksum = format!("{:x}", Sha256::digest(b"model"));
        let downloader = Downloader::new().unwrap();
        downloader.missing(&path, &url, Some(&checksum)).unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"model");
        // No connection is attempted when the local file already exists.
        downloader.missing(&path, &url, Some(&checksum)).unwrap();
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn failed_downloads_leave_no_model_or_partial_files() {
        for (response, checksum) in [
            (
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                None,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 99\r\nConnection: close\r\n\r\nshort",
                None,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                None,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nmodel",
                Some("0000000000000000000000000000000000000000000000000000000000000000"),
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("model.safetensors");
            let (url, server) = serve(response);
            assert!(Downloader::new()
                .unwrap()
                .missing(&path, &url, checksum)
                .is_err());
            server.join().unwrap();
            assert!(!path.exists());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn sources_require_https_and_valid_checksums() {
        for url in [
            "http://example.com/model",
            "file:///model",
            "https://user:pass@example.com/model",
        ] {
            assert!(validate_source(url, None).is_err());
        }
        assert!(validate_source("https://example.com/model", Some("bad")).is_err());
        assert!(validate_source("https://example.com/model", None).is_ok());
    }

    #[test]
    fn builtin_download_fetches_both_assets_and_adapts_target_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let weights = dir.path().join("mbr_instfv6n_gabox.safetensors");
        let config = dir.path().join("mbr_instfv6n_gabox_config.yaml");
        let (model_url, model_server) =
            serve("HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nmodel");
        let (config_url, config_server) =
            serve("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\ntraining:\n  target_instrument: Instrumental\nx: 1\n");
        let mut models = crate::catalog::builtin_models().unwrap();
        let model = models
            .iter_mut()
            .find(|model| model.filename == "mbr_instfv6n_gabox.safetensors")
            .unwrap();
        let source = model.download.as_mut().unwrap();
        source.model_url = model_url;
        source.config_url = config_url;
        source.format = DownloadFormat::Safetensors;
        source.sha256 = Some(format!("{:x}", Sha256::digest(b"model")));
        let downloader = Downloader::new().unwrap();
        downloader.separator(&models, &weights, &config).unwrap();
        model_server.join().unwrap();
        config_server.join().unwrap();
        assert_eq!(fs::read(&weights).unwrap(), b"model");
        let text = fs::read_to_string(&config).unwrap();
        assert!(text.contains("target_instrument: instrumental"));
        assert!(text.contains("x: 1"));
        downloader.separator(&models, &weights, &config).unwrap();
        assert_eq!(fs::read_to_string(&config).unwrap(), text);
    }

    #[test]
    fn separator_downloads_only_missing_config_from_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let weights = dir.path().join("custom.safetensors");
        let config = dir.path().join("custom.yaml");
        fs::write(&weights, b"local weights").unwrap();
        let (url, server) =
            serve("HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nmodel: ");
        let manifest = dir.path().join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!([{
                "name": "Custom", "filename": "custom.safetensors",
                "architecture": "MelBandRoformer", "stems": ["vocals"],
                "download": {
                    "model_url": "https://example.com/unused.safetensors",
                    "config_url": url,
                    "config_sha256": format!("{:x}", Sha256::digest(b"model: "))
                }
            }]))
            .unwrap(),
        )
        .unwrap();
        let models = crate::catalog::load_models(&manifest).unwrap();
        let downloader = Downloader::new().unwrap();
        downloader.separator(&models, &weights, &config).unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&weights).unwrap(), b"local weights");
        assert_eq!(fs::read(&config).unwrap(), b"model: ");
        downloader.separator(&[], &weights, &config).unwrap();
    }

    #[test]
    fn unsupported_models_fail_without_creating_files() {
        let dir = tempfile::tempdir().unwrap();
        let downloader = Downloader::new().unwrap();
        let mut models = crate::catalog::builtin_models().unwrap();
        models[0].download = None;
        let error = downloader
            .separator(
                &models,
                &dir.path().join("bs_roformer_inst_hyperacev2.safetensors"),
                &dir.path().join("config.yaml"),
            )
            .unwrap_err();
        assert!(error.contains("automatic download is unavailable"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(downloader
            .whisper(
                "../bad",
                &dir.path().join("c"),
                &dir.path().join("t"),
                &dir.path().join("w")
            )
            .is_err());
    }
}
