//! `aikit mcp` — an MCP server exposing the script runner to an agent.
//!
//! This is a deliberately different contract from `aikit script run`:
//!
//! | | CLI | MCP |
//! |---|---|---|
//! | Footprint | `.aikit/` required, run record persisted | none retained |
//! | Repo | required | irrelevant |
//! | Dirty check | enforced | not applicable |
//! | Runner | inferred from extension/shebang/config | **required**, never inferred |
//! | Config | consulted | never read |
//!
//! The MCP path exists to remove the script *file write* — the friction no CLI flag can
//! take away, because writing a file is itself an approval step for an agent.
//!
//! Server state is per-request only: a concurrency semaphore, a request-id → job registry
//! and per-instance temp directories. Nothing a client can address survives a `tools/call`,
//! so a stateless transport remains possible later.

mod envmap;
mod exec;
mod platform;
mod server;
mod tools;
mod workdir;

use crate::cli::McpServeArgs;
use crate::errors::AikitError;

/// The agent guide, resolved from the embedded text or the `AGENTS_MD` override.
///
/// Re-exported so `aikit agents-md` and the `agents_md` tool serve the same bytes from one
/// implementation — two copies of this resolution could disagree about the override.
pub use server::resolve_agents_md;

/// Run the MCP server on stdio until the client disconnects.
pub fn serve(args: McpServeArgs) -> Result<(), AikitError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| AikitError::other(format!("failed to start the async runtime: {e}")))?;

    runtime.block_on(server::run(args))
}
