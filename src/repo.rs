//! Repo-root detection and small Git helpers.
//!
//! Git is the source of truth for the repository root and working-tree state.
//! Batch 1 shells out to the `git` CLI rather than linking a Git library — that
//! keeps the dependency surface minimal and matches how the architect's workflows
//! already observe repo state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{FolderInitArgs, InitArgs, RepoDoctorArgs, RepoInitArgs};
use crate::errors::{blocked, AikitError};
use crate::formats::{
    PathStatus, RepoDoctor, RepoInit, RunnerStatus, KIND_REPO_DOCTOR, KIND_REPO_INIT,
    SCHEMA_VERSION,
};
use crate::policy::script as policy;

/// Version-control system managing a detected repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vcs {
    Git,
    Mercurial,
}

impl Vcs {
    /// Machine-readable tag recorded in JSON records.
    pub fn tag(self) -> &'static str {
        match self {
            Vcs::Git => "git",
            Vcs::Mercurial => "mercurial",
        }
    }

    /// Repo-relative path of the local-only ignore location this VCS uses for
    /// `.aikit/` coverage (git's native exclude file, or aikit's hgrc-registered
    /// local ignore file for Mercurial).
    fn exclude_path(self) -> &'static str {
        match self {
            Vcs::Git => ".git/info/exclude",
            Vcs::Mercurial => ".hg/hgignore.aikit",
        }
    }
}

/// VCS / aikit marker directories found in a single directory during a walk-up.
struct Markers {
    /// `.git` present as a file *or* directory (worktrees/submodules use a `.git` file).
    git: bool,
    /// `.hg` present as a directory.
    hg: bool,
    /// `.aikit` present as a directory.
    aikit: bool,
}

fn markers_in(dir: &Path) -> Markers {
    Markers {
        git: dir.join(".git").exists(),
        hg: dir.join(".hg").is_dir(),
        aikit: dir.join(".aikit").is_dir(),
    }
}

/// Walk up from the current directory, applying `f` to each ancestor's markers and
/// returning the first `Some` result. Purely filesystem-based — no subprocess, no
/// dependency on `git`/`hg` being installed or on PATH.
fn walk_up<F, T>(mut f: F) -> Option<T>
where
    F: FnMut(&Path, &Markers) -> Option<T>,
{
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let markers = markers_in(&dir);
        if let Some(found) = f(&dir, &markers) {
            return Some(found);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Locate the nearest enclosing Git or Mercurial repository root by walking up for a
/// `.git`/`.hg` marker (Git takes precedence when both are present at the same level).
/// Returns `None` when the current directory is not inside any repository. No `git`/`hg`
/// subprocess is run, so this is fast and works on hosts without either CLI installed.
pub fn find_vcs_root() -> Option<(PathBuf, Vcs)> {
    walk_up(|dir, m| {
        if m.git {
            Some((dir.to_path_buf(), Vcs::Git))
        } else if m.hg {
            Some((dir.to_path_buf(), Vcs::Mercurial))
        } else {
            None
        }
    })
}

/// Detect the aikit run root for `script run`: the nearest enclosing directory carrying
/// a `.git`, `.hg`, or `.aikit` marker. Returns the root and its VCS (`None` for a
/// non-repo `.aikit`-only folder), or `blocked_repo_not_found` when no marker is found.
///
/// This is the script gate's containment anchor; it deliberately uses pure filesystem
/// detection (no `git rev-parse`, no `hg` spawn) so a non-repo run incurs zero
/// subprocesses and the gate never depends on a VCS CLI being installed.
pub fn detect_marker_root() -> Result<(PathBuf, Option<Vcs>), AikitError> {
    walk_up(|dir, m| {
        if m.git {
            Some((dir.to_path_buf(), Some(Vcs::Git)))
        } else if m.hg {
            Some((dir.to_path_buf(), Some(Vcs::Mercurial)))
        } else if m.aikit {
            Some((dir.to_path_buf(), None))
        } else {
            None
        }
    })
    .ok_or_else(|| {
        AikitError::blocked(
            blocked::REPO_NOT_FOUND,
            "no aikit root found from the current directory: run `aikit init` (or \
`aikit repo init` inside a repository) to create .aikit/",
        )
    })
}

/// Detect the repository root via `git rev-parse --show-toplevel`, run from the
/// current working directory. Returns `blocked_repo_not_found` when there is none.
pub fn detect_root() -> Result<PathBuf, AikitError> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| AikitError::other(format!("failed to run git: {e}")))?;

    if !output.status.success() {
        return Err(AikitError::blocked(
            blocked::REPO_NOT_FOUND,
            "no Git repository root detected from the current directory",
        ));
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err(AikitError::blocked(
            blocked::REPO_NOT_FOUND,
            "no Git repository root detected from the current directory",
        ));
    }
    Ok(PathBuf::from(root))
}

/// Detect the repository root, returning `None` (rather than a blocked error) when the
/// current directory is not inside a Git repository. Used by `env snapshot`, which still
/// reports non-repo facts when run outside a repo.
pub fn detect_root_opt() -> Option<PathBuf> {
    detect_root().ok()
}

/// Current HEAD commit hash, or an empty string on an unborn branch (no commits).
pub fn git_head(root: &Path) -> String {
    run_git(root, &["rev-parse", "HEAD"]).unwrap_or_default()
}

/// Current branch name, or an empty string when unavailable.
pub fn git_branch(root: &Path) -> String {
    run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default()
}

/// VCS-aware branch/bookmark label for display (informational; never gates anything).
/// Git: `git rev-parse --abbrev-ref HEAD`. Mercurial: the named branch (`hg branch`),
/// with the active bookmark appended when set — Mercurial's named branch is usually
/// `default`, so the bookmark is what a Git user most often means by "branch". Empty
/// string when the relevant CLI is unavailable.
pub fn vcs_branch(root: &Path, vcs: Vcs) -> String {
    match vcs {
        Vcs::Git => git_branch(root),
        Vcs::Mercurial => {
            let branch = run_hg(root, &["branch"]).unwrap_or_default();
            let bookmark =
                run_hg(root, &["log", "-r", ".", "-T", "{activebookmark}"]).unwrap_or_default();
            match (branch.is_empty(), bookmark.is_empty()) {
                (_, true) => branch,
                (true, false) => bookmark,
                (false, false) => format!("{branch} ({bookmark})"),
            }
        }
    }
}

/// VCS-aware HEAD/commit id for display. Git: `git rev-parse HEAD`. Mercurial: the
/// working-directory parent node (`hg log -r . -T '{node}'`). The all-zero null node
/// (an empty/unborn repo) is normalized to an empty string to match Git's behavior.
pub fn vcs_head(root: &Path, vcs: Vcs) -> String {
    match vcs {
        Vcs::Git => git_head(root),
        Vcs::Mercurial => {
            let node = run_hg(root, &["log", "-r", ".", "-T", "{node}"]).unwrap_or_default();
            if node.is_empty() || node.chars().all(|c| c == '0') {
                String::new()
            } else {
                node
            }
        }
    }
}

/// `git status --porcelain=v1` output (working-tree state vs HEAD), trailing
/// newline trimmed but internal lines preserved verbatim. Used for the anchor
/// snapshot recorded by `batch start`.
pub fn git_status_porcelain(root: &Path) -> String {
    // `--no-optional-locks` keeps this probe strictly read-only: a plain `git status`
    // may refresh and rewrite `.git/index` (a stat-cache optimization), which would
    // violate the read-only guarantee of callers like `env snapshot` / `repo doctor`.
    run_git_raw(root, &["--no-optional-locks", "status", "--porcelain=v1"]).unwrap_or_default()
}

/// Repo-relative paths of all tracked files (`git ls-files -z`), sorted. Used for the
/// optional initial snapshot recorded by `batch start --snapshot`. Returns an empty
/// vector when git fails or there are no tracked files.
pub fn git_tracked_files(root: &Path) -> Vec<String> {
    let output = match Command::new("git")
        .current_dir(root)
        .args(["--no-optional-locks", "ls-files", "-z"])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&output);
    let mut files: Vec<String> = text
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    files.sort();
    files
}

/// Whether the **tracked** working tree has uncommitted changes (modified, staged,
/// or deleted tracked files). Untracked files do not count, and `git status` already
/// excludes ignored files, so local-only areas like `.aikit/outputs/`, `.scratch/`,
/// `.claude/`, and `target/` never make the tree "dirty" for `--require-clean`.
pub fn is_tracked_tree_dirty(root: &Path) -> bool {
    let porcelain = git_status_porcelain(root);
    porcelain
        .lines()
        .any(|line| line.len() >= 2 && &line[..2] != "??")
}

/// Fallible Git analog of [`is_tracked_tree_dirty`] used by the `script run` clean gate.
/// Errors (rather than reporting "clean") when `git status` cannot run or exits non-zero.
/// Detection is now filesystem-based, so a `.git` root is reachable without a working
/// `git` CLI; the infallible [`is_tracked_tree_dirty`] would swallow that failure as
/// "clean" and let `--require-clean` silently pass. This mirrors [`hg_tracked_tree_dirty`]
/// so both VCS clean checks fail loudly instead of degrading the guarantee.
pub fn git_tracked_tree_dirty(root: &Path) -> Result<bool, AikitError> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["--no-optional-locks", "status", "--porcelain=v1"])
        .output()
        .map_err(|e| {
            AikitError::other(format!(
                "--require-clean needs Git, but `git` could not be run: {e}"
            ))
        })?;
    if !output.status.success() {
        return Err(AikitError::other(format!(
            "`git status` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .any(|line| line.len() >= 2 && &line[..2] != "??"))
}

/// Mercurial analog of [`is_tracked_tree_dirty`]: whether the tracked working tree has
/// uncommitted changes. Runs `hg status -mard` (modified/added/removed/deleted),
/// which — like git's porcelain check — excludes untracked (`?`) and ignored (`I`)
/// files. Any output line means dirty. This is the one place `script run` shells out to
/// `hg`, and only when `--require-clean` is explicitly requested for an hg root.
///
/// Returns an error (not `false`) when `hg` cannot be run, so a missing `hg` cannot be
/// silently misread as a clean tree.
pub fn hg_tracked_tree_dirty(root: &Path) -> Result<bool, AikitError> {
    let output = hg_command(root)
        .args(["status", "-mard"])
        .output()
        .map_err(|e| {
            AikitError::other(format!(
                "--require-clean needs Mercurial, but `hg` could not be run: {e}"
            ))
        })?;
    if !output.status.success() {
        return Err(AikitError::other(format!(
            "`hg status` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Run a git command and return trimmed stdout, or `None` on failure.
fn run_git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Like `run_git` but only trims the trailing newline, preserving internal
/// whitespace (needed for porcelain status lines whose first columns are spaces).
fn run_git_raw(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.trim_end_matches('\n').to_string())
}

/// Build a `Command` for an aikit-internal `hg` invocation, pinned to `root` with
/// `HGPLAIN=1` so user aliases, extensions, and locale cannot alter the output we
/// parse. Every one of aikit's own `hg` calls goes through this helper, so HGPLAIN is
/// always applied without any need to "detect" that we are calling hg. We deliberately
/// do NOT set HGPLAIN globally on aikit's process: `script run` executes user scripts,
/// and those should see the user's normal environment, not aikit's parsing tweaks.
fn hg_command(root: &Path) -> Command {
    let mut cmd = Command::new("hg");
    cmd.current_dir(root).env("HGPLAIN", "1");
    cmd
}

/// Run an `hg` command (HGPLAIN-pinned) and return trimmed stdout, or `None` on
/// failure (including `hg` not being installed).
fn run_hg(root: &Path, args: &[&str]) -> Option<String> {
    let output = hg_command(root).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Absolute path to the repository's git directory (`.git`), via
/// `git rev-parse --absolute-git-dir`.
fn absolute_git_dir(root: &Path) -> Option<PathBuf> {
    run_git(root, &["rev-parse", "--absolute-git-dir"]).map(PathBuf::from)
}

/// Whether `rev` resolves to a commit object present in the local repository. Used by
/// `diff anchor` to verify the anchor's recorded base head still exists locally.
pub fn commit_exists(root: &Path, rev: &str) -> bool {
    if rev.is_empty() {
        return false;
    }
    Command::new("git")
        .current_dir(root)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Raw stdout of `git diff <args>` (e.g. `--name-status -z <base>`, `--stat <base>`,
/// or `<base>` for a patch), or `None` on failure. Internal whitespace/NUL bytes are
/// preserved (only lossy UTF-8 decoding is applied).
pub fn git_diff(root: &Path, args: &[&str]) -> Option<String> {
    let mut full = vec!["diff"];
    full.extend_from_slice(args);
    let output = Command::new("git")
        .current_dir(root)
        .args(&full)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// The ignore source covering the whole `.aikit/` directory (e.g. `.gitignore` or
/// `.git/info/exclude`), or `None` when `.aikit/` is not ignored by any Git ignore
/// source. Probes the `.aikit` directory itself (via `git check-ignore -v`), so a rule
/// that only covers a child (e.g. `/.aikit/temp/`) does NOT count as covering `.aikit/`.
pub fn aikit_ignore_source(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["check-ignore", "-v", "--", ".aikit"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None; // exit 1 = not ignored
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?;
    // Format: "<source>:<line>:<pattern>\t<path>"; the source is up to the first ':'.
    let source = line.split(':').next()?.trim();
    if source.is_empty() {
        None
    } else {
        Some(source.to_string())
    }
}

/// Print any serializable record as pretty JSON to stdout.
fn print_json<T: serde::Serialize>(record: &T) -> Result<(), AikitError> {
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| AikitError::other(format!("failed to serialize record: {e}")))?;
    println!("{json}");
    Ok(())
}

fn exists_word(present: bool) -> &'static str {
    if present {
        "exists"
    } else {
        "missing"
    }
}

fn short_hash(head: &str) -> &str {
    if head.len() >= 7 {
        &head[..7]
    } else {
        head
    }
}

/// Rooted, syntax-explicit Mercurial ignore pattern covering the whole `.aikit/` tree.
/// The `re:` prefix forces regexp regardless of any file-level `syntax:` directive, and
/// `^` roots it at the repository root (Mercurial patterns are otherwise unrooted).
const HG_AIKIT_PATTERN: &str = r"re:^\.aikit/";

/// Repo-relative path of the local-only Mercurial ignore file aikit manages. It lives
/// under `.hg/`, so it is never tracked or transferred on clone, and is wired in via
/// `.hg/hgrc`'s `[ui] ignore.aikit` key (paths there resolve relative to the repo root).
const HG_IGNORE_FILE: &str = ".hg/hgignore.aikit";

/// The `[ui]` config entry that registers `HG_IGNORE_FILE` with Mercurial.
const HG_IGNORE_ENTRY: &str = "ignore.aikit = .hg/hgignore.aikit";

/// Ensure `.aikit/` is locally ignored for the detected VCS. Returns
/// `(ignored, source, exclude_updated)`.
fn ensure_aikit_ignored(
    root: &Path,
    vcs: Vcs,
    actions: &mut Vec<String>,
) -> Result<(bool, Option<String>, bool), AikitError> {
    match vcs {
        Vcs::Git => ensure_aikit_ignored_git(root, actions),
        Vcs::Mercurial => ensure_aikit_ignored_hg(root, actions),
    }
}

/// Ensure `.aikit/` is locally ignored in a Git repo. Returns
/// `(ignored, source, exclude_updated)`.
///
/// If `.aikit/` is already ignored by any Git ignore source, nothing is changed. Else
/// a local `/.aikit/` entry is appended to `.git/info/exclude` — local Git metadata
/// that is never staged. `.gitignore` is never modified by this command.
fn ensure_aikit_ignored_git(
    root: &Path,
    actions: &mut Vec<String>,
) -> Result<(bool, Option<String>, bool), AikitError> {
    if let Some(source) = aikit_ignore_source(root) {
        actions.push(format!(".aikit/ already ignored (source: {source})"));
        return Ok((true, Some(source), false));
    }

    let git_dir = absolute_git_dir(root)
        .ok_or_else(|| AikitError::other("failed to resolve the git directory".to_string()))?;
    let info = git_dir.join("info");
    fs::create_dir_all(&info)
        .map_err(|e| AikitError::other(format!("failed to create {}: {e}", info.display())))?;
    let exclude = info.join("exclude");
    let existing = fs::read_to_string(&exclude).unwrap_or_default();

    if existing.lines().any(|l| l.trim() == "/.aikit/") {
        // Already present in exclude though git did not report it ignored; do not add a
        // duplicate entry.
        actions.push("/.aikit/ already present in .git/info/exclude".to_string());
        return Ok((true, Some(".git/info/exclude".to_string()), false));
    }

    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str("/.aikit/\n");
    fs::write(&exclude, content)
        .map_err(|e| AikitError::other(format!("failed to write {}: {e}", exclude.display())))?;
    actions.push("added /.aikit/ to .git/info/exclude".to_string());
    Ok((true, Some(".git/info/exclude".to_string()), true))
}

/// Ensure `.aikit/` is locally ignored in a Mercurial repo. Returns
/// `(ignored, source, exclude_updated)`.
///
/// Mercurial has no `.git/info/exclude` equivalent file that it reads automatically;
/// instead an untracked, never-cloned ignore file is registered from `.hg/hgrc` via
/// the `[ui] ignore.<name>` key (see `hg help config`). This writes the rooted
/// `.aikit/` pattern to `.hg/hgignore.aikit` and registers it under `[ui]` in
/// `.hg/hgrc`; both files live inside `.hg/`, so neither is tracked or pushed. The
/// repo-root `.hgignore` (tracked) is never modified. No `hg` binary is required.
///
/// If a tracked `.hgignore` already covers `.aikit/`, nothing is changed.
fn ensure_aikit_ignored_hg(
    root: &Path,
    actions: &mut Vec<String>,
) -> Result<(bool, Option<String>, bool), AikitError> {
    if hgignore_covers_aikit(root) {
        actions.push(".aikit/ already ignored (source: .hgignore)".to_string());
        return Ok((true, Some(".hgignore".to_string()), false));
    }

    let hg_dir = root.join(".hg");
    let ignore_file = hg_dir.join("hgignore.aikit");
    let hgrc = hg_dir.join("hgrc");
    let mut updated = false;

    // 1) Ensure the local ignore file carries the rooted `.aikit/` pattern.
    let existing = fs::read_to_string(&ignore_file).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == HG_AIKIT_PATTERN) {
        actions.push(format!(
            "{HG_AIKIT_PATTERN} already present in {HG_IGNORE_FILE}"
        ));
    } else {
        let mut content = existing;
        if content.is_empty() {
            content.push_str(
                "# Local-only ignore patterns for aikit working files.\n\
                 # Managed by `aikit repo init`; registered via .hg/hgrc and never committed.\n",
            );
        } else if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(HG_AIKIT_PATTERN);
        content.push('\n');
        fs::write(&ignore_file, content).map_err(|e| {
            AikitError::other(format!("failed to write {}: {e}", ignore_file.display()))
        })?;
        actions.push(format!("added {HG_AIKIT_PATTERN} to {HG_IGNORE_FILE}"));
        updated = true;
    }

    // 2) Ensure `.hg/hgrc` registers the ignore file under `[ui]`.
    let hgrc_text = fs::read_to_string(&hgrc).unwrap_or_default();
    if hgrc_has_ignore_aikit(&hgrc_text) {
        actions.push("[ui] ignore.aikit already configured in .hg/hgrc".to_string());
    } else {
        let new_text = insert_ui_ignore_aikit(&hgrc_text);
        fs::write(&hgrc, new_text)
            .map_err(|e| AikitError::other(format!("failed to write {}: {e}", hgrc.display())))?;
        actions.push(format!(
            "registered {HG_IGNORE_FILE} via [ui] ignore.aikit in .hg/hgrc"
        ));
        updated = true;
    }

    Ok((true, Some(HG_IGNORE_FILE.to_string()), updated))
}

/// The Mercurial ignore source covering `.aikit/`, or `None` when it is not ignored.
/// Filesystem-only (no `hg` binary): it recognizes aikit's own local config
/// (`.hg/hgignore.aikit` registered in `.hg/hgrc`) and a tracked `.hgignore`, mirroring
/// how `repo init` decides coverage. It does not see ignore rules contributed by
/// user/global hgrc files (which `hg debugignore` would) — an accepted edge case in
/// exchange for working without `hg` installed.
fn hg_aikit_ignore_source(root: &Path) -> Option<String> {
    let hg_dir = root.join(".hg");
    let ignore_file = fs::read_to_string(hg_dir.join("hgignore.aikit")).unwrap_or_default();
    let hgrc = fs::read_to_string(hg_dir.join("hgrc")).unwrap_or_default();
    if hgrc_has_ignore_aikit(&hgrc) && ignore_file.lines().any(|l| l.trim() == HG_AIKIT_PATTERN) {
        return Some(HG_IGNORE_FILE.to_string());
    }
    if hgignore_covers_aikit(root) {
        return Some(".hgignore".to_string());
    }
    None
}

/// Best-effort detection of whether a tracked repo-root `.hgignore` already ignores the
/// whole `.aikit/` directory, so `repo init` does not add redundant local config.
/// Conservative: it recognizes the common rooted forms across glob/regexp syntaxes and
/// returns `false` when unsure (in which case local coverage is added — harmless).
fn hgignore_covers_aikit(root: &Path) -> bool {
    match fs::read_to_string(root.join(".hgignore")) {
        Ok(text) => text.lines().any(hg_line_covers_aikit),
        Err(_) => false,
    }
}

/// Whether a single `.hgignore` line appears to cover the whole `.aikit/` directory.
fn hg_line_covers_aikit(line: &str) -> bool {
    let mut t = line.trim();
    if t.is_empty() || t.starts_with('#') || t.starts_with("syntax:") {
        return false;
    }
    // Strip an inline per-pattern syntax prefix (e.g. `glob:`, `re:`, `rootglob:`).
    if let Some((prefix, rest)) = t.split_once(':') {
        if matches!(
            prefix,
            "glob"
                | "re"
                | "regexp"
                | "rootglob"
                | "rootfilesin"
                | "path"
                | "relpath"
                | "relre"
                | "relglob"
        ) {
            t = rest.trim();
        }
    }
    // Normalize anchors, regexp escapes, and leading/trailing separators shared across
    // the common rooted forms (`.aikit`, `.aikit/`, `/.aikit/`, `^\.aikit/?$`).
    let norm = t
        .trim_start_matches('^')
        .trim_start_matches('/')
        .trim_end_matches('$')
        .replace("\\.", ".");
    let norm = norm.trim_end_matches('/');
    // Covered when the pattern targets `.aikit` itself or anything under it — including
    // directory-content forms like `.aikit/**` (glob) or `.aikit/.*` (regexp), which cover
    // the same tree. `norm.starts_with(".aikit/")` catches those without matching siblings
    // such as `.aikitfoo`.
    norm == ".aikit" || norm.starts_with(".aikit/")
}

/// Whether the hgrc text already contains aikit's **managed** ignore entry —
/// `ignore.aikit = .hg/hgignore.aikit` (value-matched, whitespace-tolerant) **under the
/// `[ui]` section**, which is the only place Mercurial honors `ignore.<name>` keys.
/// Section-scoping matters: a matching key under any other section (or our exact value
/// pointed at a different file) must NOT be reported as coverage, so `repo init` still
/// inserts the real `[ui]` registration and `repo doctor` does not over-report.
///
/// Mercurial honors the **last** value of a repeated key within (merged) `[ui]` sections,
/// so this evaluates the *effective* (last-wins) `ignore.aikit` value across all `[ui]`
/// blocks and returns whether it equals our managed file. A later conflicting
/// `ignore.aikit = <other>` therefore correctly reads as "not covered by us" even if our
/// line appears earlier — matching what Mercurial would actually apply.
fn hgrc_has_ignore_aikit(text: &str) -> bool {
    let mut in_ui = false;
    let mut effective_is_managed: Option<bool> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(section) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_ui = section.trim() == "ui";
            continue;
        }
        if !in_ui {
            continue;
        }
        if let Some(value) = t
            .strip_prefix("ignore.aikit")
            .and_then(|rest| rest.trim_start().strip_prefix('='))
        {
            effective_is_managed = Some(value.trim() == HG_IGNORE_FILE);
        }
    }
    effective_is_managed.unwrap_or(false)
}

/// Insert the `ignore.aikit` entry into hgrc text, creating the `[ui]` section if absent.
/// The entry is appended at the **end of the LAST `[ui]` section** so that, regardless of
/// how many `[ui]` sections exist or where a conflicting `ignore.aikit = <other>` lives,
/// our entry is the final assignment Mercurial applies (it merges same-named sections and
/// takes the last value). Existing content, comments, and other sections are preserved.
fn insert_ui_ignore_aikit(text: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if let Some(idx) = lines.iter().rposition(|l| l.trim() == "[ui]") {
        // Find the end of that (last) [ui] section: the next section header, or EOF.
        let insert_at = lines
            .iter()
            .enumerate()
            .skip(idx + 1)
            .find(|(_, l)| {
                let t = l.trim();
                t.starts_with('[') && t.ends_with(']')
            })
            .map(|(i, _)| i)
            .unwrap_or(lines.len());
        lines.insert(insert_at, HG_IGNORE_ENTRY.to_string());
        let mut out = lines.join("\n");
        out.push('\n');
        out
    } else {
        let mut out = String::from(text);
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("[ui]\n");
        out.push_str(HG_IGNORE_ENTRY);
        out.push('\n');
        out
    }
}

/// Which init variant is being run, controlling repo-detection policy.
#[derive(Debug, Clone, Copy)]
enum InitKind {
    /// `aikit init` — adaptive: repo mode if inside a Git/Mercurial repo, else folder.
    Auto,
    /// `aikit repo init` — force repo mode; error if not inside a repo.
    Repo,
    /// `aikit folder init` — force non-repo mode; error if inside a repo.
    Folder,
}

impl InitKind {
    fn command_label(self) -> &'static str {
        match self {
            InitKind::Auto => "aikit init",
            InitKind::Repo => "aikit repo init",
            InitKind::Folder => "aikit folder init",
        }
    }
}

/// `aikit init` — adaptive setup: repo mode (dirs + ignore) inside a Git/Mercurial
/// repository, else a plain non-repo `.aikit/` folder (dirs only, no ignore).
pub fn init_auto(args: InitArgs) -> Result<(), AikitError> {
    do_init(InitKind::Auto, args.json)
}

/// `aikit repo init` — prepare the current repository for local aikit usage. Errors
/// (`blocked_repo_not_found`) when not inside a Git or Mercurial repository.
///
/// Creates `.aikit/` and `.aikit/temp/` if missing and ensures `.aikit/` is locally
/// ignored using the VCS's local, never-committed mechanism — Git's `.git/info/exclude`,
/// or a `.hg/hgrc`-registered `.hg/hgignore.aikit` for Mercurial. Tracked ignore files
/// (`.gitignore` / `.hgignore`) are never modified. Idempotent.
pub fn init(args: RepoInitArgs) -> Result<(), AikitError> {
    do_init(InitKind::Repo, args.json)
}

/// `aikit folder init` — prepare a non-repo folder for local aikit usage: create
/// `.aikit/` and `.aikit/temp/` in the current directory without adding any VCS ignore.
/// Errors (`blocked_repo_present`) when run inside a Git or Mercurial repository, where
/// `repo init` (or `init`) should be used so `.aikit/` is properly ignored.
pub fn init_folder(args: FolderInitArgs) -> Result<(), AikitError> {
    do_init(InitKind::Folder, args.json)
}

/// Shared implementation behind `init` / `repo init` / `folder init`. Repo detection is
/// pure filesystem walk-up (no `git`/`hg` subprocess); the VCS, when present, only
/// drives the ignore step.
fn do_init(kind: InitKind, json: bool) -> Result<(), AikitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| AikitError::other(format!("failed to resolve current directory: {e}")))?;
    let vcs_root = find_vcs_root();

    // Resolve the target root and effective VCS per the init variant's policy.
    let (root, vcs): (PathBuf, Option<Vcs>) = match kind {
        InitKind::Repo => match vcs_root {
            Some((r, v)) => (r, Some(v)),
            None => {
                return Err(AikitError::blocked(
                    blocked::REPO_NOT_FOUND,
                    "`aikit repo init` requires a Git or Mercurial repository; run \
`aikit folder init` (or `aikit init`) to initialize a non-repo folder",
                ))
            }
        },
        InitKind::Folder => match vcs_root {
            Some((_, v)) => {
                return Err(AikitError::blocked(
                    blocked::REPO_PRESENT,
                    format!(
                        "this directory is inside a {} repository; run `aikit repo init` \
(or `aikit init`) so .aikit/ is ignored",
                        v.tag()
                    ),
                ))
            }
            None => (cwd, None),
        },
        InitKind::Auto => match vcs_root {
            Some((r, v)) => (r, Some(v)),
            None => (cwd, None),
        },
    };

    let aikit = root.join(".aikit");
    let temp = aikit.join("temp");
    let mut created_dirs: Vec<String> = Vec::new();
    let mut actions: Vec<String> = Vec::new();

    if aikit.is_dir() {
        actions.push(".aikit/ already present".to_string());
    } else {
        fs::create_dir_all(&aikit)
            .map_err(|e| AikitError::other(format!("failed to create .aikit/: {e}")))?;
        created_dirs.push(".aikit".to_string());
        actions.push("created .aikit/".to_string());
    }

    if temp.is_dir() {
        actions.push(".aikit/temp/ already present".to_string());
    } else {
        fs::create_dir_all(&temp)
            .map_err(|e| AikitError::other(format!("failed to create .aikit/temp/: {e}")))?;
        created_dirs.push(".aikit/temp".to_string());
        actions.push("created .aikit/temp/".to_string());
    }

    // Ignore coverage only applies in repo mode; a non-repo folder gets none.
    let (aikit_ignored, ignore_source, info_exclude_updated) = match vcs {
        Some(v) => ensure_aikit_ignored(&root, v, &mut actions)?,
        None => {
            actions.push("non-repo folder: no VCS ignore coverage added".to_string());
            (false, None, false)
        }
    };

    let record = RepoInit {
        schema_version: SCHEMA_VERSION,
        kind: KIND_REPO_INIT.to_string(),
        repo_root: root.display().to_string(),
        vcs: vcs.map(|v| v.tag()).unwrap_or("none").to_string(),
        aikit_dir: ".aikit".to_string(),
        temp_dir: ".aikit/temp".to_string(),
        created_dirs,
        aikit_ignored,
        ignore_source,
        info_exclude_updated,
        actions,
        blocked_state: None,
    };

    if json {
        print_json(&record)?;
    } else {
        let aikit_status = if record.created_dirs.iter().any(|d| d == ".aikit") {
            "created"
        } else {
            "already present"
        };
        let temp_status = if record.created_dirs.iter().any(|d| d == ".aikit/temp") {
            "created"
        } else {
            "already present"
        };
        println!("{}:", kind.command_label());
        println!("  root: {}", record.repo_root);
        println!("  vcs: {}", record.vcs);
        println!("  .aikit/: {aikit_status}");
        println!("  .aikit/temp/: {temp_status}");
        match &record.ignore_source {
            Some(source) => println!("  .aikit/ ignored: yes (source: {source})"),
            None if vcs.is_none() => println!("  .aikit/ ignored: n/a (non-repo folder)"),
            None => println!("  .aikit/ ignored: no"),
        }
        if let Some(v) = vcs {
            println!(
                "  {} updated: {}",
                v.exclude_path(),
                record.info_exclude_updated
            );
        }
        for action in &record.actions {
            println!("  - {action}");
        }
        println!("Prepared for local aikit usage.");
    }
    Ok(())
}

/// `aikit repo doctor` — report repo-local aikit readiness without mutating anything.
///
/// Read-only: creates and modifies nothing (no `.aikit/`, `.scratch/`, `.claude/`,
/// `.gitignore`, or `.git/info/exclude`). Exits 0 when a repo is found, even with
/// warnings; only `blocked_repo_not_found` (outside a repo) is an error.
pub fn doctor(args: RepoDoctorArgs) -> Result<(), AikitError> {
    // Filesystem detection: a Git/Mercurial repo or a non-repo `.aikit/` folder.
    let (root, vcs) = detect_marker_root()?;
    let aikit = root.join(".aikit");
    let temp = aikit.join("temp");
    let outputs = aikit.join("outputs");

    let mut warnings: Vec<String> = Vec::new();

    // Informational VCS facts (branch/head). Empty for a non-repo folder, or when the
    // relevant CLI is unavailable — doctor stays read-only and never fails on these.
    let branch = vcs.map(|v| vcs_branch(&root, v)).unwrap_or_default();
    let head = vcs.map(|v| vcs_head(&root, v)).unwrap_or_default();

    // Tracked-tree cleanliness. Non-repo folders are vacuously clean. For a repo whose VCS
    // CLI is unavailable (reachable now that detection is filesystem-based), degrade to
    // "clean" + a warning rather than erroring or silently misreporting. The fallible
    // checks are used for BOTH Git and Mercurial so neither silently reports clean on
    // failure; the single warning also covers the branch/HEAD degradation (which empties
    // under the same missing-CLI condition), satisfying the §5.6 "with a warning" rule.
    let tracked_tree_clean = match vcs {
        Some(Vcs::Git) => match git_tracked_tree_dirty(&root) {
            Ok(dirty) => !dirty,
            Err(_) => {
                warnings.push(
                    "Git repo but `git` is unavailable; branch/HEAD and tracked-tree state \
not determined"
                        .to_string(),
                );
                true
            }
        },
        Some(Vcs::Mercurial) => match hg_tracked_tree_dirty(&root) {
            Ok(dirty) => !dirty,
            Err(_) => {
                warnings.push(
                    "Mercurial repo but `hg` is unavailable; branch/HEAD and tracked-tree \
state not determined"
                        .to_string(),
                );
                true
            }
        },
        None => true,
    };

    let aikit_dir_exists = aikit.is_dir();
    let temp_dir_exists = temp.is_dir();
    let outputs_dir_exists = outputs.is_dir();

    // Ignore coverage is VCS-specific; a non-repo folder has nothing to ignore against.
    let ignore_source = match vcs {
        Some(Vcs::Git) => aikit_ignore_source(&root),
        Some(Vcs::Mercurial) => hg_aikit_ignore_source(&root),
        None => None,
    };
    let aikit_ignored = ignore_source.is_some();

    let allowed_script_locations: Vec<PathStatus> = policy::ALLOWED_SCRIPT_DIRS
        .iter()
        .map(|d| PathStatus {
            path: d.to_string(),
            exists: root.join(d.trim_end_matches('/')).is_dir(),
        })
        .collect();

    // Shell interpreter probe is informational only (kept for compatibility); readiness
    // now uses cross-OS runner availability instead of requiring Unix shells.
    let interpreters: Vec<PathStatus> = ["/bin/sh", "/bin/zsh"]
        .iter()
        .map(|p| PathStatus {
            path: p.to_string(),
            exists: Path::new(p).exists(),
        })
        .collect();

    // Runner availability aligned with policy::script (the script runner's own model).
    let runners: Vec<RunnerStatus> = policy::runner_availability()
        .into_iter()
        .map(|r| RunnerStatus {
            name: r.name.to_string(),
            available: r.available,
            applicable: r.applicable,
        })
        .collect();
    let any_runner_available = runners.iter().any(|r| r.available);

    let current_exe = std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string());
    let version = env!("CARGO_PKG_VERSION").to_string();

    if !temp_dir_exists {
        warnings.push(".aikit/temp/ is missing; run `aikit init`".to_string());
    }
    // Ignore coverage only applies inside a repository; a non-repo `.aikit/` folder has
    // nothing to ignore against, so its absence is not a warning there.
    if vcs.is_some() && !aikit_ignored {
        warnings.push(
            ".aikit/ is not ignored; run `aikit repo init` to add local ignore coverage"
                .to_string(),
        );
    }
    if !any_runner_available {
        warnings.push(
            "no supported script runner is available on this system (looked for: \
sh, bash, zsh, pwsh, powershell, cmd, python3, python, node)"
                .to_string(),
        );
    }

    // Readiness: local aikit state is sane AND at least one supported runner exists for
    // this OS. It does NOT require any specific Unix shell (so Windows is ready with
    // pwsh/cmd, and a host without zsh is still ready). Ignore coverage is required only
    // in a repository; a non-repo `.aikit/` folder is ready without it.
    let ignore_ok = vcs.is_none() || aikit_ignored;
    let ready = temp_dir_exists && ignore_ok && any_runner_available;

    let record = RepoDoctor {
        schema_version: SCHEMA_VERSION,
        kind: KIND_REPO_DOCTOR.to_string(),
        repo_root: root.display().to_string(),
        vcs: vcs.map(|v| v.tag()).unwrap_or("none").to_string(),
        git_branch: branch,
        git_head: head,
        tracked_tree_clean,
        aikit_dir_exists,
        temp_dir_exists,
        outputs_dir_exists,
        aikit_ignored,
        ignore_source,
        default_output_root: ".aikit/outputs".to_string(),
        allowed_script_locations,
        interpreters,
        runners,
        any_runner_available,
        current_exe,
        version,
        warnings,
        ready,
        blocked_state: None,
    };

    if args.json {
        print_json(&record)?;
    } else {
        println!("aikit repo doctor (read-only):");
        println!("  repo root: {}", record.repo_root);
        println!("  vcs: {}", record.vcs);
        println!(
            "  branch: {}  HEAD: {}",
            record.git_branch,
            short_hash(&record.git_head)
        );
        println!(
            "  tracked tree: {}",
            if record.tracked_tree_clean {
                "clean"
            } else {
                "dirty"
            }
        );
        println!("  .aikit/: {}", exists_word(record.aikit_dir_exists));
        println!("  .aikit/temp/: {}", exists_word(record.temp_dir_exists));
        println!(
            "  .aikit/outputs/: {}",
            exists_word(record.outputs_dir_exists)
        );
        match &record.ignore_source {
            Some(source) => println!("  .aikit/ ignored: yes (source: {source})"),
            None if vcs.is_none() => println!("  .aikit/ ignored: n/a (non-repo folder)"),
            None => println!("  .aikit/ ignored: no"),
        }
        println!("  default output root: {}", record.default_output_root);
        println!("  allowed script input locations (not aikit state):");
        for l in &record.allowed_script_locations {
            println!("    {} ({})", l.path, exists_word(l.exists));
        }
        println!("  shell interpreters (informational):");
        for i in &record.interpreters {
            println!("    {} ({})", i.path, exists_word(i.exists));
        }
        println!("  script runners:");
        for r in &record.runners {
            let state = if r.available {
                "available"
            } else if r.applicable {
                "not found"
            } else {
                "not applicable (other OS)"
            };
            println!("    {} ({state})", r.name);
        }
        println!("  any runner available: {}", record.any_runner_available);
        if let Some(exe) = &record.current_exe {
            println!("  current exe: {exe}");
        }
        println!("  version: {}", record.version);
        if record.warnings.is_empty() {
            println!("  warnings: none");
        } else {
            println!("  warnings:");
            for w in &record.warnings {
                println!("    - {w}");
            }
        }
        println!("  ready: {}", record.ready);
    }
    Ok(())
}
