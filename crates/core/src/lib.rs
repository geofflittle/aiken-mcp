pub mod error;
pub mod project;
pub mod runner;
pub mod corpus;
pub mod docs;
pub mod diagnostic;

pub use error::{CoreError, CoreResult};
pub use project::{Project, ProjectRoot};
pub use runner::{AikenRunner, CheckOutcome, BuildOutcome, TestOutcome, FmtOutcome};
pub use corpus::{CorpusSearch, CorpusHit};
pub use docs::DocsFetcher;
pub use diagnostic::{Diagnostic, Severity, SourceSpan};
