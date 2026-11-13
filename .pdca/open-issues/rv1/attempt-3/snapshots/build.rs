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
