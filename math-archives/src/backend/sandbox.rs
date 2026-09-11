use std::{
    hint::unreachable_unchecked,
    path::{Path, PathBuf},
};

use ftml_ontology::{
    narrative::{DocumentRange, documents::Document},
    utils::{Css, RefTree},
};
use ftml_uris::{ArchiveId, DocumentUri, UriPath, UriWithArchive};

use crate::{
    Archive, LocalArchive, MathArchive,
    backend::{GlobalBackend, LocalBackend},
    manager::{ArchiveGroup, ArchiveManager, ArchiveOrGroup, ArchiveTree},
    utils::{
        AsyncEngine,
        errors::{ArtifactSaveError, BackendError, FileError},
        path_ext::{PathExt, RelPath},
    },
};

#[derive(Debug, Clone)]
pub enum SandboxedRepository {
    Copy(ArchiveId),
    Git {
        id: ArchiveId,
        branch: Box<str>,
        commit: flams_backend_types::git::Commit,
        remote: Box<str>,
    },
}

impl SandboxedRepository {
    #[inline]
    #[must_use]
    pub const fn id(&self) -> &ArchiveId {
        match self {
            Self::Copy(id) | Self::Git { id, .. } => id,
        }
    }
}

#[derive(Debug)]
pub(super) struct SandboxedBackendI {
    path: Box<Path>,
    span: tracing::Span,
    pub(super) repos: parking_lot::RwLock<Vec<SandboxedRepository>>,
    manager: ArchiveManager,
}

#[derive(Debug, Clone)]
pub struct SandboxedBackend(pub(super) triomphe::Arc<SandboxedBackendI>);
impl Drop for SandboxedBackendI {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

impl SandboxedBackend {
    pub fn load_all(&self) {
        self.0.manager.load(&[&self.0.path]);
    }
    #[inline]
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.0.path
    }

    #[inline]
    #[must_use]
    pub fn get_repos(&self) -> Vec<SandboxedRepository> {
        self.0.repos.read().clone()
    }

    #[inline]
    pub fn with_repos<R>(&self, f: impl FnOnce(&[SandboxedRepository]) -> R) -> R {
        let inner = self.0.repos.read();
        f(inner.as_slice())
    }

    #[inline]
    #[must_use]
    pub fn path_for(&self, id: &ArchiveId) -> PathBuf {
        self.0.path.join(id.as_ref())
    }

    pub fn new(name: &str, temp_dir: &Path) -> Self {
        let p = temp_dir.join(name);
        let i = SandboxedBackendI {
            span: tracing::info_span!(target:"sandbox","sandbox",path=%p.display()),
            path: p.into(),
            repos: parking_lot::RwLock::new(Vec::new()),
            #[cfg(not(feature = "rocksdb"))]
            manager: ArchiveManager::default(),
            #[cfg(feature = "rocksdb")]
            manager: ArchiveManager::new(&temp_dir.join(".rdf")),
        };
        Self(triomphe::Arc::new(i))
    }

    #[inline]
    pub fn clear(&self) {
        self.0.repos.write().clear();
    }

    /// # Errors
    /// # Panics
    #[tracing::instrument(level = "info",
        parent = &self.0.span,
        target = "sandbox",
        name = "migrating",
        fields(path = %self.0.path.display()),
        skip_all
    )]
    pub fn migrate<A: AsyncEngine>(&self) -> Result<usize, FileError> {
        let mut count = 0;
        let cnt = &mut count;
        self.0.manager.reinit::<Result<(), FileError>>(
            move |sandbox| {
                GlobalBackend.reinit::<Result<(), FileError>>(
                    |_| {
                        sandbox.top.clear();
                        let Some(main) = crate::mathhub::mathhubs().first() else {
                            unreachable!()
                        };
                        for a in std::mem::take(&mut sandbox.archives) {
                            *cnt += 1;
                            let Archive::Local(a) = a else {
                                // SAFETY: sandboxes have only local archives
                                unsafe { unreachable_unchecked() }
                            };
                            let source = a.path();
                            let target = main.join(a.id().as_ref());

                            if let Some(p) = target.parent() {
                                std::fs::create_dir_all(p)
                                    .map_err(|e| FileError::Creation(p.to_path_buf(), e))?;
                            }
                            // SAFETY: we know target is in some mathhub directory,
                            // so it has a parent
                            let safe_target = unsafe { target.parent().unwrap_unchecked() }.join(
                                format!(".{}.tmp", target.file_name().expect("weird fs").display()),
                            );
                            if safe_target.exists() {
                                std::fs::remove_dir_all(&safe_target)
                                    .map_err(|e| FileError::Rename(safe_target.clone(), e))?;
                            }
                            source.rename_safe(&safe_target)?;
                            if target.exists() {
                                std::fs::remove_dir_all(&target)
                                    .map_err(|e| FileError::Rename(target.clone(), e))?;
                            }
                            std::fs::rename(&safe_target, target)
                                .map_err(|e| FileError::Rename(safe_target.clone(), e))?;
                        }
                        Ok(())
                    },
                    crate::mathhub::mathhubs(),
                )
            },
            &[&*self.0.path],
        )?;

        #[cfg(feature = "rdf")]
        A::background(|| {
            GlobalBackend
                .triple_store()
                .load_archives(&GlobalBackend.all_archives());
        });
        Ok(count)
    }

    /// # Panics
    #[tracing::instrument(level = "info",
        parent = &self.0.span,
        target = "sandbox",
        name = "adding",
        fields(repository = ?sb),
        skip_all
    )]
    pub fn add(&self, sb: SandboxedRepository, then: impl FnOnce()) {
        let mut repos = self.0.repos.write();
        let id = sb.id();
        if let Some(i) = repos.iter().position(|r| r.id() == id) {
            repos.remove(i);
        }
        self.require_meta_infs(
            id,
            &mut repos,
            |_, _| {},
            |_, _, _| {
                tracing::error!("A group with id {id} already exists!");
            },
            || {},
        );
        let id = sb.id().clone();
        repos.push(sb);
        drop(repos);
        then();
        let manifest = LocalArchive::manifest_of(&self.0.path.join(id.as_ref()))
            .expect("archive does not exist");
        self.0.manager.load_one(&manifest, RelPath::from_id(&id));
    }

    fn require_meta_infs(
        &self,
        id: &ArchiveId,
        repos: &mut Vec<SandboxedRepository>,
        then: impl FnOnce(&LocalArchive, &mut Vec<SandboxedRepository>),
        group: impl FnOnce(&ArchiveGroup, &ArchiveTree, &mut Vec<SandboxedRepository>),
        else_: impl FnOnce(),
    ) {
        if repos.iter().any(|r| r.id() == id) {
            return;
        }
        GlobalBackend.with_tree(move |t| {
            let mut steps = id.steps();
            let Some(mut current) = steps.next() else {
                tracing::error!("empty archive ID");
                return;
            };
            let mut ls = &t.top;
            loop {
                let Some(a) = ls.iter().find(|a| a.id().last() == current) else {
                    else_();
                    return;
                };
                match a {
                    ArchiveOrGroup::Archive(_) => {
                        if steps.next().is_some() {
                            else_();
                            return;
                        }
                        let Some(Archive::Local(a)) = t.get(id) else {
                            else_();
                            return;
                        };
                        then(a, repos);
                        return;
                    }
                    ArchiveOrGroup::Group(g) => {
                        let Some(next) = steps.next() else {
                            group(g, t, repos);
                            return;
                        };
                        if let Some(ArchiveOrGroup::Archive(a)) =
                            g.children.iter().find(|a| a.id().is_meta())
                            && !repos.iter().any(|r| r.id() == a)
                        {
                            let Some(Archive::Local(a)) = t.get(a) else {
                                else_();
                                return;
                            };
                            repos.push(SandboxedRepository::Copy(a.id().clone()));
                            if self.copy_archive(a).is_ok()
                                && let Some(manifest) =
                                    LocalArchive::manifest_of(&self.0.path.join(a.id().as_ref()))
                            {
                                self.0.manager.load_one(&manifest, RelPath::from_id(a.id()));
                            }
                        }
                        current = next;
                        ls = &g.children;
                    }
                }
            }
        });
    }

    /// # Panics
    #[tracing::instrument(level = "info",
        parent = &self.0.span,
        target = "sandbox",
        name = "require",
        skip(self)
    )]
    pub fn require(&self, id: &ArchiveId, load: bool) {
        // TODO this can be massively optimized
        let mut repos = self.0.repos.write();
        self.require_meta_infs(
            id,
            &mut repos,
            |a, repos| {
                if !repos.iter().any(|r| r.id() == id) {
                    repos.push(SandboxedRepository::Copy(id.clone()));
                    if let Err(e) = self.copy_archive(a) {
                        tracing::error!("Error copying {id}: {e}");
                    }
                }
            },
            |g, t, repos| {
                for a in g.dfs() {
                    if let ArchiveOrGroup::Archive(id) = a
                        && let Some(Archive::Local(a)) = t.get(id)
                        && !repos.iter().any(|r| r.id() == id)
                    {
                        repos.push(SandboxedRepository::Copy(id.clone()));
                        if let Err(e) = self.copy_archive(a) {
                            tracing::error!("Error copying {id}: {e}");
                        }
                        if let Some(manifest) =
                            LocalArchive::manifest_of(&self.0.path.join(id.as_ref()))
                        {
                            let _ = self.0.manager.load_one(&manifest, RelPath::from_id(id));
                        }
                    }
                }
            },
            || tracing::error!("could not find archive {id}"),
        );
        drop(repos);

        if load {
            let Some(manifest) = LocalArchive::manifest_of(&self.0.path.join(id.as_ref())) else {
                tracing::error!("Error loading manifest of archive {id}");
                panic!("archive does not exist")
            };
            let _ = self.0.manager.load_one(&manifest, RelPath::from_id(id));
        }
    }

    //#[deprecated(note = "needs refactoring: should register with manager, but can't")]
    pub fn maybe_copy(&self, archive: &LocalArchive) {
        if !self.0.repos.read().iter().any(|a| a.id() == archive.id()) {
            self.0
                .repos
                .write()
                .push(SandboxedRepository::Copy(archive.id().clone()));
            let _ = self.copy_archive(archive);
        }
    }

    fn copy_archive(&self, a: &LocalArchive) -> Result<(), FileError> {
        let path = a.path();
        let target = self.0.path.join(a.id().as_ref());
        if target.exists() {
            return Err(FileError::AlreadyExists);
        }
        tracing::info!("copying archive {} to {}", a.id(), target.display());
        path.copy_dir_all(&target)
    }
}

impl LocalBackend for SandboxedBackend {
    type ArchiveIter<'a> =
        std::iter::Chain<std::slice::Iter<'a, Archive>, std::slice::Iter<'a, Archive>>;

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
                        self.0.manager.documents.remove(&d.uri);
                    }
                    for m in &r.modules {
                        self.0.manager.modules.remove(&m.uri);
                    }
                } else if let Some(r) = r.as_any().downcast_ref::<crate::artifacts::ContentResult>()
                {
                    self.0.manager.documents.remove(&r.document.uri);
                    for m in &r.modules {
                        self.0.manager.modules.remove(&m.uri);
                    }
                }
            }
        }
        self.0
            .manager
            .with_buildable_archive(in_doc.archive_id(), |a| {
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
                    self.0.manager.triple_store(),
                    #[cfg(feature = "rdf")]
                    false,
                )
            })
    }

    fn get_document(&self, uri: &DocumentUri) -> Result<Document, BackendError> {
        self.0
            .manager
            .get_document(uri)
            .or_else(|_| GlobalBackend.get_document(uri))
    }

    #[allow(clippy::future_not_send)]
    fn get_document_async<A: AsyncEngine>(
        &self,
        uri: &DocumentUri,
    ) -> impl Future<Output = Result<Document, BackendError>> + Send + use<A>
    where
        Self: Sized,
    {
        let mgr = self.0.manager.get_document_async::<A>(uri);
        let uri = uri.clone();
        async move {
            if let Ok(d) = mgr.await {
                return Ok(d);
            }
            GlobalBackend.get_document_async::<A>(&uri).await
        }
    }

    fn get_module(
        &self,
        uri: &ftml_uris::ModuleUri,
    ) -> Result<ftml_ontology::domain::modules::ModuleLike, crate::utils::errors::BackendError>
    {
        self.0
            .manager
            .get_module(uri)
            .or_else(|_| GlobalBackend.get_module(uri))
    }

    fn get_module_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::ModuleUri,
    ) -> impl Future<Output = Result<ftml_ontology::domain::modules::ModuleLike, BackendError>>
    + Send
    + use<A>
    where
        Self: Sized,
    {
        let uri = uri.clone();
        let mgr = self.0.manager.get_module_async::<A>(&uri);
        async move {
            if let Ok(d) = mgr.await {
                return Ok(d);
            }
            GlobalBackend.get_module_async::<A>(&uri).await
        }
    }

    fn with_archive_or_group<R>(
        &self,
        id: &ArchiveId,
        f: impl FnOnce(Option<&ArchiveOrGroup>) -> R,
    ) -> R
    where
        Self: Sized,
    {
        match self.0.manager.with_archive_or_group(id, |a| {
            if a.is_some() {
                either::Left(f(a))
            } else {
                either::Right(f)
            }
        }) {
            either::Left(v) => v,
            either::Right(f) => GlobalBackend.with_archive_or_group(id, f),
        }
    }

    fn with_archive<R>(&self, id: &ArchiveId, f: impl FnOnce(Option<&Archive>) -> R) -> R
    where
        Self: Sized,
    {
        match self.0.manager.with_archive(id, |a| {
            if a.is_some() {
                either::Left(f(a))
            } else {
                either::Right(f)
            }
        }) {
            either::Left(v) => v,
            either::Right(f) => GlobalBackend.with_archive(id, f),
        }
    }

    fn with_archives<R>(&self, f: impl FnOnce(Self::ArchiveIter<'_>) -> R) -> R
    where
        Self: Sized,
    {
        self.0
            .manager
            .with_archives(|t1| GlobalBackend.with_archives(|t2| f(t1.iter().chain(t2.iter()))))
    }

    fn get_html_full(&self, uri: &DocumentUri) -> Result<Box<str>, BackendError> {
        self.0
            .manager
            .get_html_full(uri)
            .or_else(|_| GlobalBackend.get_html_full(uri))
    }

    fn get_html_body(&self, uri: &DocumentUri) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.0
            .manager
            .get_html_body(uri)
            .or_else(|_| GlobalBackend.get_html_body(uri))
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
        let mgr = self.0.manager.get_html_body_async::<A>(uri);
        let uri = uri.clone();
        async move {
            if let Ok(d) = mgr.await {
                return Ok(d);
            }
            GlobalBackend.get_html_body_async::<A>(&uri).await
        }
    }

    fn get_html_body_inner(
        &self,
        uri: &DocumentUri,
    ) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.0
            .manager
            .get_html_body_inner(uri)
            .or_else(|_| GlobalBackend.get_html_body_inner(uri))
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
        let mgr = self.0.manager.get_html_body_inner_async::<A>(uri);
        let uri = uri.clone();
        async move {
            if let Ok(d) = mgr.await {
                return Ok(d);
            }
            GlobalBackend.get_html_body_inner_async::<A>(&uri).await
        }
    }

    fn get_html_fragment(
        &self,
        uri: &DocumentUri,
        range: DocumentRange,
    ) -> Result<(Box<[Css]>, Box<str>), BackendError> {
        self.0
            .manager
            .get_html_fragment(uri, range)
            .or_else(|_| GlobalBackend.get_html_fragment(uri, range))
    }

    fn get_html_fragment_async<A: AsyncEngine>(
        &self,
        uri: &ftml_uris::DocumentUri,
        range: ftml_ontology::narrative::DocumentRange,
    ) -> impl Future<Output = Result<(Box<[ftml_ontology::utils::Css]>, Box<str>), BackendError>>
    + Send
    + use<A> {
        let mgr = self.0.manager.get_html_fragment_async::<A>(uri, range);
        let uri = uri.clone();
        async move {
            if let Ok(d) = mgr.await {
                return Ok(d);
            }
            GlobalBackend
                .get_html_fragment_async::<A>(&uri, range)
                .await
        }
    }

    fn get_reference<T: bincode::Decode<()>>(
        &self,
        rf: &ftml_ontology::narrative::DocDataRef<T>,
    ) -> Result<T, BackendError>
    where
        Self: Sized,
    {
        self.0
            .manager
            .get_reference(rf)
            .or_else(|_| GlobalBackend.get_reference(rf))
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
        GlobalBackend.get_notations::<E>(uri)
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
        GlobalBackend.get_var_notations::<E>(uri)
    }
}
