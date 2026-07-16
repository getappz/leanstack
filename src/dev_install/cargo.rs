//! `cargo build` + built-artifact discovery for `dev-install`.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Build the `agentflare` binary from the current source tree and return the
/// path cargo actually wrote it to.
///
/// Reads the executable path from cargo's `compiler-artifact` JSON message
/// rather than reconstructing `target_directory/<profile>/agentflare`: that
/// reconstruction is wrong whenever `build.target` / `CARGO_BUILD_TARGET` adds a
/// `<triple>/` segment, or a custom profile changes the directory name. Human
/// progress and diagnostics still stream to stderr.
pub(crate) fn build_and_locate(release: bool) -> Result<PathBuf, String> {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "build",
        "-p",
        "agentflare",
        "--bin",
        "agentflare",
        "--message-format",
        "json-render-diagnostics",
    ]);
    if release {
        cmd.arg("--release");
    }

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to run cargo build: {e}"))?;

    // stderr is inherited (live progress); stdout is the JSON stream we parse.
    // Only stdout is a pipe, so draining it fully cannot deadlock.
    let mut json = String::new();
    child
        .stdout
        .take()
        .expect("stdout was piped")
        .read_to_string(&mut json)
        .map_err(|e| format!("reading cargo output: {e}"))?;

    let status = child.wait().map_err(|e| format!("waiting on cargo: {e}"))?;
    if !status.success() {
        return Err("cargo build failed".to_string());
    }

    parse_executable_path(&json)
        .ok_or_else(|| "cargo build did not report an agentflare executable".to_string())
}

/// Find the `agentflare` binary path in cargo's JSON build output. Pure, so it
/// is unit-testable without invoking cargo. Returns the last matching
/// `compiler-artifact` executable (there is normally exactly one).
fn parse_executable_path(build_json: &str) -> Option<PathBuf> {
    let mut found = None;
    for line in build_json.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
            continue;
        }
        let name = v
            .get("target")
            .and_then(|t| t.get("name"))
            .and_then(serde_json::Value::as_str);
        if name != Some("agentflare") {
            continue;
        }
        if let Some(exe) = v.get("executable").and_then(serde_json::Value::as_str) {
            found = Some(PathBuf::from(exe));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_executable_path_reads_the_agentflare_artifact_under_a_target_triple() {
        // The path carries a `<triple>/` segment (build.target set) — exactly the
        // case the reconstructed `target/<profile>/` lookup got wrong.
        let json = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"serde"},"executable":null}"#,
            "\n",
            r#"{"reason":"compiler-artifact","target":{"name":"agentflare"},"executable":"/repo/target/x86_64-unknown-linux-gnu/release/agentflare"}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
            "\n",
        );
        assert_eq!(
            parse_executable_path(json),
            Some(PathBuf::from(
                "/repo/target/x86_64-unknown-linux-gnu/release/agentflare"
            ))
        );
    }

    #[test]
    fn parse_executable_path_none_when_no_agentflare_executable() {
        let json = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"agentflare"},"executable":null}"#,
            "\n",
            "not json\n",
        );
        assert_eq!(parse_executable_path(json), None);
    }
}
