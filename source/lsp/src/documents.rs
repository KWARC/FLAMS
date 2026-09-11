use std::{path::Path, sync::atomic::AtomicBool};

use async_lsp::lsp_types::{Position, Range};
use flams_math_archives::{
    MathArchive,
    backend::{AnyBackend, GlobalBackend, LocalBackend},
    utils::path_ext::PathExt,
};
use flams_stex::quickparse::stex::{STeXParseData, STeXParseDataI};
use flams_utils::sourcerefs::{
    ByteOffset, LSPLineCol, PositionConverter, StringPosition, StringRange,
};
use ftml_uris::{ArchiveUri, DocumentUri};

use crate::{
    LSPStore,
    state::{LSPState, UrlOrFile},
};

#[derive(Debug, PartialEq, Eq)]
struct DocumentData {
    path: Option<std::sync::Arc<Path>>,
    archive: Option<ArchiveUri>,
    rel_path: Option<Box<str>>,
    doc_uri: Option<DocumentUri>,
}

#[derive(Clone, Debug)]
pub struct LSPDocument {
    pub(crate) up_to_date: triomphe::Arc<AtomicBool>,
    text: triomphe::Arc<parking_lot::Mutex<LSPText>>,
    //pub(crate) ignore_verbalizations:triomphe::Arc<parking_lot::Mutex<Vec<(String, Option<StringRange<LSPLineCol>>)>>>,
    pub annotations: STeXParseData,
    data: triomphe::Arc<DocumentData>,
    pub(crate) force_snify: triomphe::Arc<AtomicBool>,
}
impl PartialEq for LSPDocument {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl LSPDocument {
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn new(text: String, lsp_uri: UrlOrFile) -> Self {
        let path = if let UrlOrFile::File(p) = lsp_uri {
            Some(p)
        } else {
            None
        }; //lsp_uri.to_file_path().ok().map(Into::into);
        let default = || {
            let path = path.as_ref()?.as_slash_str().into_owned();
            Some((
                ArchiveUri::no_archive().clone(),
                Some(path.into_boxed_str()),
            ))
        };
        let ap = path
            .as_ref()
            .and_then(|path| {
                GlobalBackend.archive_of_source(path, |a, rp| {
                    let uri = a.uri().clone();
                    (uri, Some(rp.to_string().into_boxed_str()))
                })
            })
            .or_else(default);
        let (archive, rel_path) = ap.map_or((None, None), |(a, p)| (Some(a), p));
        let r = LSPText {
            text,
            html_up_to_date: false,
        };
        let doc_uri = archive.as_ref().and_then(|a| {
            rel_path.as_deref().and_then(|rp: &str| {
                match DocumentUri::from_archive_relpath(a.clone(), rp) {
                    Ok(u) => Some(u),
                    Err(e) => {
                        tracing::error!("Error in URI {rp} in {a}: {e} ({path:?})");
                        None
                    }
                }
            })
        });
        //tracing::info!("Document: {lsp_uri}\n - {doc_uri:?}\n - [{archive:?}]{{{rel_path:?}}}");
        let data = DocumentData {
            path,
            archive,
            rel_path,
            doc_uri,
        };
        Self {
            up_to_date: triomphe::Arc::new(AtomicBool::new(false)),
            text: triomphe::Arc::new(parking_lot::Mutex::new(r)),
            data: triomphe::Arc::new(data),
            //ignore_verbalizations: triomphe::Arc::new(parking_lot::Mutex::new(Vec::new())),
            annotations: STeXParseData::default(),
            force_snify: triomphe::Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline]
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.data.path.as_deref()
    }

    #[inline]
    #[must_use]
    pub fn archive(&self) -> Option<&ArchiveUri> {
        self.data.archive.as_ref()
    }

    #[inline]
    #[must_use]
    pub fn relative_path(&self) -> Option<&str> {
        self.data.rel_path.as_deref()
    }

    #[inline]
    #[must_use]
    pub fn document_uri(&self) -> Option<&DocumentUri> {
        self.data.doc_uri.as_ref()
    }

    #[inline]
    pub fn set_text(&self, s: String) -> bool {
        let mut txt = self.text.lock();
        if txt.text == s {
            return false;
        }
        txt.text = s;
        self.up_to_date
            .store(false, std::sync::atomic::Ordering::SeqCst);
        true
    }

    #[inline]
    pub fn with_text<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        f(&self.text.lock().text)
    }

    #[inline]
    pub fn html_up_to_date(&self) -> bool {
        self.text.lock().html_up_to_date
    }

    pub fn set_html_up_to_date(&self) {
        self.text.lock().html_up_to_date = true
    }

    #[inline]
    pub fn delta(&self, text: String, range: Option<Range>) {
        self.up_to_date
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.text.lock().delta(text, range);
    }

    /*#[inline]#[must_use]
    pub fn get_range(&self, range: Range) -> (usize, usize) {
        self.text.lock().get_range(range)
    }*/
    #[inline]
    #[must_use]
    pub fn get_position(&self, pos: Position) -> usize {
        self.text.lock().get_position(pos)
    }

    #[inline]
    #[must_use]
    pub fn has_annots(&self) -> bool {
        self.data.doc_uri.is_some() && self.data.path.is_some()
    }

    #[allow(clippy::significant_drop_tightening)]
    pub(crate) fn load_annotations_and<R>(
        &self,
        state: LSPState,
        snify: bool,
        f: impl FnOnce(&STeXParseDataI) -> R,
    ) -> Option<R> {
        let text_lock = self.text.lock();
        let uri = self.data.doc_uri.as_ref()?;
        let path = self.data.path.as_ref()?;

        let mut docs = state.documents.write();
        let mut vlock = state.verbalizations.lock();
        //let ignores = self.ignore_verbalizations.lock();
        let ignores = if snify {
            let mut ret = Vec::new();
            for line in text_lock.text.lines().rev() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('%') {
                    break;
                }
                let Some(line) = trimmed.strip_prefix("% srskip ") else {
                    continue;
                };
                for v in line.split(',') {
                    let trimmed = v.trim();
                    if let Some(name) = trimmed.strip_prefix("l:") {
                        ret.push(name);
                    }
                }
            }
            ret
        } else {
            Vec::new()
        };
        let mut store = LSPStore::<true>::new(&mut docs, Some(&mut vlock), &ignores, snify);
        let data = flams_stex::quickparse::stex::quickparse(
            uri,
            &text_lock.text,
            path,
            &AnyBackend::Global,
            &mut store,
        );
        data.replace(&self.annotations);
        self.up_to_date
            .store(true, std::sync::atomic::Ordering::SeqCst);
        drop(store);
        drop(vlock);
        drop(docs);
        //tracing::info!("quickparse took {t}");
        drop(text_lock);
        /*let path = path.clone();
        let _ = tokio::task::spawn_blocking(move || {
          state.relint_dependents(path);
        });*/
        let lock = self.annotations.lock();
        Some(f(&lock))
    }

    pub fn is_up_to_date(&self) -> bool {
        self.up_to_date.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[inline]
    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub async fn with_annots<R: Send + 'static>(
        self,
        state: LSPState,
        snify: bool,
        f: impl FnOnce(&STeXParseDataI) -> R + Send + 'static,
    ) -> Option<R> {
        if !self.has_annots() {
            return None;
        }
        if self.is_up_to_date() {
            let lock = self.annotations.lock();
            if lock.is_empty() {
                return None;
            }
            return Some(f(&lock));
        }
        let forced = self
            .force_snify
            .swap(false, std::sync::atomic::Ordering::AcqRel);
        let snify = snify || forced;
        match tokio::task::spawn_blocking(move || self.load_annotations_and(state, snify, f)).await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Error computing annots: {}", e);
                None
            }
        }
    }

    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub async fn with_annots_block<R: Send + 'static>(
        self,
        state: LSPState,
        snify: bool,
        f: impl FnOnce(&STeXParseDataI) -> R + Send + 'static,
    ) -> Option<R> {
        if !self.has_annots() {
            return None;
        }
        if self.is_up_to_date() {
            if self.annotations.lock().is_empty() {
                return None;
            }
            let annot = self.annotations.clone();
            return match tokio::task::spawn_blocking(move || f(&annot.lock())).await {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::error!("Error computing annots: {}", e);
                    None
                }
            };
        }
        match tokio::task::spawn_blocking(move || self.load_annotations_and(state, snify, f)).await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Error computing annots: {}", e);
                None
            }
        }
    }

    #[inline]
    pub fn compute_annots(&self, state: LSPState, snify: bool) {
        self.load_annotations_and(state, snify, |_| ());
    }
}

#[derive(Debug)]
struct LSPText {
    text: String,
    html_up_to_date: bool,
}

impl LSPText {
    fn get_position(
        &self,
        Position {
            mut line,
            character,
        }: Position,
    ) -> usize {
        let mut rest = self.text.as_str();
        let mut off = 0;
        while line > 0 {
            if let Some(i) = rest.find(['\n', '\r']) {
                off += i + 1;
                if rest.as_bytes()[i] == b'\r' && rest.as_bytes().get(i + 1) == Some(&b'\n') {
                    off += 1;
                    rest = &rest[i + 2..];
                } else {
                    rest = &rest[i + 1..];
                }
                line -= 1;
            } else {
                off = self.text.len();
                rest = "";
                break;
            }
        }
        let next = rest
            .chars()
            .take(character as usize)
            .map(char::len_utf8)
            .sum::<usize>();
        off += next;
        off
    }

    fn get_range(&self, range: Range) -> (usize, usize) {
        let Range { start, end } = range;
        let off = PositionConverter::<LSPLineCol, ByteOffset>::new(self.text.as_str()).next_range(
            StringRange {
                start: LSPLineCol::new(start.line, start.character),
                end: LSPLineCol::new(end.line, end.character),
            },
        );
        (off.start.0, off.end.0)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delta(&mut self, text: String, range: Option<Range>) {
        let Some(range) = range else {
            self.text = text;
            return;
        };
        let (start, end) = self.get_range(range);
        self.text.replace_range(start..end, &text);
        self.html_up_to_date = false;
    }
}
