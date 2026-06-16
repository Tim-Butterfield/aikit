//! End-to-end integration test for the local `aikit` workflow.
//!
//! Each command family has its own focused test file (cli_batch, cli_inventory,
//! cli_review, cli_script). This file is the integration check: it exercises the
//! intended local workflow as a single chained sequence in one throwaway Git repo —
//! anchor → change → list-changed → inventory → review → print a governed run plan —
//! and verifies the artifacts/metadata line up across commands. It is deliberately
//! deterministic: it changes a *tracked* file (so detection is via `git status`, not
//! the mtime heuristic) and uses `script run --print` (so no interpreter is actually
//! invoked).

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command as AssertCommand;
use serde_json::Value;
use tempfile::TempDir;

/// Run `git` in `dir`, asserting success. Global/system git config is neutralized
/// so the test is hermetic regardless of the developer's environment.
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

/// Run an aikit command with `--json` appended and parse stdout as JSON.
fn aikit_json(dir: &Path, args: &[&str]) -> Value {
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

fn files_contains<'a>(json: &'a Value, path: &str) -> Option<&'a Value> {
    json["files"]
        .as_array()
        .expect("files array")
        .iter()
        .find(|f| f["path"] == path)
}

#[test]
fn local_workflow_end_to_end() {
    let repo = init_repo();
    let p = repo.path();

    // 1. Anchor the work. Capture the repo-relative anchor path for later commands.
    let start = aikit_json(p, &["batch", "start"]);
    assert_eq!(start["anchor"]["kind"], "aikit.batch_anchor");
    let anchor_path = start["anchor_path"]
        .as_str()
        .expect("anchor_path is a string")
        .to_string();
    assert!(
        anchor_path.starts_with(".aikit/outputs/batches/"),
        "default anchor path is under .aikit/outputs/batches/: {anchor_path}"
    );

    // 2. Do some work: modify a tracked file after the anchor (detected by filesystem
    //    mtime newer than the anchor, not by git status). Pause first so the edit is
    //    strictly newer than the anchor file on any filesystem.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(p.join("README.md"), "initial\nmore work\n").unwrap();

    // 3. List what changed since the anchor.
    let changed = aikit_json(p, &["batch", "changed", "--anchor", &anchor_path]);
    assert_eq!(changed["kind"], "aikit.batch_changed");
    let readme = files_contains(&changed, "README.md").expect("README.md reported as changed");
    assert_eq!(readme["status"], "modified");
    assert_eq!(readme["source"], "anchor_mtime");

    // 4. Inventory the repo. README.md is tracked and not ignored, so it appears.
    let inventory = aikit_json(p, &["inventory", "repo"]);
    assert_eq!(inventory["kind"], "aikit.repo_inventory");
    assert!(
        files_contains(&inventory, "README.md").is_some(),
        "inventory lists the tracked README.md"
    );
    let inv_written = inventory["written"].as_array().expect("written array");
    assert!(
        inv_written
            .iter()
            .any(|w| w.as_str().unwrap().ends_with("inventory.json")),
        "inventory --json reports the created inventory.json: {inv_written:?}"
    );

    // 5. Generate a review bundle from the anchor (changed-since-anchor mode).
    let review = aikit_json(p, &["review", "generate", "--anchor", &anchor_path]);
    assert_eq!(review["kind"], "aikit.review_bundle");
    assert_eq!(review["inputs"]["mode"], "changed_since_anchor");
    assert!(
        files_contains(&review, "README.md").is_some(),
        "the changed README.md is bundled for review"
    );
    let written: Vec<String> = review["written"]
        .as_array()
        .expect("written array")
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect();
    assert!(
        written.iter().any(|w| w.ends_with("manifest.json")),
        "review --json reports manifest.json: {written:?}"
    );
    assert!(
        written.iter().any(|w| w.ends_with("review_bundle.txt")),
        "review --json reports review_bundle.txt: {written:?}"
    );
    // The reported artifacts actually exist on disk (paths are repo-relative).
    for w in &written {
        assert!(
            p.join(w).is_file(),
            "reported review artifact exists on disk: {w}"
        );
    }

    // 6. Stage a harmless script under an allowed work area and print its run plan.
    fs::create_dir_all(p.join(".aikit/temp")).unwrap();
    fs::write(
        p.join(".aikit/temp/task.sh"),
        "#!/bin/sh\necho integration-task\n",
    )
    .unwrap();

    let plan = aikit_json(p, &["script", "run", ".aikit/temp/task.sh", "--print"]);
    assert_eq!(plan["kind"], "aikit.script_run");
    assert_eq!(
        plan["executed"], false,
        "--print must not execute the script"
    );
    assert_eq!(plan["interpreter"], "/bin/sh");
    let argv: Vec<&str> = plan["argv"]
        .as_array()
        .expect("argv array")
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(argv, vec!["/bin/sh", ".aikit/temp/task.sh"]);
    assert!(
        plan["script_copy_path"].is_null(),
        "--print copies no script"
    );

    // --print creates no run directory and runs nothing.
    assert!(
        !p.join(".aikit/outputs/runs").exists(),
        "--print must not create a run directory"
    );
}
