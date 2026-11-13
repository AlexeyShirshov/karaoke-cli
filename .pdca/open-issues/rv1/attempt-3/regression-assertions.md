# AC-5 regression assertions

The helper at `src/main.rs:662-681` (source snapshot `.pdca/open-issues/rv1/attempt-3/snapshots/src/main.rs:662-681`, identical to `624d456`) reads the named package fields from `Cargo.lock`. The test at `src/main.rs:781-816` compares the reported `candle-roformer` version and source, and `candle-core` version, to those lockfile values; it also asserts that the misleading dependency revision label is absent.

Verbatim helper and regression test from `624d456:src/main.rs`:

```rust
    fn cargo_lock_field(package_name: &str, field: &str) -> String {
        let prefix = format!("{field} = \"");
        include_str!("../Cargo.lock")
            .split("[[package]]")
            .skip(1)
            .find(|package| {
                package
                    .lines()
                    .any(|line| line.trim() == format!("name = \"{package_name}\""))
            })
            .and_then(|package| {
                package.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix(&prefix)
                        .and_then(|value| value.strip_suffix('\"'))
                        .map(str::to_owned)
                })
            })
            .unwrap_or_else(|| panic!("Cargo.lock has no {field} for {package_name}"))
    }

    #[test]
    fn env_info_reports_versions_features_and_resolved_paths_without_loading_assets() {
        let report = env_info_report(Some("/tmp/model-root"), Some("separator.safetensors"), None);
        let locked_roformer_version = cargo_lock_field("candle-roformer", "version");
        let locked_roformer_source = cargo_lock_field("candle-roformer", "source");
        let locked_core_version = cargo_lock_field("candle-core", "version");
        let source_label =
            if locked_roformer_source == "registry+https://github.com/rust-lang/crates.io-index" {
                "crates.io registry"
            } else if locked_roformer_source.starts_with("registry+") {
                "registry"
            } else if locked_roformer_source.starts_with("git+") {
                "git"
            } else {
                "other source"
            };
        assert!(report.contains(&format!("CLI version: {VERSION}")));
        assert!(report.contains(&format!(
            "candle-roformer version: {locked_roformer_version} ({source_label})"
        )));
        assert_eq!(CANDLE_ROFORMER_VERSION, locked_roformer_version);
        assert_eq!(CANDLE_ROFORMER_SOURCE, source_label);
        assert!(report.contains(&format!("candle-core version: {locked_core_version}")));
        assert_eq!(CANDLE_CORE_VERSION, locked_core_version);
        assert!(!report.contains("candle-roformer revision:"));
        assert!(report.contains(&format!(
            "compiled features: cuda={}, metal={}",
            cfg!(feature = "cuda"),
            cfg!(feature = "metal")
        )));
        assert!(report.contains("available parallelism:"));
        assert!(report.contains("CPU device: available"));
        assert!(report.contains("resolved model checkpoint: /tmp/model-root/separator.safetensors"));
        assert!(report.contains("resolved model config: /tmp/model-root/separator.yaml"));
        assert!(!report.contains("API_KEY"));
    }
```
