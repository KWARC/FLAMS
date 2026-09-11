#![recursion_limit = "256"]

pub mod components;
pub mod vscode;

use flams_backend_types::search::{FragmentQueryFilter, SearchResult};
use ftml_uris::DocumentElementUri;
use ftml_uris::SymbolUri;
use leptos::prelude::*;

#[server(prefix = "/api", endpoint = "search")]
#[allow(clippy::unused_async)]
pub async fn search_query(
    query: String,
    opts: FragmentQueryFilter,
    num_results: usize,
) -> Result<Vec<(f32, SearchResult)>, ServerFnError<String>> {
    use flams_math_archives::backend::LocalBackend;
    use flams_search::Searcher;
    tokio::task::spawn_blocking(move || {
        // throws errors if I label it mut in the signature, for some reason
        let mut opts = opts;
        opts.close(|u| flams_system::backend::backend().get_document(u).ok());
        Searcher::get()
            .query(&query, opts, num_results)
            .map_err(|e| ServerFnError::ServerError(format!("Search error: {e}")))
    })
    .await
    .map_err(|e| ServerFnError::ServerError(e.to_string()))?
}

#[server(prefix = "/api", endpoint = "search_symbols")]
#[allow(clippy::unused_async)]
pub async fn search_symbols(
    query: String,
    num_results: usize,
) -> Result<Vec<(f32, SymbolUri, DocumentElementUri)>, ServerFnError<String>> {
    use flams_search::Searcher;
    tokio::task::spawn_blocking(move || {
        Searcher::get()
            .query_symbols(&query, num_results)
            .unwrap_or_default()
    })
    .await
    .map_err(|e| ServerFnError::ServerError(e.to_string()))
}
