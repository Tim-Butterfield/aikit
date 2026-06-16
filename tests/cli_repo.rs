//! Integration tests for the `aikit repo` command family (`repo init` / `repo doctor`).
//!
//! `repo init` prepares local `.aikit/temp/` and local ignore coverage (via
//! `.git/info/exclude`, never `.gitignore`); `repo doctor` reports readiness read-only.
//! Each test builds a throwaway Git repo (with no `.gitignore`, so `.aikit/` is not
//! ignored until `repo init` adds local coverage) and runs the compiled binary inside.

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

/// Run an aikit command with `--json` and parse stdout.
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

fn read_info_exclude(repo: &Path) -> String {
    fs::read_to_string(repo.join(".git/info/exclude")).unwrap_or_default()
}

// ---- help ----

#[test]
fn repo_help_lists_init_and_doctor() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["repo", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("init"))
        .stdout(predicates::str::contains("doctor"))
        .stdout(predicates::str::contains("local"));
}

#[test]
fn repo_init_help_describes_setup() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["repo", "init", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/temp/"))
        .stdout(predicates::str::contains(".git/info/exclude"))
        .stdout(predicates::str::contains(".gitignore"))
        .stdout(predicates::str::contains("idempotent"));
}

#[test]
fn repo_doctor_help_says_read_only() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["repo", "doctor", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("read-only"))
        .stdout(predicates::str::contains("readiness"))
        .stdout(predicates::str::contains("creates no files"));
}

#[test]
fn repo_doctor_help_describes_runner_availability() {
    let out = AssertCommand::new(cargo_bin("aikit"))
        .args(["repo", "doctor", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&out);
    // Must describe the runner-availability model.
    assert!(
        help.contains("runner"),
        "repo doctor --help should mention runner availability"
    );
    assert!(
        help.contains("any_runner_available") || help.contains("runners"),
        "repo doctor --help should reference the runners/any_runner_available fields"
    );
    // Must not present /bin/sh + /bin/zsh as readiness-gating interpreters.
    assert!(
        !help.contains("supported interpreters (`/bin/sh`, `/bin/zsh`)"),
        "repo doctor --help still presents /bin/sh + /bin/zsh as gating interpreters"
    );
    // If the legacy shells are mentioned at all, they must be framed as informational.
    if help.contains("/bin/zsh") {
        assert!(
            help.contains("informational"),
            "legacy /bin/sh,/bin/zsh probes must be labeled informational"
        );
    }
}

// ---- repo init ----

#[test]
fn init_fails_outside_git_repo() {
    let dir = TempDir::new().unwrap();
    aikit(dir.path())
        .args(["repo", "init"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

#[test]
fn init_creates_aikit_and_temp_only() {
    let repo = init_repo();
    let p = repo.path();
    aikit(p).args(["repo", "init"]).assert().success();
    assert!(p.join(".aikit").is_dir(), ".aikit/ created");
    assert!(p.join(".aikit/temp").is_dir(), ".aikit/temp/ created");
    assert!(
        !p.join(".aikit/outputs").exists(),
        "repo init must not create .aikit/outputs/"
    );
    assert!(
        !p.join(".scratch").exists(),
        "repo init must not create .scratch/"
    );
    assert!(
        !p.join(".claude").exists(),
        "repo init must not create .claude/"
    );
}

#[test]
fn init_adds_info_exclude_and_not_gitignore() {
    let repo = init_repo();
    let p = repo.path();
    let json = json_out(p, &["repo", "init"]);
    assert_eq!(json["kind"], "aikit.repo_init");
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["info_exclude_updated"], true);
    assert_eq!(json["ignore_source"], ".git/info/exclude");
    assert!(
        read_info_exclude(p).contains("/.aikit/"),
        ".git/info/exclude should contain /.aikit/"
    );
    assert!(
        !p.join(".gitignore").exists(),
        "repo init must not create or modify .gitignore"
    );
}

#[test]
fn init_is_idempotent_without_duplicate_ignore() {
    let repo = init_repo();
    let p = repo.path();
    // First run adds everything.
    let first = json_out(p, &["repo", "init"]);
    assert_eq!(first["info_exclude_updated"], true);
    assert!(first["created_dirs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d == ".aikit/temp"));

    // Second run is a no-op for dirs and ignore coverage.
    let second = json_out(p, &["repo", "init"]);
    assert_eq!(second["created_dirs"].as_array().unwrap().len(), 0);
    assert_eq!(second["info_exclude_updated"], false);
    assert_eq!(second["aikit_ignored"], true);

    // Exactly one /.aikit/ line in .git/info/exclude.
    let count = read_info_exclude(p)
        .lines()
        .filter(|l| l.trim() == "/.aikit/")
        .count();
    assert_eq!(count, 1, "ignore entry must not be duplicated");

    assert!(!p.join(".gitignore").exists());
}

#[test]
fn init_does_not_add_duplicate_when_already_ignored_by_gitignore() {
    let repo = init_repo();
    let p = repo.path();
    // Pre-existing .gitignore already covers .aikit/.
    fs::write(p.join(".gitignore"), "/.aikit/\n").unwrap();
    let json = json_out(p, &["repo", "init"]);
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["info_exclude_updated"], false);
    assert_eq!(json["ignore_source"], ".gitignore");
    assert!(
        !p.join(".git/info/exclude").exists() || !read_info_exclude(p).contains("/.aikit/"),
        "must not add an exclude entry when already ignored by .gitignore"
    );
}

#[test]
fn init_adds_full_coverage_when_only_a_child_is_ignored() {
    let repo = init_repo();
    let p = repo.path();
    // A pre-existing rule ignores only `.aikit/temp/`, not the whole `.aikit/`.
    fs::write(p.join(".gitignore"), "/.aikit/temp/\n").unwrap();
    let json = json_out(p, &["repo", "init"]);
    // `.aikit/` itself is not covered by the child-only rule, so init must add full
    // coverage via .git/info/exclude.
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["info_exclude_updated"], true);
    assert_eq!(json["ignore_source"], ".git/info/exclude");
    assert!(read_info_exclude(p).contains("/.aikit/"));
    // .gitignore is left as the caller wrote it.
    assert_eq!(
        fs::read_to_string(p.join(".gitignore")).unwrap(),
        "/.aikit/temp/\n"
    );
}

// ---- repo doctor ----

#[test]
fn doctor_fails_outside_git_repo() {
    let dir = TempDir::new().unwrap();
    aikit(dir.path())
        .args(["repo", "doctor"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

#[test]
fn doctor_is_read_only() {
    let repo = init_repo();
    let p = repo.path();
    aikit(p).args(["repo", "doctor"]).assert().success();
    assert!(!p.join(".aikit").exists(), "doctor must not create .aikit/");
    // `git init` creates a default `.git/info/exclude`; doctor must not add ignore
    // coverage to it (i.e. it must not write the `/.aikit/` entry that only `repo init`
    // adds).
    assert!(
        !read_info_exclude(p).contains("/.aikit/"),
        "doctor must not modify .git/info/exclude"
    );
    assert!(!p.join(".scratch").exists());
    assert!(!p.join(".claude").exists());
}

#[test]
fn doctor_reports_not_ready_before_init_and_ready_after() {
    let repo = init_repo();
    let p = repo.path();

    let before = json_out(p, &["repo", "doctor"]);
    assert_eq!(before["kind"], "aikit.repo_doctor");
    assert_eq!(before["ready"], false);
    assert_eq!(before["temp_dir_exists"], false);
    assert_eq!(before["aikit_ignored"], false);
    // doctor never makes the repo ready by itself.
    assert!(!p.join(".aikit").exists());

    aikit(p).args(["repo", "init"]).assert().success();

    let after = json_out(p, &["repo", "doctor"]);
    assert_eq!(after["temp_dir_exists"], true);
    assert_eq!(after["aikit_ignored"], true);
    assert_eq!(after["ignore_source"], ".git/info/exclude");
    // Readiness now depends on at least one supported runner being available for this
    // OS (not on any specific Unix shell). It does NOT require /bin/zsh.
    assert_eq!(after["any_runner_available"], true);
    assert_eq!(after["ready"], true);
}

#[test]
fn doctor_reports_runner_availability_not_just_shells() {
    let repo = init_repo();
    let p = repo.path();
    aikit(p).args(["repo", "init"]).assert().success();
    let json = json_out(p, &["repo", "doctor"]);

    // Runner availability is reported for every supported runner.
    let runners = json["runners"].as_array().expect("runners array present");
    let names: Vec<&str> = runners
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    for expected in [
        "sh",
        "bash",
        "zsh",
        "pwsh",
        "powershell",
        "cmd",
        "python3",
        "python",
        "node",
    ] {
        assert!(names.contains(&expected), "runners missing {expected}");
    }
    // Each entry carries availability + applicability booleans.
    for r in runners {
        assert!(r["available"].is_boolean());
        assert!(r["applicable"].is_boolean());
    }
    assert!(json["any_runner_available"].is_boolean());

    // The legacy interpreters field is retained for compatibility (informational).
    assert!(json["interpreters"].is_array());
}

#[cfg(unix)]
#[test]
fn doctor_readiness_does_not_require_zsh_on_unix() {
    let repo = init_repo();
    let p = repo.path();
    aikit(p).args(["repo", "init"]).assert().success();
    let json = json_out(p, &["repo", "doctor"]);
    // On Unix, /bin/sh provides a baseline runner, so the repo is ready regardless of
    // whether zsh is present (zsh is optional unless a zsh script is selected).
    if std::path::Path::new("/bin/sh").exists() {
        assert_eq!(json["any_runner_available"], true);
        assert_eq!(json["ready"], true);
    }
}

#[test]
fn doctor_reports_tracked_clean_then_dirty() {
    let repo = init_repo();
    let p = repo.path();

    let clean = json_out(p, &["repo", "doctor"]);
    assert_eq!(clean["tracked_tree_clean"], true);

    fs::write(p.join("README.md"), "# readme\nchanged\n").unwrap();
    let dirty = json_out(p, &["repo", "doctor"]);
    assert_eq!(dirty["tracked_tree_clean"], false);
    // A dirty tree does not, by itself, change anything about creation.
    assert!(!p.join(".aikit").exists());
}

#[test]
fn doctor_reports_locations_interpreters_and_metadata() {
    let repo = init_repo();
    let p = repo.path();
    let json = json_out(p, &["repo", "doctor"]);

    let locs = json["allowed_script_locations"].as_array().unwrap();
    let loc_paths: Vec<&str> = locs.iter().map(|l| l["path"].as_str().unwrap()).collect();
    assert!(loc_paths.contains(&".aikit/temp/"));
    assert!(loc_paths.contains(&".scratch/work/temp/"));
    assert!(loc_paths.contains(&".scratch/work/outputs/"));

    let interps = json["interpreters"].as_array().unwrap();
    let interp_paths: Vec<&str> = interps
        .iter()
        .map(|i| i["path"].as_str().unwrap())
        .collect();
    assert!(interp_paths.contains(&"/bin/sh"));
    assert!(interp_paths.contains(&"/bin/zsh"));

    assert_eq!(json["default_output_root"], ".aikit/outputs");
    assert!(json["version"].as_str().is_some());
    assert!(json["repo_root"].as_str().is_some());
    assert!(json["git_branch"].as_str().is_some());
}

/// The human-readable (non-JSON) doctor output must label the script-input
/// allowlist so `.scratch/...` entries cannot be misread as aikit state, while
/// keeping readiness/state/output fields tied to `.aikit/`.
#[test]
fn doctor_text_output_distinguishes_script_inputs_from_aikit_state() {
    let repo = init_repo();
    let p = repo.path();
    let out = aikit(p)
        .args(["repo", "doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).expect("stdout is UTF-8");

    // The script-input allowlist is clearly marked as not aikit state.
    assert!(
        text.contains("allowed script input locations (not aikit state):"),
        "doctor text output missing clarified script-input label:\n{text}"
    );

    // Readiness/state/output fields stay tied to `.aikit/`.
    assert!(
        text.contains(".aikit/:"),
        "missing .aikit/ state line:\n{text}"
    );
    assert!(
        text.contains(".aikit/temp/:"),
        "missing .aikit/temp/ state line:\n{text}"
    );
    assert!(
        text.contains("default output root: .aikit/outputs"),
        "missing .aikit/outputs default output root line:\n{text}"
    );
}
