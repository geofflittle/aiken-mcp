//! Tool handlers. Each handler is pure logic over `core` traits — no MCP
//! transport knowledge. The server crate adapts these into rmcp Tool
//! registrations.
//!
//! Adding a new tool: define request/response types, write a handler fn that
//! takes the trait dependencies it needs, register it in the server crate.

pub mod blueprint;
pub mod budget;
pub mod build;
pub mod check;
pub mod completions;
pub mod corpus_list;
pub mod definition;
pub mod docs_lookup;
pub mod explain;
pub mod fmt;
pub mod hover;
pub mod new_project;
pub mod search;
pub mod symbol_lookup;
pub mod test;
pub mod uplc;
pub mod version;

pub use blueprint::*;
pub use budget::*;
pub use build::*;
pub use check::*;
pub use completions::*;
pub use corpus_list::*;
pub use definition::*;
pub use docs_lookup::*;
pub use explain::*;
pub use fmt::*;
pub use hover::*;
pub use new_project::*;
pub use search::*;
pub use symbol_lookup::*;
pub use test::*;
pub use uplc::*;
pub use version::*;
