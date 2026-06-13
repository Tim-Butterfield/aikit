//! Deterministic local policy rules for the governed script runner.
//!
//! These are mechanical guards (allowed locations, interpreter selection, a
//! best-effort forbidden-operation scan) — **not** a security sandbox. The
//! allowed-location policy is the primary control; the static scan only catches
//! obvious accidental mistakes and is trivially bypassable.

pub mod script;
