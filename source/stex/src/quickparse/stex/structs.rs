use super::{
    DiagnosticLevel, STeXParseData,
    rules::{
        MathStructureArg, NotationArg, ParagraphArg, ProblemArg, SModuleArg, SymdeclArg, SymdefArg,
        TextSymdeclArg, VardefArg,
    },
};
use crate::quickparse::{
    latex::{
        Environment, FromLaTeXToken, Group, GroupState, Groups, LaTeXParser, Macro, ParserState,
        rules::{AnyEnv, AnyMacro, DynMacro},
    },
    stex::rules::{IncludeProblemArg, MHGraphicsArg, SRefOptsA, SRefOptsB},
};
use flams_math_archives::{
    MathArchive,
    backend::{AnyBackend, LocalBackend},
};
use flams_utils::{
    id_counters::IdCounter,
    impossible,
    prelude::HMap,
    sourcerefs::{LSPLineCol, StringPosition, StringRange},
    vecmap::{VecMap, VecSet},
};
use ftml_ontology::narrative::elements::{paragraphs::ParagraphKind, problems::CognitiveDimension};
use ftml_uris::{
    ArchiveId, ArchiveUri, DocumentElementUri, DocumentUri, DomainUri, IsDomainUri, Language,
    ModuleUri, PathUri, SymbolUri, UriName, UriPath, UriWithArchive, UriWithPath,
};
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    collections::hash_map::Entry,
    fmt::Write,
    path::{Path, PathBuf},
};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, serde::Serialize)]
pub enum STeXToken<Pos: StringPosition> {
    ImportModule {
        archive_range: Option<StringRange<Pos>>,
        path_range: StringRange<Pos>,
        module: ModuleReference,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    UseModule {
        archive_range: Option<StringRange<Pos>>,
        path_range: StringRange<Pos>,
        module: ModuleReference,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    UseStructure {
        structure: SymbolReference<Pos>,
        structure_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    SetMetatheory {
        archive_range: Option<StringRange<Pos>>,
        path_range: StringRange<Pos>,
        module: ModuleReference,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    Inputref {
        archive: Option<(ArchiveId, StringRange<Pos>)>,
        filepath: (std::sync::Arc<str>, StringRange<Pos>),
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    SRef {
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        opt_args: Vec<SRefOptsA<Pos, Self>>,
        label_range: StringRange<Pos>,
        in_opt_args: Vec<SRefOptsB<Pos, Self>>,
        target: DocumentElementUri,
        target_path: std::sync::Arc<Path>,
        in_doc: Option<(DocumentUri, std::sync::Arc<Path>)>,
    },
    IncludeProblem {
        filepath: (std::sync::Arc<str>, StringRange<Pos>),
        archive: Option<(ArchiveId, StringRange<Pos>)>,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        args: Vec<IncludeProblemArg<Pos>>,
    },
    MHGraphics {
        filepath: (std::sync::Arc<str>, StringRange<Pos>),
        archive: Option<(ArchiveId, StringRange<Pos>)>,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        args: Vec<MHGraphicsArg<Pos>>,
    },
    MHInput {
        archive: Option<(ArchiveId, StringRange<Pos>)>,
        filepath: (std::sync::Arc<str>, StringRange<Pos>),
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    Module {
        uri: ModuleUri,
        rules: ModuleRules<Pos>,
        name_range: StringRange<Pos>,
        opts: Vec<SModuleArg<Pos, Self>>,
        sig: Option<Language>,
        meta_theory: Option<ModuleReference>,
        full_range: StringRange<Pos>,
        children: Vec<Self>,
        smodule_range: StringRange<Pos>,
    },
    MathStructure {
        uri: SymbolReference<Pos>,
        extends: Vec<(SymbolReference<Pos>, StringRange<Pos>)>,
        name_range: StringRange<Pos>,
        opts: Vec<MathStructureArg<Pos, Self>>,
        full_range: StringRange<Pos>,
        children: Vec<Self>,
        mathstructure_range: StringRange<Pos>,
    },
    ConservativeExt {
        uri: SymbolReference<Pos>,
        ext_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
        children: Vec<Self>,
        extstructure_range: StringRange<Pos>,
    },
    MorphismEnv {
        full_range: StringRange<Pos>,
        env_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
        uri: SymbolUri,
        star: bool,
        domain: ModuleOrStruct<Pos>,
        domain_range: StringRange<Pos>,
        kind: MorphismKind,
        children: Vec<Self>,
    },
    InlineMorphism {
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
        uri: SymbolUri,
        star: bool,
        domain: ModuleOrStruct<Pos>,
        domain_range: StringRange<Pos>,
        kind: MorphismKind,
        assignments: Vec<InlineMorphAssign<Pos, Self>>,
    },
    Paragraph {
        kind: ParagraphKind,
        full_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
        symbol: Option<SymbolReference<Pos>>,
        parsed_args: Vec<ParagraphArg<Pos, Self>>,
        children: Vec<Self>,
    },
    Problem {
        sub: bool,
        full_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
        parsed_args: Vec<ProblemArg<Pos, Self>>,
        children: Vec<Self>,
    },
    InlineParagraph {
        kind: ParagraphKind,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        symbol: Option<SymbolReference<Pos>>,
        parsed_args: Vec<ParagraphArg<Pos, Self>>,
        children: Vec<Self>,
        children_range: StringRange<Pos>,
    },
    #[allow(clippy::type_complexity)]
    Symdecl {
        uri: SymbolReference<Pos>,
        main_name_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
        parsed_args: Vec<SymdeclArg<Pos, Self>>,
        token_range: StringRange<Pos>,
    },
    #[allow(clippy::type_complexity)]
    TextSymdecl {
        uri: SymbolReference<Pos>,
        main_name_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
        parsed_args: Vec<TextSymdeclArg<Pos, Self>>,
        token_range: StringRange<Pos>,
    },
    Notation {
        uri: SmallVec<SymbolReference<Pos>, 1>,
        token_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
        notation_args: Vec<NotationArg<Pos, Self>>,
        full_range: StringRange<Pos>,
    },
    RenameDecl {
        uri: SymbolReference<Pos>,
        token_range: StringRange<Pos>,
        orig_range: StringRange<Pos>,
        name_range: Option<StringRange<Pos>>,
        macroname_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
    },
    Assign {
        uri: SymbolReference<Pos>,
        token_range: StringRange<Pos>,
        orig_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
    },
    #[allow(clippy::type_complexity)]
    Symdef {
        uri: SymbolReference<Pos>,
        main_name_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
        parsed_args: Vec<SymdefArg<Pos, Self>>,
        token_range: StringRange<Pos>,
    },
    #[allow(clippy::type_complexity)]
    Vardef {
        name: UriName,
        main_name_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
        parsed_args: Vec<VardefArg<Pos, Self>>,
        token_range: StringRange<Pos>,
    },
    #[allow(clippy::type_complexity)]
    Varseq {
        name: UriName,
        main_name_range: StringRange<Pos>,
        full_range: StringRange<Pos>,
        parsed_args: Vec<VardefArg<Pos, Self>>,
        token_range: StringRange<Pos>,
    },
    SemanticMacro {
        uri: SymbolReference<Pos>,
        argnum: u8,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    VariableMacro {
        name: UriName,
        orig: StringRange<Pos>,
        argnum: u8,
        sequence: bool,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
    },
    SymName {
        uri: SmallVec<SymbolReference<Pos>, 1>,
        is_def: bool,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
        mode: SymnameMode<Pos>,
    },
    Symuse {
        uri: SmallVec<SymbolReference<Pos>, 1>,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
    },
    Definiens {
        uri: SmallVec<SymbolReference<Pos>, 1>,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        name_range: Option<StringRange<Pos>>,
    },
    Defnotation {
        full_range: StringRange<Pos>,
    },
    Svar {
        name: UriName,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        name_range: Option<StringRange<Pos>>,
        arg_range: StringRange<Pos>,
    },
    Symref {
        uri: SmallVec<SymbolReference<Pos>, 1>,
        is_def: bool,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        name_range: StringRange<Pos>,
        text: (StringRange<Pos>, Vec<Self>),
    },
    Precondition {
        uri: SmallVec<SymbolReference<Pos>, 1>,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        dim_range: StringRange<Pos>,
        symbol_range: StringRange<Pos>,
        dim: CognitiveDimension,
    },
    Objective {
        uri: SmallVec<SymbolReference<Pos>, 1>,
        full_range: StringRange<Pos>,
        token_range: StringRange<Pos>,
        dim_range: StringRange<Pos>,
        symbol_range: StringRange<Pos>,
        dim: CognitiveDimension,
    },
    SnifySuggestion {
        range: StringRange<Pos>,
        symbols: SmallVec<(SymbolUri, bool), 1>,
    },
    Vec(Vec<Self>),
}

impl<'a, P: StringPosition> FromLaTeXToken<'a, P> for STeXToken<P> {
    fn from_comment(_: StringRange<P>) -> Option<Self> {
        None
    }
    fn from_group(_: StringRange<P>, v: Vec<Self>) -> Option<Self> {
        Some(Self::Vec(v))
    }
    fn from_math(_: bool, _: StringRange<P>, v: Vec<Self>) -> Option<Self> {
        Some(Self::Vec(v))
    }
    fn from_control_sequence(_: P, _: &'a str) -> Option<Self> {
        None
    }
    fn from_text(_: StringRange<P>, _: &'a str) -> Option<Self> {
        None
    }
    fn from_macro_application(_: Macro<'a, P>) -> Option<Self> {
        None
    }
    fn from_environment(e: Environment<'a, P, Self>) -> Option<Self> {
        Some(Self::Vec(e.children))
    }
}

#[derive(Copy, Clone, Debug, serde::Serialize)]
pub enum MorphismKind {
    CopyModule,
    InterpretModule,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SymnameMode<Pos: StringPosition> {
    Cap {
        post: Option<(StringRange<Pos>, StringRange<Pos>, String)>,
    },
    PostS {
        pre: Option<(StringRange<Pos>, StringRange<Pos>, String)>,
    },
    CapAndPostS,
    PrePost {
        pre: Option<(StringRange<Pos>, StringRange<Pos>, String)>,
        post: Option<(StringRange<Pos>, StringRange<Pos>, String)>,
    },
}
impl<Pos: StringPosition> SymnameMode<Pos> {
    pub fn apply<'s>(&'s self, s: &'s str) -> impl std::fmt::Display + 's {
        struct Disp<'s, Pos: StringPosition> {
            sn: &'s SymnameMode<Pos>,
            txt: &'s str,
        }
        impl<Pos: StringPosition> std::fmt::Display for Disp<'_, Pos> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.sn {
                    SymnameMode::Cap {
                        post: Some((_, _, post)),
                    } => {
                        if self.txt.is_empty() {
                            return post.fmt(f);
                        }
                        // SAFETY: !self.txt.is_empty()
                        let cap = unsafe { self.txt.chars().next().unwrap_unchecked() };
                        for c in cap.to_uppercase() {
                            f.write_char(c)?;
                        }
                        write!(f, "{}{post}", &self.txt[cap.len_utf8()..])
                    }
                    SymnameMode::Cap { .. } => {
                        if self.txt.is_empty() {
                            return Ok(());
                        }
                        // SAFETY: !self.txt.is_empty()
                        let cap = unsafe { self.txt.chars().next().unwrap_unchecked() };
                        for c in cap.to_uppercase() {
                            f.write_char(c)?;
                        }
                        self.txt[cap.len_utf8()..].fmt(f)
                    }
                    SymnameMode::PostS {
                        pre: Some((_, _, pre)),
                    } => write!(f, "{pre}{}s", self.txt),
                    SymnameMode::PostS { .. } => write!(f, "{}s", self.txt),
                    SymnameMode::CapAndPostS => {
                        if self.txt.is_empty() {
                            return Ok(());
                        }
                        // SAFETY: !self.txt.is_empty()
                        let cap = unsafe { self.txt.chars().next().unwrap_unchecked() };
                        for c in cap.to_uppercase() {
                            f.write_char(c)?;
                        }
                        write!(f, "{}s", &self.txt[cap.len_utf8()..])
                    }
                    SymnameMode::PrePost {
                        pre: Some((_, _, pre)),
                        post: Some((_, _, post)),
                    } => write!(f, "{pre}{}{post}", self.txt),
                    SymnameMode::PrePost {
                        pre: Some((_, _, pre)),
                        ..
                    } => write!(f, "{pre}{}", self.txt),
                    SymnameMode::PrePost {
                        post: Some((_, _, post)),
                        ..
                    } => write!(f, "{}{post}", self.txt),
                    _ => self.txt.fmt(f),
                }
            }
        }
        Disp { sn: self, txt: s }
    }
    pub fn make_cow<'s>(&self, s: &'s str) -> Cow<'s, str> {
        match self {
            Self::PrePost {
                pre: None,
                post: None,
            } => Cow::Borrowed(s),
            _ => Cow::Owned(self.apply(s).to_string()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InlineMorphAssign<Pos: StringPosition, T> {
    pub symbol: SymbolReference<Pos>,
    pub symbol_range: StringRange<Pos>,
    pub first: Option<(Pos, InlineMorphAssKind<Pos, T>)>,
    pub second: Option<(Pos, InlineMorphAssKind<Pos, T>)>,
}

impl<Pos: StringPosition, T1> InlineMorphAssign<Pos, T1> {
    pub fn into_other<T2>(
        self,
        mut cont: impl FnMut(Vec<T1>) -> Vec<T2>,
    ) -> InlineMorphAssign<Pos, T2> {
        let Self {
            symbol,
            symbol_range,
            first,
            second,
        } = self;
        InlineMorphAssign {
            symbol,
            symbol_range,
            first: first.map(|(p, k)| {
                (
                    p,
                    match k {
                        InlineMorphAssKind::Rename(a, b, c) => InlineMorphAssKind::Rename(a, b, c),
                        InlineMorphAssKind::Df(v) => InlineMorphAssKind::Df(cont(v)),
                    },
                )
            }),
            second: second.map(|(p, k)| {
                (
                    p,
                    match k {
                        InlineMorphAssKind::Rename(a, b, c) => InlineMorphAssKind::Rename(a, b, c),
                        InlineMorphAssKind::Df(v) => InlineMorphAssKind::Df(cont(v)),
                    },
                )
            }),
        }
    }
}

pub struct InlineMorphAssIter<'a, Pos: StringPosition, T>(
    std::slice::Iter<'a, InlineMorphAssign<Pos, T>>,
    Option<std::slice::Iter<'a, T>>,
);
impl<'a, Pos: StringPosition, T> InlineMorphAssIter<'a, Pos, T> {
    pub fn new(v: &'a [InlineMorphAssign<Pos, T>]) -> Self {
        Self(v.iter(), None)
    }
}
impl<'a, Pos: StringPosition, T> Iterator for InlineMorphAssIter<'a, Pos, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(n) = &mut self.1 {
                if let Some(n) = n.next() {
                    return Some(n);
                }
            }
            if let Some(a) = self.0.next() {
                if let Some((_, InlineMorphAssKind::Df(v))) = &a.first {
                    self.1 = Some(v.iter());
                    continue;
                }
                if let Some((_, InlineMorphAssKind::Df(v))) = &a.second {
                    self.1 = Some(v.iter());
                }
            } else {
                return None;
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum InlineMorphAssKind<Pos: StringPosition, T> {
    Df(Vec<T>),
    Rename(
        Option<(UriName, StringRange<Pos>)>,
        Box<str>,
        StringRange<Pos>,
    ),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolReference<Pos: StringPosition> {
    pub uri: SymbolUri,
    pub filepath: Option<std::sync::Arc<Path>>,
    pub range: StringRange<Pos>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleReference {
    pub uri: ModuleUri,
    pub in_doc: DocumentUri,
    pub rel_path: Option<std::sync::Arc<str>>,
    pub full_path: Option<std::sync::Arc<Path>>,
}
impl ModuleReference {
    /*
    #[must_use]
    pub fn doc_uri(&self) -> Option<DocumentUri> {
      let rel_path = &**self.rel_path.as_ref()?;
      let (path,name) = rel_path.rsplit_once('/').map_or_else(
        || (None,rel_path),
        |(path,name)| (Some(path),name)
      );
      let path = path.map_or_else(
        || Ok(self.uri.archive_uri().owned().into()),
        |path| self.uri.archive_uri().owned() % path
      ).ok()?;
      let (name,language) = name.rsplit_once('.')
        .map_or((name,Language::default()), |(name,l)| (name,l.parse().unwrap_or_default()));
      let name = if name.ends_with(Into::<&str>::into(language)) && name.len() > 3 {
        &name[..name.len() - 3]
      } else {name};
      (path & (name,language)).ok()
    }
     */
}

pub enum GetModuleError {
    NotFound(ModuleUri),
    Cycle(Vec<DocumentUri>),
}
impl std::fmt::Display for GetModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(uri) => write!(f, "module not found: {uri}"),
            Self::Cycle(cycle) => write!(
                f,
                "cycle in module dependencies: {}",
                cycle
                    .iter()
                    .map(DocumentUri::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
        }
    }
}

#[allow(unused_variables)]
pub trait STeXModuleStore {
    const FULL: bool;
    /// # Errors
    fn get_module(
        &mut self,
        module: &ModuleReference,
        in_path: Option<&std::sync::Arc<Path>>,
    ) -> Result<STeXParseData, GetModuleError>;
    #[inline]
    fn add_text<Pos: StringPosition>(
        &self,
        r: StringRange<Pos>,
        text: &str,
        language: Language,
        needs_usemodule: &dyn Fn(&SymbolUri) -> bool,
    ) -> Option<STeXToken<Pos>> {
        None
    }
    fn add_verbalization(&mut self, s: &str, symbol: &SymbolUri, language: Language) {}
}
impl STeXModuleStore for () {
    const FULL: bool = false;
    #[inline]
    fn get_module(
        &mut self,
        r: &ModuleReference,
        _: Option<&std::sync::Arc<Path>>,
    ) -> Result<STeXParseData, GetModuleError> {
        Err(GetModuleError::NotFound(r.uri.clone()))
    }
}

#[derive(Debug, serde::Serialize)]
pub enum ModuleRule<Pos: StringPosition> {
    Import(ModuleReference),
    Symbol(SymbolRule<Pos>),
    Structure {
        symbol: SymbolRule<Pos>,
        //reference:ModuleReference,
        rules: ModuleRules<Pos>,
    },
    ConservativeExt(SymbolReference<Pos>, ModuleRules<Pos>),
    StructureImport(SymbolReference<Pos>),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolRule<Pos: StringPosition> {
    pub uri: SymbolReference<Pos>,
    pub macroname: Option<std::sync::Arc<str>>,
    pub has_tp: bool,
    pub has_df: bool,
    pub argnum: u8,
}
impl<Pos: StringPosition> SymbolRule<Pos> {
    fn as_rule<'a, MS: STeXModuleStore>(
        &self,
    ) -> Option<(
        Cow<'a, str>,
        AnyMacro<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>>,
    )> {
        self.macroname.as_ref().map(|m| {
            (
                m.to_string().into(),
                AnyMacro::Ext(DynMacro {
                    ptr: super::rules::semantic_macro as _,
                    arg: MacroArg::Symbol(self.uri.clone(), self.argnum),
                }),
            )
        })
    }
}
impl<Pos: StringPosition> Eq for SymbolReference<Pos> {}
impl<Pos: StringPosition> PartialEq for SymbolReference<Pos> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleRules<Pos: StringPosition> {
    pub rules: std::sync::Arc<[ModuleRule<Pos>]>,
}
impl<Pos: StringPosition> Default for ModuleRules<Pos> {
    #[inline]
    fn default() -> Self {
        Self {
            rules: std::sync::Arc::new([]),
        }
    }
}

pub struct STeXParseState<'a, Pos: StringPosition, MS: STeXModuleStore> {
    pub(super) archive: Option<&'a ArchiveUri>,
    pub(super) in_path: Option<std::sync::Arc<Path>>,
    pub(super) doc_uri: &'a DocumentUri,
    pub(super) backend: &'a AnyBackend,
    pub(super) language: Language,
    pub(super) dependencies: Vec<std::sync::Arc<Path>>,
    pub(super) modules: SmallVec<(ModuleUri, ModuleRules<Pos>), 1>,
    pub(super) module_store: MS,
    name_counter: IdCounter,
}
impl<'a, MS: STeXModuleStore> STeXParseState<'a, LSPLineCol, MS> {
    fn load_module(
        &mut self,
        module: &ModuleReference,
    ) -> Result<ModuleRules<LSPLineCol>, GetModuleError> {
        for (uri, m) in &self.modules {
            if *uri == module.uri {
                return Ok(m.clone());
            }
        }
        /*if let Some(fp) = &module.full_path {
          self.dependencies.push(fp.clone());
        }*/
        match self.module_store.get_module(module, self.in_path.as_ref()) {
            Ok(d) => {
                for (uri, m) in &d.lock().modules {
                    if *uri == module.uri {
                        return Ok(m.clone());
                    }
                }
                Err(GetModuleError::NotFound(module.uri.clone()))
            }
            Err(e) => Err(e),
        }
    }

    fn load_rules(
        mod_ref: ModuleReference,
        irules: ModuleRules<LSPLineCol>,
        prev: &[STeXGroup<'a, MS, LSPLineCol>],
        current: &mut HMap<Cow<'a, str>, AnyMacro<'a, LSPLineCol, STeXToken<LSPLineCol>, Self>>,
        changes: &mut HMap<
            Cow<'a, str>,
            Option<AnyMacro<'a, LSPLineCol, STeXToken<LSPLineCol>, Self>>,
        >,
        semantic_rules: &mut Vec<SemanticRule<LSPLineCol>>,
        f: &mut impl FnMut(&ModuleReference) -> Option<ModuleRules<LSPLineCol>>,
        cycles_count: u16,
    ) -> Result<(), ()> {
        if cycles_count >= 500 {
            return Err(());
        }
        if Self::has_module(prev, semantic_rules, &mod_ref) {
            return Ok(());
        }
        for rule in irules.rules.iter() {
            match rule {
                ModuleRule::Import(m) => {
                    if let Some(rls) = f(m) {
                        Self::load_rules(
                            m.clone(),
                            rls.clone(),
                            prev,
                            current,
                            changes,
                            semantic_rules,
                            f,
                            cycles_count + 1,
                        )?;
                    }
                }
                ModuleRule::Symbol(rule) if MS::FULL => {
                    //symbols.push(rule.clone());
                    if let Some((name, rule)) = rule.as_rule() {
                        let old = current.insert(name.clone(), rule);
                        if let Entry::Vacant(e) = changes.entry(name) {
                            e.insert(old);
                        }
                    }
                }
                ModuleRule::Structure { symbol, rules } => {
                    semantic_rules.push(SemanticRule::Structure {
                        symbol: symbol.clone(),
                        rules: rules.clone(),
                    });
                    if MS::FULL {
                        if let Some((name, rule)) = symbol.as_rule() {
                            let old = current.insert(name.clone(), rule);
                            if let Entry::Vacant(e) = changes.entry(name) {
                                e.insert(old);
                            }
                        }
                    }
                }
                ModuleRule::ConservativeExt(s, rls) => {
                    semantic_rules.push(SemanticRule::ConservativeExt(s.clone(), rls.clone()));
                }
                _ => (),
            }
        }
        semantic_rules.push(SemanticRule::Module(mod_ref, irules));
        Ok(())
    }

    fn has_module(
        prev: &[STeXGroup<'a, MS, LSPLineCol>],
        current: &Vec<SemanticRule<LSPLineCol>>,
        mod_ref: &ModuleReference,
    ) -> bool {
        if current
            .iter()
            .any(|e| matches!(e,SemanticRule::Module(r,_) if r.uri == mod_ref.uri))
        {
            return true;
        }
        for p in prev.iter().rev() {
            if matches!(&p.kind,GroupKind::Module { uri, .. } if *uri == mod_ref.uri) {
                return true;
            }
            if p.semantic_rules
                .iter()
                .any(|e| matches!(e,SemanticRule::Module(r,_) if r.uri == mod_ref.uri))
            {
                return true;
            }
        }
        false
    }

    /// # Panics
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_use(
        &mut self,
        module: &ModuleReference,
        groups: Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        range: StringRange<LSPLineCol>,
    ) {
        let groups_ls = &mut **groups.groups;
        assert!(!groups_ls.is_empty());
        let i = groups_ls.len() - 1;
        let (prev, after) = groups_ls.split_at_mut(i);
        let prev = &*prev;
        let g = &mut after[0];
        match self.load_module(module) {
            Ok(irules) => {
                if Self::load_rules(
                    module.clone(),
                    irules,
                    prev,
                    groups.rules,
                    &mut g.inner.macro_rule_changes,
                    &mut g.semantic_rules,
                    &mut |m| match self.load_module(m) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            groups
                                .tokenizer
                                .problem(range.start, e, DiagnosticLevel::Error);
                            None
                        }
                    },
                    0,
                )
                .is_err()
                {
                    groups
                        .tokenizer
                        .problem(range.start, "Import cycle", DiagnosticLevel::Error);
                }
            }
            Err(e) => groups
                .tokenizer
                .problem(range.start, e, DiagnosticLevel::Error),
        }
    }

    fn has_structure(
        prev: &[STeXGroup<'a, MS, LSPLineCol>],
        current: &Vec<SemanticRule<LSPLineCol>>,
        sym_ref: &SymbolReference<LSPLineCol>,
    ) -> bool {
        if current
            .iter()
            .any(|e| matches!(e,SemanticRule::StructureImport(r,_) if r.uri == sym_ref.uri))
        {
            return true;
        }
        for p in prev.iter().rev() {
            if p.semantic_rules
                .iter()
                .any(|e| matches!(e,SemanticRule::StructureImport(r,_) if r.uri == sym_ref.uri))
            {
                return true;
            }
        }
        false
    }
    fn load_structure(
        symbol: &SymbolReference<LSPLineCol>,
        prev: &[STeXGroup<'a, MS, LSPLineCol>],
        semantic_rules: &Vec<SemanticRule<LSPLineCol>>,
    ) -> Option<ModuleRules<LSPLineCol>> {
        for r in semantic_rules.iter().rev() {
            match r {
                SemanticRule::Structure {
                    symbol: isymbol,
                    rules,
                    ..
                } if isymbol.uri.uri == symbol.uri => return Some(rules.clone()),
                _ => (),
            }
        }
        for g in prev.iter().rev() {
            for r in g.semantic_rules.iter().rev() {
                match r {
                    SemanticRule::Structure {
                        symbol: isymbol,
                        rules,
                        ..
                    } if isymbol.uri.uri == symbol.uri => return Some(rules.clone()),
                    _ => (),
                }
            }
        }
        None
    }

    fn load_structure_rules(
        symbol: SymbolReference<LSPLineCol>,
        irules: ModuleRules<LSPLineCol>,
        prev: &[STeXGroup<'a, MS, LSPLineCol>],
        current: &mut HMap<Cow<'a, str>, AnyMacro<'a, LSPLineCol, STeXToken<LSPLineCol>, Self>>,
        changes: &mut HMap<
            Cow<'a, str>,
            Option<AnyMacro<'a, LSPLineCol, STeXToken<LSPLineCol>, Self>>,
        >,
        semantic_rules: &mut Vec<SemanticRule<LSPLineCol>>,
    ) {
        macro_rules! do_rule {
            ($rule:ident) => {
                match $rule {
                    ModuleRule::StructureImport(m)
                        if !Self::has_structure(prev, semantic_rules, &symbol) =>
                    {
                        if let Some(rls) = Self::load_structure(m, prev, semantic_rules) {
                            Self::load_structure_rules(
                                m.clone(),
                                rls,
                                prev,
                                current,
                                changes,
                                semantic_rules,
                            );
                        }
                    }
                    ModuleRule::Symbol(rule) if MS::FULL => {
                        //symbols.push(rule.clone());
                        if let Some((name, rule)) = rule.as_rule() {
                            let old = current.insert(name.clone(), rule);
                            if let Entry::Vacant(e) = changes.entry(name) {
                                e.insert(old);
                            }
                        }
                    }
                    ModuleRule::Structure { symbol, rules } => {
                        semantic_rules.push(SemanticRule::Structure {
                            symbol: symbol.clone(),
                            rules: rules.clone(),
                        });
                        if MS::FULL {
                            if let Some((name, rule)) = symbol.as_rule() {
                                let old = current.insert(name.clone(), rule);
                                if let Entry::Vacant(e) = changes.entry(name) {
                                    e.insert(old);
                                }
                            }
                        }
                    }
                    _ => (),
                }
            };
        }
        for rule in irules.rules.iter() {
            do_rule!(rule);
        }
        for g in prev.iter().rev() {
            for rule in g.semantic_rules.iter().rev() {
                if let SemanticRule::ConservativeExt(s, rls) = rule {
                    //tracing::info!("Checking {} vs {}",s.uri,symbol.uri);
                    if s.uri == symbol.uri {
                        for rule in rls.rules.iter() {
                            do_rule!(rule);
                        }
                    }
                }
            }
        }
        semantic_rules.push(SemanticRule::StructureImport(symbol, irules));
    }

    pub fn import_structure(
        &mut self,
        symbol: &SymbolReference<LSPLineCol>,
        srules: &ModuleRules<LSPLineCol>,
        groups: &mut Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        range: StringRange<LSPLineCol>,
    ) {
        let groups_ls = &mut **groups.groups;
        let Some(i) = groups_ls.iter().enumerate().rev().find_map(|(i, g)| {
            if matches!(
                &g.kind,
                GroupKind::Module { .. } | GroupKind::MathStructure { .. }
            ) {
                Some(i)
            } else {
                None
            }
        }) else {
            groups.tokenizer.problem(
                range.start,
                "\\importmodule is only allowed in a module".to_string(),
                DiagnosticLevel::Error,
            );
            return;
        };
        let (prev, after) = groups_ls.split_at_mut(i);
        let prev = &*prev;
        let g = &mut after[0];
        let (GroupKind::Module { rules, .. } | GroupKind::MathStructure { rules, .. }) =
            &mut g.kind
        else {
            impossible!()
        };
        if rules
            .iter()
            .any(|r| matches!(r,ModuleRule::StructureImport(s) if s.uri == symbol.uri))
        {
            return;
        }
        rules.push(ModuleRule::StructureImport(symbol.clone()));
        if !Self::has_structure(prev, &g.semantic_rules, &symbol) {
            // if MS::FULL {
            Self::load_structure_rules(
                symbol.clone(),
                srules.clone(),
                prev,
                groups.rules,
                &mut g.inner.macro_rule_changes,
                &mut g.semantic_rules,
            );
            // }
        }
    }

    pub fn use_structure(
        &mut self,
        symbol: &SymbolReference<LSPLineCol>,
        srules: &ModuleRules<LSPLineCol>,
        groups: &mut Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        _range: StringRange<LSPLineCol>,
    ) {
        let groups_ls = &mut **groups.groups;
        let i = groups_ls.len() - 1;
        let (prev, after) = groups_ls.split_at_mut(i);
        let prev = &*prev;
        let g = &mut after[0];
        if !Self::has_structure(prev, &g.semantic_rules, &symbol) {
            // if MS::FULL {
            Self::load_structure_rules(
                symbol.clone(),
                srules.clone(),
                prev,
                groups.rules,
                &mut g.inner.macro_rule_changes,
                &mut g.semantic_rules,
            );
            // }
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn add_import(
        &mut self,
        module: &ModuleReference,
        groups: Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        range: StringRange<LSPLineCol>,
    ) {
        let groups_ls = &mut **groups.groups;
        let Some(i) = groups_ls.iter().enumerate().rev().find_map(|(i, g)| {
            if matches!(
                &g.kind,
                GroupKind::Module { .. } | GroupKind::MathStructure { .. }
            ) {
                Some(i)
            } else {
                None
            }
        }) else {
            groups.tokenizer.problem(
                range.start,
                "\\importmodule is only allowed in a module".to_string(),
                DiagnosticLevel::Error,
            );
            return;
        };
        let (prev, after) = groups_ls.split_at_mut(i);
        let prev = &*prev;
        let g = &mut after[0];
        let (GroupKind::Module { rules, .. } | GroupKind::MathStructure { rules, .. }) =
            &mut g.kind
        else {
            unreachable!()
        };
        if rules
            .iter()
            .any(|r| matches!(r,ModuleRule::Import(m) if m.uri == module.uri))
        {
            return;
        }
        rules.push(ModuleRule::Import(module.clone()));
        match self.load_module(module) {
            Ok(irules) => {
                if Self::load_rules(
                    module.clone(),
                    irules,
                    prev,
                    groups.rules,
                    &mut g.inner.macro_rule_changes,
                    &mut g.semantic_rules,
                    &mut |m| match self.load_module(m) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            groups
                                .tokenizer
                                .problem(range.start, e, DiagnosticLevel::Error);
                            None
                        }
                    },
                    0,
                )
                .is_err()
                {
                    groups
                        .tokenizer
                        .problem(range.start, "Import cycle", DiagnosticLevel::Error);
                }
            }
            Err(e) => groups
                .tokenizer
                .problem(range.start, e, DiagnosticLevel::Error),
        }
    }

    #[allow(clippy::unused_self)]
    fn get_symbol_macro_or_name(
        &self,
        groups: &Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        namestr: &str,
    ) -> Option<SmallVec<SymbolReference<LSPLineCol>, 1>> {
        let mut ret = SmallVec::new();
        for g in groups.groups.iter().rev() {
            for r in g.semantic_rules.iter().rev() {
                match r {
                    SemanticRule::Symbol(r) | SemanticRule::Structure { symbol: r, .. } => {
                        if r.macroname.as_ref().is_some_and(|n| &**n == namestr) {
                            if !ret.contains(&r.uri) {
                                ret.push(r.uri.clone());
                            }
                            continue;
                        }
                        if r.uri.uri.name().last() == namestr {
                            if !ret.contains(&r.uri) {
                                ret.push(r.uri.clone());
                            }
                        }
                    }
                    SemanticRule::Module(_, r) | SemanticRule::StructureImport(_, r) => {
                        for r in r.rules.iter().rev() {
                            match r {
                                ModuleRule::Symbol(r) | ModuleRule::Structure { symbol: r, .. } => {
                                    if r.macroname.as_ref().is_some_and(|n| &**n == namestr) {
                                        if !ret.contains(&r.uri) {
                                            ret.push(r.uri.clone());
                                        }
                                        continue;
                                    }
                                    if r.uri.uri.name().last() == namestr {
                                        if !ret.contains(&r.uri) {
                                            ret.push(r.uri.clone());
                                        }
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    SemanticRule::ConservativeExt(s, rls)
                        if Self::has_structure(&groups.groups, &Vec::new(), s) =>
                    {
                        for r in rls.rules.iter().rev() {
                            match r {
                                ModuleRule::Symbol(r) | ModuleRule::Structure { symbol: r, .. } => {
                                    if r.macroname.as_ref().is_some_and(|n| &**n == namestr) {
                                        if !ret.contains(&r.uri) {
                                            ret.push(r.uri.clone());
                                        }
                                        continue;
                                    }
                                    if r.uri.uri.name().last() == namestr {
                                        if !ret.contains(&r.uri) {
                                            ret.push(r.uri.clone());
                                        }
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    SemanticRule::ConservativeExt(..) => (),
                }
            }
        }
        if ret.is_empty() { None } else { Some(ret) }
    }

    #[allow(clippy::unused_self)]
    fn get_structure_macro_or_name(
        &self,
        groups: &Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        namestr: &str,
    ) -> Option<(SymbolReference<LSPLineCol>, ModuleRules<LSPLineCol>)> {
        for g in groups.groups.iter().rev() {
            for r in g.semantic_rules.iter().rev() {
                match r {
                    SemanticRule::Structure { symbol, rules, .. } => {
                        if symbol.macroname.as_ref().is_some_and(|n| &**n == namestr) {
                            return Some((symbol.uri.clone(), rules.clone()));
                        }
                        if symbol.uri.uri.name().last() == namestr {
                            return Some((symbol.uri.clone(), rules.clone()));
                        }
                    }
                    SemanticRule::Module(_, r) => {
                        for r in r.rules.iter().rev() {
                            match r {
                                ModuleRule::Structure { symbol, rules, .. } => {
                                    if symbol.macroname.as_ref().is_some_and(|n| &**n == namestr) {
                                        return Some((symbol.uri.clone(), rules.clone()));
                                    }
                                    if symbol.uri.uri.name().last() == namestr {
                                        return Some((symbol.uri.clone(), rules.clone()));
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        None
    }

    fn compare(symbol: &str, module: &str, path: Option<&str>, uri: &SymbolUri) -> bool {
        fn compare_names(n1: &str, n2: &UriName) -> Option<bool> {
            let mut symbol_steps = n1.split('/').rev();
            let mut uri_steps = n2.steps().rev();
            loop {
                let Some(sym) = symbol_steps.next() else {
                    return if uri_steps.next().is_some() {
                        None
                    } else {
                        Some(true)
                    };
                };
                let Some(uristep) = uri_steps.next() else {
                    return Some(false);
                };
                if sym != uristep {
                    if symbol_steps.next().is_none() && uristep.ends_with(sym) {
                        return None;
                    }
                    return Some(false);
                }
            }
        }
        if compare_names(symbol, uri.name()) != Some(true) {
            return false;
        }
        match compare_names(module, uri.module_name()) {
            None | Some(true) if path.is_none() => return true,
            Some(false) | None => return false,
            Some(true) => (),
        }
        let Some(mut path) = path else { unreachable!() };
        if let Some(uri_path) = uri.path() {
            for step in uri_path.steps().rev() {
                if path.is_empty() {
                    return true;
                }
                if let Some(p) = path.strip_suffix(step) {
                    if let Some(p) = p.strip_suffix('/') {
                        path = p;
                    } else {
                        if p.is_empty() {
                            return true;
                        }
                    }
                } else {
                    return false;
                }
            }
        }
        let id = uri.archive_id();
        return id.as_ref().ends_with(path);
    }

    #[allow(clippy::unused_self)]
    fn get_symbol_complex(
        &self,
        groups: &Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        symbol: &str,
        module: &str,
        path: Option<&str>,
    ) -> Option<SmallVec<SymbolReference<LSPLineCol>, 1>> {
        let mut ret = SmallVec::new();
        for g in groups.groups.iter().rev() {
            for r in g.semantic_rules.iter().rev() {
                match r {
                    SemanticRule::Symbol(r) | SemanticRule::Structure { symbol: r, .. }
                        if Self::compare(symbol, module, path, &r.uri.uri) =>
                    {
                        if !ret.contains(&r.uri) {
                            ret.push(r.uri.clone());
                        }
                    }
                    SemanticRule::Module(_, r) | SemanticRule::StructureImport(_, r) => {
                        for r in r.rules.iter().rev() {
                            match r {
                                ModuleRule::Symbol(r) | ModuleRule::Structure { symbol: r, .. }
                                    if Self::compare(symbol, module, path, &r.uri.uri) =>
                                {
                                    if !ret.contains(&r.uri) {
                                        ret.push(r.uri.clone());
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    SemanticRule::ConservativeExt(s, rls)
                        if Self::has_structure(&groups.groups, &Vec::new(), s) =>
                    {
                        for r in rls.rules.iter().rev() {
                            match r {
                                ModuleRule::Symbol(r) | ModuleRule::Structure { symbol: r, .. }
                                    if Self::compare(symbol, module, path, &r.uri.uri) =>
                                {
                                    if !ret.contains(&r.uri) {
                                        ret.push(r.uri.clone());
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        if ret.is_empty() { None } else { Some(ret) }
    }

    #[allow(clippy::unused_self)]
    fn get_structure_uri(
        &self,
        groups: &Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        uri: &SymbolReference<LSPLineCol>,
    ) -> Option<ModuleRules<LSPLineCol>> {
        for g in groups.groups.iter().rev() {
            for r in g.semantic_rules.iter().rev() {
                match r {
                    SemanticRule::Structure { symbol, rules, .. } if symbol.uri.uri == uri.uri => {
                        return Some(rules.clone());
                    }
                    SemanticRule::Module(_, r) => {
                        for r in r.rules.iter().rev() {
                            match r {
                                ModuleRule::Structure { symbol, rules, .. }
                                    if symbol.uri.uri == uri.uri =>
                                {
                                    return Some(rules.clone());
                                }
                                _ => (),
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        None
    }

    #[allow(clippy::unused_self)]
    fn get_structure_complex(
        &self,
        groups: &Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        namestr: &str,
        module: &str,
        path: Option<&str>,
    ) -> Option<(SymbolReference<LSPLineCol>, ModuleRules<LSPLineCol>)> {
        for g in groups.groups.iter().rev() {
            for r in g.semantic_rules.iter().rev() {
                match r {
                    SemanticRule::Structure { symbol, rules, .. }
                        if Self::compare(namestr, module, path, &symbol.uri.uri) =>
                    {
                        return Some((symbol.uri.clone(), rules.clone()));
                    }
                    SemanticRule::Module(_, r) => {
                        for r in r.rules.iter().rev() {
                            match r {
                                ModuleRule::Structure { symbol, rules, .. }
                                    if Self::compare(namestr, module, path, &symbol.uri.uri) =>
                                {
                                    return Some((symbol.uri.clone(), rules.clone()));
                                }
                                _ => (),
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        None
    }

    pub fn get_symbol(
        &self,
        start: LSPLineCol,
        groups: &mut Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        namestr: &str,
    ) -> Option<SmallVec<SymbolReference<LSPLineCol>, 1>> {
        //let realname = namestr.trim().split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        let mut steps = namestr.split('?').rev(); //realname.split('?').rev();
        let name = steps.next()?;

        let module = if let Some(module) = steps.next() {
            module
        } else {
            if !name.contains('/') {
                //return self.get_symbol_macro_or_name(groups,name);
                let r = self.get_symbol_macro_or_name(groups, name)?;
                if r.len() > 1 {
                    groups.tokenizer.problem(
                        start,
                        format!("Ambiguous symbol reference: {namestr}"),
                        DiagnosticLevel::Warning,
                    );
                }
                return Some(r);
            }
            ""
        };
        let path = if steps.next().is_none() {
            None
        } else {
            let i = namestr.len() - (name.len() + 1 + module.len() + 1);
            Some(&namestr[..i])
        };
        let r = self.get_symbol_complex(groups, name, module, path)?;
        if r.len() > 1 {
            groups.tokenizer.problem(
                start,
                format!("Ambiguous symbol reference: {namestr}"),
                DiagnosticLevel::Warning,
            );
        }
        Some(r)
    }

    pub fn get_structure(
        &self,
        groups: &Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        namestr: &str,
    ) -> Option<(SymbolReference<LSPLineCol>, ModuleRules<LSPLineCol>)> {
        //let realname = namestr.trim().split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        let mut steps = namestr.split('?').rev(); //realname.split('?').rev();
        let name = steps.next()?;

        let Some(module) = steps.next() else {
            return self.get_structure_macro_or_name(groups, name);
        };
        let path = if steps.next().is_none() {
            None
        } else {
            let i = namestr.len() - (name.len() + 1 + module.len() + 1);
            Some(&namestr[..i])
        };
        self.get_structure_complex(groups, name, module, path)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn resolve_module_or_struct(
        &mut self,
        groups: &Groups<'a, '_, LSPLineCol, STeXToken<LSPLineCol>, Self>,
        module_or_struct: &str,
        archive: Option<ArchiveId>,
    ) -> Option<(ModuleOrStruct<LSPLineCol>, Vec<ModuleRules<LSPLineCol>>)> {
        fn mmatch<'a, MS: STeXModuleStore>(
            slf: &mut STeXParseState<'a, LSPLineCol, MS>,
            groups: &Groups<
                'a,
                '_,
                LSPLineCol,
                STeXToken<LSPLineCol>,
                STeXParseState<'a, LSPLineCol, MS>,
            >,
            rules: &ModuleRules<LSPLineCol>,
            dones: &mut Vec<DomainUri>,
            target: &mut Vec<ModuleRules<LSPLineCol>>,
        ) -> Option<()> {
            for r in rules.rules.iter() {
                match r {
                    ModuleRule::Import(m)
                        if !dones
                            .iter()
                            .any(|u| matches!(u,DomainUri::Module(u) if *u == m.uri)) =>
                    {
                        load_module(slf, groups, m, dones, target)?;
                    }
                    ModuleRule::StructureImport(s)
                        if !dones
                            .iter()
                            .any(|u| matches!(u,DomainUri::Symbol(u) if *u == s.uri)) =>
                    {
                        load_structure(slf, groups, s, dones, target)?;
                    }
                    _ => (),
                }
            }
            Some(())
        }
        fn load_module<'a, MS: STeXModuleStore>(
            slf: &mut STeXParseState<'a, LSPLineCol, MS>,
            groups: &Groups<
                'a,
                '_,
                LSPLineCol,
                STeXToken<LSPLineCol>,
                STeXParseState<'a, LSPLineCol, MS>,
            >,
            module: &ModuleReference,
            dones: &mut Vec<DomainUri>,
            target: &mut Vec<ModuleRules<LSPLineCol>>,
        ) -> Option<()> {
            dones.push(module.uri.clone().into());
            let rls = slf.load_module(module).ok()?;
            mmatch(slf, groups, &rls, dones, target)?;
            target.push(rls);
            Some(())
        }
        fn load_structure<'a, MS: STeXModuleStore>(
            slf: &mut STeXParseState<'a, LSPLineCol, MS>,
            groups: &Groups<
                'a,
                '_,
                LSPLineCol,
                STeXToken<LSPLineCol>,
                STeXParseState<'a, LSPLineCol, MS>,
            >,
            structure: &SymbolReference<LSPLineCol>,
            dones: &mut Vec<DomainUri>,
            target: &mut Vec<ModuleRules<LSPLineCol>>,
        ) -> Option<()> {
            dones.push(structure.uri.clone().into());
            let rls = slf.get_structure_uri(groups, structure)?;
            mmatch(slf, groups, &rls, dones, target)?;
            target.push(rls);
            Some(())
        }
        let mut dones = Vec::new();
        if archive.is_none() {
            if let Some((m, rls)) = self.find_module(module_or_struct) {
                let mut ret = Vec::new();
                let rls = rls.clone();
                let rf = ModuleOrStruct::Module(ModuleReference {
                    uri: m.clone(),
                    in_doc: self.doc_uri.clone(),
                    rel_path: None,
                    full_path: self.in_path.clone(),
                });
                mmatch(self, groups, &rls, &mut dones, &mut ret)?;
                ret.push(rls);
                return Some((rf, ret));
            }
            if let Some((s, r)) = self.get_structure(groups, module_or_struct) {
                let mut ret = Vec::new();
                mmatch(self, groups, &r, &mut dones, &mut ret)?;
                ret.push(r);
                return Some((ModuleOrStruct::Struct(s), ret));
            }
        }
        if let Some(m) = self.resolve_module(module_or_struct, archive) {
            let mut ret = Vec::new();
            load_module(self, groups, &m, &mut dones, &mut ret)?;
            Some((ModuleOrStruct::Module(m), ret))
        } else {
            None
        }
    }
}

impl<'a, Pos: StringPosition, MS: STeXModuleStore> STeXParseState<'a, Pos, MS> {
    #[inline]
    #[must_use]
    pub fn new(
        archive: Option<&'a ArchiveUri>,
        in_path: Option<&'a Path>,
        uri: &'a DocumentUri,
        backend: &'a AnyBackend,
        on_module: MS,
    ) -> Self {
        let language = in_path.map(Language::from).unwrap_or_default();
        Self {
            archive,
            in_path: in_path.map(Into::into),
            doc_uri: uri,
            language,
            backend,
            modules: SmallVec::new(),
            module_store: on_module,
            name_counter: IdCounter::default(),
            dependencies: Vec::new(),
        }
    }

    pub fn set_structure(
        &mut self,
        groups: &mut Groups<'a, '_, Pos, STeXToken<Pos>, Self>,
        rules: ModuleRules<Pos>,
        range: StringRange<Pos>,
    ) {
        for g in groups.groups.iter_mut().rev() {
            match &mut g.kind {
                GroupKind::Module { rules: rls, .. } => match rls.last_mut() {
                    Some(ModuleRule::Structure {
                        symbol,
                        rules: rls1,
                        ..
                    }) => {
                        for sr in g.semantic_rules.iter_mut().rev() {
                            match sr {
                                SemanticRule::Structure {
                                    symbol: symbol2,
                                    rules: rls2,
                                    ..
                                } if symbol.uri.uri == symbol2.uri.uri => {
                                    *rls2 = rules.clone();
                                    break;
                                }
                                _ => (),
                            }
                        }
                        *rls1 = rules;
                        return;
                    }
                    Some(ModuleRule::ConservativeExt(_, rls1)) => {
                        for sr in g.semantic_rules.iter_mut().rev() {
                            match sr {
                                SemanticRule::ConservativeExt(_, rls2) => {
                                    *rls2 = rules.clone();
                                    break;
                                }
                                _ => (),
                            }
                        }
                        *rls1 = rules;
                        return;
                    }
                    _ => {
                        groups.tokenizer.problem(
                            range.start,
                            "mathstructure ended unexpectedly".to_string(),
                            DiagnosticLevel::Error,
                        );
                        return;
                    }
                },
                _ => (),
            }
        }
        groups.tokenizer.problem(
            range.start,
            "mathstructure is only allowed in a module".to_string(),
            DiagnosticLevel::Error,
        );
    }

    pub fn add_structure(
        &mut self,
        groups: &mut Groups<'a, '_, Pos, STeXToken<Pos>, Self>,
        name: UriName,
        macroname: Option<std::sync::Arc<str>>,
        range: StringRange<Pos>,
    ) -> Option<SymbolReference<Pos>> {
        for g in groups.groups.iter_mut().rev() {
            match &mut g.kind {
                GroupKind::Module { uri, rules, .. } => {
                    let suri = uri.clone() | name;
                    let uri = SymbolReference {
                        uri: suri,
                        filepath: self.in_path.clone(),
                        range,
                    };
                    for r in &*rules {
                        match r {
                            ModuleRule::Symbol(s) | ModuleRule::Structure { symbol: s, .. }
                                if s.uri.uri == uri.uri =>
                            {
                                groups.tokenizer.problem(
                                    range.start,
                                    format!("symbol with name {} already exists", s.uri.uri),
                                    DiagnosticLevel::Warning,
                                );
                            }
                            _ => (),
                        }
                    }
                    self.module_store.add_verbalization(
                        uri.uri.name.last(),
                        &uri.uri,
                        Language::English,
                    );
                    let rule = SymbolRule {
                        uri,
                        macroname,
                        has_tp: false,
                        has_df: false,
                        argnum: 0,
                    };
                    if MS::FULL {
                        if let Some((name, rule)) = rule.as_rule() {
                            let old = groups.rules.insert(name.clone(), rule);
                            if let Entry::Vacant(e) = g.inner.macro_rule_changes.entry(name) {
                                e.insert(old);
                            }
                        }
                    }
                    g.semantic_rules.push(SemanticRule::Structure {
                        //module_uri: rule.uri.uri.clone().into_module(),
                        symbol: rule.clone(),
                        rules: ModuleRules::default(),
                    });
                    let uri = rule.uri.clone();
                    rules.push(ModuleRule::Structure {
                        symbol: rule,
                        rules: ModuleRules::default(),
                    });
                    return Some(uri);
                }
                _ => (),
            }
        }
        groups.tokenizer.problem(
            range.start,
            "mathstructure is only allowed in a module".to_string(),
            DiagnosticLevel::Error,
        );
        None
    }

    #[inline]
    fn new_id(&mut self, prefix: Cow<'static, str>) -> Box<str> {
        self.name_counter.new_id(prefix)
    }

    pub fn add_conservative_ext(
        &mut self,
        groups: &mut Groups<'a, '_, Pos, STeXToken<Pos>, Self>,
        orig: &SymbolReference<Pos>,
        range: StringRange<Pos>,
    ) -> Option<ModuleUri> {
        for g in groups.groups.iter_mut().rev() {
            match &mut g.kind {
                GroupKind::Module { uri, rules, .. } => {
                    let name = self.new_id(Cow::Borrowed("EXTSTRUCT"));
                    let euri = uri.clone() / &name.parse().ok()?;
                    g.semantic_rules.push(SemanticRule::ConservativeExt(
                        orig.clone(),
                        ModuleRules::default(),
                    ));
                    rules.push(ModuleRule::ConservativeExt(
                        orig.clone(),
                        ModuleRules::default(),
                    ));
                    return Some(euri);
                }
                _ => (),
            }
        }
        groups.tokenizer.problem(
            range.start,
            "mathstructure is only allowed in a module".to_string(),
            DiagnosticLevel::Error,
        );
        None
    }

    pub fn add_symbol(
        &mut self,
        groups: &mut Groups<'a, '_, Pos, STeXToken<Pos>, Self>,
        name: UriName,
        macroname: Option<std::sync::Arc<str>>,
        range: StringRange<Pos>,
        has_tp: bool,
        has_df: bool,
        argnum: u8,
    ) -> Option<SymbolReference<Pos>> {
        for g in groups.groups.iter_mut().rev() {
            match &mut g.kind {
                GroupKind::Module { uri, rules, .. }
                | GroupKind::MathStructure { uri, rules }
                | GroupKind::ConservativeExt(uri, rules) => {
                    let suri = uri.clone() | name;
                    let uri = SymbolReference {
                        uri: suri,
                        filepath: self.in_path.clone(),
                        range,
                    };
                    for r in &*rules {
                        match r {
                            ModuleRule::Symbol(s) | ModuleRule::Structure { symbol: s, .. }
                                if s.uri.uri == uri.uri =>
                            {
                                groups.tokenizer.problem(
                                    range.start,
                                    format!("symbol with name {} already exists", s.uri.uri),
                                    DiagnosticLevel::Warning,
                                );
                            }
                            _ => (),
                        }
                    }
                    self.module_store.add_verbalization(
                        uri.uri.name.last(),
                        &uri.uri,
                        Language::English,
                    );
                    let rule = SymbolRule {
                        uri,
                        macroname,
                        has_tp,
                        has_df,
                        argnum,
                    };
                    if MS::FULL
                        && let Some((name, rule)) = rule.as_rule()
                    {
                        let old = groups.rules.insert(name.clone(), rule);
                        if let Entry::Vacant(e) = g.inner.macro_rule_changes.entry(name) {
                            e.insert(old);
                        }
                    }
                    g.semantic_rules.push(SemanticRule::Symbol(rule.clone()));
                    let uri = rule.uri.clone();
                    rules.push(ModuleRule::Symbol(rule));
                    //g.symbols.push(rule);
                    return Some(uri);
                }
                _ => (),
            }
        }
        groups.tokenizer.problem(
            range.start,
            "\\symdecl is only allowed in a module".to_string(),
            DiagnosticLevel::Error,
        );
        None
    }

    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    #[allow(clippy::needless_pass_by_value)]
    #[allow(clippy::too_many_lines)]
    pub(super) fn resolve_module(
        &self,
        module: &'a str,
        archive: Option<ArchiveId>,
    ) -> Option<ModuleReference> {
        if let Some((m, _)) = self.find_module(module) {
            return Some(ModuleReference {
                uri: m.clone(),
                in_doc: self.doc_uri.clone(),
                rel_path: None,
                full_path: self.in_path.clone(),
            });
        }
        let (mut basepath, archive) = archive.as_ref().map_or_else(
            || {
                self.archive.and_then(|a| {
                    self.in_path
                        .as_ref()
                        .and_then(|p| p.to_str())
                        .and_then(|s| {
                            s.find("source")
                                .map(|i| (PathBuf::from(&s[..i - 1]).join("source"), a.clone()))
                        })
                })
            },
            |a| {
                self.backend
                    .with_local_archive(a, |a| a.map(|a| (a.source_dir(), a.uri().clone())))
            },
        )?;

        let (mut path, module) = if let Some((a, b)) = module.split_once('?') {
            (a.trim(), b)
        } else {
            ("", module)
        };

        let top_module = if let Some((t, _)) = module.split_once('/') {
            t
        } else {
            module
        };

        let last = if let Some((p, last)) = path.rsplit_once('/') {
            basepath = p.split('/').fold(basepath, |p, s| p.join(s));
            last
        } else {
            path
        };

        let uri: ModuleUri = if path.trim().is_empty() {
            PathUri::from(archive) | module.parse().ok()?
        } else {
            (PathUri::from(archive) / path.trim().parse::<UriPath>().ok()?) | module.parse().ok()?
        };

        let p = basepath
            .join(last)
            .join(format!("{top_module}.{}.tex", self.language));
        if p.exists() {
            let rel_path = if path.is_empty() {
                format!("{top_module}.{}.tex", self.language)
            } else {
                format!("{path}/{top_module}.{}.tex", self.language)
            };
            return Some(ModuleReference {
                rel_path: Some(rel_path.into()),
                in_doc: uri.path_uri().clone() & (top_module.parse().ok()?, self.language),
                full_path: Some(p.into()),
                uri,
            });
        }

        let p = basepath.join(last).join(format!("{top_module}.en.tex"));
        if p.exists() {
            let rel_path = if path.is_empty() {
                format!("{top_module}.en.tex")
            } else {
                format!("{path}/{top_module}.en.tex")
            };
            return Some(ModuleReference {
                rel_path: Some(rel_path.into()),
                in_doc: uri.path_uri().clone() & (top_module.parse().ok()?, Language::English),
                full_path: Some(p.into()),
                uri,
            });
        }

        let p = basepath.join(last).join(format!("{top_module}.tex"));
        if p.exists() {
            let rel_path = if path.is_empty() {
                format!("{top_module}.tex")
            } else {
                format!("{path}/{top_module}.tex")
            };
            return Some(ModuleReference {
                rel_path: Some(rel_path.into()),
                in_doc: uri.path_uri().clone() & (top_module.parse().ok()?, Language::English),
                full_path: Some(p.into()),
                uri,
            });
        }

        let path_uri = uri.path_uri().clone().up();

        let p = basepath.join(format!("{last}.{}.tex", self.language));
        if p.exists() {
            return Some(ModuleReference {
                uri,
                in_doc: path_uri & (last.parse().ok()?, self.language),
                rel_path: Some(format!("{path}.{}.tex", self.language).into()),
                full_path: Some(p.into()),
            });
        }

        let p = basepath.join(format!("{last}.en.tex"));
        if p.exists() {
            return Some(ModuleReference {
                uri,
                in_doc: path_uri & (last.parse().ok()?, Language::English),
                rel_path: Some(format!("{path}.en.tex").into()),
                full_path: Some(p.into()),
            });
        }

        let p = basepath.join(format!("{last}.tex"));
        if p.exists() {
            return Some(ModuleReference {
                uri,
                in_doc: path_uri & (last.parse().ok()?, Language::English),
                rel_path: Some(format!("{path}.tex").into()),
                full_path: Some(p.into()),
            });
        }
        None
    }

    fn find_module(&self, m: &str) -> Option<(&ModuleUri, &ModuleRules<Pos>)> {
        'top: for (muri, rls) in &self.modules {
            let mut f_steps = m.split('/');
            let mut m_steps = muri.module_name().steps();
            loop {
                let Some(f) = f_steps.next() else {
                    if m_steps.next().is_none() {
                        return Some((muri, rls));
                    }
                    continue 'top;
                };
                let Some(m) = m_steps.next() else {
                    continue 'top;
                };
                if f != m {
                    continue 'top;
                }
            }
        }
        None
    }
}

#[derive(Default)]
#[allow(clippy::large_enum_variant)]
pub enum GroupKind<Pos: StringPosition> {
    #[default]
    None,
    Problem,
    Module {
        uri: ModuleUri,
        rules: Vec<ModuleRule<Pos>>,
    },
    MathStructure {
        uri: ModuleUri,
        rules: Vec<ModuleRule<Pos>>,
    },
    ConservativeExt(ModuleUri, Vec<ModuleRule<Pos>>),
    DefPara(Vec<SymbolReference<Pos>>),
    Morphism {
        domain: ModuleOrStruct<Pos>,
        rules: Vec<ModuleRules<Pos>>,
        specs: VecMap<SymbolReference<Pos>, MorphismSpec<Pos>>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct MorphismSpec<Pos: StringPosition> {
    pub macroname: Option<Box<str>>,
    pub new_name: Option<UriName>,
    pub is_assigned_at: Option<StringRange<Pos>>,
    pub decl_range: StringRange<Pos>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ModuleOrStruct<Pos: StringPosition> {
    Module(ModuleReference),
    Struct(SymbolReference<Pos>),
}

#[non_exhaustive]
pub struct STeXGroup<'a, MS: STeXModuleStore, Pos: StringPosition + 'a> {
    pub inner: Group<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>>,
    pub kind: GroupKind<Pos>,
    pub semantic_rules: Vec<SemanticRule<Pos>>,
    pub uses: VecSet<ModuleUri>,
}

pub enum SemanticRule<Pos: StringPosition> {
    Symbol(SymbolRule<Pos>),
    Module(ModuleReference, ModuleRules<Pos>),
    Structure {
        symbol: SymbolRule<Pos>,
        //module_uri:ModuleUri,
        rules: ModuleRules<Pos>,
    },
    ConservativeExt(SymbolReference<Pos>, ModuleRules<Pos>),
    StructureImport(SymbolReference<Pos>, ModuleRules<Pos>),
}

impl<'a, MS: STeXModuleStore, Pos: StringPosition>
    GroupState<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>> for STeXGroup<'a, MS, Pos>
{
    #[inline]
    fn new(parent: Option<&mut Self>) -> Self {
        Self {
            inner: Group::new(parent.map(|p| &mut p.inner)),
            kind: GroupKind::None,
            semantic_rules: Vec::new(),
            uses: VecSet::default(),
        }
    }

    #[inline]
    fn inner(&self) -> &Group<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>> {
        &self.inner
    }
    #[inline]
    fn inner_mut(&mut self) -> &mut Group<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>> {
        &mut self.inner
    }
    #[inline]
    fn close(self, parser: &mut LaTeXParser<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>>) {
        self.inner.close(parser);
    }
    #[inline]
    fn add_macro_rule(
        &mut self,
        name: Cow<'a, str>,
        old: Option<AnyMacro<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>>>,
    ) {
        self.inner.add_macro_rule(name, old);
    }
    #[inline]
    fn add_environment_rule(
        &mut self,
        name: Cow<'a, str>,
        old: Option<AnyEnv<'a, Pos, STeXToken<Pos>, STeXParseState<'a, Pos, MS>>>,
    ) {
        self.inner.add_environment_rule(name, old);
    }
    #[inline]
    fn letter_change(&mut self, old: &str) {
        self.inner.letter_change(old);
    }
}

#[derive(Clone, Debug)]
pub enum MacroArg<Pos: StringPosition> {
    Symbol(SymbolReference<Pos>, u8),
    Variable(UriName, StringRange<Pos>, bool, u8),
}

impl<'a, MS: STeXModuleStore, Pos: StringPosition> ParserState<'a, Pos, STeXToken<Pos>>
    for STeXParseState<'a, Pos, MS>
{
    type Group = STeXGroup<'a, MS, Pos>;
    type MacroArg = MacroArg<Pos>;
    #[inline]
    fn from_text(
        &self,
        r: StringRange<Pos>,
        text: &'a str,
        in_document: bool,
        in_math: bool,
        groups: &mut Groups<'a, '_, Pos, STeXToken<Pos>, Self>,
    ) -> Option<STeXToken<Pos>> {
        if MS::FULL && in_document && !in_math {
            let f = |s: &SymbolUri| {
                for g in groups.groups.iter().rev() {
                    for r in g.semantic_rules.iter().rev() {
                        if let SemanticRule::Symbol(sym)
                        | SemanticRule::Structure { symbol: sym, .. } = r
                            && sym.uri.uri == *s
                        {
                            return false;
                        }
                    }
                }
                true
            };
            let r = self.module_store.add_text(r, text, self.language, &f);
            if let Some(STeXToken::Vec(v)) = &r {
                for t in v {
                    if let STeXToken::SnifySuggestion { range, .. } = t {
                        (groups.tokenizer.err)(
                            "snify suggestion".to_string(),
                            *range,
                            DiagnosticLevel::Info,
                        )
                    }
                }
            }
            r
        } else {
            None
        }
    }
}
