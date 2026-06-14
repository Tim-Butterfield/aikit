//! Integration tests for `aikit scan secrets` (Slice 5).
//!
//! `scan secrets` is a local, best-effort heuristic scan over explicit repo-local paths.
//! It rejects path escapes, skips binary/oversized files, never prints raw secret values
//! (human or JSON), creates no artifacts, exits 0 by default even with findings, and
//! exits 3 (blocked_secret_findings) under `--fail-on-findings`.

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command as AssertCommand;
use serde_json::Value;
use tempfile::TempDir;

/// A fake long token value used across tests. It is deliberately not a real credential;
/// every test asserts this string never appears in command output.
const FAKE_TOKEN: &str = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEF";

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

fn scan_json(dir: &Path, args: &[&str]) -> Value {
    let mut full = vec!["scan", "secrets"];
    full.extend_from_slice(args);
    full.push("--json");
    let out = aikit(dir)
        .args(&full)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).expect("stdout is JSON")
}

// ---- help ----

#[test]
fn scan_help_lists_secrets() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["scan", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("secrets"));
}

#[test]
fn scan_secrets_help_documents_behavior() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["scan", "secrets", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("explicit"))
        .stdout(predicates::str::contains("--json"))
        .stdout(predicates::str::contains("best-effort"))
        .stdout(predicates::str::contains("raw secret values"))
        .stdout(predicates::str::contains("--fail-on-findings"));
}

// ---- usage / path safety ----

#[test]
fn scan_secrets_requires_a_path() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["scan", "secrets"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn scan_secrets_blocks_outside_a_git_repo() {
    let plain = TempDir::new().unwrap();
    aikit(plain.path())
        .args(["scan", "secrets", "."])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

#[test]
fn scan_secrets_rejects_path_outside_repo() {
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("secret.txt");
    fs::write(&target, "password = hunter2value\n").unwrap();
    aikit(repo.path())
        .args(["scan", "secrets", target.to_str().unwrap()])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[test]
fn scan_secrets_rejects_parent_traversal() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["scan", "secrets", "../escape.txt"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[cfg(unix)]
#[test]
fn scan_secrets_rejects_symlink_escape() {
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let secret = outside.path().join("secret.txt");
    fs::write(&secret, "password = hunter2value\n").unwrap();
    let link = repo.path().join("link.txt");
    std::os::unix::fs::symlink(&secret, &link).unwrap();
    aikit(repo.path())
        .args(["scan", "secrets", "link.txt"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

// ---- detection (no raw values emitted) ----

#[test]
fn scan_secrets_detects_private_key_without_printing_it() {
    let repo = init_repo();
    let secret_body = "KEYMATERIALabcdef0123456789SHOULDNOTLEAK";
    fs::write(
        repo.path().join("id_rsa"),
        format!("-----BEGIN OPENSSH PRIVATE KEY-----\n{secret_body}\n-----END OPENSSH PRIVATE KEY-----\n"),
    )
    .unwrap();
    let json = scan_json(repo.path(), &["id_rsa"]);
    let findings = json["findings"].as_array().unwrap();
    assert!(findings.iter().any(|f| f["rule_id"] == "private_key_block"));
    let text = serde_json::to_string(&json).unwrap();
    assert!(!text.contains(secret_body), "key material must not leak");
}

#[test]
fn scan_secrets_detects_credential_assignment_without_value() {
    let repo = init_repo();
    fs::write(
        repo.path().join("config.txt"),
        "password = hunter2secretpw\n",
    )
    .unwrap();
    let json = scan_json(repo.path(), &["config.txt"]);
    let findings = json["findings"].as_array().unwrap();
    let f = findings
        .iter()
        .find(|f| f["path"] == "config.txt")
        .expect("a finding");
    assert!(f["line"].as_u64().is_some());
    assert!(f["rule_id"].as_str().is_some());
    assert_eq!(f["redacted"], true);
    let text = serde_json::to_string(&json).unwrap();
    assert!(!text.contains("hunter2secretpw"), "value must not leak");
}

#[test]
fn scan_secrets_detects_long_token_assignment_without_value() {
    let repo = init_repo();
    fs::write(
        repo.path().join("creds.env"),
        format!("API_KEY={FAKE_TOKEN}\n"),
    )
    .unwrap();
    let json = scan_json(repo.path(), &["creds.env"]);
    let findings = json["findings"].as_array().unwrap();
    assert!(findings
        .iter()
        .any(|f| f["rule_id"] == "long_token_assignment" && f["severity"] == "high"));
    let text = serde_json::to_string(&json).unwrap();
    assert!(!text.contains(FAKE_TOKEN), "token must not leak");
}

#[test]
fn scan_secrets_reports_path_and_line() {
    let repo = init_repo();
    fs::write(
        repo.path().join("multi.txt"),
        format!("clean line\nharmless\napi_key = {FAKE_TOKEN}\n"),
    )
    .unwrap();
    let json = scan_json(repo.path(), &["multi.txt"]);
    let f = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "multi.txt")
        .unwrap()
        .clone();
    assert_eq!(f["line"], 3);
}

// ---- exit behavior ----

#[test]
fn scan_secrets_default_exits_zero_with_findings() {
    let repo = init_repo();
    fs::write(repo.path().join("c.txt"), format!("token = {FAKE_TOKEN}\n")).unwrap();
    // No --json here: default human output, default exit 0 even with findings.
    aikit(repo.path())
        .args(["scan", "secrets", "c.txt"])
        .assert()
        .success();
}

#[test]
fn scan_secrets_fail_on_findings_exits_three() {
    let repo = init_repo();
    fs::write(repo.path().join("c.txt"), format!("token = {FAKE_TOKEN}\n")).unwrap();
    let out = aikit(repo.path())
        .args(["scan", "secrets", "c.txt", "--fail-on-findings"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_secret_findings"))
        .get_output()
        .stdout
        .clone();
    // The report still prints to stdout, with values redacted.
    let text = String::from_utf8_lossy(&out);
    assert!(!text.contains(FAKE_TOKEN));
}

#[test]
fn scan_secrets_fail_on_findings_clean_exits_zero() {
    let repo = init_repo();
    fs::write(repo.path().join("clean.txt"), "nothing to see here\n").unwrap();
    aikit(repo.path())
        .args(["scan", "secrets", "clean.txt", "--fail-on-findings"])
        .assert()
        .success();
}

// ---- traversal / skipping ----

#[test]
fn scan_secrets_skips_git_directory() {
    let repo = init_repo();
    // Plant a secret-looking value inside .git/ — it must never be scanned.
    fs::write(
        repo.path().join(".git/planted.txt"),
        format!("api_key = {FAKE_TOKEN}\n"),
    )
    .unwrap();
    let json = scan_json(repo.path(), &["."]);
    let findings = json["findings"].as_array().unwrap();
    assert!(
        findings
            .iter()
            .all(|f| !f["path"].as_str().unwrap().starts_with(".git")),
        ".git/ must be excluded"
    );
}

#[test]
fn scan_secrets_excludes_nested_git_explicit_file() {
    let repo = init_repo();
    // A nested `.git/` (as a submodule would have) holding a secret-looking value.
    let nested = repo.path().join("sub/.git");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("config"), format!("api_key = {FAKE_TOKEN}\n")).unwrap();
    // An explicit path into a nested .git/ must be excluded (always-exclude .git/).
    let json = scan_json(repo.path(), &["sub/.git/config"]);
    assert!(json["findings"].as_array().unwrap().is_empty());
    assert_eq!(json["files_scanned"], 0);
}

#[test]
fn scan_secrets_skips_binary_files() {
    let repo = init_repo();
    let mut bytes = b"api_key = ".to_vec();
    bytes.extend_from_slice(FAKE_TOKEN.as_bytes());
    bytes.push(0); // NUL byte -> binary
    bytes.extend_from_slice(b"\nmore\n");
    fs::write(repo.path().join("blob.bin"), &bytes).unwrap();
    let json = scan_json(repo.path(), &["blob.bin"]);
    assert!(json["findings"].as_array().unwrap().is_empty());
    let skipped = json["files_skipped"].as_array().unwrap();
    assert!(skipped
        .iter()
        .any(|s| s["path"] == "blob.bin" && s["reason"] == "binary"));
}

#[test]
fn scan_secrets_skips_oversized_files() {
    let repo = init_repo();
    fs::write(
        repo.path().join("big.txt"),
        format!("api_key = {FAKE_TOKEN}\npadding padding padding\n"),
    )
    .unwrap();
    let json = scan_json(repo.path(), &["big.txt", "--max-file-bytes", "10"]);
    assert!(json["findings"].as_array().unwrap().is_empty());
    let skipped = json["files_skipped"].as_array().unwrap();
    assert!(skipped
        .iter()
        .any(|s| s["path"] == "big.txt" && s["reason"] == "too_large"));
}

#[test]
fn scan_secrets_directory_findings_are_sorted() {
    let repo = init_repo();
    fs::write(repo.path().join("b.txt"), format!("token = {FAKE_TOKEN}\n")).unwrap();
    fs::write(repo.path().join("a.txt"), format!("token = {FAKE_TOKEN}\n")).unwrap();
    let json = scan_json(repo.path(), &["."]);
    let paths: Vec<String> = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "findings must be deterministically sorted");
}

// ---- ignore handling ----

#[test]
fn scan_secrets_directory_respects_gitignore_by_default() {
    let repo = init_repo();
    fs::write(repo.path().join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(
        repo.path().join("ignored.txt"),
        format!("api_key = {FAKE_TOKEN}\n"),
    )
    .unwrap();
    // Default directory traversal skips ignored files.
    let json = scan_json(repo.path(), &["."]);
    assert!(json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f["path"] != "ignored.txt"));
    // --include-ignored picks it up.
    let json2 = scan_json(repo.path(), &[".", "--include-ignored"]);
    assert!(json2["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "ignored.txt"));
}

#[test]
fn scan_secrets_scans_explicit_ignored_file() {
    let repo = init_repo();
    fs::write(repo.path().join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(
        repo.path().join("ignored.txt"),
        format!("api_key = {FAKE_TOKEN}\n"),
    )
    .unwrap();
    // An explicit ignored file is still scanned.
    let json = scan_json(repo.path(), &["ignored.txt"]);
    assert!(json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "ignored.txt"));
}

// ---- no artifacts ----

#[test]
fn scan_secrets_creates_no_artifacts() {
    let repo = init_repo();
    fs::write(repo.path().join("c.txt"), format!("token = {FAKE_TOKEN}\n")).unwrap();
    aikit(repo.path())
        .args(["scan", "secrets", "c.txt", "--json"])
        .assert()
        .success();
    assert!(!repo.path().join(".aikit/outputs").exists());
    assert!(!repo.path().join(".scratch").exists());
}
