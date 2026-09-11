#![allow(unexpected_cfgs)]
#![cfg_attr(all(doc, CHANNEL_NIGHTLY), feature(doc_cfg))]
#![doc = include_str!("../README.md")]
/*!
 * ## Feature flags
 */
#![cfg_attr(doc,doc = document_features::document_features!())]

use crate::index::SearchIndex;
use flams_backend_types::search::FragmentQueryFilter;
use flams_backend_types::search::SearchResult;
use flams_math_archives::{
    Archive, LocallyBuilt,
    artifacts::{Artifact, ContentResult, FileOrString},
    backend::{AnyBackend, GlobalBackend, LocalBackend},
    build_target,
    formats::BuildResult,
    utils::errors::{ArtifactSaveError, FileError},
};
use flams_system::FlamsExtension;
use ftml_uris::DocumentElementUri;
use ftml_uris::{DocumentUri, SymbolUri, UriPath, UriWithArchive};

#[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
use crate::schema::SearchSchema;

pub mod index;
pub mod query;
#[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
pub mod schema;
pub mod textify;

#[cfg(feature = "tantivy")]
const MEMORY_SIZE: usize = 150_000_000;

flams_system::register_exension!(FlamsExtension {
    name: "search",
    on_start: initialize,
    on_build_result: |b, uri, rel_path, a| {
        //println!("Here");
        if let Some(content) = a.as_any().downcast_ref::<ContentResult>() {
            index(b, uri, rel_path, content);
        }
    },
    on_reload: initialize
});

#[cfg(feature = "tantivy")]
build_target!(TANTIVY {
    name: "tantivy_search",
    description: "search index",
    run: |_| BuildResult::default()
});

#[cfg(feature = "vectorsearch")]
build_target!(VECTORSEARCH {
    name: "vector_search",
    description: "search index",
    run: |_| BuildResult::default()
});

// -------------------------------------------------

#[cfg(feature = "vectorsearch")]
pub(crate) struct Embedder;

#[cfg(feature = "vectorsearch")]
impl Embedder {
    pub fn embed<S: AsRef<str> + Send + Sync>(
        texts: impl AsRef<[S]>,
    ) -> Result<Vec<flams_backend_types::search::Embedding>, String> {
        MODEL.as_ref().map_or_else(
            || Err("No model".to_string()),
            |lock| {
                let mut model = lock.lock();
                model.embed(texts, None).map_or_else(
                    |e| Err(format!("Error embedding texts: {e}")),
                    |r| {
                        Ok(r.into_iter()
                            .map(|v| {
                                // SAFETY: invariant
                                let boxed = unsafe { v.try_into().unwrap_unchecked() };
                                flams_backend_types::search::Embedding::new(boxed)
                            })
                            .collect())
                    },
                )
            },
        )
    }
}

#[cfg(feature = "vectorsearch")]
pub(crate) static MODEL: std::sync::LazyLock<Option<parking_lot::Mutex<fastembed::TextEmbedding>>> =
    std::sync::LazyLock::new(|| {
        tracing::info_span!("initializing vector search model").in_scope(|| {
            use flams_system::settings::CONFIG_DIR;
            // https://ort.pyke.io/backends/candle
            //ort::set_api(ort_candle::api());
            let model_path = flams_system::settings::Settings::get()
                .embedding_dir
                .as_ref()
                .map_or_else(
                    || {
                        CONFIG_DIR
                            .as_ref()
                            .expect("no default directory")
                            .join("embedding")
                    },
                    |d| (*d).to_path_buf(),
                );

            match fastembed::TextEmbedding::try_new(
                fastembed::InitOptions::new(fastembed::EmbeddingModel::ParaphraseMLMiniLML12V2Q)
                    .with_show_download_progress(false)
                    .with_cache_dir(model_path),
            ) {
                Ok(m) => {
                    tracing::info!("Model initialized");
                    Some(parking_lot::Mutex::new(m))
                }
                Err(e) => {
                    tracing::error!("Error downloading embedding model: {e}");
                    None
                }
            }
        })
    });

// -------------------------------------------------

static SEARCHER: std::sync::LazyLock<Searcher> = std::sync::LazyLock::new(Searcher::new);
static SPAN: std::sync::LazyLock<tracing::Span> =
    std::sync::LazyLock::new(|| tracing::info_span!(target:"search",parent:None,"search"));

#[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
pub struct Searcher {
    index: parking_lot::RwLock<tantivy::index::Index>,
    reader: parking_lot::RwLock<tantivy::IndexReader>,
    writer: parking_lot::Mutex<()>,
}

#[cfg(feature = "vectorsearch")]
pub struct Searcher {
    index: parking_lot::RwLock<Vec<SearchIndex>>,
}

impl Searcher {
    #[inline]
    #[must_use]
    pub fn get() -> &'static Self {
        &SEARCHER
    }

    #[cfg(feature = "vectorsearch")]
    pub fn size(&self) -> (usize, usize) {
        let slf = self.index.read();
        (slf.len(), slf.len() * std::mem::size_of::<SearchIndex>())
    }

    #[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
    pub fn size(&self) -> (usize, usize) {
        let reader = self.reader.read();
        (
            reader.searcher().num_docs() as usize,
            reader
                .searcher()
                .space_usage()
                .expect("test")
                .total()
                .get_bytes() as usize,
        )
    }

    #[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
    fn new() -> Self {
        let index =
            tantivy::index::Index::create_in_ram(schema::SearchSchema::get().schema.clone());
        Self {
            reader: parking_lot::RwLock::new(index.reader().expect("Failed to build reader")),
            index: parking_lot::RwLock::new(index),
            writer: parking_lot::Mutex::new(()),
        }
    }

    #[cfg(feature = "vectorsearch")]
    #[inline]
    const fn new() -> Self {
        Self {
            index: parking_lot::RwLock::new(Vec::new()),
        }
    }

    #[cfg(feature = "vectorsearch")]
    #[inline]
    pub fn add_one(&self, index: SearchIndex) {
        self.index.write().push(index);
    }

    #[cfg(feature = "vectorsearch")]
    #[inline]
    pub fn add(&self, iter: impl IntoIterator<Item = SearchIndex>) {
        self.index.write().extend(iter);
    }

    #[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
    pub fn query(
        &self,
        s: &str,
        mut opts: FragmentQueryFilter,
        num_results: usize,
    ) -> Result<Vec<(f32, SearchResult)>, String> {
        SPAN.in_scope(move || {
            let searcher = self.reader.read().searcher();
            let in_documents = std::mem::take(&mut opts.in_documents)
                .into_iter()
                .map(|u| u.to_string())
                .collect::<Vec<_>>();
            let query = query::build_query(s, &self.index.read(), opts)
                .ok_or_else(|| "erro building query".to_string())?;
            let top_num = if num_results == 0 {
                usize::MAX / 2
            } else {
                num_results
            };
            let mut ret = Vec::new();
            let iter = if in_documents.is_empty() {
                searcher
                    .search(
                        &*query,
                        &tantivy::collector::TopDocs::with_limit(top_num).order_by_score(),
                    )
                    .map_err(|e| e.to_string())?
            } else {
                searcher
                    .search(
                        &*query,
                        &tantivy::collector::BytesFilterCollector::new(
                            "uri".to_string(),
                            move |u: &[u8]| {
                                in_documents.iter().any(|d| u.starts_with(d.as_bytes()))
                            },
                            tantivy::collector::TopDocs::with_limit(top_num).order_by_score(),
                        ),
                    )
                    .map_err(|e| e.to_string())?
            };
            for (s, a) in iter {
                let Ok(doc) = searcher.doc::<tantivy::schema::TantivyDocument>(a) else {
                    continue;
                };
                if let Some(doc) = SearchIndex::from_document(doc) {
                    ret.push((s, doc));
                };
            }
            Ok(ret)
        })
    }

    #[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
    #[allow(clippy::type_complexity)]
    pub fn query_symbols(
        &self,
        s: &str,
        num_results: usize,
    ) -> Option<Vec<(f32, SymbolUri, DocumentElementUri)>> {
        SPAN.in_scope(move || {
            const FILTER: FragmentQueryFilter = {
                use flams_backend_types::search::QueryFilterFlags;

                let mut f = FragmentQueryFilter::new();
                f.flags = QueryFilterFlags::definition_like_only();
                f
            };
            let searcher = self.reader.read().searcher();

            let query = query::build_query(s, &self.index.read(), FILTER)?;
            let top_num = if num_results == 0 {
                usize::MAX / 2
            } else {
                num_results
            };
            let mut ret: Vec<(f32, SymbolUri, DocumentElementUri)> = Vec::new();
            for (score, a) in searcher
                .search(
                    &*query,
                    &tantivy::collector::TopDocs::with_limit(top_num * 3).order_by_score(),
                )
                .map_err(|e| tracing::error!("Search Error A: {e}"))
                .ok()?
            {
                let Ok(doc) = searcher
                    .doc::<tantivy::schema::TantivyDocument>(a)
                    .map_err(|e| tracing::error!("Search Error: {e}"))
                else {
                    continue;
                };
                if let Some(doc) = SearchIndex::from_document(doc) {
                    if let SearchResult::Paragraph { fors, uri, .. } = doc {
                        for sym in fors {
                            if let Some((r, _, e)) = ret.iter_mut().find(|(_, k, _)| *k == sym) {
                                if score > *r {
                                    *e = uri.clone();
                                }
                            } else {
                                ret.push((score, sym, uri.clone()));
                            }
                        }
                    }
                }
            }
            ret.sort_by_key(|(s, _, _)| ordered_float::OrderedFloat(-*s));
            ret.truncate(num_results);
            Some(ret)
        })
    }

    #[cfg(feature = "vectorsearch")]
    #[allow(clippy::cast_possible_truncation)]
    pub fn query_symbols(
        &self,
        s: &str,
        num_results: usize,
    ) -> Option<Vec<(f32, SymbolUri, DocumentElementUri)>> {
        // SAFETY: invariant: input.len() == output.len()
        let query = unsafe { crate::Embedder::embed([s]).ok()?.pop().unwrap_unchecked() };
        let top_num = if num_results == 0 {
            usize::MAX / 2
        } else {
            num_results
        };
        let mut ret: Vec<(f32, SymbolUri, DocumentElementUri)> =
            Vec::with_capacity(if num_results == 0 { 1 } else { num_results + 1 });
        let searcher = self.index.read();
        for par in searcher.iter().filter(|e| {
            if let SearchIndex::Paragraph {
                definition_like: true,
                fors,
                ..
            } = e
                && !fors.is_empty()
            {
                true
            } else {
                false
            }
        }) {
            let SearchIndex::Paragraph {
                title,
                fors,
                body,
                uri: elem_uri,
                ..
            } = par
            else {
                // SAFETY: filter_map above
                unsafe {
                    use std::hint::unreachable_unchecked;
                    unreachable_unchecked()
                }
            };
            let title_score = title.as_ref().map(|t| (t % &query) as f32);
            let body_score = (body % &query) as f32;
            let neg_score = ordered_float::OrderedFloat(
                -title_score.map_or(body_score, |t| t.mul_add(2.0, body_score) / 3.0),
            );
            let index = ret
                .binary_search_by_key(&neg_score, |(e, _, _)| ordered_float::OrderedFloat(-*e))
                .unwrap_or_else(|i| i);

            // this could be optimized to iterate less
            for f in fors {
                if let Some((i, (_, _, _))) =
                    ret.iter().enumerate().find(|(_, (_, uri, _))| uri == f)
                {
                    if i >= index {
                        let (_, uri, _) = ret.remove(i);
                        ret.insert(index, (-neg_score.0, uri, elem_uri.clone()));
                    }
                } else {
                    ret.insert(index, (-neg_score.0, f.clone(), elem_uri.clone()));
                }
            }
            ret.truncate(top_num);
        }
        drop(searcher);
        Some(ret)
    }

    #[cfg(feature = "vectorsearch")]
    #[allow(clippy::cast_possible_truncation)]
    pub fn query(
        &self,
        s: &str,
        opts: FragmentQueryFilter,
        num_results: usize,
    ) -> Result<Vec<(f32, SearchResult)>, String> {
        // SAFETY: invariant: input.len() == output.len()
        let query = unsafe { crate::Embedder::embed([s])?.pop().unwrap_unchecked() };
        let top_num = if num_results == 0 {
            usize::MAX / 2
        } else {
            num_results
        };
        let mut ret: Vec<(f32, SearchResult)> =
            Vec::with_capacity(if num_results == 0 { 1 } else { num_results + 1 });
        let searcher = self.index.read();
        for e in searcher.iter().filter(|e| filter(&opts, e)) {
            match e {
                SearchIndex::Document { uri, title, body } => {
                    let title_score = title.as_ref().map(|t| (t % &query) as f32);
                    let body_score = (body % &query) as f32;
                    let neg_score = ordered_float::OrderedFloat(
                        -title_score.map_or(body_score, |t| t.mul_add(2.0, body_score) / 3.0),
                    );
                    let i = ret
                        .binary_search_by_key(&neg_score, |(e, _)| ordered_float::OrderedFloat(-*e))
                        .unwrap_or_else(|i| i);
                    ret.insert(i, (-neg_score.0, SearchResult::Document(uri.clone())));
                    if ret.len() > top_num {
                        let _ = ret.pop();
                    }
                }
                SearchIndex::Paragraph {
                    uri,
                    kind,
                    definition_like,
                    title,
                    fors,
                    body,
                } => {
                    let title_score = title.as_ref().map(|t| (t % &query) as f32);
                    let body_score = (body % &query) as f32;
                    let neg_score = ordered_float::OrderedFloat(
                        -title_score.map_or(body_score, |t| t.mul_add(2.0, body_score) / 3.0),
                    );
                    let i = ret
                        .binary_search_by_key(&neg_score, |(e, _)| ordered_float::OrderedFloat(-*e))
                        .unwrap_or_else(|i| i);
                    ret.insert(
                        i,
                        (
                            -neg_score.0,
                            SearchResult::Paragraph {
                                uri: uri.clone(),
                                fors: fors.clone(),
                                def_like: *definition_like,
                                kind: *kind,
                            },
                        ),
                    );
                    if ret.len() > top_num {
                        let _ = ret.pop();
                    }
                }
            }
        }
        drop(searcher);
        Ok(ret)
    }
}

pub fn index(backend: &AnyBackend, uri: &DocumentUri, rel_path: &UriPath, result: &ContentResult) {
    backend.with_buildable_archive(uri.archive_id(), |a| {
        if let Some(a) = a {
            let it = index::index_document(&result.document, &result.ftml);
            let _ = a.save(
                uri,
                Some(rel_path),
                FileOrString::Str(String::new().into_boxed_str()),
                #[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
                TANTIVY.id(),
                #[cfg(feature = "vectorsearch")]
                VECTORSEARCH.id(),
                Some(Box::new(IndexFile(it)) as _),
                GlobalBackend.triple_store(),
                false,
            );
        } else {
            tracing::error!("Archive not found! {}", uri.archive_id());
        }
    });
}

struct IndexFile(Vec<SearchIndex>);
impl Artifact for IndexFile {
    fn as_any(&self) -> &dyn std::any::Any {
        self as _
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self as _
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self as _
    }

    #[cfg(all(not(feature = "tantivy"), feature = "vectorsearch"))]
    fn kind(&self) -> &'static str {
        "vectorsearch"
    }

    #[cfg(feature = "tantivy")]
    fn kind(&self) -> &'static str {
        "tantivy"
    }

    fn write(&self, into: &std::path::Path) -> Result<(), ArtifactSaveError> {
        let file = std::fs::File::create(into)
            .map_err(|e| ArtifactSaveError::Fs(FileError::Creation(into.to_path_buf(), e)))?;
        #[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
        {
            bincode::encode_into_std_write(
                &self.0,
                &mut std::io::BufWriter::new(file),
                bincode::config::standard(),
            )?;
        }
        #[cfg(feature = "vectorsearch")]
        {
            bincode::encode_into_std_write(
                &self.0,
                &mut std::io::BufWriter::new(file),
                bincode::config::standard(),
            )?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_lines)]
fn initialize() {
    #[cfg(feature = "vectorsearch")]
    SPAN.in_scope(|| {
        use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

        let _ = std::thread::spawn(|| {
            std::sync::LazyLock::force(&MODEL);
        });
        let mut index = SEARCHER.index.write();
        let nidx = tracing::info_span!("Loading search indices").in_scope(move || {
            GlobalBackend
                .all_archives()
                .par_iter()
                .filter_map(|a| match a {
                    Archive::Local(a) => Some(a),
                    Archive::Ext(_, _) => None,
                })
                .flat_map(|a| {
                    let out = a.out_dir();
                    if out.exists() && out.is_dir() {
                        Some(
                            walkdir::WalkDir::new(out)
                                .into_iter()
                                .filter_map(Result::ok)
                                .filter(|entry| entry.file_name() == "vectorsearch")
                                .filter_map(|e| {
                                    let Ok(f) = std::fs::File::open(e.path()) else {
                                        tracing::error!(
                                            "error reading file {}",
                                            e.path().display()
                                        );
                                        return None;
                                    };
                                    let file = std::io::BufReader::new(f);

                                    let Ok(v): Result<Vec<SearchIndex>, _> =
                                        bincode::decode_from_reader(
                                            file,
                                            bincode::config::standard(),
                                        )
                                    else {
                                        tracing::error!(
                                            "error deserializing file {}",
                                            e.path().display()
                                        );
                                        return None;
                                    };
                                    Some(v)
                                })
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        None
                    }
                })
                .flatten()
                .flatten()
                .collect::<Vec<_>>()
        });
        *index = nidx;
    });

    #[cfg(all(feature = "tantivy", not(feature = "vectorsearch")))]
    SPAN.in_scope(|| {
        use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
        let index = tantivy::index::Index::create_in_ram(SearchSchema::get().schema.clone());
        let mut writer = index
            .writer(MEMORY_SIZE)
            .expect("Failed to instantiate search writer");
        let wr = &writer;
        tracing::info_span!("Loading search indices").in_scope(move || {
            GlobalBackend
                .all_archives()
                .par_iter()
                .filter_map(|a| match a {
                    Archive::Local(a) => Some(a),
                    Archive::Ext(_, _) => None,
                })
                .for_each(|a| {
                    let out = a.out_dir();
                    if out.exists() && out.is_dir() {
                        for e in walkdir::WalkDir::new(out)
                            .into_iter()
                            .filter_map(Result::ok)
                            .filter(|entry| entry.file_name() == "tantivy")
                        {
                            let Ok(f) = std::fs::File::open(e.path()) else {
                                tracing::error!("error reading file {}", e.path().display());
                                return;
                            };
                            let file = std::io::BufReader::new(f);

                            let Ok(v): Result<Vec<SearchIndex>, _> =
                                bincode::decode_from_reader(file, bincode::config::standard())
                            else {
                                tracing::error!("error deserializing file {}", e.path().display());
                                return;
                            };
                            for d in v {
                                use tantivy::schema::Value;

                                let d: tantivy::TantivyDocument = d.to_document();
                                if let Err(e) = wr.add_document(d) {
                                    tracing::error!("{e}");
                                }
                            }
                        }
                    }
                });
        });
        match writer.commit() {
            Ok(i) => tracing::info!("Loaded {i} entries"),
            Err(e) => tracing::error!("Error: {e}"),
        }
        let slf = Searcher::get();
        let writer = slf.writer.lock();
        let mut old_index = slf.index.write();
        let mut reader = slf.reader.write();
        let Ok(r) = index.reader() else {
            tracing::error!("Failed to instantiate search reader");
            return;
        };
        *reader = r;
        *old_index = index;
        drop(reader);
        drop(old_index);
        drop(writer);
    });
}

#[cfg(feature = "vectorsearch")]
fn filter(cplx: &FragmentQueryFilter, idx: &SearchIndex) -> bool {
    use flams_backend_types::search::SearchResultKind;
    use ftml_uris::IsNarrativeUri;

    match idx {
        SearchIndex::Document { uri, .. } => {
            cplx.flags.allow_documents()
                && (cplx.languages.is_empty() || cplx.languages.contains(&uri.language))
        }
        SearchIndex::Paragraph {
            definition_like: true,
            uri,
            ..
        } => {
            cplx.flags.allow_definitions()
                && (cplx.languages.is_empty() || cplx.languages.contains(&uri.language()))
        }
        SearchIndex::Paragraph {
            kind: SearchResultKind::Assertion,
            uri,
            ..
        } => {
            cplx.flags.allow_assertions()
                && (cplx.languages.is_empty() || cplx.languages.contains(&uri.language()))
        }
        SearchIndex::Paragraph {
            kind: SearchResultKind::Example,
            uri,
            ..
        } => {
            cplx.flags.allow_examples()
                && (cplx.languages.is_empty() || cplx.languages.contains(&uri.language()))
        }
        SearchIndex::Paragraph {
            kind: SearchResultKind::Paragraph,
            uri,
            ..
        } => {
            cplx.flags.allow_paragraphs()
                && (cplx.languages.is_empty() || cplx.languages.contains(&uri.language()))
        }
        SearchIndex::Paragraph {
            kind: SearchResultKind::Problem,
            uri,
            ..
        } => {
            cplx.flags.allow_problems()
                && (cplx.languages.is_empty() || cplx.languages.contains(&uri.language()))
        }
        SearchIndex::Paragraph { .. } => false,
    }
}
