//! Optional repository configuration for bundle generation and anchor discovery.
//!
//! Configuration is layered with a fixed precedence (lowest to highest):
//!   1. built-in defaults (this module)
//!   2. `aikit.config.json` at the repo root, if present
//!   3. `.aikit/config.json`, if present
//!   4. CLI flags (applied by the caller via [`ResolvedConfig::apply_overrides`])
//!
//! The protective exclude globs are *always* applied and cannot be removed by a
//! config file; a config file's `exclude_globs` are appended to them. This keeps
//! large/sensitive paths (`.git/`, `target/`, `node_modules/`, provider/secret
//! output areas, …) out of bundles regardless of configuration.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::errors::AikitError;

/// Protective exclude globs. Always applied to anchor discovery in addition to any
/// `exclude_globs` from a config file; a config file can add to this list but never
/// remove an entry. Patterns use `**` to match across directories.
///
/// Common large/dependency directories are matched by name *anywhere* in the tree
/// (`**/<name>/**`), not only at the repo root, so a nested `pkg/node_modules/` or
/// `crates/foo/target/` is protected too. aikit's own provider/raw/secret output areas
/// are matched root-relative because they only exist at the repo root.
pub const DEFAULT_EXCLUDE_GLOBS: &[&str] = &[
    "**/.git/**",
    ".aikit/outputs/raw/**",
    ".aikit/outputs/provider/**",
    ".aikit/outputs/secrets/**",
    "**/node_modules/**",
    "**/target/**",
    "**/dist/**",
    "**/build/**",
];

/// The fully resolved configuration after layering config files (CLI flags are then
/// applied with [`ResolvedConfig::apply_overrides`]).
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Write exactly one bundle file (no review directory, no sidecar manifest).
    pub single_file: bool,
    /// Embed the manifest inside the bundle text.
    pub embed_manifest: bool,
    /// Write a sidecar `manifest.json` (directory mode only; ignored for single-file).
    pub sidecar_manifest: bool,
    /// Enable enhanced anchor discovery (untracked non-ignored files, allowlisted
    /// ignored files modified after the anchor, and recorded tracked deletions).
    pub include_ignored_batch_files: bool,
    /// Output override: a directory root (directory mode) or a file path (single-file).
    pub output: Option<String>,
    /// Allowlist globs for otherwise-ignored files to pull into anchor discovery.
    pub include_globs: Vec<String>,
    /// User-supplied exclude globs (appended to [`DEFAULT_EXCLUDE_GLOBS`]).
    pub user_exclude_globs: Vec<String>,
    /// Explicit repo-relative files always included in anchor discovery when present.
    pub include_files: Vec<String>,
    /// Script-runner detection preferences.
    pub script_runner: ScriptRunnerConfig,
    /// Repo-relative paths of the config files that were loaded, in precedence order.
    pub sources: Vec<String>,
}

/// Script-runner detection preferences. An empty `extension_map` means the built-in
/// extension map (in `policy::script`) is used; a present key overrides it for that
/// extension. `preferred_runners` is a global priority order applied when more than one
/// candidate runner could satisfy an extension.
#[derive(Debug, Clone)]
pub struct ScriptRunnerConfig {
    pub preferred_runners: Vec<String>,
    pub detect_from_shebang: bool,
    pub detect_from_extension: bool,
    /// Extension (with leading dot, lowercased) → ordered candidate runner names.
    pub extension_map: BTreeMap<String, Vec<String>>,
}

impl Default for ScriptRunnerConfig {
    fn default() -> Self {
        ScriptRunnerConfig {
            preferred_runners: Vec::new(),
            detect_from_shebang: true,
            detect_from_extension: true,
            extension_map: BTreeMap::new(),
        }
    }
}

/// Directory-pruning rules derived from the effective exclude globs, used to avoid
/// descending into large/sensitive trees during the ignored-file walk.
#[derive(Debug, Default, Clone)]
pub struct ExcludeDirRules {
    /// Root-relative directory paths to prune (e.g. `.aikit/outputs/raw`).
    pub prefixes: Vec<String>,
    /// Directory base names to prune anywhere in the tree (e.g. `node_modules`).
    pub basenames: Vec<String>,
}

impl ExcludeDirRules {
    /// Whether a repo-relative directory path should be pruned.
    pub fn prunes(&self, rel_dir: &str) -> bool {
        let base = rel_dir.rsplit('/').next().unwrap_or(rel_dir);
        if self.basenames.iter().any(|b| b == base) {
            return true;
        }
        self.prefixes
            .iter()
            .any(|p| rel_dir == p || rel_dir.starts_with(&format!("{p}/")))
    }
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        ResolvedConfig {
            single_file: false,
            embed_manifest: false,
            sidecar_manifest: true,
            include_ignored_batch_files: false,
            output: None,
            include_globs: Vec::new(),
            user_exclude_globs: Vec::new(),
            include_files: Vec::new(),
            script_runner: ScriptRunnerConfig::default(),
            sources: Vec::new(),
        }
    }
}

impl ResolvedConfig {
    /// Effective exclude globs: the protective defaults followed by any user excludes.
    pub fn effective_exclude_globs(&self) -> Vec<String> {
        let mut v: Vec<String> = DEFAULT_EXCLUDE_GLOBS
            .iter()
            .map(|s| s.to_string())
            .collect();
        v.extend(self.user_exclude_globs.iter().cloned());
        v
    }

    /// Compiled allowlist glob set (empty set matches nothing).
    pub fn include_globset(&self) -> Result<GlobSet, AikitError> {
        build_globset(&self.include_globs)
    }

    /// Compiled exclude glob set (protective defaults plus user excludes).
    pub fn exclude_globset(&self) -> Result<GlobSet, AikitError> {
        build_globset(&self.effective_exclude_globs())
    }

    /// Directory-pruning rules derived from exclude globs of the form `<dir>/**` (a
    /// root-relative prefix) or `**/<name>/**` (a directory base name matched anywhere).
    /// Pruning these never descends into large/sensitive trees such as `target/` or a
    /// nested `pkg/node_modules/`.
    pub fn exclude_dir_rules(&self) -> ExcludeDirRules {
        let mut rules = ExcludeDirRules::default();
        for g in self.effective_exclude_globs() {
            let Some(p) = g.strip_suffix("/**") else {
                continue;
            };
            if let Some(name) = p.strip_prefix("**/") {
                if !name.is_empty() && !name.contains('*') && !name.contains('/') {
                    rules.basenames.push(name.to_string());
                }
            } else if !p.is_empty() && !p.contains('*') {
                rules.prefixes.push(p.to_string());
            }
        }
        rules
    }

    /// Apply CLI flag overrides (highest precedence). Boolean flags can only turn a
    /// feature on; `no_sidecar` turns the sidecar manifest off. Single-file mode forces
    /// an embedded manifest and disables the sidecar (a single file cannot also write a
    /// sibling manifest).
    pub fn apply_overrides(
        &mut self,
        single_file: bool,
        embed_manifest: bool,
        no_sidecar: bool,
        include_ignored: bool,
        output: Option<&str>,
    ) {
        self.single_file = self.single_file || single_file;
        self.embed_manifest = self.embed_manifest || embed_manifest;
        if no_sidecar {
            self.sidecar_manifest = false;
        }
        self.include_ignored_batch_files = self.include_ignored_batch_files || include_ignored;
        if let Some(o) = output {
            self.output = Some(o.to_string());
        }
        if self.single_file {
            self.embed_manifest = true;
            self.sidecar_manifest = false;
        }
    }

    fn merge_raw(&mut self, raw: RawConfig) {
        if let Some(bundle) = raw.bundle {
            if let Some(v) = bundle.single_file {
                self.single_file = v;
            }
            if let Some(v) = bundle.embed_manifest {
                self.embed_manifest = v;
            }
            if let Some(v) = bundle.sidecar_manifest {
                self.sidecar_manifest = v;
            }
            if let Some(v) = bundle.output {
                self.output = Some(v);
            }
        }
        if let Some(d) = raw.discovery {
            if let Some(v) = d.include_ignored_batch_files {
                self.include_ignored_batch_files = v;
            }
            if let Some(v) = d.include_globs {
                self.include_globs = v;
            }
            if let Some(v) = d.exclude_globs {
                self.user_exclude_globs = v;
            }
            if let Some(v) = d.include_files {
                self.include_files = v;
            }
        }
        if let Some(s) = raw.script_runner {
            if let Some(v) = s.preferred_runners {
                self.script_runner.preferred_runners = v;
            }
            if let Some(v) = s.detect_from_shebang {
                self.script_runner.detect_from_shebang = v;
            }
            if let Some(v) = s.detect_from_extension {
                self.script_runner.detect_from_extension = v;
            }
            if let Some(map) = s.extension_map {
                // Normalize keys to a leading-dot, lowercased form.
                let mut normalized = BTreeMap::new();
                for (k, v) in map {
                    let key = if k.starts_with('.') {
                        k.to_lowercase()
                    } else {
                        format!(".{}", k.to_lowercase())
                    };
                    normalized.insert(key, v);
                }
                self.script_runner.extension_map = normalized;
            }
        }
    }
}

/// Load and layer config files under `repo_root`. Missing files are skipped; a present
/// file that is malformed (bad JSON or an unknown field) fails clearly.
pub fn load(repo_root: &Path) -> Result<ResolvedConfig, AikitError> {
    let mut cfg = ResolvedConfig::default();
    for rel in ["aikit.config.json", ".aikit/config.json"] {
        let path = repo_root.join(rel);
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|e| AikitError::other(format!("failed to read config {rel}: {e}")))?;
        let raw: RawConfig = serde_json::from_str(&text)
            .map_err(|e| AikitError::other(format!("invalid config {rel}: {e}")))?;
        cfg.merge_raw(raw);
        cfg.sources.push(rel.to_string());
    }
    Ok(cfg)
}

/// Build a glob set with gitignore-like semantics: `*` does not cross `/`, `**` does.
pub fn build_globset(patterns: &[String]) -> Result<GlobSet, AikitError> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        let glob = GlobBuilder::new(p)
            .literal_separator(true)
            .build()
            .map_err(|e| AikitError::other(format!("invalid glob {p:?}: {e}")))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|e| AikitError::other(format!("failed to build glob set: {e}")))
}

/// Partial config as read from a JSON file. Every field is optional so a config file
/// can set only what it needs; unknown fields are rejected to catch typos. A `_comment`
/// key is accepted and ignored everywhere (JSON has no native comments), so the annotated
/// example config can be copied verbatim.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default, rename = "_comment")]
    #[allow(dead_code)]
    _comment: Option<serde_json::Value>,
    #[serde(default)]
    bundle: Option<RawBundle>,
    #[serde(default)]
    discovery: Option<RawDiscovery>,
    #[serde(default)]
    script_runner: Option<RawScriptRunner>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScriptRunner {
    #[serde(default, rename = "_comment")]
    #[allow(dead_code)]
    _comment: Option<serde_json::Value>,
    preferred_runners: Option<Vec<String>>,
    detect_from_shebang: Option<bool>,
    detect_from_extension: Option<bool>,
    extension_map: Option<BTreeMap<String, Vec<String>>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBundle {
    #[serde(default, rename = "_comment")]
    #[allow(dead_code)]
    _comment: Option<serde_json::Value>,
    single_file: Option<bool>,
    embed_manifest: Option<bool>,
    sidecar_manifest: Option<bool>,
    output: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDiscovery {
    #[serde(default, rename = "_comment")]
    #[allow(dead_code)]
    _comment: Option<serde_json::Value>,
    include_ignored_batch_files: Option<bool>,
    include_globs: Option<Vec<String>>,
    exclude_globs: Option<Vec<String>>,
    include_files: Option<Vec<String>>,
}
