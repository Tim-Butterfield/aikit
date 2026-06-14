//! Integration tests for `aikit review generate --files` and its help surfaces.
//!
//! Each test builds a throwaway Git repository in a temp directory and runs the
//! compiled `aikit` binary inside it. Coverage maps to the Batch 3 manifest's test
//! expectations (help, output files, repo-relative resolution, determinism, path
//! and symlink escapes, hashing, caps/truncation, total-byte omission, exactly-once
//! manifest entries, nested-backtick fencing, and output location).

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
    fs::create_dir_all(p.join("src")).unwrap();
    fs::write(p.join("src/main.rs"), "fn main() {}\n").unwrap();
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

/// Run `review generate --files <files...> [extra...] --json` and parse stdout.
fn review_json(dir: &Path, files: &[&str], extra: &[&str]) -> Value {
    let mut args = vec!["review", "generate", "--files"];
    args.extend_from_slice(files);
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

fn paths_of(json: &Value) -> Vec<String> {
    json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect()
}

/// Locate the single written review directory under the fallback base.
fn find_review_dir(repo: &Path) -> PathBuf {
    let base = repo.join(".aikit/outputs/reviews");
    let entry = fs::read_dir(&base)
        .unwrap_or_else(|_| panic!("review base {} missing", base.display()))
        .next()
        .expect("a review id dir")
        .expect("dir entry");
    entry.path()
}

/// Run `aikit batch start` and return the repo-relative path of the created anchor.
fn make_anchor(repo: &Path) -> String {
    aikit(repo).args(["batch", "start"]).assert().success();
    let batches = repo.join(".aikit/outputs/batches");
    let entry = fs::read_dir(&batches)
        .expect("batches dir exists")
        .next()
        .expect("an anchor file")
        .expect("dir entry");
    format!(
        ".aikit/outputs/batches/{}",
        entry.file_name().to_string_lossy()
    )
}

/// Run `review generate --anchor <anchor> [extra...] --json` and parse stdout.
fn anchor_review_json(dir: &Path, anchor: &str, extra: &[&str]) -> Value {
    let mut args = vec!["review", "generate", "--anchor", anchor];
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

// ---- help ----

#[test]
fn review_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["review", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("generate"));
}

#[test]
fn review_generate_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["review", "generate", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--files"))
        .stdout(predicates::str::contains("--max-file-bytes"))
        .stdout(predicates::str::contains("--max-total-bytes"))
        .stdout(predicates::str::contains("--max-file-lines"));
}

#[test]
fn review_generate_help_advertises_anchor_not_changed() {
    let out = AssertCommand::new(cargo_bin("aikit"))
        .args(["review", "generate", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&out);
    assert!(help.contains("--anchor"), "help should advertise --anchor");
    // `--changed` must not be offered as a flag (it may only appear as prose noting
    // it is not implemented). Check the Options section has no `--changed` flag line.
    let options = help.split("Options:").nth(1).unwrap_or("");
    assert!(
        !options
            .lines()
            .any(|l| l.trim_start().starts_with("--changed")),
        "--changed must not be offered as a flag"
    );
}

// ---- core behavior ----

#[test]
fn generate_creates_review_directory_and_files() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["review", "generate", "--files", "README.md", "src/main.rs"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Review bundle written"))
        // Human output references the new bundle file name.
        .stdout(predicates::str::contains("review_bundle.txt"));

    let dir = find_review_dir(repo.path());
    assert!(
        dir.join("review_bundle.txt").is_file(),
        "review_bundle.txt written"
    );
    assert!(dir.join("manifest.json").is_file(), "manifest.json written");
    // The old bundle file name must not be produced for new bundles.
    assert!(
        !dir.join("run_for_review.txt").exists(),
        "old run_for_review.txt must not be created"
    );
}

#[test]
fn manifest_has_expected_shape() {
    let repo = init_repo();
    let json = review_json(repo.path(), &["README.md", "src/main.rs"], &[]);

    for field in [
        "schema_version",
        "kind",
        "review_id",
        "repo_root",
        "git_head",
        "generated_at",
        "inputs",
        "limits",
        "files",
        "bundle_path",
        "totals",
    ] {
        assert!(json.get(field).is_some(), "manifest missing field: {field}");
    }
    assert_eq!(json["kind"], "aikit.review_bundle");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["bundle_path"], "review_bundle.txt");
    assert_eq!(json["inputs"]["mode"], "explicit_files");

    let f = &json["files"][0];
    for field in [
        "path",
        "size_bytes",
        "sha256",
        "included",
        "truncated",
        "lines_included",
        "bytes_included",
        "omitted_reason",
        "cap_hit",
    ] {
        assert!(f.get(field).is_some(), "file entry missing field: {field}");
    }
}

#[test]
fn inputs_resolved_repo_relative_and_sorted() {
    let repo = init_repo();
    // Pass in reverse order; manifest must be sorted by repo-relative path.
    let json = review_json(repo.path(), &["src/main.rs", "README.md"], &[]);
    let paths = paths_of(&json);
    assert_eq!(
        paths,
        vec!["README.md", "src/main.rs"],
        "sorted repo-relative"
    );
    for p in &paths {
        assert!(!Path::new(p).is_absolute(), "repo-relative path: {p}");
    }
}

#[test]
fn sha256_and_size_are_recorded() {
    let repo = init_repo();
    let json = review_json(repo.path(), &["README.md"], &[]);
    let f = &json["files"][0];
    let sha = f["sha256"].as_str().unwrap();
    assert_eq!(sha.len(), 64);
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(f["size_bytes"], "# readme\n".len() as u64);
}

#[test]
fn files_outside_repo_are_rejected() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["review", "generate", "--files", "/etc/hosts"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));

    // A `..` escape resolves outside the repo too — even when the target does not
    // exist, it is reported as a path escape (not merely unreadable).
    aikit(repo.path())
        .args(["review", "generate", "--files", "../escape.txt"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    symlink("/etc/hosts", repo.path().join("escape-link")).unwrap();
    aikit(repo.path())
        .args(["review", "generate", "--files", "escape-link"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_path_escape"));
}

#[cfg(unix)]
#[test]
fn symlinked_dir_with_parent_does_not_alias_to_other_file() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    // dirlink -> a/b ; only a/secret.txt exists (no top-level secret.txt).
    fs::create_dir_all(repo.path().join("a/b")).unwrap();
    fs::write(repo.path().join("a/secret.txt"), "secret\n").unwrap();
    symlink("a/b", repo.path().join("dirlink")).unwrap();

    // Lexically `dirlink/../secret.txt` is `secret.txt`, which does not exist at the
    // repo root, so it must be rejected rather than silently reading a/secret.txt.
    aikit(repo.path())
        .args(["review", "generate", "--files", "dirlink/../secret.txt"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_unreadable_file"));
}

#[test]
fn missing_input_file_is_blocked() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["review", "generate", "--files", "does-not-exist.txt"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_unreadable_file"));
}

// ---- caps ----

#[test]
fn max_file_bytes_truncates_and_records() {
    let repo = init_repo();
    fs::write(repo.path().join("big.txt"), "abcdefghij\n").unwrap();
    git(repo.path(), &["add", "big.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "big"]);

    let json = review_json(repo.path(), &["big.txt"], &["--max-file-bytes", "4"]);
    let f = &json["files"][0];
    assert_eq!(f["included"], true);
    assert_eq!(f["truncated"], true);
    assert_eq!(f["cap_hit"], "file_bytes");
    assert_eq!(f["bytes_included"], 4);
    assert_eq!(f["size_bytes"], 11);
}

#[test]
fn max_file_lines_truncates_and_records() {
    let repo = init_repo();
    fs::write(repo.path().join("lines.txt"), "l1\nl2\nl3\nl4\n").unwrap();
    git(repo.path(), &["add", "lines.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "lines"]);

    let json = review_json(repo.path(), &["lines.txt"], &["--max-file-lines", "2"]);
    let f = &json["files"][0];
    assert_eq!(f["truncated"], true);
    assert_eq!(f["cap_hit"], "file_lines");
    assert_eq!(f["lines_included"], 2);
    assert_eq!(f["bytes_included"], 6); // "l1\nl2\n"
}

#[test]
fn max_total_bytes_omits_later_files_deterministically() {
    let repo = init_repo();
    // aaaa.txt (sorts first) is 6 bytes; zzzz.txt is omitted once the cap is hit.
    fs::write(repo.path().join("aaaa.txt"), "12345\n").unwrap();
    fs::write(repo.path().join("zzzz.txt"), "67890\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-q", "-m", "two"]);

    let json = review_json(
        repo.path(),
        &["zzzz.txt", "aaaa.txt"],
        &["--max-total-bytes", "6"],
    );
    let files = json["files"].as_array().unwrap();
    let a = files.iter().find(|f| f["path"] == "aaaa.txt").unwrap();
    let z = files.iter().find(|f| f["path"] == "zzzz.txt").unwrap();
    assert_eq!(a["included"], true, "first sorted file fits within the cap");
    assert_eq!(z["included"], false, "later file omitted by total-byte cap");
    assert_eq!(z["omitted_reason"], "max_total_bytes");
    assert_eq!(z["cap_hit"], "total_bytes");
    // An omitted file still records its real hash and size.
    assert_eq!(z["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(z["size_bytes"], 6);
    assert_eq!(json["totals"]["files_included"], 1);
    assert_eq!(json["totals"]["files_omitted"], 1);

    // The omitted file's bundle section carries its hash and size (not blank).
    let dir = find_review_dir(repo.path());
    let bundle = fs::read_to_string(dir.join("review_bundle.txt")).unwrap();
    let z_sha = z["sha256"].as_str().unwrap();
    assert!(
        bundle.contains(&format!("### zzzz.txt\nSHA-256: {z_sha}\nSize: 6")),
        "omitted bundle section must include the real SHA-256 and size"
    );
    assert!(bundle.contains("Omitted: max_total_bytes"));
}

#[test]
fn every_scoped_file_appears_exactly_once() {
    let repo = init_repo();
    // Duplicate inputs (and a relative form) must collapse to a single entry.
    let json = review_json(repo.path(), &["README.md", "README.md", "./README.md"], &[]);
    let paths = paths_of(&json);
    assert_eq!(paths, vec!["README.md"], "duplicates collapse to one entry");
    assert_eq!(json["totals"]["files_total"], 1);
}

#[test]
fn nested_backticks_do_not_break_the_bundle() {
    let repo = init_repo();
    fs::write(
        repo.path().join("fences.md"),
        "before\n```\ncode\n```\nafter\n",
    )
    .unwrap();
    git(repo.path(), &["add", "fences.md"]);
    git(repo.path(), &["commit", "-q", "-m", "fences"]);

    aikit(repo.path())
        .args(["review", "generate", "--files", "fences.md"])
        .assert()
        .success();
    let dir = find_review_dir(repo.path());
    let bundle = fs::read_to_string(dir.join("review_bundle.txt")).unwrap();
    // The wrapper fence must be longer than the file's own ``` run.
    assert!(
        bundle.contains("````text"),
        "expected a 4-backtick wrapper fence around content with a triple-backtick run"
    );
    // The original content is preserved.
    assert!(bundle.contains("```\ncode\n```"));
}

// ---- output location ----

#[test]
fn review_default_output_is_aikit_outputs_reviews() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["review", "generate", "--files", "README.md"])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/outputs/reviews/"));
    assert!(
        repo.path().join(".aikit/outputs/reviews").is_dir(),
        "default review output under .aikit/outputs/reviews"
    );
}

#[test]
fn review_default_ignores_scratch_even_when_present() {
    let repo = init_repo();
    fs::create_dir_all(repo.path().join(".scratch/work/outputs")).unwrap();
    aikit(repo.path())
        .args(["review", "generate", "--files", "README.md"])
        .assert()
        .success();
    assert!(
        repo.path().join(".aikit/outputs/reviews").is_dir(),
        "default output stays under .aikit/outputs even when .scratch exists"
    );
    assert!(
        !repo.path().join(".scratch/work/outputs/aikit").exists(),
        ".scratch must never be auto-selected for output"
    );
}

#[test]
fn review_output_override_is_honored() {
    let repo = init_repo();
    aikit(repo.path())
        .args([
            "review",
            "generate",
            "--files",
            "README.md",
            "--output",
            ".scratch/work/outputs/aikit",
        ])
        .assert()
        .success();
    assert!(
        repo.path()
            .join(".scratch/work/outputs/aikit/reviews")
            .is_dir(),
        "explicit --output should be used as requested"
    );
    assert!(
        !repo.path().join(".aikit/outputs").exists(),
        "default .aikit/outputs should not be created when --output is given"
    );
}

#[test]
fn review_json_includes_written_artifact_paths() {
    let repo = init_repo();
    let json = review_json(repo.path(), &["README.md"], &[]);
    let written: Vec<String> = json["written"]
        .as_array()
        .expect("written array present")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        written
            .iter()
            .any(|w| w.starts_with(".aikit/outputs/reviews/") && w.ends_with("review_bundle.txt")),
        "written must list review_bundle.txt: {written:?}"
    );
    assert!(
        written.iter().any(|w| w.ends_with("manifest.json")),
        "written must list manifest.json: {written:?}"
    );
}

// ---- anchor-driven mode ----

#[test]
fn anchor_mode_creates_review_dir_and_files() {
    let repo = init_repo();
    let anchor = make_anchor(repo.path());
    // Change a tracked file after the anchor.
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();

    aikit(repo.path())
        .args(["review", "generate", "--anchor", &anchor])
        .assert()
        .success()
        .stdout(predicates::str::contains(".aikit/outputs/reviews/"));

    let dir = find_review_dir(repo.path());
    assert!(dir.join("review_bundle.txt").is_file());
    assert!(dir.join("manifest.json").is_file());
    assert!(
        !dir.join("run_for_review.txt").exists(),
        "old run_for_review.txt must not be created in anchor mode"
    );
}

#[test]
fn anchor_mode_records_mode_and_anchor_in_manifest() {
    let repo = init_repo();
    let anchor = make_anchor(repo.path());
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();

    let json = anchor_review_json(repo.path(), &anchor, &[]);
    assert_eq!(json["inputs"]["mode"], "changed_since_anchor");
    assert_eq!(json["inputs"]["anchor_path"], anchor);
    assert!(
        !json["inputs"]["anchor_id"].as_str().unwrap().is_empty(),
        "anchor_id should be recorded"
    );
    // --json includes the created artifact paths.
    let written: Vec<String> = json["written"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(written.iter().any(|w| w.ends_with("review_bundle.txt")));
    assert!(written.iter().any(|w| w.ends_with("manifest.json")));
}

#[test]
fn anchor_mode_includes_changed_excludes_unchanged() {
    let repo = init_repo();
    // README.md and src/main.rs are committed by init_repo; commit one more.
    fs::write(repo.path().join("stable.txt"), "stable\n").unwrap();
    git(repo.path(), &["add", "stable.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "add stable"]);

    let anchor = make_anchor(repo.path());
    // Change only README.md after the anchor.
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();

    let json = anchor_review_json(repo.path(), &anchor, &[]);
    let paths = paths_of(&json);
    assert!(
        paths.contains(&"README.md".to_string()),
        "changed file included"
    );
    assert!(
        !paths.contains(&"stable.txt".to_string()),
        "unchanged file excluded"
    );
    assert!(
        !paths.contains(&"src/main.rs".to_string()),
        "unchanged file excluded"
    );
}

#[test]
fn anchor_mode_missing_anchor_is_rejected() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["review", "generate", "--anchor", "does-not-exist.json"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_missing_anchor"));
}

#[test]
fn anchor_mode_invalid_anchor_is_rejected() {
    let repo = init_repo();
    fs::write(repo.path().join("bad.json"), "{ not valid json").unwrap();
    aikit(repo.path())
        .args(["review", "generate", "--anchor", "bad.json"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn anchor_mode_foreign_anchor_is_rejected() {
    let repo_a = init_repo();
    let anchor_rel = make_anchor(repo_a.path());
    let foreign = repo_a.path().join(&anchor_rel);

    let repo_b = init_repo();
    aikit(repo_b.path())
        .args(["review", "generate", "--anchor", foreign.to_str().unwrap()])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_invalid_anchor"));
}

#[test]
fn both_files_and_anchor_is_invalid_usage() {
    let repo = init_repo();
    let anchor = make_anchor(repo.path());
    aikit(repo.path())
        .args([
            "review",
            "generate",
            "--files",
            "README.md",
            "--anchor",
            &anchor,
        ])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn neither_files_nor_anchor_is_invalid_usage() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["review", "generate"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn anchor_mode_default_output_ignores_scratch_even_when_present() {
    let repo = init_repo();
    fs::create_dir_all(repo.path().join(".scratch/work/outputs")).unwrap();
    let anchor = make_anchor(repo.path());
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();

    aikit(repo.path())
        .args(["review", "generate", "--anchor", &anchor])
        .assert()
        .success();
    assert!(
        repo.path().join(".aikit/outputs/reviews").is_dir(),
        "anchor-mode default output stays under .aikit/outputs even when .scratch exists"
    );
    assert!(
        !repo.path().join(".scratch/work/outputs/aikit").exists(),
        ".scratch must never be auto-selected"
    );
}

#[test]
fn anchor_mode_output_override_is_honored() {
    let repo = init_repo();
    let anchor = make_anchor(repo.path());
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();

    aikit(repo.path())
        .args([
            "review",
            "generate",
            "--anchor",
            &anchor,
            "--output",
            ".scratch/work/outputs/aikit",
        ])
        .assert()
        .success();
    assert!(
        repo.path()
            .join(".scratch/work/outputs/aikit/reviews")
            .is_dir(),
        "explicit --output should be used as requested"
    );
}

#[test]
fn anchor_mode_respects_per_file_caps() {
    let repo = init_repo();
    let anchor = make_anchor(repo.path());
    // Change README.md to long content, then cap it.
    fs::write(repo.path().join("README.md"), "abcdefghij\n").unwrap();

    let json = anchor_review_json(repo.path(), &anchor, &["--max-file-bytes", "4"]);
    let f = json["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "README.md")
        .expect("changed file present");
    assert_eq!(f["truncated"], true);
    assert_eq!(f["cap_hit"], "file_bytes");
    assert_eq!(f["bytes_included"], 4);
}

#[test]
fn anchor_mode_includes_renamed_destination() {
    let repo = init_repo();
    fs::write(repo.path().join("old.txt"), "content\n").unwrap();
    git(repo.path(), &["add", "old.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "add old"]);

    let anchor = make_anchor(repo.path());
    git(repo.path(), &["mv", "old.txt", "new.txt"]);

    let json = anchor_review_json(repo.path(), &anchor, &[]);
    let paths = paths_of(&json);
    assert!(
        paths.contains(&"new.txt".to_string()),
        "rename destination included"
    );
    assert!(
        !paths.contains(&"old.txt".to_string()),
        "renamed-from path not bundled"
    );
}

#[test]
fn anchor_mode_excludes_deleted_rename_destination() {
    let repo = init_repo();
    fs::write(repo.path().join("old.txt"), "content\n").unwrap();
    git(repo.path(), &["add", "old.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "add old"]);

    let anchor = make_anchor(repo.path());
    // Stage a rename, then delete the destination in the worktree (status `RD`).
    git(repo.path(), &["mv", "old.txt", "new.txt"]);
    fs::remove_file(repo.path().join("new.txt")).unwrap();

    // The review must still succeed (the missing destination is simply excluded).
    let json = anchor_review_json(repo.path(), &anchor, &[]);
    let paths = paths_of(&json);
    assert!(
        !paths.contains(&"new.txt".to_string()),
        "deleted rename destination must not be bundled"
    );
}

#[test]
fn anchor_mode_excludes_deleted_tracked_file() {
    let repo = init_repo();
    fs::write(repo.path().join("del.txt"), "bye\n").unwrap();
    git(repo.path(), &["add", "del.txt"]);
    git(repo.path(), &["commit", "-q", "-m", "add del"]);

    let anchor = make_anchor(repo.path());
    fs::remove_file(repo.path().join("del.txt")).unwrap();

    let json = anchor_review_json(repo.path(), &anchor, &[]);
    assert!(
        !paths_of(&json).contains(&"del.txt".to_string()),
        "deleted tracked file is not bundle-able"
    );
}

#[test]
fn anchor_mode_excludes_untracked_file() {
    let repo = init_repo();
    let anchor = make_anchor(repo.path());
    fs::write(repo.path().join("README.md"), "# readme\nchanged\n").unwrap();
    fs::write(repo.path().join("untracked.txt"), "new\n").unwrap();

    let json = anchor_review_json(repo.path(), &anchor, &[]);
    let paths = paths_of(&json);
    assert!(
        paths.contains(&"README.md".to_string()),
        "tracked change included"
    );
    assert!(
        !paths.contains(&"untracked.txt".to_string()),
        "untracked file excluded from anchor-driven review"
    );
}

#[test]
fn anchor_mode_respects_total_byte_cap() {
    let repo = init_repo();
    fs::write(repo.path().join("aaaa.txt"), "12345\n").unwrap();
    fs::write(repo.path().join("zzzz.txt"), "67890\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-q", "-m", "two"]);

    let anchor = make_anchor(repo.path());
    fs::write(repo.path().join("aaaa.txt"), "12345x\n").unwrap();
    fs::write(repo.path().join("zzzz.txt"), "67890x\n").unwrap();

    let json = anchor_review_json(repo.path(), &anchor, &["--max-total-bytes", "7"]);
    let files = json["files"].as_array().unwrap();
    let a = files.iter().find(|f| f["path"] == "aaaa.txt").unwrap();
    let z = files.iter().find(|f| f["path"] == "zzzz.txt").unwrap();
    assert_eq!(a["included"], true, "first sorted file fits");
    assert_eq!(z["included"], false, "later file omitted by total cap");
    assert_eq!(z["omitted_reason"], "max_total_bytes");
    assert_eq!(z["cap_hit"], "total_bytes");
}
