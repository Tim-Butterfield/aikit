//! `aikit scan secrets <path>...` — a local, best-effort heuristic scan for likely
//! secrets in explicit repo-local paths.
//!
//! This is HEURISTIC and best-effort: it can false-positive and false-negative, never
//! proves a file or repo is safe to share, makes no judgment about whether a finding is a
//! live credential, and does not replace dedicated secret-scanning tools. It NEVER emits
//! raw matched secret values — findings carry only path/line/rule/severity, and `redacted`
//! is always true. It creates no output artifacts and never touches remotes.

use std::fs;
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;
use regex::Regex;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::cli::ScanSecretsArgs;
use crate::errors::{blocked, AikitError};
use crate::formats::{
    ScanCounts, ScanFinding, ScanSecrets, ScanSkipped, KIND_SCAN_SECRETS, SCHEMA_VERSION,
};
use crate::repo;

const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");

/// Default per-file size cap (1 MiB). Files larger than this are skipped.
const DEFAULT_MAX_FILE_BYTES: u64 = 1_048_576;

/// Byte budget scanned for a NUL byte when deciding if a file is binary.
const BINARY_SNIFF_BYTES: usize = 8192;

/// Compiled heuristic rules. Built once per invocation.
struct Rules {
    private_key: Regex,
    access_key_id: Regex,
    assignment: Regex,
}

impl Rules {
    fn new() -> Self {
        // Private-key PEM block marker (e.g. `-----BEGIN OPENSSH PRIVATE KEY-----`).
        let private_key =
            Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----").expect("valid regex");
        // Generic cloud access-key identifier form (no vendor naming in the rule id/desc).
        let access_key_id = Regex::new(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b").expect("valid regex");
        // Credential-style assignment: a known secret-ish name assigned to a value. The
        // value capture is used ONLY to classify severity (length/charset); it is never
        // emitted.
        let assignment = Regex::new(
            r#"(?i)\b(api[-_]?key|apikey|access[-_]?token|auth[-_]?token|client[-_]?secret|secret|password|passwd|private[-_]?key|token)\b["']?\s*[:=]\s*["']?([^\s"'`]{3,})"#,
        )
        .expect("valid regex");
        Rules {
            private_key,
            access_key_id,
            assignment,
        }
    }

    /// Findings on one line, as `(rule_id, description, severity)`. The matched value is
    /// never returned.
    fn scan_line(&self, line: &str) -> Vec<(&'static str, &'static str, &'static str)> {
        let mut out: Vec<(&'static str, &'static str, &'static str)> = Vec::new();
        if self.private_key.is_match(line) {
            out.push(("private_key_block", "Private key PEM block marker", "high"));
        }
        if self.access_key_id.is_match(line) {
            out.push((
                "access_key_id",
                "Cloud access key identifier pattern",
                "high",
            ));
        }
        for caps in self.assignment.captures_iter(line) {
            let value = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if is_token_like(value) {
                out.push((
                    "long_token_assignment",
                    "Long token-like value assigned to a credential-style name",
                    "high",
                ));
            } else {
                out.push((
                    "credential_assignment",
                    "Credential-style name assigned a value",
                    "medium",
                ));
            }
        }
        out
    }
}

/// Whether a value looks like a long opaque token (length + token charset). Used only to
/// raise severity; the value itself is never recorded.
fn is_token_like(value: &str) -> bool {
    value.len() >= 20
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./+=".contains(c))
}

/// `aikit scan secrets` — scan explicit repo-local paths for likely secrets.
pub fn secrets(args: ScanSecretsArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let repo_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let max_file_bytes = args.max_file_bytes.unwrap_or(DEFAULT_MAX_FILE_BYTES);
    let rules = Rules::new();

    // Resolve and collect the deterministic set of repo-relative files to scan. Explicit
    // files are scanned even when ignored; directories are traversed (gitignore-aware by
    // default). `.git/` is always excluded.
    let mut targets: Vec<(PathBuf, String)> = Vec::new();
    for input in &args.paths {
        let real = resolve_input(&repo_canon, input)?;
        if real.is_dir() {
            collect_dir(&repo_canon, &real, args.include_ignored, &mut targets);
        } else {
            let rel = rel_path(&repo_canon, &real);
            if !is_under_git(&rel) {
                targets.push((real, rel));
            }
        }
    }
    targets.sort_by(|a, b| a.1.cmp(&b.1));
    targets.dedup_by(|a, b| a.1 == b.1);

    let mut findings: Vec<ScanFinding> = Vec::new();
    let mut skipped: Vec<ScanSkipped> = Vec::new();
    let mut files_scanned = 0usize;

    for (abs, rel) in &targets {
        match read_text(abs, max_file_bytes) {
            ReadOutcome::Text(content) => {
                files_scanned += 1;
                for (idx, line) in content.lines().enumerate() {
                    for (rule_id, description, severity) in rules.scan_line(line) {
                        findings.push(ScanFinding {
                            path: rel.clone(),
                            line: idx + 1,
                            rule_id: rule_id.to_string(),
                            description: description.to_string(),
                            severity: severity.to_string(),
                            redacted: true,
                        });
                    }
                }
            }
            ReadOutcome::Skip(reason) => skipped.push(ScanSkipped {
                path: rel.clone(),
                reason: reason.to_string(),
            }),
        }
    }

    findings.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.rule_id.cmp(&b.rule_id))
    });
    skipped.sort_by(|a, b| a.path.cmp(&b.path));

    let high = findings.iter().filter(|f| f.severity == "high").count();
    let medium = findings.iter().filter(|f| f.severity == "medium").count();
    let low = findings.iter().filter(|f| f.severity == "low").count();

    let should_fail = args.fail_on_findings && !findings.is_empty();
    let counts = ScanCounts {
        findings: findings.len(),
        high,
        medium,
        low,
        files_scanned,
        files_skipped: skipped.len(),
    };

    let record = ScanSecrets {
        schema_version: SCHEMA_VERSION,
        kind: KIND_SCAN_SECRETS.to_string(),
        repo_root: repo_canon.display().to_string(),
        generated_at: OffsetDateTime::now_utc()
            .format(TS_FORMAT)
            .unwrap_or_default(),
        inputs: args.paths.clone(),
        include_ignored: args.include_ignored,
        max_file_bytes,
        files_scanned,
        files_skipped: skipped,
        findings,
        counts,
        fail_on_findings: args.fail_on_findings,
        blocked_state: if should_fail {
            Some(blocked::SECRET_FINDINGS.to_string())
        } else {
            None
        },
    };

    if args.json {
        let json = serde_json::to_string_pretty(&record)
            .map_err(|e| AikitError::other(format!("failed to serialize report: {e}")))?;
        println!("{json}");
    } else {
        render_human(&record);
    }

    if should_fail {
        return Err(AikitError::blocked(
            blocked::SECRET_FINDINGS,
            format!(
                "{} likely-secret finding(s) present (--fail-on-findings)",
                record.counts.findings
            ),
        ));
    }
    Ok(())
}

/// Resolve an explicit input path relative to the repo root, rejecting `..`, symlink
/// targets, and anything that resolves outside the repository.
fn resolve_input(repo_canon: &Path, input: &str) -> Result<PathBuf, AikitError> {
    let raw = if Path::new(input).is_absolute() {
        PathBuf::from(input)
    } else {
        repo_canon.join(input)
    };
    if raw.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("path must not contain `..`: {input}"),
        ));
    }
    // Do not follow a symlink target supplied as an explicit input.
    if let Ok(m) = fs::symlink_metadata(&raw) {
        if m.file_type().is_symlink() {
            return Err(AikitError::blocked(
                blocked::PATH_ESCAPE,
                format!("path is a symlink (not followed): {input}"),
            ));
        }
    }
    let real = fs::canonicalize(&raw)
        .map_err(|_| AikitError::other(format!("path not found or unreadable: {input}")))?;
    if !real.starts_with(repo_canon) {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("path resolves outside the repository: {input}"),
        ));
    }
    Ok(real)
}

/// Collect regular files under a directory via gitignore-aware traversal (symlinks are
/// not followed). `.git/` is always pruned.
fn collect_dir(
    repo_canon: &Path,
    dir: &Path,
    include_ignored: bool,
    out: &mut Vec<(PathBuf, String)>,
) {
    let mut builder = WalkBuilder::new(dir);
    builder
        .hidden(false)
        .follow_links(false)
        .git_ignore(!include_ignored)
        .git_global(!include_ignored)
        .git_exclude(!include_ignored)
        .ignore(!include_ignored)
        .parents(!include_ignored);
    // Prune `.git/` regardless of ignore settings.
    builder.filter_entry(|entry| entry.file_name() != ".git");

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        // Regular files only (symlinks report as symlink and are skipped).
        let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }
        let abs = entry.path().to_path_buf();
        let rel = rel_path(repo_canon, &abs);
        if is_under_git(&rel) {
            continue;
        }
        out.push((abs, rel));
    }
}

/// The repo-relative, forward-slash form of `path`.
fn rel_path(repo_canon: &Path, path: &Path) -> String {
    path.strip_prefix(repo_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string())
}

fn is_under_git(rel: &str) -> bool {
    // Component-based so the always-exclude-`.git/` guarantee holds for a nested `.git`
    // (e.g. a submodule's `sub/.git/...`) supplied as an explicit path, not just the
    // repo's top-level `.git`.
    rel.split('/').any(|c| c == ".git")
}

enum ReadOutcome {
    Text(String),
    Skip(&'static str),
}

/// Read a file for scanning: skip oversized files, binary files (NUL byte in the sniff
/// window), and unreadable files.
fn read_text(path: &Path, max_file_bytes: u64) -> ReadOutcome {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return ReadOutcome::Skip("unreadable"),
    };
    if meta.len() > max_file_bytes {
        return ReadOutcome::Skip("too_large");
    }
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return ReadOutcome::Skip("unreadable"),
    };
    let sniff = bytes.len().min(BINARY_SNIFF_BYTES);
    if bytes[..sniff].contains(&0u8) {
        return ReadOutcome::Skip("binary");
    }
    match String::from_utf8(bytes) {
        Ok(s) => ReadOutcome::Text(s),
        Err(_) => ReadOutcome::Skip("binary"),
    }
}

fn render_human(r: &ScanSecrets) {
    println!("aikit scan secrets (best-effort, heuristic):");
    println!("  repo root: {}", r.repo_root);
    println!("  inputs: {}", r.inputs.join(", "));
    println!(
        "  files scanned: {}  skipped: {}",
        r.counts.files_scanned, r.counts.files_skipped
    );
    if r.findings.is_empty() {
        println!("  findings: none");
    } else {
        println!(
            "  findings: {} (high {}, medium {}, low {}) — values redacted",
            r.counts.findings, r.counts.high, r.counts.medium, r.counts.low
        );
        for f in &r.findings {
            println!(
                "    {}:{}  [{}] {} ({})",
                f.path, f.line, f.severity, f.rule_id, f.description
            );
        }
    }
    if !r.files_skipped.is_empty() {
        println!("  skipped files:");
        for s in &r.files_skipped {
            println!("    {} ({})", s.path, s.reason);
        }
    }
    println!(
        "  note: heuristic and best-effort; findings are not proof of a live credential, \
and no findings does not prove a file is safe to share. Inspect every finding."
    );
}
