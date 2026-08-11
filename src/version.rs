//! `aikit version` — report the package/binary version and build metadata.
//!
//! The package version (`CARGO_PKG_VERSION`) is the same string clap prints for
//! `aikit --version`. It is distinct from the per-record `schema_version` used by other
//! artifacts. `git_commit`, `build_profile`, and `target` come from `build.rs` and are
//! best-effort (empty -> reported as `null`). Read-only; creates nothing.

use crate::cli::VersionArgs;
use crate::errors::AikitError;
use crate::formats::{VersionInfo, KIND_VERSION, SCHEMA_VERSION};

/// Build the version record from compile-time metadata.
pub fn info() -> VersionInfo {
    VersionInfo {
        schema_version: SCHEMA_VERSION,
        kind: KIND_VERSION.to_string(),
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit: non_empty(option_env!("AIKIT_GIT_COMMIT")),
        build_profile: non_empty(option_env!("AIKIT_BUILD_PROFILE")),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        target: non_empty(option_env!("AIKIT_TARGET")),
        // Reserved for a future toolchain/profile string.
        rust_profile: None,
    }
}

pub fn version(args: VersionArgs) -> Result<(), AikitError> {
    let record = info();
    if args.json {
        let json = serde_json::to_string_pretty(&record)
            .map_err(|e| AikitError::other(format!("failed to serialize version: {e}")))?;
        println!("{json}");
    } else {
        println!("{} {}", record.name, record.version);
        if let Some(commit) = &record.git_commit {
            println!("  git commit: {commit}");
        }
        if let Some(profile) = &record.build_profile {
            println!("  build profile: {profile}");
        }
        println!("  os/arch: {}/{}", record.os, record.arch);
        if let Some(target) = &record.target {
            println!("  target: {target}");
        }
    }
    Ok(())
}

/// Treat an absent or empty compile-time env value as `None`.
fn non_empty(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}
