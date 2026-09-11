#![allow(unexpected_cfgs)]
#![cfg_attr(all(doc, CHANNEL_NIGHTLY), feature(doc_cfg))]
#![doc = include_str!("../README.md")]
/*!
 * ## Feature flags
 */
#![cfg_attr(doc,doc = document_features::document_features!())]

pub mod archive_iter;
pub mod artifacts;
pub mod backend;
pub mod document_file;
pub mod formats;
pub mod manager;
pub mod manifest;
pub mod mathhub;
pub mod source_files;
#[cfg(feature = "rdf")]
pub mod triple_store;
pub mod utils;
pub use flams_backend_types as types;
#[cfg(feature = "img")]
pub mod images;

#[cfg(feature = "rdf")]
use crate::triple_store::RDFStore;
use crate::{
    artifacts::{Artifact, ContentResult, ContentUpdate, FileOrString},
    formats::{BuildTargetId, SourceFormat, SourceFormatId},
    manifest::RepositoryData,
    source_files::{FileStates, SourceDir},
    utils::{
        AsyncEngine,
        errors::{ArtifactSaveError, BackendError, FileError},
        ignore_source::IgnoreSource,
        path_ext::PathExt,
    },
};
use flams_backend_types::{
    archive_json::{ArchiveIndex, Institution},
    archives::FileStateSummary,
};
use ftml_ontology::{domain::modules::Module, narrative::documents::Document};
use ftml_uris::{
    ArchiveId, ArchiveUri, DocumentUri, IsDomainUri, Language, ModuleUri, SimpleUriName, UriName,
    UriPath, UriWithArchive, UriWithPath,
};
use std::{
    hint::unreachable_unchecked,
    path::{Path, PathBuf},
    str::{self, FromStr},
};

pub struct FlamsExtension {
    pub name: &'static str,
    pub on_start: fn(),
    pub on_build_result: fn(&backend::AnyBackend, &DocumentUri, &UriPath, &dyn Artifact),
    pub on_reload: fn(),
}

inventory::collect!(FlamsExtension);

type Result<T> = std::result::Result<T, BackendError>;
/*
pub trait DocumentSource: std::fmt::Debug {
    fn get_document(&self) -> impl Future<Output = Result<Document>>
    where
        Self: Sized;
    fn get_css(&self) -> impl Future<Output = Result<Box<[Css]>>>
    where
        Self: Sized;
    fn get_html(&self) -> impl Future<Output = Result<Box<str>>>
    where
        Self: Sized;
    fn get_document_sync(&self) -> Result<Document>;
    fn get_css_sync(&self) -> Result<Box<[Css]>>;
    fn get_html_sync(&self) -> Result<Box<str>>;
}
 */

pub trait MathArchive {
    fn uri(&self) -> &ArchiveUri;
    fn path(&self) -> &Path;
    fn is_meta(&self) -> bool;

    #[inline]
    fn id(&self) -> &ArchiveId {
        self.uri().archive_id()
    }

    /// # Errors
    fn load_module(&self, path: Option<&UriPath>, name: &UriName) -> Result<Module>;

    /// # Errors
    fn load_module_async<A: AsyncEngine>(
        &self,
        path: Option<&UriPath>,
        name: &UriName,
    ) -> impl Future<Output = Result<Module>> + 'static + use<Self, A>
    where
        Self: Sized;

    /*
    fn load_html(&self, path: Option<&UriPath>, name: &str, language: Language) -> Option<String>;
    fn load_html_body(
        &self,
        path: Option<&UriPath>,
        name: &str,
        language: Language,
        full: bool,
    ) -> Option<(Vec<Css>, String)>;
    fn load_html_fragment(
        &self,
        path: Option<&UriPath>,
        name: &str,
        language: Language,
        range: DocumentRange,
    ) -> Option<(Vec<Css>, String)>;
    fn load_reference<T: flams_ontology::Resourcable>(
        &self,
        path: Option<&UriPath>,
        name: &str,
        language: Language,
        range: DocumentRange,
    ) -> eyre::Result<T>;
    */
}

pub trait ExternalArchive: Send + Sync + MathArchive + std::any::Any + std::fmt::Debug {
    #[inline]
    fn local_out(&self) -> Option<&dyn LocallyBuilt> {
        None
    }
    #[inline]
    fn buildable(&self) -> Option<&dyn BuildableArchive> {
        None
    }

    fn load_document(
        &self,
        path: Option<&UriPath>,
        name: &str,
        language: Language,
    ) -> Option<Document>;
}

#[derive(Debug)]
pub enum Archive {
    Local(Box<LocalArchive>),
    Ext(&'static ArchiveKind, Box<dyn ExternalArchive>),
}
impl Archive {
    fn buildable(&self) -> Option<&dyn BuildableArchive> {
        match self {
            Self::Local(l) => Some(&**l as _),
            Self::Ext(_, a) => a.buildable(),
        }
    }
}

impl std::hash::Hash for Archive {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uri().hash(state);
    }
}
impl std::borrow::Borrow<ArchiveUri> for Archive {
    #[inline]
    fn borrow(&self) -> &ArchiveUri {
        self.uri()
    }
}
impl PartialEq for Archive {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        *self.uri() == *other.uri()
    }
}
impl Eq for Archive {}

impl MathArchive for Archive {
    fn uri(&self) -> &ArchiveUri {
        match self {
            Self::Local(a) => a.uri(),
            Self::Ext(_, a) => a.uri(),
        }
    }
    fn path(&self) -> &Path {
        match self {
            Self::Local(a) => a.path(),
            Self::Ext(_, a) => a.path(),
        }
    }
    fn is_meta(&self) -> bool {
        match self {
            Self::Local(a) => a.is_meta(),
            Self::Ext(_, a) => a.is_meta(),
        }
    }
    fn load_module(&self, path: Option<&UriPath>, name: &UriName) -> Result<Module> {
        match self {
            Self::Local(a) => a.load_module(path, name),
            Self::Ext(_, a) => a.load_module(path, name),
        }
    }
    fn load_module_async<A: AsyncEngine>(
        &self,
        path: Option<&UriPath>,
        name: &UriName,
    ) -> impl Future<Output = Result<Module>> + 'static + use<A> {
        match self {
            Self::Local(a) => a.load_module_async::<A>(path, name),
            Self::Ext(_, a) => todo!(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ArchiveKind {
    pub name: &'static str,
    #[allow(clippy::type_complexity)]
    make_new: fn(RepositoryData, &Path) -> std::result::Result<Box<dyn ExternalArchive>, String>,
}

impl ArchiveKind {
    #[inline]
    pub fn all() -> impl Iterator<Item = &'static Self> {
        inventory::iter.into_iter()
    }
    #[must_use]
    pub fn get(name: &str) -> Option<&'static Self> {
        Self::all().find(|e| e.name == name)
    }
}
inventory::collect!(ArchiveKind);
#[macro_export]
macro_rules! archive_kind {
    ($i:ident { $($t:tt)* }) => {
        pub static $i : $crate::ArchiveKind = $crate::ArchiveKind { $($t)* };
        $crate::formats::__reexport::submit!{ $i }
    };
}

pub trait BuildableArchive: MathArchive {
    fn file_state(&self) -> FileStates;
    fn formats(&self) -> &[SourceFormatId];
    fn get_log(&self, relative_path: &str, target: BuildTargetId) -> PathBuf;

    #[allow(clippy::too_many_arguments)]
    /// # Errors
    fn save(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        log: FileOrString,
        from: BuildTargetId,
        result: Option<Box<dyn Artifact>>,
        #[cfg(feature = "rdf")] relational: &RDFStore,
        #[cfg(feature = "rdf")] load: bool,
    ) -> std::result::Result<(), ArtifactSaveError>;

    #[cfg(feature = "rdf")]
    fn save_triples(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        relational: &RDFStore,
        load: bool,
        iter: Vec<ulo::rdf_types::Triple>,
    );

    fn escape_module_name(&self, in_path: &Path, name: &str) -> PathBuf {
        in_path.join(name.replace('*', "__AST__"))
    }
}

pub trait LocallyBuilt: BuildableArchive {
    fn out_dir(&self) -> &Path;

    fn out_path_of(
        &self,
        path: Option<&UriPath>,
        doc_name: &SimpleUriName,
        rel_path: Option<&UriPath>,
        language: Language,
    ) -> PathBuf;

    fn document_file(
        &self,
        path: Option<&UriPath>,
        rel_path: Option<&UriPath>,
        doc_name: &SimpleUriName,
        language: Language,
    ) -> PathBuf {
        self.out_path_of(path, doc_name, rel_path, language)
            .join("content")
    }

    fn save_modules(&self, modules: &[Module]) -> std::result::Result<(), ArtifactSaveError> {
        for m in modules {
            let path = m.uri.path();
            let name = m.uri.module_name();
            let out = path.map_or_else(
                || self.out_dir().join(".modules"),
                |n| self.out_dir().join_uri_path(n).join(".modules"),
            );
            std::fs::create_dir_all(&out)
                .map_err(|e| ArtifactSaveError::Fs(FileError::Creation(out.clone(), e)))?;
            let out = self.escape_module_name(&out, name.as_ref());
            let file = std::fs::File::create(&out)
                .map_err(|e| ArtifactSaveError::Fs(FileError::Creation(out, e)))?;
            let mut buf = std::io::BufWriter::new(file);
            bincode::encode_into_std_write(m, &mut buf, bincode::config::standard())?;
            //postcard::to_io(m, &mut buf)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct LocalArchive {
    pub(crate) uri: ArchiveUri,
    pub(crate) out_path: PathBuf,
    pub(crate) source: Option<Box<str>>,
    //pub(crate) attributes: Vec<(Box<str>, Box<str>)>,
    pub(crate) formats: smallvec::SmallVec<SourceFormatId, 1>,
    //pub dependencies: Box<[ArchiveId]>,
    pub(crate) file_state: parking_lot::RwLock<SourceDir>,
    //pub(crate) institutions: Box<[Institution]>,
    //pub(crate) index: Box<[ArchiveIndex]>,
    pub ignore: IgnoreSource,
    #[cfg(feature = "git")]
    pub(crate) is_managed: std::sync::OnceLock<Option<flams_git::GitUrl>>,
}
impl MathArchive for LocalArchive {
    #[inline]
    fn uri(&self) -> &ArchiveUri {
        &self.uri
    }
    fn path(&self) -> &Path {
        self.out_path
            .parent()
            .expect("out path of an archive *must* have a parent")
    }

    fn is_meta(&self) -> bool {
        self.uri.archive_id().is_meta()
    }

    fn load_module(&self, path: Option<&UriPath>, name: &UriName) -> Result<Module> {
        let out = path.map_or_else(
            || self.out_dir().join(".modules"),
            |n| self.out_dir().join_uri_path(n).join(".modules"),
        );
        let out = self.escape_module_name(&out, name.as_ref());
        if !out.exists() {
            return Err(BackendError::NotFound(
                ((self.uri.clone() / path.cloned()) | name.clone()).into(),
            ));
        }
        let file = std::io::BufReader::new(std::fs::File::open(out)?);
        let ret: Module = bincode::decode_from_reader(file, bincode::config::standard())?;
        Ok(ret)
    }

    fn load_module_async<A: AsyncEngine>(
        &self,
        path: Option<&UriPath>,
        name: &UriName,
    ) -> impl Future<Output = Result<Module>> + 'static + use<A>
    where
        Self: Sized,
    {
        let out = path.map_or_else(
            || self.out_dir().join(".modules"),
            |n| self.out_dir().join_uri_path(n).join(".modules"),
        );
        let out = self.escape_module_name(&out, name.as_ref());
        let uri = (self.uri.clone() / path.cloned()) | name.clone();
        A::block_on(move || {
            if !out.exists() {
                return Err(BackendError::NotFound(uri.into()));
            }
            let file = std::io::BufReader::new(std::fs::File::open(out)?);
            let ret = bincode::decode_from_reader(file, bincode::config::standard())?;
            Ok(ret)
        })
    }
}
impl BuildableArchive for LocalArchive {
    #[inline]
    fn file_state(&self) -> FileStates {
        self.file_state.read().state().clone()
    }

    #[inline]
    fn formats(&self) -> &[SourceFormatId] {
        &self.formats
    }
    fn get_log(&self, relative_path: &str, target: BuildTargetId) -> PathBuf {
        use std::str::FromStr;
        let rel_path = if let Some((first, lang)) = relative_path.rsplit_once('.')
            && Language::from_str(lang).is_err()
        {
            first
        } else {
            relative_path
        };
        self.out_dir()
            .join(rel_path)
            .join(target.name)
            .with_extension("log")
    }

    fn save(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        log: FileOrString,
        from: BuildTargetId,
        result: Option<Box<dyn Artifact>>,
        #[cfg(feature = "rdf")] relational: &RDFStore,
        #[cfg(feature = "rdf")] load: bool,
    ) -> std::result::Result<(), ArtifactSaveError> {
        let out = self.out_path_of(in_doc.path(), &in_doc.name, rel_path, in_doc.language);
        if let Err(e) = std::fs::create_dir_all(&out) {
            return Err(ArtifactSaveError::Fs(FileError::Creation(out, e)));
        }
        let logfile = out.join(from.name).with_extension("log");
        match log {
            FileOrString::File(f) => f.rename_safe(&logfile)?,
            FileOrString::Str(s) => {
                if let Err(e) = std::fs::write(&logfile, s.as_bytes()) {
                    return Err(ArtifactSaveError::Fs(FileError::Write(logfile, e)));
                }
            }
        }
        let Some(mut res) = result else { return Ok(()) };
        let outfile = out.join(res.kind());
        if res.as_any_mut().downcast_mut::<ContentUpdate>().is_some() {
            // SAFETY: downcast_mut just succeeded
            let e = unsafe {
                res.into_any()
                    .downcast::<ContentUpdate>()
                    .unwrap_unchecked()
            };
            if let Some(d) = e.document {
                let mut cr = ContentResult::read(outfile.clone())?;
                //println!("Parsed result: {cr:#?}");
                cr.document = d;
                cr.write(&outfile)?;
            }
            if !e.modules.is_empty() {
                self.save_modules(&e.modules)?;
            }
            return Ok(());
        }
        res.write(&outfile)?;
        if let Some(e) = res.as_any_mut().downcast_mut::<ContentResult>() {
            #[cfg(feature = "rdf")]
            self.save_triples(
                in_doc,
                rel_path,
                relational,
                load,
                std::mem::take(&mut e.triples),
            );
            self.save_modules(&e.modules)?;
        }
        Ok(())
    }

    #[cfg(feature = "rdf")]
    fn save_triples(
        &self,
        in_doc: &ftml_uris::DocumentUri,
        rel_path: Option<&UriPath>,
        relational: &RDFStore,
        load: bool,
        iter: Vec<ulo::rdf_types::Triple>,
    ) {
        use ftml_uris::FtmlUri;
        let out = self.out_path_of(in_doc.path(), &in_doc.name, rel_path, in_doc.language);
        let _ = std::fs::create_dir_all(&out);
        let out = out.join("index.ttl");
        relational.export(iter.into_iter(), &out, in_doc);
        if load {
            //println!("Loading newly saved rdf triples");
            relational.load(&out, in_doc.to_iri());
        }
    }
}

impl LocallyBuilt for LocalArchive {
    #[inline]
    fn out_dir(&self) -> &Path {
        &self.out_path
    }

    fn out_path_of(
        &self,
        path: Option<&UriPath>,
        doc_name: &SimpleUriName,
        rel_path: Option<&UriPath>,
        language: Language,
    ) -> PathBuf {
        if let Some(rp) = rel_path {
            use std::str::FromStr;
            let mut steps = rp.steps();
            let Some(mut last) = steps.next_back() else {
                //SAFETY steps is never empty
                unsafe { unreachable_unchecked() }
            };
            let out = steps.fold(self.out_dir().to_path_buf(), |p, s| p.join(s));
            if let Some((first, lang)) = last.rsplit_once('.')
                && Language::from_str(lang).is_err()
            {
                last = first;
            }
            return out.join(last);
        }
        self.rel_path_of(path, doc_name, language).map_or_else(
            || {
                let lang: &'static str = language.into();
                let p = path.map_or_else(
                    || self.out_path.join(doc_name.as_ref()),
                    |n| self.out_path.join_uri_path(n).join(doc_name.as_ref()),
                );
                let mp = p.with_added_extension(lang);
                if mp.exists() {
                    mp
                } else {
                    let mp2 = p.with_extension(lang);
                    if mp2 != mp && mp2.exists() { mp2 } else { p }
                }
            },
            |rel_path| {
                // SAFETY source is ancestor of source_dir
                //let rel_path = unsafe { source.relative_to(&self.source_dir()).unwrap_unchecked() };
                self.out_path.join(rel_path)
            },
        )
    }
}
impl LocalArchive {
    pub fn document_of(&self, path: Option<&UriPath>, name: &UriName) -> Option<DocumentUri> {
        let mut mname = name.first();
        let mut file = self.source_dir();
        let maybe_step = if let Some(path) = path {
            let mut steps = path.steps();
            let _ = steps.next_back();
            for step in steps {
                file = file.join(step);
            }
            path.steps().next_back()
        } else {
            None
        };
        if let Some(step) = maybe_step
            && let Ok(mut d) = std::fs::read_dir(file.join(step))
        {
            if let Some(rp) = d.find_map::<String, _>(|p| {
                p.ok().and_then(|p| {
                    let fnm = p.file_name();
                    let name = fnm.as_os_str().as_encoded_bytes();
                    let name = name.strip_prefix(mname.as_bytes())?.strip_prefix(b".")?;
                    let lang = self.formats.iter().find_map(|f| {
                        f.file_extensions.iter().find_map(|e| {
                            name.strip_suffix(e.as_bytes())
                                .and_then(|s| s.strip_suffix(b"."))
                        })
                    })?;
                    if Language::from_str(std::str::from_utf8(lang).ok()?).is_ok() {
                        Some(
                            p.path()
                                .as_os_str()
                                .to_str()?
                                .strip_prefix(self.source_dir().as_os_str().to_str()?)?[1..]
                                .to_string(),
                        )
                    } else {
                        None
                    }
                })
            }) {
                return DocumentUri::from_archive_relpath(self.uri.clone(), &rp).ok();
            }
            mname = step;
        };

        if let Ok(mut d) = std::fs::read_dir(file) {
            if let Some(rp) = d.find_map::<String, _>(|p| {
                p.ok().and_then(|p| {
                    let fnm = p.file_name();
                    let name = fnm.as_os_str().as_encoded_bytes();
                    let Some(name) = name.strip_prefix(mname.as_bytes()) else {
                        return None;
                    };
                    let Some(name) = name.strip_prefix(b".") else {
                        return None;
                    };
                    let Some(lang) = name.strip_suffix(b".tex") else {
                        return None;
                    };
                    if Language::from_str(std::str::from_utf8(lang).ok()?).is_ok() {
                        Some(
                            p.path()
                                .as_os_str()
                                .to_str()?
                                .strip_prefix(self.source_dir().as_os_str().to_str()?)?[1..]
                                .to_string(),
                        )
                    } else {
                        None
                    }
                })
            }) {
                return DocumentUri::from_archive_relpath(self.uri.clone(), &rp).ok();
            }
        };
        None
    }

    #[cfg(feature = "git")]
    pub fn git_url(&self, on_host: &url::Url) -> Option<&flams_git::GitUrl> {
        self.is_managed
            .get_or_init(|| {
                let Ok(repo) = flams_git::repos::GitRepo::open(self.path()) else {
                    return None;
                };
                on_host.host_str().and_then(|s| repo.is_managed(s))
            })
            .as_ref()
    }

    pub fn state_summary(&self) -> FileStateSummary {
        self.file_state.read().state().summarize()
    }

    #[must_use]
    pub fn source_dir(&self) -> PathBuf {
        self.path().join(self.source.as_deref().unwrap_or("source"))
    }

    /// blocks
    #[must_use]
    pub fn manifest_of(p: &Path) -> Option<PathBuf> {
        for e in std::fs::read_dir(p).ok()? {
            let Ok(e) = e else { continue };
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() && e.file_name().eq_ignore_ascii_case("meta-inf") {
                return crate::archive_iter::find_manifest(&e.path());
            }
        }
        None
    }

    #[inline]
    #[must_use]
    fn out_dir_of(p: &Path) -> PathBuf
    where
        Self: Sized,
    {
        p.join(".flams")
    }

    #[inline]
    pub fn with_sources<R>(&self, f: impl FnOnce(&SourceDir) -> R) -> R {
        f(&self.file_state.read())
    }

    pub fn update_sources(&self) {
        let dir = SourceDir::new(&self.source_dir(), &self.ignore, self.formats());
        let mut state = self.file_state.write();
        state.update(dir);
    }

    /// blocks! removes File extension!
    pub fn rel_path_of(
        &self,
        path: Option<&UriPath>,
        doc_name: &SimpleUriName,
        language: Language,
    ) -> Option<PathBuf> {
        let dir = path.map_or_else(|| self.source_dir(), |n| self.source_dir().join_uri_path(n));
        for f in std::fs::read_dir(&dir)
            .ok()?
            .filter_map(std::result::Result::ok)
        {
            let Ok(m) = f.metadata() else { continue };
            if !m.is_file() {
                continue;
            }
            let fname = f.file_name();
            let Some(name) = fname.to_str() else { continue };
            let Some((_, ext)) = name.rsplit_once('.') else {
                continue;
            };
            if !self
                .formats
                .iter()
                .flat_map(|sf| sf.file_extensions.iter())
                .any(|e| *e == ext)
            {
                continue;
            }

            if !name.starts_with(doc_name.as_ref()) {
                continue;
            }
            let rest = &name[doc_name.as_ref().len()..];
            if !rest.is_empty() && !rest.starts_with('.') {
                continue;
            }
            let rest = rest.strip_prefix('.').unwrap_or(rest);
            if rest.contains('.') {
                let lang: &'static str = language.into();
                if !rest.starts_with(lang) {
                    continue;
                }
            }
            let path = f
                .path()
                .strip_prefix(self.source_dir())
                .ok()?
                .with_extension("");
            return Some(path);
        }
        None
    }
}
