#![allow(clippy::too_many_lines)]

use crate::capabilities::STeXSemanticTokens;
use crate::{
    IsLSPRange, ProgressCallbackClient,
    state::{DocData, LSPState, UrlOrFile},
};
use async_lsp::lsp_types as lsp;
use flams_math_archives::LocalArchive;
use flams_math_archives::backend::{GlobalBackend, LocalBackend};
use flams_stex::quickparse::stex::rules::{IncludeProblemArg, SRefOptsA, SRefOptsB};
use flams_stex::quickparse::{
    latex::ParsedKeyValue,
    stex::{
        AnnotIter, DiagnosticLevel, STeXAnnot, STeXDiagnostic, STeXParseDataI,
        rules::{
            MathStructureArg, NotationArg, ParagraphArg, ProblemArg, SModuleArg, SymdeclArg,
            SymdefArg, TextSymdeclArg, VardefArg,
        },
        structs::{
            InlineMorphAssKind, InlineMorphAssign, ModuleOrStruct, MorphismKind, SymbolReference,
        },
    },
};
use flams_utils::{
    prelude::TreeChildIter,
    sourcerefs::{LSPLineCol, StringRange},
};
use ftml_ontology::narrative::elements::paragraphs::ParagraphKind;
use ftml_uris::{
    ArchiveId, ArchiveUri, IsDomainUri, IsNarrativeUri, ModuleUri, SymbolUri, Uri, UriWithArchive,
    UriWithPath,
};
use futures::FutureExt;
use smallvec::SmallVec;

trait AnnotExt: Sized {
    fn as_symbol(&self) -> Option<(lsp::DocumentSymbol, &[Self])>;
    fn links(&self, top_archive: Option<&ArchiveUri>, f: impl FnMut(lsp::DocumentLink));
    fn goto_definition(
        &self,
        in_doc: &UrlOrFile,
        pos: LSPLineCol,
    ) -> Option<lsp::GotoDefinitionResponse>;
    fn semantic_tokens(&self, cont: &mut impl FnMut(StringRange<LSPLineCol>, u32));
    fn hover(&self, top_archive: Option<&ArchiveUri>, pos: LSPLineCol) -> Option<lsp::Hover>;
    fn inlay_hint(&self) -> Option<lsp::InlayHint>;
    fn code_action(&self, pos: LSPLineCol, url: &lsp::Url) -> lsp::CodeActionResponse;
}

pub(crate) fn uri_from_archive_relpath(id: &ArchiveId, relpath: &str) -> Option<lsp::Url> {
    let path = GlobalBackend.with_local_archive(id, |a| a.map(LocalArchive::source_dir))?;
    let path = relpath.split('/').fold(path, |p, s| p.join(s));
    lsp::Url::from_file_path(path).ok()
}

#[allow(deprecated)]
impl AnnotExt for STeXAnnot {
    fn as_symbol(&self) -> Option<(lsp::DocumentSymbol, &[Self])> {
        match self {
            Self::Module {
                uri,
                full_range,
                name_range,
                children,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: uri.to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: name_range.into_range(),
                    children: None,
                },
                children,
            )),
            Self::MathStructure {
                uri,
                name_range,
                full_range,
                children,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: uri.uri.to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::STRUCT,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: name_range.into_range(),
                    children: None,
                },
                children,
            )),
            Self::ConservativeExt {
                uri,
                full_range,
                extstructure_range,
                children,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: format!("{}_EXT", uri.uri),
                    detail: None,
                    kind: lsp::SymbolKind::STRUCT,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: extstructure_range.into_range(),
                    children: None,
                },
                children,
            )),
            Self::MorphismEnv {
                full_range,
                env_range,
                uri,
                children,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: uri.to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::INTERFACE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: env_range.into_range(),
                    children: None,
                },
                children,
            )),
            Self::InlineMorphism {
                full_range,
                uri,
                token_range,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: uri.to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::INTERFACE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: token_range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::Paragraph {
                kind,
                full_range,
                name_range,
                children,
                ..
            }
            | Self::InlineParagraph {
                kind,
                full_range,
                token_range: name_range,
                children,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: kind.to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::PACKAGE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: name_range.into_range(),
                    children: None,
                },
                children,
            )),
            Self::Problem {
                full_range,
                name_range,
                children,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: "problem".to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::PACKAGE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: name_range.into_range(),
                    children: None,
                },
                children,
            )),
            Self::Symdecl {
                uri,
                main_name_range,
                full_range,
                ..
            }
            | Self::TextSymdecl {
                uri,
                main_name_range,
                full_range,
                ..
            }
            | Self::Symdef {
                uri,
                main_name_range,
                full_range,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: uri.uri.to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::OBJECT,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: main_name_range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::Vardef {
                name,
                main_name_range,
                full_range,
                ..
            }
            | Self::Varseq {
                name,
                main_name_range,
                full_range,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: name.to_string(),
                    detail: None,
                    kind: lsp::SymbolKind::VARIABLE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: main_name_range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::ImportModule {
                module, full_range, ..
            } => Some((
                lsp::DocumentSymbol {
                    name: format!("import@{}", module.uri),
                    detail: Some(module.uri.to_string()),
                    kind: lsp::SymbolKind::PACKAGE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: full_range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::UseModule {
                module, full_range, ..
            } => Some((
                lsp::DocumentSymbol {
                    name: format!("usemodule@{}", module.uri),
                    detail: Some(module.uri.to_string()),
                    kind: lsp::SymbolKind::PACKAGE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: full_range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::UseStructure {
                structure,
                full_range,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: format!("usestructure@{}", structure.uri),
                    detail: None,
                    kind: lsp::SymbolKind::PACKAGE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: full_range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::SetMetatheory {
                module, full_range, ..
            } => Some((
                lsp::DocumentSymbol {
                    name: format!("metatheory@{}", module.uri),
                    detail: Some(module.uri.to_string()),
                    kind: lsp::SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range: full_range.into_range(),
                    selection_range: full_range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::Inputref {
                archive,
                filepath,
                full_range: range,
                ..
            }
            | Self::IncludeProblem {
                archive,
                filepath,
                full_range: range,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: archive.as_ref().map_or_else(
                        || format!("inputref@{}", filepath.0),
                        |(a, _)| format!("inputref@[{a}]{}", filepath.0),
                    ),
                    detail: None,
                    kind: lsp::SymbolKind::PACKAGE,
                    tags: None,
                    deprecated: None,
                    range: range.into_range(),
                    selection_range: range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::MHInput {
                archive,
                filepath,
                full_range: range,
                ..
            } => Some((
                lsp::DocumentSymbol {
                    name: archive.as_ref().map_or_else(
                        || format!("mhinput@{}", filepath.0),
                        |(a, _)| format!("mhinput@[{a}]{}", filepath.0),
                    ),
                    detail: None,
                    kind: lsp::SymbolKind::PACKAGE,
                    tags: None,
                    deprecated: None,
                    range: range.into_range(),
                    selection_range: range.into_range(),
                    children: None,
                },
                &[],
            )),
            Self::MHGraphics { .. }
            | Self::SemanticMacro { .. }
            | Self::VariableMacro { .. }
            | Self::SymName { .. }
            | Self::Symref { .. }
            | Self::Notation { .. }
            | Self::Svar { .. }
            | Self::Symuse { .. }
            | Self::Definiens { .. }
            | Self::Defnotation { .. }
            | Self::RenameDecl { .. }
            | Self::Precondition { .. }
            | Self::Objective { .. }
            | Self::Assign { .. }
            | Self::SRef { .. }
            | Self::SnifySuggestion { .. } => None,
        }
    }

    fn links(&self, top_archive: Option<&ArchiveUri>, mut cont: impl FnMut(lsp::DocumentLink)) {
        match self {
            Self::IncludeProblem {
                archive, filepath, ..
            } => {
                let Some(a) = archive
                    .as_ref()
                    .map_or_else(|| top_archive.map(ArchiveUri::archive_id), |(a, _)| Some(a))
                else {
                    return;
                };
                let Some(uri) = uri_from_archive_relpath(a, &filepath.0) else {
                    return;
                };
                cont(lsp::DocumentLink {
                    range: filepath.1.into_range(),
                    target: Some(uri),
                    tooltip: None,
                    data: None,
                });
            }
            Self::SRef {
                label_range,
                opt_args,
                in_opt_args,
                target_path,
                in_doc,
                ..
            } => {
                if let Ok(url) = lsp::Url::from_file_path(target_path) {
                    for o in opt_args {
                        if let SRefOptsA::File(f) = o {
                            cont(lsp::DocumentLink {
                                range: f.val_range.into_range(),
                                target: Some(url.clone()),
                                tooltip: None,
                                data: None,
                            });
                            break;
                        }
                    }
                    cont(lsp::DocumentLink {
                        range: label_range.into_range(),
                        target: Some(url),
                        tooltip: None,
                        data: None,
                    });
                }
                if let Some((_, id)) = in_doc
                    && let Ok(url) = lsp::Url::from_file_path(id)
                    && let Some(rng) = in_opt_args.iter().find_map(|a| {
                        if let SRefOptsB::File(f) = a {
                            Some(f.val_range)
                        } else {
                            None
                        }
                    })
                {
                    cont(lsp::DocumentLink {
                        range: rng.into_range(),
                        target: Some(url),
                        tooltip: None,
                        data: None,
                    });
                }
            }
            Self::MHGraphics {
                archive, filepath, ..
            } => {
                let Some(a) = archive.as_ref().map_or_else(
                    || top_archive.map(UriWithArchive::archive_id),
                    |(a, _)| Some(a),
                ) else {
                    return;
                };
                let Some(uri) = uri_from_archive_relpath(a, &filepath.0) else {
                    return;
                };
                cont(lsp::DocumentLink {
                    range: filepath.1.into_range(),
                    target: Some(uri),
                    tooltip: None,
                    data: None,
                });
            }
            Self::Inputref {
                archive,
                token_range,
                filepath,
                full_range: range,
                ..
            }
            | Self::MHInput {
                archive,
                token_range,
                filepath,
                full_range: range,
                ..
            } => {
                let Some(a) = archive.as_ref().map_or_else(
                    || top_archive.map(UriWithArchive::archive_id),
                    |(a, _)| Some(a),
                ) else {
                    return;
                };
                let Some(uri) = uri_from_archive_relpath(a, &filepath.0) else {
                    return;
                };
                let mut range = *range;
                range.start = token_range.end;
                cont(lsp::DocumentLink {
                    range: range.into_range(),
                    target: Some(uri),
                    tooltip: None,
                    data: None,
                });
            }
            Self::ImportModule { .. }
            | Self::UseModule { .. }
            | Self::SemanticMacro { .. }
            | Self::VariableMacro { .. }
            | Self::SetMetatheory { .. }
            | Self::Module { .. }
            | Self::MathStructure { .. }
            | Self::Symdecl { .. }
            | Self::TextSymdecl { .. }
            | Self::SymName { .. }
            | Self::Symref { .. }
            | Self::Notation { .. }
            | Self::Symdef { .. }
            | Self::Vardef { .. }
            | Self::Paragraph { .. }
            | Self::Symuse { .. }
            | Self::Svar { .. }
            | Self::Varseq { .. }
            | Self::Definiens { .. }
            | Self::Defnotation { .. }
            | Self::ConservativeExt { .. }
            | Self::UseStructure { .. }
            | Self::InlineParagraph { .. }
            | Self::Problem { .. }
            | Self::MorphismEnv { .. }
            | Self::RenameDecl { .. }
            | Self::Precondition { .. }
            | Self::Objective { .. }
            | Self::Assign { .. }
            | Self::InlineMorphism { .. }
            | Self::SnifySuggestion { .. } => (),
        }
    }

    fn semantic_tokens(&self, cont: &mut impl FnMut(StringRange<LSPLineCol>, u32)) {
        match self {
            Self::SRef {
                token_range,
                opt_args,
                in_opt_args,
                ..
            } => {
                use SRefOptsA as A;
                use SRefOptsB as B;
                cont(*token_range, STeXSemanticTokens::REF_MACRO);
                for o in opt_args {
                    match o {
                        A::Archive(a) | A::File(a) => {
                            cont(a.key_range, STeXSemanticTokens::KEYWORD);
                        }
                        A::Fallback(a) => {
                            cont(a.key_range, STeXSemanticTokens::KEYWORD);
                            for e in &a.val {
                                e.semantic_tokens(cont);
                            }
                        }
                        A::Pre(a) | A::Post(a) => {
                            cont(a.key_range, STeXSemanticTokens::KEYWORD);
                        }
                    }
                }
                for o in in_opt_args {
                    match o {
                        B::Archive(a) | B::File(a) => {
                            cont(a.key_range, STeXSemanticTokens::KEYWORD);
                        }
                        B::Title(a) => {
                            cont(a.key_range, STeXSemanticTokens::KEYWORD);
                            for e in &a.val {
                                e.semantic_tokens(cont);
                            }
                        }
                    }
                }
            }
            Self::Module {
                name_range,
                full_range,
                smodule_range,
                opts,
                children,
                ..
            } => {
                cont(*smodule_range, STeXSemanticTokens::DECLARATION);
                for o in opts {
                    match o {
                        SModuleArg::Title(v) => {
                            cont(v.key_range, STeXSemanticTokens::KEYWORD);
                            for e in &v.val {
                                e.semantic_tokens(cont);
                            }
                        }
                        SModuleArg::Sig(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::KEYWORD);
                        }
                        //SModuleArg::Creators(ParsedKeyValue{key_range,..}) |
                        //SModuleArg::Contributors(ParsedKeyValue{key_range,..}) |
                        SModuleArg::Meta(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                        }
                    }
                }
                cont(*name_range, STeXSemanticTokens::NAME);
                for c in children {
                    c.semantic_tokens(cont);
                }
                let mut end_range = *full_range;
                end_range.end.col -= 1;
                end_range.start.line = end_range.end.line;
                #[allow(clippy::cast_possible_truncation)]
                {
                    end_range.start.col = end_range.end.col - const { "smodule".len() as u32 };
                }
                cont(end_range, STeXSemanticTokens::DECLARATION);
            }
            Self::MathStructure {
                extends,
                name_range,
                opts,
                full_range,
                children,
                mathstructure_range,
                ..
            } => {
                cont(*mathstructure_range, STeXSemanticTokens::DECLARATION);
                cont(*name_range, STeXSemanticTokens::NAME);
                for o in opts {
                    match o {
                        MathStructureArg::This(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for e in val {
                                e.semantic_tokens(cont);
                            }
                        }
                        MathStructureArg::Name(range, _) => {
                            cont(*range, STeXSemanticTokens::NAME);
                        }
                    }
                }
                for (_, r) in extends {
                    cont(*r, STeXSemanticTokens::SYMBOL);
                }
                for c in children {
                    c.semantic_tokens(cont);
                }
                let mut end_range = *full_range;
                end_range.end.col -= 1;
                end_range.start.line = end_range.end.line;
                let s = if extends.is_empty() {
                    "mathstructure"
                } else {
                    "extstructure"
                };
                end_range.start.col = end_range.end.col - s.len() as u32;
                cont(end_range, STeXSemanticTokens::DECLARATION);
            }
            Self::ConservativeExt {
                ext_range,
                full_range,
                extstructure_range,
                children,
                ..
            } => {
                cont(*extstructure_range, STeXSemanticTokens::DECLARATION);
                cont(*ext_range, STeXSemanticTokens::SYMBOL);
                for c in children {
                    c.semantic_tokens(cont);
                }
                let mut end_range = *full_range;
                end_range.end.col -= 1;
                end_range.start.line = end_range.end.line;
                end_range.start.col = end_range.end.col - "extstructure*".len() as u32;
                cont(end_range, STeXSemanticTokens::DECLARATION);
            }
            Self::MorphismEnv {
                env_range,
                star,
                kind,
                name_range,
                domain_range,
                full_range,
                domain,
                children,
                ..
            } => {
                cont(*env_range, STeXSemanticTokens::DECLARATION);
                cont(*name_range, STeXSemanticTokens::NAME);
                match domain {
                    ModuleOrStruct::Struct(_) => cont(*domain_range, STeXSemanticTokens::SYMBOL),
                    _ => (),
                }
                for c in children {
                    c.semantic_tokens(cont);
                }

                let mut end_range = *full_range;
                end_range.end.col -= 1;
                end_range.start.line = end_range.end.line;
                let len = match kind {
                    MorphismKind::CopyModule => "copymodule".len(),
                    MorphismKind::InterpretModule => "interpretmodule".len(),
                };
                let len = if *star { len + 1 } else { len };
                end_range.start.col = end_range.end.col - len as u32;
                cont(end_range, STeXSemanticTokens::DECLARATION);
            }
            Self::InlineMorphism {
                token_range,
                name_range,
                domain_range,
                domain,
                assignments,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION);
                cont(*name_range, STeXSemanticTokens::NAME);
                match domain {
                    ModuleOrStruct::Struct(_) => cont(*domain_range, STeXSemanticTokens::SYMBOL),
                    _ => (),
                }
                for InlineMorphAssign {
                    first,
                    second,
                    symbol_range,
                    ..
                } in assignments
                {
                    cont(*symbol_range, STeXSemanticTokens::SYMBOL);
                    if let Some((e, knd)) = first {
                        let end = LSPLineCol::new(e.line, e.col + 1);
                        let range = StringRange { start: *e, end };
                        cont(range, STeXSemanticTokens::KEYWORD);
                        match knd {
                            InlineMorphAssKind::Df(v) => {
                                for c in v {
                                    c.semantic_tokens(cont);
                                }
                            }
                            InlineMorphAssKind::Rename(mn, _, n) => {
                                if let Some((_, mn)) = mn {
                                    cont(*mn, STeXSemanticTokens::NAME);
                                }
                                cont(*n, STeXSemanticTokens::NAME);
                            }
                        }
                    }
                    if let Some((e, knd)) = second {
                        let end = LSPLineCol::new(e.line, e.col + 1);
                        let range = StringRange { start: *e, end };
                        cont(range, STeXSemanticTokens::KEYWORD);
                        match knd {
                            InlineMorphAssKind::Df(v) => {
                                for c in v {
                                    c.semantic_tokens(cont);
                                }
                            }
                            InlineMorphAssKind::Rename(mn, _, n) => {
                                if let Some((_, mn)) = mn {
                                    cont(*mn, STeXSemanticTokens::NAME);
                                }
                                cont(*n, STeXSemanticTokens::NAME);
                            }
                        }
                    }
                }
            }
            Self::UseModule { token_range, .. } => cont(*token_range, STeXSemanticTokens::LOCAL),
            Self::UseStructure {
                token_range,
                structure_range,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::LOCAL);
                cont(*structure_range, STeXSemanticTokens::SYMBOL);
            }
            Self::IncludeProblem {
                token_range, args, ..
            } => {
                cont(*token_range, STeXSemanticTokens::REF_MACRO);
                for a in args {
                    let r = match a {
                        IncludeProblemArg::Min(m) => m.key_range,
                        IncludeProblemArg::Pts(m) => m.key_range,
                        IncludeProblemArg::Archive(m) => m.key_range,
                    };
                    cont(r, STeXSemanticTokens::KEYWORD);
                }
            }
            Self::Inputref {
                token_range: range, ..
            }
            | Self::MHInput {
                token_range: range, ..
            }
            | Self::MHGraphics {
                token_range: range, ..
            }
            | Self::Defnotation { full_range: range } => {
                cont(*range, STeXSemanticTokens::REF_MACRO)
            }
            Self::SetMetatheory { token_range, .. } | Self::ImportModule { token_range, .. } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION)
            }
            Self::SemanticMacro { token_range, .. } => {
                cont(*token_range, STeXSemanticTokens::SYMBOL)
            }
            Self::VariableMacro { token_range, .. } => {
                cont(*token_range, STeXSemanticTokens::VARIABLE)
            }
            Self::Problem {
                full_range,
                name_range,
                parsed_args,
                children,
                ..
            } => {
                cont(*name_range, STeXSemanticTokens::REF_MACRO);
                for e in parsed_args {
                    match e {
                        /*ProblemArg::Name(ParsedKeyValue{key_range,val_range,..}) => {
                            cont(*key_range,STeXSemanticTokens::KEYWORD);
                            cont(*val_range,STeXSemanticTokens::NAME);
                        }*/
                        ProblemArg::Autogradable(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::KEYWORD);
                        }
                        ProblemArg::Style(ParsedKeyValue { key_range, .. })
                        | ProblemArg::Pts(ParsedKeyValue { key_range, .. })
                        | ProblemArg::Min(ParsedKeyValue { key_range, .. })
                        | ProblemArg::Id(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                        ProblemArg::Title(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                    }
                }
                for c in children {
                    c.semantic_tokens(cont);
                }
                let mut end_range = *full_range;
                end_range.end.col -= 1;
                end_range.start.line = end_range.end.line;
                cont(end_range, STeXSemanticTokens::REF_MACRO);
            }
            Self::Paragraph {
                kind,
                symbol,
                name_range,
                parsed_args,
                children,
                full_range,
                ..
            } => {
                if symbol.is_some() {
                    cont(*name_range, STeXSemanticTokens::DECLARATION);
                } else {
                    cont(*name_range, STeXSemanticTokens::REF_MACRO);
                }
                for e in parsed_args {
                    match e {
                        ParagraphArg::Tp(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Df(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Return(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Argtypes(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Title(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                        ParagraphArg::Args(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::KEYWORD);
                        }
                        ParagraphArg::Name(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        })
                        | ParagraphArg::MacroName(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::NAME);
                        }
                        ParagraphArg::Style(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Assoc(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Role(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Reorder(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::From(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::To(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Judgment(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Id(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                        ParagraphArg::Fors(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for (_, f) in val {
                                cont(*f, STeXSemanticTokens::SYMBOL);
                            }
                        }
                    }
                }
                for c in children {
                    c.semantic_tokens(cont);
                }
                let mut end_range = *full_range;
                end_range.end.col -= 1;
                end_range.start.line = end_range.end.line;
                let parname = match kind {
                    ParagraphKind::Definition => "sdefinition".len(),
                    ParagraphKind::Paragraph => "sparagraph".len(),
                    ParagraphKind::Proof => "sproof".len(),
                    ParagraphKind::Example => "sexample".len(),
                    ParagraphKind::Assertion => "sassertion".len(),
                    _ => return,
                };
                end_range.start.col = end_range.end.col - parname as u32;
                if symbol.is_some() {
                    cont(end_range, STeXSemanticTokens::DECLARATION);
                } else {
                    cont(end_range, STeXSemanticTokens::REF_MACRO);
                }
            }
            Self::InlineParagraph {
                symbol,
                token_range,
                parsed_args,
                children,
                ..
            } => {
                if symbol.is_some() {
                    cont(*token_range, STeXSemanticTokens::DECLARATION);
                } else {
                    cont(*token_range, STeXSemanticTokens::REF_MACRO);
                }
                for e in parsed_args {
                    match e {
                        ParagraphArg::Tp(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Df(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Return(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Argtypes(ParsedKeyValue { key_range, val, .. })
                        | ParagraphArg::Title(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                        ParagraphArg::Args(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::KEYWORD);
                        }
                        ParagraphArg::Name(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        })
                        | ParagraphArg::MacroName(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::NAME);
                        }
                        ParagraphArg::Style(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Assoc(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Role(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Reorder(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Judgment(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::From(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::To(ParsedKeyValue { key_range, .. })
                        | ParagraphArg::Id(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                        ParagraphArg::Fors(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for (_, f) in val {
                                cont(*f, STeXSemanticTokens::SYMBOL);
                            }
                        }
                    }
                }
                for c in children {
                    c.semantic_tokens(cont);
                }
            }
            Self::Symdecl {
                main_name_range,
                token_range,
                parsed_args,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION);
                cont(*main_name_range, STeXSemanticTokens::NAME);

                for e in parsed_args {
                    match e {
                        SymdeclArg::Tp(ParsedKeyValue { key_range, val, .. })
                        | SymdeclArg::Df(ParsedKeyValue { key_range, val, .. })
                        | SymdeclArg::Return(ParsedKeyValue { key_range, val, .. })
                        | SymdeclArg::Argtypes(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                        SymdeclArg::Args(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::KEYWORD);
                        }
                        SymdeclArg::Name(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::NAME);
                        }
                        SymdeclArg::Style(ParsedKeyValue { key_range, .. })
                        | SymdeclArg::Assoc(ParsedKeyValue { key_range, .. })
                        | SymdeclArg::Role(ParsedKeyValue { key_range, .. })
                        | SymdeclArg::Reorder(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                    }
                }
            }
            Self::TextSymdecl {
                main_name_range,
                token_range,
                parsed_args,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION);
                cont(*main_name_range, STeXSemanticTokens::NAME);

                for e in parsed_args {
                    match e {
                        TextSymdeclArg::Tp(ParsedKeyValue { key_range, val, .. })
                        | TextSymdeclArg::Df(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                        TextSymdeclArg::Name(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::NAME);
                        }
                        TextSymdeclArg::Style(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                    }
                }
            }
            Self::Symdef {
                main_name_range,
                token_range,
                parsed_args,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION);
                cont(*main_name_range, STeXSemanticTokens::NAME);

                for e in parsed_args {
                    match e {
                        SymdefArg::Tp(ParsedKeyValue { key_range, val, .. })
                        | SymdefArg::Df(ParsedKeyValue { key_range, val, .. })
                        | SymdefArg::Return(ParsedKeyValue { key_range, val, .. })
                        | SymdefArg::Op(ParsedKeyValue { key_range, val, .. })
                        | SymdefArg::Argtypes(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                        SymdefArg::Args(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::KEYWORD);
                        }
                        SymdefArg::Name(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::NAME);
                        }
                        SymdefArg::Id(v, _) => cont(*v, STeXSemanticTokens::NAME),
                        SymdefArg::Style(ParsedKeyValue { key_range, .. })
                        | SymdefArg::Assoc(ParsedKeyValue { key_range, .. })
                        | SymdefArg::Role(ParsedKeyValue { key_range, .. })
                        | SymdefArg::Prec(ParsedKeyValue { key_range, .. })
                        | SymdefArg::Reorder(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                    }
                }
            }
            Self::RenameDecl {
                token_range,
                orig_range,
                name_range,
                macroname_range,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION);
                cont(*orig_range, STeXSemanticTokens::SYMBOL);
                if let Some(n) = name_range {
                    cont(*n, STeXSemanticTokens::NAME);
                }
                cont(*macroname_range, STeXSemanticTokens::NAME);
            }
            Self::Assign {
                token_range,
                orig_range,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION);
                cont(*orig_range, STeXSemanticTokens::SYMBOL);
            }
            Self::Svar { full_range, .. } => cont(*full_range, STeXSemanticTokens::VARIABLE),
            Self::Vardef {
                main_name_range,
                token_range,
                parsed_args,
                ..
            }
            | Self::Varseq {
                main_name_range,
                token_range,
                parsed_args,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::LOCAL);
                cont(*main_name_range, STeXSemanticTokens::NAME);

                for e in parsed_args {
                    match e {
                        VardefArg::Tp(ParsedKeyValue { key_range, val, .. })
                        | VardefArg::Df(ParsedKeyValue { key_range, val, .. })
                        | VardefArg::Return(ParsedKeyValue { key_range, val, .. })
                        | VardefArg::Op(ParsedKeyValue { key_range, val, .. })
                        | VardefArg::Argtypes(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                        VardefArg::Args(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::KEYWORD);
                        }
                        VardefArg::Name(ParsedKeyValue {
                            key_range,
                            val_range,
                            ..
                        }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            cont(*val_range, STeXSemanticTokens::NAME);
                        }
                        VardefArg::Id(v, _) => cont(*v, STeXSemanticTokens::NAME),
                        VardefArg::Style(ParsedKeyValue { key_range, .. })
                        | VardefArg::Assoc(ParsedKeyValue { key_range, .. })
                        | VardefArg::Role(ParsedKeyValue { key_range, .. })
                        | VardefArg::Prec(ParsedKeyValue { key_range, .. })
                        | VardefArg::Bind(ParsedKeyValue { key_range, .. })
                        | VardefArg::Reorder(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                    }
                }
            }
            Self::Notation {
                token_range,
                name_range,
                notation_args,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::DECLARATION);
                cont(*name_range, STeXSemanticTokens::SYMBOL);
                for e in notation_args {
                    match e {
                        NotationArg::Op(ParsedKeyValue { key_range, val, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD);
                            for c in val {
                                c.semantic_tokens(cont);
                            }
                        }
                        NotationArg::Id(v, _) => cont(*v, STeXSemanticTokens::NAME),
                        NotationArg::Prec(ParsedKeyValue { key_range, .. }) => {
                            cont(*key_range, STeXSemanticTokens::KEYWORD)
                        }
                    }
                }
            }
            Self::SymName {
                token_range,
                name_range,
                ..
            }
            | Self::Symref {
                token_range,
                name_range,
                ..
            }
            | Self::Symuse {
                token_range,
                name_range,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::REF_MACRO);
                cont(*name_range, STeXSemanticTokens::SYMBOL);
            }
            Self::Precondition {
                token_range,
                dim_range,
                symbol_range,
                ..
            }
            | Self::Objective {
                token_range,
                dim_range,
                symbol_range,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::REF_MACRO);
                cont(*dim_range, STeXSemanticTokens::KEYWORD);
                cont(*symbol_range, STeXSemanticTokens::SYMBOL);
            }
            Self::Definiens {
                token_range,
                name_range,
                ..
            } => {
                cont(*token_range, STeXSemanticTokens::REF_MACRO);
                if let Some(r) = name_range {
                    cont(*r, STeXSemanticTokens::SYMBOL);
                }
            }
            Self::SnifySuggestion { .. } => (),
        }
    }

    fn hover(&self, top_archive: Option<&ArchiveUri>, pos: LSPLineCol) -> Option<lsp::Hover> {
        fn uriname(pre: &str, d: &impl std::fmt::Display) -> String {
            format!("{pre}<sup>`{d}`</sup>")
        }
        //tracing::info!("Here: {self:?}");
        match self {
            Self::SymName {
                uri,
                name_range: range,
                ..
            }
            | Self::Symref {
                uri,
                name_range: range,
                ..
            }
            | Self::Notation {
                uri,
                name_range: range,
                ..
            }
            | Self::Symuse {
                uri,
                name_range: range,
                ..
            }
            | Self::Precondition {
                uri,
                symbol_range: range,
                ..
            }
            | Self::Objective {
                uri,
                symbol_range: range,
                ..
            }
            | Self::Definiens {
                uri,
                name_range: Some(range),
                ..
            } => Some(lsp::Hover {
                range: Some(StringRange::into_range(*range)),
                contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                    kind: lsp::MarkupKind::Markdown,
                    value: uriname("", &uri.first().unwrap_or_else(|| unreachable!()).uri),
                }),
            }),
            Self::SemanticMacro {
                uri,
                full_range: range,
                ..
            }
            | Self::ConservativeExt {
                uri,
                ext_range: range,
                ..
            }
            | Self::UseStructure {
                structure: uri,
                structure_range: range,
                ..
            }
            | Self::MorphismEnv {
                domain: ModuleOrStruct::Struct(uri),
                domain_range: range,
                ..
            }
            | Self::RenameDecl {
                uri,
                orig_range: range,
                ..
            }
            | Self::Assign {
                uri,
                orig_range: range,
                ..
            } => Some(lsp::Hover {
                range: Some(StringRange::into_range(*range)),
                contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                    kind: lsp::MarkupKind::Markdown,
                    value: uriname("", &uri.uri),
                }),
            }),
            Self::InlineMorphism {
                domain_range,
                domain,
                assignments,
                ..
            } => {
                if domain_range.contains(pos) {
                    let uri = match domain {
                        ModuleOrStruct::Struct(sym) => &sym.uri,
                        _ => return None,
                    };
                    return Some(lsp::Hover {
                        range: Some(StringRange::into_range(*domain_range)),
                        contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                            kind: lsp::MarkupKind::Markdown,
                            value: uriname("", uri),
                        }),
                    });
                }
                for a in assignments {
                    if a.symbol_range.contains(pos) {
                        return Some(lsp::Hover {
                            range: Some(StringRange::into_range(a.symbol_range)),
                            contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                                kind: lsp::MarkupKind::Markdown,
                                value: uriname("", &a.symbol.uri),
                            }),
                        });
                    }
                }
                None
            }
            Self::Svar {
                full_range, name, ..
            }
            | Self::VariableMacro {
                name, full_range, ..
            } => Some(lsp::Hover {
                range: Some(StringRange::into_range(*full_range)),
                contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                    kind: lsp::MarkupKind::Markdown,
                    value: uriname("Variable ", name),
                }),
            }),
            Self::MathStructure { extends, .. } => extends.iter().find_map(|(s, r)| {
                if r.contains(pos) {
                    Some(lsp::Hover {
                        range: Some(StringRange::into_range(*r)),
                        contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                            kind: lsp::MarkupKind::Markdown,
                            value: uriname("", &s.uri),
                        }),
                    })
                } else {
                    None
                }
            }),
            Self::Paragraph { parsed_args, .. } | Self::InlineParagraph { parsed_args, .. } => {
                for p in parsed_args {
                    if let ParagraphArg::Fors(ParsedKeyValue { val_range, val, .. }) = p {
                        if val_range.contains(pos) {
                            for (s, r) in val {
                                if r.contains(pos) {
                                    return Some(lsp::Hover {
                                        range: Some(StringRange::into_range(*r)),
                                        contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                                            kind: lsp::MarkupKind::Markdown,
                                            value: uriname(
                                                "",
                                                &s.first().unwrap_or_else(|| unreachable!()).uri,
                                            ),
                                        }),
                                    });
                                }
                            }
                        }
                        return None;
                    }
                }
                None
            }
            Self::MHGraphics {
                filepath,
                archive,
                full_range,
                ..
            } => {
                let Some(a) = archive.as_ref().map_or_else(
                    || top_archive.map(UriWithArchive::archive_id),
                    |(a, _)| Some(a),
                ) else {
                    return None;
                };
                let Some(uri) = uri_from_archive_relpath(a, &filepath.0) else {
                    return None;
                };
                Some(lsp::Hover {
                    range: Some(StringRange::into_range(*full_range)),
                    contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                        kind: lsp::MarkupKind::Markdown,
                        value: format!("![img]({uri})"),
                    }),
                })
            }
            Self::Module {
                uri,
                name_range,
                full_range,
                ..
            } => {
                let range = StringRange {
                    start: full_range.start,
                    end: name_range.end,
                };
                Some(lsp::Hover {
                    range: Some(StringRange::into_range(range)),
                    contents: lsp::HoverContents::Markup(lsp::MarkupContent {
                        kind: lsp::MarkupKind::Markdown,
                        value: uri.to_string(),
                    }),
                })
            }
            Self::ImportModule { .. }
            | Self::UseModule { .. }
            | Self::SetMetatheory { .. }
            | Self::Inputref { .. }
            | Self::IncludeProblem { .. }
            | Self::MHInput { .. }
            | Self::Symdecl { .. }
            | Self::Symdef { .. }
            | Self::Vardef { .. }
            | Self::Varseq { .. }
            | Self::Definiens { .. }
            | Self::TextSymdecl { .. }
            | Self::Problem { .. }
            | Self::Defnotation { .. }
            | Self::MorphismEnv { .. }
            | Self::SnifySuggestion { .. }
            | Self::SRef { .. } => None,
        }
    }
    fn inlay_hint(&self) -> Option<lsp::InlayHint> {
        match self {
            Self::SymName {
                uri,
                full_range,
                mode: mod_,
                ..
            } => {
                let name = uri
                    .first()
                    .unwrap_or_else(|| unreachable!())
                    .uri
                    .name()
                    .last();
                let name = format!("={}", mod_.apply(name));
                Some(lsp::InlayHint {
                    position: StringRange::into_range(*full_range).end,
                    label: lsp::InlayHintLabel::String(name),
                    kind: Some(lsp::InlayHintKind::PARAMETER),
                    text_edits: None,
                    tooltip: None,
                    padding_left: None,
                    padding_right: None,
                    data: None,
                })
            }
            Self::Definiens {
                uri,
                name_range: None,
                full_range,
                ..
            } => Some(lsp::InlayHint {
                position: StringRange::into_range(*full_range).end,
                label: lsp::InlayHintLabel::String(format!(
                    "[{}]",
                    uri.first().unwrap_or_else(|| unreachable!()).uri.name()
                )),
                kind: Some(lsp::InlayHintKind::PARAMETER),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: None,
                data: None,
            }),
            _ => None,
        }
    }
    fn code_action(&self, pos: LSPLineCol, url: &lsp::Url) -> lsp::CodeActionResponse {
        fn from_syms(
            url: &lsp::Url,
            v: &[SymbolReference<LSPLineCol>],
            r: StringRange<LSPLineCol>,
        ) -> lsp::CodeActionResponse {
            use std::fmt::Write;
            let all_strs: SmallVec<_, 2> = v
                .iter()
                .map(|u| {
                    let mut ret = u.uri.archive_id().to_string();
                    if let Some(p) = u.uri.path() {
                        ret.push('/');
                        ret.push_str(p.as_ref());
                    }
                    let _ = write!(ret, "?{}?{}", u.uri.module_name(), u.uri.name());
                    ret
                })
                .collect();
            v.iter()
                .map(|u| {
                    let disam = disamb(&u.uri, &all_strs);
                    let mut edits = std::collections::HashMap::default();
                    edits.insert(
                        url.clone(),
                        vec![lsp::TextEdit {
                            range: StringRange::into_range(r),
                            new_text: disam.clone(),
                        }],
                    );
                    lsp::CodeActionOrCommand::CodeAction(lsp::CodeAction {
                        title: disam,
                        kind: Some(lsp::CodeActionKind::QUICKFIX),
                        diagnostics: None,
                        edit: Some(lsp::WorkspaceEdit {
                            document_changes: None,
                            changes: Some(edits),
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: None,
                        disabled: None,
                        data: None,
                    })
                })
                .collect()
        }
        match self {
            Self::Notation {
                uri, name_range, ..
            }
            | Self::SymName {
                uri, name_range, ..
            }
            | Self::Symuse {
                uri, name_range, ..
            }
            | Self::Symref {
                uri, name_range, ..
            }
            | Self::Precondition {
                uri,
                symbol_range: name_range,
                ..
            }
            | Self::Objective {
                uri,
                symbol_range: name_range,
                ..
            }
            | Self::Definiens {
                uri,
                name_range: Some(name_range),
                ..
            } => {
                if !name_range.contains(pos) {
                    return Vec::new();
                }
                if uri.len() > 1 {
                    from_syms(url, uri, *name_range)
                } else {
                    Vec::new()
                }
            }

            Self::Paragraph { parsed_args, .. } | Self::InlineParagraph { parsed_args, .. } => {
                let fors = parsed_args.iter().find_map(|a| {
                    if let ParagraphArg::Fors(ls) = a {
                        Some(ls)
                    } else {
                        None
                    }
                });
                if let Some(fors) = fors {
                    for (s, r) in &fors.val {
                        if !r.contains(pos) {
                            continue;
                        }
                        if s.len() > 1 {
                            return from_syms(url, s, *r);
                        }
                        return Vec::new();
                    }
                }
                Vec::new()
            }
            Self::SnifySuggestion { range, symbols } => {
                if range.contains(pos) {
                    let mut ret: Vec<lsp::CodeActionOrCommand> = symbols
                        .iter()
                        .map(|(s, needs_usemodule)| {
                            lsp::CodeActionOrCommand::CodeAction(lsp::CodeAction {
                                title: s.to_string(),
                                kind: Some(lsp::CodeActionKind::QUICKFIX), //::QUICKFIX),
                                diagnostics: None,
                                edit: None,
                                command: Some(lsp::Command {
                                    title: "insert".to_string(),
                                    command: "snify/annotate".to_string(),
                                    arguments: Some(vec![
                                        s.to_string().into(),
                                        (*needs_usemodule).into(),
                                        url.to_string().into(),
                                        ::serde_json::to_value(*range).expect("wut"),
                                    ]),
                                }),
                                is_preferred: None,
                                disabled: None,
                                data: None,
                            })
                        })
                        .collect();
                    ret.push(lsp::CodeActionOrCommand::CodeAction(lsp::CodeAction {
                        title: "(ignore)".to_string(),
                        kind: Some(lsp::CodeActionKind::QUICKFIX), //::QUICKFIX),
                        diagnostics: None,
                        edit: None,
                        command: Some(lsp::Command {
                            title: "insert".to_string(),
                            command: "snify/annotate".to_string(),
                            arguments: Some(vec![
                                String::new().into(),
                                false.into(),
                                url.to_string().into(),
                                ::serde_json::to_value(*range).expect("wut"),
                            ]),
                        }),
                        is_preferred: None,
                        disabled: None,
                        data: None,
                    }));
                    ret
                } else {
                    Vec::new()
                }
            }
            Self::Problem { .. }
            | Self::Module { .. }
            | Self::MathStructure { .. }
            | Self::ConservativeExt { .. }
            | Self::MorphismEnv { .. }
            | Self::InlineMorphism { .. }
            | Self::SemanticMacro { .. }
            | Self::VariableMacro { .. }
            | Self::Svar { .. }
            | Self::ImportModule { .. }
            | Self::UseModule { .. }
            | Self::UseStructure { .. }
            | Self::SetMetatheory { .. }
            | Self::Inputref { .. }
            | Self::IncludeProblem { .. }
            | Self::MHGraphics { .. }
            | Self::MHInput { .. }
            | Self::Symdecl { .. }
            | Self::TextSymdecl { .. }
            | Self::RenameDecl { .. }
            | Self::Symdef { .. }
            | Self::Vardef { .. }
            | Self::Varseq { .. }
            | Self::Defnotation { .. }
            | Self::Definiens { .. }
            | Self::Assign { .. }
            | Self::SRef { .. } => Vec::new(),
        }
    }

    fn goto_definition(
        &self,
        in_doc: &UrlOrFile,
        pos: LSPLineCol,
    ) -> Option<lsp::GotoDefinitionResponse> {
        macro_rules! here {
            ($r:expr) => {
                Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                    uri: in_doc.clone().into(),
                    range: StringRange::into_range($r),
                }))
            };
        }
        match self {
            Self::Module { name_range, .. } => {
                if !name_range.contains(pos) {
                    return None;
                };
                here!(*name_range)
            }
            Self::MathStructure {
                extends,
                name_range,
                opts,
                ..
            } => {
                if name_range.contains(pos) {
                    return here!(*name_range);
                }
                for o in opts {
                    if let MathStructureArg::Name(range, _) = o {
                        if range.contains(pos) {
                            return here!(*range);
                        }
                    }
                }
                extends.iter().find_map(|(uri, r)| {
                    if r.contains(pos) {
                        let Some(p) = &uri.filepath else { return None };
                        let Ok(url) = lsp::Url::from_file_path(p) else {
                            return None;
                        };
                        //tracing::info!("Going to definition for {}: {}@{:?}",uri.uri,url,range);
                        Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                            uri: url,
                            range: StringRange::into_range(uri.range),
                        }))
                    } else {
                        None
                    }
                })
            }
            Self::MorphismEnv {
                domain_range,
                name_range,
                domain,
                ..
            } => {
                if name_range.contains(pos) {
                    return here!(*name_range);
                }
                if domain_range.contains(pos) {
                    let Some((p, range)) = (match domain {
                        ModuleOrStruct::Module(uri) => {
                            uri.full_path.as_ref().map(|r| (r, StringRange::default()))
                        }
                        ModuleOrStruct::Struct(uri) => {
                            uri.filepath.as_ref().map(|r| (r, uri.range))
                        }
                    }) else {
                        return None;
                    };
                    let Ok(url) = lsp::Url::from_file_path(p) else {
                        return None;
                    };
                    return Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                        uri: url,
                        range: StringRange::into_range(range),
                    }));
                } else {
                    None
                }
            }
            Self::InlineMorphism {
                domain_range,
                domain,
                assignments,
                name_range,
                ..
            } => {
                if name_range.contains(pos) {
                    return here!(*name_range);
                }
                if domain_range.contains(pos) {
                    let Some((p, range)) = (match domain {
                        ModuleOrStruct::Module(uri) => {
                            uri.full_path.as_ref().map(|r| (r, StringRange::default()))
                        }
                        ModuleOrStruct::Struct(uri) => {
                            uri.filepath.as_ref().map(|r| (r, uri.range))
                        }
                    }) else {
                        return None;
                    };
                    let Ok(url) = lsp::Url::from_file_path(p) else {
                        return None;
                    };
                    return Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                        uri: url,
                        range: StringRange::into_range(range),
                    }));
                }
                for a in assignments {
                    if a.symbol_range.contains(pos) {
                        let Some(p) = &a.symbol.filepath else {
                            return None;
                        };
                        let Ok(url) = lsp::Url::from_file_path(p) else {
                            return None;
                        };
                        return Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                            uri: url,
                            range: StringRange::into_range(a.symbol_range),
                        }));
                    }
                    if let Some((_, InlineMorphAssKind::Rename(_, _, r))) = &a.first {
                        if r.contains(pos) {
                            return here!(*r);
                        }
                    }
                    if let Some((_, InlineMorphAssKind::Rename(_, _, r))) = &a.second {
                        if r.contains(pos) {
                            return here!(*r);
                        }
                    }
                }
                None
            }
            Self::Paragraph { parsed_args, .. } | Self::InlineParagraph { parsed_args, .. } => {
                for p in parsed_args {
                    match p {
                        ParagraphArg::Fors(ParsedKeyValue { val_range, val, .. }) => {
                            if val_range.contains(pos) {
                                for (s, r) in val {
                                    if r.contains(pos) {
                                        let Some(p) =
                                            &s.first().unwrap_or_else(|| unreachable!()).filepath
                                        else {
                                            return None;
                                        };
                                        let Ok(url) = lsp::Url::from_file_path(p) else {
                                            return None;
                                        };
                                        //tracing::info!("Going to definition for {}: {}@{:?}",s.uri,url,range);
                                        return Some(lsp::GotoDefinitionResponse::Scalar(
                                            lsp::Location {
                                                uri: url,
                                                range: StringRange::into_range(
                                                    s.first()
                                                        .unwrap_or_else(|| unreachable!())
                                                        .range,
                                                ),
                                            },
                                        ));
                                    }
                                }
                            }
                            return None;
                        }
                        ParagraphArg::Name(ParsedKeyValue { val_range, .. })
                        | ParagraphArg::MacroName(ParsedKeyValue { val_range, .. })
                            if val_range.contains(pos) =>
                        {
                            return here!(*val_range);
                        }
                        _ => (),
                    }
                }
                None
            }
            Self::Symdecl {
                main_name_range,
                parsed_args,
                ..
            } => {
                if main_name_range.contains(pos) {
                    return here!(*main_name_range);
                }
                for a in parsed_args {
                    if let SymdeclArg::Name(ParsedKeyValue { val_range, .. }) = a {
                        if val_range.contains(pos) {
                            return here!(*val_range);
                        }
                    }
                }
                None
            }
            Self::TextSymdecl {
                main_name_range,
                parsed_args,
                ..
            } => {
                if main_name_range.contains(pos) {
                    return here!(*main_name_range);
                }
                for a in parsed_args {
                    if let TextSymdeclArg::Name(ParsedKeyValue { val_range, .. }) = a {
                        if val_range.contains(pos) {
                            return here!(*val_range);
                        }
                    }
                }
                None
            }
            Self::Symdef {
                main_name_range,
                parsed_args,
                ..
            } => {
                if main_name_range.contains(pos) {
                    return here!(*main_name_range);
                }
                for a in parsed_args {
                    if let SymdefArg::Name(ParsedKeyValue { val_range, .. }) = a {
                        if val_range.contains(pos) {
                            return here!(*val_range);
                        }
                    }
                }
                None
            }
            Self::RenameDecl {
                uri,
                orig_range,
                name_range,
                macroname_range,
                ..
            } => {
                if let Some(name_range) = name_range {
                    if name_range.contains(pos) {
                        return here!(*name_range);
                    }
                }
                if macroname_range.contains(pos) {
                    return here!(*macroname_range);
                }
                if !orig_range.contains(pos) {
                    return None;
                };
                let Some(p) = &uri.filepath else { return None };
                let Ok(url) = lsp::Url::from_file_path(p) else {
                    return None;
                };
                //tracing::info!("Going to definition for {}: {}@{:?}",uri.uri,url,range);
                Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                    uri: url,
                    range: StringRange::into_range(uri.range),
                }))
            }
            Self::Vardef {
                main_name_range,
                parsed_args,
                ..
            }
            | Self::Varseq {
                main_name_range,
                parsed_args,
                ..
            } => {
                if main_name_range.contains(pos) {
                    return here!(*main_name_range);
                }
                for a in parsed_args {
                    if let VardefArg::Name(ParsedKeyValue { val_range, .. }) = a {
                        if val_range.contains(pos) {
                            return here!(*val_range);
                        }
                    }
                }
                None
            }

            Self::ImportModule {
                module,
                archive_range,
                path_range,
                ..
            }
            | Self::UseModule {
                module,
                archive_range,
                path_range,
                ..
            }
            | Self::SetMetatheory {
                archive_range,
                path_range,
                module,
                ..
            } => {
                let range = archive_range.map_or(*path_range, |a| StringRange {
                    start: a.start,
                    end: path_range.end,
                });
                if !range.contains(pos) {
                    return None;
                };
                let Some(p) = module.full_path.as_ref() else {
                    return None;
                };
                let Ok(uri) = lsp::Url::from_file_path(p) else {
                    return None;
                };
                Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                    uri,
                    range: lsp::Range::default(),
                }))
            }
            Self::ConservativeExt { ext_range, uri, .. } => {
                if !ext_range.contains(pos) {
                    return None;
                };
                let Some(p) = &uri.filepath else { return None };
                let Ok(url) = lsp::Url::from_file_path(p) else {
                    return None;
                };
                //tracing::info!("Going to definition for {}: {}@{:?}",uri.uri,url,range);
                Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                    uri: url,
                    range: StringRange::into_range(uri.range),
                }))
            }
            Self::SymName {
                uri,
                name_range: range,
                ..
            }
            | Self::Symref {
                uri,
                name_range: range,
                ..
            }
            | Self::Notation {
                uri,
                name_range: range,
                ..
            }
            | Self::Symuse {
                uri,
                name_range: range,
                ..
            }
            | Self::Precondition {
                uri,
                symbol_range: range,
                ..
            }
            | Self::Objective {
                uri,
                symbol_range: range,
                ..
            }
            | Self::Definiens {
                uri,
                name_range: Some(range),
                ..
            } => {
                if !range.contains(pos) {
                    return None;
                };
                let Some(p) = &uri.first().unwrap_or_else(|| unreachable!()).filepath else {
                    return None;
                };
                let Ok(url) = lsp::Url::from_file_path(p) else {
                    return None;
                };
                //tracing::info!("Going to definition for {}: {}@{:?}",uri.uri,url,range);
                Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                    uri: url,
                    range: StringRange::into_range(
                        uri.first().unwrap_or_else(|| unreachable!()).range,
                    ),
                }))
            }
            Self::SemanticMacro {
                uri,
                token_range: range,
                ..
            }
            | Self::UseStructure {
                structure: uri,
                structure_range: range,
                ..
            }
            | Self::Assign {
                uri,
                orig_range: range,
                ..
            } => {
                if !range.contains(pos) {
                    return None;
                };
                let Some(p) = &uri.filepath else { return None };
                let Ok(url) = lsp::Url::from_file_path(p) else {
                    return None;
                };
                //tracing::info!("Going to definition for {}: {}@{:?}",uri.uri,url,range);
                Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                    uri: url,
                    range: StringRange::into_range(uri.range),
                }))
            }
            Self::VariableMacro {
                orig, full_range, ..
            } => {
                if !full_range.contains(pos) {
                    return None;
                };
                Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location {
                    uri: in_doc.clone().into(),
                    range: StringRange::into_range(*orig),
                }))
            }
            Self::Svar { .. }
            | Self::Inputref { .. }
            | Self::IncludeProblem { .. }
            | Self::MHGraphics { .. }
            | Self::MHInput { .. }
            | Self::Problem { .. }
            | Self::Definiens { .. }
            | Self::Defnotation { .. }
            | Self::SnifySuggestion { .. }
            | Self::SRef { .. } => None,
        }
    }
}

impl LSPState {
    #[must_use]
    pub fn get_diagnostics(
        &self,
        uri: &UrlOrFile,
        progress: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = lsp::DocumentDiagnosticReportResult> + use<>>
    {
        fn default() -> lsp::DocumentDiagnosticReportResult {
            lsp::DocumentDiagnosticReportResult::Report(lsp::DocumentDiagnosticReport::Full(
                lsp::RelatedFullDocumentDiagnosticReport::default(),
            ))
        }
        let d = self.get(uri)?;
        let slf = self.clone();
        Some(async move {
            d.with_annots(slf, false, |data| {
                let diags = &data.diagnostics;
                let r = lsp::DocumentDiagnosticReportResult::Report(
                    lsp::DocumentDiagnosticReport::Full(lsp::RelatedFullDocumentDiagnosticReport {
                        related_documents: None,
                        full_document_diagnostic_report: lsp::FullDocumentDiagnosticReport {
                            result_id: None,
                            items: diags.iter().map(to_diagnostic).collect(),
                        },
                    }),
                );
                tracing::trace!("diagnostics: {:?}", r);
                if let Some(p) = progress {
                    p.finish()
                }
                r
            })
            .await
            .unwrap_or_else(default)
        })
    }

    #[must_use]
    pub fn get_symbols(
        &self,
        uri: &UrlOrFile,
        progress: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<lsp::DocumentSymbolResponse>> + use<>>
    {
        #[allow(deprecated)]
        fn to_symbols(v: &[STeXAnnot]) -> Vec<lsp::DocumentSymbol> {
            let mut curr = v.iter();
            let mut ret = Vec::new();
            let mut stack = Vec::new();
            //tracing::info!("Annotations: {v:?}");
            loop {
                if let Some(e) = curr.next() {
                    if let Some((mut symbol, children)) = e.as_symbol() {
                        if children.is_empty() {
                            ret.push(symbol)
                        } else {
                            let old = std::mem::replace(&mut curr, children.iter());
                            symbol.children = Some(std::mem::take(&mut ret));
                            stack.push((old, symbol));
                        }
                    }
                } else if let Some((i, mut s)) = stack.pop() {
                    curr = i;
                    std::mem::swap(
                        &mut ret,
                        s.children.as_mut().unwrap_or_else(|| unreachable!()),
                    );
                    ret.push(s);
                } else {
                    break;
                }
            }
            //tracing::info!("Returns: {ret:?}");
            ret
        }

        let d = self.get(uri)?;
        let slf = self.clone();
        Some(d.with_annots(slf, false, |data| {
            let r = lsp::DocumentSymbolResponse::Nested(to_symbols(&data.annotations));
            tracing::trace!("document symbols: {:?}", r);
            if let Some(p) = progress {
                p.finish()
            }
            r
        }))
    }

    #[must_use]
    pub fn get_links(
        &self,
        uri: &UrlOrFile,
        progress: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<Vec<lsp::DocumentLink>>> + use<>> {
        let d = self.get(uri)?;
        let da = d.archive().cloned();
        let slf = self.clone();
        Some(d.with_annots(slf, false, move |data| {
            let mut ret = Vec::new();
            let iter: AnnotIter = data.annotations.iter().into();
            for e in <AnnotIter as TreeChildIter<STeXAnnot>>::dfs(iter) {
                e.links(da.as_ref(), |l| ret.push(l));
            }
            //tracing::info!("document links: {:?}",ret);
            if let Some(p) = progress {
                p.finish()
            }
            ret
        }))
    }

    pub fn prepare_module_hierarchy(
        &self,
        uri: UrlOrFile,
        _: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<Vec<lsp::CallHierarchyItem>>> + use<>>
    {
        let d = self.get(&uri)?;
        let url: lsp::Url = uri.into();
        d.document_uri().map(|doc| {
            std::future::ready(Some(vec![lsp::CallHierarchyItem {
                name: format!("{}.{}", doc.document_name(), doc.language()),
                kind: lsp::SymbolKind::FILE,
                tags: None,
                detail: None,
                uri: url.clone(),
                range: lsp::Range::default(), //{start:lsp::Position{line:0,character:0},end:lsp::Position{line:u32::MAX,character:u32::MAX}},
                selection_range: lsp::Range::default(),
                data: Some(doc.to_string().into()),
            }]))
        })
    }

    pub fn module_hierarchy_imports(
        &self,
        url: lsp::Url,
        kind: lsp::SymbolKind,
        uri: Uri,
        _: Option<ProgressCallbackClient>,
    ) -> Option<
        impl std::future::Future<Output = Option<Vec<lsp::CallHierarchyIncomingCall>>> + use<>,
    > {
        Some(std::future::ready({
            let url = url.into();
            let d = self.documents.read().get(&url).cloned()?;
            let annots = match d {
                DocData::Doc(d) => d.annotations,
                DocData::Data(d, _) => d,
            };
            let data = annots.lock();
            let mut rets = Vec::new();
            let (chs, usemods) = if kind == lsp::SymbolKind::FILE {
                (&*data.annotations, true)
            } else {
                let mut iter: AnnotIter = data.annotations.iter().into();
                (
                    iter.find_map(|e| match e {
                        STeXAnnot::Module {
                            uri: muri,
                            children,
                            ..
                        } if matches!(&uri,Uri::Module(u) if u == muri) => Some(&**children),
                        STeXAnnot::MathStructure {
                            uri: suri,
                            children,
                            extends,
                            ..
                        } if matches!(&uri,Uri::Symbol(u) if u == &suri.uri) => {
                            for (sym, range) in extends {
                                if let Some(p) = sym.filepath.as_ref() {
                                    let Ok(url) = lsp::Url::from_file_path(p) else {
                                        continue;
                                    };
                                    rets.push(lsp::CallHierarchyIncomingCall {
                                        from_ranges: vec![IsLSPRange::into_range(*range)],
                                        from: lsp::CallHierarchyItem {
                                            name: sym.uri.name().to_string(),
                                            detail: Some(
                                                sym.uri
                                                    .to_string()
                                                    .split_once("a=")
                                                    .unwrap_or_else(|| unreachable!())
                                                    .1
                                                    .to_string(),
                                            ),
                                            kind: lsp::SymbolKind::STRUCT,
                                            tags: None,
                                            uri: url,
                                            range: lsp::Range::default(),
                                            selection_range: lsp::Range::default(),
                                            data: Some(sym.uri.to_string().into()),
                                        },
                                    });
                                }
                            }
                            Some(&**children)
                        }
                        _ => None,
                    })?,
                    false,
                )
            };
            let iter: AnnotIter = chs.iter().into();
            for e in <AnnotIter as TreeChildIter<STeXAnnot>>::dfs(iter) {
                match e {
                    STeXAnnot::ImportModule {
                        module, full_range, ..
                    } if !usemods => {
                        let Some(p) = module.full_path.as_ref() else {
                            continue;
                        };
                        let Ok(url) = lsp::Url::from_file_path(p) else {
                            continue;
                        };
                        rets.push(lsp::CallHierarchyIncomingCall {
                            from_ranges: vec![lsp::Range::default()],
                            from: lsp::CallHierarchyItem {
                                detail: Some(
                                    module
                                        .uri
                                        .to_string()
                                        .split_once("a=")
                                        .unwrap_or_else(|| unreachable!())
                                        .1
                                        .to_string(),
                                ),
                                name: module.uri.module_name().to_string(),
                                kind: lsp::SymbolKind::CLASS,
                                tags: None,
                                uri: url,
                                range: IsLSPRange::into_range(*full_range),
                                selection_range: IsLSPRange::into_range(*full_range),
                                data: Some(module.uri.to_string().into()),
                            },
                        })
                    }
                    STeXAnnot::Module {
                        uri,
                        name_range,
                        full_range,
                        ..
                    } if usemods => rets.push(lsp::CallHierarchyIncomingCall {
                        from_ranges: vec![IsLSPRange::into_range(*full_range)],
                        from: lsp::CallHierarchyItem {
                            detail: Some(
                                uri.to_string()
                                    .split_once("a=")
                                    .unwrap_or_else(|| unreachable!())
                                    .1
                                    .to_string(),
                            ),
                            name: uri.module_name().to_string(),
                            kind: lsp::SymbolKind::MODULE,
                            tags: None,
                            uri: url.clone().into(),
                            range: IsLSPRange::into_range(*full_range),
                            selection_range: IsLSPRange::into_range(*name_range),
                            data: Some(uri.to_string().into()),
                        },
                    }),
                    STeXAnnot::UseModule {
                        module, full_range, ..
                    } if usemods => {
                        let Some(p) = module.full_path.as_ref() else {
                            continue;
                        };
                        let Ok(url) = lsp::Url::from_file_path(p) else {
                            continue;
                        };
                        rets.push(lsp::CallHierarchyIncomingCall {
                            from_ranges: vec![lsp::Range::default()],
                            from: lsp::CallHierarchyItem {
                                detail: Some(
                                    module
                                        .uri
                                        .to_string()
                                        .split_once("a=")
                                        .unwrap_or_else(|| unreachable!())
                                        .1
                                        .to_string(),
                                ),
                                name: module.uri.module_name().to_string(),
                                kind: lsp::SymbolKind::METHOD,
                                tags: None,
                                uri: url,
                                range: IsLSPRange::into_range(*full_range),
                                selection_range: IsLSPRange::into_range(*full_range),
                                data: Some(module.uri.to_string().into()),
                            },
                        })
                    }
                    _ => (),
                }
            }
            Some(rets)
        }))
    }

    #[must_use]
    pub fn get_references(
        &self,
        uri: UrlOrFile,
        position: lsp::Position,
        _: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<Vec<lsp::Location>>> + use<>> {
        let d = self.get(&uri)?;
        let pos = LSPLineCol::new(position.line, position.character);
        let slf = self.clone();
        enum Target {
            Module(ModuleUri),
            Structure(SymbolUri),
            Symbol(SymbolUri),
            Morphism(SymbolUri),
        }
        Some(async move {
            let e = d
                .with_annots(slf.clone(), false, move |data| {
                    match at_position(data, pos)? {
                        STeXAnnot::Module { uri, .. } => Some(Target::Module(uri.clone())),
                        STeXAnnot::MathStructure { uri, .. } => {
                            Some(Target::Structure(uri.uri.clone()))
                        }
                        STeXAnnot::MorphismEnv { uri, .. }
                        | STeXAnnot::InlineMorphism { uri, .. } => {
                            Some(Target::Morphism(uri.clone()))
                        }
                        STeXAnnot::Symdecl { uri, .. }
                        | STeXAnnot::TextSymdecl { uri, .. }
                        | STeXAnnot::Paragraph {
                            symbol: Some(uri), ..
                        }
                        | STeXAnnot::InlineParagraph {
                            symbol: Some(uri), ..
                        }
                        | STeXAnnot::Symdef { uri, .. } => Some(Target::Symbol(uri.uri.clone())),
                        STeXAnnot::RenameDecl { .. } => None, // TODO
                        STeXAnnot::Vardef { .. } | STeXAnnot::Varseq { .. } => {
                            // TODO
                            None
                        }
                        _ => None,
                    }
                })
                .await??;
            tokio::task::spawn_blocking(move || {
                let all = slf.documents.read();
                macro_rules! iter {
                    ($annot:ident => $then:expr) => {
                        for (k,v) in all.iter() {
                            let data = match v {
                                DocData::Data(d,_ ) => d,
                                DocData::Doc(d) => &d.annotations
                            };
                            let data = data.lock();
                            let iter : AnnotIter = data.annotations.iter().into();
                            macro_rules! here {
                                ($e:expr) => {
                                    lsp::Location {
                                        uri:k.clone().into(),
                                        range: StringRange::into_range($e)
                                    }
                                }
                            }
                            for $annot in <AnnotIter as TreeChildIter<STeXAnnot>>::dfs(iter) { $then }
                            drop(data);
                        }
                    }
                }
                let mut ret = Vec::new();
                match e {
                    Target::Module(muri) => {
                        iter!(a => match a {
                            STeXAnnot::ImportModule{module,full_range,..} | STeXAnnot::UseModule{module,full_range,..} if module.uri == muri => {
                                ret.push(here!(*full_range))
                            }
                            STeXAnnot::MorphismEnv{domain:ModuleOrStruct::Module(rf),domain_range,..} |
                            STeXAnnot::InlineMorphism{domain:ModuleOrStruct::Module(rf),domain_range,..}
                                if rf.uri == muri => ret.push(here!(*domain_range)),
                            _ => ()
                        })
                    }
                    Target::Structure(suri) | Target::Morphism(suri) | Target::Symbol(suri) => {
                        iter!(a => match a {
                            STeXAnnot::MathStructure{extends,..} => for (s,r) in extends {
                                if s.uri == suri { ret.push(here!(*r)); }
                            }
                            STeXAnnot::ConservativeExt{uri,extstructure_range:range,..} |
                            STeXAnnot::MorphismEnv{domain:ModuleOrStruct::Struct(uri),domain_range:range,..} |
                            STeXAnnot::InlineMorphism{domain:ModuleOrStruct::Struct(uri),domain_range:range,..} |
                            STeXAnnot::SemanticMacro{uri,token_range:range,..} |
                            STeXAnnot::UseStructure{structure:uri,structure_range:range,..} |
                            STeXAnnot::RenameDecl{uri,orig_range:range,..} |
                            STeXAnnot::Assign{uri,orig_range:range,..}
                                if uri.uri == suri => ret.push(here!(*range)),
                            STeXAnnot::Notation{uri,name_range:range,..} |
                            STeXAnnot::SymName{uri,name_range:range,..} |
                            STeXAnnot::Symuse{uri,name_range:range,..} |
                            STeXAnnot::Symref{uri,name_range:range,..} |
                            STeXAnnot::Precondition{uri,symbol_range:range,.. } |
                            STeXAnnot::Objective{uri,symbol_range:range,.. } |
                            STeXAnnot::Definiens{uri,full_range:range,..} => {
                                for u in uri {
                                    if u.uri == suri { ret.push(here!(*range)) }
                                }
                            }
                            STeXAnnot::Paragraph{parsed_args,..} | STeXAnnot::InlineParagraph{parsed_args,..} => {
                                for a in parsed_args {
                                    if let ParagraphArg::Fors(ParsedKeyValue{val,..}) = a {
                                        for (uri,range) in val {
                                            for u in uri {
                                                if u.uri == suri { ret.push(here!(*range)) }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => ()
                        })
                    }
                }
                ret
            }).await.ok()
        })
    }

    #[must_use]
    pub fn get_hover(
        &self,
        uri: &UrlOrFile,
        position: lsp::Position,
        _: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<lsp::Hover>> + use<>> {
        let d = self.get(uri)?;
        let da = d.archive().cloned();
        let pos = LSPLineCol::new(position.line, position.character);
        Some(
            d.with_annots(self.clone(), false, move |data| {
                at_position(data, pos).and_then(|e| e.hover(da.as_ref(), pos))
            })
            .map(|o| o.flatten()),
        )
    }

    #[must_use]
    pub fn get_codeaction(
        &self,
        uri: UrlOrFile,
        range: lsp::Range,
        _context: lsp::CodeActionContext,
        _: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<lsp::CodeActionResponse>> + use<>> {
        let d = self.get(&uri)?;
        let pos = LSPLineCol::new(range.start.line, range.start.character);
        let url = uri.into();
        Some(
            d.with_annots(self.clone(), false, move |data| {
                at_position(data, pos).map(|e| e.code_action(pos, &url))
            })
            .map(|o| o.flatten()),
        )
    }

    #[must_use]
    pub fn get_goto_definition(
        &self,
        uri: UrlOrFile,
        position: lsp::Position,
        _: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<lsp::GotoDefinitionResponse>> + use<>>
    {
        let d = self.get(&uri)?;
        let pos = LSPLineCol::new(position.line, position.character);
        Some(
            d.with_annots(self.clone(), false, move |data| {
                at_position(data, pos).and_then(|e| e.goto_definition(&uri, pos))
            })
            .map(|o| o.flatten()),
        )
    }

    #[must_use]
    pub fn get_inlay_hints(
        &self,
        uri: &UrlOrFile,
        _: Option<ProgressCallbackClient>,
    ) -> Option<impl std::future::Future<Output = Option<Vec<lsp::InlayHint>>> + use<>> {
        let d = self.get(uri)?;
        Some(d.with_annots(self.clone(), false, move |data| {
            let iter: AnnotIter = data.annotations.iter().into();
            <AnnotIter as TreeChildIter<STeXAnnot>>::dfs(iter)
                .filter_map(|e| e.inlay_hint())
                .collect()
        }))
    }

    pub fn get_semantic_tokens(
        &self,
        uri: &UrlOrFile,
        progress: Option<ProgressCallbackClient>,
        _range: Option<lsp::Range>,
    ) -> Option<impl std::future::Future<Output = Option<lsp::SemanticTokens>> + use<>> {
        //let range = range.map(SourceRange::from_range);
        let d = self.get(uri)?;
        Some(d.with_annots(self.clone(), false, |data| {
            let mut ret = Vec::new();
            let mut curr = (0u32, 0u32);
            for e in data.annotations.iter() {
                e.semantic_tokens(&mut |range, tp| {
                    if range.start.line < curr.0 {
                        return;
                    }
                    let delta_line = range.start.line - curr.0;
                    let delta_start = if delta_line == 0 {
                        if range.start.col > curr.1 {
                            range.start.col - curr.1
                        } else {
                            return;
                        }
                    } else {
                        range.start.col
                    };
                    curr = (range.start.line, range.start.col);
                    if range.start.line == range.end.line {
                        let length = if range.end.col < range.start.col {
                            999
                        } else {
                            range.end.col - range.start.col
                        };
                        ret.push(lsp::SemanticToken {
                            delta_line,
                            delta_start,
                            length,
                            token_type: tp,
                            token_modifiers_bitset: 0,
                        });
                    } else {
                        ret.push(lsp::SemanticToken {
                            delta_line,
                            delta_start,
                            length: 999,
                            token_type: tp,
                            token_modifiers_bitset: 0,
                        });
                        // TODO
                    }
                });
            }

            if let Some(p) = progress {
                p.finish()
            }
            lsp::SemanticTokens {
                result_id: None,
                data: ret,
            }
        }))
    }
}

fn at_position(data: &STeXParseDataI, position: LSPLineCol) -> Option<&STeXAnnot> {
    let mut ret = None;
    let iter: AnnotIter = data.annotations.iter().into();
    for e in <AnnotIter as TreeChildIter<STeXAnnot>>::dfs(iter) {
        let range = e.range();
        if range.contains(position) {
            ret = Some(e);
        } else if range.start > position {
            if ret.is_some() {
                break;
            }
        }
    }
    ret
}

#[must_use]
pub fn to_diagnostic(diag: &STeXDiagnostic) -> lsp::Diagnostic {
    lsp::Diagnostic {
        range: diag.range.into_range(),
        severity: Some(match diag.level {
            DiagnosticLevel::Error => lsp::DiagnosticSeverity::ERROR,
            DiagnosticLevel::Info => lsp::DiagnosticSeverity::INFORMATION,
            DiagnosticLevel::Warning => lsp::DiagnosticSeverity::WARNING,
            DiagnosticLevel::Hint => lsp::DiagnosticSeverity::HINT,
        }),
        code: None,
        code_description: None,
        source: None,
        message: diag.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}
#[must_use]
pub fn into_diagnostic(diag: STeXDiagnostic) -> lsp::Diagnostic {
    lsp::Diagnostic {
        range: diag.range.into_range(),
        severity: Some(match diag.level {
            DiagnosticLevel::Error => lsp::DiagnosticSeverity::ERROR,
            DiagnosticLevel::Info => lsp::DiagnosticSeverity::INFORMATION,
            DiagnosticLevel::Warning => lsp::DiagnosticSeverity::WARNING,
            DiagnosticLevel::Hint => lsp::DiagnosticSeverity::HINT,
        }),
        code: None,
        code_description: None,
        source: None,
        message: diag.message,
        related_information: None,
        tags: None,
        data: None,
    }
}

fn disamb(uri: &SymbolUri, all: &[String]) -> String {
    let mut ret = format!("?{}", uri.name());
    if all.iter().filter(|s| s.ends_with(&ret)).count() == 1 {
        return ret;
    }
    ret = format!("?{}{ret}", uri.module_name());
    if all.iter().filter(|s| s.ends_with(&ret)).count() == 1 {
        return ret;
    }
    if let Some(path) = uri.path() {
        let mut had_path = false;
        for s in path.steps().rev() {
            if had_path {
                ret = format!("{s}/{ret}");
            } else {
                had_path = true;
                ret = format!("{s}{ret}");
            }
            if all.iter().filter(|s| s.ends_with(&ret)).count() == 1 {
                return ret;
            }
        }
    }
    for i in uri.archive_id().steps().rev() {
        ret = format!("{i}/{ret}");
        if all.iter().filter(|s| s.ends_with(&ret)).count() == 1 {
            return ret;
        }
    }
    ret
}
