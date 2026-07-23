//! `flare_docs` MCP tool handler body.

use super::*;
// `pub(crate)` (not a plain `use`): the local `mod flare_docs` declared in
// `mcp_server.rs` shadows the `flare_docs` extern crate for any unqualified
// `flare_docs::...` path written in that parent file (extern-prelude entries
// are shadowed by same-named local items). `mcp_server.rs`'s struct field and
// `ensure_flare_docs_store`/`with_flare_docs_store` helpers reference
// `flare_docs::DocsStore` expecting the crate type, so it must be
// re-exported (not just privately imported) through this submodule for that
// path to resolve.
pub(crate) use ::flare_docs::{fetch_and_store, DocsStore, UreqFetcher};

const DEFAULT_LIMIT: usize = 10;
const DEFAULT_VERSION: &str = "latest";

impl AgentflareMcp {
    pub fn flare_docs_impl(&self, req: FlareDocsRequest) -> Result<String, ErrorData> {
        self.with_flare_docs_store(|store| match req.action.as_str() {
            "search" => {
                let query = req
                    .query
                    .ok_or_else(|| ErrorData::invalid_params("search requires \"query\"", None))?;
                let limit = req.limit.unwrap_or(DEFAULT_LIMIT);
                let hits = store
                    .search(&query, limit)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                serde_json::to_string(&hits)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            "list" => {
                let docs = store
                    .list()
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                serde_json::to_string(&docs)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            "get" => {
                if let Some(id) = &req.id {
                    let doc = store
                        .get(id)
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                    return serde_json::to_string(&doc)
                        .map_err(|e| ErrorData::internal_error(e.to_string(), None));
                }
                let package = req
                    .package
                    .as_deref()
                    .ok_or_else(|| ErrorData::invalid_params("get requires \"id\" or \"package\"", None))?;
                let version = req.version.as_deref().unwrap_or(DEFAULT_VERSION);
                let fetcher = UreqFetcher::new();
                let doc = fetch_and_store(&fetcher, store, package, version)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                serde_json::to_string(&doc).map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            "refresh" => {
                let package = req
                    .package
                    .ok_or_else(|| ErrorData::invalid_params("refresh requires \"package\"", None))?;
                let version = req.version.as_deref().unwrap_or(DEFAULT_VERSION);
                let fetcher = UreqFetcher::new();
                let doc = fetch_and_store(&fetcher, store, &package, version)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                serde_json::to_string(&doc).map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            other => Err(ErrorData::invalid_params(
                format!("unknown action \"{other}\" (expected search|get|list|refresh)"),
                None,
            )),
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mcp() -> AgentflareMcp {
        AgentflareMcp {
            flare_docs_store_override: Some(std::path::PathBuf::from(":memory:")),
            ..Default::default()
        }
    }

    #[test]
    fn list_on_empty_store_returns_empty_array() {
        let mcp = test_mcp();
        let req = FlareDocsRequest {
            action: "list".to_string(),
            ..Default::default()
        };
        let result = mcp.flare_docs_impl(req).unwrap();
        assert_eq!(result, "[]");
    }

    #[test]
    fn unknown_action_is_rejected() {
        let mcp = test_mcp();
        let req = FlareDocsRequest {
            action: "bogus".to_string(),
            ..Default::default()
        };
        let result = mcp.flare_docs_impl(req);
        assert!(result.is_err());
    }
}
