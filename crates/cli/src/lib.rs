//! Concrete `AikenRunner` impl invoking the `aiken` CLI as a subprocess.
//!
//! The runner is intentionally thin: it shells out, captures stdout/stderr,
//! and parses output via `parse` submodule. Output formats may evolve across
//! Aiken versions; keep parsing logic isolated so adapter shims live in one
//! place.

mod parse;
mod process;

pub use parse::{parse_check, parse_test};
pub use process::AikenCliRunner;
