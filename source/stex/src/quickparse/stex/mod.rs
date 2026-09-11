pub mod rules;
pub mod structs;

use flams_math_archives::backend::AnyBackend;
use flams_utils::{
    prelude::{TreeChild, TreeLike},
    sourcerefs::{LSPLineCol, StringRange},
    vecmap::VecSet,
};
use ftml_ontology::narrative::elements::{paragraphs::ParagraphKind, problems::CognitiveDimension};
use ftml_solver::results::DocumentCheckResult;
use ftml_uris::{
    ArchiveId, DocumentElementUri, DocumentUri, Language, ModuleUri, SymbolUri, UriName,
    UriWithArchive,
};
use rules::{
    MathStructureArg, MathStructureArgIter, NotationArg, NotationArgIter, ParagraphArg,
    ParagraphArgIter, ProblemArg, ProblemArgIter, SModuleArg, SModuleArgIter, SymdeclArg,
    SymdeclArgIter, SymdefArg, SymdefArgIter, TextSymdeclArg, TextSymdeclArgIter, VardefArg,
    VardefArgIter,
};
use smallvec::SmallVec;
use std::path::Path;
use structs::{
    InlineMorphAssIter, InlineMorphAssign, ModuleOrStruct, ModuleReference, ModuleRules,
    MorphismKind, STeXModuleStore, STeXParseState, STeXToken, SymbolReference, SymnameMode,
};

use crate::quickparse::stex::rules::{
    IncludeProblemArg, MHGraphicsArg, SRefOptsA, SRefOptsAIter, SRefOptsB, SRefOptsBIter,
};

use super::latex::LaTeXParser;

#[derive(Default, Debug)]
pub struct STeXParseDataI {
    pub annotations: Vec<STeXAnnot>,
    pub diagnostics: VecSet<STeXDiagnostic>,
    pub check: Option<DocumentCheckResult>,
    pub modules: SmallVec<(ModuleUri, ModuleRules<LSPLineCol>), 1>,
    pub dependencies: Vec<std::sync::Arc<Path>>,
}
impl STeXParseDataI {
    #[inline]
    #[must_use]
    pub fn lock(self) -> STeXParseData {
        flams_utils::triomphe::Arc::new(parking_lot::Mutex::new(self))
    }
    #[inline]
    pub fn replace(self, old: &STeXParseData) {
        *old.lock() = self;
    }
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.annotations.is_empty() && self.diagnostics.is_empty()
    }
}

pub type STeXParseData = flams_utils::triomphe::Arc<parking_lot::Mutex<STeXParseDataI>>;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize)]
pub enum STeXAnnot {
    Module {
        uri: ModuleUri,
        name_range: StringRange<LSPLineCol>,
        opts: Vec<SModuleArg<LSPLineCol, Self>>,
        sig: Option<Language>,
        meta_theory: Option<ModuleReference>,
        full_range: StringRange<LSPLineCol>,
        smodule_range: StringRange<LSPLineCol>,
        children: Vec<Self>,
    },
    MathStructure {
        uri: SymbolReference<LSPLineCol>,
        extends: Vec<(SymbolReference<LSPLineCol>, StringRange<LSPLineCol>)>,
        name_range: StringRange<LSPLineCol>,
        opts: Vec<MathStructureArg<LSPLineCol, Self>>,
        full_range: StringRange<LSPLineCol>,
        children: Vec<Self>,
        mathstructure_range: StringRange<LSPLineCol>,
    },
    ConservativeExt {
        uri: SymbolReference<LSPLineCol>,
        ext_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
        extstructure_range: StringRange<LSPLineCol>,
        children: Vec<Self>,
    },
    MorphismEnv {
        full_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
        env_range: StringRange<LSPLineCol>,
        uri: SymbolUri,
        star: bool,
        domain: ModuleOrStruct<LSPLineCol>,
        domain_range: StringRange<LSPLineCol>,
        kind: MorphismKind,
        children: Vec<Self>,
    },
    InlineMorphism {
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
        uri: SymbolUri,
        domain: ModuleOrStruct<LSPLineCol>,
        domain_range: StringRange<LSPLineCol>,
        kind: MorphismKind,
        assignments: Vec<InlineMorphAssign<LSPLineCol, Self>>,
    },
    SemanticMacro {
        uri: SymbolReference<LSPLineCol>,
        argnum: u8,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    VariableMacro {
        name: UriName,
        argnum: u8,
        orig: StringRange<LSPLineCol>,
        sequence: bool,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    Svar {
        name: UriName,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
        arg_range: StringRange<LSPLineCol>,
        name_range: Option<StringRange<LSPLineCol>>,
    },
    ImportModule {
        archive_range: Option<StringRange<LSPLineCol>>,
        path_range: StringRange<LSPLineCol>,
        module: ModuleReference,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    UseModule {
        archive_range: Option<StringRange<LSPLineCol>>,
        path_range: StringRange<LSPLineCol>,
        module: ModuleReference,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    UseStructure {
        structure: SymbolReference<LSPLineCol>,
        structure_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    SetMetatheory {
        archive_range: Option<StringRange<LSPLineCol>>,
        path_range: StringRange<LSPLineCol>,
        module: ModuleReference,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    Inputref {
        archive: Option<(ArchiveId, StringRange<LSPLineCol>)>,
        filepath: (std::sync::Arc<str>, StringRange<LSPLineCol>),
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    MHInput {
        archive: Option<(ArchiveId, StringRange<LSPLineCol>)>,
        filepath: (std::sync::Arc<str>, StringRange<LSPLineCol>),
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    #[allow(clippy::type_complexity)]
    Symdecl {
        uri: SymbolReference<LSPLineCol>,
        main_name_range: StringRange<LSPLineCol>,
        parsed_args: Vec<SymdeclArg<LSPLineCol, Self>>,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    #[allow(clippy::type_complexity)]
    TextSymdecl {
        uri: SymbolReference<LSPLineCol>,
        main_name_range: StringRange<LSPLineCol>,
        parsed_args: Vec<TextSymdeclArg<LSPLineCol, Self>>,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    Notation {
        uri: SmallVec<SymbolReference<LSPLineCol>, 1>,
        token_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
        notation_args: Vec<NotationArg<LSPLineCol, Self>>,
        full_range: StringRange<LSPLineCol>,
    },
    RenameDecl {
        uri: SymbolReference<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        orig_range: StringRange<LSPLineCol>,
        name_range: Option<StringRange<LSPLineCol>>,
        macroname_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    Assign {
        uri: SymbolReference<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        orig_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    #[allow(clippy::type_complexity)]
    Symdef {
        uri: SymbolReference<LSPLineCol>,
        main_name_range: StringRange<LSPLineCol>,
        parsed_args: Vec<SymdefArg<LSPLineCol, Self>>,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    #[allow(clippy::type_complexity)]
    Vardef {
        name: UriName,
        main_name_range: StringRange<LSPLineCol>,
        parsed_args: Vec<VardefArg<LSPLineCol, Self>>,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    #[allow(clippy::type_complexity)]
    Varseq {
        name: UriName,
        main_name_range: StringRange<LSPLineCol>,
        parsed_args: Vec<VardefArg<LSPLineCol, Self>>,
        token_range: StringRange<LSPLineCol>,
        full_range: StringRange<LSPLineCol>,
    },
    SymName {
        is_def: bool,
        uri: SmallVec<SymbolReference<LSPLineCol>, 1>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
        mode: SymnameMode<LSPLineCol>,
    },
    IncludeProblem {
        filepath: (std::sync::Arc<str>, StringRange<LSPLineCol>),
        archive: Option<(ArchiveId, StringRange<LSPLineCol>)>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        args: Vec<IncludeProblemArg<LSPLineCol>>,
    },
    Symuse {
        uri: SmallVec<SymbolReference<LSPLineCol>, 1>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
    },
    Symref {
        is_def: bool,
        uri: SmallVec<SymbolReference<LSPLineCol>, 1>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
        text: (StringRange<LSPLineCol>, Vec<Self>),
    },
    Definiens {
        uri: SmallVec<SymbolReference<LSPLineCol>, 1>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        name_range: Option<StringRange<LSPLineCol>>,
    },
    Defnotation {
        full_range: StringRange<LSPLineCol>,
    },
    Paragraph {
        kind: ParagraphKind,
        full_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
        symbol: Option<SymbolReference<LSPLineCol>>,
        parsed_args: Vec<ParagraphArg<LSPLineCol, Self>>,
        children: Vec<Self>,
    },
    Problem {
        sub: bool,
        full_range: StringRange<LSPLineCol>,
        name_range: StringRange<LSPLineCol>,
        parsed_args: Vec<ProblemArg<LSPLineCol, Self>>,
        children: Vec<Self>,
    },
    Precondition {
        uri: SmallVec<SymbolReference<LSPLineCol>, 1>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        dim_range: StringRange<LSPLineCol>,
        symbol_range: StringRange<LSPLineCol>,
        dim: CognitiveDimension,
    },
    Objective {
        uri: SmallVec<SymbolReference<LSPLineCol>, 1>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        dim_range: StringRange<LSPLineCol>,
        symbol_range: StringRange<LSPLineCol>,
        dim: CognitiveDimension,
    },
    InlineParagraph {
        kind: ParagraphKind,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        symbol: Option<SymbolReference<LSPLineCol>>,
        parsed_args: Vec<ParagraphArg<LSPLineCol, Self>>,
        children: Vec<Self>,
        children_range: StringRange<LSPLineCol>,
    },
    MHGraphics {
        filepath: (std::sync::Arc<str>, StringRange<LSPLineCol>),
        archive: Option<(ArchiveId, StringRange<LSPLineCol>)>,
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        args: Vec<MHGraphicsArg<LSPLineCol>>,
    },
    SRef {
        full_range: StringRange<LSPLineCol>,
        token_range: StringRange<LSPLineCol>,
        opt_args: Vec<SRefOptsA<LSPLineCol, Self>>,
        label_range: StringRange<LSPLineCol>,
        in_opt_args: Vec<SRefOptsB<LSPLineCol, Self>>,
        target: DocumentElementUri,
        target_path: std::sync::Arc<Path>,
        in_doc: Option<(DocumentUri, std::sync::Arc<Path>)>,
    },
    SnifySuggestion {
        range: StringRange<LSPLineCol>,
        symbols: SmallVec<(SymbolUri, bool), 1>,
    },
}
impl STeXAnnot {
    #[allow(clippy::too_many_lines)]
    fn from_tokens<I: IntoIterator<Item = STeXToken<LSPLineCol>>>(
        iter: I,
        mut modules: Option<&mut SmallVec<(ModuleUri, ModuleRules<LSPLineCol>), 1>>,
    ) -> Vec<Self> {
        let mut v = Vec::new();
        macro_rules! cont {
      ($e:ident) => { $e.into_iter().map(|o| o.into_other(cont!(+))).collect() };
      (+) => { |v| Self::from_tokens(v,if let Some(m) = modules.as_mut() { Some(*m) } else { None }) };
    }
        for t in iter {
            match t {
                STeXToken::SnifySuggestion { range, symbols } => {
                    v.push(Self::SnifySuggestion { range, symbols })
                }
                STeXToken::Module {
                    uri,
                    name_range,
                    sig,
                    meta_theory,
                    full_range,
                    smodule_range,
                    children,
                    rules,
                    opts,
                } => {
                    if let Some(ref mut m) = modules {
                        m.push((uri.clone(), rules));
                    }
                    v.push(Self::Module {
                        uri,
                        name_range,
                        sig,
                        meta_theory,
                        full_range,
                        smodule_range,
                        opts: cont!(opts),
                        children: Self::from_tokens(children, None),
                    });
                }
                STeXToken::SRef {
                    full_range,
                    token_range,
                    opt_args,
                    label_range,
                    in_opt_args,
                    target,
                    target_path,
                    in_doc,
                } => v.push(Self::SRef {
                    full_range,
                    token_range,
                    opt_args: cont!(opt_args),
                    label_range,
                    in_opt_args: cont!(in_opt_args),
                    target,
                    target_path,
                    in_doc,
                }),
                STeXToken::MHGraphics {
                    filepath,
                    archive,
                    full_range,
                    token_range,
                    args,
                } => v.push(Self::MHGraphics {
                    filepath,
                    archive,
                    full_range,
                    token_range,
                    args,
                }),
                STeXToken::UseStructure {
                    structure,
                    structure_range,
                    full_range,
                    token_range,
                } => v.push(Self::UseStructure {
                    structure,
                    structure_range,
                    full_range,
                    token_range,
                }),
                STeXToken::ConservativeExt {
                    uri,
                    ext_range,
                    full_range,
                    children,
                    extstructure_range,
                } => v.push(Self::ConservativeExt {
                    uri,
                    ext_range,
                    full_range,
                    children: Self::from_tokens(children, None),
                    extstructure_range,
                }),
                STeXToken::MathStructure {
                    uri,
                    extends,
                    name_range,
                    opts,
                    full_range,
                    children,
                    mathstructure_range,
                    ..
                } => {
                    v.push(Self::MathStructure {
                        uri,
                        extends,
                        name_range,
                        opts: cont!(opts),
                        full_range,
                        children: Self::from_tokens(children, None),
                        mathstructure_range,
                    });
                }
                STeXToken::MorphismEnv {
                    uri,
                    star,
                    env_range,
                    full_range,
                    children,
                    name_range,
                    domain,
                    domain_range,
                    kind,
                    ..
                } => v.push(Self::MorphismEnv {
                    uri,
                    env_range,
                    star,
                    full_range,
                    children: Self::from_tokens(children, None),
                    name_range,
                    domain,
                    domain_range,
                    kind,
                }),
                STeXToken::InlineMorphism {
                    full_range,
                    token_range,
                    name_range,
                    uri,
                    domain,
                    domain_range,
                    kind,
                    assignments,
                    ..
                } => v.push(Self::InlineMorphism {
                    full_range,
                    token_range,
                    name_range,
                    uri,
                    domain,
                    domain_range,
                    kind,
                    assignments: cont!(assignments),
                }),
                STeXToken::SemanticMacro {
                    uri,
                    argnum,
                    token_range,
                    full_range,
                } => v.push(Self::SemanticMacro {
                    uri,
                    argnum,
                    token_range,
                    full_range,
                }),
                STeXToken::VariableMacro {
                    name,
                    sequence,
                    argnum,
                    orig,
                    token_range,
                    full_range,
                } => v.push(Self::VariableMacro {
                    name,
                    argnum,
                    sequence,
                    orig,
                    token_range,
                    full_range,
                }),
                STeXToken::Svar {
                    name,
                    full_range,
                    token_range,
                    name_range,
                    arg_range,
                } => v.push(Self::Svar {
                    name,
                    full_range,
                    token_range,
                    name_range,
                    arg_range,
                }),
                STeXToken::ImportModule {
                    archive_range,
                    path_range,
                    module,
                    token_range,
                    full_range,
                } => v.push(Self::ImportModule {
                    archive_range,
                    path_range,
                    module,
                    token_range,
                    full_range,
                }),
                STeXToken::UseModule {
                    archive_range,
                    path_range,
                    module,
                    token_range,
                    full_range,
                } => v.push(Self::UseModule {
                    archive_range,
                    path_range,
                    module,
                    token_range,
                    full_range,
                }),
                STeXToken::IncludeProblem {
                    filepath,
                    full_range,
                    token_range,
                    archive,
                    args,
                } => v.push(Self::IncludeProblem {
                    filepath,
                    archive,
                    full_range,
                    token_range,
                    args,
                }),
                STeXToken::SetMetatheory {
                    archive_range,
                    path_range,
                    module,
                    token_range,
                    full_range,
                } => v.push(Self::SetMetatheory {
                    archive_range,
                    path_range,
                    module,
                    token_range,
                    full_range,
                }),
                STeXToken::Inputref {
                    archive,
                    filepath,
                    token_range,
                    full_range,
                } => v.push(Self::Inputref {
                    archive,
                    filepath,
                    token_range,
                    full_range,
                }),
                STeXToken::MHInput {
                    archive,
                    filepath,
                    token_range,
                    full_range,
                } => v.push(Self::MHInput {
                    archive,
                    filepath,
                    token_range,
                    full_range,
                }),
                STeXToken::Symdecl {
                    uri,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args,
                } => v.push(Self::Symdecl {
                    uri,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args: cont!(parsed_args),
                }),
                STeXToken::TextSymdecl {
                    uri,
                    main_name_range,
                    full_range,
                    parsed_args,
                    token_range,
                } => v.push(Self::TextSymdecl {
                    uri,
                    main_name_range,
                    full_range,
                    token_range,
                    parsed_args: cont!(parsed_args),
                }),
                STeXToken::Definiens {
                    uri,
                    full_range,
                    token_range,
                    name_range,
                } => v.push(Self::Definiens {
                    uri,
                    full_range,
                    token_range,
                    name_range,
                }),
                STeXToken::Defnotation { full_range } => {
                    v.push(Self::Defnotation { full_range });
                }
                STeXToken::Notation {
                    uri,
                    token_range,
                    name_range,
                    notation_args,
                    full_range,
                } => v.push(Self::Notation {
                    uri,
                    token_range,
                    name_range,
                    full_range,
                    notation_args: cont!(notation_args),
                }),
                STeXToken::Symdef {
                    uri,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args,
                } => v.push(Self::Symdef {
                    uri,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args: cont!(parsed_args),
                }),
                STeXToken::Vardef {
                    name,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args,
                } => v.push(Self::Vardef {
                    name,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args: cont!(parsed_args),
                }),
                STeXToken::Varseq {
                    name,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args,
                } => v.push(Self::Varseq {
                    name,
                    main_name_range,
                    token_range,
                    full_range,
                    parsed_args: cont!(parsed_args),
                }),
                STeXToken::Symref {
                    uri,
                    is_def,
                    full_range,
                    token_range,
                    name_range,
                    text,
                } => v.push(Self::Symref {
                    uri,
                    is_def,
                    full_range,
                    token_range,
                    name_range,
                    text: (text.0, Self::from_tokens(text.1, None)),
                }),
                STeXToken::Precondition {
                    uri,
                    full_range,
                    token_range,
                    dim_range,
                    symbol_range,
                    dim,
                } => v.push(Self::Precondition {
                    uri,
                    full_range,
                    token_range,
                    dim_range,
                    symbol_range,
                    dim,
                }),
                STeXToken::Objective {
                    uri,
                    full_range,
                    token_range,
                    dim_range,
                    symbol_range,
                    dim,
                } => v.push(Self::Objective {
                    uri,
                    full_range,
                    token_range,
                    dim_range,
                    symbol_range,
                    dim,
                }),
                STeXToken::SymName {
                    uri,
                    is_def,
                    full_range,
                    token_range,
                    name_range,
                    mode: mod_,
                } => v.push(Self::SymName {
                    uri,
                    is_def,
                    full_range,
                    token_range,
                    name_range,
                    mode: mod_,
                }),
                STeXToken::Symuse {
                    uri,
                    full_range,
                    token_range,
                    name_range,
                } => v.push(Self::Symuse {
                    uri,
                    full_range,
                    token_range,
                    name_range,
                }),
                STeXToken::Paragraph {
                    kind,
                    full_range,
                    name_range,
                    symbol,
                    parsed_args,
                    children,
                } => v.push(Self::Paragraph {
                    symbol,
                    kind,
                    full_range,
                    name_range,
                    parsed_args: cont!(parsed_args),
                    children: Self::from_tokens(children, None),
                }),
                STeXToken::Problem {
                    sub,
                    full_range,
                    name_range,
                    parsed_args,
                    children,
                } => v.push(Self::Problem {
                    sub,
                    full_range,
                    name_range,
                    parsed_args: cont!(parsed_args),
                    children: Self::from_tokens(children, None),
                }),
                STeXToken::InlineParagraph {
                    kind,
                    full_range,
                    token_range,
                    children_range,
                    symbol,
                    parsed_args,
                    children,
                } => v.push(Self::InlineParagraph {
                    symbol,
                    kind,
                    full_range,
                    token_range,
                    children_range,
                    parsed_args: cont!(parsed_args),
                    children: Self::from_tokens(children, None),
                }),
                STeXToken::RenameDecl {
                    uri,
                    token_range,
                    orig_range,
                    name_range,
                    macroname_range,
                    full_range,
                } => v.push(Self::RenameDecl {
                    uri,
                    token_range,
                    orig_range,
                    name_range,
                    macroname_range,
                    full_range,
                }),
                STeXToken::Assign {
                    uri,
                    token_range,
                    orig_range,
                    full_range,
                } => v.push(Self::Assign {
                    uri,
                    token_range,
                    orig_range,
                    full_range,
                }),
                STeXToken::Vec(vi) => v.extend(Self::from_tokens(
                    vi,
                    modules.as_mut().map_or(None, |m| Some(*m)),
                )),
            }
        }
        v
    }

    #[must_use]
    #[inline]
    pub const fn range(&self) -> StringRange<LSPLineCol> {
        match self {
            Self::Module { full_range, .. }
            | Self::MathStructure { full_range, .. }
            | Self::SemanticMacro { full_range, .. }
            | Self::ImportModule { full_range, .. }
            | Self::UseModule { full_range, .. }
            | Self::SetMetatheory { full_range, .. }
            | Self::Symdecl { full_range, .. }
            | Self::Symdef { full_range, .. }
            | Self::IncludeProblem { full_range, .. }
            | Self::SymName { full_range, .. }
            | Self::Symuse { full_range, .. }
            | Self::Symref { full_range, .. }
            | Self::Vardef { full_range, .. }
            | Self::VariableMacro { full_range, .. }
            | Self::Varseq { full_range, .. }
            | Self::Notation { full_range, .. }
            | Self::Svar { full_range, .. }
            | Self::Definiens { full_range, .. }
            | Self::Defnotation { full_range }
            | Self::ConservativeExt { full_range, .. }
            | Self::Paragraph { full_range, .. }
            | Self::Problem { full_range, .. }
            | Self::UseStructure { full_range, .. }
            | Self::InlineParagraph { full_range, .. }
            | Self::MorphismEnv { full_range, .. }
            | Self::RenameDecl { full_range, .. }
            | Self::Assign { full_range, .. }
            | Self::Inputref { full_range, .. }
            | Self::MHInput { full_range, .. }
            | Self::InlineMorphism { full_range, .. }
            | Self::Precondition { full_range, .. }
            | Self::Objective { full_range, .. }
            | Self::MHGraphics { full_range, .. }
            | Self::SRef { full_range, .. }
            | Self::SnifySuggestion {
                range: full_range, ..
            }
            | Self::TextSymdecl { full_range, .. } => *full_range,
        }
    }
}

pub enum AnnotIter<'a> {
    Module(
        std::iter::Chain<
            SModuleArgIter<'a, LSPLineCol, STeXAnnot>,
            std::slice::Iter<'a, STeXAnnot>,
        >,
    ),
    InlineAss(InlineMorphAssIter<'a, LSPLineCol, STeXAnnot>),
    Slice(std::slice::Iter<'a, STeXAnnot>),
    Paragraph(std::iter::Chain<ParagraphArgIter<'a, STeXAnnot>, std::slice::Iter<'a, STeXAnnot>>),
    Problem(std::iter::Chain<ProblemArgIter<'a, STeXAnnot>, std::slice::Iter<'a, STeXAnnot>>),
    Structure(
        std::iter::Chain<
            MathStructureArgIter<'a, LSPLineCol, STeXAnnot>,
            std::slice::Iter<'a, STeXAnnot>,
        >,
    ),
    SRef(
        std::iter::Chain<
            SRefOptsAIter<'a, LSPLineCol, STeXAnnot>,
            SRefOptsBIter<'a, LSPLineCol, STeXAnnot>,
        >,
    ),
    Symdecl(SymdeclArgIter<'a, LSPLineCol, STeXAnnot>),
    TextSymdecl(TextSymdeclArgIter<'a, LSPLineCol, STeXAnnot>),
    Notation(NotationArgIter<'a, LSPLineCol, STeXAnnot>),
    Symdef(SymdefArgIter<'a, LSPLineCol, STeXAnnot>),
    Vardef(VardefArgIter<'a, LSPLineCol, STeXAnnot>),
}
impl<'a> From<std::slice::Iter<'a, STeXAnnot>> for AnnotIter<'a> {
    #[inline]
    fn from(v: std::slice::Iter<'a, STeXAnnot>) -> Self {
        Self::Slice(v)
    }
}
impl<'a> Iterator for AnnotIter<'a> {
    type Item = &'a STeXAnnot;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::SRef(i) => i.next(),
            Self::Module(i) => i.next(),
            Self::Structure(i) => i.next(),
            Self::InlineAss(i) => i.next(),
            Self::Paragraph(i) => i.next(),
            Self::Problem(i) => i.next(),
            Self::Symdecl(i) => i.next(),
            Self::TextSymdecl(i) => i.next(),
            Self::Notation(i) => i.next(),
            Self::Symdef(i) => i.next(),
            Self::Vardef(i) => i.next(),
            Self::Slice(i) => i.next(),
        }
    }
}

impl TreeLike for STeXAnnot {
    type Child<'a> = &'a Self;
    type RefIter<'a> = AnnotIter<'a>;
    fn children(&self) -> Option<Self::RefIter<'_>> {
        match self {
            Self::Module { opts, children, .. } => Some(AnnotIter::Module(
                SModuleArgIter::new(opts).chain(children.iter()),
            )),
            Self::InlineMorphism { assignments, .. } => {
                Some(AnnotIter::InlineAss(InlineMorphAssIter::new(assignments)))
            }
            Self::SRef {
                opt_args,
                in_opt_args,
                ..
            } => Some(AnnotIter::SRef(
                SRefOptsAIter::new(opt_args).chain(SRefOptsBIter::new(in_opt_args)),
            )),
            Self::Paragraph {
                parsed_args,
                children,
                ..
            }
            | Self::InlineParagraph {
                parsed_args,
                children,
                ..
            } => Some(AnnotIter::Paragraph(
                ParagraphArgIter::new(parsed_args).chain(children.iter()),
            )),
            Self::Problem {
                parsed_args,
                children,
                ..
            } => Some(AnnotIter::Problem(
                ProblemArgIter::new(parsed_args).chain(children.iter()),
            )),
            Self::Symdecl { parsed_args, .. } => {
                Some(AnnotIter::Symdecl(SymdeclArgIter::new(parsed_args)))
            }
            Self::TextSymdecl { parsed_args, .. } => {
                Some(AnnotIter::TextSymdecl(TextSymdeclArgIter::new(parsed_args)))
            }
            Self::Notation { notation_args, .. } => {
                Some(AnnotIter::Notation(NotationArgIter::new(notation_args)))
            }
            Self::Symdef { parsed_args, .. } => {
                Some(AnnotIter::Symdef(SymdefArgIter::new(parsed_args)))
            }
            Self::Vardef { parsed_args, .. } | Self::Varseq { parsed_args, .. } => {
                Some(AnnotIter::Vardef(VardefArgIter::new(parsed_args)))
            }
            Self::MathStructure { children, opts, .. } => Some(AnnotIter::Structure(
                MathStructureArgIter::new(opts).chain(children.iter()),
            )),
            Self::ConservativeExt { children, .. } | Self::MorphismEnv { children, .. } => {
                Some(AnnotIter::Slice(children.iter()))
            }
            Self::SemanticMacro { .. }
            | Self::VariableMacro { .. }
            | Self::ImportModule { .. }
            | Self::UseModule { .. }
            | Self::SetMetatheory { .. }
            | Self::Inputref { .. }
            | Self::MHInput { .. }
            | Self::SymName { .. }
            | Self::Symref { .. }
            | Self::Symuse { .. }
            | Self::Svar { .. }
            | Self::Definiens { .. }
            | Self::Defnotation { .. }
            | Self::UseStructure { .. }
            | Self::Precondition { .. }
            | Self::Objective { .. }
            | Self::RenameDecl { .. }
            | Self::IncludeProblem { .. }
            | Self::MHGraphics { .. }
            | Self::Assign { .. }
            | Self::SnifySuggestion { .. } => None,
        }
    }
}

impl TreeChild<STeXAnnot> for &STeXAnnot {
    fn children<'a>(&self) -> Option<AnnotIter<'a>>
    where
        Self: 'a,
    {
        <STeXAnnot as TreeLike>::children(self)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(PartialEq, Eq, Debug)]
pub struct STeXDiagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub range: StringRange<LSPLineCol>,
}

#[must_use]
pub fn quickparse<'a, S: STeXModuleStore>(
    uri: &'a DocumentUri,
    source: &'a str,
    path: &'a Path,
    backend: &'a AnyBackend,
    store: S,
) -> STeXParseDataI {
    let mut diagnostics = VecSet::new();
    let mut modules = SmallVec::new();
    let mut err = |message, range, level| {
        diagnostics.insert(STeXDiagnostic {
            level,
            message,
            range,
        });
    };
    let mut parser = if S::FULL {
        LaTeXParser::with_rules(
            source,
            STeXParseState::new(Some(uri.archive_uri()), Some(path), uri, backend, store),
            &mut err,
            LaTeXParser::default_rules()
                .into_iter()
                .chain(rules::all_rules()),
            LaTeXParser::default_env_rules()
                .into_iter()
                .chain(rules::all_env_rules()),
        )
    } else {
        LaTeXParser::with_rules(
            source,
            STeXParseState::new(Some(uri.archive_uri()), Some(path), uri, backend, store),
            &mut err,
            LaTeXParser::default_rules()
                .into_iter()
                .chain(rules::declarative_rules()),
            LaTeXParser::default_env_rules()
                .into_iter()
                .chain(rules::declarative_env_rules()),
        )
    };

    let annotations = STeXAnnot::from_tokens(&mut parser, Some(&mut modules));

    let dependents = parser.state.dependencies;
    STeXParseDataI {
        annotations,
        diagnostics,
        check: None,
        modules,
        dependencies: dependents,
    }
}
