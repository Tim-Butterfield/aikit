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

use crate::cli::{ScanSecretsArgs, Severity};
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

/// What a severity means.
///
/// Severity is **confidence that the match is a real credential**, not an estimate of blast
/// radius — aikit cannot know what a credential protects, and pretending otherwise would be
/// the kind of judgment this tool does not make.
///
/// * `high` — the text matches a credential format that **identifies itself**: a fixed
///   prefix and length that essentially nothing else produces (a PEM private-key header, a
///   `ghp_`/`xox`/`sk_live_` token), or a credential-named field assigned a long opaque
///   value. A false positive here is usually a deliberately published example.
/// * `medium` — a credential-named field (`password`, `api_key`, …) assigned an ordinary
///   value. Frequently a placeholder, a variable reference, or documentation prose, but it
///   cannot be ruled out.
/// * `low` — the same shape as `medium`, in a file whose path marks it as an example,
///   template, or fixture. Reported for completeness; least likely to be real.
///
/// The scale is deliberately monotonic so `--fail-on <severity>` means "this level or
/// above" without a caller needing a lookup table.
pub const SEVERITY_HIGH: &str = "high";
pub const SEVERITY_MEDIUM: &str = "medium";
pub const SEVERITY_LOW: &str = "low";

/// A self-identifying credential format: a fixed prefix and shape that essentially nothing
/// else produces. These are the rules that make `high` mean something, which is what lets
/// `--fail-on high` be a defensible CI gate.
///
/// Deliberately NOT a corpus. aikit is not a secret scanner and will not win a coverage race
/// against gitleaks or trufflehog; every entry here has to be unambiguous enough that a hit
/// is worth acting on. Breadth belongs to a dedicated tool — see the command's help.
struct FormatRule {
    id: &'static str,
    description: &'static str,
    re: Regex,
}

/// Compiled heuristic rules. Built once per invocation.
struct Rules {
    /// Self-identifying credential formats, all `high`.
    formats: Vec<FormatRule>,
    assignment: Regex,
}

impl Rules {
    fn new() -> Self {
        // Rule ids and descriptions stay vendor-neutral in wording; the *patterns* must be
        // vendor-specific, because a fixed prefix is exactly what makes a match unambiguous.
        let formats = vec![
            FormatRule {
                id: "private_key_block",
                description: "Private key PEM block marker",
                re: Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----").expect("valid regex"),
            },
            FormatRule {
                id: "access_key_id",
                description: "Cloud access key identifier pattern",
                re: Regex::new(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b").expect("valid regex"),
            },
            FormatRule {
                id: "vcs_host_token",
                description: "Source-host personal/OAuth access token pattern",
                re: Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{36}\b").expect("valid regex"),
            },
            FormatRule {
                id: "chat_platform_token",
                description: "Chat-platform bot/user token pattern",
                re: Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b").expect("valid regex"),
            },
            FormatRule {
                id: "payment_live_secret_key",
                description: "Payment-provider LIVE secret key pattern",
                re: Regex::new(r"\b[sr]k_live_[A-Za-z0-9]{16,}\b").expect("valid regex"),
            },
            FormatRule {
                id: "cloud_api_key",
                description: "Cloud API key pattern",
                re: Regex::new(r"\bAIza[0-9A-Za-z_\-]{35}\b").expect("valid regex"),
            },
            FormatRule {
                id: "model_provider_key",
                description: "Model-provider API key pattern",
                re: Regex::new(r"\bsk-ant-[A-Za-z0-9_\-]{20,}\b").expect("valid regex"),
            },
            FormatRule {
                id: "package_registry_token",
                description: "Package-registry access token pattern",
                re: Regex::new(r"\bnpm_[A-Za-z0-9]{36}\b").expect("valid regex"),
            },
            FormatRule {
                id: "signed_web_token",
                description: "Signed web token (JWT) pattern",
                // Three base64url segments where the first two start with `eyJ` — the
                // encoding of `{"`, so this is the token's own self-description.
                re: Regex::new(
                    r"\beyJ[A-Za-z0-9_\-]{8,}\.eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}",
                )
                .expect("valid regex"),
            },
        ];

        // Credential-style assignment: a known secret-ish name assigned to a value. The
        // value capture is used ONLY to classify severity (length/charset); it is never
        // emitted.
        let assignment = Regex::new(
            r#"(?i)\b(api[-_]?key|apikey|access[-_]?token|auth[-_]?token|client[-_]?secret|secret|password|passwd|private[-_]?key|token)\b["']?\s*[:=]\s*["']?([^\s"'`]{3,})"#,
        )
        .expect("valid regex");

        Rules {
            formats,
            assignment,
        }
    }

    /// Findings on one line, as `(rule_id, description, severity)`. The matched value is
    /// never returned.
    ///
    /// `example_file` marks a path that names itself as an example, template, or fixture.
    /// It downgrades only the *name-based* rule: `password = hunter2` in `config.example`
    /// is almost certainly a placeholder, whereas a real PEM key committed under
    /// `testdata/` is still a real leaked key and keeps its severity.
    fn scan_line(
        &self,
        line: &str,
        example_file: bool,
    ) -> Vec<(&'static str, &'static str, &'static str)> {
        let mut out: Vec<(&'static str, &'static str, &'static str)> = Vec::new();
        for rule in &self.formats {
            if rule.re.is_match(line) {
                out.push((rule.id, rule.description, SEVERITY_HIGH));
            }
        }
        for caps in self.assignment.captures_iter(line) {
            let value = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if is_token_like(value) {
                out.push((
                    "long_token_assignment",
                    "Long token-like value assigned to a credential-style name",
                    SEVERITY_HIGH,
                ));
            } else if example_file {
                out.push((
                    "credential_assignment",
                    "Credential-style name assigned a value, in an example/template file",
                    SEVERITY_LOW,
                ));
            } else {
                out.push((
                    "credential_assignment",
                    "Credential-style name assigned a value",
                    SEVERITY_MEDIUM,
                ));
            }
        }
        out
    }
}

/// Whether a repo-relative path names itself as an example, template, or fixture.
///
/// Used only to demote the name-based rule (see `scan_line`). Matching is on the path text,
/// which is the point: this is about how the file is *labelled*, not what it contains.
fn is_example_path(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    const MARKERS: &[&str] = &[".example", ".sample", ".template", ".dist", ".default"];
    if MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    const DIRS: &[&str] = &["examples/", "fixtures/", "testdata/", "__fixtures__/"];
    DIRS.iter().any(|d| lower.contains(d))
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
    let repo_canon = crate::formats::canonicalize(&root)
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
                let example_file = is_example_path(rel);
                for (idx, line) in content.lines().enumerate() {
                    for (rule_id, description, severity) in rules.scan_line(line, example_file) {
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

    let high = findings
        .iter()
        .filter(|f| f.severity == SEVERITY_HIGH)
        .count();
    let medium = findings
        .iter()
        .filter(|f| f.severity == SEVERITY_MEDIUM)
        .count();
    let low = findings
        .iter()
        .filter(|f| f.severity == SEVERITY_LOW)
        .count();

    // `--fail-on <severity>` means "this level or above". Gating on `findings > 0` would
    // fail identically for one leaked private key and eleven `password =` lines in
    // documentation — collapsing a graded result onto its least reliable aggregate.
    let should_fail = match args.fail_on {
        Some(Severity::High) => high > 0,
        Some(Severity::Medium) => high + medium > 0,
        Some(Severity::Low) => !findings.is_empty(),
        None => false,
    };
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
        fail_on: args.fail_on.map(|s| s.as_str().to_string()),
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
        let threshold = args.fail_on.map(|s| s.as_str()).unwrap_or(SEVERITY_LOW);
        let message = format!(
            "findings at or above {threshold}: {} high, {} medium, {} low",
            record.counts.high, record.counts.medium, record.counts.low
        );
        // The report above *is* the machine-readable answer; the non-zero exit is only the
        // gate. Adding an error document after it would put two JSON objects on stdout.
        return Err(if args.json {
            AikitError::blocked_after_json(blocked::SECRET_FINDINGS, message)
        } else {
            AikitError::blocked(blocked::SECRET_FINDINGS, message)
        });
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
    let real = crate::formats::canonicalize(&raw)
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
