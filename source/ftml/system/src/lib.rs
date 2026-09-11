//#![feature(string_from_utf8_lossy_owned)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//mod parser;

use flams_math_archives::{
    artifacts::{ContentResult, FileOrString, FtmlFile, FtmlString},
    backend::{AnyBackend, LocalBackend},
    build_target,
    formats::{BuildResult, BuildSpec},
    source_format, Archive, LocallyBuilt, MathArchive,
};
pub use ftml5ever::FtmlResult;
use ftml_ontology::{
    domain::{declarations::AnyDeclarationRef, HasDeclarations},
    utils::RefTree,
    Ftml,
};
use ftml_uris::{DocumentUri, UriWithArchive, UriWithPath};

source_format! { FTML {
    name:"ftml",
    description:"Flexiformal HTML",
    targets:&[FTML_IMPORT.id(),FTML_CONTENT.id()],
    file_extensions: &["html","html","xhtml"],
    dependencies: |_| Vec::new()
}}

build_target! { FTML_IMPORT {
    name:"import-ftml",
    description:"imports existent FTML",
    run: import
}}

build_target! { FTML_CONTENT {
    name:"ftml-content",
    description:"extracts content from FTML",
    run: extract
}}

#[allow(clippy::needless_pass_by_value)]
fn import(spec: BuildSpec) -> BuildResult {
    match spec.source {
        either::Either::Right(s) => BuildResult {
            log: FileOrString::Str("ok".to_string().into_boxed_str()),
            result: Ok(Some(
                Box::new(FtmlString(s.to_string().into_boxed_str())) as _
            )),
        },
        either::Either::Left(p) => BuildResult {
            log: FileOrString::Str("ok".to_string().into_boxed_str()),
            result: Ok(Some(Box::new(FtmlFile(p.to_path_buf())) as _)),
        },
    }
}

#[deprecated(note = "uses local archives only")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::needless_pass_by_value)]
fn extract(spec: BuildSpec) -> BuildResult {
    let html: Result<String, _> = spec.backend.with_archive(spec.uri.archive_id(), |a| {
        let Some(Archive::Local(a)) = a else {
            return Err(BuildResult::err());
        };
        let path = a
            .out_path_of(
                spec.uri.path(),
                &spec.uri.name,
                Some(spec.rel_path),
                spec.uri.language,
            )
            .join(FTML.name);
        std::fs::read_to_string(path).map_err(|e| {
            let mut err = BuildResult::err();
            err.log = FileOrString::Str(e.to_string().into_boxed_str());
            err
        })
    });
    let html = match html {
        Err(e) => return e,
        Ok(h) => h,
    };
    let uri = spec.uri.clone();
    let (lg, r) = flams_system::span_capture(|| build_ftml(spec.backend, &html, uri));
    match r {
        Err(e) => {
            let mut err = BuildResult::err();
            err.log = FileOrString::Str(format!("{lg}\n{e}").into_boxed_str());
            err
        }
        Ok(FtmlResult {
            ftml,
            css,
            errors,
            doc,
            body,
            inner_offset,
        }) => {
            let has_errored = !errors.is_empty();
            BuildResult {
                log: FileOrString::Str(format!("{lg}\n{errors:?}").into_boxed_str()),
                result: if has_errored {
                    Err(Vec::new())
                } else {
                    Ok(Some(Box::new(ContentResult {
                        document: doc.document,
                        modules: doc.modules,
                        data: doc.data,
                        body,
                        inner_offset,
                        css,
                        ftml,
                        triples: doc.triples,
                    })))
                },
            }
        }
    }

    /*match build_ftml(backend, &html.0, uri, task.rel_path()) {
        Err(e) => BuildResult {
            log: Either::Left(e),
            result: Err(Vec::new()),
        },
        Ok((r, s)) => BuildResult {
            log: Either::Left(s),
            result: Ok(BuildResultArtifact::Data(Box::new(r))),
        },
    }*/
}

/// # Errors
pub fn build_ftml(
    backend: &AnyBackend,
    html: &str,
    uri: DocumentUri,
) -> Result<FtmlResult, String> {
    static CSS_SUBSTS: [(&str, &str); 1] = [(
        "https://raw.githack.com/FlexiFormal/RusTeX/main/rustex/src/resources/rustex.css",
        "srv:/rustex.css",
    )];
    let path = uri.path().cloned();
    let archive = uri.archive_uri().clone();
    let mut r = ftml5ever::run(
        html,
        |src| {
            let path = std::path::Path::new(src);
            if let Some(s) =
                backend.archive_of(path, |a, rp| format!("srv:/img?a={}&rp={}", a.id(), rp))
            {
                return Some(s);
            }
            let kpsewhich = &*tex_engine::engine::filesystem::kpathsea::KPATHSEA;
            let last = src.rsplit_once('/').map_or(src, |(_, p)| p);
            kpsewhich.which(last).map_or_else(
                || Some(format!("srv:/img?file={src}")),
                |file| {
                    if file == path {
                        Some(format!("srv:/img?kpse={last}"))
                    } else {
                        None
                    }
                },
            )
        },
        |mut css| {
            CSS_SUBSTS
                .iter()
                .find_map(|(old, new)| {
                    if css == *old {
                        Some((*new).to_string().into_boxed_str())
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if css.contains("://") {
                        return None;
                    }
                    let mut path = path.as_ref()?.as_ref();
                    loop {
                        if let Some(s) = css.strip_prefix("./") {
                            css = s;
                        } else if let Some(s) = css.strip_prefix("../") {
                            if path.is_empty() {
                                return None;
                            }
                            css = s;
                            path = if let Some((p, _)) = path.rsplit_once('/') {
                                p
                            } else {
                                ""
                            };
                        } else {
                            break;
                        }
                    }
                    Some(
                        format!(
                            "srv:/aux?a={}&f={path}{}{css}",
                            archive.id,
                            if path.is_empty() { "" } else { "/" }
                        )
                        .into_boxed_str(),
                    )
                })
        },
        uri,
        true,
    );
    if let Ok(dr) = r.as_mut() {
        let content = &mut dr.doc;
        for m in &content.modules {
            let _ = m.initialize(&mut |m| backend.get_module(m).ok());
            for e in m.dfs() {
                if let AnyDeclarationRef::Morphism(m) = e {
                    content.triples.extend(m.triples());
                }
            }
        }
    }
    r
}
