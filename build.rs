//! Build script: capture low-risk build-time metadata for `aikit version`.
//!
//! Best-effort only. The short git commit is read from the source tree at build time
//! (empty when git is unavailable or this is not a checkout). The build profile and
//! target triple come from Cargo's own environment. Nothing here fails the build.

use std::process::Command;

fn main() {
    // Short git commit (best-effort; empty string when unavailable -> reported as null).
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=AIKIT_GIT_COMMIT={commit}");

    // Cargo exposes these to build scripts; pass them through as compile-time env.
    let profile = std::env::var("PROFILE").unwrap_or_default();
    println!("cargo:rustc-env=AIKIT_BUILD_PROFILE={profile}");
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=AIKIT_TARGET={target}");

    // Re-run when the commit could have changed or this script is edited. `.git/HEAD`
    // alone is insufficient on a normal branch: it holds `ref: refs/heads/<branch>`,
    // which does not change as the branch advances. Also watch the branch ref file (and
    // packed-refs as a best-effort fallback) so the captured commit stays fresh.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        if let Some(ref_path) = head.trim().strip_prefix("ref: ") {
            // e.g. "ref: refs/heads/main" -> ".git/refs/heads/main".
            println!("cargo:rerun-if-changed=.git/{ref_path}");
        }
    }
    // Packed refs cover the case where the loose ref file does not exist (best-effort;
    // a detached HEAD or a linked worktree's gitdir may still go untracked).
    println!("cargo:rerun-if-changed=.git/packed-refs");
}
