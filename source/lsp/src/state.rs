use std::{hint::unreachable_unchecked, path::Path};

use crate::Entry;
use async_lsp::{ClientSocket, LanguageClient, lsp_types as lsp};
use flams_ftml::FtmlResult;
use flams_math_archives::{
    Archive, MathArchive,
    backend::{AnyBackend, GlobalBackend, HTMLData, TemporaryBackend},
    source_files::SourceEntry,
};
use flams_stex::{
    OutputCont, RusTeX,
    quickparse::stex::{DiagnosticLevel, STeXDiagnostic, STeXParseData, STeXParseDataI},
};
use flams_utils::{
    impossible,
    //prelude::HMap,
    sourcerefs::{LSPLineCol, StringRange},
};
use ftml_ontology::{
    domain::{declarations::symbols::Symbol, modules::Module},
    narrative::{
        documents::Document,
        elements::{DocumentTerm, LogicalParagraph, VariableDeclaration},
    },
    terms::TermContainer,
    utils::RefTree,
};
use ftml_solver::results::{
    CheckResult, ContentCheckResult, DocumentCheckResult, SymbolCheckResult,
};
use ftml_uris::DocumentUri;
use smallvec::SmallVec;

use crate::{
    ClientExt, LSPStore, ProgressCallbackServer, annotations::to_diagnostic,
    documents::LSPDocument, verbalizations::VerbalizationTrie,
};

#[derive(Clone)]
pub enum DocData {
    Doc(LSPDocument),
    Data(STeXParseData, bool),
}
impl DocData {
    pub fn merge(&mut self, other: Self) {
        fn merge_a(from: &mut STeXParseDataI, to: &mut STeXParseDataI) {
            to.dependencies = std::mem::take(&mut from.dependencies);
            /*for dep in std::mem::take(&mut from.dependents) {
              to.dependents.insert(dep);
            }*/
            for d in std::mem::take(&mut from.diagnostics) {
                to.diagnostics.insert(d);
            }
        }
        match (self, other) {
            (Self::Doc(d1), Self::Doc(d2)) => {
                //merge_a(&mut d1.annotations.lock(),&mut d2.annotations.lock());
                *d1 = d2;
            }
            (Self::Doc(d1), Self::Data(d2, _)) => {
                merge_a(&mut d2.lock(), &mut d1.annotations.lock());
            }
            (d2 @ Self::Data(_, _), Self::Doc(d1)) => {
                {
                    let Self::Data(d2, _) = d2 else { impossible!() };
                    merge_a(&mut d2.lock(), &mut d1.annotations.lock());
                }
                *d2 = Self::Doc(d1);
            }
            (d1 @ Self::Data(_, false), Self::Data(d2, true)) => {
                {
                    let Self::Data(_, _) = d1 else { impossible!() };
                    //merge_a(&mut d1.lock(),&mut d2.lock());
                }
                *d1 = Self::Data(d2, true);
            }
            (Self::Data(d1, _), Self::Data(d2, _)) => {
                merge_a(&mut d2.lock(), &mut d1.lock());
            }
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum UrlOrFile {
    Url(lsp::Url),
    File(std::sync::Arc<Path>),
}
impl UrlOrFile {
    pub fn name(&self) -> &str {
        match self {
            Self::Url(u) => u.path().split('/').next_back().unwrap_or(""),
            Self::File(p) => p.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        }
    }
}
impl From<lsp::Url> for UrlOrFile {
    fn from(value: lsp::Url) -> Self {
        match value.to_file_path() {
            Ok(p) => Self::File(p.into()),
            Err(()) => {
                tracing::error!("Not a file uri: {value}");
                Self::Url(value)
            }
        }
    }
}
impl Into<lsp::Url> for UrlOrFile {
    fn into(self) -> lsp::Url {
        match self {
            Self::Url(u) => u,
            Self::File(p) => lsp::Url::from_file_path(p).expect("this shouldn't happen"),
        }
    }
}
impl std::fmt::Display for UrlOrFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Url(u) => u.fmt(f),
            Self::File(p) => p.display().fmt(f),
        }
    }
}

#[derive(Default, Clone)]
pub struct LSPState {
    pub documents: triomphe::Arc<crate::LMap<UrlOrFile, DocData>>, //parking_lot::RwLock<HMap<UrlOrFile, DocData>>>,
    rustex: triomphe::Arc<std::sync::OnceLock<RusTeX>>,
    pub(crate) verbalizations: VerbalizationTrie, //backend: TemporaryBackend,
}
impl LSPState {
    #[inline]
    #[must_use]
    pub fn backend(&self) -> &TemporaryBackend {
        let AnyBackend::Temp(t) = flams_system::backend::backend() else {
            panic!("this is a bug")
        };
        t
    }

    #[must_use]
    #[inline]
    pub fn rustex(&self) -> &RusTeX {
        self.rustex.get_or_init(|| {
            RusTeX::get().unwrap_or_else(|()| {
                tracing::error!("Could not initialize RusTeX");
                panic!("Could not initialize RusTeX")
            })
        })
    }

    pub fn build_html(&self, uri: &UrlOrFile, client: &mut ClientSocket) -> Option<DocumentUri> {
        let Some(DocData::Doc(doc)) = self.documents.read().get(uri).cloned() else {
            return None;
        };
        let UrlOrFile::File(path) = uri else {
            return None;
        }; //.to_file_path().ok()?;
        let doc_uri = doc.document_uri().cloned()?;
        if doc.html_up_to_date() {
            return Some(doc_uri);
        };
        doc.relative_path()?;
        let engine = self
            .rustex()
            .builder()
            .set_sourcerefs(true)
            .set_font_debug_info(true);
        let engine = doc.with_text(|text| engine.set_string(path, text))?;
        ProgressCallbackServer::with(
            client.clone(),
            format!("Building {}", uri.name()),
            None,
            move |progress| {
                let out = ClientOutput(std::cell::RefCell::new(progress));
                let (mut res, old) = engine.set_output(out).run();
                doc.set_html_up_to_date();
                {
                    let mut lock = doc.annotations.lock();
                    for (fnt, dt) in &res.font_data {
                        if dt.web.is_none() {
                            lock.diagnostics.insert(STeXDiagnostic {
                                level: DiagnosticLevel::Warning,
                                message: format!("Unknown web font for {fnt}"),
                                range: StringRange::default(),
                            });
                            for (glyph, char) in &dt.missing.inner {
                                lock.diagnostics.insert(STeXDiagnostic {
                                    level: DiagnosticLevel::Warning,
                                    message: format!("unknown unicode character for glyph {char} ({glyph}) in font {fnt}"),
                                    range: StringRange::default()
                                });
                            }
                        }
                    }
                }
                //let progress: ClientOutput = old.take_output().unwrap_or_else(|| unreachable!());
                if let Some((e, ft)) = &mut res.error {
                    let mut done = None;
                    for ft in std::mem::take(ft) {
                        let url = UrlOrFile::File(ft.file.into());
                        if url == *uri {
                            done = Some(StringRange {
                                start: LSPLineCol::new(ft.line, 0),
                                end: LSPLineCol::new(ft.line, ft.col),
                            });
                        } else if let Some(dc) = self.documents.read().get(&url) {
                            let data = match dc {
                                DocData::Data(d, _) => d,
                                DocData::Doc(d) => &d.annotations,
                            };
                            let mut lock = data.lock();
                            lock.diagnostics.insert(STeXDiagnostic {
                                level: DiagnosticLevel::Error,
                                message: format!("RusTeX Error: {e}"),
                                range: StringRange {
                                    start: LSPLineCol::new(ft.line, 0),
                                    end: LSPLineCol::new(ft.line, ft.col),
                                },
                            });
                            let _ = client.publish_diagnostics(lsp::PublishDiagnosticsParams {
                                uri: url.clone().into(),
                                version: None,
                                diagnostics: lock.diagnostics.iter().map(to_diagnostic).collect(),
                            });
                        }
                    }
                    let mut lock = doc.annotations.lock();
                    lock.check = None;
                    lock.diagnostics.insert(STeXDiagnostic {
                        level: DiagnosticLevel::Error,
                        message: format!("RusTeX Error: {e}"),
                        range: done.unwrap_or_default(),
                    });
                    let _ = client.publish_diagnostics(lsp::PublishDiagnosticsParams {
                        uri: uri.clone().into(),
                        version: None,
                        diagnostics: lock.diagnostics.iter().map(to_diagnostic).collect(),
                    });
                    drop(lock);
                    None
                } else {
                    let html = res.to_string();
                    //let rel_path = doc.relative_path().unwrap_or_else(|| unreachable!());
                    match flams_system::logging::ignore_traces(|| {
                        flams_ftml::build_ftml(
                            &AnyBackend::Temp(self.backend().clone()),
                            &html,
                            doc_uri.clone(),
                        )
                    }) {
                        Ok(FtmlResult {
                            doc: docresult,
                            ftml,
                            css,
                            body,
                            inner_offset,
                            ..
                        }) => {
                            /*tracing::warn!(
                                "Adding HTML for {}\nSanity check: {:?}\n{:#?}",
                                docresult.document.uri,
                                &docresult.data[0..140],
                                docresult.document
                            );*/
                            self.backend().add_html(
                                docresult.document.uri.clone(),
                                HTMLData {
                                    html: ftml,
                                    css,
                                    body,
                                    inner_offset: inner_offset as _,
                                    refs: docresult.data,
                                },
                            );
                            self.backend()
                                .add_triples(&docresult.document.uri, docresult.triples);
                            self.backend().add_document(docresult.document.clone());
                            for m in &docresult.modules {
                                self.backend().add_module(m.clone());
                            }
                            old.memorize(self.rustex());
                            let mut checker = ftml_solver::Checker::<
                                ftml_solver::split::SingleThreadedSplit,
                            >::new(AnyBackend::Temp(
                                self.backend().clone(),
                            ));
                            let _ = checker.add_modules(docresult.modules);
                            let Ok((logs, mods)) = checker
                                .check_document(&docresult.document)
                                .inspect_err(|m| {
                                    let _ =
                                        client.publish_diagnostics(lsp::PublishDiagnosticsParams {
                                            uri: uri.clone().into(),
                                            version: None,
                                            diagnostics: vec![to_diagnostic(&STeXDiagnostic {
                                                level: DiagnosticLevel::Error,
                                                message: format!("Module {m} not found"),
                                                range: StringRange::default(),
                                            })],
                                        });
                                })
                            else {
                                return Some(doc_uri);
                            };
                            let mut lock = doc.annotations.lock();
                            lock.diagnostics
                                .0
                                .extend(check_diagnostics(&logs, (&docresult.document, &mods)));
                            lock.check = Some(logs);
                            if !lock.diagnostics.is_empty() {
                                let _ = client.publish_diagnostics(lsp::PublishDiagnosticsParams {
                                    uri: uri.clone().into(),
                                    version: None,
                                    diagnostics: lock
                                        .diagnostics
                                        .iter()
                                        .map(to_diagnostic)
                                        .collect(),
                                });
                            }
                            drop(lock);
                            for m in mods {
                                self.backend().add_module(m.clone());
                            }
                            Some(doc_uri)
                        }
                        Err(e) => {
                            let mut lock = doc.annotations.lock();
                            lock.check = None;
                            lock.diagnostics.insert(STeXDiagnostic {
                                level: DiagnosticLevel::Error,
                                message: format!("FTML Error: {e}"),
                                range: StringRange::default(),
                            });
                            let _ = client.publish_diagnostics(lsp::PublishDiagnosticsParams {
                                uri: uri.clone().into(),
                                version: None,
                                diagnostics: lock.diagnostics.iter().map(to_diagnostic).collect(),
                            });
                            drop(lock);
                            None
                        }
                    }
                }
            },
        )
    }

    #[inline]
    pub fn build_html_and_notify(&self, uri: &UrlOrFile, mut client: ClientSocket) {
        if let Some(uri) = self.build_html(uri, &mut client) {
            client.html_result(&uri);
        }
    }

    pub fn relint_dependents(self, path: std::sync::Arc<Path>) { /*
        let docs = self.documents.read();
        let mut deps = vec![UrlOrFile::File(path.clone())];
        for (k,v) in docs.iter() {
        if matches!(k,UrlOrFile::File(p) if p == path) { continue }
        let d = match v {
        DocData::Doc(d) => &d.annotations,
        DocData::Data(d,_) => d
        };
        let lock = d.lock();
        if lock.dependencies.contains(&path) {
        deps.push(k.clone());
        }
        } */
    }
    /*
    fn relint_dependents_i(&mut self,path:std::sync::Arc<Path>,&mut v:Vec<UrlOrFile>) {
      let docs = self.documents.read();
      let mut deps = vec![UrlOrFile::File(path.clone())];
      for (k,v) in docs.iter() {
        if matches!(k,UrlOrFile::File(p) if p == path) { continue }
        let d = match v {
          DocData::Doc(d) => &d.annotations,
          DocData::Data(d,_) => d
        };
        let lock = d.lock();
        if lock.dependencies.contains(&path) {
          deps.push(k.clone());
        }
      }
    } */

    pub fn load_mathhubs(&self, client: ClientSocket) {
        let (_, t) = ftml_ontology::utils::time::measure(move || {
            let mut files = Vec::new();

            for a in GlobalBackend.all_archives().iter() {
                if let Archive::Local(a) = a {
                    let mut v = Vec::new();
                    a.with_sources(|d| {
                        for e in d.dfs() {
                            match e {
                                SourceEntry::File(f) => {
                                    let uri = match DocumentUri::from_archive_relpath(
                                        a.uri().clone(),
                                        f.relative_path.as_ref(),
                                    ) {
                                        Ok(u) => u,
                                        Err(e) => {
                                            tracing::error!("{e}");
                                            continue;
                                        }
                                    };
                                    v.push((
                                        f.relative_path
                                            .steps()
                                            .fold(a.source_dir(), |p, s| p.join(s))
                                            .into(),
                                        uri,
                                    ))
                                }
                                _ => {}
                            }
                        }
                    });
                    files.push((a.id().clone(), v))
                }
            }

            ProgressCallbackServer::with(
                client,
                "Linting MathHub".to_string(),
                Some(files.len() as _),
                move |progress| {
                    self.load_all(
                        files
                            .into_iter()
                            .map(|(id, v)| {
                                progress.update(id.to_string(), Some(1));
                                v
                            })
                            .flatten(),
                        |file, data| {
                            let lock = data.lock();
                            if !lock.diagnostics.is_empty() {
                                if let Ok(uri) = lsp::Url::from_file_path(&file) {
                                    let _ = progress.client().clone().publish_diagnostics(
                                        lsp::PublishDiagnosticsParams {
                                            uri,
                                            version: None,
                                            diagnostics: lock
                                                .diagnostics
                                                .iter()
                                                .map(to_diagnostic)
                                                .collect(),
                                        },
                                    );
                                }
                            }
                        },
                    );
                },
            );
        });
        tracing::info!("Linting mathhubs finished after {t}");
    }

    pub fn load_all<I: IntoIterator<Item = (std::sync::Arc<Path>, DocumentUri)>>(
        &self,
        iter: I,
        mut and_then: impl FnMut(&std::sync::Arc<Path>, &STeXParseData),
    ) {
        let mut ndocs = crate::HMap::default();
        let verbalizations = VerbalizationTrie::default();
        let mut vlock = verbalizations.lock();
        let mut state = LSPStore::<true>::new(&mut ndocs, Some(&mut vlock), &[], false);
        for (p, uri) in iter {
            let p = UrlOrFile::File(p);
            if !state.map.contains_key(&p) {
                let UrlOrFile::File(p) = p else {
                    // SAFETY: we just constructed it such
                    unsafe { unreachable_unchecked() }
                };
                if let Some(ret) = state.load(p.as_ref(), &uri) {
                    and_then(&p, &ret);
                    let p = UrlOrFile::File(p);
                    match state.map.entry(p) {
                        Entry::Vacant(e) => {
                            e.insert(DocData::Data(ret, true));
                        }
                        Entry::Occupied(mut e) => {
                            e.get_mut().merge(DocData::Data(ret, true));
                        }
                    }
                }
            }
        }
        drop(vlock);
        self.verbalizations.merge(verbalizations);
        /*{
            use radix_trie::TrieCommon;
            use std::fmt::Write;
            let lck = self.verbalizations.0.lock();
            let mut s = String::new();
            for (k, v) in lck.iter() {
                writeln!(s, " - {k}: {v:?}");
            }
            drop(lck);
            tracing::warn!("{s}");
        }*/
        let docs = &mut self.documents.write();
        for (k, v) in ndocs {
            match docs.entry(k) {
                Entry::Vacant(e) => {
                    e.insert(v);
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().merge(v);
                }
            }
        }
    }

    /*
    pub fn load<const FULL: bool>(
        &self,
        p: std::sync::Arc<Path>,
        uri: &DocumentUri,
        and_then: impl FnOnce(&STeXParseData),
    ) {
        //let Some(lsp_uri) = lsp::Url::from_file_path(p).ok() else {return};
        let lsp_uri = UrlOrFile::File(p);
        let UrlOrFile::File(path) = &lsp_uri else {
            unreachable!()
        };
        let mut docs = self.documents.write();
        if docs.get(&lsp_uri).is_some() {
            return;
        }
        let mut vlock = self.verbalizations.lock();
        let mut state = LSPStore::<'_, '_, FULL>::new(&mut docs, Some(&mut vlock), true);
        if let Some(ret) = state.load(path, uri) {
            drop(state);
            drop(vlock);
            and_then(&ret);
            match docs.entry(lsp_uri) {
                Entry::Vacant(e) => {
                    e.insert(DocData::Data(ret, FULL));
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().merge(DocData::Data(ret, FULL));
                }
            }
        }
    }
     */

    #[allow(clippy::let_underscore_future)]
    pub fn insert(&self, uri: UrlOrFile, doctext: String) {
        let doc = self.documents.read().get(&uri).as_deref().cloned();
        match doc {
            Some(DocData::Doc(doc)) => {
                if doc.set_text(doctext) {
                    doc.compute_annots(self.clone(), false);
                }
            }
            _ => {
                let doc = LSPDocument::new(doctext, uri.clone());
                if doc.has_annots() {
                    doc.compute_annots(self.clone(), false);
                }
                match self.documents.write().entry(uri) {
                    Entry::Vacant(e) => {
                        e.insert(DocData::Doc(doc));
                    }
                    Entry::Occupied(mut e) => {
                        e.get_mut().merge(DocData::Doc(doc));
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn get(&self, uri: &UrlOrFile) -> Option<LSPDocument> {
        if let Some(DocData::Doc(doc)) = self.documents.read().get(uri) {
            Some(doc.clone())
        } else {
            None
        }
    }

    pub fn force_get(&self, uri: &UrlOrFile) -> Option<LSPDocument> {
        if let Some(DocData::Doc(doc)) = self.documents.read().get(uri) {
            return Some(doc.clone());
        }
        let UrlOrFile::File(f) = uri else { return None };
        let Some(s) = std::fs::read_to_string(f).ok() else {
            return None;
        };
        self.insert(uri.clone(), s);
        self.get(uri)
    }
}

struct ClientOutput(std::cell::RefCell<ProgressCallbackServer>);
impl OutputCont for ClientOutput {
    fn message(&self, _: String) {}
    fn errmessage(&self, text: String) {
        let _ = self
            .0
            .borrow_mut()
            .client_mut()
            .show_message(lsp::ShowMessageParams {
                typ: lsp::MessageType::ERROR,
                message: text,
            });
    }
    fn file_open(&self, text: String) {
        self.0.borrow().update(text, None);
    }
    fn file_close(&self, _text: String) {}
    fn write_16(&self, _text: String) {}
    fn write_17(&self, _text: String) {}
    fn write_18(&self, _text: String) {}
    fn write_neg1(&self, _text: String) {}
    fn write_other(&self, _text: String) {}

    #[inline]
    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

fn check_diagnostics(
    res: &DocumentCheckResult,
    src: (&Document, &[Module]),
) -> impl Iterator<Item = STeXDiagnostic> {
    use either_of::EitherOf5 as E;
    res.checks.iter().flat_map(|cr| match cr {
        CheckResult::Missing(u) => E::A(std::iter::once(STeXDiagnostic {
            level: DiagnosticLevel::Error,
            message: format!("Module {u} not found"),
            range: StringRange::default(),
        })),
        CheckResult::Variable(e, r) => {
            let vd = src.0.get_as::<VariableDeclaration>(e.name());
            if vd.is_none() {
                tracing::error!("Variable {e} not found!");
            }
            E::B(symbol_check_result(
                r,
                e.name().as_ref(),
                vd.as_ref().map(|v| &v.data.tp),
                vd.as_ref().map(|v| &v.data.df),
            ))
        }
        CheckResult::Proof(uri, res) if res.iter().any(|r| !r.success()) => {
            let prf = src.0.get_as::<LogicalParagraph>(uri.name());
            E::A(std::iter::once(STeXDiagnostic {
                level: DiagnosticLevel::Error,
                message: format!("Checking proof {uri} failed"),
                range: if let Some(prf) = prf {
                    conv_range(prf.source)
                } else {
                    StringRange::default()
                },
            }))
        }
        CheckResult::Proof(..) => E::C(std::iter::empty()),
        CheckResult::Term { uri, inferred, .. } => {
            if inferred.is_some() {
                E::C(std::iter::empty())
            } else {
                let term = src.0.get_as::<DocumentTerm>(uri.name());
                if term.is_none() {
                    tracing::error!("Term {uri} not found!");
                }
                let range = term.map(|t| conv_range(t.term.source)).unwrap_or_default();
                E::A(std::iter::once(STeXDiagnostic {
                    level: DiagnosticLevel::Info,
                    message: format!("checking term {} failed", uri.name()),
                    range,
                }))
            }
        }
        CheckResult::Content(ccr) => match ccr {
            ContentCheckResult::Symbol(uri, res) => {
                let uri = uri.as_simple_module();
                let sym = src
                    .1
                    .iter()
                    .find(|m| m.uri == uri.module)
                    .and_then(|m| m.get_as::<Symbol>(uri.name()));
                if sym.is_none() {
                    tracing::error!("Symbol {uri} not found!");
                }
                E::D(
                    symbol_check_result(
                        res,
                        uri.name().as_ref(),
                        sym.as_ref().map(|v| &v.data.tp),
                        sym.as_ref().map(|v| &v.data.df),
                    )
                    .collect::<SmallVec<_, 1>>()
                    .into_iter(),
                )
            }
        },
        CheckResult::Module { checks, .. } => E::E(checks.iter().flat_map(|ccr| match ccr {
            ContentCheckResult::Symbol(uri, res) => {
                let uri = uri.as_simple_module();
                let sym = src
                    .1
                    .iter()
                    .find(|m| m.uri == uri.module)
                    .and_then(|m| m.get_as::<Symbol>(uri.name()));
                if sym.is_none() {
                    tracing::error!("Symbol {uri} not found!");
                }
                symbol_check_result(
                    res,
                    uri.name().as_ref(),
                    sym.as_ref().map(|v| &v.data.tp),
                    sym.as_ref().map(|v| &v.data.df),
                )
                .collect::<SmallVec<_, 1>>()
                .into_iter()
            }
        })),
    })
}

fn symbol_check_result(
    result: &SymbolCheckResult,
    name: &str,
    tp: Option<&TermContainer>,
    df: Option<&TermContainer>,
) -> impl Iterator<Item = STeXDiagnostic> + use<> {
    use either_of::Either::{Left as A, Right as B};
    match result {
        SymbolCheckResult::TypeOnly { result } => {
            if result.success {
                A(std::iter::empty())
            } else {
                let range = tp.map(|v| conv_range(v.source)).unwrap_or_default();
                B(std::iter::once(STeXDiagnostic {
                    level: DiagnosticLevel::Info,
                    message: format!("checking type of {name} failed"),
                    range,
                }))
            }
        }
        SymbolCheckResult::DefiniensOnly { inferred, .. } => {
            if inferred.is_some() {
                A(std::iter::empty())
            } else {
                let range = df.map(|v| conv_range(v.source)).unwrap_or_default();
                B(std::iter::once(STeXDiagnostic {
                    level: DiagnosticLevel::Info,
                    message: format!("checking definiens of {name} failed ({range:?})"),
                    range,
                }))
            }
        }
        SymbolCheckResult::Both { matches: None, .. } => {
            let range = tp.map(|v| conv_range(v.source)).unwrap_or_default();
            B(std::iter::once(STeXDiagnostic {
                level: DiagnosticLevel::Info,
                message: format!("checking type of {name} failed ({range:?})"),
                range,
            }))
        }
        SymbolCheckResult::Both {
            matches: Some(matches),
            ..
        } => {
            if matches.success {
                return A(std::iter::empty());
            }
            let df_range = df.map(|v| conv_range(v.source)).unwrap_or_default();
            B(std::iter::once(STeXDiagnostic {
                level: DiagnosticLevel::Info,
                message: format!("checking definiens of {name} failed"),
                range: df_range,
            }))
        }
    }
}

const fn conv_range(
    ftml_ontology::utils::SourceRange { start, end }: ftml_ontology::utils::SourceRange,
) -> StringRange<LSPLineCol> {
    StringRange {
        start: LSPLineCol::new(start.line, start.col),
        end: LSPLineCol::new(end.line, end.col),
    }
}
