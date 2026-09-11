use std::path::Path;

#[cfg(feature = "rdf")]
use ftml_uris::FtmlUri;
use ftml_uris::{ArchiveId, DocumentUri, ModuleUri, UriPath};

use crate::{
    artifacts::{Artifact, FileOrString},
    backend::AnyBackend,
};

pub mod __reexport {
    pub use inventory::*;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FormatOrTargets<'a> {
    Format(SourceFormatId),
    Targets(&'a [BuildTargetId]),
}

#[derive(Copy, Clone, Debug)]
pub struct SourceFormat {
    pub name: &'static str,
    pub description: &'static str,
    pub targets: &'static [BuildTargetId],
    pub file_extensions: &'static [&'static str],
    pub dependencies: fn(BuildSpec) -> Vec<(BuildTargetId, TaskDependency)>,
}
impl SourceFormat {
    #[inline]
    #[must_use]
    pub const fn id(&'static self) -> SourceFormatId {
        SourceFormatId(self)
    }
}
inventory::collect!(SourceFormat);
#[macro_export]
macro_rules! source_format {
    ($i:ident { $($t:tt)* }) => {
        pub static $i : $crate::formats::SourceFormat = $crate::formats::SourceFormat { $($t)* };
        $crate::formats::__reexport::submit!{ $i }
    };
}

impl SourceFormat {
    #[inline]
    pub fn all() -> impl Iterator<Item = SourceFormatId> {
        inventory::iter.into_iter().map(SourceFormatId)
    }
    #[must_use]
    pub fn get(name: &str) -> Option<SourceFormatId> {
        Self::all().find(|e| e.name == name)
    }
}

pub struct BuildResult {
    pub log: FileOrString,
    pub result: Result<Option<Box<dyn Artifact>>, Vec<TaskDependency>>,
}
impl BuildResult {
    #[must_use]
    pub fn err() -> Self {
        Self {
            log: FileOrString::Str(String::new().into_boxed_str()),
            result: Err(Vec::new()),
        }
    }
}
impl Default for BuildResult {
    fn default() -> Self {
        Self {
            log: FileOrString::Str(String::new().into_boxed_str()),
            result: Ok(None),
        }
    }
}

pub struct BuildSpec<'a> {
    pub uri: &'a DocumentUri,
    pub source: either::Either<&'a Path, &'a str>,
    pub backend: &'a AnyBackend,
    pub rel_path: &'a UriPath,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TaskRef {
    pub archive: ArchiveId,
    pub rel_path: UriPath,
    pub target: BuildTargetId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskDependency {
    Physical { task: TaskRef, strict: bool },
    Logical { uri: ModuleUri, strict: bool },
}

#[derive(Copy, Clone, Debug)]
pub struct BuildTarget {
    pub name: &'static str,
    pub description: &'static str,
    // dependencies
    // yields
    pub run: fn(BuildSpec<'_>) -> BuildResult,
}
impl BuildTarget {
    #[inline]
    #[must_use]
    pub const fn id(&'static self) -> BuildTargetId {
        BuildTargetId(self)
    }
}
inventory::collect!(BuildTarget);
#[macro_export]
macro_rules! build_target {
    ($i:ident { $($t:tt)* }) => {
        pub static $i : $crate::formats::BuildTarget = $crate::formats::BuildTarget { $($t)* };
        $crate::formats::__reexport::submit!{ $i }
    };
}

impl BuildTarget {
    #[inline]
    pub fn all() -> impl Iterator<Item = BuildTargetId> {
        inventory::iter.into_iter().map(BuildTargetId)
    }
    #[must_use]
    pub fn get(name: &str) -> Option<BuildTargetId> {
        Self::all().find(|e| e.name == name)
    }
}

#[derive(Copy, Clone)]
pub struct SourceFormatId(&'static SourceFormat);
impl std::ops::Deref for SourceFormatId {
    type Target = &'static SourceFormat;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl PartialEq for SourceFormatId {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self.0, other.0)
    }
}
impl Eq for SourceFormatId {}
impl std::hash::Hash for SourceFormatId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.name.hash(state);
    }
}
impl std::fmt::Debug for SourceFormatId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.name.fmt(f)
    }
}

impl std::fmt::Display for SourceFormatId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.name.fmt(f)
    }
}

impl ::serde::Serialize for SourceFormatId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        serializer.serialize_str(self.0.name)
    }
}

impl<'de> ::serde::Deserialize<'de> for SourceFormatId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SourceFormat::get(&s).map_or_else(
            || Err(::serde::de::Error::custom("Unknown source format")),
            Ok,
        )
    }
}

#[derive(Copy, Clone)]
pub struct BuildTargetId(&'static BuildTarget);
impl std::ops::Deref for BuildTargetId {
    type Target = &'static BuildTarget;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl PartialEq for BuildTargetId {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self.0, other.0)
    }
}
impl Eq for BuildTargetId {}
impl std::hash::Hash for BuildTargetId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.name.hash(state);
    }
}
impl std::fmt::Debug for BuildTargetId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.name.fmt(f)
    }
}

impl std::fmt::Display for BuildTargetId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.name.fmt(f)
    }
}

impl ::serde::Serialize for BuildTargetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        serializer.serialize_str(self.0.name)
    }
}

impl<'de> ::serde::Deserialize<'de> for BuildTargetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        BuildTarget::get(&s).map_or_else(
            || Err(::serde::de::Error::custom("Unknown build target")),
            Ok,
        )
    }
}

#[cfg(feature = "rdf")]
source_format! { RDF {
    name:"rdf",
    description:"Import .ttl-RDF-files into the triple store",
    targets:&[RDF_IMPORT.id()],
    file_extensions: &["ttl"],
    dependencies: |_| Vec::new()
}}

#[cfg(feature = "rdf")]
build_target! { RDF_IMPORT {
    name:"import-rdf",
    description:"imports existent .ttl file",
    run: |s| {
        let v = match s.source {
            either::Left(path) => {
                let file = match std::fs::File::open(path) {
                    Err(e) => return BuildResult {
                            log: FileOrString::Str(format!("Error reading {}: {e}",path.display()).into_boxed_str()),
                            result: Err(Vec::new())
                        },
                    Ok(f) => f
                };
                let buf = std::io::BufReader::new(file);
                let reader = oxigraph::io::RdfParser::from_format(oxigraph::io::RdfFormat::Turtle)
                    .for_reader(buf);
                match reader.map(|q| q.map(|q| ulo::rdf_types::Triple {
                    subject:q.subject,
                    predicate:q.predicate,
                    object:q.object
                })).collect::<Result<Vec<_>,_>>() {
                    Ok(v) => v,
                    Err(e) => return BuildResult {
                            log: FileOrString::Str(format!("Error reading {}: {e}",path.display()).into_boxed_str()),
                            result: Err(Vec::new())
                        }
                }
            }
            either::Right(txt) => {
                let reader = oxigraph::io::RdfParser::from_format(oxigraph::io::RdfFormat::Turtle)
                    .for_reader(txt.as_bytes());
                match reader.map(|q| q.map(|q| ulo::rdf_types::Triple {
                    subject:q.subject,
                    predicate:q.predicate,
                    object:q.object
                })).collect::<Result<Vec<_>,_>>() {
                    Ok(v) => v,
                    Err(e) => return BuildResult {
                            log: FileOrString::Str(format!("Error reading ttl: {e}").into_boxed_str()),
                            result: Err(Vec::new())
                        }
                }
            }
        };
        BuildResult {
            log: FileOrString::Str(String::new().into_boxed_str()),
            result: Ok(Some(Box::new(v)))
        }
    }
}}
