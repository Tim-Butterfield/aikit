//! Repo-root detection and small Git helpers.
//!
//! Git is the source of truth for the repository root and working-tree state.
//! Batch 1 shells out to the `git` CLI rather than linking a Git library — that
//! keeps the dependency surface minimal and matches how the architect's workflows
//! already observe repo state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{RepoDoctorArgs, RepoInitArgs};
use crate::errors::{blocked, AikitError};
use crate::formats::{
    PathStatus, RepoDoctor, RepoInit, RunnerStatus, KIND_REPO_DOCTOR, KIND_REPO_INIT,
    SCHEMA_VERSION,
};
use crate::policy::script as policy;

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

/// Ensure `.aikit/` is locally ignored. Returns `(ignored, source, exclude_updated)`.
///
/// If `.aikit/` is already ignored by any Git ignore source, nothing is changed. Else
/// a local `/.aikit/` entry is appended to `.git/info/exclude` — local Git metadata
/// that is never staged. `.gitignore` is never modified by this command.
fn ensure_aikit_ignored(
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

/// `aikit repo init` — prepare the current repository for local aikit usage.
///
/// Creates `.aikit/` and `.aikit/temp/` if missing and ensures `.aikit/` is locally
/// ignored (via `.git/info/exclude`, never `.gitignore`). Idempotent; creates no
/// output artifacts, `.scratch/`, or `.claude/`, and never touches remote Git state.
pub fn init(args: RepoInitArgs) -> Result<(), AikitError> {
    let root = detect_root()?;
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

    let (aikit_ignored, ignore_source, info_exclude_updated) =
        ensure_aikit_ignored(&root, &mut actions)?;

    let record = RepoInit {
        schema_version: SCHEMA_VERSION,
        kind: KIND_REPO_INIT.to_string(),
        repo_root: root.display().to_string(),
        aikit_dir: ".aikit".to_string(),
        temp_dir: ".aikit/temp".to_string(),
        created_dirs,
        aikit_ignored,
        ignore_source,
        info_exclude_updated,
        actions,
        blocked_state: None,
    };

    if args.json {
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
        println!("aikit repo init:");
        println!("  repo root: {}", record.repo_root);
        println!("  .aikit/: {aikit_status}");
        println!("  .aikit/temp/: {temp_status}");
        match &record.ignore_source {
            Some(source) => println!("  .aikit/ ignored: yes (source: {source})"),
            None => println!("  .aikit/ ignored: no"),
        }
        println!(
            "  .git/info/exclude updated: {}",
            record.info_exclude_updated
        );
        for action in &record.actions {
            println!("  - {action}");
        }
        println!("Repository is prepared for local aikit usage.");
    }
    Ok(())
}

/// `aikit repo doctor` — report repo-local aikit readiness without mutating anything.
///
/// Read-only: creates and modifies nothing (no `.aikit/`, `.scratch/`, `.claude/`,
/// `.gitignore`, or `.git/info/exclude`). Exits 0 when a repo is found, even with
/// warnings; only `blocked_repo_not_found` (outside a repo) is an error.
pub fn doctor(args: RepoDoctorArgs) -> Result<(), AikitError> {
    let root = detect_root()?;
    let aikit = root.join(".aikit");
    let temp = aikit.join("temp");
    let outputs = aikit.join("outputs");

    let branch = git_branch(&root);
    let head = git_head(&root);
    let tracked_tree_clean = !is_tracked_tree_dirty(&root);

    let aikit_dir_exists = aikit.is_dir();
    let temp_dir_exists = temp.is_dir();
    let outputs_dir_exists = outputs.is_dir();

    let ignore_source = aikit_ignore_source(&root);
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

    let mut warnings: Vec<String> = Vec::new();
    if !temp_dir_exists {
        warnings.push(".aikit/temp/ is missing; run `aikit repo init`".to_string());
    }
    if !aikit_ignored {
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
    // pwsh/cmd, and a host without zsh is still ready).
    let ready = temp_dir_exists && aikit_ignored && any_runner_available;

    let record = RepoDoctor {
        schema_version: SCHEMA_VERSION,
        kind: KIND_REPO_DOCTOR.to_string(),
        repo_root: root.display().to_string(),
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
