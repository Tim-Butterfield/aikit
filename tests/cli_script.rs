//! Integration tests for the `aikit script` command family (`script run` /
//! `script check`) and for the removal of the old `aikit run script` shape.
//!
//! Scripts used here are harmless (echo / write a marker / exit with a code).
//! Forbidden-operation cases use static fixture text that is blocked *before*
//! execution. Each test builds a throwaway Git repo and runs the compiled binary
//! inside it.

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

/// Write a script at a repo-relative path, creating parent dirs.
fn write_script(repo: &Path, rel: &str, content: &str) {
    let path = repo.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, content).unwrap();
}

/// The single run directory under a base (default `.aikit/outputs/runs`).
fn find_run_dir(repo: &Path, base_rel: &str) -> PathBuf {
    let base = repo.join(base_rel);
    let entry = fs::read_dir(&base)
        .unwrap_or_else(|_| panic!("run base {} missing", base.display()))
        .next()
        .expect("a run id dir")
        .expect("dir entry");
    entry.path()
}

// ---- removal of the old `run` command shape ----

#[test]
fn old_run_command_is_unavailable() {
    // `aikit run --help` must be invalid usage now that `run` no longer exists.
    AssertCommand::new(cargo_bin("aikit"))
        .args(["run", "--help"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn old_run_script_shape_is_invalid_usage() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["run", "script", ".aikit/temp/h.sh"])
        .assert()
        .failure()
        .code(2);
}

// ---- help ----

#[test]
fn script_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["script", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("run"))
        .stdout(predicates::str::contains("check"));
}

#[test]
fn script_run_help_states_not_a_sandbox() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["script", "run", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--print"))
        .stdout(predicates::str::contains("--require-clean"))
        .stdout(predicates::str::contains("NOT a security sandbox"));
}

#[test]
fn script_check_help_states_not_a_sandbox() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["script", "check", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--require-clean"))
        .stdout(predicates::str::contains("NOT a security sandbox"));
}

// ---- script run: path / location / interpreter policy ----

#[test]
fn run_script_outside_repo_is_rejected() {
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let script = outside.path().join("x.sh");
    fs::write(&script, "#!/bin/sh\necho hi\n").unwrap();
    aikit(repo.path())
        .args(["script", "run", script.to_str().unwrap()])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[test]
fn run_script_outside_allowed_locations_is_rejected() {
    let repo = init_repo();
    write_script(repo.path(), "tools/build.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["script", "run", "tools/build.sh"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_script_not_allowed"));
}

#[cfg(unix)]
#[test]
fn run_symlinked_script_escaping_repo_is_rejected() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("evil.sh");
    fs::write(&target, "#!/bin/sh\necho hi\n").unwrap();
    fs::create_dir_all(repo.path().join(".aikit/temp")).unwrap();
    symlink(&target, repo.path().join(".aikit/temp/link.sh")).unwrap();
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/link.sh"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[cfg(unix)]
#[test]
fn run_symlinked_script_to_in_repo_outside_allowlist_is_rejected() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    write_script(repo.path(), "tools/real.sh", "#!/bin/sh\necho hi\n");
    fs::create_dir_all(repo.path().join(".aikit/temp")).unwrap();
    symlink(
        repo.path().join("tools/real.sh"),
        repo.path().join(".aikit/temp/link.sh"),
    )
    .unwrap();
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/link.sh"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_script_not_allowed"));
    assert!(
        !repo.path().join(".aikit/outputs/runs").exists(),
        "no run directory should be created on a blocked path"
    );
}

#[test]
fn run_extensionless_script_is_rejected() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/noext", "echo hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/noext"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_unsupported_mode"));
}

#[test]
fn run_unknown_extension_script_is_rejected() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.py", "print('hi')\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/x.py"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_unsupported_mode"));
}

#[test]
fn run_sh_script_runs_through_bin_sh() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.sh"])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");
    let json: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(json["interpreter"], "/bin/sh");
    assert_eq!(json["executed"], true);
}

#[test]
fn run_zsh_script_runs_through_bin_zsh() {
    if !Path::new("/bin/zsh").exists() {
        return; // /bin/zsh is not present on this host; skip.
    }
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.zsh", "echo hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.zsh"])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");
    let json: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(json["interpreter"], "/bin/zsh");
}

// ---- script run: print / clean-tree ----

#[test]
fn run_print_does_not_execute_and_reports_not_executed() {
    let repo = init_repo();
    write_script(
        repo.path(),
        ".aikit/temp/marker.sh",
        "#!/bin/sh\ntouch did-run\n",
    );
    let out = aikit(repo.path())
        .args([
            "script",
            "run",
            ".aikit/temp/marker.sh",
            "--print",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["executed"], false);
    assert!(json["exit_code"].is_null());
    assert!(
        !repo.path().join("did-run").exists(),
        "--print must not execute"
    );
    assert!(
        !repo.path().join(".aikit/outputs/runs").exists(),
        "--print must not create a run directory"
    );
}

#[test]
fn run_default_policy_is_allow_dirty() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    let out = aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.sh", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["executed"], true);
    assert_eq!(json["require_clean"], false);
    assert_eq!(json["allow_dirty"], true);
}

#[test]
fn run_require_clean_blocks_dirty_tree() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.sh", "--require-clean"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_dirty_tree"));
}

#[test]
fn run_allow_dirty_permits_dirty_tree() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.sh", "--allow-dirty"])
        .assert()
        .success();
}

#[test]
fn run_require_clean_and_allow_dirty_together_is_invalid_usage() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args([
            "script",
            "run",
            ".aikit/temp/h.sh",
            "--require-clean",
            "--allow-dirty",
        ])
        .assert()
        .failure()
        .code(2);
}

// ---- script run: forbidden scan ----

#[test]
fn run_forbidden_operation_is_blocked_before_execution() {
    let repo = init_repo();
    write_script(
        repo.path(),
        ".aikit/temp/bad.sh",
        "#!/bin/sh\ntouch did-run\ngit push origin main\n",
    );
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/bad.sh"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_forbidden_operation"));
    assert!(
        !repo.path().join("did-run").exists(),
        "forbidden scan must block before execution"
    );
}

// ---- script run: capture / metadata / exit code ----

#[test]
fn run_captures_stdout_stderr_and_writes_run_json_with_metadata() {
    let repo = init_repo();
    write_script(
        repo.path(),
        ".aikit/temp/io.sh",
        "#!/bin/sh\necho to-out\necho to-err 1>&2\n",
    );
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/io.sh"])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");

    assert_eq!(
        fs::read_to_string(dir.join("stdout.txt")).unwrap(),
        "to-out\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("stderr.txt")).unwrap(),
        "to-err\n"
    );

    let json: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    for field in [
        "schema_version",
        "kind",
        "run_id",
        "repo_root",
        "script_path",
        "script_sha256",
        "script_copy_path",
        "interpreter",
        "argv",
        "cwd",
        "require_clean",
        "allow_dirty",
        "executed",
        "started_at",
        "finished_at",
        "duration_ms",
        "git_head_before",
        "git_head_after",
        "exit_code",
        "blocked_state",
        "stdout_path",
        "stderr_path",
    ] {
        assert!(json.get(field).is_some(), "run.json missing field: {field}");
    }
    assert_eq!(json["kind"], "aikit.script_run");
    assert_eq!(json["stdout_path"], "stdout.txt");
    assert_eq!(json["stderr_path"], "stderr.txt");
    assert!(json["blocked_state"].is_null());
}

#[test]
fn run_executed_script_exit_code_is_propagated() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/seven.sh", "#!/bin/sh\nexit 7\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/seven.sh"])
        .assert()
        .code(7);
}

// ---- script run: output location ----

#[test]
fn run_default_output_goes_to_aikit_outputs_runs() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.sh"])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/outputs/runs/"));
    assert!(repo.path().join(".aikit/outputs/runs").is_dir());
}

#[test]
fn run_output_override_uses_scratch_when_requested() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args([
            "script",
            "run",
            ".aikit/temp/h.sh",
            "--output",
            ".scratch/work/outputs/aikit",
        ])
        .assert()
        .success();
    assert!(
        repo.path()
            .join(".scratch/work/outputs/aikit/runs")
            .is_dir(),
        "explicit --output should be used as requested"
    );
    assert!(
        !repo.path().join(".aikit/outputs").exists(),
        "default .aikit/outputs should not be created when --output is given"
    );
}

#[test]
fn run_copied_script_retains_extension() {
    if !Path::new("/bin/zsh").exists() {
        return; // /bin/zsh is not present on this host; skip.
    }
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.zsh", "echo hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.zsh"])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");
    assert!(dir.join("script.zsh").is_file(), "copied script keeps .zsh");
}

// ---- script check ----

/// Run `script check ... --json` and parse the report.
fn check_json(repo: &Path, args: &[&str]) -> (i32, Value) {
    let mut full = vec!["script", "check"];
    full.extend_from_slice(args);
    full.push("--json");
    let output = aikit(repo).args(&full).assert().get_output().clone();
    let code = output.status.code().unwrap_or(-1);
    let json: Value = serde_json::from_slice(&output.stdout).expect("check stdout is JSON");
    (code, json)
}

#[test]
fn check_accepts_valid_sh_script() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/ok.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/ok.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["kind"], "aikit.script_check");
    assert_eq!(json["accepted"], true);
    assert_eq!(json["executed"], false);
    assert_eq!(json["output_created"], false);
    assert_eq!(json["interpreter"], "/bin/sh");
    assert!(json["blocked_state"].is_null());
    // No run output of any kind.
    assert!(
        !repo.path().join(".aikit/outputs/runs").exists(),
        "check must not create a run directory"
    );
}

#[test]
fn check_accepts_valid_zsh_script_without_executing() {
    // `script check` resolves the interpreter from the extension but never runs it,
    // so this passes even where /bin/zsh is absent.
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/ok.zsh", "echo hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/ok.zsh"]);
    assert_eq!(code, 0);
    assert_eq!(json["accepted"], true);
    assert_eq!(json["interpreter"], "/bin/zsh");
}

#[test]
fn check_rejects_script_outside_allowed_locations() {
    let repo = init_repo();
    write_script(repo.path(), "tools/build.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &["tools/build.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["accepted"], false);
    assert_eq!(json["blocked_state"], "blocked_script_not_allowed");
}

#[test]
fn check_rejects_script_outside_repo() {
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let script = outside.path().join("x.sh");
    fs::write(&script, "#!/bin/sh\necho hi\n").unwrap();
    let (code, json) = check_json(repo.path(), &[script.to_str().unwrap()]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_path_escape");
}

#[cfg(unix)]
#[test]
fn check_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("evil.sh");
    fs::write(&target, "#!/bin/sh\necho hi\n").unwrap();
    fs::create_dir_all(repo.path().join(".aikit/temp")).unwrap();
    symlink(&target, repo.path().join(".aikit/temp/link.sh")).unwrap();
    let (code, json) = check_json(repo.path(), &[".aikit/temp/link.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_path_escape");
}

#[test]
fn check_rejects_extensionless_script() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/noext", "echo hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/noext"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_unsupported_mode");
}

#[test]
fn check_rejects_unknown_extension() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.py", "print('hi')\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.py"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_unsupported_mode");
}

#[test]
fn check_blocks_forbidden_operation_text() {
    let repo = init_repo();
    write_script(
        repo.path(),
        ".aikit/temp/bad.sh",
        "#!/bin/sh\ngit push origin main\n",
    );
    let (code, json) = check_json(repo.path(), &[".aikit/temp/bad.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_forbidden_operation");
    // Even on a forbidden-op block the interpreter/resolution were known.
    assert_eq!(json["interpreter"], "/bin/sh");
}

#[test]
fn check_require_clean_blocks_dirty_tree() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    write_script(repo.path(), ".aikit/temp/ok.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/ok.sh", "--require-clean"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_dirty_tree");
    assert_eq!(json["require_clean"], true);
}

#[test]
fn check_default_policy_is_allow_dirty() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    write_script(repo.path(), ".aikit/temp/ok.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/ok.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["accepted"], true);
    assert_eq!(json["require_clean"], false);
    assert_eq!(json["allow_dirty"], true);
}

#[test]
fn check_require_clean_and_allow_dirty_together_is_invalid_usage() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/ok.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args([
            "script",
            "check",
            ".aikit/temp/ok.sh",
            "--require-clean",
            "--allow-dirty",
        ])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn check_does_not_execute_or_create_run_output() {
    let repo = init_repo();
    write_script(
        repo.path(),
        ".aikit/temp/marker.sh",
        "#!/bin/sh\ntouch did-run\n",
    );
    aikit(repo.path())
        .args(["script", "check", ".aikit/temp/marker.sh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("ACCEPTED"))
        .stdout(predicates::str::contains("not executed"));
    assert!(
        !repo.path().join("did-run").exists(),
        "check must not execute the script"
    );
    assert!(
        !repo.path().join(".aikit/outputs").exists(),
        "check must not create any run output"
    );
}
