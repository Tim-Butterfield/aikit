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

// ---- core behavior ----

#[test]
fn generate_creates_review_directory_and_files() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["review", "generate", "--files", "README.md", "src/main.rs"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Review bundle written"));

    let dir = find_review_dir(repo.path());
    assert!(
        dir.join("run_for_review.txt").is_file(),
        "run_for_review.txt written"
    );
    assert!(dir.join("manifest.json").is_file(), "manifest.json written");
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
    assert_eq!(json["bundle_path"], "run_for_review.txt");
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
    let bundle = fs::read_to_string(dir.join("run_for_review.txt")).unwrap();
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
    let bundle = fs::read_to_string(dir.join("run_for_review.txt")).unwrap();
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
            .any(|w| w.starts_with(".aikit/outputs/reviews/") && w.ends_with("run_for_review.txt")),
        "written must list run_for_review.txt: {written:?}"
    );
    assert!(
        written.iter().any(|w| w.ends_with("manifest.json")),
        "written must list manifest.json: {written:?}"
    );
}
