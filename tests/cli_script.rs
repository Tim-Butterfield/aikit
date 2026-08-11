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

// ---- portable fixtures ----
//
// Tests whose subject is *not* the runner — output locations, VCS detection, exit-code
// propagation, clean-tree policy — must not depend on a POSIX shell. A Windows host is not
// guaranteed one: `.sh` resolves only when a discoverable interpreter exists, and the
// `System32` `bash.exe` is a WSL launcher that aikit deliberately refuses (a `.sh` script
// run there executes in a Linux filesystem namespace where the repo-relative paths mean
// something else). `cmd` is always present on Windows and `/bin/sh` always on Unix, so this
// pair is the portable floor. Tests whose subject *is* a specific interpreter keep naming it
// and are gated on that interpreter existing.

/// Extension for the always-available runner on this platform.
fn any_ext() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "sh"
    }
}

/// Write a portable script under `.aikit/temp/` and return its repo-relative path.
fn write_any(repo: &Path, stem: &str, body: &str) -> String {
    let rel = format!(".aikit/temp/{}.{}", stem, any_ext());
    write_script(repo, &rel, body);
    rel
}

/// A script that prints `hi` and nothing else.
///
/// The leading `@` matters on cmd: without it the interpreter echoes the command line
/// itself into stdout, which would break any assertion on captured output.
fn body_hello() -> &'static str {
    if cfg!(windows) {
        "@echo hi\r\n"
    } else {
        "#!/bin/sh\necho hi\n"
    }
}

/// A script that exits with status 7.
fn body_exit_7() -> &'static str {
    if cfg!(windows) {
        "@exit /b 7\r\n"
    } else {
        "#!/bin/sh\nexit 7\n"
    }
}

/// A script that creates an empty marker file in the working directory.
fn body_marker(name: &str) -> String {
    if cfg!(windows) {
        format!("@type nul > {name}\r\n")
    } else {
        format!("#!/bin/sh\ntouch {name}\n")
    }
}

/// A script that writes one line to stdout and one to stderr.
fn body_two_streams() -> &'static str {
    if cfg!(windows) {
        "@echo to-out\r\n@echo to-err 1>&2\r\n"
    } else {
        "#!/bin/sh\necho to-out\necho to-err 1>&2\n"
    }
}

/// A script containing a forbidden Git operation, refused before execution.
fn body_forbidden() -> String {
    if cfg!(windows) {
        "@git push origin main\r\n".to_string()
    } else {
        "#!/bin/sh\ngit push origin main\n".to_string()
    }
}

/// Write `.aikit/config.json` with the given JSON value.
fn write_config(repo: &Path, value: &Value) {
    let p = repo.join(".aikit/config.json");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, serde_json::to_string_pretty(value).unwrap()).unwrap();
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
fn script_help_describes_cross_os_runner_detection() {
    let out = AssertCommand::new(cargo_bin("aikit"))
        .args(["script", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&out);
    // New behavior must be described.
    for needle in [
        ".ps1",
        ".py",
        ".js",
        "pwsh",
        "powershell",
        "cmd",
        "node",
        "--runner",
        "shebang",
        "extension_map",
        "no Git Bash",
        "blocked_runner_not_found",
        "NOT a security sandbox",
    ] {
        assert!(help.contains(needle), "script --help missing {needle:?}");
    }
    // Stale claims must be gone.
    assert!(
        !help.contains("fixed interpreter"),
        "script --help still says 'fixed interpreter'"
    );
    assert!(
        !help.contains("never from a shebang"),
        "script --help still says 'never from a shebang'"
    );
    assert!(
        !help.contains("Only `.zsh`"),
        "script --help still says only .zsh/.sh are supported"
    );
}

/// Help output with whitespace collapsed, so assertions test what the help says rather
/// than where clap happened to wrap it.
fn help_text(args: &[&str]) -> String {
    let out = AssertCommand::new(cargo_bin("aikit"))
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8_lossy(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn script_run_help_states_not_a_sandbox() {
    let help = help_text(&["script", "run", "--help"]);
    for needle in ["--print", "--require-clean", "NOT a security sandbox"] {
        assert!(
            help.contains(needle),
            "script run --help missing {needle:?}"
        );
    }
}

#[test]
fn script_run_help_describes_detected_runner_not_fixed_interpreter() {
    let help = help_text(&["script", "run", "--help"]);
    assert!(
        !help.contains("fixed interpreter"),
        "script run --help still says 'fixed interpreter'"
    );
    assert!(
        help.contains("detected runner"),
        "script run --help should describe the detected runner"
    );
    // Sufficiently describes the cross-OS detected-runner behavior.
    for needle in [
        ".ps1",
        ".py",
        "pwsh",
        "cmd",
        "node",
        "--runner",
        "shebang",
        "extension map",
        "no Git Bash",
        "blocked_runner_not_found",
        "NOT a security sandbox",
    ] {
        assert!(
            help.contains(needle),
            "script run --help missing {needle:?}"
        );
    }
}

#[test]
fn script_run_help_describes_vcs_aware_behavior() {
    let out = AssertCommand::new(cargo_bin("aikit"))
        .args(["script", "run", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&out);
    // The help must document the VCS-aware root detection + clean-tree behavior so it
    // does not drift from docs/aikit-cli-spec.md §5.1.
    for needle in [
        ".aikit",
        ".hg",
        "Mercurial",
        "vcs",
        "hg status -mard",
        "blocked_require_clean_unsupported",
        "blocked_repo_not_found",
    ] {
        assert!(
            help.contains(needle),
            "script run --help missing VCS-aware needle {needle:?}"
        );
    }
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
        .stderr(predicates::str::contains("blocked_unknown_script_type"));
}

#[test]
fn run_unknown_extension_script_is_rejected() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.xyz", "data\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/x.xyz"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_unknown_script_type"));
}

#[test]
#[cfg(unix)]
fn run_sh_script_runs_through_bin_sh() {
    // Subject is `/bin/sh` specifically, so this one stays POSIX and is gated rather than
    // made portable — asserting the resolved interpreter is the whole point.
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

// ---- script run: non-repo .aikit folder & hg root (filesystem detection) ----

/// Create a non-repo folder with a `.aikit/` marker (as `aikit init --require-folder` would),
/// no `.git`/`.hg`. `script run` should detect it via filesystem walk-up alone.
fn init_aikit_folder() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir_all(dir.path().join(".aikit/temp")).unwrap();
    dir
}

#[test]
fn run_in_non_repo_aikit_folder_executes_and_records_vcs_none() {
    let folder = init_aikit_folder();
    let p = folder.path();
    let script = write_any(p, "h", body_hello());
    aikit(p).args(["script", "run", &script]).assert().success();
    let dir = find_run_dir(p, ".aikit/outputs/runs");
    let json: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(json["executed"], true);
    assert_eq!(json["vcs"], "none");
    // Head is git-only; a non-repo run records empty heads and spawns no git probe.
    assert_eq!(json["git_head_before"], "");
    assert_eq!(json["git_head_after"], "");
}

#[test]
fn run_require_clean_in_non_repo_folder_is_blocked() {
    let folder = init_aikit_folder();
    let p = folder.path();
    let script = write_any(p, "h", body_hello());
    aikit(p)
        .args(["script", "run", &script, "--require-clean"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains(
            "blocked_require_clean_unsupported",
        ));
}

#[test]
fn run_outside_any_marker_is_blocked_repo_not_found() {
    // A bare temp dir with no .git/.hg/.aikit, and a script written directly in it.
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    fs::create_dir_all(p.join(".scratch/work/temp")).unwrap();
    let rel = format!(".scratch/work/temp/h.{}", any_ext());
    fs::write(p.join(&rel), body_hello()).unwrap();
    aikit(p)
        .args(["script", "run", &rel])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

#[test]
fn run_in_hg_repo_executes_and_records_vcs_mercurial() {
    // `.hg/` marker only (no hg binary needed for detection).
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    fs::create_dir_all(p.join(".hg")).unwrap();
    let script = write_any(p, "h", body_hello());
    aikit(p).args(["script", "run", &script]).assert().success();
    let dir = find_run_dir(p, ".aikit/outputs/runs");
    let json: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(json["executed"], true);
    assert_eq!(json["vcs"], "mercurial");
    // Head is git-only; an hg run records empty heads (and spawns no git head probe).
    assert_eq!(json["git_head_before"], "");
    assert_eq!(json["git_head_after"], "");
}

// ---- script run: print / clean-tree ----

#[test]
fn run_print_does_not_execute_and_reports_not_executed() {
    let repo = init_repo();
    let script = write_any(repo.path(), "marker", &body_marker("did-run"));
    let out = aikit(repo.path())
        .args(["script", "run", &script, "--print", "--json"])
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
    let script = write_any(repo.path(), "h", body_hello());
    let out = aikit(repo.path())
        .args(["script", "run", &script, "--json"])
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
    let script = write_any(repo.path(), "h", body_hello());
    aikit(repo.path())
        .args(["script", "run", &script, "--require-clean"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_dirty_tree"));
}

#[test]
fn run_allow_dirty_permits_dirty_tree() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    let script = write_any(repo.path(), "h", body_hello());
    aikit(repo.path())
        .args(["script", "run", &script, "--allow-dirty"])
        .assert()
        .success();
}

#[test]
fn run_require_clean_and_allow_dirty_together_is_invalid_usage() {
    let repo = init_repo();
    let script = write_any(repo.path(), "h", body_hello());
    aikit(repo.path())
        .args(["script", "run", &script, "--require-clean", "--allow-dirty"])
        .assert()
        .failure()
        .code(2);
}

// ---- script run: forbidden scan ----

#[test]
fn run_forbidden_operation_is_blocked_before_execution() {
    let repo = init_repo();
    // A marker write *before* the forbidden line: if the scan ran late, the marker would
    // exist. It must not, because the refusal happens before anything executes.
    let body = if cfg!(windows) {
        "@type nul > did-run\r\n@git push origin main\r\n".to_string()
    } else {
        "#!/bin/sh\ntouch did-run\ngit push origin main\n".to_string()
    };
    let script = write_any(repo.path(), "bad", &body);
    aikit(repo.path())
        .args(["script", "run", &script])
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
    let script = write_any(repo.path(), "io", body_two_streams());
    aikit(repo.path())
        .args(["script", "run", &script])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");

    // Compare trimmed: cmd emits CRLF, so the line ending is the interpreter's, not aikit's.
    assert_eq!(
        fs::read_to_string(dir.join("stdout.txt"))
            .unwrap()
            .trim_end(),
        "to-out"
    );
    assert_eq!(
        fs::read_to_string(dir.join("stderr.txt"))
            .unwrap()
            .trim_end(),
        "to-err"
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
    let script = write_any(repo.path(), "seven", body_exit_7());
    aikit(repo.path())
        .args(["script", "run", &script])
        .assert()
        .code(7);
}

// ---- script run: output location ----

#[test]
fn run_default_output_goes_to_aikit_outputs_runs() {
    let repo = init_repo();
    let script = write_any(repo.path(), "h", body_hello());
    aikit(repo.path())
        .args(["script", "run", &script])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/outputs/runs/"));
    assert!(repo.path().join(".aikit/outputs/runs").is_dir());
}

#[test]
fn run_output_override_uses_scratch_when_requested() {
    let repo = init_repo();
    let script = write_any(repo.path(), "h", body_hello());
    aikit(repo.path())
        .args([
            "script",
            "run",
            &script,
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
fn a_script_can_be_supplied_on_stdin() {
    // The hook workflow: one call, no file authored by the caller, run from anywhere.
    let repo = init_repo();
    let p = repo.path();
    let (runner, body) = if cfg!(windows) {
        ("cmd", "@echo from-stdin\r\n")
    } else {
        ("sh", "echo from-stdin\n")
    };

    let out = aikit(p)
        .args(["script", "run", "-", "--runner", runner, "--json"])
        .write_stdin(body)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["exit_code"], 0);
    assert_eq!(json["detected_runner"], runner);

    // aikit generated this file, so it applies the same rules the MCP path does — including
    // the exit-code epilogue. This is the one `script run` case where a cmd or PowerShell
    // exit code is trustworthy rather than `unpropagated`.
    let expected_source = if cfg!(windows) {
        "epilogue"
    } else {
        "interpreter"
    };
    assert_eq!(json["exit_code_source"], expected_source);

    // aikit wrote it into an allowed location, so the allowlist still governed the run.
    let script_path = json["script_path"].as_str().unwrap();
    assert!(
        script_path.starts_with(".aikit/temp/"),
        "a stdin script must be materialized under an allowed location: {script_path}"
    );
    // The extension is derived from the runner, not guessed, and is well formed.
    assert!(
        !script_path.contains(".."),
        "malformed generated filename: {script_path}"
    );

    // The run is fully recorded despite there being no caller-authored file.
    let dir = find_run_dir(p, ".aikit/outputs/runs");
    let captured = fs::read_to_string(dir.join("stdout.txt")).unwrap();
    assert_eq!(captured.trim_end(), "from-stdin");

    // And the temporary does not persist: one per invocation would recreate the growth
    // problem retention exists to prevent.
    let leftovers: Vec<String> = fs::read_dir(p.join(".aikit/temp"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("stdin-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "stdin temporaries were left behind: {leftovers:?}"
    );
}

#[test]
fn a_blocked_stdin_check_leaves_no_temporary_behind() {
    // `script check` exits via `process::exit` on every blocked path, which runs no
    // destructors. Only the first of four such paths used to release the guard, so a
    // policy-rejected stdin check left its materialized script in `.aikit/temp/`.
    let repo = init_repo();
    let p = repo.path();
    let runner = if cfg!(windows) { "cmd" } else { "sh" };

    // A forbidden operation blocks *after* location and runner resolution — one of the
    // later paths, not the first.
    aikit(p)
        .args(["script", "check", "-", "--runner", runner, "--json"])
        .write_stdin("git push origin main\n")
        .assert()
        .failure()
        .code(3);

    let leftovers: Vec<String> = fs::read_dir(p.join(".aikit/temp"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("stdin-"))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "a blocked stdin check left its temporary behind: {leftovers:?}"
    );
}

#[test]
fn script_check_records_the_acknowledgements_it_accepts() {
    // `script check` accepts --acknowledge-forbidden and its help promises that every
    // acknowledgement is recorded. That promise has to hold for the check record too, or it
    // holds only for `script run` and an auditor sees nothing for the validation step.
    let repo = init_repo();
    let p = repo.path();
    let body = if cfg!(windows) {
        "@git push origin main\r\n"
    } else {
        "#!/bin/sh\ngit push origin main\n"
    };
    let script = write_any(p, "ack", body);

    let (code, json) = check_json(p, &[&script, "--acknowledge-forbidden", "git push"]);
    assert_eq!(code, 0, "the acknowledged operation should be accepted");
    assert_eq!(
        json["acknowledged_forbidden"],
        serde_json::json!(["git push"]),
        "the check record must carry what was waived: {json}"
    );
}

#[test]
fn a_stdin_script_still_obeys_policy() {
    let repo = init_repo();
    let p = repo.path();

    // No runner to infer from a filename, so one must be named — the same refusal to guess
    // aikit makes everywhere else.
    let out = aikit(p)
        .args(["script", "run", "-", "--json"])
        .write_stdin("echo hi\n")
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["blocked_state"], "blocked_unknown_script_type");

    // The forbidden scan applies to a script that never existed as a caller's file.
    let runner = if cfg!(windows) { "cmd" } else { "sh" };
    let out = aikit(p)
        .args(["script", "run", "-", "--runner", runner, "--json"])
        .write_stdin("git push origin main\n")
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["blocked_state"], "blocked_forbidden_operation");
}

#[test]
fn cwd_runs_the_command_against_another_directory() {
    // Removes shell navigation from a wrapper's critical path — the step that breaks
    // cross-platform, where quoting and `cd` semantics differ.
    let repo = init_repo();
    let p = repo.path();
    let script = write_any(p, "h", body_hello());

    // Run from a directory that is not the repo, naming the repo with --cwd.
    let elsewhere = TempDir::new().unwrap();
    let out = AssertCommand::new(cargo_bin("aikit"))
        .current_dir(elsewhere.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args([
            "--cwd",
            &p.to_string_lossy(),
            "script",
            "run",
            &script,
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["exit_code"], 0);

    // A directory that cannot be entered is invalid usage, not a blocked state: nothing
    // about a repository has been examined yet.
    AssertCommand::new(cargo_bin("aikit"))
        .args(["--cwd", "/no/such/directory/anywhere", "doctor"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn retention_keeps_the_newest_runs_and_reports_what_it_removed() {
    // Retention deletes audit trails, so it must be bounded, oldest-first, and visible. A
    // silent deletion is indistinguishable from a record that was never written.
    let repo = init_repo();
    let p = repo.path();
    write_config(p, &serde_json::json!({ "output": { "retain_runs": 3 } }));
    let script = write_any(p, "h", body_hello());

    let runs = p.join(".aikit/outputs/runs");
    for _ in 0..6 {
        aikit(p).args(["script", "run", &script]).assert().success();
    }

    let dirs: Vec<String> = fs::read_dir(&runs)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().unwrap().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        dirs.len(),
        3,
        "retain_runs=3 should leave exactly 3 run directories, found: {dirs:?}"
    );

    // Every surviving directory still holds a complete record: retention must not leave a
    // half-deleted directory that looks like a corrupt run.
    for d in &dirs {
        assert!(
            runs.join(d).join("run.json").is_file(),
            "surviving run {d} is missing run.json"
        );
    }

    // The run that reported pruning says so in its own record.
    let last = aikit(p)
        .args(["script", "run", &script, "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&last).unwrap();
    assert_eq!(
        json["pruned_runs"], 1,
        "a run over the limit should report the directory it removed: {json}"
    );
}

#[test]
fn retention_can_be_disabled() {
    // `0` means unlimited. Someone keeping a full audit trail must be able to say so.
    let repo = init_repo();
    let p = repo.path();
    write_config(p, &serde_json::json!({ "output": { "retain_runs": 0 } }));
    let script = write_any(p, "h", body_hello());

    for _ in 0..4 {
        aikit(p).args(["script", "run", &script]).assert().success();
    }

    let count = fs::read_dir(p.join(".aikit/outputs/runs")).unwrap().count();
    assert_eq!(count, 4, "retain_runs=0 must not delete anything");
}

#[test]
fn run_record_is_written_atomically() {
    // The record is written via a temporary file and renamed, so a reader never sees a torn
    // JSON document. The observable consequence: no stray temporary is left beside it.
    let repo = init_repo();
    let p = repo.path();
    let script = write_any(p, "h", body_hello());
    aikit(p).args(["script", "run", &script]).assert().success();

    let dir = find_run_dir(p, ".aikit/outputs/runs");
    let leftovers: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "atomic write left temporaries behind: {leftovers:?}"
    );
    // And the record parses — a truncated write would not.
    let json: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(json["kind"], "aikit.script_run");
}

#[test]
fn run_argv_script_path_uses_native_separators() {
    // cmd.exe rejects a forward-slash path outright (`'.aikit' is not recognized as an
    // internal or external command`, exit 1), so the executed form must use the platform
    // separator — and the recorded argv must show the form that actually ran, not a tidied
    // one. The canonical forward-slash path stays in `script_path`.
    let repo = init_repo();
    let script = write_any(repo.path(), "h", body_hello());
    let out = aikit(repo.path())
        .args(["script", "run", &script, "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();

    let argv = json["argv"].as_array().expect("argv array");
    let last = argv.last().unwrap().as_str().unwrap();
    let expected = if cfg!(windows) {
        script.replace('/', "\\")
    } else {
        script.clone()
    };
    assert_eq!(last, expected, "argv tail is the executed script path");

    // The record keeps the portable spelling regardless of platform.
    assert_eq!(json["script_path"], script);
}

#[test]
fn run_copied_script_retains_extension() {
    let repo = init_repo();
    let script = write_any(repo.path(), "h", body_hello());
    aikit(repo.path())
        .args(["script", "run", &script])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");
    let copied = format!("script.{}", any_ext());
    assert!(
        dir.join(&copied).is_file(),
        "copied script keeps its extension: expected {copied}"
    );
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
#[cfg(unix)]
fn check_accepts_valid_sh_script() {
    // Asserts the resolved interpreter is `/bin/sh`, so it is POSIX by subject. The
    // platform-neutral acceptance path is covered by `check_default_policy_is_allow_dirty`.
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
    // `script check` resolves the runner (the program must be discoverable) but never
    // runs it. Skip where no zsh is installed.
    if !Path::new("/bin/zsh").exists() {
        return;
    }
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/ok.zsh", "echo hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/ok.zsh"]);
    assert_eq!(code, 0);
    assert_eq!(json["accepted"], true);
    assert_eq!(json["detected_runner"], "zsh");
    assert_eq!(json["interpreter"], "/bin/zsh");
    assert_eq!(json["detection_source"], "extension_map");
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
    assert_eq!(json["blocked_state"], "blocked_unknown_script_type");
}

#[test]
fn check_rejects_unknown_extension() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.xyz", "data\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.xyz"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_unknown_script_type");
}

#[test]
fn check_blocks_forbidden_operation_text() {
    let repo = init_repo();
    let script = write_any(repo.path(), "bad", &body_forbidden());
    let (code, json) = check_json(repo.path(), &[&script]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_forbidden_operation");
    // Even on a forbidden-op block the interpreter was resolved, not left unknown. The
    // path itself is platform-specific, so assert only that resolution happened.
    assert!(
        json["interpreter"].as_str().is_some_and(|s| !s.is_empty()),
        "interpreter should be resolved even when the scan blocks: {json}"
    );
}

#[test]
fn check_require_clean_blocks_dirty_tree() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    let script = write_any(repo.path(), "ok", body_hello());
    let (code, json) = check_json(repo.path(), &[&script, "--require-clean"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_dirty_tree");
    assert_eq!(json["require_clean"], true);
}

#[test]
fn check_default_policy_is_allow_dirty() {
    let repo = init_repo();
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    let script = write_any(repo.path(), "ok", body_hello());
    let (code, json) = check_json(repo.path(), &[&script]);
    assert_eq!(code, 0);
    assert_eq!(json["accepted"], true);
    assert_eq!(json["require_clean"], false);
    assert_eq!(json["allow_dirty"], true);
}

#[test]
fn check_require_clean_and_allow_dirty_together_is_invalid_usage() {
    let repo = init_repo();
    let script = write_any(repo.path(), "ok", body_hello());
    aikit(repo.path())
        .args([
            "script",
            "check",
            &script,
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
    let script = write_any(repo.path(), "marker", &body_marker("did-run"));
    aikit(repo.path())
        .args(["script", "check", &script])
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

// ---- cross-OS runner detection ----

/// Whether a program responds to `--version` (a cheap availability probe).
fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
#[cfg(unix)]
fn run_records_detection_metadata() {
    // Subject is shebang-sourced detection, which `.cmd` has no equivalent for.
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/h.sh"])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");
    let json: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(json["detected_runner"], "sh");
    assert_eq!(json["detection_source"], "shebang");
    assert_eq!(json["used_shebang"], true);
    assert_eq!(json["used_extension_map"], false);
    let argv = json["argv"].as_array().expect("argv array");
    assert_eq!(argv.last().unwrap(), ".aikit/temp/h.sh");
}

#[test]
#[cfg(unix)]
fn run_no_shebang_uses_extension_map() {
    // `--no-shebang` only has meaning for a script that carries one.
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/h.sh", "#!/bin/sh\necho hi\n");
    let out = aikit(repo.path())
        .args([
            "script",
            "run",
            ".aikit/temp/h.sh",
            "--no-shebang",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["detection_source"], "extension_map");
    assert_eq!(json["used_shebang"], false);
    assert_eq!(json["used_extension_map"], true);
}

#[test]
#[cfg(unix)]
fn check_reports_detection_metadata() {
    // Subject is shebang-sourced detection; see `run_records_detection_metadata`.
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/ok.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/ok.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["detected_runner"], "sh");
    assert_eq!(json["detection_source"], "shebang");
    assert_eq!(json["used_shebang"], true);
    assert!(json["argv"].is_array());
}

#[test]
fn explicit_runner_overrides_extension_mapping() {
    if !have("bash") {
        return;
    }
    let repo = init_repo();
    // .sh would default to sh; --runner bash forces bash regardless of the shebang.
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh", "--runner", "bash"]);
    assert_eq!(code, 0);
    assert_eq!(json["detected_runner"], "bash");
    assert_eq!(json["detection_source"], "explicit_runner");
    assert_eq!(json["used_extension_map"], false);
}

#[test]
fn explicit_unknown_runner_is_not_allowed() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/ok.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/ok.sh", "--runner", "fish"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_runner_not_allowed");
}

#[test]
fn ps1_maps_to_powershell_family_or_reports_unavailable() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.ps1", "Write-Output hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.ps1"]);
    if code == 0 {
        let runner = json["detected_runner"].as_str().unwrap();
        assert!(
            runner == "pwsh" || runner == "powershell",
            "ps1 should resolve to a PowerShell runner, got {runner}"
        );
        assert_eq!(json["detection_source"], "extension_map");
    } else {
        assert_eq!(code, 3);
        assert_eq!(json["blocked_state"], "blocked_runner_not_found");
    }
}

#[test]
fn cmd_maps_to_cmd_on_windows_else_unavailable() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.cmd", "echo hi\r\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.cmd"]);
    if cfg!(windows) {
        assert_eq!(code, 0);
        assert_eq!(json["detected_runner"], "cmd");
    } else {
        assert_eq!(code, 3);
        assert_eq!(json["blocked_state"], "blocked_runner_not_found");
    }
}

#[test]
fn bat_maps_to_cmd_on_windows_else_unavailable() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.bat", "echo hi\r\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.bat"]);
    if cfg!(windows) {
        assert_eq!(code, 0);
        assert_eq!(json["detected_runner"], "cmd");
    } else {
        assert_eq!(code, 3);
        assert_eq!(json["blocked_state"], "blocked_runner_not_found");
    }
}

#[test]
fn py_maps_to_python_when_available_else_unavailable() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.py", "print('hi')\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.py"]);
    if have("python3") || have("python") {
        assert_eq!(code, 0);
        let runner = json["detected_runner"].as_str().unwrap();
        assert!(runner == "python3" || runner == "python");
        assert_eq!(json["detection_source"], "extension_map");
    } else {
        assert_eq!(code, 3);
        assert_eq!(json["blocked_state"], "blocked_runner_not_found");
    }
}

#[test]
fn js_maps_to_node_when_available_else_unavailable() {
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/x.js", "console.log('hi')\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.js"]);
    if have("node") {
        assert_eq!(code, 0);
        assert_eq!(json["detected_runner"], "node");
        assert_eq!(json["detection_source"], "extension_map");
    } else {
        assert_eq!(code, 3);
        assert_eq!(json["blocked_state"], "blocked_runner_not_found");
    }
}

#[test]
fn py_script_executes_when_python_available() {
    if !(have("python3") || have("python")) {
        return;
    }
    let repo = init_repo();
    write_script(repo.path(), ".aikit/temp/p.py", "print('from-python')\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/p.py"])
        .assert()
        .success();
    let dir = find_run_dir(repo.path(), ".aikit/outputs/runs");
    // Python's `print` emits CRLF on Windows. The captured bytes are the interpreter's, and
    // aikit records them verbatim rather than rewriting a script's output — so the
    // assertion normalises instead.
    let captured = fs::read_to_string(dir.join("stdout.txt")).unwrap();
    assert_eq!(captured.replace("\r\n", "\n"), "from-python\n");
}

// ---- config-driven runner detection ----

#[test]
fn config_extension_map_overrides_builtin() {
    if !have("bash") {
        return;
    }
    let repo = init_repo();
    // Map .sh -> bash via config; this beats the shebang too (config tier > shebang).
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "extension_map": { ".sh": ["bash"] } } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["detected_runner"], "bash");
    assert_eq!(json["detection_source"], "config");
    assert_eq!(json["used_extension_map"], true);
}

#[test]
fn config_preferred_runners_changes_order() {
    if !have("bash") {
        return;
    }
    let repo = init_repo();
    // Built-in .sh candidates are [sh, bash]; preferring bash reorders to [bash, sh].
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "preferred_runners": ["bash"] } }),
    );
    // No shebang, so the (reordered) built-in extension map decides.
    write_script(repo.path(), ".aikit/temp/x.sh", "echo hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["detected_runner"], "bash");
    assert_eq!(json["detection_source"], "extension_map");
}

#[test]
#[cfg(unix)]
fn config_detect_from_shebang_false_ignores_shebang() {
    // `.sh` must resolve for `script check` to accept, and a Windows host is not guaranteed
    // a POSIX shell. This passed on Windows only while `.sh` silently resolved to the
    // WindowsApps `bash` App Execution Alias — a WSL launcher aikit now refuses, which is
    // the point: the test was green for a reason that was itself the hazard.
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "detect_from_shebang": false } }),
    );
    // Shebang says sh, but with detection disabled the extension map decides (.sh -> sh).
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["detection_source"], "extension_map");
    assert_eq!(json["used_shebang"], false);
}

#[test]
#[cfg(unix)]
fn config_detect_from_extension_false_allows_shebang_only() {
    // With extension mapping off, only the shebang can select a runner — and a `#!/bin/sh`
    // shebang needs an actual `sh`, which a Windows host is not guaranteed.
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "detect_from_extension": false } }),
    );
    // Extension mapping is off, but the shebang still selects a runner.
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["detected_runner"], "sh");
    assert_eq!(json["detection_source"], "shebang");
}

#[test]
fn config_detect_from_extension_false_blocks_extension_only_script() {
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "detect_from_extension": false } }),
    );
    // No shebang and extension mapping disabled -> unknown script type.
    write_script(repo.path(), ".aikit/temp/x.sh", "echo hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_unknown_script_type");
}

#[test]
fn config_unknown_preferred_runner_fails_clearly() {
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "preferred_runners": ["bsah"] } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_runner_not_allowed");
}

#[test]
fn config_unknown_extension_map_runner_fails_clearly() {
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "extension_map": { ".sh": ["bsah"] } } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_runner_not_allowed");
}

#[test]
fn config_unknown_runner_also_blocks_script_run() {
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "preferred_runners": ["bsah"] } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    aikit(repo.path())
        .args(["script", "run", ".aikit/temp/x.sh"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_runner_not_allowed"));
}

#[test]
fn config_mixed_case_preferred_runner_fails_clearly() {
    let repo = init_repo();
    // Configured runner names must be lowercase; "Bash" must not pass validation.
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "preferred_runners": ["Bash"] } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_runner_not_allowed");
}

#[test]
fn config_mixed_case_extension_map_runner_fails_clearly() {
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "extension_map": { ".sh": ["Python3"] } } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_runner_not_allowed");
}

#[test]
fn config_uppercase_runner_value_fails_clearly() {
    let repo = init_repo();
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "preferred_runners": ["NODE"] } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "#!/bin/sh\necho hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 3);
    assert_eq!(json["blocked_state"], "blocked_runner_not_allowed");
}

#[test]
fn config_valid_lowercase_runner_still_works() {
    if !have("bash") {
        return;
    }
    let repo = init_repo();
    // Lowercase configured names continue to work.
    write_config(
        repo.path(),
        &serde_json::json!({ "script_runner": { "extension_map": { ".sh": ["bash"] } } }),
    );
    write_script(repo.path(), ".aikit/temp/x.sh", "echo hi\n");
    let (code, json) = check_json(repo.path(), &[".aikit/temp/x.sh"]);
    assert_eq!(code, 0);
    assert_eq!(json["detected_runner"], "bash");
    assert_eq!(json["detection_source"], "config");
}
