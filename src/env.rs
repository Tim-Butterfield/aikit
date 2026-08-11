//! `aikit env snapshot` — a mechanical local environment report.
//!
//! This captures a bounded, fixed set of debugging facts (aikit version, executable,
//! OS/arch, working directory, repo facts, interpreter and tool availability). It is
//! strictly read-only and runs no network commands. It deliberately does NOT dump the
//! full environment, the raw PATH, or any tokens/credentials/keys: PATH is summarized to
//! a count plus a single boolean, and only `$SHELL` (a shell path, not a secret) is read
//! by name from the environment.

use std::path::Path;
use std::process::Command;

use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::cli::EnvSnapshotArgs;
use crate::errors::AikitError;
use crate::formats::{
    EnvPaths, EnvRepo, EnvSnapshot, EnvTools, PathStatus, KIND_ENV_SNAPSHOT, SCHEMA_VERSION,
};
use crate::repo;

const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");

const INTERPRETERS: &[&str] = &["/bin/sh", "/bin/zsh"];

/// `aikit env snapshot` — report mechanical local environment facts. Works inside or
/// outside a Git repository; read-only and creates nothing.
pub fn snapshot(args: EnvSnapshotArgs) -> Result<(), AikitError> {
    let mut warnings: Vec<String> = Vec::new();

    let version = env!("CARGO_PKG_VERSION").to_string();
    let current_exe = std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string());
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let current_dir = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    // Repo facts are optional: outside a Git repo we still report the rest.
    let repo = repo::detect_root_opt().map(|root| {
        let aikit = root.join(".aikit");
        EnvRepo {
            root: root.display().to_string(),
            branch: repo::git_branch(&root),
            head: repo::git_head(&root),
            tracked_tree_clean: !repo::is_tracked_tree_dirty(&root),
            default_output_root: ".aikit/outputs".to_string(),
            aikit_dir_exists: aikit.is_dir(),
            temp_dir_exists: aikit.join("temp").is_dir(),
            outputs_dir_exists: aikit.join("outputs").is_dir(),
            aikit_ignored: repo::aikit_ignore_source(&root).is_some(),
        }
    });
    if repo.is_none() {
        warnings.push("not inside a Git repository; repo facts omitted".to_string());
    }

    let interpreters: Vec<PathStatus> = INTERPRETERS
        .iter()
        .map(|p| PathStatus {
            path: p.to_string(),
            exists: Path::new(p).exists(),
        })
        .collect();
    for i in &interpreters {
        if !i.exists {
            warnings.push(format!("interpreter {} is missing", i.path));
        }
    }

    let paths = path_summary(current_exe.as_deref());

    let tools = EnvTools {
        git_version: tool_version("git", &["--version"]),
        rustc_version: tool_version("rustc", &["--version"]),
        cargo_version: tool_version("cargo", &["--version"]),
    };

    // `$SHELL` is a single, non-secret value (a shell path); we never read the rest of
    // the environment.
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty());

    let record = EnvSnapshot {
        schema_version: SCHEMA_VERSION,
        kind: KIND_ENV_SNAPSHOT.to_string(),
        generated_at: OffsetDateTime::now_utc()
            .format(TS_FORMAT)
            .unwrap_or_default(),
        version,
        current_exe,
        os,
        arch,
        current_dir,
        repo,
        paths,
        interpreters,
        tools,
        shell,
        warnings,
        blocked_state: None,
    };

    if args.json {
        let json = serde_json::to_string_pretty(&record)
            .map_err(|e| AikitError::other(format!("failed to serialize snapshot: {e}")))?;
        println!("{json}");
    } else {
        render_human(&record);
    }
    Ok(())
}

/// A safe PATH summary: the number of entries and whether the current executable's
/// directory is on PATH. The raw PATH value is never emitted.
fn path_summary(current_exe: Option<&str>) -> EnvPaths {
    let raw = std::env::var_os("PATH");
    let entries: Vec<std::path::PathBuf> = raw
        .as_ref()
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    let exe_dir = current_exe
        .map(Path::new)
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let current_exe_dir_on_path = match (&exe_dir, raw.is_some()) {
        (Some(dir), true) => Some(entries.iter().any(|e| e == dir)),
        _ => None,
    };
    EnvPaths {
        current_exe_dir_on_path,
        path_entry_count: entries.len(),
    }
}

/// Capture a local tool's `--version` line (first line, trimmed), or `None` when the
/// tool is unavailable or errors. No network access is involved.
fn tool_version(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

fn exists_word(present: bool) -> &'static str {
    if present {
        "exists"
    } else {
        "missing"
    }
}

fn render_human(r: &EnvSnapshot) {
    println!("aikit env snapshot (read-only):");
    println!("  aikit version: {}", r.version);
    if let Some(exe) = &r.current_exe {
        println!("  current exe: {exe}");
    }
    println!("  os: {}  arch: {}", r.os, r.arch);
    println!("  current dir: {}", r.current_dir);
    match &r.repo {
        Some(repo) => {
            println!("  repo root: {}", repo.root);
            println!("    branch: {}  HEAD: {}", repo.branch, repo.head);
            println!(
                "    tracked tree: {}",
                if repo.tracked_tree_clean {
                    "clean"
                } else {
                    "dirty"
                }
            );
            println!("    default output root: {}", repo.default_output_root);
            println!("    .aikit/: {}", exists_word(repo.aikit_dir_exists));
            println!("    .aikit/temp/: {}", exists_word(repo.temp_dir_exists));
            println!(
                "    .aikit/outputs/: {}",
                exists_word(repo.outputs_dir_exists)
            );
            println!(
                "    .aikit/ ignored: {}",
                if repo.aikit_ignored { "yes" } else { "no" }
            );
        }
        None => println!("  repo: (not inside a Git repository)"),
    }
    println!("  interpreters:");
    for i in &r.interpreters {
        println!("    {} ({})", i.path, exists_word(i.exists));
    }
    println!("  tools:");
    println!(
        "    git: {}",
        r.tools.git_version.as_deref().unwrap_or("n/a")
    );
    println!(
        "    rustc: {}",
        r.tools.rustc_version.as_deref().unwrap_or("n/a")
    );
    println!(
        "    cargo: {}",
        r.tools.cargo_version.as_deref().unwrap_or("n/a")
    );
    match r.paths.current_exe_dir_on_path {
        Some(on) => println!(
            "  PATH: {} entries; current exe dir on PATH: {}",
            r.paths.path_entry_count, on
        ),
        None => println!("  PATH: {} entries", r.paths.path_entry_count),
    }
    if let Some(shell) = &r.shell {
        println!("  shell: {shell}");
    }
    if r.warnings.is_empty() {
        println!("  warnings: none");
    } else {
        println!("  warnings:");
        for w in &r.warnings {
            println!("    - {w}");
        }
    }
}
