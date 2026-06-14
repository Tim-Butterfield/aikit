//! Integration tests for the `aikit output` command family (`list` / `show` / `clean`).
//!
//! Artifacts are created directly under `.aikit/outputs/<family>/` with realistic
//! shapes so the tests are deterministic and do not depend on the other commands.
//! `list` and `show` must be read-only; `clean` must be dry-run by default and delete
//! only known artifacts under the output root with `--execute` + a selector.

use std::fs;
use std::path::{Path, PathBuf};
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

fn json_out(dir: &Path, args: &[&str]) -> Value {
    let mut full = args.to_vec();
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

fn outputs(repo: &Path) -> PathBuf {
    repo.join(".aikit/outputs")
}

/// Create a batch anchor artifact `batches/<id>.json`.
fn make_batch(repo: &Path, id: &str) {
    let dir = outputs(repo).join("batches");
    fs::create_dir_all(&dir).unwrap();
    let body = format!(
        r#"{{"schema_version":1,"kind":"aikit.batch_anchor","anchor_id":"{id}","git_head":"deadbee"}}"#
    );
    fs::write(dir.join(format!("{id}.json")), body).unwrap();
}

/// Create a directory artifact `<family>/<id>/` with a main json + companion file.
fn make_dir_artifact(repo: &Path, family: &str, id: &str, main: &str, kind: &str, idfield: &str) {
    let dir = outputs(repo).join(family).join(id);
    fs::create_dir_all(&dir).unwrap();
    let body = format!(r#"{{"schema_version":1,"kind":"{kind}","{idfield}":"{id}"}}"#);
    fs::write(dir.join(main), body).unwrap();
    fs::write(dir.join("companion.txt"), "data\n").unwrap();
}

fn make_all(repo: &Path) {
    make_batch(repo, "b1");
    make_dir_artifact(
        repo,
        "inventory",
        "inv1",
        "inventory.json",
        "aikit.repo_inventory",
        "inventory_id",
    );
    make_dir_artifact(
        repo,
        "reviews",
        "rev1",
        "manifest.json",
        "aikit.review_bundle",
        "review_id",
    );
    make_dir_artifact(
        repo,
        "runs",
        "run1",
        "run.json",
        "aikit.script_run",
        "run_id",
    );
}

/// Set an old mtime (year 2000) via `touch -t`, so age filters treat it as old.
fn set_old(path: &Path) {
    let status = Command::new("touch")
        .args(["-t", "200001010000"])
        .arg(path)
        .status()
        .expect("touch");
    assert!(status.success(), "touch failed for {}", path.display());
}

// ---- help ----

#[test]
fn output_help_lists_subcommands() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["output", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("list"))
        .stdout(predicates::str::contains("show"))
        .stdout(predicates::str::contains("clean"));
}

#[test]
fn output_list_help_documents_default_and_json() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["output", "list", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/outputs/"))
        .stdout(predicates::str::contains("--json"))
        .stdout(predicates::str::contains("--family"));
}

#[test]
fn output_show_help_documents_artifact_and_json() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["output", "show", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("artifact"))
        .stdout(predicates::str::contains("--json"));
}

#[test]
fn output_clean_help_documents_safety() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["output", "clean", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("dry-run"))
        .stdout(predicates::str::contains("--execute"))
        .stdout(predicates::str::contains("--older-than"))
        .stdout(predicates::str::contains("--all"));
}

// ---- output list ----

#[test]
fn list_empty_when_no_outputs() {
    let repo = init_repo();
    let json = json_out(repo.path(), &["output", "list"]);
    assert_eq!(json["kind"], "aikit.output_list");
    assert_eq!(json["counts"]["total"], 0);
    assert_eq!(json["artifacts"].as_array().unwrap().len(), 0);
    // Read-only: no output dir is created.
    assert!(!outputs(repo.path()).exists());
}

#[test]
fn list_finds_all_families_sorted_and_is_read_only() {
    let repo = init_repo();
    make_all(repo.path());
    let json = json_out(repo.path(), &["output", "list"]);
    let arts = json["artifacts"].as_array().unwrap();
    assert_eq!(arts.len(), 4);
    let families: Vec<&str> = arts.iter().map(|a| a["family"].as_str().unwrap()).collect();
    // Sorted by family order batches, inventory, reviews, runs.
    assert_eq!(families, vec!["batches", "inventory", "reviews", "runs"]);
    assert_eq!(json["counts"]["batches"], 1);
    assert_eq!(json["counts"]["runs"], 1);
    assert_eq!(arts[0]["artifact_type"], "file");
    assert_eq!(arts[3]["artifact_type"], "dir");

    // Read-only: artifacts unchanged after listing.
    aikit(repo.path())
        .args(["output", "list"])
        .assert()
        .success();
    assert!(outputs(repo.path()).join("batches/b1.json").exists());
    assert!(outputs(repo.path()).join("runs/run1").is_dir());
}

#[test]
fn list_family_filter() {
    let repo = init_repo();
    make_all(repo.path());
    let json = json_out(repo.path(), &["output", "list", "--family", "runs"]);
    let arts = json["artifacts"].as_array().unwrap();
    assert_eq!(arts.len(), 1);
    assert_eq!(arts[0]["family"], "runs");
    assert_eq!(arts[0]["artifact_id"], "run1");
}

// ---- output show ----

#[test]
fn show_batch_by_id_and_path() {
    let repo = init_repo();
    make_all(repo.path());
    // By id.
    let json = json_out(repo.path(), &["output", "show", "b1"]);
    assert_eq!(json["kind"], "aikit.output_show");
    assert_eq!(json["artifact"]["family"], "batches");
    assert_eq!(json["metadata"]["kind"], "aikit.batch_anchor");
    // By repo-relative path.
    let json2 = json_out(
        repo.path(),
        &["output", "show", ".aikit/outputs/batches/b1.json"],
    );
    assert_eq!(json2["artifact"]["artifact_id"], "b1");
}

#[test]
fn show_run_inventory_review() {
    let repo = init_repo();
    make_all(repo.path());
    let run = json_out(repo.path(), &["output", "show", "run1"]);
    assert_eq!(run["artifact"]["family"], "runs");
    assert_eq!(run["metadata"]["kind"], "aikit.script_run");
    let files: Vec<&str> = run["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(files.contains(&"run.json"));

    let inv = json_out(repo.path(), &["output", "show", "inv1"]);
    assert_eq!(inv["metadata"]["kind"], "aikit.repo_inventory");
    let rev = json_out(repo.path(), &["output", "show", "rev1"]);
    assert_eq!(rev["metadata"]["kind"], "aikit.review_bundle");
}

#[test]
fn show_missing_artifact_is_blocked() {
    let repo = init_repo();
    make_all(repo.path());
    aikit(repo.path())
        .args(["output", "show", "does-not-exist"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_artifact_not_found"));
}

#[test]
fn show_rejects_path_outside_output_root() {
    let repo = init_repo();
    make_all(repo.path());
    // A real file in the repo but outside .aikit/outputs/.
    aikit(repo.path())
        .args(["output", "show", "README.md"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[cfg(unix)]
#[test]
fn show_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    make_all(repo.path());
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("secret.json");
    fs::write(&target, "{}").unwrap();
    let link = outputs(repo.path()).join("batches/link.json");
    symlink(&target, &link).unwrap();
    aikit(repo.path())
        .args(["output", "show", ".aikit/outputs/batches/link.json"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

// ---- output clean ----

#[test]
fn clean_default_and_dry_run_delete_nothing() {
    let repo = init_repo();
    make_all(repo.path());
    // Default (no selector): dry-run, lists candidates, deletes nothing.
    let def = json_out(repo.path(), &["output", "clean"]);
    assert_eq!(def["dry_run"], true);
    assert_eq!(def["counts"]["deleted"], 0);
    assert!(def["counts"]["candidates"].as_u64().unwrap() >= 1);
    // --all --dry-run: still deletes nothing.
    let dry = json_out(repo.path(), &["output", "clean", "--all", "--dry-run"]);
    assert_eq!(dry["dry_run"], true);
    assert_eq!(dry["counts"]["deleted"], 0);
    assert_eq!(dry["deleted"].as_array().unwrap().len(), 0);
    // Everything still present.
    assert!(outputs(repo.path()).join("batches/b1.json").exists());
    assert!(outputs(repo.path()).join("runs/run1").is_dir());
}

#[test]
fn clean_execute_without_selector_is_invalid_usage() {
    let repo = init_repo();
    make_all(repo.path());
    aikit(repo.path())
        .args(["output", "clean", "--execute"])
        .assert()
        .failure()
        .code(2);
    // Nothing deleted.
    assert!(outputs(repo.path()).join("batches/b1.json").exists());
}

#[test]
fn clean_older_than_and_all_together_is_invalid_usage() {
    let repo = init_repo();
    aikit(repo.path())
        .args([
            "output",
            "clean",
            "--all",
            "--older-than",
            "7d",
            "--dry-run",
        ])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn clean_all_execute_deletes_known_artifacts_only() {
    let repo = init_repo();
    make_all(repo.path());
    // A non-artifact file directly in the output root, and protected local dirs.
    fs::write(outputs(repo.path()).join("stray.txt"), "keep\n").unwrap();
    fs::create_dir_all(repo.path().join(".aikit/temp")).unwrap();
    fs::write(repo.path().join(".aikit/temp/keep.sh"), "#!/bin/sh\n").unwrap();
    fs::create_dir_all(repo.path().join(".scratch/work")).unwrap();
    fs::create_dir_all(repo.path().join("target/debug")).unwrap();

    let json = json_out(repo.path(), &["output", "clean", "--all", "--execute"]);
    assert_eq!(json["execute"], true);
    assert_eq!(json["counts"]["deleted"], 4);

    // Known artifacts gone.
    assert!(!outputs(repo.path()).join("batches/b1.json").exists());
    assert!(!outputs(repo.path()).join("runs/run1").exists());
    // Family dirs remain.
    assert!(outputs(repo.path()).join("batches").is_dir());
    assert!(outputs(repo.path()).join("runs").is_dir());
    // Non-artifact and protected paths untouched.
    assert!(outputs(repo.path()).join("stray.txt").exists());
    assert!(repo.path().join(".aikit/temp/keep.sh").exists());
    assert!(repo.path().join(".scratch/work").is_dir());
    assert!(repo.path().join("target/debug").is_dir());
}

#[test]
fn clean_family_filter_execute() {
    let repo = init_repo();
    make_all(repo.path());
    let json = json_out(
        repo.path(),
        &["output", "clean", "--family", "runs", "--all", "--execute"],
    );
    assert_eq!(json["counts"]["deleted"], 1);
    assert!(!outputs(repo.path()).join("runs/run1").exists());
    // Other families remain.
    assert!(outputs(repo.path()).join("batches/b1.json").exists());
    assert!(outputs(repo.path()).join("inventory/inv1").is_dir());
}

#[test]
fn clean_older_than_filters_by_age() {
    let repo = init_repo();
    make_all(repo.path());
    // Make only the run artifact old.
    set_old(&outputs(repo.path()).join("runs/run1"));

    // Dry-run: candidates limited to the old artifact; nothing deleted.
    let dry = json_out(
        repo.path(),
        &["output", "clean", "--older-than", "7d", "--dry-run"],
    );
    let cands = dry["candidates"].as_array().unwrap();
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0]["artifact_id"], "run1");
    assert_eq!(dry["counts"]["deleted"], 0);
    assert!(outputs(repo.path()).join("runs/run1").is_dir());

    // Execute: deletes only the old one; fresh artifacts remain.
    let exec = json_out(
        repo.path(),
        &["output", "clean", "--older-than", "7d", "--execute"],
    );
    assert_eq!(exec["counts"]["deleted"], 1);
    assert!(!outputs(repo.path()).join("runs/run1").exists());
    assert!(outputs(repo.path()).join("batches/b1.json").exists());
}

#[test]
fn clean_rejects_invalid_duration() {
    let repo = init_repo();
    make_all(repo.path());
    aikit(repo.path())
        .args(["output", "clean", "--older-than", "5w", "--execute"])
        .assert()
        .failure();
    // Nothing deleted.
    assert!(outputs(repo.path()).join("batches/b1.json").exists());
}

#[test]
fn output_root_rejects_protected_or_non_output_roots() {
    let repo = init_repo();
    make_all(repo.path());
    fs::create_dir_all(repo.path().join("target/runs/x")).unwrap();
    // --root pointing at protected / non-output directories must be rejected.
    for bad in ["target", ".git", ".", ".aikit", ".scratch"] {
        aikit(repo.path())
            .args(["output", "clean", "--root", bad, "--all", "--execute"])
            .assert()
            .failure()
            .code(3)
            .stderr(predicates::str::contains("blocked_path_escape"));
    }
    // The protected dir's contents are untouched.
    assert!(repo.path().join("target/runs/x").is_dir());
    // The canonical output root is accepted.
    let json = json_out(
        repo.path(),
        &[
            "output",
            "clean",
            "--root",
            ".aikit/outputs",
            "--all",
            "--dry-run",
        ],
    );
    assert_eq!(json["dry_run"], true);
}

#[test]
fn clean_rejects_overflowing_duration() {
    let repo = init_repo();
    make_all(repo.path());
    // A value that fits in u64 but overflows when multiplied to seconds must be rejected
    // (not silently wrapped to a tiny duration that would delete fresh artifacts).
    aikit(repo.path())
        .args([
            "output",
            "clean",
            "--older-than",
            "300000000000000d",
            "--execute",
        ])
        .assert()
        .failure();
    assert!(outputs(repo.path()).join("batches/b1.json").exists());
    assert!(outputs(repo.path()).join("runs/run1").is_dir());
}

#[cfg(unix)]
#[test]
fn show_does_not_follow_symlinked_metadata_file() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    // A real in-root run artifact whose run.json is a symlink to a file outside the repo.
    let dir = outputs(repo.path()).join("runs/sym");
    fs::create_dir_all(&dir).unwrap();
    let outside = TempDir::new().unwrap();
    let secret = outside.path().join("secret.json");
    fs::write(&secret, r#"{"kind":"SECRET","leaked":true}"#).unwrap();
    symlink(&secret, dir.join("run.json")).unwrap();

    let json = json_out(repo.path(), &["output", "show", "sym"]);
    // The artifact resolves (it is a real in-root dir), but its symlinked metadata is
    // not followed — metadata is omitted rather than leaking outside content.
    assert!(
        json["metadata"].is_null(),
        "symlinked metadata must not be read"
    );
}

#[cfg(unix)]
#[test]
fn clean_does_not_follow_symlink_escape() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    make_all(repo.path());
    let outside = TempDir::new().unwrap();
    let target_dir = outside.path().join("precious");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("file.txt"), "precious\n").unwrap();
    // A symlinked "run" artifact pointing outside the repo.
    symlink(&target_dir, outputs(repo.path()).join("runs/evil")).unwrap();

    let json = json_out(repo.path(), &["output", "clean", "--all", "--execute"]);
    // The symlink is not a known artifact, so it is not a candidate and not deleted.
    let ids: Vec<&str> = json["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["artifact_id"].as_str().unwrap())
        .collect();
    assert!(
        !ids.contains(&"evil"),
        "symlink must not be a clean candidate"
    );
    // The symlink target outside the repo is untouched.
    assert!(target_dir.join("file.txt").exists());
}
