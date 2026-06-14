//! Integration tests for `aikit diff anchor` (Slice 4).
//!
//! `diff anchor` is a mechanical, read-only Git diff from an anchor's recorded head to
//! the current working tree. It validates the anchor, rejects foreign/escaped/missing
//! anchors and missing base commits, excludes untracked file contents, and creates no
//! review bundle or output artifact.

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

fn head(dir: &Path) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Write a valid anchor JSON recording this repo's root and the given head.
fn make_anchor(repo: &Path, id: &str, git_head: &str) {
    let dir = repo.join(".aikit/outputs/batches");
    fs::create_dir_all(&dir).unwrap();
    let body = serde_json::json!({
        "schema_version": 1, "kind": "aikit.batch_anchor", "anchor_id": id,
        "created_at": "2026-01-01T00:00:00Z", "repo_root": repo.to_str().unwrap(),
        "git_head": git_head, "git_branch": "main", "git_status_porcelain": "",
        "filesystem_anchor_time": "2026-01-01T00:00:00Z",
    });
    fs::write(dir.join(format!("{id}.json")), body.to_string()).unwrap();
}

fn diff_json(repo: &Path, args: &[&str]) -> Value {
    let mut full = vec!["diff", "anchor"];
    full.extend_from_slice(args);
    full.push("--json");
    let out = aikit(repo)
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
fn diff_help_lists_anchor() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["diff", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("anchor"));
}

#[test]
fn diff_anchor_help_documents_behavior() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["diff", "anchor", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("anchor id"))
        .stdout(predicates::str::contains("base"))
        .stdout(predicates::str::contains("Untracked"))
        .stdout(predicates::str::contains("--json"))
        .stdout(predicates::str::contains("--patch"));
}

// ---- validation ----

#[test]
fn diff_anchor_rejects_missing() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["diff", "anchor", "nope"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_missing_anchor"));
}

#[test]
fn diff_anchor_rejects_invalid() {
    let repo = init_repo();
    let dir = repo.path().join(".aikit/outputs/batches");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("bad.json"),
        r#"{"schema_version":1,"kind":"nope"}"#,
    )
    .unwrap();
    aikit(repo.path())
        .args(["diff", "anchor", "bad"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn diff_anchor_rejects_foreign_repo() {
    let repo = init_repo();
    let other = init_repo();
    let dir = repo.path().join(".aikit/outputs/batches");
    fs::create_dir_all(&dir).unwrap();
    let body = serde_json::json!({
        "schema_version": 1, "kind": "aikit.batch_anchor", "anchor_id": "foreign",
        "created_at": "2026-01-01T00:00:00Z", "repo_root": other.path().to_str().unwrap(),
        "git_head": head(other.path()), "git_branch": "main", "git_status_porcelain": "",
        "filesystem_anchor_time": "2026-01-01T00:00:00Z",
    });
    fs::write(dir.join("foreign.json"), body.to_string()).unwrap();
    aikit(repo.path())
        .args(["diff", "anchor", "foreign"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn diff_anchor_rejects_path_escape() {
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let anchor = outside.path().join("a.json");
    fs::write(
        &anchor,
        r#"{"schema_version":1,"kind":"aikit.batch_anchor"}"#,
    )
    .unwrap();
    aikit(repo.path())
        .args(["diff", "anchor", anchor.to_str().unwrap()])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[test]
fn diff_anchor_rejects_missing_base_commit() {
    let repo = init_repo();
    make_anchor(
        repo.path(),
        "ghost",
        "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
    );
    aikit(repo.path())
        .args(["diff", "anchor", "ghost"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_missing_base_commit"));
}

// ---- diff content ----

#[test]
fn diff_anchor_reports_committed_changes() {
    let repo = init_repo();
    let base = head(repo.path());
    make_anchor(repo.path(), "a1", &base);
    // Commit a change after the anchor.
    fs::write(repo.path().join("README.md"), "# readme\nmore\n").unwrap();
    git(repo.path(), &["commit", "-q", "-am", "change"]);

    let json = diff_json(repo.path(), &["a1"]);
    assert_eq!(json["kind"], "aikit.diff_anchor");
    assert_eq!(json["base_git_head"], base);
    let files = json["files"].as_array().unwrap();
    let readme = files.iter().find(|f| f["path"] == "README.md").unwrap();
    assert_eq!(readme["status"], "modified");
    assert!(json["counts"]["total"].as_u64().unwrap() >= 1);
    // diff anchor creates no review bundle / output dir.
    assert!(!repo.path().join(".aikit/outputs/reviews").exists());
    assert!(!repo.path().join(".aikit/outputs/runs").exists());
}

#[test]
fn diff_anchor_reports_worktree_changes() {
    let repo = init_repo();
    make_anchor(repo.path(), "a1", &head(repo.path()));
    // Uncommitted tracked change.
    fs::write(repo.path().join("README.md"), "# readme\nedited\n").unwrap();

    let json = diff_json(repo.path(), &["a1"]);
    assert_eq!(json["tracked_tree_clean"], false);
    let files = json["files"].as_array().unwrap();
    assert!(files.iter().any(|f| f["path"] == "README.md"));
}

#[test]
fn diff_anchor_excludes_untracked_contents_and_notes() {
    let repo = init_repo();
    make_anchor(repo.path(), "a1", &head(repo.path()));
    // An untracked new file must not appear in the Git diff file list.
    fs::write(repo.path().join("brand-new.txt"), "hi\n").unwrap();

    let json = diff_json(repo.path(), &["a1"]);
    let files = json["files"].as_array().unwrap();
    assert!(
        files.iter().all(|f| f["path"] != "brand-new.txt"),
        "untracked file must not be in the diff"
    );
    let notes = json["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n.as_str().unwrap().contains("Untracked")),
        "expected an untracked-file note"
    );
}

#[test]
fn diff_anchor_patch_flag_includes_patch() {
    let repo = init_repo();
    make_anchor(repo.path(), "a1", &head(repo.path()));
    fs::write(repo.path().join("README.md"), "# readme\npatched\n").unwrap();

    let no_patch = diff_json(repo.path(), &["a1"]);
    assert!(
        no_patch.get("patch").is_none(),
        "patch omitted without --patch"
    );

    let with_patch = diff_json(repo.path(), &["a1", "--patch"]);
    assert!(with_patch["patch"].as_str().unwrap().contains("README.md"));
}

#[test]
fn diff_anchor_clean_tree_has_no_files() {
    let repo = init_repo();
    make_anchor(repo.path(), "a1", &head(repo.path()));
    // No changes since the anchor (HEAD == base, clean tree).
    let json = diff_json(repo.path(), &["a1"]);
    assert_eq!(json["counts"]["total"], 0);
    assert_eq!(json["tracked_tree_clean"], true);
}
