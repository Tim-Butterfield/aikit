//! `aikit script run` / `aikit script check` — governed local script handling.
//!
//! This is a mechanical guard, NOT a security sandbox. Both subcommands share one
//! validation path: resolve the script under an allowed local work area
//! (canonicalized; symlink escapes rejected), detect the runner cross-OS (explicit
//! `--runner`, config extension map, `#!` shebang, built-in extension map, OS-aware
//! fallback — see `policy::script`), run a best-effort forbidden-operation scan, and
//! apply the clean-tree policy. `script run` then either prints the plan (`--print`)
//! or executes through the detected runner and records an audit trail (copied script,
//! stdout.txt, stderr.txt, run.json), propagating the script's exit code.
//! `script check` stops after validation and writes nothing — it only reports whether
//! the policy accepts the script.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::cli::{ScriptCheckArgs, ScriptRunArgs};
use crate::config;
use crate::errors::{blocked, AikitError};
use crate::formats::{ScriptCheck, ScriptRun, KIND_SCRIPT_CHECK, KIND_SCRIPT_RUN, SCHEMA_VERSION};
use crate::policy::script::{self as policy, Detection};
use crate::{output, repo};

const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
const ID_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

const STDOUT_NAME: &str = "stdout.txt";
const STDERR_NAME: &str = "stderr.txt";
const RUN_RECORD_NAME: &str = "run.json";

const SANDBOX_NOTE: &str =
    "aikit script run is NOT a security sandbox. The forbidden-operation scan is \
best-effort and easily bypassed; the allowed-location policy is the primary control. \
Running a script here does not make it safe.";

/// A script that passed path/location resolution: its aikit-root-relative path and real
/// canonical path. The runner is detected separately (it needs the script content).
struct Located {
    root: PathBuf,
    rel: String,
    real: PathBuf,
    /// VCS of the run root (`None` for a non-repo `.aikit/` folder). Drives the
    /// `--require-clean` check and the audit record's `vcs`/`git_head_*` fields.
    vcs: Option<repo::Vcs>,
}

/// Detect the aikit run root (filesystem-based; no `git`/`hg` subprocess), resolve +
/// canonicalize the script path, and reject root/allowlist escapes. Does not read
/// content or pick a runner.
fn resolve_and_locate(input: &str) -> Result<Located, AikitError> {
    let (root, vcs) = repo::detect_marker_root()?;
    let root_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let (rel, real) = resolve_script_path(&root_canon, input)?;
    Ok(Located {
        root,
        rel,
        real,
        vcs,
    })
}

/// Read the located script's bytes, mapping failure to `blocked_unreadable_file`.
fn read_script(located: &Located) -> Result<Vec<u8>, AikitError> {
    fs::read(&located.real).map_err(|_| {
        AikitError::blocked(
            blocked::UNREADABLE_FILE,
            format!("script could not be read: {}", located.rel),
        )
    })
}

/// Best-effort forbidden-operation scan plus the clean-tree policy (both applied after
/// runner detection so a `check` can still report the detected runner on a block).
fn check_forbidden_and_clean(
    content: &[u8],
    located: &Located,
    require_clean: bool,
) -> Result<(), AikitError> {
    let content_str = String::from_utf8_lossy(content);
    if let Some(pattern) = policy::scan_forbidden(&content_str) {
        return Err(AikitError::blocked(
            blocked::FORBIDDEN_OPERATION,
            format!(
                "script contains a forbidden operation (best-effort scan matched {pattern:?}); \
not a security check — refusing to run"
            ),
        ));
    }
    // Clean-tree policy. Default is allow-dirty; --require-clean blocks a dirty tracked
    // tree. (--require-clean / --allow-dirty are mutually exclusive in clap.) The dirty
    // check is VCS-specific and only runs when --require-clean is given — so a non-repo
    // run never touches a VCS, and `hg` is only invoked here for an hg root.
    if require_clean {
        let dirty = match located.vcs {
            Some(repo::Vcs::Git) => repo::git_tracked_tree_dirty(&located.root)?,
            Some(repo::Vcs::Mercurial) => repo::hg_tracked_tree_dirty(&located.root)?,
            None => {
                return Err(AikitError::blocked(
                    blocked::REQUIRE_CLEAN_UNSUPPORTED,
                    "--require-clean needs a Git or Mercurial working tree, but this is a \
non-repo aikit folder",
                ))
            }
        };
        if dirty {
            return Err(AikitError::blocked(
                blocked::DIRTY_TREE,
                "tracked working tree is dirty and --require-clean was given",
            ));
        }
    }
    Ok(())
}

/// The full argv to run a script: program, runner flags, then the script path.
fn build_argv(detection: &Detection, rel: &str) -> Vec<String> {
    let mut argv = Vec::with_capacity(detection.argv_flags.len() + 2);
    argv.push(detection.program.clone());
    argv.extend(detection.argv_flags.iter().cloned());
    argv.push(rel.to_string());
    argv
}

/// `aikit script run <script-path>` — validate, then print the plan (`--print`) or
/// execute and record an audit trail. The script's exit code is propagated.
pub fn run(args: ScriptRunArgs) -> Result<(), AikitError> {
    let located = resolve_and_locate(&args.script)?;
    let content = read_script(&located)?;
    let cfg = config::load(&located.root)?;
    let detection = policy::detect_runner(
        &located.rel,
        &String::from_utf8_lossy(&content),
        &cfg.script_runner,
        args.runner.as_deref(),
        !args.no_shebang,
    )?;
    check_forbidden_and_clean(&content, &located, args.require_clean)?;

    let Located {
        root,
        rel,
        real,
        vcs,
    } = located;
    let vcs_tag = vcs.map(|v| v.tag()).unwrap_or("none").to_string();

    let script_sha256 = sha256_bytes(&content);
    let ext = Path::new(&rel)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let script_copy_name = if ext.is_empty() {
        "script".to_string()
    } else {
        format!("script.{ext}")
    };
    let argv = build_argv(&detection, &rel);

    let now = OffsetDateTime::now_utc();
    // Head probe is git-only; hg/non-repo roots record an empty head (run_id falls back
    // to "nohead"). This keeps `script run` from spawning a VCS process off the git path.
    let head = match vcs {
        Some(repo::Vcs::Git) => repo::git_head(&root),
        _ => String::new(),
    };
    let run_id = format!("{}-{}", format_ts(now, ID_FORMAT), short_head(&head));
    let started_at = format_ts(now, TS_FORMAT);
    let cwd = root.display().to_string();

    // --print: validate (done above) and show the plan; do not execute. The planned
    // argv references the source script (no copy is made in print mode).
    if args.print {
        let record = ScriptRun {
            schema_version: SCHEMA_VERSION,
            kind: KIND_SCRIPT_RUN.to_string(),
            run_id,
            repo_root: cwd.clone(),
            vcs: vcs_tag.clone(),
            script_path: rel.clone(),
            script_sha256,
            script_copy_path: None,
            interpreter: detection.program.clone(),
            detected_runner: detection.runner.clone(),
            detection_source: detection.source.clone(),
            used_shebang: detection.used_shebang,
            used_extension_map: detection.used_extension_map,
            argv,
            cwd,
            require_clean: args.require_clean,
            allow_dirty: !args.require_clean,
            executed: false,
            started_at,
            finished_at: None,
            duration_ms: None,
            git_head_before: head,
            git_head_after: None,
            exit_code: None,
            blocked_state: None,
            stdout_path: None,
            stderr_path: None,
        };
        if args.json {
            print_json(&record)?;
        } else {
            println!("Would run (not executed; --print):");
            println!("  {}", record.argv.join(" "));
            println!(
                "  runner: {} (source: {})",
                record.detected_runner, record.detection_source
            );
            println!("  cwd: {}", record.cwd);
            println!(
                "  require_clean: {} (allow_dirty: {})",
                record.require_clean, record.allow_dirty
            );
            println!("  executed: false");
            println!("note: {SANDBOX_NOTE}");
        }
        return Ok(());
    }

    // Execute: create a unique run directory and copy the script (audit snapshot).
    let selected = output::select_output_root(&root, args.output.as_deref());
    let out_root = if selected.is_absolute() {
        selected
    } else {
        root.join(selected)
    };
    let (run_id, dir) = unique_run_dir(&output::runs_dir(&out_root), &run_id)?;

    let script_copy = dir.join(&script_copy_name);
    fs::copy(&real, &script_copy).map_err(|e| {
        AikitError::other(format!(
            "failed to copy script to {}: {e}",
            script_copy.display()
        ))
    })?;

    // Run the *original* script (in place) so `$0`/dirname-based companion-file
    // resolution works and the recorded argv matches exactly what ran. The copy in
    // the run directory is the immutable audit snapshot. stdin is /dev/null so an
    // accidental interactive prompt fails fast instead of hanging.
    let exec_start = OffsetDateTime::now_utc();
    let out = Command::new(&detection.program)
        .args(&detection.argv_flags)
        .arg(&rel)
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| AikitError::other(format!("failed to execute {}: {e}", detection.program)))?;
    let finished = OffsetDateTime::now_utc();
    let duration_ms = (finished - exec_start).whole_milliseconds().max(0) as u64;
    let exit_code = exit_code_from_status(&out.status);

    // Write captured output best-effort; the audit record (run.json) is written
    // regardless, so a failed log write never loses the fact that the run occurred.
    let stdout_path = dir.join(STDOUT_NAME);
    let stderr_path = dir.join(STDERR_NAME);
    let _ = fs::write(&stdout_path, &out.stdout);
    let _ = fs::write(&stderr_path, &out.stderr);

    // Head probe stays git-only (same gating as `head` before execution), so hg and
    // non-repo runs spawn no VCS subprocess here.
    let head_after = match vcs {
        Some(repo::Vcs::Git) => repo::git_head(&root),
        _ => String::new(),
    };
    let record = ScriptRun {
        schema_version: SCHEMA_VERSION,
        kind: KIND_SCRIPT_RUN.to_string(),
        run_id,
        repo_root: cwd.clone(),
        vcs: vcs_tag.clone(),
        script_path: rel.clone(),
        script_sha256,
        script_copy_path: Some(script_copy_name),
        interpreter: detection.program.clone(),
        detected_runner: detection.runner.clone(),
        detection_source: detection.source.clone(),
        used_shebang: detection.used_shebang,
        used_extension_map: detection.used_extension_map,
        argv,
        cwd,
        require_clean: args.require_clean,
        allow_dirty: !args.require_clean,
        executed: true,
        started_at,
        finished_at: Some(format_ts(finished, TS_FORMAT)),
        duration_ms: Some(duration_ms),
        git_head_before: head,
        git_head_after: Some(head_after),
        exit_code: Some(exit_code),
        blocked_state: None,
        stdout_path: Some(STDOUT_NAME.to_string()),
        stderr_path: Some(STDERR_NAME.to_string()),
    };

    let json = serde_json::to_string_pretty(&record)
        .map_err(|e| AikitError::other(format!("failed to serialize run record: {e}")))?;
    let record_path = dir.join(RUN_RECORD_NAME);
    write_with_newline(&record_path, &json)?;

    if args.json {
        let written = vec![
            display_relative(&root, &script_copy),
            display_relative(&root, &stdout_path),
            display_relative(&root, &stderr_path),
            display_relative(&root, &record_path),
        ];
        let mut value = serde_json::to_value(&record)
            .map_err(|e| AikitError::other(format!("failed to serialize run record: {e}")))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("written".to_string(), serde_json::json!(written));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&value)
                .map_err(|e| AikitError::other(format!("failed to serialize output: {e}")))?
        );
    } else {
        println!("Script run recorded:");
        println!("  {}", display_relative(&root, &record_path));
        println!("  {}", display_relative(&root, &stdout_path));
        println!("  {}", display_relative(&root, &stderr_path));
        println!("  {}", display_relative(&root, &script_copy));
        println!("  exit code: {exit_code}");
        println!("note: {SANDBOX_NOTE}");
    }

    // Propagate the script's exit code. Flush stdout first since process::exit skips
    // the normal runtime flush.
    std::io::stdout().flush().ok();
    std::process::exit(exit_code);
}

/// `aikit script check <script-path>` — apply the same run policy without executing.
/// Writes no run directory, copied script, stdout/stderr, or run.json. Exits 0 when
/// the policy accepts the script and 3 (with the blocked state) when it does not.
pub fn check(args: ScriptCheckArgs) -> Result<(), AikitError> {
    let require_clean = args.require_clean;

    // Tier 0: locate the script (repo, path, allowed location).
    let located = match resolve_and_locate(&args.script) {
        Ok(l) => l,
        Err(AikitError::Blocked { state, message }) => {
            return emit_blocked_check(
                None,
                args.script.clone(),
                None,
                None,
                None,
                require_clean,
                state,
                message,
                args.json,
            );
        }
        Err(other) => return Err(other),
    };
    let repo_root = Some(located.root.display().to_string());
    let resolved = Some(display_relative(&located.root, &located.real));

    // Read content (needed for shebang detection and the forbidden scan).
    let content = match read_script(&located) {
        Ok(c) => c,
        Err(AikitError::Blocked { state, message }) => {
            return emit_blocked_check(
                repo_root,
                located.rel.clone(),
                resolved,
                None,
                None,
                require_clean,
                state,
                message,
                args.json,
            );
        }
        Err(other) => return Err(other),
    };

    let cfg = config::load(&located.root)?;
    // Detect the runner.
    let detection = match policy::detect_runner(
        &located.rel,
        &String::from_utf8_lossy(&content),
        &cfg.script_runner,
        args.runner.as_deref(),
        !args.no_shebang,
    ) {
        Ok(d) => d,
        Err(AikitError::Blocked { state, message }) => {
            return emit_blocked_check(
                repo_root,
                located.rel.clone(),
                resolved,
                None,
                None,
                require_clean,
                state,
                message,
                args.json,
            );
        }
        Err(other) => return Err(other),
    };
    let argv = build_argv(&detection, &located.rel);

    // Forbidden scan + clean-tree (detection metadata is reported either way).
    match check_forbidden_and_clean(&content, &located, require_clean) {
        Ok(()) => {
            let record = build_check(
                repo_root,
                located.rel.clone(),
                resolved,
                Some(&detection),
                Some(argv),
                require_clean,
                true,
                None,
                None,
            );
            emit_check(&record, args.json);
            Ok(())
        }
        Err(AikitError::Blocked { state, message }) => emit_blocked_check(
            repo_root,
            located.rel.clone(),
            resolved,
            Some(&detection),
            Some(argv),
            require_clean,
            state,
            message,
            args.json,
        ),
        Err(other) => Err(other),
    }
}

/// Emit a blocked `script check` record and exit 3.
#[allow(clippy::too_many_arguments)]
fn emit_blocked_check(
    repo_root: Option<String>,
    script_path: String,
    resolved_script_path: Option<String>,
    detection: Option<&Detection>,
    argv: Option<Vec<String>>,
    require_clean: bool,
    state: &'static str,
    message: String,
    json: bool,
) -> Result<(), AikitError> {
    let record = build_check(
        repo_root,
        script_path,
        resolved_script_path,
        detection,
        argv,
        require_clean,
        false,
        Some(state.to_string()),
        Some(message),
    );
    emit_check(&record, json);
    std::io::stdout().flush().ok();
    std::process::exit(3);
}

#[allow(clippy::too_many_arguments)]
fn build_check(
    repo_root: Option<String>,
    script_path: String,
    resolved_script_path: Option<String>,
    detection: Option<&Detection>,
    argv: Option<Vec<String>>,
    require_clean: bool,
    accepted: bool,
    blocked_state: Option<String>,
    detail: Option<String>,
) -> ScriptCheck {
    ScriptCheck {
        schema_version: SCHEMA_VERSION,
        kind: KIND_SCRIPT_CHECK.to_string(),
        repo_root,
        script_path,
        resolved_script_path,
        interpreter: detection.map(|d| d.program.clone()),
        detected_runner: detection.map(|d| d.runner.clone()),
        detection_source: detection.map(|d| d.source.clone()),
        used_shebang: detection.map(|d| d.used_shebang).unwrap_or(false),
        used_extension_map: detection.map(|d| d.used_extension_map).unwrap_or(false),
        argv,
        require_clean,
        allow_dirty: !require_clean,
        executed: false,
        output_created: false,
        accepted,
        blocked_state,
        detail,
    }
}

/// Print a check record as JSON or human-readable text.
fn emit_check(record: &ScriptCheck, json: bool) {
    if json {
        match serde_json::to_string_pretty(record) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("error: failed to serialize check record: {e}"),
        }
        return;
    }
    if record.accepted {
        println!("Script check: ACCEPTED (not executed)");
    } else {
        println!("Script check: BLOCKED (not executed)");
    }
    println!("  script: {}", record.script_path);
    if let Some(resolved) = &record.resolved_script_path {
        println!("  resolved: {resolved}");
    }
    match (&record.detected_runner, &record.interpreter) {
        (Some(runner), Some(interp)) => {
            let source = record.detection_source.as_deref().unwrap_or("?");
            println!("  runner: {runner} ({interp}) [source: {source}]");
        }
        _ => println!("  runner: (not resolved)"),
    }
    if let Some(argv) = &record.argv {
        println!("  argv: {}", argv.join(" "));
    }
    println!(
        "  require_clean: {} (allow_dirty: {})",
        record.require_clean, record.allow_dirty
    );
    if record.accepted {
        println!("  forbidden-operation scan: passed");
    } else if let Some(state) = &record.blocked_state {
        println!("  blocked_state: {state}");
        if let Some(detail) = &record.detail {
            println!("  detail: {detail}");
        }
    }
    println!("  no run output created");
    println!("note: {SANDBOX_NOTE}");
}

/// Resolve a script path to `(repo-relative, real absolute)`, rejecting missing
/// scripts, directories, symlink escapes, and paths outside the allowed work areas.
fn resolve_script_path(root_canon: &Path, input: &str) -> Result<(String, PathBuf), AikitError> {
    let raw = PathBuf::from(input);
    let candidate = if raw.is_absolute() {
        raw
    } else {
        root_canon.join(&raw)
    };
    // Canonicalize resolves `..` and symlinks; failure means missing/unreadable.
    let real = fs::canonicalize(&candidate).map_err(|_| {
        AikitError::blocked(
            blocked::UNREADABLE_FILE,
            format!("script not found or unreadable: {input}"),
        )
    })?;
    let rel = real.strip_prefix(root_canon).map_err(|_| {
        AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("script path resolves outside the repository: {input}"),
        )
    })?;
    if !real.is_file() {
        return Err(AikitError::blocked(
            blocked::UNREADABLE_FILE,
            format!("script path is not a regular file: {input}"),
        ));
    }
    let rel = rel.to_string_lossy().replace('\\', "/");
    if !policy::is_in_allowed_location(&rel) {
        return Err(AikitError::blocked(
            blocked::SCRIPT_NOT_ALLOWED,
            format!(
                "script {rel:?} is outside the allowed locations ({})",
                policy::ALLOWED_SCRIPT_DIRS.join(", ")
            ),
        ));
    }
    Ok((rel, real))
}

fn print_json(record: &ScriptRun) -> Result<(), AikitError> {
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| AikitError::other(format!("failed to serialize run record: {e}")))?;
    println!("{json}");
    Ok(())
}

/// Create a unique run directory under `runs`, returning `(run_id, dir)`. If the
/// base id (second-resolution timestamp + short HEAD) already exists, a numeric
/// suffix is appended so concurrent runs in the same second never overwrite an
/// existing audit trail.
fn unique_run_dir(runs: &Path, base_id: &str) -> Result<(String, PathBuf), AikitError> {
    fs::create_dir_all(runs).map_err(|e| {
        AikitError::other(format!(
            "failed to create output dir {}: {e}",
            runs.display()
        ))
    })?;
    for n in 0..10_000 {
        let id = if n == 0 {
            base_id.to_string()
        } else {
            format!("{base_id}-{n}")
        };
        let dir = runs.join(&id);
        match fs::create_dir(&dir) {
            Ok(()) => return Ok((id, dir)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(AikitError::other(format!(
                    "failed to create output dir {}: {e}",
                    dir.display()
                )))
            }
        }
    }
    Err(AikitError::other(
        "could not allocate a unique run directory".to_string(),
    ))
}

/// Process exit code from a child status. On Unix, a signal-killed child (no exit
/// code) maps to `128 + signal` per shell convention rather than masking as 1.
fn exit_code_from_status(status: &std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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
