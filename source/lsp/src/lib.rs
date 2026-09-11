#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod annotations;
pub mod capabilities;
pub mod documents;
mod implementation;
pub mod state;
pub mod verbalizations;
#[cfg(feature = "ws")]
pub mod ws;

//use dashmap::Entry;
pub(crate) use std::collections::hash_map::Entry;
pub(crate) type HMap<A, B> = rustc_hash::FxHashMap<A, B>;
// pub(crate) type HMap<A,B> = dashmap::DashMap<A,B,rustc_hash::FxHashBuilder>
pub(crate) type LMap<A, B> = parking_lot::RwLock<HMap<A, B>>;
// pub(crate) type LMap<A,B> = dashmap::DashMap<A,B,rustc_hash::FxHashBuilder>

use std::path::Path;

pub use async_lsp;
use async_lsp::{ClientSocket, LanguageClient, lsp_types as lsp};
use flams_math_archives::backend::AnyBackend;
use flams_stex::quickparse::stex::{
    STeXParseData,
    structs::{GetModuleError, ModuleReference, STeXModuleStore},
};
use flams_system::settings::Settings;
use flams_utils::{
    background,
    parsing::StrParser,
    sourcerefs::{LSPLineCol, PositionKind, StringRange},
    unwrap,
};
use ftml_uris::{ArchiveId, BaseUri, DocumentUri, Language, SymbolUri};
use state::{DocData, LSPState, UrlOrFile};

use crate::verbalizations::UnlockedVerbalizationTrie;

static GLOBAL_STATE: std::sync::OnceLock<LSPState> = std::sync::OnceLock::new();
pub struct STDIOLSPServer {
    client: ClientSocket,
    on_port: tokio::sync::watch::Receiver<Option<u16>>,
    workspaces: Vec<(String, lsp::Url)>,
}

impl STDIOLSPServer {
    #[inline]
    pub fn global_state() -> Option<&'static LSPState> {
        GLOBAL_STATE.get()
    }
    fn load_all(&self) {
        let client = self.client.clone();
        let state = unwrap!(Self::global_state());
        for (name, uri) in &self.workspaces {
            tracing::info!("workspace: {name}@{uri}");
        }
        background(move || state.load_mathhubs(client));
    }

    fn new_router(
        client: ClientSocket,
        on_port: tokio::sync::watch::Receiver<Option<u16>>,
    ) -> async_lsp::router::Router<ServerWrapper<Self>> {
        let _ = GLOBAL_STATE.set(LSPState::default());
        let server = ServerWrapper::new(Self {
            client,
            on_port,
            workspaces: Vec::new(),
        });
        server.router()
    }
}

#[allow(clippy::future_not_send)]
/// #### Panics
pub async fn start_lsp(on_port: tokio::sync::watch::Receiver<Option<u16>>) {
    let (server, _client) = async_lsp::MainLoop::new_server(|client| {
        tower::ServiceBuilder::new()
            .layer(async_lsp::tracing::TracingLayer::default())
            .layer(async_lsp::server::LifecycleLayer::default())
            .layer(async_lsp::panic::CatchUnwindLayer::default())
            .layer(async_lsp::concurrency::ConcurrencyLayer::default())
            .layer(async_lsp::client_monitor::ClientProcessMonitorLayer::new(
                client.clone(),
            ))
            .service(STDIOLSPServer::new_router(client, on_port))
    });

    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().expect("Failed to lock stdin"),
        async_lsp::stdio::PipeStdout::lock_tokio().expect("Failed to lock stdout"),
    );
    #[cfg(not(unix))]
    let (stdin, stdout) = (
        tokio_util::compat::TokioAsyncReadCompatExt::compat(tokio::io::stdin()),
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tokio::io::stdout()),
    );

    server
        .run_buffered(stdin, stdout)
        .await
        .expect("Failed to run server");
}

impl FLAMSLSPServer for STDIOLSPServer {
    #[inline]
    fn client_mut(&mut self) -> &mut ClientSocket {
        &mut self.client
    }
    #[inline]
    fn client(&self) -> &ClientSocket {
        &self.client
    }
    #[inline]
    fn state(&self) -> &LSPState {
        Self::global_state().unwrap_or_else(|| unreachable!())
    }
    fn initialized(&mut self) {
        let v = *self.on_port.borrow();
        if v.is_some() {
            if let Err(r) = self.client.notify::<ServerURL>(ServerURL::get()) {
                tracing::error!("failed to send notification: {}", r);
            }
        } else {
            let mut port = self.on_port.clone();
            let client = self.client.clone();
            tokio::spawn(async move {
                let _ = port
                    .wait_for(|e| {
                        e.map_or(false, |_| {
                            if let Err(r) = client.notify::<ServerURL>(ServerURL::get()) {
                                tracing::error!("failed to send notification: {}", r);
                            };
                            true
                        })
                    })
                    .await;
            });
        }
        tracing::info!(
            "Using {} threads",
            tokio::runtime::Handle::current().metrics().num_workers()
        );
        //#[cfg(not(debug_assertions))]
        {
            self.load_all();
        }
    }

    fn initialize<I: Iterator<Item = (String, lsp::Url)> + Send + 'static>(
        &mut self,
        workspaces: I,
    ) {
        self.workspaces = workspaces.collect();
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ReloadParams {}
struct Reload;
impl lsp::notification::Notification for Reload {
    type Params = ReloadParams;
    const METHOD: &str = "flams/reload";
}

#[derive(serde::Serialize, serde::Deserialize)]
struct InstallParams {
    pub archives: Vec<ArchiveId>,
    pub remote_url: String,
}
struct InstallArchives;
impl lsp::notification::Notification for InstallArchives {
    type Params = InstallParams;
    const METHOD: &str = "flams/install";
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NewArchiveParams {
    pub archive: ArchiveId,
    pub urlbase: BaseUri,
}
struct NewArchive;
impl lsp::notification::Notification for NewArchive {
    type Params = NewArchiveParams;
    const METHOD: &str = "flams/newArchive";
}

pub(crate) struct StandaloneExport;
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StandaloneExportParams {
    pub uri: lsp::Url,
    pub target: std::path::PathBuf,
}
impl lsp::notification::Notification for StandaloneExport {
    type Params = StandaloneExportParams;
    const METHOD: &str = "flams/standaloneExport";
}

pub(crate) struct HtmlExport;
impl lsp::notification::Notification for HtmlExport {
    type Params = StandaloneExportParams;
    const METHOD: &str = "flams/htmlExport";
}

struct UpdateMathHub;
impl lsp::notification::Notification for UpdateMathHub {
    type Params = Vec<()>;
    const METHOD: &str = "flams/updateMathHub";
}

struct OpenFile;
impl lsp::notification::Notification for OpenFile {
    type Params = UriParams;
    const METHOD: &str = "flams/openFile";
}

struct HTMLResult;
impl lsp::notification::Notification for HTMLResult {
    type Params = StringParams;
    const METHOD: &str = "flams/htmlResult";
}

pub(crate) struct SnifyNotification;
impl lsp::notification::Notification for SnifyNotification {
    type Params = UriParams;
    const METHOD: &str = "flams/snify";
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UriParams {
    pub uri: lsp::Url,
}
pub(crate) struct HTMLRequest;
impl lsp::request::Request for HTMLRequest {
    type Params = UriParams;
    type Result = Option<String>;
    const METHOD: &'static str = "flams/htmlRequest";
}

struct BuildOne;
impl lsp::request::Request for BuildOne {
    type Params = UriParams;
    type Result = ();
    const METHOD: &str = "flams/buildOne";
}
struct BuildAll;
impl lsp::request::Request for BuildAll {
    type Params = UriParams;
    type Result = ();
    const METHOD: &str = "flams/buildAll";
}

pub(crate) struct QuizRequest;
impl lsp::request::Request for QuizRequest {
    type Params = UriParams;
    type Result = String;
    const METHOD: &'static str = "flams/quizRequest";
}

pub struct ServerURL;
impl ServerURL {
    fn get() -> StringParams {
        let settings = Settings::get();
        StringParams {
            url: format!("http://{}:{}", settings.ip, settings.port()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct StringParams {
    url: String,
}

impl lsp::notification::Notification for ServerURL {
    type Params = StringParams;
    const METHOD: &str = "flams/serverURL";
}

pub trait ClientExt {
    fn html_result(&self, uri: &DocumentUri);
    fn update_mathhub(&self);
    fn open_file(&self, path: &Path);
}
impl ClientExt for ClientSocket {
    #[inline]
    fn html_result(&self, uri: &DocumentUri) {
        let _ = self.notify::<HTMLResult>(StringParams {
            url: uri.to_string(),
        });
    }
    #[inline]
    fn update_mathhub(&self) {
        if let Err(e) = self.notify::<UpdateMathHub>(Vec::new()) {
            tracing::error!("failed to send notification: {}", e);
        }
    }

    fn open_file(&self, path: &Path) {
        let Ok(url) = lsp::Url::from_file_path(path) else {
            return;
        };
        if let Err(e) = self.notify::<OpenFile>(UriParams { uri: url }) {
            tracing::error!("failed to send notification: {}", e);
        }
    }
}

pub trait FLAMSLSPServer: 'static {
    fn client_mut(&mut self) -> &mut ClientSocket;
    fn client(&self) -> &ClientSocket;
    fn state(&self) -> &LSPState;
    #[inline]
    fn initialized(&mut self) {}
    #[inline]
    fn initialize<I: Iterator<Item = (String, lsp::Url)> + Send + 'static>(
        &mut self,
        _workspaces: I,
    ) {
    }
}

pub struct ServerWrapper<T: FLAMSLSPServer> {
    pub inner: T,
}
impl<T: FLAMSLSPServer> ServerWrapper<T> {
    #[inline]
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn router(self) -> async_lsp::router::Router<Self> {
        let mut r = async_lsp::router::Router::from_language_server(self);
        r.request::<HTMLRequest, _>(Self::html_request);
        r.request::<QuizRequest, _>(Self::quiz_request);
        r.request::<BuildOne, _>(Self::build_one);
        r.request::<BuildAll, _>(Self::build_all);

        r.notification::<Reload>(Self::reload);
        r.notification::<StandaloneExport>(Self::export_standalone);
        r.notification::<SnifyNotification>(Self::snify);
        r.notification::<HtmlExport>(Self::export_html);
        r.notification::<InstallArchives>(Self::install);
        r.notification::<NewArchive>(Self::new_archive);
        //r.request(handler)
        r
    }

    pub fn get_progress(&self, tk: lsp::ProgressToken) -> ProgressCallbackClient {
        match &tk {
            lsp::ProgressToken::Number(n) if *n > 0 => {
                TOKEN.fetch_max(*n as u32 + 1, std::sync::atomic::Ordering::Relaxed);
            }
            _ => (),
        }
        ProgressCallbackClient {
            client: self.inner.client().clone(),
            token: tk,
        }
    }
}

pub struct LSPStore<'a, 'l, const FULL: bool> {
    pub(crate) map: &'a mut HMap<UrlOrFile, DocData>,
    verbalizations: Option<&'a mut UnlockedVerbalizationTrie<'l>>,
    ignore_verbalizations: &'a [&'a str],
    cycles: Vec<DocumentUri>,
    snify: bool,
}
impl<'a, 'l, const FULL: bool> LSPStore<'a, 'l, FULL> {
    #[inline]
    pub const fn new(
        map: &'a mut HMap<UrlOrFile, DocData>,
        verbalizations: Option<&'a mut UnlockedVerbalizationTrie<'l>>,
        ignore_verbalizations: &'a [&'a str],
        snify: bool,
    ) -> Self {
        Self {
            map,
            cycles: Vec::new(),
            ignore_verbalizations,
            verbalizations,
            snify,
        }
    }

    pub fn load(&mut self, p: &Path, uri: &DocumentUri) -> Option<STeXParseData> {
        let text = std::fs::read_to_string(p).ok()?;
        let r = flams_stex::quickparse::stex::quickparse(uri, &text, p, &AnyBackend::Global, self)
            .lock();
        Some(r)
    }

    fn load_as_false(&mut self, p: &Path, uri: &DocumentUri) -> Option<STeXParseData> {
        if !FULL {
            self.load(p, uri)
        } else {
            let mut nstore = if let Some(verb) = &mut self.verbalizations {
                LSPStore::<'_, '_, false>::new(self.map, Some(verb), &[], false)
            } else {
                LSPStore::<'_, '_, false>::new(self.map, None, &[], false)
            };
            nstore.cycles = std::mem::take(&mut self.cycles);
            let r = nstore.load(p, uri);
            self.cycles = nstore.cycles;
            r
        }
    }
}

impl<const FULL: bool> STeXModuleStore for &mut LSPStore<'_, '_, FULL> {
    const FULL: bool = FULL;
    fn get_module(
        &mut self,
        module: &ModuleReference,
        _in_path: Option<&std::sync::Arc<Path>>,
    ) -> Result<STeXParseData, GetModuleError> {
        let Some(p) = module.full_path.as_ref() else {
            return Err(GetModuleError::NotFound(module.uri.clone()));
        };
        let uri = &module.in_doc;
        if let Some(i) = self.cycles.iter().position(|u| u == uri) {
            let mut ret = self.cycles[i..].to_vec();
            ret.push(uri.clone());
            return Err(GetModuleError::Cycle(ret));
        }

        macro_rules! do_return {
            ($e:expr) => {{
                /*if TRACK_DEPS {
                  if let Some(in_path) = in_path {
                    if module.uri != *flams_ontology::metatheory::URI {
                      $e.lock().dependents.insert_clone(in_path);
                    }
                  }
                }*/
                return Ok($e);
            }};
        }

        let lsp_uri = UrlOrFile::File(p.clone());
        //let lsp_uri = lsp::Url::from_file_path(p).map_err(|_| GetModuleError::NotFound(module.uri.clone()))?;
        match self.map.get(&lsp_uri).as_deref() {
            Some(DocData::Data(d, _)) => do_return!(d.clone()),
            Some(DocData::Doc(d)) if d.is_up_to_date() => do_return!(d.annotations.clone()),
            _ => (),
        }

        self.cycles.push(uri.clone());
        let r = self
            .load_as_false(p, uri)
            .inspect(|ret| match self.map.entry(lsp_uri) {
                Entry::Vacant(e) => {
                    e.insert(DocData::Data(ret.clone(), FULL));
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().merge(DocData::Data(ret.clone(), FULL));
                }
            });
        /*if TRACK_DEPS {
          if let Some(r) = &r {
            if let Some(in_path) = in_path {
              if module.uri != *flams_ontology::metatheory::URI {
                r.lock().dependencies.insert_clone(in_path);
              }
            }
          }
        }*/
        self.cycles.pop();
        r.ok_or_else(|| GetModuleError::NotFound(module.uri.clone()))
    }
    fn add_verbalization(
        &mut self,
        s: &str,
        symbol: &ftml_uris::prelude::SymbolUri,
        language: Language,
    ) {
        if let Some(verb) = &mut self.verbalizations {
            verb.insert(language, s, symbol);
        }
    }
    fn add_text<Pos: flams_utils::sourcerefs::StringPosition>(
        &self,
        r: StringRange<Pos>,
        text: &str,
        language: Language,
        needs_usemodule: &dyn Fn(&SymbolUri) -> bool,
    ) -> Option<flams_stex::quickparse::stex::structs::STeXToken<Pos>> {
        //Vec<(StringRange<Pos>, SmallVec<SymbolUri, 1>)> {
        if self.snify
            && Pos::KIND == PositionKind::LineColUtf16
            && Self::FULL
            && let Some(verb) = &self.verbalizations
        {
            let v = verb.find_all_in(
                language,
                text,
                r.start,
                self.ignore_verbalizations,
                needs_usemodule,
            );
            if v.is_empty() {
                None
            } else {
                Some(flams_stex::quickparse::stex::structs::STeXToken::Vec(
                    v.into_iter()
                        .map(|(r, s)| {
                            flams_stex::quickparse::stex::structs::STeXToken::SnifySuggestion {
                                range: r,
                                symbols: s,
                            }
                        })
                        .collect(),
                ))
            }
        } else {
            None
        }
    }
}

pub trait IsLSPRange {
    fn into_range(self) -> lsp::Range;
    fn from_range(range: lsp::Range) -> Self;
}

impl IsLSPRange for StringRange<LSPLineCol> {
    fn into_range(self) -> lsp::Range {
        lsp::Range {
            start: lsp::Position {
                line: self.start.line,
                character: self.start.col,
            },
            end: lsp::Position {
                line: self.end.line,
                character: self.end.col,
            },
        }
    }
    fn from_range(range: lsp::Range) -> Self {
        Self {
            start: LSPLineCol::new(range.start.line, range.start.character),
            end: LSPLineCol::new(range.end.line, range.end.character),
        }
    }
}

pub struct ProgressCallbackServer {
    client: ClientSocket,
    token: u32,
    handle: tokio::task::JoinHandle<()>,
    progress: Option<parking_lot::Mutex<(u32, u32)>>,
}

lazy_static::lazy_static! {
  static ref TOKEN:triomphe::Arc<std::sync::atomic::AtomicU32> = triomphe::Arc::new(std::sync::atomic::AtomicU32::new(42));
}

impl ProgressCallbackServer {
    #[inline]
    pub const fn client_mut(&mut self) -> &mut ClientSocket {
        &mut self.client
    }

    #[inline]
    pub fn client(&self) -> ClientSocket {
        self.client.clone()
    }

    pub fn with<R>(
        client: ClientSocket,
        title: String,
        total: Option<u32>,
        f: impl FnOnce(Self) -> R,
    ) -> R {
        let slf = Self::new(client, title, total);
        f(slf)
    }

    #[must_use]
    #[allow(clippy::let_underscore_future)]
    pub fn new(mut client: ClientSocket, title: String, total: Option<u32>) -> Self {
        let token = TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let f = client.work_done_progress_create(lsp::WorkDoneProgressCreateParams {
            token: lsp::NumberOrString::Number(token as _),
        });
        let mut c = client.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = f.await {
                tracing::error!("Error: {}", e);
            } else {
                let _ = c.progress(async_lsp::lsp_types::ProgressParams {
                    token: async_lsp::lsp_types::ProgressToken::Number(token as _),
                    value: async_lsp::lsp_types::ProgressParamsValue::WorkDone(
                        async_lsp::lsp_types::WorkDoneProgress::Begin(
                            async_lsp::lsp_types::WorkDoneProgressBegin {
                                message: None,
                                title,
                                percentage: total.map(|_| 0),
                                cancellable: None,
                            },
                        ),
                    ),
                });
            }
        });
        //tracing::info!("New progress with id {token}");
        Self {
            client,
            token,
            handle,
            progress: total.map(|i| parking_lot::Mutex::new((0, i))),
        }
    }

    pub fn update(&self, message: String, add_step: Option<u32>) {
        let (message, percentage) = if let Some(i) = add_step {
            if let Some(lock) = self.progress.as_ref() {
                let mut lock = lock.lock();
                lock.0 += i;
                (
                    format!("{}/{}:{message}", lock.0, lock.1),
                    Some(100 * lock.0 / lock.1),
                )
            } else {
                (message, None)
            }
        } else if let Some(lock) = self.progress.as_ref() {
            let lock = lock.lock();
            (
                format!("{}/{}:{message}", lock.0, lock.1),
                Some(100 * lock.0 / lock.1),
            )
        } else {
            (message, None)
        };
        let token = async_lsp::lsp_types::ProgressToken::Number(self.token as _);
        //tracing::info!("updating progress {}",self.token);
        while !self.handle.is_finished() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = self
            .client
            .clone()
            .progress(async_lsp::lsp_types::ProgressParams {
                token,
                value: async_lsp::lsp_types::ProgressParamsValue::WorkDone(
                    async_lsp::lsp_types::WorkDoneProgress::Report(
                        async_lsp::lsp_types::WorkDoneProgressReport {
                            message: Some(message),
                            percentage,
                            cancellable: None,
                        },
                    ),
                ),
            });
    }
}

impl Drop for ProgressCallbackServer {
    fn drop(&mut self) {
        let token = async_lsp::lsp_types::ProgressToken::Number(self.token as _);
        let _ = self.client.progress(async_lsp::lsp_types::ProgressParams {
            token,
            value: async_lsp::lsp_types::ProgressParamsValue::WorkDone(
                async_lsp::lsp_types::WorkDoneProgress::End(
                    async_lsp::lsp_types::WorkDoneProgressEnd {
                        message: Some("done".to_string()),
                    },
                ),
            ),
        });
    }
}

pub struct ProgressCallbackClient {
    client: ClientSocket,
    token: async_lsp::lsp_types::ProgressToken,
}

impl ProgressCallbackClient {
    pub fn finish(mut self) {
        let _ = self.client.progress(async_lsp::lsp_types::ProgressParams {
            token: self.token,
            value: async_lsp::lsp_types::ProgressParamsValue::WorkDone(
                async_lsp::lsp_types::WorkDoneProgress::End(
                    async_lsp::lsp_types::WorkDoneProgressEnd {
                        message: Some("done".to_string()),
                    },
                ),
            ),
        });
    }

    #[allow(clippy::let_underscore_future)]
    pub fn finish_delay(self) {
        let _ = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs_f32(0.5)).await;
            self.finish();
        });
    }
}
