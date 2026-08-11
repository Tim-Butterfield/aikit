//! Integration tests for `aikit inventory repo` and its help surfaces.
//!
//! Each test builds a throwaway Git repository in a temp directory and runs the
//! compiled `aikit` binary inside it. Coverage maps to the Batch 2 manifest's
//! test expectations (help availability, JSON shape, `.git`/default-dir exclusion,
//! gitignore handling, SHA-256, determinism, `--max-files`, and output location).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command as AssertCommand;
use serde_json::Value;
use tempfile::TempDir;

/// Run `git` in `dir`, asserting success; global/system config neutralized.
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

/// Create a temp Git repo with a couple of committed files.
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

/// Run `inventory repo --json [extra...]` and parse the stdout JSON.
fn inventory_json(dir: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["inventory", "repo", "--json"];
    args.extend_from_slice(extra);
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

/// Find the single written inventory directory under a base output location.
fn find_inventory_dir(base: &Path) -> PathBuf {
    let entry = fs::read_dir(base)
        .unwrap_or_else(|_| panic!("inventory base {} missing", base.display()))
        .next()
        .expect("at least one inventory id dir")
        .expect("dir entry");
    entry.path()
}

// ---- help ----

#[test]
fn inventory_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["inventory", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("repo"));
}

#[test]
fn inventory_repo_help_is_available() {
    AssertCommand::new(cargo_bin("aikit"))
        .args(["inventory", "repo", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--include-ignored"))
        .stdout(predicates::str::contains("--max-files"))
        .stdout(predicates::str::contains("--json"));
}

// ---- core behavior ----

#[test]
fn inventory_repo_inventories_simple_repo_with_expected_json() {
    let repo = init_repo();
    let json = inventory_json(repo.path(), &[]);

    for field in [
        "schema_version",
        "kind",
        "inventory_id",
        "repo_root",
        "git_head",
        "generated_at",
        "files",
        "counts",
    ] {
        assert!(
            json.get(field).is_some(),
            "inventory missing field: {field}"
        );
    }
    assert_eq!(json["kind"], "aikit.repo_inventory");
    assert_eq!(json["schema_version"], 1);

    let paths = paths_of(&json);
    assert!(paths.contains(&"README.md".to_string()));
    assert!(paths.contains(&"src/main.rs".to_string()));
    assert_eq!(json["counts"]["files"], paths.len());
}

#[test]
fn git_dir_is_always_excluded() {
    let repo = init_repo();
    let json = inventory_json(repo.path(), &["--include-ignored"]);
    assert!(
        paths_of(&json).iter().all(|p| !p.starts_with(".git/")),
        ".git/ must never appear in the inventory"
    );
}

#[test]
fn output_is_deterministic_and_repo_relative() {
    let repo = init_repo();
    let a = paths_of(&inventory_json(repo.path(), &[]));
    let b = paths_of(&inventory_json(repo.path(), &[]));
    assert_eq!(a, b, "inventory file ordering must be deterministic");
    let mut sorted = a.clone();
    sorted.sort();
    assert_eq!(a, sorted, "paths must be lexicographically sorted");
    for p in &a {
        assert!(
            !Path::new(p).is_absolute(),
            "path must be repo-relative: {p}"
        );
    }
}

#[test]
fn sha256_is_computed_for_files() {
    let repo = init_repo();
    let json = inventory_json(repo.path(), &[]);
    for f in json["files"].as_array().unwrap() {
        let sha = f["sha256"].as_str().unwrap();
        assert_eq!(sha.len(), 64, "SHA-256 hex digest expected");
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(f["size_bytes"].is_u64());
        assert!(f["kind_hint"].is_string());
    }
    // README.md should be classified as markdown.
    let readme = json["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "README.md")
        .unwrap();
    assert_eq!(readme["kind_hint"], "markdown");
}

#[test]
fn gitignore_is_respected_by_default_and_overridable() {
    let repo = init_repo();
    fs::write(repo.path().join(".gitignore"), "*.log\n").unwrap();
    fs::write(repo.path().join("debug.log"), "noise\n").unwrap();
    git(repo.path(), &["add", ".gitignore"]);
    git(repo.path(), &["commit", "-q", "-m", "gitignore"]);

    let default_paths = paths_of(&inventory_json(repo.path(), &[]));
    assert!(
        !default_paths.contains(&"debug.log".to_string()),
        "gitignored file excluded by default"
    );

    let included_paths = paths_of(&inventory_json(repo.path(), &["--include-ignored"]));
    assert!(
        included_paths.contains(&"debug.log".to_string()),
        "gitignored file included with --include-ignored"
    );
}

#[test]
fn default_directories_excluded_by_directory_only_rules() {
    let repo = init_repo();
    // A file whose name merely contains an excluded name, and a top-level file
    // literally named like an excluded dir, must both be INCLUDED.
    fs::write(repo.path().join("mytarget.txt"), "keep\n").unwrap();
    fs::write(repo.path().join("build"), "this is a file, not a dir\n").unwrap();
    // Real excluded directories must be pruned even when not gitignored.
    fs::create_dir_all(repo.path().join("target/debug")).unwrap();
    fs::write(repo.path().join("target/debug/artifact"), "junk\n").unwrap();
    fs::create_dir_all(repo.path().join("node_modules/pkg")).unwrap();
    fs::write(repo.path().join("node_modules/pkg/index.js"), "x\n").unwrap();
    git(repo.path(), &["add", "mytarget.txt", "build"]);
    git(repo.path(), &["commit", "-q", "-m", "edge files"]);

    let paths = paths_of(&inventory_json(repo.path(), &["--include-ignored"]));
    assert!(
        paths.contains(&"mytarget.txt".to_string()),
        "substring name kept"
    );
    assert!(
        paths.contains(&"build".to_string()),
        "file named like an excluded dir is kept (directory-only)"
    );
    assert!(
        paths.iter().all(|p| !p.starts_with("target/")),
        "target/ directory excluded"
    );
    assert!(
        paths.iter().all(|p| !p.starts_with("node_modules/")),
        "node_modules/ directory excluded"
    );
}

#[test]
fn max_files_limits_deterministically_and_reports() {
    let repo = init_repo();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(repo.path().join(name), "x\n").unwrap();
    }
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-q", "-m", "more files"]);

    let json = inventory_json(repo.path(), &["--max-files", "2"]);
    let paths = paths_of(&json);
    assert_eq!(paths.len(), 2, "limited to 2 files");
    assert_eq!(json["counts"]["truncated"], true);
    assert_eq!(json["counts"]["max_files"], 2);
    assert!(json["counts"]["total_discovered"].as_u64().unwrap() > 2);
    assert!(json["notes"]
        .as_array()
        .map(|n| !n.is_empty())
        .unwrap_or(false));

    // Deterministic: the same first-N across runs.
    let again = paths_of(&inventory_json(repo.path(), &["--max-files", "2"]));
    assert_eq!(paths, again);
}

// ---- output location ----

#[test]
fn inventory_default_output_is_aikit_outputs() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["inventory", "repo"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Repository inventory written"))
        .stdout(predicates::str::contains(".aikit/outputs/inventory/"));

    let base = repo.path().join(".aikit/outputs/inventory");
    assert!(base.is_dir(), "default inventory base should exist");
    let dir = find_inventory_dir(&base);
    assert!(
        dir.join("inventory.json").is_file(),
        "inventory.json written"
    );
    assert!(dir.join("inventory.txt").is_file(), "inventory.txt written");
}

#[test]
fn inventory_default_ignores_scratch_even_when_present() {
    let repo = init_repo();
    fs::create_dir_all(repo.path().join(".scratch/work/outputs")).unwrap();
    aikit(repo.path())
        .args(["inventory", "repo"])
        .assert()
        .success();
    assert!(
        repo.path().join(".aikit/outputs/inventory").is_dir(),
        "default output stays under .aikit/outputs even when .scratch exists"
    );
    assert!(
        !repo.path().join(".scratch/work/outputs/aikit").exists(),
        ".scratch must never be auto-selected for output"
    );
}

#[test]
fn inventory_output_override_is_honored() {
    let repo = init_repo();
    aikit(repo.path())
        .args([
            "inventory",
            "repo",
            "--output",
            ".scratch/work/outputs/aikit",
        ])
        .assert()
        .success();
    assert!(
        repo.path()
            .join(".scratch/work/outputs/aikit/inventory")
            .is_dir(),
        "explicit --output should be used as requested"
    );
    assert!(
        !repo.path().join(".aikit/outputs").exists(),
        "default .aikit/outputs should not be created when --output is given"
    );
}

#[test]
fn inventory_json_includes_written_artifact_paths() {
    let repo = init_repo();
    let json = inventory_json(repo.path(), &[]);
    let written: Vec<String> = json["written"]
        .as_array()
        .expect("written array present")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        written
            .iter()
            .any(|w| w.starts_with(".aikit/outputs/inventory/") && w.ends_with("inventory.json")),
        "written must list the created inventory.json: {written:?}"
    );
    assert!(
        written.iter().any(|w| w.ends_with("inventory.txt")),
        "written must list the created inventory.txt: {written:?}"
    );
}

#[test]
fn inventory_excludes_its_own_output_by_default() {
    let repo = init_repo();
    // First run creates .aikit/outputs/inventory/... ; a second run must not list it.
    aikit(repo.path())
        .args(["inventory", "repo"])
        .assert()
        .success();
    let json = inventory_json(repo.path(), &["--include-ignored"]);
    assert!(
        paths_of(&json)
            .iter()
            .all(|p| !p.starts_with(".aikit/outputs/")),
        "aikit's own output area must be excluded"
    );
}

#[test]
fn custom_output_dir_inside_repo_is_excluded_on_later_runs() {
    let repo = init_repo();
    // First run writes into an in-repo --output dir; a later run must not list it.
    aikit(repo.path())
        .args(["inventory", "repo", "--output", "inv-out"])
        .assert()
        .success();
    assert!(repo.path().join("inv-out/inventory").is_dir());

    let json = inventory_json(repo.path(), &["--output", "inv-out", "--include-ignored"]);
    assert!(
        paths_of(&json).iter().all(|p| !p.starts_with("inv-out/")),
        "the run's own --output directory must be excluded from traversal"
    );
}

#[test]
fn output_dot_excludes_inventory_subdir_on_later_runs() {
    let repo = init_repo();
    // Output root equal to the repo root (`--output .`) must still exclude the
    // command's own `inventory/` write subdir on subsequent runs.
    aikit(repo.path())
        .args(["inventory", "repo", "--output", "."])
        .assert()
        .success();
    assert!(repo.path().join("inventory").is_dir());

    let json = inventory_json(repo.path(), &["--output", ".", "--include-ignored"]);
    assert!(
        paths_of(&json).iter().all(|p| !p.starts_with("inventory/")),
        "the command's own inventory/ output subdir must be excluded"
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_recorded_not_followed() {
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    symlink("README.md", repo.path().join("link-to-readme")).unwrap();
    git(repo.path(), &["add", "link-to-readme"]);
    git(repo.path(), &["commit", "-q", "-m", "add symlink"]);

    let json = inventory_json(repo.path(), &[]);
    let files = json["files"].as_array().unwrap();
    let link = files
        .iter()
        .find(|f| f["path"] == "link-to-readme")
        .expect("symlink recorded in inventory");
    assert_eq!(link["kind_hint"], "symlink");
    assert_eq!(link["sha256"].as_str().unwrap().len(), 64);
    // The symlink digest hashes the target path, not the target's contents, so it
    // must differ from README.md's own content hash (i.e. the link is not followed).
    let readme = files.iter().find(|f| f["path"] == "README.md").unwrap();
    assert_ne!(link["sha256"], readme["sha256"]);
}

#[cfg(unix)]
#[test]
fn symlink_targets_hashed_by_native_bytes_not_lossy() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    let repo = init_repo();
    // Two distinct non-UTF-8 targets. A lossy conversion would map both to U+FFFD
    // and produce identical digests; native-byte hashing keeps them distinct.
    symlink(OsStr::from_bytes(&[0xff]), repo.path().join("link_a")).unwrap();
    symlink(OsStr::from_bytes(&[0xfe]), repo.path().join("link_b")).unwrap();

    let json = inventory_json(repo.path(), &[]);
    let files = json["files"].as_array().unwrap();
    let a = files
        .iter()
        .find(|f| f["path"] == "link_a")
        .expect("link_a");
    let b = files
        .iter()
        .find(|f| f["path"] == "link_b")
        .expect("link_b");
    assert_eq!(a["kind_hint"], "symlink");
    assert_ne!(
        a["sha256"], b["sha256"],
        "distinct non-UTF-8 symlink targets must hash differently"
    );
}
