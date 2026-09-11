use std::path::Path;

#[cfg(feature = "rdf")]
use ftml_ontology::narrative::{
    DataRef, SharedDocumentElement,
    elements::{IsDocumentElement, Notation},
};
use ftml_ontology::{
    domain::modules::{Module, ModuleLike},
    narrative::{DocDataRef, DocumentRange, documents::Document},
    utils::Css,
};
#[cfg(feature = "rdf")]
use ftml_uris::DocumentElementUri;
use ftml_uris::{
    ArchiveId, DocumentUri, IsNarrativeUri, ModuleUri, NamedUri, SymbolUri, UriPath,
    UriWithArchive, UriWithPath,
};
use futures_util::TryFutureExt;

use crate::{
    Archive, ExternalArchive, LocallyBuilt,
    backend::LocalBackend,
    document_file::DocumentFile,
    manager::{ArchiveManager, ArchiveOrGroup},
    utils::{
        AsyncEngine,
        errors::{ArtifactSaveError, BackendError},
    },
};

#[cfg(feature = "rocksdb")]
static RDF_PATH: std::sync::Mutex<Option<Box<Path>>> = std::sync::Mutex::new(None);

#[cfg(not(feature = "rocksdb"))]
static GLOBAL: std::sync::LazyLock<ArchiveManager> =
    std::sync::LazyLock::new(ArchiveManager::default);

#[cfg(feature = "rocksdb")]
static GLOBAL: std::sync::LazyLock<ArchiveManager> = std::sync::LazyLock::new(|| {
    if let Some(p) = RDF_PATH.lock().expect("could not access RDF_PATH").as_ref() {
        ArchiveManager::new(p)
    } else {
        ArchiveManager::default()
    }
});

#[cfg(feature = "rocksdb")]
pub fn set_global(rdf_path: &Path) {
    *RDF_PATH.lock().expect("could not access RDF_PATH") =
        Some(rdf_path.to_path_buf().into_boxed_path());
}

#[derive(Debug, Copy, Clone)]
pub struct GlobalBackend;
impl std::ops::Deref for GlobalBackend {
    type Target = ArchiveManager;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &GLOBAL
    }
}

impl GlobalBackend {
    #[inline]
    #[must_use]
    pub fn get(&self) -> &'static ArchiveManager {
        &GLOBAL
    }
    pub fn initialize<A: AsyncEngine>(rdf: bool) {
        Self.load(crate::mathhub::mathhubs());
        #[cfg(feature = "rdf")]
        {
            if rdf {
                A::background(|| Self.triple_store().load_archives(&Self.all_archives()));
            }
        }
    }

    pub fn reset<A: AsyncEngine>(self, rdf: bool) {
        self.reinit(|_| (), crate::mathhub::mathhubs());
        #[cfg(feature = "rdf")]
        {
            if rdf {
                A::background(|| Self.triple_store().load_archives(&Self.all_archives()));
            }
        }
    }
}

impl LocalBackend for ArchiveManager {
    type ArchiveIter<'a>
        = &'a [Archive]
    where
        Self: Sized;

    fn save(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        log: crate::artifacts::FileOrString,
        from: crate::formats::BuildTargetId,
        result: Option<Box<dyn crate::artifacts::Artifact>>,
    ) -> std::result::Result<(), crate::utils::errors::ArtifactSaveError> {
        #[cfg(feature = "cached")]
        {
            if let Some(r) = result.as_ref() {
                if let Some(r) = r.as_any().downcast_ref::<crate::artifacts::ContentUpdate>() {
                    if let Some(d) = &r.document {
                        self.documents.remove(&d.uri);
                    }
                    for m in &r.modules {
                        self.modules.remove(&m.uri);
                    }
                } else if let Some(r) = r.as_any().downcast_ref::<crate::artifacts::ContentResult>()
                {
                    self.documents.remove(&r.document.uri);
                    for m in &r.modules {
                        self.modules.remove(&m.uri);
                    }
                }
            }
        }
        self.with_buildable_archive(in_doc.archive_id(), |a| {
            let Some(a) = a else {
                return Err(ArtifactSaveError::NoArchive);
            };
            a.save(
                in_doc,
                rel_path,
                log,
                from,
                result,
                #[cfg(feature = "rdf")]
                self.triple_store(),
                #[cfg(feature = "rdf")]
                true,
            )
        })
    }

    fn with_archive<R>(&self, id: &ArchiveId, f: impl FnOnce(Option<&Archive>) -> R) -> R {
        let tree = self.tree.read();
        f(tree.get(id))
    }

    fn with_archives<R>(&self, f: impl FnOnce(Self::ArchiveIter<'_>) -> R) -> R
    where
        Self: Sized,
    {
        f(&self.all_archives())
    }

    fn with_archive_or_group<R>(
        &self,
        id: &ArchiveId,
        f: impl FnOnce(Option<&ArchiveOrGroup>) -> R,
    ) -> R
    where
        Self: Sized,
    {
        self.with_tree(|t| f(t.get_group_or_archive(id)))
    }

    fn get_document(&self, uri: &DocumentUri) -> Result<Document, BackendError> {
        self.with_doc(
            uri,
            |docfile| docfile.get_document().map_err(Into::into),
            |o| todo!(),
        )
    }

    fn get_document_async<A: AsyncEngine>(
        &self,
        uri: &DocumentUri,
    ) -> impl Future<Output = Result<Document, BackendError>> + Send + use<A>
    where
        Self: Sized,
    {
        self.with_doc_async::<A, _, _, _, _, _>(
            uri,
            |docfile| async move { docfile.get_document_async::<A>().await.map_err(Into::into) },
            |o| std::future::ready(todo!()),
        )
    }

    fn get_html_full(&self, uri: &DocumentUri) -> Result<Box<str>, BackendError> {
        self.with_doc(
            uri,
            |docfile| docfile.get_html().map_err(Into::into),
            |o| todo!(),
        )
    }

    fn get_html_body(&self, uri: &DocumentUri) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.with_doc(
            uri,
            |docfile| {
                docfile
                    .get_html_body()
                    .map_err(Into::into)
                    .map(|s| (docfile.get_css(), s))
            },
            |o| todo!(),
        )
    }

    fn get_html_body_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentUri,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<A>
    where
        Self: Sized,
    {
        self.with_doc_async::<A, _, _, _, _, _>(
            uri,
            |docfile| {
                A::block_on(move || {
                    docfile
                        .get_html_body()
                        .map_err(Into::into)
                        .map(|s| (docfile.get_css(), s))
                })
            },
            |o| std::future::ready(todo!()),
        )
    }

    fn get_html_body_inner(
        &self,
        uri: &DocumentUri,
    ) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.with_doc(
            uri,
            |docfile| {
                docfile
                    .get_html_body_inner()
                    .map_err(Into::into)
                    .map(|s| (docfile.get_css(), s))
            },
            |o| todo!(),
        )
    }

    fn get_html_body_inner_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentUri,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<A>
    where
        Self: Sized,
    {
        self.with_doc_async::<A, _, _, _, _, _>(
            uri,
            |docfile| {
                A::block_on(move || {
                    docfile
                        .get_html_body_inner()
                        .map_err(Into::into)
                        .map(|s| (docfile.get_css(), s))
                })
            },
            |o| std::future::ready(todo!()),
        )
    }

    fn get_html_fragment(
        &self,
        uri: &DocumentUri,
        range: DocumentRange,
    ) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.with_doc(
            uri,
            |docfile| {
                docfile
                    .get_html_range(range)
                    .map_err(Into::into)
                    .map(|s| (docfile.get_css(), s))
            },
            |o| todo!(),
        )
    }

    fn get_html_fragment_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentUri,
        range: ftml_ontology::narrative::DocumentRange,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<A> {
        self.with_doc_async::<A, _, _, _, _, _>(
            uri,
            move |docfile| {
                A::block_on(move || {
                    docfile
                        .get_html_range(range)
                        .map_err(Into::into)
                        .map(|s| (docfile.get_css(), s))
                })
            },
            |o| std::future::ready(todo!()),
        )
    }

    fn get_reference<T: bincode::Decode<()>>(&self, rf: &DocDataRef<T>) -> Result<T, BackendError>
    where
        Self: Sized,
    {
        let DocDataRef {
            start,
            end,
            in_doc: uri,
            ..
        } = rf;
        self.with_doc(
            uri,
            |docfile| docfile.get_data(*start, *end).map_err(Into::into),
            |o| todo!(),
        )
    }

    fn get_module(&self, uri: &ModuleUri) -> Result<ModuleLike, BackendError> {
        if uri.is_top() {
            #[cfg(feature = "cached")]
            {
                self.modules
                    .get_sync(uri.clone(), |uri| {
                        self.load_module(uri.archive_uri(), uri.path(), uri.name())
                    })
                    .map(ModuleLike::Module)
            }
            #[cfg(not(feature = "cached"))]
            {
                self.load_module(uri.archive_uri(), uri.path(), uri.name())
                    .map(ModuleLike::Module)
            }
        } else {
            // SAFETY: !uri.is_top()
            let SymbolUri { name, module } =
                unsafe { uri.clone().into_symbol().unwrap_unchecked() };
            let mcl = module.clone();
            let m = {
                #[cfg(feature = "cached")]
                {
                    self.modules.get_sync(module, |uri| {
                        self.load_module(uri.archive_uri(), uri.path(), uri.name())
                    })?
                }
                #[cfg(not(feature = "cached"))]
                {
                    self.load_module(module.archive_uri(), module.path(), module.name())?
                }
            };

            m.as_module_like(&name)
                .ok_or_else(|| BackendError::NotFound(SymbolUri { name, module: mcl }.into()))
        }
    }

    fn get_module_async<A: AsyncEngine>(
        &self,
        uri: &ModuleUri,
    ) -> impl Future<Output = Result<ModuleLike, BackendError>> + Send + use<A>
    where
        Self: Sized,
    {
        if uri.is_top() {
            #[cfg(feature = "cached")]
            {
                if let Some(m) = self.modules.has(uri) {
                    return either::Left(either::Left(m.map_ok(ModuleLike::Module)));
                }
                let lm = self.load_module_async::<A>(uri.archive_uri(), uri.path(), uri.name());
                either::Left(either::Right(
                    self.modules
                        .get(uri.clone(), |_| lm)
                        .map_ok(ModuleLike::Module),
                ))
            }
            #[cfg(not(feature = "cached"))]
            {
                either::Left(
                    self.load_module_async::<A>(uri.archive_uri(), uri.path(), uri.name())
                        .map_ok(ModuleLike::Module),
                )
            }
        } else {
            // SAFETY: !uri.is_top()
            let SymbolUri { name, module } =
                unsafe { uri.clone().into_symbol().unwrap_unchecked() };
            let m = {
                #[cfg(feature = "cached")]
                {
                    if let Some(m) = self.modules.has(&module) {
                        either::Left(m)
                    } else {
                        either::Right(self.load_module_async::<A>(
                            module.archive_uri(),
                            module.path(),
                            module.name(),
                        ))
                    }
                }
                #[cfg(not(feature = "cached"))]
                {
                    self.load_module_async::<A>(module.archive_uri(), module.path(), module.name())
                }
            };
            either::Right(m.and_then(move |m| {
                std::future::ready(
                    m.as_module_like(&name)
                        .ok_or_else(|| BackendError::NotFound(SymbolUri { name, module }.into())),
                )
            }))
        }
    }

    #[cfg(feature = "rdf")]
    fn get_notations<E: AsyncEngine>(
        &self,
        uri: &SymbolUri,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)>
    where
        Self: Sized,
    {
        use ftml_uris::FtmlUri;
        self.query_notations::<E, ftml_ontology::narrative::elements::notations::NotationReference>(
            uri.to_iri(),
            self,
            |n| n.notation,
        )
    }

    #[cfg(feature = "rdf")]
    fn get_var_notations<E: AsyncEngine>(
        &self,
        uri: &DocumentElementUri,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)>
    where
        Self: Sized,
    {
        use ftml_uris::FtmlUri;
        self.query_notations::<E,ftml_ontology::narrative::elements::notations::VariableNotationReference>(
            uri.to_iri(),
            self,
            |n| n.notation,
        )
    }
}

impl ArchiveManager {
    fn with_doc<R>(
        &self,
        uri: &DocumentUri,
        then: impl FnOnce(&DocumentFile) -> Result<R, BackendError>,
        other: impl FnOnce(&dyn ExternalArchive) -> Result<R, BackendError>,
    ) -> Result<R, BackendError> {
        #[cfg(feature = "cached")]
        {
            if let Some(v) = self.documents.has_sync(uri) {
                let docfile = v?;
                return then(&docfile);
            }
        }
        let file_or_other = self.with_archive(uri.archive_id(), |a| {
            let Some(a) = a else {
                return Err(BackendError::ArchiveNotFound(uri.archive_uri().clone()));
            };
            match a {
                Archive::Local(a) => Ok(either::Left(a.document_file(
                    uri.path(),
                    None,
                    &uri.name,
                    uri.language(),
                ))),
                Archive::Ext(_, ext) => other(&**ext).map(either::Right),
            }
        })?;
        match file_or_other {
            either::Left(file) => {
                let docfile = {
                    #[cfg(feature = "cached")]
                    {
                        self.documents.get_sync(uri.clone(), |_| {
                            DocumentFile::from_file(file)
                                .map(triomphe::Arc::new)
                                .map_err(Into::into)
                        })?
                    }
                    #[cfg(not(feature = "cached"))]
                    {
                        DocumentFile::from_file(file).map(triomphe::Arc::new)?
                    }
                };
                then(&docfile)
            }
            either::Right(r) => Ok(r),
        }
    }

    fn with_doc_async<
        A: AsyncEngine,
        R: Send,
        T: Future<Output = Result<R, BackendError>> + Send,
        O: Future<Output = Result<R, BackendError>> + Send,
        Then: FnOnce(triomphe::Arc<DocumentFile>) -> T + Send,
        Other: FnOnce(&dyn ExternalArchive) -> O,
    >(
        &self,
        uri: &DocumentUri,
        then: Then,
        other: Other,
    ) -> impl Future<Output = Result<R, BackendError>> + Send + use<A, R, T, O, Then, Other> {
        #[cfg(feature = "cached")]
        {
            if let Some(v) = self.documents.has(uri) {
                return either::Right(either::Left(async move {
                    match v.await {
                        Ok(f) => then(f).await,
                        Err(e) => Err(e),
                    }
                }));
            }
        }
        // TODO: a.document_file blocks; avoid!
        let file_or_other = match self.with_archive(uri.archive_id(), |a| {
            let Some(a) = a else {
                return Err(BackendError::ArchiveNotFound(uri.archive_uri().clone()));
            };
            match a {
                Archive::Local(a) => Ok(either::Left(a.document_file(
                    uri.path(),
                    None,
                    &uri.name,
                    uri.language(),
                ))),
                Archive::Ext(_, ext) => Ok(either::Right(other(&**ext))),
            }
        }) {
            Ok(v) => v,
            Err(e) => return either::Left(std::future::ready(Err(e))),
        };
        #[cfg(feature = "cached")]
        {
            match file_or_other {
                either::Left(file) => {
                    let docfile = self.documents.get(uri.clone(), |_| {
                        A::block_on(move || {
                            DocumentFile::from_file(file)
                                .map(triomphe::Arc::new)
                                .map_err(Into::into)
                        })
                    });
                    either::Right(either::Right(either::Left(async move {
                        let docfile = docfile.await?;
                        then(docfile).await
                    })))
                }
                either::Right(r) => either::Right(either::Right(either::Right(r))),
            }
        }
        #[cfg(not(feature = "cached"))]
        {
            match file_or_other {
                either::Left(file) => {
                    let docfile =
                        A::block_on(move || DocumentFile::from_file(file).map(triomphe::Arc::new));
                    either::Right(either::Left(async move {
                        let docfile = docfile.await?;
                        then(docfile).await
                    }))
                }
                either::Right(r) => either::Right(either::Right(r)),
            }
        }
    }

    #[cfg(feature = "rdf")]
    pub(crate) fn query_notations<E: AsyncEngine, T: IsDocumentElement + 'static>(
        &self,
        iri: ulo::rdf_types::NamedNode,
        backend: &impl LocalBackend,
        get_not: fn(&SharedDocumentElement<T>) -> DataRef<Notation>,
        //get_ref: impl Fn(&DocDataRef<Notation>) -> Result<Notation, BackendError>,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)> {
        let iricl = iri.clone();
        let q = crate::sparql!(SELECT DISTINCT ?n WHERE {
            { ?n ulo:notation_for iricl. } UNION
            {
                iri ulo:generated_by ?o .
                ?n ulo:notation_for ?o.
            }
        });
        self.triple_store()
            .query::<E>(q)
            .expect("Notations query should be valid")
            .into_uris::<DocumentElementUri>()
            .filter_map(move |uri| {
                //tracing::warn!("Found {uri}");
                let notation = backend.get_typed_document_element::<T>(&uri).ok()?;
                //tracing::warn!("Found {notation:?}");
                backend
                    .get_reference(&get_not(&notation).with_doc(uri.document.clone()))
                    //self.get_reference(&get_not(&notation).with_doc(uri.document.clone()))
                    .map_err(|e| tracing::error!("Error getting notation {uri}: {e}"))
                    .ok()
                    .map(|n| (uri, n))
            })
    }

    /*
    #[cfg(feature = "rdf")]
    fn do_notations<E: AsyncEngine>(
        &self,
        iri: ulo::rdf_types::NamedNode,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)> {
        let q = crate::sparql!(SELECT DISTINCT ?n WHERE { ?n ulo:notation_for iri. });
        self.triple_store()
            .query::<E>(q)
            .expect("Notations query should be valid")
            .into_uris::<DocumentElementUri>()
            .filter_map(|uri| {
                use ftml_ontology::narrative::elements::notations::NotationReference;
                //tracing::warn!("Found {uri}");
                let notation = self
                    .get_typed_document_element::<NotationReference>(&uri)
                    .ok()?;
                //tracing::warn!("Found {notation:?}");
                self.get_reference(&notation.notation.with_doc(uri.document.clone()))
                    .map_err(|e| tracing::error!("Error getting notation {uri}: {e}"))
                    .ok()
                    .map(|n| (uri, n))
            })
    }

    #[cfg(feature = "rdf")]
    fn do_var_notations<E: AsyncEngine>(
        &self,
        iri: ulo::rdf_types::NamedNode,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)> {
        let q = crate::sparql!(SELECT DISTINCT ?n WHERE { ?n ulo:notation_for iri. });
        self.triple_store()
            .query::<E>(q)
            .expect("Notations query should be valid")
            .into_uris::<DocumentElementUri>()
            .filter_map(|uri| {
                use ftml_ontology::narrative::elements::notations::VariableNotationReference;
                //tracing::warn!("Found {uri}");
                let notation = self
                    .get_typed_document_element::<VariableNotationReference>(&uri)
                    .ok()?;
                //tracing::warn!("Found {notation:?}");
                self.get_reference(&notation.notation.with_doc(uri.document.clone()))
                    .map_err(|e| tracing::error!("Error getting variable notation {uri}: {e}"))
                    .ok()
                    .map(|n| (uri, n))
            })
    }
     */
}
