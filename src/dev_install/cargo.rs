//! `cargo build` + built-artifact path resolution for `dev-install`.

use std::path::PathBuf;
use std::process::Command;

/// Build the `agentflare` binary from the current source tree.
///
/// Inherits stdio so the user sees cargo's progress/errors directly.
pub(crate) fn build(release: bool) -> Result<(), String> {
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "-p", "agentflare", "--bin", "agentflare"]);
    if release {
        cmd.arg("--release");
    }
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run cargo build: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("cargo build failed".to_string())
    }
}

/// Resolve the path to the freshly built binary via `cargo metadata`'s
/// `target_directory` (rather than assuming `./target`, which is wrong under a
/// custom `CARGO_TARGET_DIR` or a workspace with a shared target).
pub(crate) fn built_binary_path(release: bool) -> Result<PathBuf, String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("failed to run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let json = String::from_utf8_lossy(&out.stdout);
    let target_dir = parse_target_directory(&json)
        .ok_or_else(|| "no target_directory in cargo metadata output".to_string())?;
    Ok(PathBuf::from(target_dir)
        .join(profile_dir(release))
        .join(binary_name()))
}

fn profile_dir(release: bool) -> &'static str {
    if release { "release" } else { "debug" }
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "agentflare.exe"
    } else {
        "agentflare"
    }
}

/// Extract `target_directory` from `cargo metadata` JSON. Pure so it can be
/// unit-tested without invoking cargo.
fn parse_target_directory(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("target_directory")?.as_str().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_directory_reads_the_field() {
        let json = r#"{"packages":[],"target_directory":"/repo/target","version":1}"#;
        assert_eq!(
            parse_target_directory(json).as_deref(),
            Some("/repo/target")
        );
    }

    #[test]
    fn parse_target_directory_missing_field_is_none() {
        assert_eq!(parse_target_directory(r#"{"version":1}"#), None);
        assert_eq!(parse_target_directory("not json"), None);
    }

    #[test]
    fn profile_dir_maps_release_flag() {
        assert_eq!(profile_dir(true), "release");
        assert_eq!(profile_dir(false), "debug");
    }
}
