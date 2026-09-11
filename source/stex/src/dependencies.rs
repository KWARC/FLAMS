use crate::PDFLATEX;
use crate::quickparse::stex::{DiagnosticLevel, rules};
use crate::{
    PDFLATEX_FIRST,
    quickparse::{
        latex::LaTeXParser,
        stex::structs::{ModuleReference, STeXParseState, STeXToken},
    },
};
use either::Either;
use flams_math_archives::MathArchive;
use flams_math_archives::backend::{AnyBackend, LocalBackend};
use flams_math_archives::formats::{BuildSpec, BuildTargetId, TaskDependency, TaskRef};
use flams_utils::sourcerefs::{NoPosition, StringRange};
use ftml_solver::CHECK;
use ftml_uris::{ArchiveId, DocumentUri, Language, UriPath, UriWithArchive};
use std::path::Path;

pub enum STeXDependency {
    ImportModule {
        archive: ArchiveId,
        module: std::sync::Arc<str>,
    },
    UseModule {
        archive: ArchiveId,
        module: std::sync::Arc<str>,
    },
    Inputref {
        archive: Option<ArchiveId>,
        filepath: std::sync::Arc<str>,
    },
    Module {
        //uri:ModuleUri,
        sig: Option<Language>,
        meta: Option<(ArchiveId, std::sync::Arc<str>)>,
    },
    Img {
        archive: Option<ArchiveId>,
        filepath: std::sync::Arc<str>,
    },
    SRef {
        filepath: std::sync::Arc<Path>,
        in_doc: Option<std::sync::Arc<Path>>,
    },
}

#[allow(clippy::type_complexity)]
pub struct DepParser<'a> {
    parser: LaTeXParser<'a, NoPosition, STeXToken<NoPosition>, STeXParseState<'a, NoPosition, ()>>,
    stack: Vec<std::vec::IntoIter<STeXToken<NoPosition>>>,
    curr: Option<std::vec::IntoIter<STeXToken<NoPosition>>>,
}

pub fn parse_deps<'a>(
    source: &'a str,
    path: &'a Path,
    doc: &'a DocumentUri,
    backend: &'a AnyBackend,
    err: &'a mut dyn FnMut(String, StringRange<NoPosition>, DiagnosticLevel),
) -> impl Iterator<Item = STeXDependency> + use<'a> {
    let archive = doc.archive_uri();
    let parser = LaTeXParser::with_rules(
        source,
        STeXParseState::<NoPosition, ()>::new(Some(archive), Some(path), doc, backend, ()),
        err,
        LaTeXParser::default_rules().into_iter().chain([
            ("importmodule", rules::importmodule_deps as _),
            ("requiremodule", rules::importmodule_deps as _),
            ("setmetatheory", rules::setmetatheory as _),
            ("usemodule", rules::usemodule_deps as _),
            ("inputref", rules::inputref as _),
            ("mhinput", rules::inputref as _),
            ("mhgraphics", rules::mhgraphics as _),
            ("cmhgraphics", rules::mhgraphics as _),
            ("stexstyleassertion", rules::stexstyleassertion as _),
            ("stexstyledefinition", rules::stexstyledefinition as _),
            ("stexstyleparagraph", rules::stexstyleparagraph as _),
            ("sref", rules::sref as _),
        ]),
        LaTeXParser::default_env_rules().into_iter().chain([(
            "smodule",
            (
                rules::smodule_deps_open as _,
                rules::smodule_deps_close as _,
            ),
        )]),
    );
    DepParser {
        parser,
        stack: Vec::new(),
        curr: None,
    }
}

impl DepParser<'_> {
    fn convert(&mut self, t: STeXToken<NoPosition>) -> Option<STeXDependency> {
        match t {
            STeXToken::ImportModule {
                module:
                    ModuleReference {
                        uri,
                        rel_path: Some(rel_path),
                        ..
                    },
                ..
            }
            | STeXToken::SetMetatheory {
                module:
                    ModuleReference {
                        uri,
                        rel_path: Some(rel_path),
                        ..
                    },
                ..
            } => Some(STeXDependency::ImportModule {
                archive: uri.archive_id().clone(),
                module: rel_path,
            }),
            STeXToken::UseModule {
                module:
                    ModuleReference {
                        uri,
                        rel_path: Some(rel_path),
                        ..
                    },
                ..
            } => Some(STeXDependency::UseModule {
                archive: uri.archive_id().clone(),
                module: rel_path,
            }),
            STeXToken::SRef {
                target_path,
                in_doc,
                ..
            } => Some(STeXDependency::SRef {
                filepath: target_path,
                in_doc: in_doc.map(|(_, p)| p),
            }),
            STeXToken::Module {
                /*uri,*/ sig,
                children,
                meta_theory,
                ..
            } => {
                let old = self.curr.replace(children.into_iter());
                if let Some(old) = old {
                    self.stack.push(old);
                }
                Some(STeXDependency::Module {
                    /*uri,*/ sig,
                    meta: meta_theory
                        .and_then(|m| m.rel_path.map(|p| (m.uri.archive_id().clone(), p))),
                })
            }
            STeXToken::Inputref {
                archive, filepath, ..
            } => Some(STeXDependency::Inputref {
                archive: archive.map(|(a, _)| a),
                filepath: filepath.0,
            }),
            STeXToken::Vec(v) => {
                let old = self.curr.replace(v.into_iter());
                if let Some(old) = old {
                    self.stack.push(old);
                }
                None
            }
            STeXToken::MHGraphics {
                filepath, archive, ..
            } => Some(STeXDependency::Img {
                archive: archive.map(|(a, _)| a),
                filepath: filepath.0,
            }),
            _ => None,
        }
    }
}

impl Iterator for DepParser<'_> {
    type Item = STeXDependency;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(curr) = &mut self.curr {
                if let Some(t) = curr.next() {
                    if let Some(t) = self.convert(t) {
                        return Some(t);
                    }
                } else {
                    self.curr = self.stack.pop();
                }
            } else if let Some(t) = self.parser.next() {
                if let Some(t) = self.convert(t) {
                    return Some(t);
                }
            } else {
                return None;
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
#[deprecated(note = "replace Arc<str>.parse by UriPath everywhere")]
#[allow(clippy::needless_pass_by_value)]
pub fn get_deps(task: BuildSpec) -> Vec<(BuildTargetId, TaskDependency)> {
    let mut deps = Vec::new();
    let Either::Left(path) = task.source else {
        return deps;
    };
    let Ok(source) = std::fs::read_to_string(path) else {
        return deps;
    };
    //let mut yields = Vec::new();
    for d in parse_deps(&source, path, task.uri, task.backend, &mut |_, _, _| {}) {
        match d {
            STeXDependency::ImportModule { archive, module }
            | STeXDependency::UseModule { archive, module }
                if !module.is_empty() =>
            {
                deps.push((
                    PDFLATEX_FIRST.id(),
                    TaskDependency::Physical {
                        strict: false,
                        task: TaskRef {
                            archive,
                            rel_path: module
                                .parse()
                                .unwrap_or_else(|e| panic!("this is a bug: {e}: {module}")),
                            target: PDFLATEX_FIRST.id(),
                        },
                    },
                ));
            }
            STeXDependency::Inputref { archive, filepath } if !filepath.is_empty() => {
                deps.push((
                    PDFLATEX_FIRST.id(),
                    TaskDependency::Physical {
                        strict: false,
                        task: TaskRef {
                            archive: archive.unwrap_or_else(|| task.uri.archive_id().clone()),
                            rel_path: filepath.parse().expect("this is a bug"),
                            target: PDFLATEX_FIRST.id(),
                        },
                    },
                ));
            }
            STeXDependency::Module {
                /*uri:_,*/ sig,
                meta,
            } => {
                //yields.push(uri);
                if let Some(lang) = sig {
                    let archive = task.uri.archive_id().clone();
                    let Some(rel_path) =
                        task.rel_path.as_ref().rsplit_once('.').and_then(|(a, _)| {
                            a.rsplit_once('.').map(|(a, _)| format!("{a}.{lang}.tex"))
                        })
                    else {
                        continue;
                    };
                    deps.push((
                        PDFLATEX_FIRST.id(),
                        TaskDependency::Physical {
                            strict: false,
                            task: TaskRef {
                                archive: archive.clone(),
                                rel_path: rel_path.clone().parse().expect("this is a bug"),
                                target: PDFLATEX_FIRST.id(),
                            },
                        },
                    ));
                    deps.push((
                        CHECK.id(),
                        TaskDependency::Physical {
                            strict: true,
                            task: TaskRef {
                                archive,
                                rel_path: rel_path.clone().parse().expect("this is a bug"),
                                target: CHECK.id(),
                            },
                        },
                    ));
                }
                if let Some((archive, module)) = meta {
                    deps.push((
                        PDFLATEX_FIRST.id(),
                        TaskDependency::Physical {
                            strict: false,
                            task: TaskRef {
                                archive: archive.clone(),
                                rel_path: module.parse().expect("this is a bug"),
                                target: PDFLATEX_FIRST.id(),
                            },
                        },
                    ));
                    deps.push((
                        CHECK.id(),
                        TaskDependency::Physical {
                            strict: true,
                            task: TaskRef {
                                archive,
                                rel_path: module.parse().expect("this is a bug"),
                                target: CHECK.id(),
                            },
                        },
                    ));
                }
            }
            STeXDependency::SRef { filepath, in_doc } => {
                if let Some((archive, rel_path)) = task
                    .backend
                    .archive_of(&filepath, |a, rp| {
                        let rp = rp.as_os_str().to_str()?;
                        rp.parse::<UriPath>().ok().map(|p| (a.id().clone(), p))
                    })
                    .flatten()
                {
                    deps.push((
                        PDFLATEX.id(),
                        TaskDependency::Physical {
                            strict: false,
                            task: TaskRef {
                                archive,
                                rel_path,
                                target: PDFLATEX_FIRST.id(),
                            },
                        },
                    ));
                }
                if let Some(id) = in_doc
                    && let Some((archive, rel_path)) = task
                        .backend
                        .archive_of(&id, |a, rp| {
                            let rp = rp.as_os_str().to_str()?;
                            rp.parse::<UriPath>().ok().map(|p| (a.id().clone(), p))
                        })
                        .flatten()
                {
                    deps.push((
                        PDFLATEX.id(),
                        TaskDependency::Physical {
                            strict: false,
                            task: TaskRef {
                                archive,
                                rel_path,
                                target: PDFLATEX_FIRST.id(),
                            },
                        },
                    ));
                }
            }
            STeXDependency::Img { .. }
            | STeXDependency::ImportModule { .. }
            | STeXDependency::UseModule { .. }
            | STeXDependency::Inputref { .. } => (),
        }
    }
    deps
}
