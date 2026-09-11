use crate::{
    document_file::DocumentFile,
    utils::{
        errors::{ArtifactSaveError, FileError, WriteError},
        lazy_file::{LazyField, LazyFile, LazyFileWriter},
        path_ext::PathExt,
    },
};
use ftml_ontology::{
    domain::modules::Module,
    narrative::{DocumentRange, documents::Document},
    utils::Css,
};
use std::path::{Path, PathBuf};

pub enum FileOrString {
    File(PathBuf),
    Str(Box<str>),
}

pub trait Artifact: std::any::Any {
    fn kind(&self) -> &'static str;
    /// # Errors
    fn write(&self, into: &Path) -> Result<(), ArtifactSaveError>;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn as_any(&self) -> &dyn std::any::Any;
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

pub trait FileArtifact: std::any::Any {
    fn kind(&self) -> &'static str;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn as_any(&self) -> &dyn std::any::Any;
    fn source(&self) -> &Path;
}
impl<F: FileArtifact> Artifact for F {
    #[inline]
    fn kind(&self) -> &'static str {
        <Self as FileArtifact>::kind(self)
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        <Self as FileArtifact>::as_any_mut(self)
    }
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        <Self as FileArtifact>::as_any(self)
    }
    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self as _
    }
    fn write(&self, into: &Path) -> Result<(), ArtifactSaveError> {
        self.source().rename_safe(&into)?;
        Ok(())
    }
}

pub struct FtmlString(pub Box<str>);
impl Artifact for FtmlString {
    #[inline]
    fn kind(&self) -> &'static str {
        "ftml"
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self as _
    }
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self as _
    }
    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self as _
    }
    fn write(&self, into: &Path) -> Result<(), ArtifactSaveError> {
        std::fs::write(into, self.0.as_bytes())
            .map_err(|e| ArtifactSaveError::Fs(FileError::Write(into.to_path_buf(), e)))
    }
}
pub struct FtmlFile(pub PathBuf);
impl Artifact for FtmlFile {
    #[inline]
    fn kind(&self) -> &'static str {
        "ftml"
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self as _
    }
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self as _
    }
    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self as _
    }
    fn write(&self, into: &Path) -> Result<(), ArtifactSaveError> {
        std::fs::copy(&self.0, into)
            .map(|_| ())
            .map_err(|e| ArtifactSaveError::Fs(FileError::Write(into.to_path_buf(), e)))
    }
}

#[cfg(feature = "rdf")]
impl Artifact for Vec<ulo::rdf_types::Triple> {
    #[inline]
    fn kind(&self) -> &'static str {
        "index.ttl"
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self as _
    }
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self as _
    }
    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self as _
    }
    fn write(&self, into: &Path) -> Result<(), ArtifactSaveError> {
        let file = match std::fs::File::create(into) {
            Ok(f) => f,
            Err(e) => {
                return Err(ArtifactSaveError::Other(
                    format!("error writing rdf: {e}").into(),
                ));
            }
        };
        let writer = std::io::BufWriter::new(file);
        let mut writer = oxigraph::io::RdfSerializer::from_format(oxigraph::io::RdfFormat::Turtle)
            .for_writer(writer);
        for t in self {
            let _ = writer.serialize_triple(t);
        }
        let _ = writer.finish();
        Ok(())
    }
}

#[derive(Debug)]
pub struct ContentResult {
    pub body: DocumentRange,
    pub inner_offset: u32,
    pub css: Box<[Css]>,
    pub data: Box<[u8]>,
    pub document: Document,
    pub ftml: Box<str>,
    pub modules: Vec<Module>,
    #[cfg(feature = "rdf")]
    pub triples: Vec<ulo::rdf_types::Triple>,
}
impl ContentResult {
    const NUM_FIELDS: usize = 6;
    /// ### Errors
    pub fn read(path: PathBuf) -> Result<Self, ArtifactSaveError> {
        macro_rules! err {
            (F $e:expr) => {
                match $e {
                    Ok(r) => r,
                    Err(e) => return Err(ArtifactSaveError::Fs(FileError::ReadEntry(path, e))),
                }
            };
            ($e:expr) => {
                $e.map_err(|e| ArtifactSaveError::Other(e.to_string().into()))?
            };
        }
        let f = err!(DocumentFile::from_file(path));
        let (body, inner_offset, css, data, document, ftml) = err!(f.get_all());
        Ok(Self {
            body,
            inner_offset,
            css,
            data,
            document,
            ftml,
            modules: Vec::new(),
            #[cfg(feature = "rdf")]
            triples: Vec::new(),
        })
    }
}
impl Artifact for ContentResult {
    #[inline]
    fn kind(&self) -> &'static str {
        "content"
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self as _
    }
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self as _
    }
    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self as _
    }
    fn write(&self, into: &Path) -> Result<(), ArtifactSaveError> {
        let mut writer = match LazyFileWriter::<{ Self::NUM_FIELDS }>::new(into) {
            Ok(w) => w,
            Err(e) => {
                return Err(ArtifactSaveError::Fs(FileError::Write(
                    into.to_path_buf(),
                    e,
                )));
            }
        };
        macro_rules! err {
            ($e:expr) => {
                if let Err(e) = $e {
                    match e {
                        WriteError::Io(e) => {
                            return Err(ArtifactSaveError::Fs(FileError::Write(
                                into.to_path_buf(),
                                e,
                            )));
                        }
                        WriteError::Encode(e) => return Err(ArtifactSaveError::Encode(e)),
                        _ => unreachable!(),
                    }
                }
            };
        }
        err!(writer.write(&self.body));
        err!(writer.write(&self.inner_offset));
        err!(writer.write(&self.css));
        err!(writer.write_bytes(&self.data));
        err!(writer.write(&self.document));
        err!(writer.write_string(&self.ftml));
        Ok(())
    }
}
#[derive(Debug)]
pub struct ContentUpdate {
    pub document: Option<Document>,
    pub modules: Vec<Module>,
}
impl Artifact for ContentUpdate {
    #[inline]
    fn kind(&self) -> &'static str {
        "content"
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self as _
    }
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self as _
    }
    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self as _
    }
    fn write(&self, _: &Path) -> Result<(), ArtifactSaveError> {
        Err(ArtifactSaveError::Other(
            "Cannot write update in isolation".into(),
        ))
    }
}
