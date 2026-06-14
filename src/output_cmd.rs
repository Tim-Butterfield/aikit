//! `aikit output list` / `output show` / `output clean` — manage local aikit output
//! artifacts under an output root (default `.aikit/outputs/`).
//!
//! Only *known* artifacts are recognized: `batches/*.json` files and `inventory/`,
//! `reviews/`, and `runs/` subdirectories. `list` and `show` are read-only. `clean` is
//! dry-run by default and deletes only with `--execute` plus a selector (`--older-than`
//! or `--all`); it never deletes outside the selected output root, never follows symlink
//! escapes, and never touches `.aikit/temp/`, `.scratch/`, `.claude/`, `target/`, or
//! `.git/` (none of which are output families under the output root).

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::Value;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::cli::{OutputCleanArgs, OutputFamily, OutputListArgs, OutputShowArgs};
use crate::errors::{blocked, AikitError};
use crate::formats::{
    OutputArtifact, OutputClean, OutputCleanCounts, OutputCleanFilters, OutputCounts, OutputFile,
    OutputList, OutputShow, KIND_OUTPUT_CLEAN, KIND_OUTPUT_LIST, KIND_OUTPUT_SHOW, SCHEMA_VERSION,
};
use crate::repo;

const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");

/// Output families in their canonical sort order. `is_dir` distinguishes the
/// directory families (each subdir is an artifact) from `batches` (each `*.json` file).
const FAMILIES: &[(&str, bool)] = &[
    ("batches", false),
    ("inventory", true),
    ("reviews", true),
    ("runs", true),
];

/// A discovered artifact, carrying the real path for internal use.
struct Artifact {
    family: &'static str,
    artifact_id: String,
    rel_path: String,
    artifact_type: &'static str,
    real_path: PathBuf,
    size_bytes: u64,
    modified: SystemTime,
}

impl Artifact {
    fn to_format(&self) -> OutputArtifact {
        OutputArtifact {
            family: self.family.to_string(),
            artifact_id: self.artifact_id.clone(),
            path: self.rel_path.clone(),
            artifact_type: self.artifact_type.to_string(),
            size_bytes: self.size_bytes,
            modified_at: format_ts(self.modified),
        }
    }
}

fn family_str(f: &OutputFamily) -> &'static str {
    match f {
        OutputFamily::Batches => "batches",
        OutputFamily::Inventory => "inventory",
        OutputFamily::Reviews => "reviews",
        OutputFamily::Runs => "runs",
    }
}

/// Resolve the selected output root (default `<repo>/.aikit/outputs`) and reject
/// escapes. `repo_canon` must be the canonicalized repo root.
fn resolve_output_root(repo_canon: &Path, root_arg: Option<&str>) -> Result<PathBuf, AikitError> {
    let candidate = match root_arg {
        None => return Ok(repo_canon.join(".aikit").join("outputs")),
        Some(p) => {
            let pb = PathBuf::from(p);
            if pb.is_absolute() {
                pb
            } else {
                repo_canon.join(pb)
            }
        }
    };
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("--root must not contain `..`: {}", candidate.display()),
        ));
    }
    let resolved = if candidate.exists() {
        let c = fs::canonicalize(&candidate)
            .map_err(|e| AikitError::other(format!("failed to resolve --root: {e}")))?;
        if !c.starts_with(repo_canon) {
            return Err(AikitError::blocked(
                blocked::PATH_ESCAPE,
                "--root resolves outside the repository".to_string(),
            ));
        }
        c
    } else {
        if !candidate.starts_with(repo_canon) {
            return Err(AikitError::blocked(
                blocked::PATH_ESCAPE,
                "--root resolves outside the repository".to_string(),
            ));
        }
        candidate
    };
    // An explicit --root must be a known aikit output area, so that `clean` can never be
    // redirected at protected/non-output directories (e.g. `.`, `.git`, `target`,
    // `.scratch`, `.claude`, `.aikit`, `.aikit/temp`) where ordinary `runs/`/`reviews/`
    // etc. subdirectories could be mistaken for output artifacts and deleted.
    let rel = resolved
        .strip_prefix(repo_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let allowed = [".aikit/outputs", ".scratch/work/outputs"];
    let ok = allowed
        .iter()
        .any(|a| rel == *a || rel.starts_with(&format!("{a}/")));
    if !ok {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("--root must be under .aikit/outputs/ or .scratch/work/outputs/ (got {rel:?})"),
        ));
    }
    Ok(resolved)
}

/// Discover all known artifacts under `output_root`. Symlinked family directories and
/// symlinked entries are skipped (never treated as artifacts), so a symlink can never
/// redirect a later `clean` outside the output root.
fn discover(repo_canon: &Path, output_root: &Path) -> Vec<Artifact> {
    let mut artifacts = Vec::new();
    for (family, is_dir) in FAMILIES {
        let fam_dir = output_root.join(family);
        // Skip a symlinked family directory entirely.
        match fs::symlink_metadata(&fam_dir) {
            Ok(m) if m.file_type().is_symlink() => continue,
            Ok(_) => {}
            Err(_) => continue,
        }
        let entries = match fs::read_dir(&fam_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let lmeta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if lmeta.file_type().is_symlink() {
                continue; // never treat a symlink as an artifact
            }
            let modified = lmeta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let name = entry.file_name().to_string_lossy().to_string();
            if *is_dir {
                if !lmeta.is_dir() {
                    continue;
                }
                artifacts.push(Artifact {
                    family,
                    artifact_id: name.clone(),
                    rel_path: rel(repo_canon, &path),
                    artifact_type: "dir",
                    size_bytes: dir_size(&path),
                    modified,
                    real_path: path,
                });
            } else {
                if !lmeta.is_file() || !name.ends_with(".json") {
                    continue;
                }
                let id = name.trim_end_matches(".json").to_string();
                artifacts.push(Artifact {
                    family,
                    artifact_id: id,
                    rel_path: rel(repo_canon, &path),
                    artifact_type: "file",
                    size_bytes: lmeta.len(),
                    modified,
                    real_path: path,
                });
            }
        }
    }
    // Deterministic: family order (as listed) then artifact id.
    artifacts.sort_by(|a, b| {
        family_rank(a.family)
            .cmp(&family_rank(b.family))
            .then_with(|| a.artifact_id.cmp(&b.artifact_id))
    });
    artifacts
}

fn family_rank(family: &str) -> usize {
    FAMILIES
        .iter()
        .position(|(f, _)| *f == family)
        .unwrap_or(usize::MAX)
}

/// Recursive size of a directory, summing regular-file sizes and not following symlinks.
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let lmeta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if lmeta.file_type().is_symlink() {
            continue;
        }
        if lmeta.is_dir() {
            total += dir_size(&path);
        } else if lmeta.is_file() {
            total += lmeta.len();
        }
    }
    total
}

fn rel(repo_canon: &Path, path: &Path) -> String {
    path.strip_prefix(repo_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string())
}

fn format_ts(t: SystemTime) -> String {
    let odt: OffsetDateTime = t.into();
    odt.to_offset(time::UtcOffset::UTC)
        .format(TS_FORMAT)
        .unwrap_or_else(|_| "unknown".to_string())
}

fn counts_for(artifacts: &[Artifact]) -> OutputCounts {
    let count = |fam: &str| artifacts.iter().filter(|a| a.family == fam).count();
    OutputCounts {
        total: artifacts.len(),
        batches: count("batches"),
        inventory: count("inventory"),
        reviews: count("reviews"),
        runs: count("runs"),
    }
}

fn print_json<T: serde::Serialize>(record: &T) -> Result<(), AikitError> {
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| AikitError::other(format!("failed to serialize record: {e}")))?;
    println!("{json}");
    Ok(())
}

/// `aikit output list` — list known output artifacts (read-only).
pub fn list(args: OutputListArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let repo_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let output_root = resolve_output_root(&repo_canon, args.root.as_deref())?;

    let mut artifacts = discover(&repo_canon, &output_root);
    if let Some(family) = &args.family {
        let fam = family_str(family);
        artifacts.retain(|a| a.family == fam);
    }

    let counts = counts_for(&artifacts);
    let record = OutputList {
        schema_version: SCHEMA_VERSION,
        kind: KIND_OUTPUT_LIST.to_string(),
        repo_root: repo_canon.display().to_string(),
        output_root: rel(&repo_canon, &output_root),
        generated_at: OffsetDateTime::now_utc()
            .format(TS_FORMAT)
            .unwrap_or_else(|_| "unknown".to_string()),
        artifacts: artifacts.iter().map(Artifact::to_format).collect(),
        counts,
        blocked_state: None,
    };

    if args.json {
        print_json(&record)?;
    } else {
        println!("aikit output list:");
        println!("  repo root: {}", record.repo_root);
        println!("  output root: {}", record.output_root);
        println!("  artifacts: {}", record.counts.total);
        for a in &record.artifacts {
            println!(
                "    {:<9} {:<28} {:>10}  {}  {}",
                a.family, a.artifact_id, a.size_bytes, a.modified_at, a.path
            );
        }
    }
    Ok(())
}

/// `aikit output show <artifact>` — show one artifact's details (read-only).
pub fn show(args: OutputShowArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let repo_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let output_root = resolve_output_root(&repo_canon, args.root.as_deref())?;
    let output_root_canon = fs::canonicalize(&output_root).unwrap_or(output_root.clone());

    let artifacts = discover(&repo_canon, &output_root);

    // Explicit-path interpretation: if the argument names an existing path that
    // resolves outside the output root, reject it as an escape (clear message).
    for base in [&repo_canon, &output_root] {
        let cand = base.join(&args.artifact);
        if cand.exists() {
            if let Ok(c) = fs::canonicalize(&cand) {
                if !c.starts_with(&output_root_canon) {
                    return Err(AikitError::blocked(
                        blocked::PATH_ESCAPE,
                        format!(
                            "artifact resolves outside the output root: {}",
                            args.artifact
                        ),
                    ));
                }
            }
        }
    }

    // Match by id or by path against the discovered (known, in-root) artifacts.
    let matches: Vec<&Artifact> = artifacts
        .iter()
        .filter(|a| artifact_matches(&repo_canon, &output_root, a, &args.artifact))
        .collect();

    let artifact = match matches.as_slice() {
        [one] => *one,
        [] => {
            return Err(AikitError::blocked(
                blocked::ARTIFACT_NOT_FOUND,
                format!("no known output artifact matches: {}", args.artifact),
            ))
        }
        many => {
            return Err(AikitError::blocked(
                blocked::AMBIGUOUS_ARTIFACT,
                format!(
                    "{} artifacts match {:?}; pass an explicit path under the output root",
                    many.len(),
                    args.artifact
                ),
            ))
        }
    };

    let files = artifact_files(artifact);
    let metadata = artifact_metadata(artifact);

    let record = OutputShow {
        schema_version: SCHEMA_VERSION,
        kind: KIND_OUTPUT_SHOW.to_string(),
        repo_root: repo_canon.display().to_string(),
        output_root: rel(&repo_canon, &output_root),
        artifact: artifact.to_format(),
        files,
        metadata,
        blocked_state: None,
    };

    if args.json {
        print_json(&record)?;
    } else {
        println!("aikit output show:");
        println!("  repo root: {}", record.repo_root);
        println!("  output root: {}", record.output_root);
        println!("  family: {}", record.artifact.family);
        println!("  artifact id: {}", record.artifact.artifact_id);
        println!("  path: {}", record.artifact.path);
        println!("  type: {}", record.artifact.artifact_type);
        println!("  size: {} byte(s)", record.artifact.size_bytes);
        println!("  files:");
        for f in &record.files {
            println!("    {} ({} byte(s))", f.path, f.size_bytes);
        }
        if !record.metadata.is_null() {
            println!("  metadata: {}", record.metadata);
        }
    }
    Ok(())
}

/// Whether the argument matches this artifact, by id or by an explicit path.
fn artifact_matches(repo_canon: &Path, output_root: &Path, a: &Artifact, arg: &str) -> bool {
    if arg == a.artifact_id || arg == a.rel_path {
        return true;
    }
    for base in [repo_canon, output_root] {
        let cand = base.join(arg);
        if let Ok(c) = fs::canonicalize(&cand) {
            if c == a.real_path {
                return true;
            }
        }
    }
    false
}

/// Top-level files within an artifact (the file itself for a batch anchor).
fn artifact_files(a: &Artifact) -> Vec<OutputFile> {
    if a.artifact_type == "file" {
        return vec![OutputFile {
            path: a.rel_path.clone(),
            size_bytes: a.size_bytes,
        }];
    }
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&a.real_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            let lmeta = match fs::symlink_metadata(&p) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let size = if lmeta.is_dir() {
                dir_size(&p)
            } else {
                lmeta.len()
            };
            files.push(OutputFile {
                path: entry.file_name().to_string_lossy().to_string(),
                size_bytes: size,
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

/// A compact summary of an artifact's main JSON file, or `Value::Null`.
fn artifact_metadata(a: &Artifact) -> Value {
    let main = match a.family {
        "batches" => a.real_path.clone(),
        "inventory" => a.real_path.join("inventory.json"),
        "reviews" => a.real_path.join("manifest.json"),
        "runs" => a.real_path.join("run.json"),
        _ => return Value::Null,
    };
    // Never follow a symlinked metadata file — it could point outside the output root.
    match fs::symlink_metadata(&main) {
        Ok(m) if !m.file_type().is_symlink() => {}
        _ => return Value::Null,
    }
    let text = match fs::read_to_string(&main) {
        Ok(t) => t,
        Err(_) => return Value::Null,
    };
    let parsed: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Value::Null,
    };
    let keys = [
        "schema_version",
        "kind",
        "anchor_id",
        "inventory_id",
        "review_id",
        "run_id",
        "generated_at",
        "git_head",
    ];
    let mut summary = serde_json::Map::new();
    if let Some(obj) = parsed.as_object() {
        for k in keys {
            if let Some(v) = obj.get(k) {
                summary.insert(k.to_string(), v.clone());
            }
        }
    }
    if summary.is_empty() {
        Value::Null
    } else {
        Value::Object(summary)
    }
}

/// `aikit output clean` — dry-run by default; deletes only with `--execute` plus a
/// selector (`--older-than` or `--all`). Deletes only known artifacts under the root.
pub fn clean(args: OutputCleanArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let repo_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let output_root = resolve_output_root(&repo_canon, args.root.as_deref())?;

    let execute = args.execute;
    let dry_run = !execute;

    // Age threshold (only when --older-than is given).
    let older_than = match &args.older_than {
        Some(s) => Some(parse_duration(s)?),
        None => None,
    };
    let now = SystemTime::now();

    let mut candidates: Vec<Artifact> = discover(&repo_canon, &output_root)
        .into_iter()
        .filter(|a| match &args.family {
            Some(f) => a.family == family_str(f),
            None => true,
        })
        .filter(|a| match older_than {
            Some(d) => is_older_than(a.modified, now, d),
            None => true,
        })
        .collect();
    candidates.sort_by(|a, b| {
        family_rank(a.family)
            .cmp(&family_rank(b.family))
            .then_with(|| a.artifact_id.cmp(&b.artifact_id))
    });

    let output_root_canon = fs::canonicalize(&output_root).unwrap_or(output_root.clone());
    let mut deleted: Vec<String> = Vec::new();
    if execute {
        for a in &candidates {
            if !safe_to_delete(&output_root_canon, a) {
                continue;
            }
            let result = if a.artifact_type == "dir" {
                fs::remove_dir_all(&a.real_path)
            } else {
                fs::remove_file(&a.real_path)
            };
            match result {
                Ok(()) => deleted.push(a.rel_path.clone()),
                Err(e) => {
                    return Err(AikitError::other(format!(
                        "failed to delete {}: {e}",
                        a.rel_path
                    )))
                }
            }
        }
    }

    let record = OutputClean {
        schema_version: SCHEMA_VERSION,
        kind: KIND_OUTPUT_CLEAN.to_string(),
        repo_root: repo_canon.display().to_string(),
        output_root: rel(&repo_canon, &output_root),
        dry_run,
        execute,
        filters: OutputCleanFilters {
            family: args.family.as_ref().map(|f| family_str(f).to_string()),
            older_than: args.older_than.clone(),
            all: args.all,
        },
        candidates: candidates.iter().map(Artifact::to_format).collect(),
        deleted: deleted.clone(),
        counts: OutputCleanCounts {
            candidates: candidates.len(),
            deleted: deleted.len(),
        },
        blocked_state: None,
    };

    if args.json {
        print_json(&record)?;
    } else {
        println!("aikit output clean:");
        println!("  repo root: {}", record.repo_root);
        println!("  output root: {}", record.output_root);
        println!(
            "  mode: {}",
            if record.dry_run { "dry-run" } else { "execute" }
        );
        println!(
            "  filters: family={:?} older_than={:?} all={}",
            record.filters.family, record.filters.older_than, record.filters.all
        );
        println!("  candidates: {}", record.counts.candidates);
        println!("  deleted: {}", record.counts.deleted);
        let verb = if record.dry_run {
            "would delete"
        } else {
            "deleted"
        };
        for a in &record.candidates {
            let marker = if record.dry_run || record.deleted.contains(&a.path) {
                verb
            } else {
                "skipped"
            };
            println!("    {marker}: {} {} ({})", a.family, a.artifact_id, a.path);
        }
    }
    Ok(())
}

/// Defensive re-check before deletion: inside the output root and not a symlink.
fn safe_to_delete(output_root_canon: &Path, a: &Artifact) -> bool {
    if !a.real_path.starts_with(output_root_canon) {
        return false;
    }
    match fs::symlink_metadata(&a.real_path) {
        Ok(m) => !m.file_type().is_symlink(),
        Err(_) => false,
    }
}

fn is_older_than(modified: SystemTime, now: SystemTime, age: Duration) -> bool {
    match now.duration_since(modified) {
        Ok(elapsed) => elapsed > age,
        Err(_) => false, // modified in the future → not older
    }
}

/// Parse a simple duration: `<n>h` (hours) or `<n>d` (days).
fn parse_duration(s: &str) -> Result<Duration, AikitError> {
    let invalid = || {
        AikitError::other(format!(
            "invalid --older-than duration {s:?}: use <n>h (hours) or <n>d (days), e.g. 24h or 7d"
        ))
    };
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let n: u64 = num.parse().map_err(|_| invalid())?;
    // checked_mul so a huge value can never silently wrap to a tiny duration (which
    // would make `--older-than` match fresh artifacts and delete them).
    match unit {
        "h" => n
            .checked_mul(3600)
            .map(Duration::from_secs)
            .ok_or_else(invalid),
        "d" => n
            .checked_mul(86_400)
            .map(Duration::from_secs)
            .ok_or_else(invalid),
        _ => Err(invalid()),
    }
}
