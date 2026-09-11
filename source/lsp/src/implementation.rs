#![allow(clippy::cognitive_complexity)]

use std::{
    io::Write,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use crate::{
    ClientExt, NewArchiveParams, ProgressCallbackServer, StandaloneExportParams, UriParams,
    annotations::{into_diagnostic, to_diagnostic},
    documents::LSPDocument,
    state::{LSPState, UrlOrFile},
};

use super::{FLAMSLSPServer, ServerWrapper};
use async_lsp::{
    ClientSocket, LanguageClient, LanguageServer, ResponseError,
    lsp_types::{self as lsp},
};
use flams_math_archives::{
    MathArchive,
    backend::{GlobalBackend, LocalBackend},
    formats::FormatOrTargets,
    utils::path_ext::RelPath,
};
use flams_stex::quickparse::stex::{AnnotIter, STeXAnnot, STeXDiagnostic};
use flams_system::{TokioEngine, backend::backend};
use flams_utils::{
    parsing::{Needle, SourceParser, StrParser},
    prelude::TreeChildIter,
    sourcerefs::{ByteOffset, LSPLineCol, PositionConverter, StringPosition, StringRange},
    unwrap,
};
use ftml_ontology::{
    narrative::{
        DataRef,
        elements::{
            DocumentElementRef,
            problems::{GradingNote, Solutions},
        },
    },
    utils::{Css, RefTree},
};
use ftml_uris::{DocumentUri, IsDomainUri, IsNarrativeUri, SymbolUri, UriWithArchive, UriWithPath};
use futures::{FutureExt, TryFutureExt, future::BoxFuture};

macro_rules! impl_request {
    ($name:ident = $struct:ident) => {
        fn $name(
            &mut self,
            params: <lsp::request::$struct as lsp::request::Request>::Params,
        ) -> Res<<lsp::request::$struct as lsp::request::Request>::Result> {
            tracing::info!("LSP: {params:?}");
            Box::pin(std::future::ready(Err(ResponseError::new(
                async_lsp::ErrorCode::METHOD_NOT_FOUND,
                format!(
                    "No such method: {}",
                    <lsp::request::$struct as lsp::request::Request>::METHOD
                ),
            ))))
        }
    };
    (? $name:ident = $struct:ident => ($default:expr)) => {
        fn $name(
            &mut self,
            params: <lsp::request::$struct as lsp::request::Request>::Params,
        ) -> Res<<lsp::request::$struct as lsp::request::Request>::Result> {
            tracing::info!("LSP: {params:?}");
            Box::pin(std::future::ready(Ok($default)))
        }
    };
    (! $name:ident = $struct:ident => ($default:expr)) => {
        fn $name(
            &mut self,
            params: <lsp::request::$struct as lsp::request::Request>::Params,
        ) -> Res<<lsp::request::$struct as lsp::request::Request>::Result> {
            tracing::debug!("LSP: {params:?}");
            Box::pin(std::future::ready(Ok($default)))
        }
    };
}

macro_rules! impl_notification {
    (! $name:ident = $struct:ident) => {
        fn $name(
            &mut self,
            params: <lsp::notification::$struct as lsp::notification::Notification>::Params,
        ) -> Self::NotifyResult {
            tracing::debug!("LSP: {params:?}");
            ControlFlow::Continue(())
        }
    };
    ($name:ident = $struct:ident) => {
        fn $name(
            &mut self,
            params: <lsp::notification::$struct as lsp::notification::Notification>::Params,
        ) -> Self::NotifyResult {
            tracing::info!("LSP: {params:?}");
            ControlFlow::Break(Err(async_lsp::Error::Routing(format!(
                "Unhandled notification: {}",
                <lsp::notification::$struct as lsp::notification::Notification>::METHOD,
            ))))
        }
    };
}

#[inline]
fn fut<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Res<T> {
    Box::pin(tokio::task::spawn_blocking(f).map_ok_or_else(
        |e| {
            Err(ResponseError::new(
                async_lsp::ErrorCode::REQUEST_FAILED,
                e.to_string(),
            ))
        },
        |r| r.map_err(|e| ResponseError::new(async_lsp::ErrorCode::REQUEST_FAILED, e)),
    ))
}
fn block<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl std::future::Future<Output = Result<T, String>> {
    tokio::task::spawn_blocking(f).map_ok_or_else(|e| Err(e.to_string()), |r| r)
}
fn wrap_fut<T: Send + 'static>(
    f: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
) -> Res<T> {
    Box::pin(f.map_err(|e| ResponseError::new(async_lsp::ErrorCode::REQUEST_FAILED, e)))
}

impl<T: FLAMSLSPServer> ServerWrapper<T> {
    pub(crate) fn html_request(&mut self, params: UriParams) -> Res<Option<String>> {
        let mut client = self.inner.client().clone();
        let state = self.inner.state().clone();
        Box::pin(
            tokio::task::spawn_blocking(move || {
                state
                    .build_html(&params.uri.into(), &mut client)
                    .map(|d| d.to_string())
            })
            .map_err(|e| ResponseError::new(async_lsp::ErrorCode::REQUEST_FAILED, e.to_string())),
        )
    }

    pub(crate) fn new_archive(
        &mut self,
        NewArchiveParams { archive, urlbase }: NewArchiveParams,
    ) -> <Self as LanguageServer>::NotifyResult {
        let mut client = self.inner.client().clone();
        tokio::task::spawn_blocking(move || {
            match GlobalBackend.new_archive(
                &archive,
                &urlbase,
                flams_stex::STEX.id(),
                "helloworld.tex",
                include_str!("stex_default.txt"),
            ) {
                Ok(mut path) => {
                    let _ = client.show_message(lsp::ShowMessageParams {
                        typ: lsp::MessageType::INFO,
                        message: format!("Created new archive {archive}"),
                    });
                    path.push("source");
                    path.push("helloworld.tex");
                    client.open_file(&path);
                }
                Err(e) => {
                    let _ = client.show_message(lsp::ShowMessageParams {
                        typ: lsp::MessageType::ERROR,
                        message: format!("Error creating new archive {archive}: {e:#}"),
                    });
                }
            }
        });
        ControlFlow::Continue(())
    }

    pub(crate) fn export_html(
        &mut self,
        params: StandaloneExportParams,
    ) -> <Self as LanguageServer>::NotifyResult {
        let StandaloneExportParams { uri, target } = params;
        let uri: UrlOrFile = uri.into();
        let state = self.inner.state().clone();
        let mut client = self.inner.client().clone();
        tokio::task::spawn_blocking(move || {
            let Some(doc) = state.get(&uri) else {
                let _ = client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: format!("Not a valid file path: {uri}"),
                });
                return;
            };
            let Some(doc_uri) = doc.document_uri() else {
                let _ = client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: format!("Document for {uri} not found"),
                });
                return;
            };

            let progress = ProgressCallbackServer::new(
                client.clone(),
                format!("Exporting {}", doc_uri.document_name()),
                None,
            );
            let _ = if let Err(e) = backend().export_html(doc_uri, &target) {
                client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: e,
                })
            } else {
                client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::INFO,
                    message: "Exported HTML successfully".to_string(),
                })
            };
            drop(progress);
        });
        ControlFlow::Continue(())
    }

    pub(crate) fn export_standalone(
        &mut self,
        params: StandaloneExportParams,
    ) -> <Self as LanguageServer>::NotifyResult {
        let StandaloneExportParams { uri, target } = params;
        let uri: UrlOrFile = uri.into();
        let state = self.inner.state().clone();
        let mut client = self.inner.client().clone();
        tokio::task::spawn_blocking(move || {
            let Some(doc) = state.get(&uri) else {
                let _ = client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: format!("Not a valid file path: {uri}"),
                });
                return;
            };
            let Some(doc_uri) = doc.document_uri() else {
                let _ = client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: format!("Document for {uri} not found"),
                });
                return;
            };
            let Some(file) = doc.path() else {
                let _ = client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: format!("File for {uri} not found"),
                });
                return;
            };
            let progress = ProgressCallbackServer::new(
                client.clone(),
                format!("Exporting {}", doc_uri.document_name()),
                None,
            );
            if let Err(e) = flams_stex::export_standalone(doc_uri, file, &target) {
                let _ = client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: format!(
                        "Error exporting {} to {}: {e:#}",
                        file.display(),
                        target.display()
                    ),
                });
            } else {
                let _ = client.show_message(lsp::ShowMessageParams {
                    typ: lsp::MessageType::INFO,
                    message: format!(
                        "Finished exporting {} to {}\n\nYou may want to verify the exported file actually compiles",
                        file.display(),
                        target.display()
                    ),
                });
            }
            drop(progress);
        });
        ControlFlow::Continue(())
    }

    pub(crate) fn quiz_request(&mut self, params: UriParams) -> Res<String> {
        use flams_system::backend::backend;
        fn get_res(url: UrlOrFile, state: LSPState) -> Result<String, String> {
            let doc = state
                .get(&url)
                .ok_or_else(|| "Document not found".to_string())?;
            let uri = doc
                .document_uri()
                .ok_or_else(|| "Document URI not found".to_string())?;
            let doc = backend().get_document(uri).map_err(|e| e.to_string())?;
            let quiz = doc
                .as_quiz(
                    &|d| backend().get_document(d).ok(),
                    &|d, r| backend().get_html_fragment(d, r).ok(),
                    &|d, r: DataRef<Solutions>| {
                        backend().get_reference(&r.with_doc(d.clone())).ok()
                    },
                    &|d, r: DataRef<GradingNote>| {
                        backend().get_reference(&r.with_doc(d.clone())).ok()
                    },
                )
                .map_err(|e| format!("{e:#}"))?;
            serde_json::to_string(&quiz).map_err(|e| format!("{e:#}"))
        }
        let state = self.inner.state().clone();
        let mut client = self.inner.client().clone();
        Box::pin(async move {
            let url: UrlOrFile = params.uri.into();
            tokio::task::spawn_blocking(move || match get_res(url, state) {
                Err(e) => {
                    let _ = client.show_message(lsp::ShowMessageParams {
                        typ: lsp::MessageType::ERROR,
                        message: e.clone(),
                    });
                    Err(ResponseError::new(async_lsp::ErrorCode::REQUEST_FAILED, e))
                }
                Ok(r) => Ok(r),
            })
            .map_err(|e| ResponseError::new(async_lsp::ErrorCode::REQUEST_FAILED, e.to_string()))
            .await?
        })
    }

    pub(crate) fn snify(
        &mut self,
        UriParams { uri }: UriParams,
    ) -> <Self as LanguageServer>::NotifyResult {
        let url: UrlOrFile = uri.into();
        let Some(doc) = self.inner.state().get(&url) else {
            let _ = self
                .inner
                .client_mut()
                .show_message(async_lsp::lsp_types::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: format!("Could not find file {url}"),
                });
            return ControlFlow::Continue(());
        };
        doc.up_to_date
            .store(false, std::sync::atomic::Ordering::SeqCst);
        doc.load_annotations_and(self.inner.state().clone(), true, |data| {
            let mut diagnostics = Vec::new();
            let iter: AnnotIter = data.annotations.iter().into();
            for e in <AnnotIter as TreeChildIter<STeXAnnot>>::dfs(iter) {
                if let STeXAnnot::SnifySuggestion { range, symbols } = e {
                    diagnostics.push(into_diagnostic(STeXDiagnostic {
                        level: flams_stex::quickparse::stex::DiagnosticLevel::Info,
                        range: *range,
                        message: "snify suggestion".to_string(),
                    }));
                }
            }
            if !diagnostics.is_empty() {
                let _ = self.inner.client_mut().publish_diagnostics(
                    async_lsp::lsp_types::PublishDiagnosticsParams {
                        uri: url.into(),
                        diagnostics,
                        version: None,
                    },
                );
            }
        });

        ControlFlow::Continue(())
    }

    fn build(
        doc: &LSPDocument,
        uri: &impl std::fmt::Display,
        stale_only: bool,
    ) -> Result<(), String> {
        let Some(id) = doc.archive().map(|a| a.archive_id()) else {
            return Err(format!("Containing archive for {uri} not found"));
        };
        let Some(rel_path) = doc.relative_path() else {
            return Err(format!("relative path for {uri} not found"));
        };
        //tracing::info!("Queueing [{id}]{{{rel_path}}}");
        flams_system::building::queue_manager::QueueManager::get().with_global(move |queue| {
            queue.enqueue_archive(
                id,
                FormatOrTargets::Format(flams_stex::STEX.id()),
                stale_only,
                Some(RelPath::new(rel_path)),
                false,
            )
        });
        Ok(())
    }

    pub(crate) fn build_one(&mut self, params: UriParams) -> Res<()> {
        let state = self.inner.state().clone();
        fut(move || {
            let url: UrlOrFile = params.uri.into();
            let Some(doc) = state.get(&url) else {
                return Err(format!("Document not found: {url}"));
            };
            Self::build(&doc, &url, false)
        })
    }
    pub(crate) fn build_all(&mut self, params: UriParams) -> Res<()> {
        let state = self.inner.state().clone();
        let client = self.inner.client().clone();
        wrap_fut(async move {
            let istate = state.clone();
            let url = block(move || {
                let url: UrlOrFile = params.uri.into();
                let Some(doc) = istate.get(&url) else {
                    return Err(format!("Document not found: {url}"));
                };
                Self::build(&doc, &url, false)?;
                Ok(url)
            })
            .await?;
            let deps = triomphe::Arc::new(parking_lot::Mutex::new(vec![url]));
            let mut curr = 0;
            loop {
                let url = {
                    let dps = deps.lock();
                    if curr == dps.len() {
                        break;
                    }
                    let d = dps[curr].clone();
                    drop(dps);
                    d
                };
                curr += 1;
                let Some(d) = state.get(&url) else {
                    // should be unreachable ?
                    continue;
                };
                //let deps = deps.clone();
                //let state = state.clone();
                let iclient = client.clone();
                let d_archive = d.archive().cloned();
                let Some(vec) = d
                    .with_annots_block(state.clone(), false, move |annots| {
                        let mut client = iclient;
                        <AnnotIter as TreeChildIter<STeXAnnot>>::dfs(AnnotIter::from(
                            annots.annotations.iter(),
                        ))
                        .filter_map(|a| match a {
                            STeXAnnot::Inputref {
                                archive, filepath, ..
                            }
                            | STeXAnnot::MHInput {
                                archive, filepath, ..
                            }
                            | STeXAnnot::IncludeProblem {
                                archive, filepath, ..
                            } => {
                                let archive = archive
                                    .as_ref()
                                    .map(|a| &a.0)
                                    .unwrap_or_else(|| unwrap!(d_archive.as_ref()).archive_id());
                                let Some(uri) = crate::annotations::uri_from_archive_relpath(
                                    &archive,
                                    &filepath.0,
                                ) else {
                                    let _ = client.show_message(lsp::ShowMessageParams {
                                        typ: lsp::MessageType::ERROR,
                                        message: format!(
                                            "Could not find file [{archive}]{{{}}}",
                                            &filepath.0
                                        ),
                                    });
                                    return None;
                                };
                                let url: UrlOrFile = uri.into();
                                Some(url)
                            }
                            STeXAnnot::ImportModule { module, .. }
                            | STeXAnnot::UseModule { module, .. } => {
                                let Some(uri) = module
                                    .full_path
                                    .as_ref()
                                    .and_then(|e| lsp::Url::from_file_path(e).ok())
                                else {
                                    let _ = client.show_message(lsp::ShowMessageParams {
                                        typ: lsp::MessageType::ERROR,
                                        message: format!(
                                            "Could not find module file {}",
                                            &module.uri
                                        ),
                                    });
                                    return None;
                                };
                                let url: UrlOrFile = uri.into();
                                Some(url)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                    })
                    .await
                else {
                    continue;
                };
                for url in vec {
                    {
                        let mut dep_lock = deps.lock();
                        if dep_lock.contains(&url) {
                            continue;
                        }
                        dep_lock.push(url.clone());
                    }
                    let state = state.clone();
                    let mut client = client.clone();
                    block(move || {
                        let Some(doc) = state.force_get(&url) else {
                            let _ = client.show_message(lsp::ShowMessageParams {
                                typ: lsp::MessageType::ERROR,
                                message: format!("Could not find document {url}"),
                            });
                            return Ok(());
                        };
                        if let Err(e) = Self::build(&doc, &url, true) {
                            let _ = client.show_message(lsp::ShowMessageParams {
                                typ: lsp::MessageType::ERROR,
                                message: format!("Could not queue document {url}: {e}"),
                            });
                        }
                        Ok(())
                    })
                    .await?
                }
            }
            Ok(())
        })
    }

    pub(crate) fn reload(
        &mut self,
        _: crate::ReloadParams,
    ) -> <Self as LanguageServer>::NotifyResult {
        let state = self.inner.state().clone();
        let client = self.inner.client().clone();
        tracing::info!("LSP: reload");
        state.backend().reset::<TokioEngine>();
        let _ = tokio::task::spawn_blocking(|| {
            for e in flams_system::iter::<flams_math_archives::FlamsExtension>() {
                (e.on_reload)();
            }
        });
        let _ = tokio::task::spawn_blocking(move || {
            state.load_mathhubs(client.clone());
            client.update_mathhub();
        });
        ControlFlow::Continue(())
    }

    pub(crate) fn install(
        &mut self,
        params: crate::InstallParams,
    ) -> <Self as LanguageServer>::NotifyResult {
        let state = self.inner.state().clone();
        let client = self.inner.client().clone();
        let mut progress = ProgressCallbackServer::new(client, "Installing".to_string(), None);
        let _ = tokio::task::spawn(async move {
            let crate::InstallParams {
                archives,
                remote_url,
            } = params;
            let mut rescan = false;
            let archives = {
                let mut ret = Vec::new();
                let exis = GlobalBackend.all_archives();
                for a in archives {
                    if exis.iter().any(|e| *e.id() == a) || ret.contains(&a) {
                        continue;
                    }
                    ret.push(a);
                }
                ret
            };
            let len = archives.len();
            for (i, a) in archives.into_iter().enumerate() {
                let url = format!("{remote_url}/api/backend/download?id={a}");
                let prefix = format!("{}/{len}: {a}", i + 1);
                progress.update(prefix.clone(), None);
                if flams_system::zip::unzip_from_remote(a.clone(), &url, |p| {
                    progress.update(format!("{prefix}: {}", p.display()), None)
                })
                .await
                .is_err()
                {
                    let _ = progress.client_mut().show_message(lsp::ShowMessageParams {
                        message: format!("Failed to install archive {a}"),
                        typ: lsp::MessageType::ERROR,
                    });
                } else {
                    rescan = true;
                }
            }
            let client = progress.client();
            drop(progress);
            if rescan {
                state.backend().reset::<TokioEngine>();
                let _ = tokio::task::spawn_blocking(|| {
                    for e in flams_system::iter::<flams_math_archives::FlamsExtension>() {
                        (e.on_reload)();
                    }
                });
                let _ = tokio::task::spawn_blocking(move || {
                    // <- necessary, but I don't quite understand why
                    state.load_mathhubs(client.clone());
                    client.update_mathhub();
                });
            } else {
                client.update_mathhub();
            }
        });
        ControlFlow::Continue(())
    }

    fn update_backend(
        files: impl Iterator<Item = PathBuf>,
    ) -> <Self as LanguageServer>::NotifyResult {
        let backend = GlobalBackend.get();
        let mut dones = Vec::new();
        let mut missing = false;
        for f in files {
            if backend
                .archive_of(&f, |a, _| {
                    if !dones.contains(a.id()) {
                        dones.push(a.id().clone());
                        a.update_sources();
                    }
                })
                .is_none()
            {
                missing = true;
            }
        }
        if missing {
            backend.load(flams_system::settings::Settings::get().mathhubs());
        }
        ControlFlow::Continue(())
    }
}

type Res<T> = BoxFuture<'static, Result<T, ResponseError>>;

impl<T: FLAMSLSPServer> LanguageServer for ServerWrapper<T> {
    type Error = ResponseError;
    type NotifyResult = ControlFlow<async_lsp::Result<()>>;

    fn initialize(&mut self, params: lsp::InitializeParams) -> Res<lsp::InitializeResult> {
        tracing::info!("LSP: initialize");
        self.inner.initialize(
            params
                .workspace_folders
                .unwrap_or_default()
                .into_iter()
                .map(|f| (f.name, f.uri)),
        );
        Box::pin(std::future::ready({
            Ok(lsp::InitializeResult {
                capabilities: super::capabilities::capabilities(),
                server_info: None,
            })
        }))
    }

    fn shutdown(&mut self, (): ()) -> Res<()> {
        tracing::info!("LSP: shutdown");
        Box::pin(std::future::ready(Ok(())))
    }

    // Notifications -------------------------------------------

    //impl_notification!(! initialized = Initialized);
    fn initialized(&mut self, _params: lsp::InitializedParams) -> Self::NotifyResult {
        tracing::info!("LSP: initialized");
        self.inner.initialized();
        /*
         */
        ControlFlow::Continue(())
    }

    impl_notification!(!exit = Exit);

    // workspace/
    impl_notification!(!did_change_workspace_folders = DidChangeWorkspaceFolders);
    impl_notification!(!did_change_configuration = DidChangeConfiguration);
    impl_notification!(!did_change_watched_files = DidChangeWatchedFiles);

    // textDocument/
    //impl_notification!(! did_open = DidOpenTextDocument);
    fn did_open(&mut self, params: lsp::DidOpenTextDocumentParams) -> Self::NotifyResult {
        let document = params.text_document;
        tracing::trace!(
            "URI: {}, language: {}, version: {}, text: \n{}",
            document.uri,
            document.language_id,
            document.version,
            document.text
        );
        self.inner
            .state()
            .insert(document.uri.into(), document.text);
        ControlFlow::Continue(())
    }

    #[allow(clippy::let_underscore_future)]
    //impl_notification!(! did_change = DidChangeTextDocument);
    fn did_change(&mut self, params: lsp::DidChangeTextDocumentParams) -> Self::NotifyResult {
        let document = params.text_document;
        let uri = document.uri.clone().into();
        if let Some(d) = self.inner.state().get(&uri) {
            for change in params.content_changes {
                tracing::trace!(
                    "URI: {},version: {}, text: \"{}\", range: {:?}",
                    document.uri,
                    document.version,
                    change.text,
                    change.range
                );
                d.delta(change.text, change.range);
            }
            let mut client = self.inner.client().clone();
            let _ = tokio::spawn(d.with_annots(self.inner.state().clone(), false, move |a| {
                let r = lsp::PublishDiagnosticsParams {
                    uri: document.uri,
                    diagnostics: a.diagnostics.iter().map(to_diagnostic).collect(),
                    version: Some(document.version),
                };
                let _ = client.publish_diagnostics(r);
            }));
        } else {
            tracing::warn!("document not found: {}", document.uri);
        }
        ControlFlow::Continue(())
    }

    #[allow(clippy::let_underscore_future)]
    //impl_notification!(! did_save = DidSaveTextDocument);
    fn did_save(&mut self, params: lsp::DidSaveTextDocumentParams) -> Self::NotifyResult {
        tracing::trace!("did_save: {}", params.text_document.uri);
        let state = self.inner.state().clone();
        let client = self.inner.client().clone();
        let uri = params.text_document.uri.into();
        let _ = tokio::task::spawn_blocking(move || {
            state.build_html_and_notify(&uri, client);
        });
        ControlFlow::Continue(())
    }

    impl_notification!(!will_save = WillSaveTextDocument);

    //impl_notification!(! did_close = DidCloseTextDocument);
    fn did_close(&mut self, params: lsp::DidCloseTextDocumentParams) -> Self::NotifyResult {
        tracing::trace!("did_close: {}", params.text_document.uri);
        ControlFlow::Continue(())
    }

    // window/
    // workDoneProgress/
    impl_notification!(work_done_progress_cancel = WorkDoneProgressCancel);

    // $/
    impl_notification!(!set_trace = SetTrace);
    impl_notification!(!cancel_request = Cancel);
    impl_notification!(!progress = Progress);

    // Requests -----------------------------------------------

    // textDocument/

    // impl_request!(document_symbol = DocumentSymbolRequest);
    fn document_symbol(
        &mut self,
        params: lsp::DocumentSymbolParams,
    ) -> Res<Option<lsp::DocumentSymbolResponse>> {
        tracing::trace_span!("document_symbol").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?}, partial_results: {:?}",
                params.text_document.uri,
                params.work_done_progress_params,
                params.partial_result_params
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_symbols(&params.text_document.uri.into(), p)
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(Result::Ok)) as _,
                )
        })
    }

    // impl_request!(! document_diagnostic = DocumentDiagnosticRequest => (lsp::DocumentDiagnosticReportResult::Report(lsp::DocumentDiagnosticReport::Full(lsp::RelatedFullDocumentDiagnosticReport::default()))));
    fn document_diagnostic(
        &mut self,
        params: lsp::DocumentDiagnosticParams,
    ) -> Res<lsp::DocumentDiagnosticReportResult> {
        fn default() -> lsp::DocumentDiagnosticReportResult {
            lsp::DocumentDiagnosticReportResult::Report(lsp::DocumentDiagnosticReport::Full(
                lsp::RelatedFullDocumentDiagnosticReport::default(),
            ))
        }
        tracing::trace_span!("document_diagnostics").in_scope(move || {
            tracing::trace!("work_done_progress_params: {:?}, partial_results: {:?}, position: {:?}, context: {:?}",
                params.work_done_progress_params,
                params.partial_result_params,
                params.text_document,
                params.identifier
            );

            let p = params.work_done_progress_params.work_done_token.map(
                |tk| self.get_progress(tk)
            );
            self.inner.state().get_diagnostics(&params.text_document.uri.into(),p)
                .map_or_else(|| Box::pin(std::future::ready(Ok(default()))) as _,
                |f| Box::pin(f.map(Result::Ok)) as _
            )
        })
    }

    //impl_request!(? references = References => (None));
    fn references(&mut self, params: lsp::ReferenceParams) -> Res<Option<Vec<lsp::Location>>> {
        tracing::trace_span!("references").in_scope(move || {
            tracing::trace!("work_done_progress_params: {:?}, partial_results: {:?}, position: {:?}, context: {:?}",
                params.work_done_progress_params,
                params.partial_result_params,
                params.text_document_position,
                params.context
            );
            let p = params.work_done_progress_params.work_done_token.map(
                |tk| self.get_progress(tk)
            );
            self.inner.state().get_references(
                params.text_document_position.text_document.uri.into(),
                params.text_document_position.position,p
            ).map_or_else(|| Box::pin(std::future::ready(Ok(None))) as _,
                |f| Box::pin(f.map(Result::Ok)) as _
                )
        })
    }

    //impl_request!(! document_link = DocumentLinkRequest => (None));
    fn document_link(
        &mut self,
        params: lsp::DocumentLinkParams,
    ) -> Res<Option<Vec<lsp::DocumentLink>>> {
        tracing::trace_span!("document_link").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?}, partial_results: {:?}",
                params.text_document.uri,
                params.work_done_progress_params,
                params.partial_result_params
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_links(&params.text_document.uri.into(), p)
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(Result::Ok)) as _,
                )
        })
    }

    // impl_request!(! hover = HoverRequest => (None));
    fn hover(&mut self, params: lsp::HoverParams) -> Res<Option<lsp::Hover>> {
        tracing::trace_span!("hover").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?}, position: {:?}",
                params.text_document_position_params.text_document.uri,
                params.work_done_progress_params,
                params.text_document_position_params.position
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_hover(
                    &params
                        .text_document_position_params
                        .text_document
                        .uri
                        .into(),
                    params.text_document_position_params.position,
                    p,
                )
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(Result::Ok)) as _,
                )
        })
    }

    // impl_request!(! definition = GotoDefinition => (None));
    fn definition(
        &mut self,
        params: lsp::GotoDefinitionParams,
    ) -> Res<Option<lsp::GotoDefinitionResponse>> {
        tracing::trace_span!("definition").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?}, position: {:?}",
                params.text_document_position_params.text_document.uri,
                params.work_done_progress_params,
                params.text_document_position_params.position
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_goto_definition(
                    params
                        .text_document_position_params
                        .text_document
                        .uri
                        .into(),
                    params.text_document_position_params.position,
                    p,
                )
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(Result::Ok)) as _,
                )
        })
    }

    impl_request!(! code_lens = CodeLensRequest => (None));

    impl_request!(! declaration = GotoDefinition => (None));

    impl_request!(! workspace_diagnostic = WorkspaceDiagnosticRequest => (lsp::WorkspaceDiagnosticReportResult::Report(lsp::WorkspaceDiagnosticReport {items:Vec::new()})));
    /*
        #[must_use]
        fn workspace_diagnostic(&mut self, params: lsp::WorkspaceDiagnosticParams) -> Res<lsp::WorkspaceDiagnosticReportResult> {
            tracing::info_span!("workspace_diagnostics").in_scope(move || {
                tracing::info!("work_done_progress_params: {:?}, partial_results: {:?}, identifier: {:?}, previous_results_id: {:?}",
                    params.work_done_progress_params,
                    params.partial_result_params,
                    params.identifier,
                    params.previous_result_ids
                );
                if let Some(_token) = params.partial_result_params.partial_result_token {
                    if self.ws_diagnostics.load(Ordering::Relaxed) {
                        self.ws_diagnostics.store(false, Ordering::Relaxed);
                        return Box::pin(std::future::ready(Ok(
                            lsp::WorkspaceDiagnosticReportResult::Partial(lsp::WorkspaceDiagnosticReportPartialResult {
                                items:Vec::new()
                            })
                        )))
                    }

                    self.ws_diagnostics.store(true, Ordering::Relaxed);
                    return Box::pin(std::future::ready(Ok(
                        lsp::WorkspaceDiagnosticReportResult::Report(lsp::WorkspaceDiagnosticReport {
                            items:Vec::new()
                        })
                    )))
                }

                /*
                if let Some(p) = params.work_done_progress_params.work_done_token {
                    self.get_progress(p).finish_delay();
                }
                if let Some(p) = params.partial_result_params.partial_result_token {
                    self.get_progress(p).finish_delay();
                }
                */
                Box::pin(std::future::ready(Ok(
                    lsp::WorkspaceDiagnosticReportResult::Report(lsp::WorkspaceDiagnosticReport {
                        items:Vec::new()
                    })
                )))
            })
        }
    */

    //impl_request!(! inlay_hint = InlayHintRequest => (None));
    fn inlay_hint(&mut self, params: lsp::InlayHintParams) -> Res<Option<Vec<lsp::InlayHint>>> {
        tracing::trace_span!("inlay hint").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?}",
                params.text_document.uri,
                params.work_done_progress_params,
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_inlay_hints(&params.text_document.uri.into(), p)
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(Result::Ok)) as _,
                )
        })
    }
    // inlayHint/
    impl_request!(inlay_hint_resolve = InlayHintResolveRequest);

    //impl_request!(! code_action = CodeActionRequest => (None));
    fn code_action(
        &mut self,
        params: lsp::CodeActionParams,
    ) -> Res<Option<lsp::CodeActionResponse>> {
        tracing::trace_span!("code_action").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?}; range: {:?}; context:{:?}",
                params.text_document.uri, //.text_document_position_params.text_document.uri,
                params.work_done_progress_params,
                params.range,
                params.context
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_codeaction(
                    params.text_document.uri.into(),
                    params.range,
                    params.context,
                    p,
                )
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(Result::Ok)) as _,
                )
        })
    }

    //impl_request!(prepare_call_hierarchy = CallHierarchyPrepare);
    fn prepare_call_hierarchy(
        &mut self,
        params: lsp::CallHierarchyPrepareParams,
    ) -> Res<Option<Vec<lsp::CallHierarchyItem>>> {
        tracing::trace_span!("prepare_call_hierarchy").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?}; position: {:?}",
                params.text_document_position_params.text_document.uri,
                params.work_done_progress_params,
                params.text_document_position_params.position
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .prepare_module_hierarchy(
                    params
                        .text_document_position_params
                        .text_document
                        .uri
                        .into(),
                    p,
                )
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(Result::Ok)) as _,
                )
        })
    }

    // callHierarchy/
    //impl_request!(incoming_calls = CallHierarchyIncomingCalls);
    fn incoming_calls(
        &mut self,
        params: lsp::CallHierarchyIncomingCallsParams,
    ) -> Res<Option<Vec<lsp::CallHierarchyIncomingCall>>> {
        tracing::trace_span!("incoming_call_hierarchy").in_scope(move || {
            tracing::trace!(
                "uri: {},work_done_progress_params: {:?};",
                params.item.uri,
                params.work_done_progress_params,
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            if let Some(d) = params
                .item
                .data
                .and_then(|d| d.as_str().and_then(|d| d.parse().ok()))
            {
                self.inner
                    .state()
                    .module_hierarchy_imports(params.item.uri, params.item.kind, d, p)
                    .map_or_else(
                        || Box::pin(std::future::ready(Ok(None))) as _,
                        |f| Box::pin(f.map(Result::Ok)) as _,
                    )
            } else {
                Box::pin(std::future::ready(Ok(None))) as _
            }
        })
    }
    impl_request!(outgoing_calls = CallHierarchyOutgoingCalls);

    impl_request!(! document_highlight = DocumentHighlightRequest => (None));
    impl_request!(! folding_range = FoldingRangeRequest => (None));

    impl_request!(implementation = GotoImplementation);
    impl_request!(type_definition = GotoTypeDefinition);
    impl_request!(document_color = DocumentColor);
    impl_request!(color_presentation = ColorPresentationRequest);
    impl_request!(selection_range = SelectionRangeRequest);
    impl_request!(moniker = MonikerRequest);
    impl_request!(inline_value = InlineValueRequest);
    impl_request!(on_type_formatting = OnTypeFormatting);
    impl_request!(range_formatting = RangeFormatting);
    impl_request!(formatting = Formatting);
    impl_request!(prepare_type_hierarchy = TypeHierarchyPrepare);
    impl_request!(will_save_wait_until = WillSaveWaitUntil);

    impl_request!(!completion = Completion => (None));

    impl_request!(signature_help = SignatureHelpRequest);
    impl_request!(linked_editing_range = LinkedEditingRange);

    // semanticTokens/
    // impl_request!(semantic_tokens_full = SemanticTokensFullRequest);
    fn semantic_tokens_full(
        &mut self,
        params: lsp::SemanticTokensParams,
    ) -> Res<Option<lsp::SemanticTokensResult>> {
        tracing::trace_span!("semantic_tokens_full").in_scope(|| {
            tracing::trace!(
                "work_done_progress_params: {:?}, partial_results: {:?}, uri: {}",
                params.work_done_progress_params,
                params.partial_result_params,
                params.text_document.uri
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_semantic_tokens(&params.text_document.uri.into(), p, None)
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(|r| Ok(r.map(lsp::SemanticTokensResult::Tokens)))) as _,
                )
        })
    }

    // impl_request!(semantic_tokens_range = SemanticTokensRangeRequest);
    fn semantic_tokens_range(
        &mut self,
        params: lsp::SemanticTokensRangeParams,
    ) -> Res<Option<lsp::SemanticTokensRangeResult>> {
        tracing::trace_span!("semantic_tokens_range").in_scope(|| {
            tracing::trace!(
                "work_done_progress_params: {:?}, partial_results: {:?}, range: {:?}, uri:{}",
                params.work_done_progress_params,
                params.partial_result_params,
                params.range,
                params.text_document.uri
            );
            let p = params
                .work_done_progress_params
                .work_done_token
                .map(|tk| self.get_progress(tk));
            self.inner
                .state()
                .get_semantic_tokens(&params.text_document.uri.into(), p, Some(params.range))
                .map_or_else(
                    || Box::pin(std::future::ready(Ok(None))) as _,
                    |f| Box::pin(f.map(|r| Ok(r.map(lsp::SemanticTokensRangeResult::Tokens)))) as _,
                )
        })
    }

    // impl_request!(semantic_tokens_full_delta = SemanticTokensFullDeltaRequest);
    fn semantic_tokens_full_delta(
        &mut self,
        params: lsp::SemanticTokensDeltaParams,
    ) -> Res<Option<lsp::SemanticTokensFullDeltaResult>> {
        tracing::info_span!("semantic_tokens_full_delta").in_scope(|| {
                tracing::info!("work_done_progress_params: {:?}, partial_results: {:?}, previous_result_id: {:?}, uri:{}",
                    params.work_done_progress_params,
                    params.partial_result_params,
                    params.previous_result_id,
                    params.text_document.uri
                );
                Box::pin(std::future::ready(Ok(None)))
            })
    }

    //impl_notification!(!did_create_files = DidCreateFiles);
    fn did_create_files(&mut self, params: lsp::CreateFilesParams) -> Self::NotifyResult {
        let files = params.files.into_iter().map(|f| PathBuf::from(f.uri));
        Self::update_backend(files)
    }

    //impl_notification!(!did_rename_files = DidRenameFiles);
    fn did_rename_files(&mut self, params: lsp::RenameFilesParams) -> Self::NotifyResult {
        let files = params
            .files
            .into_iter()
            .flat_map(|f| vec![PathBuf::from(f.old_uri), PathBuf::from(f.new_uri)]);
        Self::update_backend(files)
    }

    //impl_notification!(!did_delete_files = DidDeleteFiles);
    fn did_delete_files(&mut self, params: lsp::DeleteFilesParams) -> Self::NotifyResult {
        let files = params.files.into_iter().map(|f| PathBuf::from(f.uri));
        Self::update_backend(files)
    }

    impl_request!(prepare_rename = PrepareRenameRequest);
    impl_request!(rename = Rename);

    // workspace/
    impl_request!(will_create_files = WillCreateFiles);
    impl_request!(will_rename_files = WillRenameFiles);
    impl_request!(will_delete_files = WillDeleteFiles);
    impl_request!(symbol = WorkspaceSymbolRequest);

    //impl_request!(execute_command = ExecuteCommand);
    fn execute_command(
        &mut self,
        params: lsp::ExecuteCommandParams,
    ) -> Res<Option<serde_json::Value>> {
        tracing::trace_span!("executing command {}", params.command).in_scope(move || {
            tracing::trace!("{:?}", params.arguments);
            match &*params.command {
                "snify/annotate" => {
                    let state = self.inner.state().clone();
                    let client = self.inner.client().clone();
                    tokio::task::spawn_blocking(move || {
                        snify_annotate(state, client, params.arguments);
                    });
                }
                c => {
                    let _ = self
                        .inner
                        .client_mut()
                        .show_message(lsp::ShowMessageParams {
                            message: format!("Unknown command {c:?}"),
                            typ: lsp::MessageType::ERROR,
                        });
                }
            }
            Box::pin(std::future::ready(Ok(None))) as _
        })
    }

    // typeHierarchy/
    impl_request!(supertypes = TypeHierarchySupertypes);
    impl_request!(subtypes = TypeHierarchySubtypes);

    // completionItem/
    impl_request!(completion_item_resolve = ResolveCompletionItem);

    // codeAction/
    //impl_request!(code_action_resolve = CodeActionResolveRequest);
    fn code_action_resolve(&mut self, params: lsp::CodeAction) -> Res<lsp::CodeAction> {
        tracing::trace_span!("executing command").in_scope(move || {
            tracing::trace!("{params:?}");
            Box::pin(std::future::ready(Ok(lsp::CodeAction {
                title: "None".to_string(),
                kind: None,
                diagnostics: None,
                edit: None,
                command: None,
                is_preferred: None,
                disabled: None,
                data: None,
            }))) as _
        })
    }

    // workspaceSymbol/
    impl_request!(workspace_symbol_resolve = WorkspaceSymbolResolve);

    // codeLens/
    impl_request!(code_lens_resolve = CodeLensResolve);

    // documentLink/
    impl_request!(document_link_resolve = DocumentLinkResolve);
}

fn snify_annotate(
    state: LSPState,
    mut client: ClientSocket,
    mut arguments: Vec<serde_json::Value>,
) {
    fn err_num(num: usize, mut client: ClientSocket) {
        let _ = client.show_message(lsp::ShowMessageParams {
            message: format!(
                "invalid number of arguments for snify/annotate; expected: 4; got: {num}"
            ),
            typ: lsp::MessageType::ERROR,
        });
    }
    let Some(range) = arguments.pop() else {
        err_num(0, client);
        return;
    };
    let Some(doc) = arguments.pop() else {
        err_num(1, client);
        return;
    };
    let Some(needs_usemodule) = arguments.pop() else {
        err_num(2, client);
        return;
    };
    let Some(uri) = arguments.pop() else {
        err_num(3, client);
        return;
    };
    if !arguments.is_empty() {
        err_num(arguments.len() + 4, client);
        return;
    }

    let Ok(range) = serde_json::from_value::<StringRange<LSPLineCol>>(range) else {
        let _ = client.show_message(lsp::ShowMessageParams {
            message: "Expected document range in argument 4".to_string(),
            typ: lsp::MessageType::ERROR,
        });
        return;
    };
    let Ok(needs_usemodule) = serde_json::from_value::<bool>(needs_usemodule) else {
        let _ = client.show_message(lsp::ShowMessageParams {
            message: "Expected boolean in argument 3".to_string(),
            typ: lsp::MessageType::ERROR,
        });
        return;
    };
    let Ok(url) = serde_json::from_value::<lsp::Url>(doc) else {
        let _ = client.show_message(lsp::ShowMessageParams {
            message: "Expected lsp uri in argument 2".to_string(),
            typ: lsp::MessageType::ERROR,
        });
        return;
    };
    let uri = if let serde_json::Value::String(s) = uri {
        if s.is_empty() {
            None
        } else {
            let Ok(uri) = s.parse::<SymbolUri>() else {
                let _ = client.show_message(lsp::ShowMessageParams {
                    message: "Expected symbol uri in argument 1".to_string(),
                    typ: lsp::MessageType::ERROR,
                });
                return;
            };
            Some(uri)
        }
    } else {
        let _ = client.show_message(lsp::ShowMessageParams {
            message: "Expected symbol uri in argument 1".to_string(),
            typ: lsp::MessageType::ERROR,
        });
        return;
    };

    tracing::trace!(
        "Executing snify/annotate on {url}@{range} with {uri:?}; Needs usemodule:{needs_usemodule}"
    );

    let urlfile = UrlOrFile::from(url.clone());
    let Some(doc) = state.get(&urlfile) else {
        let _ = client.show_message(lsp::ShowMessageParams {
            message: format!("Document {urlfile} not found"),
            typ: lsp::MessageType::ERROR,
        });
        return;
    };
    let edits = doc.with_text(|txt| {
        use crate::IsLSPRange;
        thread_local! {
            static NEEDLE:Needle<'static> = Needle::new("% srskip ");
        }
        let mut ret = Vec::new();
        let Some(uri) = uri else {
            let Some(text) = LSPLineCol::get_range(range.start, range.end, txt) else {
                return Vec::new();
            };
            let mut parser = StrParser::<LSPLineCol>::new(txt);
            let _ = NEEDLE.with(|n| parser.read_until_needle(n));
            if parser.rest().is_empty() {
                return vec![lsp::TextEdit {
                    range: StringRange::into_range(StringRange {
                        start: parser.pos,
                        end: parser.pos,
                    }),
                    new_text: format!("\n% srskip l:{text}"),
                }];
            }
            let _ = parser.drop_prefix("% srskip ");
            return vec![lsp::TextEdit {
                range: StringRange::into_range(StringRange {
                    start: parser.pos,
                    end: parser.pos,
                }),
                new_text: format!("l:{text}, "),
            }];
        };
        if needs_usemodule {
            use std::fmt::Write;
            let mut parser = StrParser::<LSPLineCol>::new(txt);
            parser.read_until_str("\\begin{document}");
            parser.drop_prefix("\\begin{document}");
            let mut insertion = format!("\n  \\usemodule[{}]{{", uri.archive_id());
            if let Some(path) = uri.path() {
                let _ = write!(insertion, "{path}?{}}}", uri.module_name());
            } else {
                let _ = write!(insertion, "{}}}", uri.module_name());
            }
            ret.push(lsp::TextEdit {
                range: StringRange::into_range(StringRange {
                    start: parser.pos,
                    end: parser.pos,
                }),
                new_text: insertion,
            });
        }
        ret.push(lsp::TextEdit {
            range: StringRange::into_range(StringRange {
                start: range.start,
                end: range.start,
            }),
            new_text: format!("\\sr{{{}}}{{", uri.name()),
        });
        ret.push(lsp::TextEdit {
            range: StringRange::into_range(StringRange {
                start: range.end,
                end: range.end,
            }),
            new_text: "}".to_string(),
        });
        ret
        /*
        let StringRange { start, end } =
            PositionConverter::<LSPLineCol, ByteOffset>::new(txt).next_range(range);
        let annot_text = txt.get(start.0..end.0) else {
            let _ = client.show_message(lsp::ShowMessageParams {
                message: format!("Range {range} outside document"),
                typ: lsp::MessageType::ERROR,
            });
            return Vec::new();
        };
         */
    });
    let mut eds = std::collections::HashMap::new();
    eds.insert(url, edits);
    doc.force_snify
        .store(true, std::sync::atomic::Ordering::Relaxed);
    std::mem::drop(tokio::task::spawn(client.apply_edit(
        lsp::ApplyWorkspaceEditParams {
            label: None,
            edit: lsp::WorkspaceEdit {
                document_changes: None,
                change_annotations: None,
                changes: Some(eds),
            },
        },
    )));
}
