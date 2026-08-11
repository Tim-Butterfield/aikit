//! Integration tests for `aikit --version` and the `aikit version` command.

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command as AssertCommand;
use serde_json::Value;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn dash_dash_version_prints_package_version() {
    AssertCommand::new(cargo_bin("aikit"))
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(PKG_VERSION));
}

#[test]
fn version_command_succeeds_and_is_compact() {
    AssertCommand::new(cargo_bin("aikit"))
        .arg("version")
        .assert()
        .success()
        .stdout(predicates::str::contains("aikit"))
        .stdout(predicates::str::contains(PKG_VERSION));
}

#[test]
fn version_json_has_expected_fields() {
    let out = AssertCommand::new(cargo_bin("aikit"))
        .args(["version", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).expect("version --json is valid JSON");
    for field in [
        "schema_version",
        "kind",
        "name",
        "version",
        "git_commit",
        "build_profile",
        "os",
        "arch",
        "target",
        "rust_profile",
    ] {
        assert!(
            json.get(field).is_some(),
            "version JSON missing field: {field}"
        );
    }
    assert_eq!(json["kind"], "aikit.version");
    assert_eq!(json["name"], "aikit");
    assert_eq!(json["version"], PKG_VERSION);
    assert!(json["os"].as_str().is_some());
    assert!(json["arch"].as_str().is_some());
}
