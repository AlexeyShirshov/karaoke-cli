use std::fs;
use std::path::Path;
use std::process::Command;

fn locked_package_version(lock: &str, package_name: &str) -> Option<String> {
    let mut in_package = false;
    let mut name_matches = false;
    for line in lock.lines() {
        if line.trim() == "[[package]]" {
            if in_package && name_matches {
                return None;
            }
            in_package = true;
            name_matches = false;
            continue;
        }
        if !in_package {
            continue;
        }
        let line = line.trim();
        if line == format!("name = \"{package_name}\"") {
            name_matches = true;
        } else if name_matches && line.starts_with("version = ") {
            return line
                .strip_prefix("version = \"")
                .and_then(|s| s.strip_suffix('\"'))
                .map(str::to_owned);
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=Cargo.lock");
    let lock_path = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.lock");
    let lock = fs::read_to_string(lock_path).unwrap_or_default();
    for (package, variable) in [
        ("candle-roformer", "CANDLE_ROFORMER_VERSION"),
        ("candle-core", "CANDLE_CORE_VERSION"),
    ] {
        let version = locked_package_version(&lock, package).unwrap_or_else(|| "unknown".into());
        println!("cargo:rustc-env={variable}={version}");
    }
    let revision = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .current_dir(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=CANDLE_ROFORMER_GIT_REVISION={revision}");
}
