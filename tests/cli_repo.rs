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

/// Build a throwaway Mercurial repo by creating the `.hg/` marker directory. aikit
/// detects Mercurial by walking up for `.hg/` (no `hg` binary required), so these
/// tests run even where Mercurial is not installed. The dir is not a Git repo, so
/// `repo init` falls through to the Mercurial branch.
fn init_hg_repo() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir_all(dir.path().join(".hg")).expect("create .hg");
    dir
}

fn read_hg_ignore(repo: &Path) -> String {
    fs::read_to_string(repo.join(".hg/hgignore.aikit")).unwrap_or_default()
}

fn read_hgrc(repo: &Path) -> String {
    fs::read_to_string(repo.join(".hg/hgrc")).unwrap_or_default()
}

// ---- help ----

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
fn top_level_help_lists_init_and_doctor() {
    // They are top-level commands now; there is no `repo` group to look inside.
    let help = help_text(&["--help"]);
    assert!(help.contains("init"));
    assert!(help.contains("doctor"));
}

#[test]
fn the_removed_subcommand_groups_are_gone() {
    // `repo`/`folder` held only init and doctor, which are now flags on the adaptive
    // commands. A group that still parsed would be a second way to say the same thing.
    for args in [
        vec!["repo", "init"],
        vec!["repo", "doctor"],
        vec!["folder", "init"],
    ] {
        let out = AssertCommand::new(cargo_bin("aikit"))
            .args(&args)
            .output()
            .expect("run");
        assert!(
            !out.status.success(),
            "{args:?} should no longer be accepted"
        );
    }
}

#[test]
fn init_help_describes_setup() {
    let help = help_text(&["init", "--help"]);
    for needle in [
        ".aikit/temp/",
        ".git/info/exclude",
        ".gitignore",
        "Idempotent",
        // The assertion flags that replaced the removed subcommands.
        "--require-repo",
        "--require-folder",
    ] {
        assert!(help.contains(needle), "init --help missing {needle:?}");
    }
}

#[test]
fn doctor_help_says_read_only() {
    let help = help_text(&["doctor", "--help"]);
    for needle in [
        "Read-only",
        "readiness",
        "creates no files",
        // VCS-aware: non-repo readiness exception + Mercurial coverage.
        "non-repo",
        ".hgignore",
        "Mercurial",
        // The assertion flag that replaced `repo doctor`.
        "--require-root",
    ] {
        assert!(help.contains(needle), "doctor --help missing {needle:?}");
    }
}

#[test]
fn doctor_help_describes_runner_availability() {
    let help = help_text(&["doctor", "--help"]);
    assert!(
        help.contains("runner"),
        "doctor --help should mention runner availability"
    );
    assert!(
        help.contains("any_runner_available") || help.contains("runners"),
        "doctor --help should reference the runners/any_runner_available fields"
    );
    // Must not present /bin/sh + /bin/zsh as readiness-gating interpreters.
    assert!(
        !help.contains("supported interpreters (`/bin/sh`, `/bin/zsh`)"),
        "doctor --help still presents /bin/sh + /bin/zsh as gating interpreters"
    );
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
        .args(["init", "--require-repo"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

#[test]
fn init_creates_aikit_and_temp_only() {
    let repo = init_repo();
    let p = repo.path();
    aikit(p).args(["init", "--require-repo"]).assert().success();
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
    let json = json_out(p, &["init", "--require-repo"]);
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
    let first = json_out(p, &["init", "--require-repo"]);
    assert_eq!(first["info_exclude_updated"], true);
    assert!(first["created_dirs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d == ".aikit/temp"));

    // Second run is a no-op for dirs and ignore coverage.
    let second = json_out(p, &["init", "--require-repo"]);
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
    let json = json_out(p, &["init", "--require-repo"]);
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
    let json = json_out(p, &["init", "--require-repo"]);
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

// ---- repo init (Mercurial) ----

#[test]
fn init_hg_writes_local_ignore_and_hgrc() {
    let repo = init_hg_repo();
    let p = repo.path();
    let json = json_out(p, &["init", "--require-repo"]);

    assert_eq!(json["kind"], "aikit.repo_init");
    assert_eq!(json["vcs"], "mercurial");
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["info_exclude_updated"], true);
    assert_eq!(json["ignore_source"], ".hg/hgignore.aikit");

    // Dirs created as usual.
    assert!(p.join(".aikit").is_dir());
    assert!(p.join(".aikit/temp").is_dir());

    // Rooted, syntax-explicit pattern in the local ignore file.
    assert!(
        read_hg_ignore(p).contains(r"re:^\.aikit/"),
        ".hg/hgignore.aikit should contain the rooted .aikit/ pattern"
    );

    // hgrc registers the ignore file under [ui].
    let hgrc = read_hgrc(p);
    assert!(hgrc.contains("[ui]"), "hgrc should contain a [ui] section");
    assert!(
        hgrc.contains("ignore.aikit = .hg/hgignore.aikit"),
        "hgrc should register the ignore file via [ui] ignore.aikit"
    );

    // Never touches the tracked root .hgignore or any Git state.
    assert!(!p.join(".hgignore").exists());
    assert!(!p.join(".git").exists());
}

#[test]
fn init_hg_is_idempotent() {
    let repo = init_hg_repo();
    let p = repo.path();

    let first = json_out(p, &["init", "--require-repo"]);
    assert_eq!(first["info_exclude_updated"], true);

    let second = json_out(p, &["init", "--require-repo"]);
    assert_eq!(second["created_dirs"].as_array().unwrap().len(), 0);
    assert_eq!(second["info_exclude_updated"], false);
    assert_eq!(second["aikit_ignored"], true);
    assert_eq!(second["ignore_source"], ".hg/hgignore.aikit");

    // Pattern present exactly once; ignore.aikit registered exactly once.
    let pat_count = read_hg_ignore(p)
        .lines()
        .filter(|l| l.trim() == r"re:^\.aikit/")
        .count();
    assert_eq!(pat_count, 1, "ignore pattern must not be duplicated");
    let entry_count = read_hgrc(p)
        .lines()
        .filter(|l| l.trim().starts_with("ignore.aikit"))
        .count();
    assert_eq!(entry_count, 1, "ignore.aikit entry must not be duplicated");
}

#[test]
fn init_hg_skips_when_hgignore_already_covers_aikit() {
    let repo = init_hg_repo();
    let p = repo.path();
    // Pre-existing tracked .hgignore already covers .aikit/ (default regexp syntax).
    fs::write(p.join(".hgignore"), "syntax: glob\n.aikit/\n").unwrap();

    let json = json_out(p, &["init", "--require-repo"]);
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["info_exclude_updated"], false);
    assert_eq!(json["ignore_source"], ".hgignore");

    // Must not create local config when already covered.
    assert!(
        !p.join(".hg/hgignore.aikit").exists(),
        "must not add local ignore file when already covered by .hgignore"
    );
    assert!(!read_hgrc(p).contains("ignore.aikit"));
}

#[test]
fn init_hg_managed_entry_wins_over_later_conflicting_ui_section() {
    let repo = init_hg_repo();
    let p = repo.path();
    // Two [ui] sections; the LATER one has a conflicting ignore.aikit. Mercurial merges
    // [ui] sections and the last value wins, so our entry must land in the last [ui].
    fs::write(
        p.join(".hg/hgrc"),
        "[ui]\nusername = x\n\n[paths]\ndefault = https://example/repo\n\n[ui]\nignore.aikit = something-else.txt\n",
    )
    .unwrap();

    let json = json_out(p, &["init", "--require-repo"]);
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["ignore_source"], ".hg/hgignore.aikit");

    let hgrc = read_hgrc(p);
    let ours = hgrc
        .find("ignore.aikit = .hg/hgignore.aikit")
        .expect("managed entry must be registered");
    let conflicting = hgrc
        .find("ignore.aikit = something-else.txt")
        .expect("pre-existing conflicting line must be preserved");
    // Ours must come after the conflicting one so Mercurial's last-wins applies it.
    assert!(
        ours > conflicting,
        "managed entry must win over a later conflicting [ui] section:\n{hgrc}"
    );
}

#[test]
fn init_hg_skips_when_hgignore_covers_aikit_via_content_glob() {
    let repo = init_hg_repo();
    let p = repo.path();
    // A directory-content glob that covers the whole .aikit/ tree (not the exact `.aikit`).
    fs::write(p.join(".hgignore"), "syntax: glob\n.aikit/**\n").unwrap();

    let json = json_out(p, &["init", "--require-repo"]);
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["info_exclude_updated"], false);
    assert_eq!(json["ignore_source"], ".hgignore");
    assert!(
        !p.join(".hg/hgignore.aikit").exists(),
        "must not add local ignore config when a content-glob already covers .aikit/"
    );
}

#[test]
fn init_hg_preserves_existing_hgrc_and_inserts_under_ui() {
    let repo = init_hg_repo();
    let p = repo.path();
    // Pre-existing hgrc with a [ui] section and an unrelated [paths] section.
    fs::write(
        p.join(".hg/hgrc"),
        "[ui]\nusername = Test User <t@e.com>\n\n[paths]\ndefault = https://example/repo\n",
    )
    .unwrap();

    json_out(p, &["init", "--require-repo"]);
    let hgrc = read_hgrc(p);

    // Existing content preserved.
    assert!(hgrc.contains("username = Test User <t@e.com>"));
    assert!(hgrc.contains("[paths]"));
    assert!(hgrc.contains("default = https://example/repo"));
    // New entry inserted under the existing [ui] section (no second [ui] header).
    assert!(hgrc.contains("ignore.aikit = .hg/hgignore.aikit"));
    assert_eq!(
        hgrc.matches("[ui]").count(),
        1,
        "must reuse the existing [ui] section, not add a second"
    );
}

#[test]
fn init_hg_registers_managed_entry_despite_conflicting_ignore_aikit_value() {
    let repo = init_hg_repo();
    let p = repo.path();
    // A pre-existing, unrelated `ignore.aikit` key pointing at a different file.
    fs::write(
        p.join(".hg/hgrc"),
        "[ui]\nignore.aikit = something-else.txt\n",
    )
    .unwrap();

    let json = json_out(p, &["init", "--require-repo"]);
    // Coverage must be reported via our managed file, not the unrelated value.
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["ignore_source"], ".hg/hgignore.aikit");
    assert_eq!(json["info_exclude_updated"], true);

    // Our managed entry is present AND ordered AFTER the conflicting one, so Mercurial's
    // last-wins makes our value effective (not just textually present).
    let hgrc = read_hgrc(p);
    let ours = hgrc
        .find("ignore.aikit = .hg/hgignore.aikit")
        .expect("managed entry must be registered");
    let conflicting = hgrc
        .find("ignore.aikit = something-else.txt")
        .expect("pre-existing line must be preserved");
    assert!(
        ours > conflicting,
        "managed entry must come AFTER the conflicting one so it wins:\n{hgrc}"
    );
}

#[test]
fn init_hg_registers_under_ui_when_managed_line_is_in_wrong_section() {
    let repo = init_hg_repo();
    let p = repo.path();
    // The managed key/value exists, but under a non-[ui] section, where Mercurial does
    // NOT honor it as ignore coverage. init must still add a real [ui] registration.
    fs::write(
        p.join(".hg/hgrc"),
        "[extensions]\nignore.aikit = .hg/hgignore.aikit\n",
    )
    .unwrap();

    let json = json_out(p, &["init", "--require-repo"]);
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["ignore_source"], ".hg/hgignore.aikit");
    assert_eq!(json["info_exclude_updated"], true);

    let hgrc = read_hgrc(p);
    assert!(hgrc.contains("[ui]"), "must add a [ui] section:\n{hgrc}");
    // The managed entry now appears under [ui] (after the [ui] header).
    let ui_idx = hgrc.find("[ui]").unwrap();
    assert!(
        hgrc[ui_idx..].contains("ignore.aikit = .hg/hgignore.aikit"),
        "managed entry must be registered under [ui]:\n{hgrc}"
    );
}

// ---- aikit init (adaptive) / init --require-folder ----

#[test]
fn init_auto_in_git_repo_matches_require_repo() {
    let repo = init_repo();
    let p = repo.path();
    let json = json_out(p, &["init"]);
    assert_eq!(json["vcs"], "git");
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["ignore_source"], ".git/info/exclude");
    assert!(read_info_exclude(p).contains("/.aikit/"));
    assert!(p.join(".aikit/temp").is_dir());
}

#[test]
fn init_auto_in_hg_repo_adds_hg_ignore() {
    let repo = init_hg_repo();
    let p = repo.path();
    let json = json_out(p, &["init"]);
    assert_eq!(json["vcs"], "mercurial");
    assert_eq!(json["aikit_ignored"], true);
    assert_eq!(json["ignore_source"], ".hg/hgignore.aikit");
    assert!(read_hgrc(p).contains("ignore.aikit = .hg/hgignore.aikit"));
}

#[test]
fn init_auto_in_non_repo_creates_dirs_without_ignore() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    let json = json_out(p, &["init"]);
    assert_eq!(json["vcs"], "none");
    assert_eq!(json["aikit_ignored"], false);
    assert!(json["ignore_source"].is_null());
    assert_eq!(json["info_exclude_updated"], false);
    assert!(p.join(".aikit").is_dir());
    assert!(p.join(".aikit/temp").is_dir());
    // No VCS state of any kind is created.
    assert!(!p.join(".git").exists());
    assert!(!p.join(".hg").exists());
    assert!(!p.join(".gitignore").exists());
}

#[test]
fn init_require_folder_in_non_repo_creates_dirs_only() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    let json = json_out(p, &["init", "--require-folder"]);
    assert_eq!(json["vcs"], "none");
    assert_eq!(json["aikit_ignored"], false);
    assert!(p.join(".aikit/temp").is_dir());
}

#[test]
fn init_require_folder_refuses_inside_git_repo() {
    let repo = init_repo();
    aikit(repo.path())
        .args(["init", "--require-folder"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_present"));
    // It must not have created .aikit/ in the repo.
    assert!(!repo.path().join(".aikit").exists());
}

#[test]
fn init_require_folder_refuses_inside_hg_repo() {
    let repo = init_hg_repo();
    aikit(repo.path())
        .args(["init", "--require-folder"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_present"));
}

#[test]
fn init_is_idempotent_in_non_repo() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    json_out(p, &["init"]);
    let second = json_out(p, &["init"]);
    assert_eq!(second["created_dirs"].as_array().unwrap().len(), 0);
    assert_eq!(second["vcs"], "none");
}

// ---- doctor (adaptive) ----

#[test]
fn top_level_doctor_reports_instead_of_failing_with_no_marker() {
    // The documented loop is "doctor first, init only if doctor says so". In a fresh
    // folder the read-only probe must therefore report what is missing, not error — an
    // error tells an agent nothing about what to do next.
    let dir = TempDir::new().unwrap();
    let out = json_out(dir.path(), &["doctor"]);
    assert_eq!(out["root_source"], "cwd_no_marker");
    assert_eq!(out["ready"], false);
    assert_eq!(out["vcs"], "none");
    assert_eq!(out["aikit_dir_exists"], false);
    assert_eq!(out["kind"], "aikit.repo_doctor");
    let warnings = out["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("aikit init")),
        "the report must carry the remediation: {warnings:?}"
    );
}

#[test]
fn top_level_doctor_marks_an_unanchored_root_as_such() {
    // cwd is where the report was taken, not a root. Without an explicit signal the two
    // are indistinguishable, since both are just a path.
    let dir = TempDir::new().unwrap();
    aikit(dir.path())
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicates::str::contains("no aikit root found"));
}

#[test]
fn top_level_doctor_matches_repo_doctor_when_a_marker_exists() {
    // One implementation, two entry points: where a marker exists the reports agree.
    let repo = init_repo();
    let p = repo.path();
    aikit(p).args(["init", "--require-repo"]).assert().success();

    let adaptive = json_out(p, &["doctor"]);
    let strict = json_out(p, &["doctor", "--require-root"]);
    assert_eq!(adaptive["root_source"], "marker");
    assert_eq!(adaptive["repo_root"], strict["repo_root"]);
    assert_eq!(adaptive["ready"], strict["ready"]);
    assert_eq!(adaptive["vcs"], strict["vcs"]);
    assert_eq!(adaptive["aikit_ignored"], strict["aikit_ignored"]);
}

#[test]
fn top_level_doctor_works_in_a_non_repo_aikit_folder() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    aikit(p).args(["init"]).assert().success();
    let out = json_out(p, &["doctor"]);
    assert_eq!(out["root_source"], "marker");
    assert_eq!(out["vcs"], "none");
    assert_eq!(
        out["ready"], true,
        "a non-repo .aikit/ folder needs no ignore coverage"
    );
}

#[test]
fn top_level_doctor_works_in_an_hg_repo() {
    let repo = init_hg_repo();
    let p = repo.path();
    aikit(p).args(["init"]).assert().success();
    let out = json_out(p, &["doctor"]);
    assert_eq!(out["root_source"], "marker");
    assert_eq!(out["vcs"], "mercurial");
}

#[test]
fn top_level_doctor_is_read_only_including_the_no_marker_path() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    aikit(p).args(["doctor"]).assert().success();
    assert!(
        !p.join(".aikit").exists(),
        "doctor must create nothing, least of all on the path that reports it is missing"
    );
    assert!(!p.join(".scratch").exists());
    assert!(!p.join(".claude").exists());
}

// ---- doctor --require-root ----

#[test]
fn doctor_require_root_blocks_with_no_marker() {
    // The default entry point degrades to a report so the "doctor first, init on a gap"
    // loop works in a fresh folder; `--require-root` is how a caller asks for the refusal.
    let dir = TempDir::new().unwrap();
    aikit(dir.path())
        .args(["doctor", "--require-root"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

#[test]
fn doctor_fails_outside_git_repo() {
    let dir = TempDir::new().unwrap();
    aikit(dir.path())
        .args(["doctor", "--require-root"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("blocked_repo_not_found"));
}

#[test]
fn doctor_is_read_only() {
    let repo = init_repo();
    let p = repo.path();
    aikit(p)
        .args(["doctor", "--require-root"])
        .assert()
        .success();
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

    let before = json_out(p, &["doctor", "--require-root"]);
    assert_eq!(before["kind"], "aikit.repo_doctor");
    assert_eq!(before["ready"], false);
    assert_eq!(before["temp_dir_exists"], false);
    assert_eq!(before["aikit_ignored"], false);
    // doctor never makes the repo ready by itself.
    assert!(!p.join(".aikit").exists());

    aikit(p).args(["init", "--require-repo"]).assert().success();

    let after = json_out(p, &["doctor", "--require-root"]);
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
    aikit(p).args(["init", "--require-repo"]).assert().success();
    let json = json_out(p, &["doctor", "--require-root"]);

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
    // Each entry carries availability + applicability booleans, and enough to act on:
    // `available` alone cannot distinguish powershell 5.1 from pwsh 7.
    for r in runners {
        assert!(r["available"].is_boolean());
        assert!(r["applicable"].is_boolean());

        let reason = r["reason"].as_str().expect("reason is a string");
        assert!(
            matches!(
                reason,
                "available"
                    | "available_via_alias"
                    | "not_found_on_path"
                    | "not_applicable_on_this_os"
            ),
            "unexpected reason token: {reason}"
        );

        // The reason must agree with the booleans rather than being decoration. Both
        // available_* tokens mean available; which one depends on whether the resolved
        // program is a real executable or a Windows App Execution Alias.
        let available = r["available"].as_bool().unwrap();
        let applicable = r["applicable"].as_bool().unwrap();
        if available {
            assert!(
                reason.starts_with("available"),
                "available runner reports {reason}: {r}"
            );
        } else {
            let expected = if applicable {
                "not_found_on_path"
            } else {
                "not_applicable_on_this_os"
            };
            assert_eq!(reason, expected, "reason disagrees with the flags: {r}");
        }

        // A resolved path exists exactly when the runner is available; version is
        // best-effort (a `sh` that is really dash reports none), so it is only ever
        // present for something that resolved.
        assert_eq!(
            r["path"].is_string(),
            available,
            "path should be present iff available: {r}"
        );
        if !available {
            assert!(
                r["version"].is_null(),
                "unavailable runner has a version: {r}"
            );
        }
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
    aikit(p).args(["init", "--require-repo"]).assert().success();
    let json = json_out(p, &["doctor", "--require-root"]);
    // On Unix, /bin/sh provides a baseline runner, so the repo is ready regardless of
    // whether zsh is present (zsh is optional unless a zsh script is selected).
    if std::path::Path::new("/bin/sh").exists() {
        assert_eq!(json["any_runner_available"], true);
        assert_eq!(json["ready"], true);
    }
}

#[test]
fn doctor_in_hg_repo_reports_vcs_and_ignore_after_init() {
    // When Mercurial is installed (e.g. CI runners), build a real hg repo so doctor can
    // actually introspect it; otherwise fall back to a bare `.hg/` marker, which still
    // exercises aikit's filesystem-only detection and the graceful degradation path taken
    // when `hg` is unavailable. This keeps the test correct in both environments.
    let hg_present = Command::new("hg")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let repo = TempDir::new().expect("tempdir");
    let p = repo.path();
    if hg_present {
        let status = Command::new("hg")
            .current_dir(p)
            .env("HGRCPATH", "") // hermetic: ignore user/global hgrc
            .args(["init"])
            .status()
            .expect("failed to run hg init");
        assert!(status.success(), "hg init failed in {}", p.display());
    } else {
        fs::create_dir_all(p.join(".hg")).expect("create .hg");
    }

    // Before init: detected as a Mercurial repo, not yet ignored/ready.
    let before = json_out(p, &["doctor", "--require-root"]);
    assert_eq!(before["vcs"], "mercurial");
    assert_eq!(before["aikit_ignored"], false);
    assert_eq!(before["ready"], false);

    aikit(p).args(["init", "--require-repo"]).assert().success();

    // After init: ignore detected (filesystem, no hg binary required), ready if a runner
    // exists.
    let after = json_out(p, &["doctor", "--require-root"]);
    assert_eq!(after["vcs"], "mercurial");
    assert_eq!(after["aikit_ignored"], true);
    assert_eq!(after["ignore_source"], ".hg/hgignore.aikit");
    if after["any_runner_available"] == true {
        assert_eq!(after["ready"], true);
    }

    // HEAD is empty either way: a fresh repo has no commits (null node), and an
    // unavailable `hg` degrades HEAD to empty.
    assert_eq!(after["git_head"], "");
    let warnings = after["warnings"].as_array().unwrap();

    if hg_present {
        // `hg` introspects the repo: the Mercurial named branch defaults to `default`,
        // and no degradation warning is emitted.
        assert_eq!(after["git_branch"], "default");
        assert!(
            !warnings
                .iter()
                .any(|w| w.as_str().unwrap().contains("`hg` is unavailable")),
            "hg is installed; expected no hg-unavailable warning: {warnings:?}"
        );
    } else {
        // `hg` is not installed, so branch/HEAD/tracked-tree degrade with a single warning
        // (not silently). Branch is empty; a warning records the cause.
        assert_eq!(after["git_branch"], "");
        assert!(
            warnings
                .iter()
                .any(|w| w.as_str().unwrap().contains("`hg` is unavailable")),
            "expected an hg-unavailable warning covering branch/HEAD + tracked-tree: {warnings:?}"
        );
    }
}

#[test]
fn doctor_in_non_repo_folder_reports_vcs_none() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    aikit(p)
        .args(["init", "--require-folder"])
        .assert()
        .success();

    let json = json_out(p, &["doctor", "--require-root"]);
    assert_eq!(json["vcs"], "none");
    assert_eq!(json["temp_dir_exists"], true);
    assert_eq!(json["aikit_ignored"], false);
    // A non-repo folder needs no ignore coverage, so it is ready when a runner exists.
    if json["any_runner_available"] == true {
        assert_eq!(json["ready"], true);
    }
    // Branch/HEAD are empty for a non-repo root.
    assert_eq!(json["git_branch"], "");
    assert_eq!(json["git_head"], "");
}

#[test]
fn doctor_reports_tracked_clean_then_dirty() {
    let repo = init_repo();
    let p = repo.path();

    let clean = json_out(p, &["doctor", "--require-root"]);
    assert_eq!(clean["tracked_tree_clean"], true);

    fs::write(p.join("README.md"), "# readme\nchanged\n").unwrap();
    let dirty = json_out(p, &["doctor", "--require-root"]);
    assert_eq!(dirty["tracked_tree_clean"], false);
    // A dirty tree does not, by itself, change anything about creation.
    assert!(!p.join(".aikit").exists());
}

#[test]
fn doctor_reports_locations_interpreters_and_metadata() {
    let repo = init_repo();
    let p = repo.path();
    let json = json_out(p, &["doctor", "--require-root"]);

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
        .args(["doctor", "--require-root"])
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
