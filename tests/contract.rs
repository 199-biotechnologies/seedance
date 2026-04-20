//! Integration tests for the core framework contract.
//!
//! These verify the rules every agent-cli-framework CLI must obey:
//!   - exit codes match the documented contract (0, 2, 3)
//!   - `agent-info` lists every command that is actually routable
//!   - `--help` exits 0 even when piped
//!   - error envelope shape is { version, status, error: { code, message, suggestion } }

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

fn bin() -> Command {
    Command::cargo_bin("seedance").unwrap()
}

/// Return a Command isolated from the user's real config file. Prevents
/// tests from picking up an API key set via `seedance config set`.
fn isolated_bin(tmp: &tempfile::TempDir) -> Command {
    let mut c = bin();
    // `directories` resolves ~/Library/Application Support on macOS and
    // ~/.config on Linux from HOME. Point HOME at an empty tmp dir so no
    // real config is found.
    c.env("HOME", tmp.path());
    c.env("XDG_CONFIG_HOME", tmp.path());
    c.env_remove("SEEDANCE_API_KEY");
    c.env_remove("ARK_API_KEY");
    c
}

#[test]
fn agent_info_is_valid_json() {
    let output = bin().arg("agent-info").output().unwrap();
    assert!(output.status.success(), "agent-info must exit 0");
    let v: Value = serde_json::from_slice(&output.stdout).expect("agent-info emits JSON");
    assert_eq!(v["name"], "seedance");
    assert!(v["commands"].is_object());
    assert!(v["exit_codes"].is_object());
    assert_eq!(v["api"]["default_model"], "dreamina-seedance-2-0-260128");
    assert_eq!(v["api"]["fast_model"], "dreamina-seedance-2-0-fast-260128");
}

#[test]
fn every_agent_info_command_is_routable() {
    let info = bin().arg("agent-info").output().unwrap();
    let v: Value = serde_json::from_slice(&info.stdout).unwrap();
    let commands = v["commands"].as_object().unwrap();
    for cmd_path in commands.keys() {
        // Skip the self-referential one; it only has --help
        if cmd_path == "agent-info" {
            continue;
        }
        // Split "config show" -> ["config", "show"] etc.
        let parts: Vec<&str> = cmd_path.split_whitespace().collect();
        let mut c = bin();
        for p in &parts {
            c.arg(p);
        }
        c.arg("--help");
        let out = c.output().unwrap();
        assert!(
            out.status.success(),
            "`seedance {cmd_path} --help` should exit 0, got {:?}. stderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn help_exits_zero_even_when_piped() {
    let output = bin().arg("--help").output().unwrap();
    assert!(output.status.success(), "--help must exit 0");
    // Piped output wraps in a success envelope.
    let v: Value = serde_json::from_slice(&output.stdout).expect("JSON envelope");
    assert_eq!(v["status"], "success");
    assert!(v["data"]["usage"].is_string());
}

#[test]
fn version_exits_zero() {
    let output = bin().arg("--version").output().unwrap();
    assert!(output.status.success(), "--version must exit 0");
}

#[test]
fn missing_subcommand_exits_three() {
    let output = bin().output().unwrap();
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn generate_with_no_inputs_is_bad_input() {
    let output = bin().arg("generate").output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    // Error goes to stderr in JSON envelope form when piped.
    let v: Value = serde_json::from_slice(&output.stderr).expect("error JSON on stderr");
    assert_eq!(v["status"], "error");
    assert_eq!(v["error"]["code"], "invalid_input");
    assert!(v["error"]["suggestion"].is_string());
    assert!(v["error"]["message"].is_string());
}

#[test]
fn generate_with_out_of_range_duration_fails() {
    let output = bin()
        .args(["generate", "--prompt", "hi", "--duration", "999"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(v["error"]["code"], "invalid_input");
    assert!(v["error"]["message"].as_str().unwrap().contains("duration"));
}

#[test]
fn generate_audio_only_references_rejected() {
    let output = bin()
        .args([
            "generate",
            "--prompt",
            "hi",
            "--audio",
            "https://example.com/a.wav",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("audio cannot be the only reference")
    );
}

#[test]
fn generate_too_many_images_rejected() {
    let mut c = bin();
    c.args(["generate", "--prompt", "hi"]);
    for i in 0..10 {
        c.args(["--image", &format!("https://example.com/{i}.png")]);
    }
    let output = c.output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("too many reference images")
    );
}

#[test]
fn generate_video_local_path_rejected() {
    // Write a temp file and pass it -- video local paths must be rejected
    // regardless of whether the file exists.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    let output = bin()
        .args(["generate", "--prompt", "hi", "--video", path])
        .env("SEEDANCE_API_KEY", "sk-test")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("video input requires a public URL")
    );
}

#[test]
fn missing_api_key_is_config_error() {
    let tmp = tempfile::tempdir().unwrap();
    let output = isolated_bin(&tmp)
        .args(["generate", "--prompt", "hi"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let v: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(v["error"]["code"], "config_error");
    assert!(v["error"]["message"].as_str().unwrap().contains("API key"));
}

#[test]
fn models_returns_seedance_2_as_default() {
    let output = bin().arg("models").output().unwrap();
    assert!(output.status.success());
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    let models = v["data"].as_array().unwrap();
    assert!(
        models
            .iter()
            .any(|m| m["id"] == "dreamina-seedance-2-0-260128")
    );
    assert!(
        models
            .iter()
            .any(|m| m["id"] == "dreamina-seedance-2-0-fast-260128")
    );
}

#[test]
fn config_show_masks_api_key() {
    let output = bin().args(["config", "show"]).output().unwrap();
    assert!(output.status.success());
    let body = String::from_utf8(output.stdout).unwrap();
    // With no config + no env, api_key should be absent, not leaked.
    assert!(!body.contains("sk-"));
}

#[test]
fn clap_error_wraps_in_json_envelope_when_piped() {
    let output = bin().arg("--nonexistent-flag").output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&output.stderr).expect("json on stderr");
    assert_eq!(v["status"], "error");
    assert_eq!(v["error"]["code"], "invalid_input");
}

#[test]
fn envelope_includes_version_field() {
    let output = bin().arg("models").output().unwrap();
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["version"], "1");
    assert_eq!(v["status"], "success");
    assert!(v["data"].is_array());
}

#[test]
fn quiet_suppresses_stderr_only_for_human_ctx() {
    // With --json, --quiet should never touch stdout; JSON always emits.
    let output = bin()
        .args(["--json", "--quiet", "models"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["status"], "success");
}

#[test]
fn download_requires_task_id() {
    let output = bin().arg("download").output().unwrap();
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn aliases_work() {
    // gen -> generate, ls -> models, info -> agent-info, get -> status, rm -> cancel
    for (alias, expected_subcmd_help) in [
        ("gen", "Create a video"),
        ("ls", "List available"),
        ("info", ""),
        ("get", "Retrieve"),
        ("rm", "Cancel"),
    ] {
        let output = bin().args([alias, "--help"]).output().unwrap();
        assert!(
            output.status.success(),
            "alias `{alias}` should resolve and --help should exit 0"
        );
        if !expected_subcmd_help.is_empty() {
            let body = String::from_utf8(output.stdout).unwrap();
            assert!(
                body.contains(expected_subcmd_help),
                "alias `{alias}` help should contain `{expected_subcmd_help}`"
            );
        }
    }
}

#[test]
fn config_path_shape() {
    let output = bin().args(["config", "path"]).output().unwrap();
    assert!(output.status.success());
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(v["data"]["path"].is_string());
    assert!(v["data"]["exists"].is_boolean());
}

#[test]
fn doctor_fails_when_no_api_key() {
    let tmp = tempfile::tempdir().unwrap();
    let output = isolated_bin(&tmp).arg("doctor").output().unwrap();
    // doctor exits 2 if any check fails
    assert_eq!(output.status.code(), Some(2));
    // Structured report still goes to stdout before the error
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"name\": \"api_key\""));
    assert!(stdout.contains("\"status\": \"fail\""));
}

#[test]
fn config_set_api_key_round_trips() {
    let tmp = tempfile::tempdir().unwrap();

    // Set
    let out = isolated_bin(&tmp)
        .args(["config", "set", "api-key", "ark-testkey-12345678"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "config set failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["key"], "api_key");
    assert_eq!(v["data"]["action"], "set");
    assert!(
        v["data"]["value_display"]
            .as_str()
            .unwrap()
            .starts_with("ark-")
    );

    // Show should now see it -- still masked
    let out = isolated_bin(&tmp)
        .args(["config", "show"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["data"]["api_key"].is_string());
    assert!(!v["data"]["api_key"].as_str().unwrap().contains("testkey"));

    // Unset clears it
    let out = isolated_bin(&tmp)
        .args(["config", "unset", "api-key"])
        .output()
        .unwrap();
    assert!(out.status.success());
}

// Sanity check: predicate-based assertion for the no-input case.
#[test]
fn no_input_error_contains_helpful_message() {
    bin().arg("generate").assert().failure().code(3).stderr(
        predicate::str::contains("provide at least --prompt")
            .or(predicate::str::contains("provide at least")),
    );
}
