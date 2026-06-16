//! Integration tests for `aikit batch start` / `aikit batch changed` and help text.
//!
//! Each test builds a throwaway Git repository in a temp directory and runs the
//! compiled `aikit` binary inside it. Coverage maps to the Batch 1 manifest's
//! test expectations (help availability, anchor creation + JSON fields, blocked
//! states, output-root selection, changed-file detection, exclusions, determinism).

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command as AssertCommand;
use serde_json::Value;
use tempfile::TempDir;

/// Run `git` in `dir`, asserting success. Global/system git config is neutralized
/// so tests are hermetic regardless of the developer's environment.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(args)
        .status()
        .expect("failed to run git");
    assert!(
        status.success(),
        "git {:?} failed in {}",
        args,
        dir.display()
    );
}

/// Create a temp Git repo with one committed file, returning the temp dir.
fn init_repo() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let p = dir.path();
    git(p, &["init", "-q"]);
    git(p, &["config", "user.email", "test@example.com"]);
    git(p, &["config", "user.name", "Test User"]);
    git(p, &["config", "commit.gpgsign", "false"]);
    fs::write(p.join("README.md"), "initial\n").unwrap();
    git(p, &["add", "README.md"]);
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

/// Find the single anchor json under .aikit/outputs/batches (fallback location).
fn find_anchor(dir: &Path) -> std::path::PathBuf {
    let batches = dir.join(".aikit/outputs/batches");
    let entry = fs::read_dir(&batches)
        .expect("batches dir exists")
        .next()
        .expect("at least one anchor")
        .expect("dir entry");
    entry.path()
}

/// `aikit batch start`, then pause so later edits have an mtime strictly newer than the
/// anchor file (the timestamp reference). Files written before this stay older and are
/// excluded by timestamp-based discovery. Returns the anchor file path.
fn anchor_then_wait(dir: &Path) -> std::path::PathBuf {
    aikit(dir).args(["batch", "start"]).assert().success();
    let anchor = find_anchor(dir);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    anchor
}

// ---- Help availability ----

#[test]
fn root_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("batch"));
}

#[test]
fn batch_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["batch", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("start"))
        .stdout(predicates::str::contains("changed"));
}

#[test]
fn batch_start_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["batch", "start", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("anchor"))
        .stdout(predicates::str::contains("--json"));
}

#[test]
fn batch_changed_help_is_available() {
    let out = AssertCommand::new(cargo_bin("aikit"))
        .args(["batch", "changed", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&out);
    assert!(help.contains("--anchor"));
    assert!(help.contains("--hash"));
    // Timestamp-based discovery is described; the removed flags are gone.
    assert!(help.to_lowercase().contains("timestamp"));
    assert!(
        !help.contains("--include-untracked"),
        "the deprecated --include-untracked flag must be gone"
    );
    assert!(
        !help.contains("--tracked-only"),
        "the deprecated --tracked-only flag must be gone"
    );
}

// ---- batch start ----

#[test]
fn batch_start_creates_anchor_with_expected_fields() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Batch anchor created"));

    let anchor_path = find_anchor(repo.path());
    let json: Value = serde_json::from_str(&fs::read_to_string(&anchor_path).unwrap()).unwrap();

    for field in [
        "schema_version",
        "kind",
        "anchor_id",
        "created_at",
        "repo_root",
        "git_head",
        "git_branch",
        "filesystem_anchor_time",
        "aikit_version",
    ] {
        assert!(json.get(field).is_some(), "anchor missing field: {field}");
    }
    assert_eq!(json["kind"], "aikit.batch_anchor");
    assert_eq!(json["schema_version"], 1);
    assert!(json["git_head"].as_str().unwrap().len() >= 7);
    // The anchor is a minimal timestamp reference and must NOT capture Git status.
    assert!(
        json.get("git_status_porcelain").is_none(),
        "anchor must not record git_status_porcelain"
    );
}

#[test]
fn batch_start_json_anchor_has_no_git_status() {
    let repo = init_repo();
    // A dirty tree so any captured status would be non-empty if it existed.
    fs::write(repo.path().join("README.md"), "initial\ndirty\n").unwrap();
    let out = aikit(repo.path())
        .args(["batch", "start", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).expect("stdout is JSON");
    assert!(
        json["anchor"].get("git_status_porcelain").is_none(),
        "batch start --json anchor must not include git_status_porcelain"
    );
}

#[test]
fn batch_start_json_outputs_machine_readable() {
    let repo = init_repo();
    let out = aikit(repo.path())
        .args(["batch", "start", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).expect("stdout is JSON");
    assert!(json.get("anchor_path").is_some());
    assert_eq!(json["anchor"]["kind"], "aikit.batch_anchor");
}

#[test]
fn batch_start_fails_outside_git_repo_with_blocked_state() {
    let dir = TempDir::new().unwrap();
    aikit(dir.path())
        .args(["batch", "start"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

// ---- output-root selection ----

#[test]
fn batch_start_default_output_is_aikit_outputs() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success();
    assert!(
        repo.path().join(".aikit/outputs/batches").is_dir(),
        "default anchor output under .aikit/outputs/batches"
    );
}

#[test]
fn batch_start_default_ignores_scratch_even_when_present() {
    let repo = init_repo();
    // The presence of .scratch/work/outputs/ must NOT change the default root.
    fs::create_dir_all(repo.path().join(".scratch/work/outputs")).unwrap();
    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success();
    assert!(
        repo.path().join(".aikit/outputs/batches").is_dir(),
        "default output stays under .aikit/outputs even when .scratch exists"
    );
    assert!(
        !repo.path().join(".scratch/work/outputs/aikit").exists(),
        ".scratch must never be auto-selected for output"
    );
}

#[test]
fn batch_start_output_override_is_honored() {
    let repo = init_repo();
    // .scratch is opt-in only via --output.
    aikit(repo.path())
        .args(["batch", "start", "--output", ".scratch/work/outputs/aikit"])
        .assert()
        .success();
    assert!(
        repo.path()
            .join(".scratch/work/outputs/aikit/batches")
            .is_dir(),
        "explicit --output should be used as requested"
    );
    assert!(
        !repo.path().join(".aikit/outputs").exists(),
        "default .aikit/outputs should not be created when --output is given"
    );
}

#[test]
fn batch_start_relative_output_resolves_under_repo_root_from_subdir() {
    let repo = init_repo();
    let subdir = repo.path().join("sub");
    fs::create_dir_all(&subdir).unwrap();
    // Run from a subdirectory; a relative --output must resolve under the repo root.
    aikit(&subdir)
        .args(["batch", "start", "--output", "out"])
        .assert()
        .success();
    assert!(
        repo.path().join("out/batches").is_dir(),
        "relative --output resolves under the repo root"
    );
    assert!(
        !subdir.join("out").exists(),
        "relative --output must not be created under the command's cwd"
    );
}

#[test]
fn batch_start_human_output_lists_created_anchor_path() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/outputs/batches/"));
}

#[test]
fn batch_start_json_includes_created_anchor_path() {
    let repo = init_repo();
    let out = aikit(repo.path())
        .args(["batch", "start", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    let path = json["anchor_path"].as_str().unwrap();
    assert!(
        path.starts_with(".aikit/outputs/batches/"),
        "JSON anchor_path should be the created repo-relative path: {path}"
    );
}

// ---- batch changed ----

#[test]
fn batch_changed_missing_anchor_is_blocked() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "changed", "--anchor", "does-not-exist.json"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_missing_anchor"));
}

#[test]
fn batch_changed_detects_file_modified_after_anchor() {
    let repo = init_repo();
    let anchor = anchor_then_wait(repo.path());

    fs::write(repo.path().join("README.md"), "initial\nmore\n").unwrap();

    let out = aikit(repo.path())
        .args([
            "batch",
            "changed",
            "--anchor",
            anchor.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["kind"], "aikit.batch_changed");
    let files = json["files"].as_array().unwrap();
    let readme = files
        .iter()
        .find(|f| f["path"] == "README.md")
        .expect("README.md reported as changed");
    assert_eq!(readme["status"], "modified");
    // Detection is timestamp-based, never git_status.
    assert_eq!(readme["source"], "anchor_mtime");
}

#[test]
fn batch_changed_excludes_file_dirty_before_anchor() {
    let repo = init_repo();
    // README is dirty relative to HEAD BEFORE the anchor (the tree is dirty).
    fs::write(repo.path().join("README.md"), "initial\npre-anchor\n").unwrap();

    // Anchor mode must not require a clean tree.
    let anchor = anchor_then_wait(repo.path());

    // A different file is modified AFTER the anchor.
    fs::write(repo.path().join("after.txt"), "new\n").unwrap();

    let json = changed_json(repo.path(), &anchor, &[]);
    let paths: Vec<&str> = json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(
        paths.contains(&"after.txt"),
        "file modified after the anchor is reported: {paths:?}"
    );
    assert!(
        !paths.contains(&"README.md"),
        "file dirty vs HEAD but modified before the anchor is NOT reported: {paths:?}"
    );
}

#[test]
fn batch_changed_detects_untracked_file_modified_after_anchor() {
    let repo = init_repo();
    let anchor = anchor_then_wait(repo.path());

    fs::write(repo.path().join("new_file.txt"), "fresh\n").unwrap();

    let json = changed_json(repo.path(), &anchor, &[]);
    let files = json["files"].as_array().unwrap();
    let created = files
        .iter()
        .find(|f| f["path"] == "new_file.txt")
        .expect("untracked file modified after the anchor is reported");
    // Status reflects "modified after anchor"; source is timestamp-based.
    assert_eq!(created["status"], "modified");
    assert_eq!(created["source"], "anchor_mtime");
}

#[test]
fn batch_changed_excludes_aikit_output_by_default() {
    let repo = init_repo();
    // Fallback case: anchor lands in .aikit/outputs/batches and is created after start.
    let anchor = anchor_then_wait(repo.path());
    // A change after the anchor so the result set is non-empty.
    fs::write(repo.path().join("touched.txt"), "x\n").unwrap();

    let json = changed_json(repo.path(), &anchor, &[]);
    assert!(
        json["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| !f["path"].as_str().unwrap().starts_with(".aikit/")),
        "aikit's own output directory must be excluded from changed results"
    );
}

#[test]
fn batch_changed_paths_are_repo_relative_and_sorted() {
    let repo = init_repo();
    fs::write(repo.path().join("b.txt"), "b\n").unwrap();
    fs::write(repo.path().join("a.txt"), "a\n").unwrap();
    git(repo.path(), &["add", "a.txt", "b.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "add files"]);

    let anchor = anchor_then_wait(repo.path());

    fs::write(repo.path().join("b.txt"), "b2\n").unwrap();
    fs::write(repo.path().join("a.txt"), "a2\n").unwrap();

    let out = aikit(repo.path())
        .args([
            "batch",
            "changed",
            "--anchor",
            anchor.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    let paths: Vec<&str> = json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["a.txt", "b.txt"],
        "paths repo-relative and sorted"
    );
    for p in &paths {
        assert!(
            !Path::new(p).is_absolute(),
            "path should be repo-relative: {p}"
        );
    }
}

/// Run `batch changed --anchor <anchor> --json [extra...]` and parse the report.
fn changed_json(dir: &Path, anchor: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["batch", "changed", "--anchor", anchor.to_str().unwrap()];
    args.extend_from_slice(extra);
    args.push("--json");
    let out = aikit(dir)
        .args(&args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn batch_changed_does_not_report_deleted_file() {
    let repo = init_repo();
    fs::write(repo.path().join("data.txt"), "data\n").unwrap();
    git(repo.path(), &["add", "data.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "add data"]);

    let anchor = anchor_then_wait(repo.path());

    fs::remove_file(repo.path().join("data.txt")).unwrap();

    // Deleted files are out of scope for timestamp-based discovery (no content on disk).
    let json = changed_json(repo.path(), &anchor, &[]);
    assert!(
        json["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["path"] != "data.txt"),
        "a deleted file must not be reported"
    );
    assert_eq!(json["counts"]["deleted"], 0);
}

#[test]
fn batch_changed_handles_special_chars_in_path() {
    // The filesystem walk uses real paths (no porcelain parsing), so a name containing
    // " -> " is handled verbatim when it is modified after the anchor.
    let repo = init_repo();
    let weird = "a -> b.txt";
    fs::write(repo.path().join(weird), "content\n").unwrap();
    git(repo.path(), &["add", "--", weird]);
    git(repo.path(), &["commit", "-q", "-m", "add weird"]);

    let anchor = anchor_then_wait(repo.path());
    fs::write(repo.path().join(weird), "content\nmore\n").unwrap();

    let json = changed_json(repo.path(), &anchor, &[]);
    let entry = json["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == weird)
        .expect("path with an arrow reported intact");
    assert_eq!(entry["status"], "modified");
    assert_eq!(entry["source"], "anchor_mtime");
}

#[test]
fn batch_changed_rejects_foreign_anchor() {
    let repo_a = init_repo();
    aikit(repo_a.path())
        .args(["batch", "start"])
        .assert()
        .success();
    let foreign_anchor = find_anchor(repo_a.path());

    // Use repo A's anchor while operating inside repo B.
    let repo_b = init_repo();
    aikit(repo_b.path())
        .args([
            "batch",
            "changed",
            "--anchor",
            foreign_anchor.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn batch_changed_rejects_invalid_anchor_schema() {
    let repo = init_repo();
    let anchor_path = repo.path().join("bad-schema.json");
    let bad = serde_json::json!({
        "schema_version": 999,
        "kind": "aikit.batch_anchor",
        "anchor_id": "x",
        "created_at": "2026-06-12T00:00:00Z",
        "repo_root": repo.path().to_str().unwrap(),
        "git_head": "",
        "git_branch": "main",
        "filesystem_anchor_time": "2026-06-12T00:00:00Z"
    });
    fs::write(&anchor_path, serde_json::to_string_pretty(&bad).unwrap()).unwrap();

    aikit(repo.path())
        .args([
            "batch",
            "changed",
            "--anchor",
            anchor_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn batch_changed_rejects_invalid_anchor_timestamp() {
    let repo = init_repo();
    let anchor_path = repo.path().join("bad-time.json");
    let bad = serde_json::json!({
        "schema_version": 1,
        "kind": "aikit.batch_anchor",
        "anchor_id": "x",
        "created_at": "not-a-timestamp",
        "repo_root": repo.path().to_str().unwrap(),
        "git_head": "",
        "git_branch": "main",
        "filesystem_anchor_time": "not-a-timestamp"
    });
    fs::write(&anchor_path, serde_json::to_string_pretty(&bad).unwrap()).unwrap();

    aikit(repo.path())
        .args([
            "batch",
            "changed",
            "--anchor",
            anchor_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn batch_changed_untracked_older_than_anchor_is_excluded() {
    let repo = init_repo();
    // Create the untracked file BEFORE the anchor, so its mtime precedes the
    // anchor's whole-second timestamp.
    fs::write(repo.path().join("preexisting.txt"), "old\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));

    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success();
    let anchor = find_anchor(repo.path());

    let json = changed_json(repo.path(), &anchor, &[]);
    assert!(
        json["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["path"] != "preexisting.txt"),
        "a file older than the anchor must be excluded by the timestamp heuristic"
    );
}

#[test]
fn batch_changed_mtime_results_include_limitation_note() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success();
    let anchor = find_anchor(repo.path());

    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(repo.path().join("fresh.txt"), "fresh\n").unwrap();

    let json = changed_json(repo.path(), &anchor, &[]);
    let notes = json["notes"]
        .as_array()
        .expect("notes present for mtime results");
    assert!(!notes.is_empty(), "expected a limitation note");
}

// ---- batch list / batch show (Slice 4) ----

fn batches_dir(repo: &Path) -> std::path::PathBuf {
    repo.join(".aikit/outputs/batches")
}

/// Write a valid anchor JSON with the given id and head, recording this repo as its root.
fn make_anchor(repo: &Path, id: &str, head: &str) {
    let dir = batches_dir(repo);
    fs::create_dir_all(&dir).unwrap();
    let body = serde_json::json!({
        "schema_version": 1,
        "kind": "aikit.batch_anchor",
        "anchor_id": id,
        "created_at": "2026-01-01T00:00:00Z",
        "repo_root": repo.to_str().unwrap(),
        "git_head": head,
        "git_branch": "main",
        "filesystem_anchor_time": "2026-01-01T00:00:00Z",
    });
    fs::write(
        dir.join(format!("{id}.json")),
        serde_json::to_string_pretty(&body).unwrap(),
    )
    .unwrap();
}

fn json_of(repo: &Path, args: &[&str]) -> Value {
    let out = aikit(repo)
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).expect("stdout is JSON")
}

#[test]
fn batch_help_lists_list_and_show() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["batch", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("start"))
        .stdout(predicates::str::contains("changed"))
        .stdout(predicates::str::contains("list"))
        .stdout(predicates::str::contains("show"));
}

#[test]
fn batch_list_help_says_no_auto_select() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["batch", "list", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/outputs/batches/"))
        .stdout(predicates::str::contains("--json"))
        .stdout(predicates::str::contains("does NOT auto-select"));
}

#[test]
fn batch_show_help_says_no_auto_select() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["batch", "show", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("path or id"))
        .stdout(predicates::str::contains("--json"))
        .stdout(predicates::str::contains("does NOT auto-select"));
}

#[test]
fn batch_list_empty_when_no_batch_dir() {
    let repo = init_repo();
    let json = json_of(repo.path(), &["batch", "list", "--json"]);
    assert_eq!(json["kind"], "aikit.batch_list");
    assert_eq!(json["counts"]["total"], 0);
    assert_eq!(json["anchors"].as_array().unwrap().len(), 0);
    assert!(
        !batches_dir(repo.path()).exists(),
        "list must not create dirs"
    );
}

#[test]
fn batch_list_lists_valid_and_skips_invalid_sorted() {
    let repo = init_repo();
    make_anchor(repo.path(), "20260101-000000-aaaaaaa", "deadbee");
    make_anchor(repo.path(), "20260102-000000-bbbbbbb", "deadbee");
    // An invalid JSON file under the batch folder.
    fs::write(batches_dir(repo.path()).join("broken.json"), "{ not json").unwrap();
    // A non-anchor json (wrong kind).
    fs::write(
        batches_dir(repo.path()).join("wrong.json"),
        r#"{"schema_version":1,"kind":"aikit.repo_inventory"}"#,
    )
    .unwrap();

    let json = json_of(repo.path(), &["batch", "list", "--json"]);
    let anchors = json["anchors"].as_array().unwrap();
    assert_eq!(anchors.len(), 2);
    // Sorted by anchor id.
    assert_eq!(anchors[0]["anchor_id"], "20260101-000000-aaaaaaa");
    assert_eq!(anchors[1]["anchor_id"], "20260102-000000-bbbbbbb");
    assert_eq!(json["counts"]["skipped"], 2);
    assert!(!json["skipped"].as_array().unwrap().is_empty());
}

#[test]
fn batch_list_does_not_auto_select_latest() {
    // batch list is read-only and reports all anchors; it must never single one out.
    let repo = init_repo();
    make_anchor(repo.path(), "20260101-000000-aaaaaaa", "deadbee");
    make_anchor(repo.path(), "20260102-000000-bbbbbbb", "deadbee");
    let json = json_of(repo.path(), &["batch", "list", "--json"]);
    assert_eq!(json["anchors"].as_array().unwrap().len(), 2);
    assert!(json.get("latest").is_none() && json.get("selected").is_none());
}

#[test]
fn batch_show_by_path_and_id() {
    let repo = init_repo();
    make_anchor(repo.path(), "anchor-x", "deadbee");
    // By id.
    let json = json_of(repo.path(), &["batch", "show", "anchor-x", "--json"]);
    assert_eq!(json["kind"], "aikit.batch_show");
    assert_eq!(json["anchor"]["anchor_id"], "anchor-x");
    assert_eq!(json["belongs_to_repo"], true);
    // By repo-relative path.
    let json2 = json_of(
        repo.path(),
        &[
            "batch",
            "show",
            ".aikit/outputs/batches/anchor-x.json",
            "--json",
        ],
    );
    assert_eq!(json2["anchor"]["anchor_id"], "anchor-x");
}

#[test]
fn batch_show_rejects_missing() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "show", "nope"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_missing_anchor"));
}

#[test]
fn batch_show_rejects_invalid_anchor() {
    let repo = init_repo();
    fs::create_dir_all(batches_dir(repo.path())).unwrap();
    fs::write(
        batches_dir(repo.path()).join("bad.json"),
        r#"{"schema_version":1,"kind":"not-an-anchor"}"#,
    )
    .unwrap();
    aikit(repo.path())
        .args(["batch", "show", "bad"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn batch_show_rejects_foreign_repo_anchor() {
    let repo = init_repo();
    let other = init_repo();
    // Anchor recorded under a different repo root.
    let dir = batches_dir(repo.path());
    fs::create_dir_all(&dir).unwrap();
    let body = serde_json::json!({
        "schema_version": 1, "kind": "aikit.batch_anchor", "anchor_id": "foreign",
        "created_at": "2026-01-01T00:00:00Z", "repo_root": other.path().to_str().unwrap(),
        "git_head": "deadbee", "git_branch": "main",
        "filesystem_anchor_time": "2026-01-01T00:00:00Z",
    });
    fs::write(dir.join("foreign.json"), body.to_string()).unwrap();
    aikit(repo.path())
        .args(["batch", "show", "foreign"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn batch_show_rejects_path_escape() {
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let anchor = outside.path().join("a.json");
    fs::write(
        &anchor,
        r#"{"schema_version":1,"kind":"aikit.batch_anchor"}"#,
    )
    .unwrap();
    aikit(repo.path())
        .args(["batch", "show", anchor.to_str().unwrap()])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[test]
fn batch_show_rejects_parent_traversal_path() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "show", "../../etc/anything.json"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[test]
fn batch_show_id_is_not_shadowed_by_stray_repo_file() {
    let repo = init_repo();
    make_anchor(repo.path(), "shadowid", "deadbee");
    // A stray repo file with the same name as the anchor id must NOT shadow id lookup.
    fs::write(repo.path().join("shadowid"), "not an anchor\n").unwrap();
    let json = json_of(repo.path(), &["batch", "show", "shadowid", "--json"]);
    assert_eq!(json["anchor"]["anchor_id"], "shadowid");
}

#[cfg(unix)]
#[test]
fn symlinked_batches_dir_is_not_followed() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    // A real anchor lives elsewhere in the repo; `.aikit/outputs/batches` is a symlink to
    // that directory. List/lookup must NOT follow the symlinked batches directory.
    let real_dir = repo.path().join("real-batches");
    fs::create_dir_all(&real_dir).unwrap();
    let body = serde_json::json!({
        "schema_version": 1, "kind": "aikit.batch_anchor", "anchor_id": "sym1",
        "created_at": "2026-01-01T00:00:00Z", "repo_root": repo.path().to_str().unwrap(),
        "git_head": "deadbee", "git_branch": "main",
        "filesystem_anchor_time": "2026-01-01T00:00:00Z",
    });
    fs::write(real_dir.join("sym1.json"), body.to_string()).unwrap();
    fs::create_dir_all(repo.path().join(".aikit/outputs")).unwrap();
    symlink(&real_dir, repo.path().join(".aikit/outputs/batches")).unwrap();

    // batch list reports an empty list (symlinked batches/ not read).
    let json = json_of(repo.path(), &["batch", "list", "--json"]);
    assert_eq!(json["counts"]["total"], 0);
    // batch show by id does not resolve through the symlinked batches/.
    aikit(repo.path())
        .args(["batch", "show", "sym1"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_missing_anchor"));
}

#[test]
fn batch_list_skips_anchor_with_invalid_timestamp() {
    let repo = init_repo();
    make_anchor(repo.path(), "good1", "deadbee");
    let dir = batches_dir(repo.path());
    let body = serde_json::json!({
        "schema_version": 1, "kind": "aikit.batch_anchor", "anchor_id": "badts",
        "created_at": "not-a-time", "repo_root": repo.path().to_str().unwrap(),
        "git_head": "deadbee", "git_branch": "main",
        "filesystem_anchor_time": "not-a-time",
    });
    fs::write(dir.join("badts.json"), body.to_string()).unwrap();

    let json = json_of(repo.path(), &["batch", "list", "--json"]);
    let anchors = json["anchors"].as_array().unwrap();
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0]["anchor_id"], "good1");
    assert!(json["counts"]["skipped"].as_u64().unwrap() >= 1);
}

// ---- anchor metadata: aikit version + optional initial snapshot ----

#[test]
fn batch_start_records_aikit_version() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success();
    let anchor_path = find_anchor(repo.path());
    let json: Value = serde_json::from_str(&fs::read_to_string(&anchor_path).unwrap()).unwrap();
    let version = json["aikit_version"]
        .as_str()
        .expect("aikit_version recorded");
    assert!(!version.is_empty(), "aikit_version must be non-empty");
    // No snapshot unless requested.
    assert!(
        json.get("initial_snapshot").is_none(),
        "initial_snapshot must be absent without --snapshot"
    );
}

#[test]
fn batch_start_snapshot_records_tracked_files() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "start", "--snapshot"])
        .assert()
        .success();
    let anchor_path = find_anchor(repo.path());
    let json: Value = serde_json::from_str(&fs::read_to_string(&anchor_path).unwrap()).unwrap();
    let snapshot = json["initial_snapshot"]
        .as_array()
        .expect("initial_snapshot present with --snapshot");
    let names: Vec<String> = snapshot
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        names.contains(&"README.md".to_string()),
        "snapshot lists tracked files: {names:?}"
    );
}

#[test]
fn batch_show_reports_aikit_version_field() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["batch", "start"])
        .assert()
        .success();
    let anchor_path = find_anchor(repo.path());
    let id = anchor_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let out = aikit(repo.path())
        .args(["batch", "show", &id, "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert!(
        json["anchor"].get("aikit_version").is_some(),
        "batch show anchor view exposes aikit_version"
    );
}
