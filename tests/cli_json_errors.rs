//! Integration tests for the `--json` failure contract.
//!
//! A machine caller passing `--json` must get one JSON document on stdout whether the
//! command succeeds or fails. Before this contract existed, most commands printed nothing to
//! stdout on a blocked state, leaving a caller to scrape prose from stderr precisely when it
//! most needed structure. These tests pin both halves: stdout is the machine channel and
//! carries exactly one document; stderr keeps the human text.

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

/// Run a command expected to fail, returning `(exit code, stdout, stderr)`.
fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = aikit(dir).args(args).output().expect("run aikit");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Number of whole JSON documents in `s`. More than one means no caller can parse stdout.
fn json_document_count(s: &str) -> usize {
    let de = serde_json::Deserializer::from_str(s).into_iter::<Value>();
    let mut n = 0;
    for item in de {
        match item {
            Ok(_) => n += 1,
            // Trailing garbage stops the count; the caller asserts on the total, so a
            // partial parse shows up as a mismatch rather than being silently tolerated.
            Err(_) => break,
        }
    }
    n
}

/// Every failing command that supports `--json`, with the blocked state it should name.
fn failing_cases() -> Vec<(&'static str, Vec<&'static str>, &'static str)> {
    vec![
        (
            "batch changed",
            vec!["batch", "changed", "--anchor", "missing.json"],
            "blocked_missing_anchor",
        ),
        (
            "batch diff",
            vec!["batch", "diff", "missing.json"],
            "blocked_missing_anchor",
        ),
        (
            "output show",
            vec!["output", "show", "no-such-artifact"],
            "blocked_artifact_not_found",
        ),
        (
            "init --require-folder",
            vec!["init", "--require-folder"],
            "blocked_repo_present",
        ),
        (
            "script run outside allowlist",
            vec!["script", "run", "tools/build.sh"],
            "blocked_script_not_allowed",
        ),
    ]
}

#[test]
fn json_failures_emit_one_error_document_on_stdout() {
    let repo = init_repo();
    let p = repo.path();
    fs::create_dir_all(p.join("tools")).unwrap();
    fs::write(p.join("tools/build.sh"), "#!/bin/sh\necho hi\n").unwrap();

    for (label, args, expected_state) in failing_cases() {
        let mut with_json = args.clone();
        with_json.push("--json");
        let (code, stdout, stderr) = run(p, &with_json);

        assert_eq!(code, 3, "{label}: expected a blocked state\n{stderr}");
        assert_eq!(
            json_document_count(&stdout),
            1,
            "{label}: stdout must hold exactly one JSON document, got:\n{stdout}"
        );

        let json: Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("{label}: stdout is not JSON ({e}):\n{stdout}"));
        assert_eq!(json["kind"], "aikit.error", "{label}");
        assert_eq!(json["ok"], false, "{label}");
        assert_eq!(json["schema_version"], 1, "{label}");
        assert_eq!(json["exit_code"], 3, "{label}");
        assert_eq!(json["blocked_state"], expected_state, "{label}");

        // The detail must not repeat the state: `blocked_state` is already its own field,
        // and a caller showing `message` should not have to strip a prefix off it.
        let message = json["message"].as_str().expect("message is a string");
        assert!(
            !message.starts_with(expected_state),
            "{label}: message repeats the blocked state: {message}"
        );
        assert!(!message.is_empty(), "{label}: message is empty");

        // Human text still goes to stderr, so stdout stays a clean machine channel.
        assert!(
            stderr.contains(expected_state),
            "{label}: stderr should still name the state, got: {stderr}"
        );
    }
}

#[test]
fn without_json_a_failure_writes_nothing_to_stdout() {
    // The error document is opt-in. A human caller's stdout must stay empty on failure,
    // otherwise piping a command into something else starts yielding JSON it never asked for.
    let repo = init_repo();
    for (label, args, _) in failing_cases() {
        // `init --require-folder` is the one case whose human path prints nothing anyway;
        // include it regardless, since the point is that no JSON appears.
        let (code, stdout, stderr) = run(repo.path(), &args);
        assert_eq!(code, 3, "{label}");
        assert!(
            stdout.is_empty(),
            "{label}: human mode wrote to stdout: {stdout}"
        );
        assert!(!stderr.is_empty(), "{label}: nothing reported on stderr");
    }
}

#[test]
fn a_command_that_prints_its_own_record_does_not_add_a_second() {
    // `scan secrets` reports a full record *and* exits 3 when the gate trips: the record is
    // the answer, the exit is the gate. A second document would make stdout unparseable.
    let repo = init_repo();
    let p = repo.path();
    fs::create_dir_all(p.join("src")).unwrap();
    fs::write(
        p.join("src/tok.txt"),
        "ghp_abcdefghijklmnopqrstuvwxyz0123456789\n",
    )
    .unwrap();

    let (code, stdout, _) = run(
        p,
        &["scan", "secrets", "src", "--fail-on", "high", "--json"],
    );
    assert_eq!(code, 3, "the gate should trip");
    assert_eq!(
        json_document_count(&stdout),
        1,
        "stdout must hold exactly one JSON document, got:\n{stdout}"
    );

    // And it is the command's own record, not a generic error document.
    let json: Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(json["kind"], "aikit.scan_secrets");
    assert_eq!(json["blocked_state"], "blocked_secret_findings");
}

#[test]
fn an_ordinary_failure_reports_a_null_blocked_state() {
    // A null `blocked_state` is how a caller tells a deterministic refusal (change something
    // and retry) from an environmental failure — without parsing the message.
    //
    // A regular file standing where a directory must be created fails on every platform,
    // which makes this an ordinary failure (exit 1) rather than a blocked state.
    let repo = init_repo();
    let p = repo.path();
    fs::write(p.join("blocker"), "not a directory\n").unwrap();

    let (code, stdout, stderr) = run(p, &["batch", "start", "--output", "blocker/sub", "--json"]);

    assert_eq!(code, 1, "expected an ordinary failure, stderr: {stderr}");
    assert_eq!(
        json_document_count(&stdout),
        1,
        "stdout must hold exactly one JSON document, got:\n{stdout}"
    );
    let json: Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(json["kind"], "aikit.error");
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_code"], 1);
    assert!(
        json["blocked_state"].is_null(),
        "an ordinary failure must report a null blocked_state: {json}"
    );
}

#[test]
fn a_cwd_that_cannot_be_entered_still_emits_a_record() {
    // Exit 2 is the right code — a directory that cannot be entered is an invalid argument
    // *value*, the same class as `--fail-on bogus`, which clap rejects with 2. But clap only
    // misses this one because validating it needs the filesystem, and aikit IS running here,
    // so it can honour `--json` where the parser physically cannot.
    let dir = TempDir::new().unwrap();
    let (code, stdout, stderr) = run(
        dir.path(),
        &["--cwd", "/no/such/directory/anywhere", "doctor", "--json"],
    );

    assert_eq!(
        code, 2,
        "an invalid argument value is exit 2, stderr: {stderr}"
    );
    assert_eq!(
        json_document_count(&stdout),
        1,
        "aikit caught this one, so it owes the caller a record:\n{stdout}"
    );
    let json: Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(json["kind"], "aikit.error");
    assert_eq!(json["exit_code"], 2);
    assert!(
        json["blocked_state"].is_null(),
        "not a named blocked state: {json}"
    );
    assert!(
        json["message"].as_str().unwrap().contains("--cwd"),
        "the message should name the offending argument: {json}"
    );
    // Prose still goes to stderr, so stdout stays a clean machine channel.
    assert!(stderr.contains("--cwd"), "stderr should explain it too");
}

#[test]
fn a_cwd_failure_writes_nothing_to_stdout_without_json() {
    let dir = TempDir::new().unwrap();
    let (code, stdout, stderr) = run(
        dir.path(),
        &["--cwd", "/no/such/directory/anywhere", "doctor"],
    );
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "human mode wrote to stdout: {stdout}");
    assert!(!stderr.is_empty());
}

#[test]
fn invalid_usage_stays_claps_and_emits_no_json() {
    // Exit 2 is clap's: it rejects the arguments before any aikit code runs, so there is no
    // command to report a record for. Pinning this keeps a future change from "helpfully"
    // emitting an aikit.error document for a command that was never dispatched.
    let dir = TempDir::new().unwrap();
    let (code, stdout, stderr) = run(
        dir.path(),
        &["output", "show", "anything", "--no-such-flag", "--json"],
    );

    assert_eq!(code, 2, "invalid usage is exit 2, stderr: {stderr}");
    assert_eq!(
        json_document_count(&stdout),
        0,
        "clap usage errors must not produce an aikit record:\n{stdout}"
    );
    assert!(!stderr.is_empty(), "clap should explain the usage error");
}
