//! Integration tests for `aikit env snapshot` (Slice 5).
//!
//! `env snapshot` is a read-only, mechanical local environment report. It reports a
//! bounded set of debugging facts, works inside or outside a Git repo, creates nothing,
//! and never dumps the full environment or secret-looking environment values.

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
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn init_repo() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let p = dir.path();
    git(p, &["init", "-q"]);
    git(p, &["config", "user.email", "test@example.com"]);
    git(p, &["config", "user.name", "Test User"]);
    git(p, &["config", "commit.gpgsign", "false"]);
    fs::write(p.join("README.md"), "# readme\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "initial"]);
    dir
}

fn aikit(dir: &Path) -> AssertCommand {
    let mut cmd = AssertCommand::new(cargo_bin("aikit"));
    cmd.current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    cmd
}

fn snapshot_json(dir: &Path) -> Value {
    let out = aikit(dir)
        .args(["env", "snapshot", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).expect("stdout is JSON")
}

// ---- help ----

#[test]
fn env_help_lists_snapshot() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["env", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("snapshot"));
}

#[test]
fn env_snapshot_help_documents_behavior() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["env", "snapshot", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("environment"))
        .stdout(predicates::str::contains("--json"))
        .stdout(predicates::str::contains(
            "does NOT dump all environment variables",
        ));
}

// ---- behavior ----

#[test]
fn env_snapshot_succeeds_in_repo() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["env", "snapshot"])
        .assert()
        .success()
        .stdout(predicates::str::contains("aikit env snapshot"));
}

#[test]
fn env_snapshot_json_reports_core_facts() {
    let repo = init_repo();
    let json = snapshot_json(repo.path());
    assert_eq!(json["kind"], "aikit.env_snapshot");
    assert!(!json["version"].as_str().unwrap().is_empty());
    assert!(!json["os"].as_str().unwrap().is_empty());
    assert!(!json["arch"].as_str().unwrap().is_empty());
    assert!(json["current_exe"].as_str().is_some());
    // Interpreter availability is reported.
    let interps = json["interpreters"].as_array().unwrap();
    assert!(interps.iter().any(|i| i["path"] == "/bin/sh"));
    // Repo facts are present inside a repo.
    let repo_facts = &json["repo"];
    assert!(repo_facts["root"].as_str().is_some());
    assert!(repo_facts["branch"].as_str().is_some());
    assert!(repo_facts["head"].as_str().is_some());
    assert_eq!(repo_facts["default_output_root"], ".aikit/outputs");
}

#[test]
fn env_snapshot_path_summary_is_a_count_not_raw_path() {
    let repo = init_repo();
    let json = snapshot_json(repo.path());
    // A safe summary: a count, never the raw PATH string.
    assert!(json["paths"]["path_entry_count"].as_u64().is_some());
    let text = serde_json::to_string(&json).unwrap();
    // The full PATH env value must not appear verbatim.
    if let Ok(path_var) = std::env::var("PATH") {
        if path_var.contains(':') {
            assert!(
                !text.contains(&path_var),
                "raw PATH must not be emitted in the snapshot"
            );
        }
    }
}

#[test]
fn env_snapshot_creates_no_files() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["env", "snapshot", "--json"])
        .assert()
        .success();
    // Read-only: no aikit working area is created.
    assert!(!repo.path().join(".aikit").exists());
    assert!(!repo.path().join(".scratch").exists());
}

#[test]
fn env_snapshot_does_not_write_git_index() {
    let repo = init_repo();
    let index = repo.path().join(".git/index");
    // Force the index stat-cache stale by rewriting a tracked file's identical bytes
    // (content unchanged → tree stays clean; mtime changes → a plain `git status` would
    // refresh and rewrite .git/index). The dirty probe must NOT rewrite it.
    fs::write(repo.path().join("README.md"), "# readme\n").unwrap();
    let before = fs::read(&index).unwrap();
    aikit(repo.path())
        .args(["env", "snapshot", "--json"])
        .assert()
        .success();
    let after = fs::read(&index).unwrap();
    assert_eq!(before, after, "env snapshot must not rewrite .git/index");
}

#[test]
fn env_snapshot_does_not_dump_secret_env_values() {
    let repo = init_repo();
    let out = aikit(repo.path())
        .args(["env", "snapshot", "--json"])
        .env("AIKIT_TEST_SECRET", "supersecret-do-not-leak-1234567890")
        .env("AWS_SECRET_ACCESS_KEY", "leakycredentialvalue0987654321")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(!text.contains("supersecret-do-not-leak-1234567890"));
    assert!(!text.contains("leakycredentialvalue0987654321"));
    // And the variable names themselves are not dumped either.
    assert!(!text.contains("AIKIT_TEST_SECRET"));
    assert!(!text.contains("AWS_SECRET_ACCESS_KEY"));
}

#[test]
fn env_snapshot_works_outside_a_git_repo() {
    let plain = TempDir::new().unwrap();
    let json = snapshot_json(plain.path());
    assert_eq!(json["kind"], "aikit.env_snapshot");
    // Outside a repo, repo facts are null and a warning is recorded.
    assert!(json["repo"].is_null());
    let warnings = json["warnings"].as_array().unwrap();
    assert!(warnings
        .iter()
        .any(|w| w.as_str().unwrap().contains("not inside a Git repository")));
}
