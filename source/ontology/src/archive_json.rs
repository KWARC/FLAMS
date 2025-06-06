use crate::{
    file_states::FileStateSummary,
    uris::{ArchiveId, ArchiveURI, ArchiveURITrait, DocumentURI},
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ArchiveDatum {
    Document(DocumentKind),
    Institution(Institution),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum DocumentKind {
    #[serde(rename = "library")]
    Library {
        title: Box<str>,
        teaser: Option<Box<str>>,
        thumbnail: Option<Box<str>>,
    },
    #[serde(rename = "book")]
    Book {
        title: Box<str>,
        authors: Vec<Person>,
        file: Box<str>,
        thumbnail: Option<Box<str>>,
        teaser: Option<Box<str>>,
    },
    #[serde(rename = "paper")]
    Paper {
        title: Box<str>,
        authors: Vec<Person>,
        file: Box<str>,
        thumbnail: Option<Box<str>>,
        teaser: Option<Box<str>>,
        venue: Option<Box<str>>,
        venue_url: Option<Box<str>>,
    },
    #[serde(rename = "course")]
    Course {
        title: Box<str>,
        landing: Box<str>,
        acronym: Option<Box<str>>,
        instructors: Vec<Person>,
        institution: Box<str>,
        notes: Box<str>,
        slides: Option<Box<str>>,
        thumbnail: Option<Box<str>>,
        #[serde(default)]
        quizzes: bool,
        #[serde(default)]
        homeworks: bool,
        #[serde(default)]
        instances: Vec<PreInstance>,
        teaser: Option<Box<str>>,
    },
    #[serde(rename = "self-study")]
    SelfStudy {
        title: Box<str>,
        landing: Box<str>,
        acronym: Option<Box<str>>,
        notes: Box<str>,
        slides: Option<Box<str>>,
        teaser: Option<Box<str>>,
        thumbnail: Option<Box<str>>,
    },
}
impl DocumentKind {
    #[inline]
    #[must_use]
    pub fn teaser(&self) -> Option<&str> {
        match self {
            Self::Library { teaser, .. }
            | Self::Book { teaser, .. }
            | Self::Paper { teaser, .. }
            | Self::Course { teaser, .. }
            | Self::SelfStudy { teaser, .. } => teaser.as_deref(),
        }
    }
    pub fn set_teaser(&mut self, new_teaser: Box<str>) {
        match self {
            Self::Library { teaser, .. }
            | Self::Book { teaser, .. }
            | Self::Paper { teaser, .. }
            | Self::Course { teaser, .. }
            | Self::SelfStudy { teaser, .. } => *teaser = Some(new_teaser),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Institution {
    #[serde(rename = "university")]
    University {
        title: Box<str>,
        place: Box<str>,
        country: Box<str>,
        url: Box<str>,
        acronym: Box<str>,
        logo: Box<str>,
    },
    #[serde(rename = "school")]
    School {
        title: Box<str>,
        place: Box<str>,
        country: Box<str>,
        url: Box<str>,
        acronym: Box<str>,
        logo: Box<str>,
    },
}
impl Institution {
    #[inline]
    #[must_use]
    pub const fn acronym(&self) -> &str {
        match self {
            Self::University { acronym, .. } | Self::School { acronym, .. } => acronym,
        }
    }
    #[inline]
    #[must_use]
    pub const fn url(&self) -> &str {
        match self {
            Self::University { url, .. } | Self::School { url, .. } => url,
        }
    }
    #[inline]
    #[must_use]
    pub const fn title(&self) -> &str {
        match self {
            Self::University { title, .. } | Self::School { title, .. } => title,
        }
    }
    #[inline]
    #[must_use]
    pub const fn logo(&self) -> &str {
        match self {
            Self::University { logo, .. } | Self::School { logo, .. } => logo,
        }
    }
}
impl PartialEq for Institution {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::University { title: t1, .. }, Self::University { title: t2, .. })
            | (Self::School { title: t1, .. }, Self::School { title: t2, .. }) => t1 == t2,
            _ => false,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Person {
    pub name: Box<str>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PreInstance {
    pub semester: Box<str>,
    pub instructors: Option<Vec<Person>>,
    #[serde(rename = "TAs")]
    pub tas: Option<Vec<Person>>,
    #[serde(rename = "leadTAs")]
    pub lead_tas: Option<Vec<Person>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum ArchiveIndex {
    #[serde(rename = "library")]
    Library {
        archive: ArchiveId,
        title: Box<str>,
        #[serde(default)]
        teaser: Option<Box<str>>,
        #[serde(default)]
        thumbnail: Option<Box<str>>,
    },
    #[serde(rename = "book")]
    Book {
        title: Box<str>,
        authors: Box<[Box<str>]>,
        file: DocumentURI,
        #[serde(default)]
        teaser: Option<Box<str>>,
        #[serde(default)]
        thumbnail: Option<Box<str>>,
    },
    #[serde(rename = "paper")]
    Paper {
        title: Box<str>,
        authors: Box<[Box<str>]>,
        file: DocumentURI,
        #[serde(default)]
        thumbnail: Option<Box<str>>,
        #[serde(default)]
        teaser: Option<Box<str>>,
        #[serde(default)]
        venue: Option<Box<str>>,
        #[serde(default)]
        venue_url: Option<Box<str>>,
    },
    #[serde(rename = "course")]
    Course {
        title: Box<str>,
        landing: DocumentURI,
        acronym: Option<Box<str>>,
        instructors: Box<[Box<str>]>,
        institution: Box<str>,
        instances: Box<[Instance]>,
        notes: DocumentURI,
        #[serde(default)]
        slides: Option<DocumentURI>,
        #[serde(default)]
        thumbnail: Option<Box<str>>,
        #[serde(default)]
        quizzes: bool,
        #[serde(default)]
        homeworks: bool,
        #[serde(default)]
        teaser: Option<Box<str>>,
    },
    #[serde(rename = "self-study")]
    SelfStudy {
        title: Box<str>,
        landing: DocumentURI,
        notes: DocumentURI,
        #[serde(default)]
        acronym: Option<Box<str>>,
        #[serde(default)]
        slides: Option<DocumentURI>,
        #[serde(default)]
        thumbnail: Option<Box<str>>,
        #[serde(default)]
        teaser: Option<Box<str>>,
    },
}
impl ArchiveIndex {
    #[inline]
    #[must_use]
    pub fn teaser(&self) -> Option<&str> {
        match self {
            Self::Library { teaser, .. }
            | Self::Book { teaser, .. }
            | Self::Paper { teaser, .. }
            | Self::Course { teaser, .. }
            | Self::SelfStudy { teaser, .. } => teaser.as_deref(),
        }
    }
    pub fn set_teaser(&mut self, new_teaser: Box<str>) {
        match self {
            Self::Library { teaser, .. }
            | Self::Book { teaser, .. }
            | Self::Paper { teaser, .. }
            | Self::Course { teaser, .. }
            | Self::SelfStudy { teaser, .. } => *teaser = Some(new_teaser),
        }
    }
}
impl Eq for ArchiveIndex {}
impl PartialEq for ArchiveIndex {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Library { archive: a1, .. }, Self::Library { archive: a2, .. }) => a1 == a2,
            (Self::Book { file: f1, .. }, Self::Book { file: f2, .. })
            | (Self::Course { notes: f1, .. }, Self::Course { notes: f2, .. })
            | (Self::Paper { file: f1, .. }, Self::Paper { file: f2, .. })
            | (Self::SelfStudy { notes: f1, .. }, Self::SelfStudy { notes: f2, .. }) => f1 == f2,
            _ => false,
        }
    }
}
impl ArchiveIndex {
    pub fn from_kind(
        d: DocumentKind,
        a: &ArchiveURI,
        images: impl FnMut(Box<str>) -> Box<str>,
    ) -> eyre::Result<Self> {
        Ok(match d {
            DocumentKind::Library {
                title,
                teaser,
                thumbnail,
            } => Self::Library {
                archive: a.archive_id().clone(),
                title,
                teaser,
                thumbnail: if thumbnail.as_ref().is_some_and(|s| s.is_empty()) {
                    None
                } else {
                    thumbnail.map(images)
                },
            },
            DocumentKind::Book {
                title,
                authors,
                file,
                teaser,
                thumbnail,
            } => Self::Book {
                title,
                teaser,
                file: DocumentURI::from_archive_relpath(a.clone(), &file)?,
                authors: authors.into_iter().map(|is| is.name).collect(),
                thumbnail: if thumbnail.as_ref().is_some_and(|s| s.is_empty()) {
                    None
                } else {
                    thumbnail.map(images)
                },
            },
            DocumentKind::Paper {
                title,
                authors,
                file,
                teaser,
                thumbnail,
                venue,
                venue_url,
            } => Self::Paper {
                title,
                teaser,
                venue,
                venue_url,
                file: DocumentURI::from_archive_relpath(a.clone(), &file)?,
                authors: authors.into_iter().map(|is| is.name).collect(),
                thumbnail: if thumbnail.as_ref().is_some_and(|s| s.is_empty()) {
                    None
                } else {
                    thumbnail.map(images)
                },
            },
            DocumentKind::Course {
                title,
                landing,
                acronym,
                instructors,
                institution,
                notes,
                slides,
                thumbnail,
                quizzes,
                homeworks,
                instances,
                teaser,
            } => Self::Course {
                title,
                acronym,
                institution,
                quizzes,
                homeworks,
                teaser,
                landing: DocumentURI::from_archive_relpath(a.clone(), &landing)?,
                thumbnail: if thumbnail.as_ref().is_some_and(|s| s.is_empty()) {
                    None
                } else {
                    thumbnail.map(images)
                },
                notes: DocumentURI::from_archive_relpath(a.clone(), &notes)?,
                slides: if slides.as_ref().is_some_and(|s| s.is_empty()) {
                    None
                } else {
                    slides
                        .map(|s| DocumentURI::from_archive_relpath(a.clone(), &s))
                        .transpose()?
                },
                instances: instances
                    .into_iter()
                    .map(|i| Instance {
                        semester: i.semester,
                        instructors: i
                            .instructors
                            .map(|is| is.into_iter().map(|i| i.name).collect()),
                        tas: i.tas.map(|is| is.into_iter().map(|i| i.name).collect()),
                        lead_tas: i
                            .lead_tas
                            .map(|is| is.into_iter().map(|i| i.name).collect()),
                    })
                    .collect(),
                instructors: instructors.into_iter().map(|is| is.name).collect(),
            },
            DocumentKind::SelfStudy {
                title,
                landing,
                acronym,
                notes,
                slides,
                thumbnail,
                teaser,
            } => Self::SelfStudy {
                title,
                acronym,
                teaser,
                landing: DocumentURI::from_archive_relpath(a.clone(), &landing)?,
                thumbnail: if thumbnail.as_ref().is_some_and(|s| s.is_empty()) {
                    None
                } else {
                    thumbnail.map(images)
                },
                notes: DocumentURI::from_archive_relpath(a.clone(), &notes)?,
                slides: if slides.as_ref().is_some_and(|s| s.is_empty()) {
                    None
                } else {
                    slides
                        .map(|s| DocumentURI::from_archive_relpath(a.clone(), &s))
                        .transpose()?
                },
            },
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Instance {
    pub semester: Box<str>,
    #[serde(default)]
    pub instructors: Option<Box<[Box<str>]>>,
    #[serde(rename = "TAs")]
    #[serde(default)]
    pub tas: Option<Box<[Box<str>]>>,
    #[serde(rename = "leadTAs")]
    #[serde(default)]
    pub lead_tas: Option<Box<[Box<str>]>>,
}

#[cfg(unix)]
#[test]
fn test() {
    use std::os::unix::ffi::OsStrExt;
    tracing_subscriber::fmt().init();
    let mathhubs: Vec<_> = std::env::var("MATHHUB")
        .expect("No MathHub directory")
        .split(',')
        .map(|s| std::path::PathBuf::from(s.trim()))
        .collect();

    for m in mathhubs {
        for entry in walkdir::WalkDir::new(m) {
            let entry = entry.expect("Error reading directory");
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|s| s.as_bytes() == b"json")
                && entry
                    .path()
                    .file_stem()
                    .is_some_and(|s| s.as_bytes() == b"archive")
            {
                tracing::info!("File: {}", entry.path().display());
                let data = std::fs::read_to_string(entry.path()).expect("Error reading file");
                let data: Vec<ArchiveDatum> =
                    serde_json::from_str(&data).expect("Error parsing JSON");
                for d in data {
                    tracing::info!("{d:#?}");
                }
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ArchiveData {
    pub id: ArchiveId,
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub summary: Option<FileStateSummary>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ArchiveGroupData {
    pub id: ArchiveId,
    #[serde(default)]
    pub summary: Option<FileStateSummary>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DirectoryData {
    pub rel_path: String,
    #[serde(default)]
    pub summary: Option<FileStateSummary>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FileData {
    pub rel_path: String,
    pub format: String, //pub summary:Option<FileStateSummary>,
}
