mod global;
mod sandbox;
mod temp;

use crate::{
    Archive, BackendError, BuildableArchive, LocalArchive, MathArchive,
    artifacts::{Artifact, FileOrString},
    formats::{BuildTarget, BuildTargetId},
    manager::ArchiveOrGroup,
    utils::{
        AsyncEngine,
        errors::ArtifactSaveError,
        path_ext::{PathExt, RelPath},
    },
};
use ftml_ontology::{
    domain::{SharedDeclaration, declarations::IsDeclaration, modules::ModuleLike},
    narrative::{
        DocDataRef, DocumentRange, SharedDocumentElement,
        documents::Document,
        elements::{DocumentElement, IsDocumentElement, Notation},
    },
    utils::Css,
};
use ftml_uris::{
    ArchiveId, DocumentElementUri, DocumentUri, IsDomainUri, IsNarrativeUri, ModuleUri, NamedUri,
    SymbolUri, UriPath,
};
pub use global::*;
pub use sandbox::*;
use std::path::Path;
pub use temp::*;

pub trait LocalBackend: Send + Sync {
    type ArchiveIter<'a>: IntoIterator<Item = &'a Archive>
    where
        Self: Sized;

    /// # Errors
    fn get_document(&self, uri: &DocumentUri) -> Result<Document, BackendError>;

    /// # Errors
    fn get_document_async<A: AsyncEngine>(
        &self,
        uri: &DocumentUri,
    ) -> impl Future<Output = Result<Document, BackendError>> + Send + use<Self, A>
    where
        Self: Sized;

    /// # Errors
    fn get_module(&self, uri: &ModuleUri) -> Result<ModuleLike, BackendError>;

    /// # Errors
    fn get_module_async<A: AsyncEngine>(
        &self,
        uri: &ModuleUri,
    ) -> impl Future<Output = Result<ModuleLike, BackendError>> + Send + use<Self, A>
    where
        Self: Sized;

    fn with_archive_or_group<R>(
        &self,
        id: &ArchiveId,
        f: impl FnOnce(Option<&ArchiveOrGroup>) -> R,
    ) -> R
    where
        Self: Sized;
    fn with_archives<R>(&self, f: impl FnOnce(Self::ArchiveIter<'_>) -> R) -> R
    where
        Self: Sized;
    fn with_archive<R>(&self, id: &ArchiveId, f: impl FnOnce(Option<&Archive>) -> R) -> R
    where
        Self: Sized;
    /// # Errors
    fn get_html_body(&self, d: &DocumentUri) -> Result<(Box<[Css]>, Box<str>), BackendError>;

    /// # Errors
    fn get_html_body_async<A: AsyncEngine>(
        &self,
        d: &ftml_uris::DocumentUri,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<Self, A>
    where
        Self: Sized;

    /// # Errors
    fn get_html_body_inner(&self, d: &DocumentUri) -> Result<(Box<[Css]>, Box<str>), BackendError>;

    /// # Errors
    fn get_html_body_inner_async<A: AsyncEngine>(
        &self,
        d: &ftml_uris::DocumentUri,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<Self, A>
    where
        Self: Sized;

    /// # Errors
    fn get_html_full(&self, d: &DocumentUri) -> Result<Box<str>, BackendError>;

    /// # Errors
    fn get_html_fragment(
        &self,
        d: &DocumentUri,
        range: DocumentRange,
    ) -> Result<(Box<[Css]>, Box<str>), BackendError>;

    /// # Errors
    fn get_html_fragment_async<A: AsyncEngine>(
        &self,
        d: &ftml_uris::DocumentUri,
        range: ftml_ontology::narrative::DocumentRange,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<Self, A>
    where
        Self: Sized;

    /// # Errors
    fn get_reference<T: bincode::Decode<()>>(&self, rf: &DocDataRef<T>) -> Result<T, BackendError>
    where
        Self: Sized;

    /// # Errors
    fn get_declaration<T: IsDeclaration>(
        &self,
        uri: &SymbolUri,
    ) -> Result<SharedDeclaration<T>, BackendError>
    where
        Self: Sized,
    {
        if uri.module.name().is_simple() {
            let m = self.get_module(uri.module_uri())?;
            return m
                .get_as(uri.name())
                .ok_or_else(|| BackendError::NotFound(uri.clone().into()));
        }
        let uri = uri.clone().simple_module();
        let m = self.get_module(uri.module_uri())?;
        m.get_as(uri.name())
            .ok_or(BackendError::NotFound(uri.into()))
    }

    /// # Errors
    fn save(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        log: FileOrString,
        from: BuildTargetId,
        result: Option<Box<dyn Artifact>>,
    ) -> std::result::Result<(), ArtifactSaveError>;

    /// # Errors
    fn get_document_element(
        &self,
        uri: &DocumentElementUri,
    ) -> Result<SharedDocumentElement<DocumentElement>, BackendError>
    where
        Self: Sized,
    {
        let d = self.get_document(uri.document_uri())?;
        d.get(uri.name())
            .ok_or_else(|| BackendError::NotFound(uri.clone().into()))
    }

    /// # Errors
    async fn get_document_element_async<A: AsyncEngine>(
        &self,
        uri: &DocumentElementUri,
    ) -> Result<SharedDocumentElement<DocumentElement>, BackendError>
    where
        Self: Sized,
    {
        let d = self.get_document_async::<A>(uri.document_uri()).await?;
        d.get(uri.name())
            .ok_or_else(|| BackendError::NotFound(uri.clone().into()))
    }

    /// # Errors
    fn get_typed_document_element<T: IsDocumentElement>(
        &self,
        uri: &DocumentElementUri,
    ) -> Result<SharedDocumentElement<T>, BackendError>
    where
        Self: Sized,
    {
        let d = self.get_document(uri.document_uri())?;
        d.get_as(uri.name())
            .ok_or_else(|| BackendError::NotFound(uri.clone().into()))
    }

    /// # Errors
    async fn get_typed_document_element_async<A: AsyncEngine, T: IsDocumentElement>(
        &self,
        uri: &DocumentElementUri,
    ) -> Result<SharedDocumentElement<T>, BackendError>
    where
        Self: Sized,
    {
        let d = self.get_document_async::<A>(uri.document_uri()).await?;
        d.get_as(uri.name())
            .ok_or_else(|| BackendError::NotFound(uri.clone().into()))
    }

    fn uri_of(&self, p: &Path) -> Option<DocumentUri>
    where
        Self: Sized,
    {
        self.archive_of_source(p, |a, rel_path| {
            let str = rel_path.as_os_str().to_str()?;
            DocumentUri::from_archive_relpath(a.uri().clone(), str).ok()
        })
        .flatten()
    }

    fn archive_of_source<R>(
        &self,
        p: &Path,
        mut f: impl FnMut(&LocalArchive, RelPath) -> R,
    ) -> Option<R>
    where
        Self: Sized,
    {
        self.with_archives(|archives| {
            for a in archives {
                let Archive::Local(a) = a else { continue };
                if p.relative_to(&a.path()).is_some() {
                    let rel_path = p.relative_to(&a.source_dir())?;
                    return Some(f(a, rel_path));
                }
            }
            None
        })
    }

    fn archive_of<R>(&self, p: &Path, mut f: impl FnMut(&LocalArchive, RelPath) -> R) -> Option<R>
    where
        Self: Sized,
    {
        self.with_archives(|archives| {
            for a in archives {
                let Archive::Local(a) = a else { continue };
                if let Some(rp) = p.relative_to(&a.path()) {
                    return Some(f(a, rp));
                }
            }
            None
        })
    }

    fn with_local_archive<R>(&self, id: &ArchiveId, f: impl FnOnce(Option<&LocalArchive>) -> R) -> R
    where
        Self: Sized,
    {
        self.with_archive(id, |a| {
            f(a.and_then(|a| match a {
                Archive::Local(a) => Some(&**a),
                Archive::Ext(..) => None,
            }))
        })
    }

    fn with_buildable_archive<R>(
        &self,
        id: &ArchiveId,
        f: impl FnOnce(Option<&dyn BuildableArchive>) -> R,
    ) -> R
    where
        Self: Sized,
    {
        self.with_archive(id, |a| {
            f(a.and_then(|a| match a {
                Archive::Local(a) => Some(&**a as _),
                Archive::Ext(_, e) => e.buildable(),
            }))
        })
    }

    #[cfg(feature = "rdf")]
    fn get_notations<E: AsyncEngine>(
        &self,
        uri: &SymbolUri,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)>
    where
        Self: Sized;

    #[cfg(feature = "rdf")]
    fn get_var_notations<E: AsyncEngine>(
        &self,
        uri: &DocumentElementUri,
    ) -> impl Iterator<Item = (DocumentElementUri, Notation)>
    where
        Self: Sized;

    #[cfg(feature = "img")]
    fn export_html(&self, uri: &DocumentUri, to: &Path) -> Result<(), String>
    where
        Self: Sized,
    {
        crate::images::html_export(uri, to, self)
    }
}

#[derive(Clone, Debug)]
pub enum AnyBackend {
    Global,
    Temp(TemporaryBackend),
    Sandbox(SandboxedBackend),
}

impl AnyBackend {
    #[must_use]
    /// # Panics
    pub fn mathhub(&self) -> &Path {
        match self {
            Self::Global | Self::Temp(_) => crate::mathhub::mathhubs()
                .iter()
                .next()
                .expect("No mathhubs found"),
            Self::Sandbox(sb) => sb.root(),
        }
    }

    pub fn mathhubs(&self) -> impl Iterator<Item = &Path> {
        match self {
            Self::Global | Self::Temp(_) => {
                either::Left(crate::mathhub::mathhubs().iter().copied())
            }
            Self::Sandbox(sb) => either::Right(
                std::iter::once(sb.root()).chain(crate::mathhub::mathhubs().iter().copied()),
            ),
        }
    }

    #[cfg(feature = "rdf")]
    pub fn add_triples(&self, doc: &DocumentUri, triples: Vec<ulo::rdf_types::Triple>) {
        use ftml_uris::FtmlUri;
        if matches!(*self, Self::Global) {
            GlobalBackend
                .get()
                .triple_store()
                .add_graph(&doc.to_iri(), triples.into_iter());
        }
    }
}

impl LocalBackend for AnyBackend {
    type ArchiveIter<'a> = either::Either<
        std::slice::Iter<'a, Archive>,
        <SandboxedBackend as LocalBackend>::ArchiveIter<'a>,
    >;

    fn save(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        log: FileOrString,
        from: BuildTargetId,
        result: Option<Box<dyn Artifact>>,
    ) -> std::result::Result<(), ArtifactSaveError> {
        if let Some(rp) = rel_path
            && let Some(res) = result.as_ref()
        {
            save_top(self, in_doc, rp, &**res);
        }
        match self {
            Self::Global => GlobalBackend.save(in_doc, rel_path, log, from, result),
            Self::Temp(b) => b.save(in_doc, rel_path, log, from, result),
            Self::Sandbox(b) => b.save(in_doc, rel_path, log, from, result),
        }
    }

    fn with_archives<R>(&self, f: impl FnOnce(Self::ArchiveIter<'_>) -> R) -> R
    where
        Self: Sized,
    {
        match self {
            Self::Global => GlobalBackend.with_archives(|i| f(either::Either::Left(i.iter()))),
            Self::Temp(b) => b.with_archives(f),
            Self::Sandbox(b) => b.with_archives(|i| f(either::Either::Right(i))),
        }
    }

    fn get_reference<T: bincode::Decode<()>>(
        &self,
        rf: &ftml_ontology::narrative::DocDataRef<T>,
    ) -> Result<T, BackendError>
    where
        Self: Sized,
    {
        match self {
            Self::Global => GlobalBackend.get_reference(rf),
            Self::Temp(b) => b.get_reference(rf),
            Self::Sandbox(b) => b.get_reference(rf),
        }
    }

    fn get_document(&self, uri: &ftml_uris::DocumentUri) -> Result<Document, BackendError> {
        match self {
            Self::Global => GlobalBackend.get_document(uri),
            Self::Temp(b) => b.get_document(uri),
            Self::Sandbox(b) => b.get_document(uri),
        }
    }

    #[allow(clippy::future_not_send)]
    fn get_document_async<A: AsyncEngine>(
        &self,
        uri: &DocumentUri,
    ) -> impl Future<Output = Result<Document, BackendError>> + Send + use<A>
    where
        Self: Sized,
    {
        match self {
            Self::Global => either::Left(GlobalBackend.get_document_async::<A>(uri)),
            Self::Temp(b) => either::Right(either::Left(b.get_document_async::<A>(uri))),
            Self::Sandbox(b) => either::Right(either::Right(b.get_document_async::<A>(uri))),
        }
    }

    fn get_html_body(
        &self,
        d: &ftml_uris::DocumentUri,
    ) -> Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError> {
        match self {
            Self::Global => GlobalBackend.get_html_body(d),
            Self::Temp(b) => b.get_html_body(d),
            Self::Sandbox(b) => b.get_html_body(d),
        }
    }

    fn get_html_body_async<A: AsyncEngine>(
        &self,
        d: &ftml_uris::DocumentUri,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<A>
    where
        Self: Sized,
    {
        match self {
            Self::Global => either::Left(GlobalBackend.get_html_body_async::<A>(d)),
            Self::Temp(b) => either::Right(either::Left(b.get_html_body_async::<A>(d))),
            Self::Sandbox(b) => either::Right(either::Right(b.get_html_body_async::<A>(d))),
        }
    }

    fn get_html_body_inner(
        &self,
        d: &ftml_uris::DocumentUri,
    ) -> Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError> {
        match self {
            Self::Global => GlobalBackend.get_html_body_inner(d),
            Self::Temp(b) => b.get_html_body_inner(d),
            Self::Sandbox(b) => b.get_html_body_inner(d),
        }
    }

    fn get_html_body_inner_async<A: AsyncEngine>(
        &self,
        d: &ftml_uris::DocumentUri,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + use<A>
    + Send
    where
        Self: Sized,
    {
        match self {
            Self::Global => either::Left(GlobalBackend.get_html_body_inner_async::<A>(d)),
            Self::Temp(b) => either::Right(either::Left(b.get_html_body_inner_async::<A>(d))),
            Self::Sandbox(b) => either::Right(either::Right(b.get_html_body_inner_async::<A>(d))),
        }
    }

    fn get_html_full(&self, d: &ftml_uris::DocumentUri) -> Result<Box<str>, BackendError> {
        match self {
            Self::Global => GlobalBackend.get_html_full(d),
            Self::Temp(b) => b.get_html_full(d),
            Self::Sandbox(b) => b.get_html_full(d),
        }
    }

    fn get_html_fragment(
        &self,
        d: &ftml_uris::DocumentUri,
        range: ftml_ontology::narrative::DocumentRange,
    ) -> Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError> {
        match self {
            Self::Global => GlobalBackend.get_html_fragment(d, range),
            Self::Temp(b) => b.get_html_fragment(d, range),
            Self::Sandbox(b) => b.get_html_fragment(d, range),
        }
    }

    fn get_html_fragment_async<A: AsyncEngine>(
        &self,
        d: &ftml_uris::DocumentUri,
        range: ftml_ontology::narrative::DocumentRange,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<A> {
        match self {
            Self::Global => either::Left(GlobalBackend.get_html_fragment_async::<A>(d, range)),
            Self::Temp(b) => either::Right(either::Left(b.get_html_fragment_async::<A>(d, range))),
            Self::Sandbox(b) => {
                either::Right(either::Right(b.get_html_fragment_async::<A>(d, range)))
            }
        }
    }

    fn get_module(&self, uri: &ftml_uris::ModuleUri) -> Result<ModuleLike, BackendError> {
        match self {
            Self::Global => GlobalBackend.get_module(uri),
            Self::Temp(b) => b.get_module(uri),
            Self::Sandbox(b) => b.get_module(uri),
        }
    }

    fn get_module_async<A: AsyncEngine>(
        &self,
        uri: &ModuleUri,
    ) -> impl Future<Output = Result<ModuleLike, BackendError>> + Send + use<A>
    where
        Self: Sized,
    {
        match self {
            Self::Global => either::Left(GlobalBackend.get_module_async::<A>(uri)),
            Self::Temp(b) => either::Right(either::Left(b.get_module_async::<A>(uri))),
            Self::Sandbox(b) => either::Right(either::Right(b.get_module_async::<A>(uri))),
        }
    }

    fn with_archive_or_group<R>(
        &self,
        id: &ftml_uris::ArchiveId,
        f: impl FnOnce(Option<&ArchiveOrGroup>) -> R,
    ) -> R
    where
        Self: Sized,
    {
        match self {
            Self::Global => GlobalBackend.with_archive_or_group(id, f),
            Self::Temp(b) => b.with_archive_or_group(id, f),
            Self::Sandbox(b) => b.with_archive_or_group(id, f),
        }
    }

    fn with_archive<R>(&self, id: &ftml_uris::ArchiveId, f: impl FnOnce(Option<&Archive>) -> R) -> R
    where
        Self: Sized,
    {
        match self {
            Self::Global => GlobalBackend.with_archive(id, f),
            Self::Temp(b) => b.with_archive(id, f),
            Self::Sandbox(b) => b.with_archive(id, f),
        }
    }

    #[cfg(feature = "rdf")]
    #[inline]
    fn get_notations<E: AsyncEngine>(
        &self,
        uri: &ftml_uris::SymbolUri,
    ) -> impl Iterator<
        Item = (
            ftml_uris::DocumentElementUri,
            ftml_ontology::narrative::elements::Notation,
        ),
    >
    where
        Self: Sized,
    {
        match self {
            Self::Temp(b) => either::Left(b.get_notations::<E>(uri)),
            _ => either::Right(GlobalBackend.get_notations::<E>(uri)),
        }
        //GlobalBackend.get_notations::<E>(uri)
    }

    #[cfg(feature = "rdf")]
    #[inline]
    fn get_var_notations<E: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentElementUri,
    ) -> impl Iterator<
        Item = (
            ftml_uris::DocumentElementUri,
            ftml_ontology::narrative::elements::Notation,
        ),
    >
    where
        Self: Sized,
    {
        match self {
            Self::Temp(b) => either::Left(b.get_var_notations::<E>(uri)),
            _ => either::Right(GlobalBackend.get_var_notations::<E>(uri)),
        }
    }
}

fn save_top(slf: &AnyBackend, uri: &DocumentUri, rel_path: &UriPath, result: &dyn Artifact) {
    for e in inventory::iter::<crate::FlamsExtension>() {
        (e.on_build_result)(slf, uri, rel_path, result);
    }
}
