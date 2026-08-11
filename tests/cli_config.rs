//! Integration tests for `aikit config show`.
//!
//! The command exists so layered configuration is explainable without file archaeology, so
//! what these pin is the provenance: which layer set each effective value.

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command as AssertCommand;
use serde_json::Value;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(args)
        .status()
        .expect("failed to run git");
    assert!(status.success(), "git {args:?} failed");
}

fn init_repo() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    dir
}

fn aikit(dir: &Path) -> AssertCommand {
    let mut cmd = AssertCommand::new(cargo_bin("aikit"));
    cmd.current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    cmd
}

fn show_json(dir: &Path) -> Value {
    let out = aikit(dir)
        .args(["config", "show", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).expect("stdout is JSON")
}

/// The reported source for one setting key.
fn source_of<'a>(json: &'a Value, key: &str) -> &'a str {
    json["settings"]
        .as_array()
        .expect("settings array")
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("setting {key} not reported"))["source"]
        .as_str()
        .expect("source is a string")
}

fn value_of<'a>(json: &'a Value, key: &str) -> &'a Value {
    &json["settings"]
        .as_array()
        .expect("settings array")
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("setting {key} not reported"))["value"]
}

#[test]
fn with_no_config_files_every_value_is_a_default() {
    let repo = init_repo();
    let json = show_json(repo.path());
    assert_eq!(json["kind"], "aikit.config_show");
    assert_eq!(json["sources"].as_array().unwrap().len(), 0);

    for setting in json["settings"].as_array().unwrap() {
        assert_eq!(
            setting["source"], "default",
            "with no config files nothing can have another source: {setting}"
        );
    }
}

#[test]
fn the_winning_layer_is_named_per_value() {
    // This is the whole point: two files, one overriding a single key, and the report must
    // attribute each value to the layer that actually set it — not to the last file loaded.
    let repo = init_repo();
    let p = repo.path();
    fs::create_dir_all(p.join(".aikit")).unwrap();
    fs::write(
        p.join("aikit.config.json"),
        r#"{ "script_runner": { "detect_from_shebang": false, "preferred_runners": ["bash"] } }"#,
    )
    .unwrap();
    fs::write(
        p.join(".aikit/config.json"),
        r#"{ "script_runner": { "detect_from_shebang": true } }"#,
    )
    .unwrap();

    let json = show_json(p);
    assert_eq!(
        json["sources"],
        serde_json::json!(["aikit.config.json", ".aikit/config.json"]),
        "sources are reported in precedence order"
    );

    // Set only by the root file, so it keeps that attribution even though a later file loaded.
    assert_eq!(
        source_of(&json, "script_runner.preferred_runners"),
        "aikit.config.json"
    );
    // Set by both; the later file wins, and the report says so.
    assert_eq!(
        source_of(&json, "script_runner.detect_from_shebang"),
        ".aikit/config.json"
    );
    assert_eq!(
        value_of(&json, "script_runner.detect_from_shebang"),
        &json!(true)
    );
    // Untouched by either file.
    assert_eq!(
        source_of(&json, "script_runner.detect_from_extension"),
        "default"
    );
}

use serde_json::json;

#[test]
fn retention_default_is_reported_and_overridable() {
    let repo = init_repo();
    let p = repo.path();

    let json = show_json(p);
    assert_eq!(value_of(&json, "output.retain_runs"), &json!(100));
    assert_eq!(source_of(&json, "output.retain_runs"), "default");

    fs::create_dir_all(p.join(".aikit")).unwrap();
    fs::write(
        p.join(".aikit/config.json"),
        r#"{ "output": { "retain_runs": 7 } }"#,
    )
    .unwrap();

    let json = show_json(p);
    assert_eq!(value_of(&json, "output.retain_runs"), &json!(7));
    assert_eq!(source_of(&json, "output.retain_runs"), ".aikit/config.json");
}

#[test]
fn show_is_read_only_and_says_mcp_ignores_config() {
    let repo = init_repo();
    let p = repo.path();

    let out = aikit(p)
        .args(["config", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    // The CLI-consults-config / MCP-does-not split is the thing most likely to be
    // misread, so the command that explains configuration has to state it.
    assert!(
        text.contains("MCP") && text.contains("does NOT consult"),
        "config show should state that MCP ignores this configuration: {text}"
    );

    // Read-only: no directories are created by asking what the configuration is.
    assert!(!p.join(".aikit/outputs").exists());
    assert!(!p.join(".aikit/temp").exists());
}
