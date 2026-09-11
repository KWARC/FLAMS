use crate::{
    Archive,
    backend::{AnyBackend, GlobalBackend, LocalBackend},
    manager::ArchiveOrGroup,
    utils::{AsyncEngine, errors::BackendError},
};
use ftml_ontology::{
    domain::modules::{Module, ModuleLike},
    narrative::{DocDataRef, DocumentRange, documents::Document, elements::Notation},
    utils::Css,
};
use ftml_uris::{ArchiveId, DocumentElementUri, DocumentUri, ModuleUri, SymbolUri, UriPath};

#[derive(Debug)]
pub struct HTMLData {
    pub html: Box<str>,
    pub css: Box<[Css]>,
    pub body: DocumentRange,
    pub inner_offset: usize,
    pub refs: Box<[u8]>,
}

#[derive(Clone, Debug)]
pub struct TemporaryBackend {
    inner: triomphe::Arc<TemporaryBackendI>,
}
impl Default for TemporaryBackend {
    #[inline]
    fn default() -> Self {
        Self::new(AnyBackend::Global)
    }
}

#[derive(Debug)]
struct TemporaryBackendI {
    modules: dashmap::DashMap<ModuleUri, Module, rustc_hash::FxBuildHasher>,
    documents: dashmap::DashMap<DocumentUri, Document, rustc_hash::FxBuildHasher>,
    html: dashmap::DashMap<DocumentUri, HTMLData, rustc_hash::FxBuildHasher>,
    parent: AnyBackend,
}

impl TemporaryBackend {
    pub fn reset<A: AsyncEngine>(&self) {
        self.inner.modules.clear();
        self.inner.documents.clear();
        GlobalBackend.reset::<A>(true);
    }

    #[must_use]
    pub fn new(parent: AnyBackend) -> Self {
        Self {
            inner: triomphe::Arc::new(TemporaryBackendI {
                modules: dashmap::DashMap::default(),
                documents: dashmap::DashMap::default(),
                html: dashmap::DashMap::default(),
                parent,
            }),
        }
    }
    pub fn add_module(&self, m: Module) {
        self.inner.modules.insert(m.uri.clone(), m);
    }
    pub fn add_document(&self, d: Document) {
        self.inner.documents.insert(d.uri.clone(), d);
    }
    pub fn add_html(&self, uri: DocumentUri, d: HTMLData) {
        self.inner.html.insert(uri, d);
    }

    #[cfg(feature = "rdf")]
    #[inline]
    pub fn add_triples(&self, doc: &DocumentUri, triples: Vec<ulo::rdf_types::Triple>) {
        self.inner.parent.add_triples(doc, triples);
    }
}

impl LocalBackend for TemporaryBackend {
    type ArchiveIter<'a> = <AnyBackend as LocalBackend>::ArchiveIter<'a>;

    #[inline]
    fn save(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        log: crate::artifacts::FileOrString,
        from: crate::formats::BuildTargetId,
        result: Option<Box<dyn crate::artifacts::Artifact>>,
    ) -> std::result::Result<(), crate::utils::errors::ArtifactSaveError> {
        self.inner.parent.save(in_doc, rel_path, log, from, result)
    }

    fn get_document(&self, uri: &DocumentUri) -> Result<Document, BackendError> {
        self.inner.documents.get(uri).map_or_else(
            || {
                //tracing::info!("Getting document globally");
                self.inner.parent.get_document(uri)
            },
            |e| {
                //tracing::info!("Got document locally");
                Ok(e.value().clone())
            },
        )
    }

    fn get_document_async<A: AsyncEngine>(
        &self,
        uri: &DocumentUri,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Document, BackendError>> + Send>>
    where
        Self: Sized,
    {
        if let Some(d) = self.inner.documents.get(uri) {
            //tracing::info!("Got document locally");
            return Box::pin(std::future::ready(Ok(d.value().clone()))) as _;
        }
        //tracing::info!("Getting document globally");
        Box::pin(self.inner.parent.get_document_async::<A>(uri)) as _
    }

    fn get_module(&self, uri: &ModuleUri) -> Result<ModuleLike, BackendError> {
        if uri.is_top() {
            self.inner.modules.get(uri).map_or_else(
                || self.inner.parent.get_module(uri),
                |e| Ok(ModuleLike::Module(e.value().clone())),
            )
        } else {
            // SAFETY: !is_top()
            let SymbolUri { name, module } =
                unsafe { uri.clone().into_symbol().unwrap_unchecked() };
            let Some(m) = self.inner.modules.get(&module) else {
                return self.inner.parent.get_module(uri);
            };
            m.as_module_like(&name)
                .ok_or_else(|| BackendError::NotFound(SymbolUri { name, module }.into()))
        }
    }

    fn get_module_async<A: AsyncEngine>(
        &self,
        uri: &ModuleUri,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<ModuleLike, BackendError>> + Send>>
    where
        Self: Sized,
    {
        if uri.is_top() {
            if let Some(m) = self.inner.modules.get(uri) {
                Box::pin(std::future::ready(Ok(ModuleLike::Module(
                    m.value().clone(),
                ))))
            } else {
                Box::pin(self.inner.parent.get_module_async::<A>(uri)) as _
            }
        } else {
            // SAFETY: !is_top()
            let SymbolUri { name, module } =
                unsafe { uri.clone().into_symbol().unwrap_unchecked() };
            if let Some(m) = self.inner.modules.get(&module) {
                let r = m
                    .as_module_like(&name)
                    .ok_or_else(|| BackendError::NotFound(SymbolUri { name, module }.into()));
                Box::pin(std::future::ready(r)) as _
            } else {
                Box::pin(self.inner.parent.get_module_async::<A>(&uri)) as _
            }
        }
    }

    #[inline]
    fn with_archive_or_group<R>(
        &self,
        id: &ArchiveId,
        f: impl FnOnce(Option<&ArchiveOrGroup>) -> R,
    ) -> R
    where
        Self: Sized,
    {
        self.inner.parent.with_archive_or_group(id, f)
    }

    #[inline]
    fn with_archive<R>(&self, id: &ArchiveId, f: impl FnOnce(Option<&Archive>) -> R) -> R
    where
        Self: Sized,
    {
        self.inner.parent.with_archive(id, f)
    }

    #[inline]
    fn with_archives<R>(&self, f: impl FnOnce(Self::ArchiveIter<'_>) -> R) -> R
    where
        Self: Sized,
    {
        self.inner.parent.with_archives(f)
    }

    fn get_html_body(&self, d: &DocumentUri) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.inner.html.get(d).map_or_else(
            || self.inner.parent.get_html_body(d),
            |html| {
                Ok((
                    html.css.clone(),
                    html.html[html.body.start..html.body.end]
                        .to_string()
                        .into_boxed_str(),
                ))
            },
        )
    }

    fn get_html_body_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentUri,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
                + Send,
        >,
    >
    where
        Self: Sized,
    {
        if let Some(html) = self.inner.html.get(uri) {
            return Box::pin(std::future::ready(Ok((
                html.css.clone(),
                html.html[html.body.start..html.body.end]
                    .to_string()
                    .into_boxed_str(),
            )))) as _;
        }
        Box::pin(self.inner.parent.get_html_body_async::<A>(uri)) as _
    }

    fn get_html_body_inner(&self, d: &DocumentUri) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.inner.html.get(d).map_or_else(
            || self.inner.parent.get_html_body_inner(d),
            |html| {
                Ok((
                    html.css.clone(),
                    html.html[html.body.start + html.inner_offset..html.body.end - "</body>".len()]
                        .to_string()
                        .into_boxed_str(),
                ))
            },
        )
    }

    fn get_html_body_inner_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentUri,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
                + Send,
        >,
    >
    where
        Self: Sized,
    {
        if let Some(html) = self.inner.html.get(uri) {
            return Box::pin(std::future::ready(Ok((
                html.css.clone(),
                html.html[html.body.start + html.inner_offset..html.body.end - "</body>".len()]
                    .to_string()
                    .into_boxed_str(),
            )))) as _;
        }
        Box::pin(self.inner.parent.get_html_body_inner_async::<A>(uri)) as _
    }

    fn get_html_full(&self, d: &DocumentUri) -> Result<Box<str>, BackendError> {
        self.inner.html.get(d).map_or_else(
            || self.inner.parent.get_html_full(d),
            |html| Ok(html.html.clone()),
        )
    }

    fn get_html_fragment(
        &self,
        d: &DocumentUri,
        range: DocumentRange,
    ) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.inner.html.get(d).map_or_else(
            || self.inner.parent.get_html_fragment(d, range),
            |html| {
                Ok((
                    html.css.clone(),
                    html.html[range.start..range.end]
                        .to_string()
                        .into_boxed_str(),
                ))
            },
        )
    }

    fn get_html_fragment_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentUri,
        range: ftml_ontology::narrative::DocumentRange,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
                + Send,
        >,
    > {
        if let Some(html) = self.inner.html.get(uri) {
            return Box::pin(std::future::ready(Ok((
                html.css.clone(),
                html.html[html.body.start + html.inner_offset..html.body.end]
                    .to_string()
                    .into_boxed_str(),
            ))));
        }
        Box::pin(self.inner.parent.get_html_fragment_async::<A>(uri, range)) as _
    }

    fn get_reference<T: bincode::Decode<()>>(&self, rf: &DocDataRef<T>) -> Result<T, BackendError>
    where
        Self: Sized,
    {
        let Some(html) = self.inner.html.get(&rf.in_doc) else {
            return self.inner.parent.get_reference(rf);
        };

        let Some(bytes) = html.refs.get(rf.start..rf.end) else {
            return Err(BackendError::OutOfRangeError(rf.start, rf.end));
        };
        //tracing::warn!("Gettin ({},{}) from {}", rf.start, rf.end, rf.in_doc);
        //tracing::warn!("Here: {}\n{bytes:?}", String::from_utf8_lossy(bytes));
        let (r, _) = bincode::decode_from_slice(bytes, bincode::config::standard())?;
        Ok(r)
    }

    #[cfg(feature = "rdf")]
    #[inline]
    fn get_notations<E: AsyncEngine>(
        &self,
        uri: &SymbolUri,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)>
    where
        Self: Sized,
    {
        use ftml_uris::FtmlUri;

        GlobalBackend
            .query_notations::<E, ftml_ontology::narrative::elements::notations::NotationReference>(
                uri.to_iri(),
                self,
                |n| n.notation,
            )
        //self.inner.parent.get_notations::<E>(uri)
    }

    #[cfg(feature = "rdf")]
    #[inline]
    fn get_var_notations<E: AsyncEngine>(
        &self,
        uri: &DocumentElementUri,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)>
    where
        Self: Sized,
    {
        use ftml_uris::FtmlUri;

        GlobalBackend
            .query_notations::<E, ftml_ontology::narrative::elements::notations::VariableNotationReference>(
                uri.to_iri(),
                self,
                |n| n.notation,
            )
        //self.inner.parent.get_var_notations::<E>(uri)
    }
}
