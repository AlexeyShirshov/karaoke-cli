use std::fs;
use std::path::Path;

fn quoted_field(package: &str, field: &str) -> Option<String> {
    let prefix = format!("{field} = \"");
    package.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix('\"'))
            .map(str::to_owned)
    })
}

fn locked_package(lock: &str, package_name: &str) -> Option<(String, Option<String>)> {
    let mut found = None;
    for package in lock.split("[[package]]").skip(1) {
        if quoted_field(package, "name").as_deref() != Some(package_name) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        let version = quoted_field(package, "version")?;
        let source = quoted_field(package, "source");
        found = Some((version, source));
    }
    found
}

fn source_kind(source: Option<&str>) -> &'static str {
    match source {
        Some("registry+https://github.com/rust-lang/crates.io-index") => "crates.io registry",
        Some(source) if source.starts_with("registry+") => "registry",
        Some(source) if source.starts_with("git+") => "git",
        Some(_) => "other source",
        None => "local path",
    }
}

fn main() {
    configure_cuda_dlls();
    println!("cargo:rerun-if-changed=Cargo.lock");
    let lock_path = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.lock");
    let lock = fs::read_to_string(lock_path).expect("failed to read Cargo.lock");
    for (package, version_variable, source_variable) in [
        (
            "candle-roformer",
            "CANDLE_ROFORMER_VERSION",
            Some("CANDLE_ROFORMER_SOURCE"),
        ),
        ("candle-core", "CANDLE_CORE_VERSION", None),
    ] {
        let (version, source) = locked_package(&lock, package)
            .unwrap_or_else(|| panic!("Cargo.lock must contain one {package} package entry"));
        println!("cargo:rustc-env={version_variable}={version}");
        if let Some(source_variable) = source_variable {
            println!(
                "cargo:rustc-env={source_variable}={}",
                source_kind(source.as_deref())
            );
        }
    }
}

fn configure_cuda_dlls() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var_os("CARGO_FEATURE_CUDA").is_none()
    {
        return;
    }

    // CUDA imports must not be resolved until main has added our bin directory
    // to the DLL search path. Keep nvcuda.dll driver-provided and startup-linked.
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=CUDA_ROOT");
    let toolkit = std::env::var_os("CUDA_PATH")
        .or_else(|| std::env::var_os("CUDA_ROOT"))
        .expect("Windows CUDA builds require CUDA_PATH or CUDA_ROOT");
    let bin = Path::new(&toolkit).join("bin");
    // CUDA 13 uses bin/x64; CUDA 12 keeps DLLs directly in bin.
    let bin = if bin.join("x64").is_dir() {
        bin.join("x64")
    } else {
        bin
    };
    println!("cargo:rerun-if-changed={}", bin.display());
    let dlls: Vec<_> = fs::read_dir(&bin)
        .expect("failed to read CUDA Toolkit bin directory")
        .map(|entry| {
            entry
                .expect("failed to read CUDA Toolkit directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    for prefix in [
        "cublas64_",
        "cublasLt64_",
        "curand64_",
        "cudart64_",
        "nvrtc64_",
    ] {
        let mut found = false;
        for dll in &dlls {
            if dll.starts_with(prefix) && dll.ends_with(".dll") && !dll.ends_with(".alt.dll") {
                println!("cargo:rustc-link-arg-bin=karaoke-cli=/DELAYLOAD:{dll}");
                found = true;
            }
        }
        assert!(
            found,
            "missing CUDA runtime {prefix}*.dll in {}",
            bin.display()
        );
    }
    println!("cargo:rustc-link-arg-bin=karaoke-cli=delayimp.lib");
}
