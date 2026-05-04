//! Tool handlers. Each handler is pure logic over `core` traits — no MCP
//! transport knowledge. The server crate adapts these into rmcp Tool
//! registrations.
//!
//! Adding a new tool: define request/response types, write a handler fn that
//! takes the trait dependencies it needs, register it in the server crate.

pub mod check;
pub mod build;
pub mod test;
pub mod fmt;
pub mod search;
pub mod docs_lookup;
pub mod version;

pub use check::*;
pub use build::*;
pub use test::*;
pub use fmt::*;
pub use search::*;
pub use docs_lookup::*;
pub use version::*;
