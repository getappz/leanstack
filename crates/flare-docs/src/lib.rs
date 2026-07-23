pub mod store;

pub use agentflare_store::documents::{DocMatch, Document, DocUpsertOpts};
pub use store::{DocsStore, Error, PROJECT_ID};
