//! `aikit review generate --files <file>...` — bounded review-bundle generation.
//!
//! Resolves explicit input files relative to the repo root (rejecting paths that
//! escape the repo, including via symlinks), sorts them deterministically, applies
//! per-file and total-byte caps, and writes a readable text bundle plus a manifest
//! that records every scoped file exactly once.
//!
//! Memory is bounded: file metadata is collected first (no content held), files are
//! processed one at a time, SHA-256 is streamed, per-file byte caps read only the
//! capped prefix, and the bundle is written incrementally to disk.

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::cli::ReviewGenerateArgs;
use crate::errors::{blocked, AikitError};
use crate::formats::{
    ReviewFile, ReviewInputs, ReviewLimits, ReviewManifest, ReviewTotals, KIND_REVIEW_BUNDLE,
    SCHEMA_VERSION,
};
use crate::{output, repo};

const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
const ID_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

const BUNDLE_NAME: &str = "review_bundle.txt";
const MANIFEST_NAME: &str = "manifest.json";

/// A resolved input file: its apparent repo-relative path, the real path to read,
/// and its content size. No file content is held at this stage.
struct ScopedMeta {
    rel: String,
    real: PathBuf,
    size_bytes: u64,
}

pub fn generate(args: ReviewGenerateArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let root_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;

    // Determine the input mode and the list of input path strings. The CLI enforces
    // exactly one of --files / --anchor, so the bundle pipeline below is identical
    // regardless of mode — only the source of the path list differs.
    let mut inputs = ReviewInputs {
        mode: "explicit_files".to_string(),
        anchor_path: None,
        anchor_id: None,
        files: Vec::new(),
    };
    let input_paths: Vec<String> = if let Some(anchor) = &args.anchor {
        // Anchor mode: validate the anchor and compute the changed files since it,
        // reusing the same logic as `batch changed` (missing/invalid/cross-repo
        // anchors surface the same blocked states).
        let (anchor_id, paths) = crate::batch::changed_files_since_anchor(&root, anchor)?;
        inputs.mode = "changed_since_anchor".to_string();
        inputs.anchor_path = Some(anchor.clone());
        inputs.anchor_id = Some(anchor_id);
        paths
    } else {
        args.files.clone()
    };

    // First pass: resolve, validate, collect metadata, de-duplicate. No content read.
    let mut metas: Vec<ScopedMeta> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for input in &input_paths {
        let (rel, real) = resolve_input(&root_canon, input)?;
        if seen.contains(&rel) {
            continue; // dedupe: every scoped file appears exactly once
        }
        let size_bytes = fs::metadata(&real)
            .map_err(|_| {
                AikitError::blocked(
                    blocked::UNREADABLE_FILE,
                    format!("input file could not be read: {input}"),
                )
            })?
            .len();
        seen.push(rel.clone());
        metas.push(ScopedMeta {
            rel,
            real,
            size_bytes,
        });
    }

    // Deterministic order before applying any caps.
    metas.sort_by(|a, b| a.rel.cmp(&b.rel));

    let now = OffsetDateTime::now_utc();
    let head = repo::git_head(&root);
    let review_id = format!("{}-{}", format_ts(now, ID_FORMAT), short_head(&head));
    let generated_at = format_ts(now, TS_FORMAT);

    // Resolve output directory (relative --output is taken under the repo root).
    let selected = output::select_output_root(&root, args.output.as_deref());
    let out_root = if selected.is_absolute() {
        selected
    } else {
        root.join(selected)
    };
    let dir = output::reviews_dir(&out_root).join(&review_id);
    fs::create_dir_all(&dir).map_err(|e| {
        AikitError::other(format!(
            "failed to create output dir {}: {e}",
            dir.display()
        ))
    })?;

    // Second pass: hash, apply caps, write the bundle incrementally, build manifest.
    let bundle_path = dir.join(BUNDLE_NAME);
    let manifest_path = dir.join(MANIFEST_NAME);
    let mut bundle = BufWriter::new(File::create(&bundle_path).map_err(|e| {
        AikitError::other(format!("failed to write {}: {e}", bundle_path.display()))
    })?);
    write_bundle_header(
        &mut bundle,
        &root.display().to_string(),
        &head,
        &generated_at,
    )
    .map_err(|e| AikitError::other(format!("failed to write bundle: {e}")))?;

    let mut files: Vec<ReviewFile> = Vec::with_capacity(metas.len());
    let mut running_total: u64 = 0;
    let mut total_cap_reached = false;

    for m in &metas {
        let sha = stream_sha256(&m.real)?;

        // Once the total cap is reached, this and every later file is omitted.
        if total_cap_reached {
            write_omitted_section(&mut bundle, &m.rel, &sha, m.size_bytes)
                .map_err(|e| AikitError::other(format!("failed to write bundle: {e}")))?;
            files.push(omitted_entry(m, sha));
            continue;
        }

        // Read only the capped prefix (bounded by --max-file-bytes when set).
        let (embedded, truncated, file_cap) = read_capped(
            &m.real,
            m.size_bytes,
            args.max_file_bytes,
            args.max_file_lines,
        )?;
        let bytes_included = embedded.len() as u64;
        let lines_included = count_lines(&embedded);

        if matches!(args.max_total_bytes, Some(cap) if running_total + bytes_included > cap) {
            total_cap_reached = true;
            write_omitted_section(&mut bundle, &m.rel, &sha, m.size_bytes)
                .map_err(|e| AikitError::other(format!("failed to write bundle: {e}")))?;
            files.push(omitted_entry(m, sha));
            continue;
        }

        running_total += bytes_included;
        write_included_section(
            &mut bundle,
            &m.rel,
            &sha,
            m.size_bytes,
            truncated,
            &embedded,
        )
        .map_err(|e| AikitError::other(format!("failed to write bundle: {e}")))?;
        files.push(ReviewFile {
            path: m.rel.clone(),
            size_bytes: m.size_bytes,
            sha256: sha,
            included: true,
            truncated,
            lines_included,
            bytes_included,
            omitted_reason: None,
            cap_hit: file_cap,
        });
    }

    bundle
        .flush()
        .map_err(|e| AikitError::other(format!("failed to write bundle: {e}")))?;

    let totals = ReviewTotals {
        files_total: files.len(),
        files_included: files.iter().filter(|f| f.included).count(),
        files_omitted: files.iter().filter(|f| !f.included).count(),
        bytes_included: running_total,
    };

    let manifest = ReviewManifest {
        schema_version: SCHEMA_VERSION,
        kind: KIND_REVIEW_BUNDLE.to_string(),
        review_id,
        repo_root: root.display().to_string(),
        git_head: head,
        generated_at,
        inputs: ReviewInputs {
            files: metas.iter().map(|m| m.rel.clone()).collect(),
            ..inputs
        },
        limits: ReviewLimits {
            max_file_bytes: args.max_file_bytes,
            max_total_bytes: args.max_total_bytes,
            max_file_lines: args.max_file_lines,
        },
        files,
        bundle_path: BUNDLE_NAME.to_string(),
        totals,
    };

    // The on-disk manifest.json stays a pure, reproducible artifact (it does not
    // embed its own location). The created paths are reported separately.
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| AikitError::other(format!("failed to serialize manifest: {e}")))?;
    write_with_newline(&manifest_path, &json)?;

    let written = vec![
        display_relative(&root, &bundle_path),
        display_relative(&root, &manifest_path),
    ];

    if args.json {
        // Stdout adds the created artifact paths alongside the manifest fields,
        // without altering the on-disk artifact (mirrors `batch start`).
        let mut value = serde_json::to_value(&manifest)
            .map_err(|e| AikitError::other(format!("failed to serialize manifest: {e}")))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("written".to_string(), serde_json::json!(written));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&value)
                .map_err(|e| AikitError::other(format!("failed to serialize output: {e}")))?
        );
    } else {
        println!("Review bundle written:");
        println!("  {}", display_relative(&root, &bundle_path));
        println!("  {}", display_relative(&root, &manifest_path));
        println!(
            "  {} file(s): {} included, {} omitted, {} byte(s) bundled",
            manifest.totals.files_total,
            manifest.totals.files_included,
            manifest.totals.files_omitted,
            manifest.totals.bytes_included
        );
    }
    Ok(())
}

fn omitted_entry(m: &ScopedMeta, sha: String) -> ReviewFile {
    ReviewFile {
        path: m.rel.clone(),
        size_bytes: m.size_bytes,
        sha256: sha,
        included: false,
        truncated: false,
        lines_included: 0,
        bytes_included: 0,
        omitted_reason: Some("max_total_bytes".to_string()),
        cap_hit: Some("total_bytes".to_string()),
    }
}

/// Resolve an input path to `(apparent repo-relative path, real path to read)`.
///
/// The candidate is lexically normalized first so that absolute or `..` escapes are
/// rejected as `blocked_path_escape` even when the target does not exist. It is then
/// canonicalized to confirm the real target exists, is a regular file, and stays
/// inside the repo (rejecting symlink escapes). The *apparent* (lexical) path is
/// reported, so a requested in-repo symlink keeps its requested name.
fn resolve_input(root_canon: &Path, input: &str) -> Result<(String, PathBuf), AikitError> {
    let raw = PathBuf::from(input);
    let candidate = if raw.is_absolute() {
        raw
    } else {
        root_canon.join(&raw)
    };

    let lexical = lexical_normalize(&candidate);
    let rel = lexical.strip_prefix(root_canon).map_err(|_| {
        AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("input path resolves outside the repository: {input}"),
        )
    })?;
    let rel = rel.to_string_lossy().replace('\\', "/");

    // Canonicalize the *lexically normalized* path (not the raw candidate), so the
    // file actually read/hashed matches the apparent path reported in `rel`. Using
    // the raw candidate would let an in-repo symlinked directory plus `..` resolve
    // to a different file than `rel` names.
    let real = fs::canonicalize(&lexical).map_err(|_| {
        AikitError::blocked(
            blocked::UNREADABLE_FILE,
            format!("input file not found or unreadable: {input}"),
        )
    })?;
    if real.strip_prefix(root_canon).is_err() {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("input path resolves (via symlink) outside the repository: {input}"),
        ));
    }
    if !real.is_file() {
        return Err(AikitError::blocked(
            blocked::UNREADABLE_FILE,
            format!("input path is not a regular file: {input}"),
        ));
    }
    Ok((rel, real))
}

/// Lexically normalize a path (resolve `.` and `..` without touching the
/// filesystem). Excess `..` components are clamped at the root.
fn lexical_normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(c) => out.push(c),
        }
    }
    out
}

/// Read the capped prefix of a file. `--max-file-bytes` bounds the read so a huge
/// file is never fully buffered; `--max-file-lines` further trims to whole lines.
fn read_capped(
    path: &Path,
    size_bytes: u64,
    max_file_bytes: Option<u64>,
    max_file_lines: Option<usize>,
) -> Result<(Vec<u8>, bool, Option<String>), AikitError> {
    let read_err = || {
        AikitError::blocked(
            blocked::UNREADABLE_FILE,
            format!("input file could not be read: {}", path.display()),
        )
    };
    let file = File::open(path).map_err(|_| read_err())?;
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut cap_hit: Option<String> = None;

    if let Some(max) = max_file_bytes {
        file.take(max)
            .read_to_end(&mut bytes)
            .map_err(|_| read_err())?;
        if size_bytes > max {
            truncated = true;
            cap_hit = Some("file_bytes".to_string());
        }
    } else {
        let mut file = file;
        file.read_to_end(&mut bytes).map_err(|_| read_err())?;
    }

    if let Some(max_lines) = max_file_lines {
        if let Some(cut) = nth_line_end(&bytes, max_lines) {
            if cut < bytes.len() {
                bytes.truncate(cut);
                truncated = true;
                if cap_hit.is_none() {
                    cap_hit = Some("file_lines".to_string());
                }
            }
        }
    }

    Ok((bytes, truncated, cap_hit))
}

/// Byte offset just past the end of the `n`th line, or `None` when the content has
/// at most `n` lines.
fn nth_line_end(bytes: &[u8], n: usize) -> Option<usize> {
    if n == 0 {
        return Some(0);
    }
    let mut seen = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            seen += 1;
            if seen == n {
                return Some(i + 1);
            }
        }
    }
    None
}

fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let newlines = bytes.iter().filter(|b| **b == b'\n').count();
    if bytes.last() == Some(&b'\n') {
        newlines
    } else {
        newlines + 1
    }
}

fn write_bundle_header<W: Write>(
    w: &mut W,
    repo_root: &str,
    head: &str,
    generated_at: &str,
) -> std::io::Result<()> {
    writeln!(w, "# aikit Review Bundle\n")?;
    writeln!(w, "Repo: {repo_root}")?;
    writeln!(w, "HEAD: {head}")?;
    writeln!(w, "Generated: {generated_at}\n")?;
    writeln!(w, "## Files")
}

fn write_included_section<W: Write>(
    w: &mut W,
    rel: &str,
    sha: &str,
    size_bytes: u64,
    truncated: bool,
    content: &[u8],
) -> std::io::Result<()> {
    writeln!(w, "\n### {rel}")?;
    writeln!(w, "SHA-256: {sha}")?;
    writeln!(w, "Size: {size_bytes}")?;
    writeln!(w, "Truncated: {truncated}\n")?;
    let body = String::from_utf8_lossy(content);
    let fence = "`".repeat(fence_len(&body));
    writeln!(w, "{fence}text")?;
    w.write_all(body.as_bytes())?;
    if !body.ends_with('\n') {
        writeln!(w)?;
    }
    writeln!(w, "{fence}")
}

fn write_omitted_section<W: Write>(
    w: &mut W,
    rel: &str,
    sha: &str,
    size_bytes: u64,
) -> std::io::Result<()> {
    writeln!(w, "\n### {rel}")?;
    writeln!(w, "SHA-256: {sha}")?;
    writeln!(w, "Size: {size_bytes}")?;
    writeln!(w, "Truncated: false")?;
    writeln!(w, "Omitted: max_total_bytes")
}

/// Fence length: one longer than the longest run of consecutive backticks in the
/// content, with a minimum of three.
fn fence_len(content: &str) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    for ch in content.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    (longest + 1).max(3)
}

/// Stream a file through SHA-256 in fixed-size chunks (bounded memory).
fn stream_sha256(path: &Path) -> Result<String, AikitError> {
    let mut file = File::open(path).map_err(|_| {
        AikitError::blocked(
            blocked::UNREADABLE_FILE,
            format!("input file could not be read: {}", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| AikitError::other(format!("failed reading {}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_with_newline(path: &Path, body: &str) -> Result<(), AikitError> {
    let mut file = File::create(path)
        .map_err(|e| AikitError::other(format!("failed to write {}: {e}", path.display())))?;
    file.write_all(body.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|e| AikitError::other(format!("failed to write {}: {e}", path.display())))
}

fn format_ts(dt: OffsetDateTime, fmt: &[FormatItem<'static>]) -> String {
    dt.format(fmt).expect("static time format is always valid")
}

fn short_head(head: &str) -> String {
    if head.is_empty() {
        "nohead".to_string()
    } else if head.len() >= 7 {
        head[..7].to_string()
    } else {
        head.to_string()
    }
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
