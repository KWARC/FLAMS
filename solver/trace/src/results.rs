use std::marker::PhantomData;

use ftml_ontology::terms::Term;
use ftml_uris::{DocumentElementUri, DocumentUri, FtmlUri, ModuleUri, SymbolUri};
use serde_json::de;

use crate::CheckLog;

#[cfg(feature = "colors")]
use crate::ColorDisplay;
#[cfg(feature = "full")]
use crate::{FmtTraceDisplay, TraceDisplay};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentCheckResult {
    pub uri: DocumentUri,
    pub checks: Box<[CheckResult]>,
}
impl DocumentCheckResult {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        if preserve_siblings {
            for c in &mut self.checks {
                if c.success() {
                    c.filter_failures(false);
                } else {
                    c.filter_failures(true);
                }
            }
        } else {
            self.checks = std::mem::take(&mut self.checks)
                .into_iter()
                .filter(|c| !c.success())
                .map(|mut e| {
                    e.filter_failures(false);
                    e
                })
                .collect();
        }
    }
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut s = Vec::<u8>::new();
        let mut serializer = serde_json::Serializer::new(&mut s);
        let serializer = serde_stacker::Serializer::new(&mut serializer);
        let _ = <Self as serde::Serialize>::serialize(self, serializer);
        String::from_utf8(s).unwrap_or_default()
    }
    /// #### Errors
    pub fn from_json(s: &str) -> Result<Self, String> {
        let mut deserializer = serde_json::Deserializer::from_str(s);
        deserializer.disable_recursion_limit();
        let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
        <Self as serde::Deserialize>::deserialize(deserializer).map_err(|e| e.to_string())
    }
    #[cfg(feature = "full")]
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        struct Displayer<'a, D: FmtTraceDisplay>(&'a DocumentCheckResult, PhantomData<D>);
        impl<D: FmtTraceDisplay> std::fmt::Display for Displayer<'_, D> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut d = D::new(f);
                d.string("Checking document ", Some(crate::MessageLevel::Header))?;
                d.uri(self.0.uri.as_uri(), Some(crate::MessageLevel::Header))?;
                d.string("\n", None)?;
                drop(d);
                for c in &self.0.checks {
                    c.display::<D>().fmt(f)?;
                }
                Ok(())
            }
        }

        //println!("test: {self:?}");
        Displayer(self, PhantomData::<D>)
    }

    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        self.display::<ColorDisplay>()
    }

    pub fn success(&self) -> bool {
        self.checks.iter().all(CheckResult::success)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SymbolCheckResult {
    TypeOnly {
        result: TypeCheckResult,
    },
    DefiniensOnly {
        inferred: Option<Term>,
        log: CheckLog,
    },
    Both {
        inhabitable: TypeCheckResult,
        matches: Option<TypeCheckResult>,
    },
}
impl SymbolCheckResult {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        match self {
            Self::TypeOnly { result } => result.filter_failures(preserve_siblings),
            Self::DefiniensOnly { log, .. } => log.filter_failures(preserve_siblings),
            Self::Both {
                inhabitable,
                matches,
            } => {
                inhabitable.filter_failures(preserve_siblings);
                if let Some(m) = matches {
                    m.filter_failures(preserve_siblings);
                }
            }
        }
    }
    #[cfg(feature = "full")]
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        struct Displayer<'a, D: FmtTraceDisplay>(&'a SymbolCheckResult, PhantomData<D>);
        impl<D: FmtTraceDisplay> std::fmt::Display for Displayer<'_, D> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.0 {
                    SymbolCheckResult::TypeOnly { result } => result.display::<D>().fmt(f),
                    SymbolCheckResult::DefiniensOnly { log, .. } => log.display::<D>().fmt(f),
                    SymbolCheckResult::Both {
                        inhabitable,
                        matches,
                    } => {
                        inhabitable.display::<D>().fmt(f)?;
                        if let Some(r) = matches.as_ref() {
                            r.display::<D>().fmt(f)?;
                        }
                        Ok(())
                    }
                }
            }
        }
        Displayer(self, PhantomData::<D>)
    }

    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        self.display::<ColorDisplay>()
    }
    #[must_use]
    pub fn success(&self) -> bool {
        match self {
            Self::TypeOnly { result } => result.success,
            Self::DefiniensOnly { inferred, .. } => inferred.is_some(),
            Self::Both {
                inhabitable,
                matches,
            } => inhabitable.success && matches.as_ref().is_some_and(|r| r.success),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CheckResult {
    Module {
        uri: ModuleUri,
        checks: Vec<ContentCheckResult>,
    },
    Variable(DocumentElementUri, SymbolCheckResult),
    Proof(DocumentElementUri, Vec<ProofStepResult>),
    Term {
        uri: DocumentElementUri,
        inferred: Option<Term>,
        log: CheckLog,
    },
    Content(ContentCheckResult),
    Missing(ModuleUri),
}
impl CheckResult {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        match self {
            Self::Module { checks, .. } => {
                if preserve_siblings {
                    for c in checks {
                        if c.success() {
                            c.filter_failures(false);
                        } else {
                            c.filter_failures(true);
                        }
                    }
                } else {
                    *checks = std::mem::take(checks)
                        .into_iter()
                        .filter(|c| !c.success())
                        .map(|mut c| {
                            c.filter_failures(false);
                            c
                        })
                        .collect();
                }
            }
            Self::Variable(_, check) => check.filter_failures(preserve_siblings),
            Self::Proof(_, checks) => {
                if preserve_siblings {
                    for c in checks {
                        if c.success() {
                            c.filter_failures(false);
                        } else {
                            c.filter_failures(true);
                        }
                    }
                } else {
                    *checks = std::mem::take(checks)
                        .into_iter()
                        .filter(|c| !c.success())
                        .map(|mut c| {
                            c.filter_failures(false);
                            c
                        })
                        .collect();
                }
            }
            Self::Term { log, .. } => log.filter_failures(preserve_siblings),
            Self::Content(check) => check.filter_failures(preserve_siblings),
            _ => (),
        }
    }
    #[cfg(feature = "full")]
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        struct Displayer<'a, D: FmtTraceDisplay>(&'a CheckResult, PhantomData<D>);
        impl<D: FmtTraceDisplay> std::fmt::Display for Displayer<'_, D> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.0 {
                    CheckResult::Missing(u) => {
                        let mut d = D::new(f);
                        d.string("\nMissing module: ", Some(crate::MessageLevel::Failure))?;
                        d.uri(u.as_uri(), Some(crate::MessageLevel::Failure))?;
                        d.string("\n", None)
                    }
                    CheckResult::Module { uri, checks } => {
                        let mut d = D::new(f);
                        d.string("\nChecking module ", Some(crate::MessageLevel::Header))?;
                        d.uri(uri.as_uri(), Some(crate::MessageLevel::Header))?;
                        d.string("\n", None)?;
                        drop(d);
                        for c in checks {
                            c.display::<D>().fmt(f)?;
                        }
                        Ok(())
                    }
                    CheckResult::Variable(uri, r) => {
                        let mut d = D::new(f);
                        d.string("\nChecking variable ", Some(crate::MessageLevel::Header))?;
                        d.uri(uri.as_uri(), Some(crate::MessageLevel::Header))?;
                        d.string("\n", None)?;
                        drop(d);
                        r.display::<D>().fmt(f)
                    }
                    CheckResult::Term { uri, log, .. } => {
                        let mut d = D::new(f);
                        d.string("\nChecking term ", Some(crate::MessageLevel::Header))?;
                        d.uri(uri.as_uri(), Some(crate::MessageLevel::Header))?;
                        d.string("\n", None)?;
                        drop(d);
                        log.display::<D>().fmt(f)
                    }
                    CheckResult::Content(c) => c.display::<D>().fmt(f),
                    CheckResult::Proof(uri, checks) => {
                        let mut d = D::new(f);
                        d.string("\nChecking proof ", Some(crate::MessageLevel::Header))?;
                        d.uri(uri.as_uri(), Some(crate::MessageLevel::Header))?;
                        d.string("\n", None)?;
                        drop(d);
                        for c in checks {
                            c.display::<D>().fmt(f)?;
                        }
                        Ok(())
                    }
                }
            }
        }

        Displayer(self, PhantomData::<D>)
    }
    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        self.display::<ColorDisplay>()
    }

    pub fn success(&self) -> bool {
        match self {
            Self::Module { checks, .. } => checks.iter().all(ContentCheckResult::success),
            Self::Variable(_, res) => res.success(),
            Self::Term { inferred, .. } => inferred.is_some(),
            Self::Missing(_) => false,
            Self::Content(c) => c.success(),
            Self::Proof(_, s) => s.iter().all(ProofStepResult::success),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ProofStepResult {
    Assumption {
        var: Option<DocumentElementUri>,
        result: ProofStepCheckResult,
    },
    Conclusion {
        var: Option<DocumentElementUri>,
        result: ProofStepCheckResult,
    },
    Step {
        var: Option<DocumentElementUri>,
        result: ProofStepCheckResult,
    },
    Subproof {
        uri: DocumentElementUri,
        var: Option<DocumentElementUri>,
        results: Vec<Self>,
    },
}
impl ProofStepResult {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        match self {
            Self::Assumption { result, .. }
            | Self::Conclusion { result, .. }
            | Self::Step { result, .. } => result.filter_failures(preserve_siblings),
            Self::Subproof { results, .. } => {
                if preserve_siblings {
                    for c in results {
                        if c.success() {
                            c.filter_failures(false);
                        } else {
                            c.filter_failures(true);
                        }
                    }
                } else {
                    *results = std::mem::take(results)
                        .into_iter()
                        .filter(|s| !s.success())
                        .map(|mut c| {
                            c.filter_failures(false);
                            c
                        })
                        .collect();
                }
            }
        }
    }
    pub fn success(&self) -> bool {
        match self {
            Self::Assumption { result, .. }
            | Self::Conclusion { result, .. }
            | Self::Step { result, .. } => result.success(),
            Self::Subproof { results, .. } => results.iter().all(Self::success),
        }
    }
    #[cfg(feature = "full")]
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        struct Displayer<'a, D: FmtTraceDisplay>(&'a ProofStepResult, PhantomData<D>);
        impl<D: FmtTraceDisplay> std::fmt::Display for Displayer<'_, D> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut d = D::new(f);
                match self.0 {
                    ProofStepResult::Assumption { var, result } => {
                        d.string("\nChecking Assumption ", Some(crate::MessageLevel::Header))?;
                        if let Some(var) = var {
                            d.uri(var.as_uri(), Some(crate::MessageLevel::Header))?;
                        }
                        d.string("\n", None)?;
                        drop(d);
                        result.display::<D>().fmt(f)
                    }
                    ProofStepResult::Conclusion { var, result } => {
                        d.string("\nChecking Conclusion ", Some(crate::MessageLevel::Header))?;
                        if let Some(var) = var {
                            d.uri(var.as_uri(), Some(crate::MessageLevel::Header))?;
                        }
                        d.string("\n", None)?;
                        drop(d);
                        result.display::<D>().fmt(f)
                    }
                    ProofStepResult::Step { var, result } => {
                        d.string("\nChecking Step ", Some(crate::MessageLevel::Header))?;
                        if let Some(var) = var {
                            d.uri(var.as_uri(), Some(crate::MessageLevel::Header))?;
                        }
                        d.string("\n", None)?;
                        drop(d);
                        result.display::<D>().fmt(f)
                    }
                    ProofStepResult::Subproof { uri, var, results } => {
                        d.string("\nChecking Subproof ", Some(crate::MessageLevel::Header))?;
                        d.uri(uri.as_uri(), Some(crate::MessageLevel::Header))?;
                        if let Some(var) = var {
                            d.string(" = ", Some(crate::MessageLevel::Header))?;
                            d.uri(var.as_uri(), Some(crate::MessageLevel::Header))?;
                        }
                        d.string("\n", None)?;
                        drop(d);
                        for r in results {
                            r.display::<D>().fmt(f)?;
                        }
                        Ok(())
                    }
                }
            }
        }

        Displayer(self, PhantomData::<D>)
    }
    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        self.display::<ColorDisplay>()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ContentCheckResult {
    Symbol(SymbolUri, SymbolCheckResult),
}
impl ContentCheckResult {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        match self {
            Self::Symbol(_, check) => check.filter_failures(preserve_siblings),
        }
    }
    #[cfg(feature = "full")]
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        struct Displayer<'a, D: FmtTraceDisplay>(&'a ContentCheckResult, PhantomData<D>);
        impl<D: FmtTraceDisplay> std::fmt::Display for Displayer<'_, D> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut d = D::new(f);
                match self.0 {
                    ContentCheckResult::Symbol(uri, s) => {
                        d.string("\nChecking symbol ", Some(crate::MessageLevel::Header))?;
                        d.uri(uri.as_uri(), Some(crate::MessageLevel::Header))?;
                        d.string("\n", None)?;
                        drop(d);
                        s.display::<D>().fmt(f)
                    }
                }
            }
        }

        Displayer(self, PhantomData::<D>)
    }
    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        self.display::<ColorDisplay>()
    }
    #[must_use]
    pub fn success(&self) -> bool {
        match self {
            Self::Symbol(_, r) => r.success(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeCheckResult {
    pub success: bool,
    pub log: CheckLog,
}
impl TypeCheckResult {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        self.log.filter_failures(preserve_siblings);
    }
    #[cfg(feature = "full")]
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        self.log.display::<D>()
    }
    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        self.display::<ColorDisplay>()
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ProofStepCheckResult {
    GoalOnly {
        result: TypeCheckResult,
    },
    ProofOnly {
        inferred: Option<Term>,
        log: CheckLog,
    },
    Both {
        inhabitable: TypeCheckResult,
        matches: Option<TypeCheckResult>,
    },
}
impl ProofStepCheckResult {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        match self {
            Self::GoalOnly { result } => result.filter_failures(preserve_siblings),
            Self::ProofOnly { log, .. } => log.filter_failures(preserve_siblings),
            Self::Both {
                inhabitable,
                matches,
            } => {
                inhabitable.filter_failures(preserve_siblings);
                if let Some(m) = matches {
                    m.filter_failures(preserve_siblings);
                }
            }
        }
    }
    #[cfg(feature = "full")]
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        struct Displayer<'a, D: FmtTraceDisplay>(&'a ProofStepCheckResult, PhantomData<D>);
        impl<D: FmtTraceDisplay> std::fmt::Display for Displayer<'_, D> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.0 {
                    ProofStepCheckResult::GoalOnly { result } => result.display::<D>().fmt(f),
                    ProofStepCheckResult::ProofOnly { log, .. } => log.display::<D>().fmt(f),
                    ProofStepCheckResult::Both {
                        inhabitable,
                        matches,
                    } => {
                        inhabitable.display::<D>().fmt(f)?;
                        if let Some(r) = matches.as_ref() {
                            r.display::<D>().fmt(f)?;
                        }
                        Ok(())
                    }
                }
            }
        }
        Displayer(self, PhantomData::<D>)
    }

    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        self.display::<ColorDisplay>()
    }
    #[must_use]
    pub fn success(&self) -> bool {
        match self {
            Self::GoalOnly { result } => result.success,
            Self::ProofOnly { inferred, .. } => inferred.is_some(),
            Self::Both {
                inhabitable,
                matches,
            } => inhabitable.success && matches.as_ref().is_some_and(|r| r.success),
        }
    }
}
