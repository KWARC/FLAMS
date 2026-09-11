pub mod context;
mod impls;
pub mod results {
    pub use ftml_solver_trace::results::*;
}
pub mod paragraphs;
pub mod rules;
pub mod split;
pub mod trace {
    pub use ftml_solver_trace::*;
}
pub mod facts;
pub mod hoas;
//pub mod patterns;
pub mod judgment_cache;
pub mod utils;

const TRUNCATE_PROOFS: bool = false;
static DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn pause() {
    use std::io::Read;
    if DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
        println!("Press Key...");
        let _ = std::io::stdin().read(&mut [0u8]);
    }
}

use crate::{
    context::ContextWrap,
    facts::GlobalFacts,
    hoas::HOASSymbols,
    impls::{
        proving::ProverState,
        solving::{Solutions, TermExtSolvable, is_solvable_var},
    },
    judgment_cache::JudgmentCache,
    results::{
        CheckResult, ContentCheckResult, DocumentCheckResult, SymbolCheckResult, TypeCheckResult,
    },
    rules::RuleSet,
    split::{CancelToken, RayonStrategiesDepth, SingleThreadedSplit, SplitStrategy},
    trace::{CheckLogCow, CheckingTask, PreCheckLog},
    utils::MutableRefList,
};
use flams_math_archives::{
    artifacts::{ContentUpdate, FileOrString},
    backend::{AnyBackend, LocalBackend},
    formats::{BuildResult, TaskDependency},
};
use ftml_ontology::{
    domain::{
        HasDeclarations,
        declarations::{AnyDeclarationRef, morphisms::Morphism, symbols::Symbol},
        modules::Module,
    },
    narrative::{
        documents::Document,
        elements::{
            DocumentElementRef, LogicalParagraph, VariableDeclaration, paragraphs::ParagraphKind,
        },
    },
    terms::{ComponentVar, Term, TermContainer, helpers::IntoTerm, termpaths::TermPath},
    utils::RefTree,
};
use ftml_solver_trace::CheckLog;
use ftml_uris::{Id, IsDomainUri, LeafUri, ModuleUri};
use smallvec::SmallVec;
use std::marker::PhantomData;

pub static DUMMY: std::sync::LazyLock<Id> =
    // SAFETY: "DUMMY" is a valid ID
    std::sync::LazyLock::new(|| unsafe { "DUMMY".parse().unwrap_unchecked() });

flams_math_archives::build_target!(CHECK {
    name: "typecheck",
    description: "check the validity of formal/complex expressions",
    run: check
});

#[allow(clippy::needless_pass_by_value)]
fn check(task: flams_math_archives::formats::BuildSpec) -> BuildResult {
    let d = match task.backend.get_document(task.uri) {
        Ok(d) => d,
        Err(e) => {
            return BuildResult {
                log: FileOrString::Str(
                    format!("Document not found {}: {e}", task.uri).into_boxed_str(),
                ),
                result: Err(Vec::new()),
            };
        }
    };
    let mut checker =
        Checker::</*RayonStrategiesDepth<4>*/ SingleThreadedSplit>::new(task.backend.clone());
    match checker.check_document(&d) {
        Ok((logs, modules)) => {
            let log = logs.to_json();
            BuildResult {
                log: FileOrString::Str(log.into_boxed_str()),
                result: Ok(Some(Box::new(ContentUpdate {
                    document: Some(d),
                    modules,
                }) as _)),
            }
        }
        Err(e) => BuildResult {
            log: FileOrString::Str(format!("Module missing: {e}").into_boxed_str()),
            result: Err(vec![TaskDependency::Logical {
                uri: e,
                strict: true,
            }]),
        },
    }
}

type BigSet<T> = dashmap::DashSet<T, rustc_hash::FxBuildHasher>;

/*
static MINIMAL_STACK: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(usize::MAX - 10);
pub fn minimal_stack() -> usize {
    MINIMAL_STACK.load(std::sync::atomic::Ordering::Acquire)
}
pub(crate) fn update_stack() {
    let curr = MINIMAL_STACK.load(std::sync::atomic::Ordering::Acquire);
    let Some(rem) = stacker::remaining_stack() else {
        return;
    };
    let min = curr.min(rem);
    if min < curr {
        MINIMAL_STACK.store(min, std::sync::atomic::Ordering::Release);
    }
}
 */

pub struct SubtermCheckResult {
    pub simplified: Term,
    pub inferred_type: Option<Term>,
    pub context: Vec<ComponentVar>,
    pub log: CheckLog,
}

pub struct Checker<Split: SplitStrategy> {
    backend: AnyBackend,
    rules: RuleSet<Split>,
    modules: BigSet<Module>,
    documents: BigSet<Document>,
    facts: GlobalFacts,
    hoas: Option<HOASSymbols>,
    implicits: std::sync::atomic::AtomicUsize,
    current: Vec<LeafUri>,
    context: Vec<ModuleUri>,
    __phantom: PhantomData<Split>,
}

pub struct CheckRef<'c, 'i, Split: SplitStrategy> {
    pub(crate) top: &'c Checker<Split>,
    pub(crate) context: ContextWrap<'c, 'i>,
    pub(crate) proof_state: &'i ProverState,
    pub(crate) judgment_cache: JudgmentCache<'c, 'i>,
    pub(crate) solutions: MutableRefList<'i, Solutions>,
    messages: &'i mut SmallVec<CheckLogCow<'c>, 2>,
    pub(crate) cancel: &'i CancelToken<'i, Split::CancelToken>,
    added: u8,
    traced: bool,
}

macro_rules! update {
    ($slf:ident $t:ident if $bind:expr) => {
        if $bind && let Some(t) = $slf.bind_implicits(&$t) {
            //println!("update: {:?}", t); //t.debug_short());
            $t = t;
        }
    };
}

#[derive(Default)]
pub struct CheckerCache {
    modules: BigSet<Module>,
    documents: BigSet<Document>,
}

impl<Split: SplitStrategy> Checker<Split> {
    fn reset(&self) {
        self.implicits
            .store(0, std::sync::atomic::Ordering::Release);
    }
    pub fn into_cache(self) -> CheckerCache {
        CheckerCache {
            modules: self.modules,
            documents: self.documents,
        }
    }
    pub fn set_cache(&mut self, cache: CheckerCache) {
        self.modules = cache.modules;
        self.documents = cache.documents;
    }
    #[must_use]
    pub fn new(backend: AnyBackend) -> Self {
        Self {
            backend,
            modules: BigSet::default(),
            documents: BigSet::default(),
            rules: RuleSet::default(),
            current: Vec::default(),
            facts: GlobalFacts::default(),
            context: Vec::default(),
            hoas: None,
            implicits: std::sync::atomic::AtomicUsize::new(1),
            __phantom: PhantomData,
        }
    }

    fn set_hoas(&mut self) {
        self.hoas = HOASSymbols::get(self);
    }
    pub const fn hoas(&self) -> Option<&HOASSymbols> {
        self.hoas.as_ref()
    }

    /// #### Errors
    /// if a module dependency is missing
    #[allow(clippy::too_many_lines)]
    pub fn check_document(
        &mut self,
        d: &Document,
    ) -> Result<(DocumentCheckResult, Vec<Module>), ModuleUri> {
        /*tracing::error!(
            "Current: {}",
            bytesize::ByteSize::b(stacker::remaining_stack().expect("wut") as _)
                .display()
                .iec_short()
        );*/

        //let mut all = Vec::new();
        let mut modules = Vec::new();
        let mut results = Vec::new();
        self.documents.insert(d.clone());
        for e in d.dfs() {
            match e {
                //DocumentElementRef::UseModule { uri: module, .. } => all.push(module.clone()),
                DocumentElementRef::Module { module, .. } => {
                    //all.push(module.clone());
                    modules.push(module.clone());
                }
                _ => (),
            }
        }
        //self.set_context_i(all)?;
        let modules: Vec<_> = modules
            .into_iter()
            .filter_map(|uri| self.get_module(&uri).ok())
            .collect();
        tracing::trace!("Rules: {:?}", self.rules);
        for e in d.dfs() {
            match e {
                DocumentElementRef::Module { module, .. }
                | DocumentElementRef::UseModule { uri: module, .. } => {
                    self.set_context(vec![module.clone()])?;
                    //let _ = self.get_module(module).map_err(|_| module.clone())?;
                    /*if self.get_module(module).is_err() {
                        results.push(CheckResult::Missing(module.clone()));
                    }*/
                }
                DocumentElementRef::Morphism { morphism, .. } => {
                    let top = morphism.clone().simple_module();
                    let Some(m) = self
                        .get_module(top.module_uri())
                        .ok()
                        .and_then(|m| m.get_as::<Morphism>(top.name()))
                    else {
                        results.push(CheckResult::Missing(morphism.module.clone()));
                        continue;
                    };
                    results.extend(self.check_morphism(&m));
                }
                DocumentElementRef::VariableDeclaration(v) => {
                    if let Some(r) = self.check_variable(v) {
                        results.push(CheckResult::Variable(v.uri.clone(), r));
                    }
                }
                DocumentElementRef::SymbolDeclaration(uri) => {
                    let top = uri.clone().simple_module();
                    if let Some(s) = modules
                        .iter()
                        .find(|m| m.uri == top.module)
                        .and_then(|m| m.find::<Symbol>(top.name.steps()))
                    {
                        if let Some(r) = self.check_symbol(s) {
                            results.push(CheckResult::Content(ContentCheckResult::Symbol(
                                uri.clone(),
                                r,
                            )));
                        }
                    } else {
                        results.push(CheckResult::Missing(uri.module.clone()));
                    }
                }
                DocumentElementRef::Term(top) => {
                    //self.set_hoas();
                    self.reset();
                    tracing::debug!("Checking term {:?}", top.get_parsed().debug_short());
                    //println!("All rules: {:#?}", self.rules);
                    let (unks, tm) = self.prepare(None, top.get_parsed().clone());
                    let (t, unks, log) = self.infer_type(Some(unks), &tm);
                    let t = t.map(|t| {
                        self.wrap_none(Some(unks), |slf| slf.revert_prepare(slf.subst(t)))
                            .1
                    });
                    if let Some(t) = &t {
                        top.set_type(t.clone());
                    }
                    results.push(CheckResult::Term {
                        uri: top.uri.clone(),
                        inferred: t,
                        log: CheckLog::from_pre(log, &mut |t| self.revert_prepare(t)),
                    });
                }
                DocumentElementRef::Paragraph(
                    p @ LogicalParagraph {
                        kind: ParagraphKind::Proof,
                        fors,
                        ..
                    },
                ) if fors.len() == 1 => {
                    //self.set_hoas();
                    self.reset();
                    results.extend(self.check_proof(p));
                }
                DocumentElementRef::Paragraph(
                    p @ LogicalParagraph {
                        kind: ParagraphKind::Assertion,
                        fors,
                        ..
                    },
                ) if p.fors.iter().any(|(_, t)| t.is_some()) => {
                    //self.set_hoas();
                    self.reset();
                    if let Some(r) = self.check_assertion(p) {
                        results.extend(r);
                    }
                }
                DocumentElementRef::Paragraph(
                    p @ LogicalParagraph {
                        kind, fors, styles, ..
                    },
                ) if kind.is_definition_like(styles) && p.fors.iter().any(|(_, t)| t.is_some()) => {
                    //self.set_hoas();
                    self.reset();
                    results.extend(self.check_definition(p));
                }
                _ => (),
            }
        }

        Ok((
            DocumentCheckResult {
                uri: d.uri.clone(),
                checks: results.into_boxed_slice(),
            },
            modules,
        ))
    }

    // TODO return checked Module
    pub fn check_module(&mut self, m: &Module) -> Result<Vec<ContentCheckResult>, ModuleUri> {
        self.modules.insert(m.clone());
        self.set_context(vec![m.uri.clone()])?;
        //self.set_hoas();
        let mut ret = Vec::new();
        for d in m.dfs().filter_map(|e| {
            if let AnyDeclarationRef::Symbol(s) = e {
                Some(s)
            } else {
                None
            }
        }) {
            if let Some(r) = self.check_symbol(d) {
                ret.push(ContentCheckResult::Symbol(d.uri.clone(), r));
            }
        }
        Ok(ret)
    }

    pub fn check_subterm_term(&mut self, term: Term, sub: Term) -> Option<SubtermCheckResult> {
        self.reset();
        let (unks, nterm) = self.wrap_none(None, |mut slf| {
            let (s, r) = slf.prepare(term, None);
            slf.merge_solutions(s);
            r
        });
        let (unks, nsub) = self.wrap_none(Some(unks), |mut slf| {
            let (s, r) = slf.prepare(sub, None);
            slf.merge_solutions(s);
            r
        });
        let ctx = nterm
            .path_of_subterm_with_ctx(&nsub)
            .map_or(Vec::new(), |p| p.0);
        Some(self.check_subterm_i(unks, &nsub, ctx))
    }

    pub fn check_subterm_path(
        &mut self,
        term: Term,
        mut path: TermPath,
    ) -> Option<SubtermCheckResult> {
        //self.set_hoas();
        self.reset();
        let (unks, nterm) = self.wrap_none(None, |mut slf| {
            let (s, r) = slf.prepare(term, Some(&mut path));
            slf.merge_solutions(s);
            r
        });
        let (ctx, t) = nterm.subterm_at_path(&path)?;
        //ctx.reverse();
        Some(self.check_subterm_i(unks, t, ctx))
    }

    fn check_subterm_i(
        &self,
        unks: Solutions,
        sub: &Term,
        ctx: Vec<&ComponentVar>,
    ) -> SubtermCheckResult {
        let mut ctx = ctx.into_iter().cloned().rev().collect::<Vec<_>>();
        let mut nt = sub.clone();
        let (r, s, log) = self.wrap_task(CheckingTask::Inference(sub), Some(unks), |mut slf| {
            let allvars = sub.free_variables();
            for v in allvars {
                if is_solvable_var(v).is_none() && !ctx.iter().any(|cv| cv.var == *v) {
                    let tp = slf.infer_var_type_i(v);
                    let df = slf.get_var_definiens(v);
                    ctx.push(ComponentVar {
                        var: v.clone(),
                        tp,
                        df,
                    });
                }
            }
            let mut i = 0;
            while let Some(vd) = ctx.get(i) {
                i += 1;
                let tp = vd.tp.clone();
                let df = vd.df.clone();
                if let Some(t) = tp {
                    let allvars = t
                        .free_variables()
                        .into_iter()
                        .filter(|v| !ctx.iter().any(|cv| cv.var == **v))
                        .cloned()
                        .collect::<smallvec::SmallVec<_, 2>>();
                    for v in allvars {
                        if is_solvable_var(&v).is_none() {
                            let tp = slf.infer_var_type_i(&v);
                            let df = slf.get_var_definiens(&v);
                            ctx.push(ComponentVar { var: v, tp, df });
                        }
                    }
                }
                if let Some(t) = df {
                    let allvars = t
                        .free_variables()
                        .into_iter()
                        .filter(|v| !ctx.iter().any(|cv| cv.var == **v))
                        .cloned()
                        .collect::<smallvec::SmallVec<_, 2>>();
                    for v in allvars {
                        if is_solvable_var(&v).is_none() {
                            let tp = slf.infer_var_type_i(&v);
                            let df = slf.get_var_definiens(&v);
                            ctx.push(ComponentVar { var: v, tp, df });
                        }
                    }
                }
            }
            ctx.reverse();
            for c in &ctx {
                slf.extend_context(c);
            }

            let tp = slf
                .infer_type(sub)
                .map(|t| slf.subst(slf.revert_prepare(t)));
            let simp = slf
                .simplify_full(impls::simplify::Expansion::NoNoexpands, sub)
                .unwrap_or_else(|| sub.clone());
            nt = slf.subst(slf.revert_prepare(simp));
            tp
        });
        /*
        let mut frees = nt.free_variables();
        for v in r.as_ref().map(|t| t.free_variables()).unwrap_or_default() {
            if !frees.contains(&v) {
                frees.push(v);
            }
        }
         */
        /*let context = ctx
        .into_iter()
        //.filter(|v| frees.iter().any(|f| f.name() == v.var.name()))
        .cloned()
        .collect();*/
        let (_, log) = self.wrap_none(None, |slf| {
            for c in &mut ctx {
                if let Some(tp) = c.tp.take() {
                    c.tp = Some(slf.revert_prepare(tp));
                }
                if let Some(df) = c.df.take() {
                    c.df = Some(slf.revert_prepare(df));
                }
            }
            CheckLog::from_pre(log, &mut |t| slf.revert_prepare(t))
        });
        //drop(frees);
        SubtermCheckResult {
            simplified: nt,
            inferred_type: r,
            context: ctx,
            log,
        }
    }

    fn check_components(
        &self,
        tpc: &TermContainer,
        dfc: &TermContainer,
        bind: bool,
    ) -> Option<SymbolCheckResult> {
        self.reset();
        let tp = tpc.get_parsed();
        let df = dfc.get_parsed();
        let df = if df.is_some_and(Term::is_marker) {
            None
        } else {
            df
        };
        //self.set_hoas();
        match (tp, df) {
            (Some(tp), None) => {
                tracing::debug!("Checking Type: {:?}", tp.debug_short());
                //tracing::debug!("Facts: {:#?}", self.facts);
                let (unks, tp) = self.prepare(None, tp.clone());
                let (b, unks, mut l) = self.check_inhabitable(Some(unks), &tp);
                let mut tp = self.wrap_none(Some(unks), |slf| slf.subst(tp)).1;
                if tp.has_solvable() {
                    l.push(PreCheckLog::Msg(
                        vec![format!("Unsolved unkowns remain: {:?}", tp.solvables()).into()],
                        ftml_solver_trace::MessageLevel::Failure,
                    ));
                    return Some(SymbolCheckResult::TypeOnly {
                        result: TypeCheckResult {
                            success: false,
                            log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                        },
                    });
                }
                update!(self tp if bind);
                tpc.set_checked(tp);
                Some(SymbolCheckResult::TypeOnly {
                    result: TypeCheckResult {
                        success: b.unwrap_or(false),
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    },
                })
            }
            (None, Some(df)) => Some(self.df_only(df, dfc, tpc, bind, false)),
            (Some(tp), Some(df)) => {
                tracing::debug!("Checking Type and Definiens");
                if ftml_uris::metatheory::AUTO_PROVE.term_is(df) {
                    Some(self.infer_df(dfc, tp, tpc, bind))
                } else {
                    //tracing::debug!("Facts: {:#?}", self.facts);
                    Some(self.df_and_tp(df, dfc, tp, tpc, bind, false))
                }
            }
            (None, None) => None,
        }
    }

    fn df_only(
        &self,
        df: &Term,
        dfc: &TermContainer,
        tpc: &TermContainer,
        bind: bool,
        set_presentation: bool,
    ) -> SymbolCheckResult {
        tracing::debug!("Checking Definiens");
        //tracing::debug!("Facts: {:#?}", self.facts);
        let (unks, df) = self.prepare(None, df.clone());

        let (tp, unks, mut l) = self.infer_type(Some(unks), &df);
        let mut df = self.wrap_none(Some(unks), |slf| slf.subst(df)).1;

        if df.has_solvable() {
            l.push(PreCheckLog::Msg(
                vec![format!("Unsolved unkowns remain: {:?}", df.solvables()).into()],
                ftml_solver_trace::MessageLevel::Failure,
            ));
            return SymbolCheckResult::DefiniensOnly {
                inferred: None,
                log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
            };
        }

        update!(self df if bind);
        if set_presentation {
            dfc.set_presentation(self.revert_prepare(df.clone()));
        }
        dfc.set_checked(df);

        if let Some(mut tp) = tp {
            if tp.has_solvable() {
                l.push(PreCheckLog::Msg(
                    vec![format!("Unsolved unkowns remain: {:?}", tp.solvables()).into()],
                    ftml_solver_trace::MessageLevel::Failure,
                ));
                return SymbolCheckResult::DefiniensOnly {
                    inferred: None,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                };
            }
            update!(self tp if bind);
            tpc.set_checked(tp.clone());
            let tp = self.revert_prepare(tp);
            tpc.set_presentation(tp.clone());
            SymbolCheckResult::DefiniensOnly {
                inferred: Some(tp),
                log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
            }
        } else {
            SymbolCheckResult::DefiniensOnly {
                inferred: None,
                log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
            }
        }
    }

    fn infer_df(
        &self,
        dfc: &TermContainer,
        tp: &Term,
        tpc: &TermContainer,
        bind: bool,
    ) -> SymbolCheckResult {
        let (tunks, tp) = self.prepare(None, tp.clone());
        let (b, tunks, mut l) = self.check_inhabitable(Some(tunks), &tp);
        if b.is_some_and(|b| !b) {
            let mut tp = self.wrap_none(Some(tunks), |slf| slf.subst(tp)).1;
            update!(self tp if bind);
            tpc.set_checked(tp);
            return SymbolCheckResult::Both {
                inhabitable: TypeCheckResult {
                    success: false,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                },
                matches: None,
            };
        }
        let (tunks, mut tp) = self.wrap_none(Some(tunks), |slf| slf.subst(tp));

        if tp.has_solvable() {
            l.push(PreCheckLog::Msg(
                vec![format!("Unsolved unkowns remain: {:?}", tp.solvables()).into()],
                ftml_solver_trace::MessageLevel::Failure,
            ));
            return SymbolCheckResult::TypeOnly {
                result: TypeCheckResult {
                    success: false,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                },
            };
        }
        let (ndf, _, mut l2) = self.prove(Some(tunks), &tp);

        update!(self tp if bind);
        tpc.set_checked(tp);
        if let Some(mut df) = ndf {
            if df.has_solvable() {
                l2.push(PreCheckLog::Msg(
                    vec![format!("Unsolved unkowns remain: {:?}", df.solvables()).into()],
                    ftml_solver_trace::MessageLevel::Failure,
                ));
                return SymbolCheckResult::Both {
                    inhabitable: TypeCheckResult {
                        success: true,
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    },
                    matches: Some(TypeCheckResult {
                        success: false,
                        log: CheckLog::from_pre(l2, &mut |t| self.revert_prepare(t)),
                    }),
                };
            }

            update!(self df if bind);
            dfc.set_checked(df.clone());
            let df = self.revert_prepare(df);
            dfc.set_presentation(df);
            SymbolCheckResult::Both {
                inhabitable: TypeCheckResult {
                    success: true,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                },
                matches: Some(TypeCheckResult {
                    success: true,
                    log: CheckLog::from_pre(l2, &mut |t| self.revert_prepare(t)),
                }),
            }
        } else {
            SymbolCheckResult::Both {
                inhabitable: TypeCheckResult {
                    success: true,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                },
                matches: Some(TypeCheckResult {
                    success: false,
                    log: CheckLog::from_pre(l2, &mut |t| self.revert_prepare(t)),
                }),
            }
        }
    }

    fn df_and_tp(
        &self,
        df: &Term,
        dfc: &TermContainer,
        tp: &Term,
        tpc: &TermContainer,
        bind: bool,
        set_df_presentation: bool,
    ) -> SymbolCheckResult {
        let (tunks, tp) = self.prepare(None, tp.clone());
        let (b, tunks, mut l) = self.check_inhabitable(Some(tunks), &tp);
        if b.is_some_and(|b| !b) {
            let mut tp = self.wrap_none(Some(tunks), |slf| slf.subst(tp)).1;
            if tp.has_solvable() {
                l.push(PreCheckLog::Msg(
                    vec![format!("Unsolved unkowns remain: {:?}", tp.solvables()).into()],
                    ftml_solver_trace::MessageLevel::Failure,
                ));
                return SymbolCheckResult::TypeOnly {
                    result: TypeCheckResult {
                        success: false,
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    },
                };
            }

            update!(self tp if bind);
            tpc.set_checked(tp);
            return SymbolCheckResult::Both {
                inhabitable: TypeCheckResult {
                    success: false,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                },
                matches: None,
            };
        }
        let (tunks, mut tp) = self.wrap_none(Some(tunks), |slf| slf.subst(tp));
        tracing::debug!("Checking definiens");
        let (dunks, df) = self.prepare(Some(tunks), df.clone());

        let (b, unks, mut l2) = self.check_type(Some(dunks), &df, &tp);
        let (unks, mut df) = self.wrap_none(Some(unks), |slf| slf.subst(df));

        if df.has_solvable() {
            l2.push(PreCheckLog::Msg(
                vec![format!("Unsolved unkowns remain: {:?}", df.solvables()).into()], /*format!(
                                                                                           "Unsolved unkowns remain in {:?}\n\n{unks:#?}",
                                                                                           df.debug_short()
                                                                                       )
                                                                                       .into()*/
                ftml_solver_trace::MessageLevel::Failure,
            ));
            return SymbolCheckResult::Both {
                inhabitable: TypeCheckResult {
                    success: true,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                },
                matches: Some(TypeCheckResult {
                    success: false,
                    log: CheckLog::from_pre(l2, &mut |t| self.revert_prepare(t)),
                }),
            };
        }

        update!(self tp if bind);
        update!(self df if bind);
        tracing::debug!("Result:\n{:?}\n : {:?}", df.debug_short(), tp.debug_short());
        tpc.set_checked(tp);
        if set_df_presentation {
            dfc.set_presentation(self.revert_prepare(df.clone()));
        }
        dfc.set_checked(df);
        SymbolCheckResult::Both {
            inhabitable: TypeCheckResult {
                success: true,
                log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
            },
            matches: Some(TypeCheckResult {
                success: b.unwrap_or(false),
                log: CheckLog::from_pre(l2, &mut |t| self.revert_prepare(t)),
            }),
        }
    }

    // TODO return checked term
    pub fn check_symbol(&mut self, s: &Symbol) -> Option<SymbolCheckResult> {
        tracing::debug!("Checking Symbol {}", s.uri);
        tracing::trace!("{s:?}");
        self.current.push(s.uri.clone().into());
        let r = self.check_components(&s.data.tp, &s.data.df, true);
        self.current.pop();
        self.add_fact(s);
        r
    }

    // TODO return checked term
    pub fn check_variable(&mut self, s: &VariableDeclaration) -> Option<SymbolCheckResult> {
        tracing::debug!("Checking Variable {}", s.uri);
        self.current.push(s.uri.clone().into());
        let r = self.check_components(&s.data.tp, &s.data.df, false);
        self.current.pop();
        r
    }

    fn check_type(
        &self,
        unknowns: Option<Solutions>,
        tm: &Term,
        tp: &Term,
    ) -> (Option<bool>, Solutions, PreCheckLog) {
        self.wrap_task(CheckingTask::HasType(tm, tp), unknowns, |mut slf| {
            slf.check_type_i(tm, tp)
        })
    }

    fn prove(
        &self,
        unknowns: Option<Solutions>,
        goal: &Term,
    ) -> (Option<Term>, Solutions, PreCheckLog) {
        self.wrap_task(CheckingTask::Proving(goal), unknowns, |mut slf| {
            slf.prove(goal)
        })
    }

    fn check_subtype(
        &self,
        unknowns: Option<Solutions>,
        sub: &Term,
        sup: &Term,
    ) -> (Option<bool>, Solutions, PreCheckLog) {
        self.wrap_task(CheckingTask::Subtype(sub, sup), unknowns, |mut slf| {
            slf.check_subtype_i(sub, sup)
        })
    }

    fn infer_type(
        &self,
        unknowns: Option<Solutions>,
        t: &Term,
    ) -> (Option<Term>, Solutions, PreCheckLog) {
        self.wrap_task(CheckingTask::Inference(t), unknowns, |mut slf| {
            slf.infer_type_i(t).map(|t| slf.subst(t))
        })
    }

    fn check_inhabitable(
        &self,
        unknowns: Option<Solutions>,
        t: &Term,
    ) -> (Option<bool>, Solutions, PreCheckLog) {
        self.wrap_task(CheckingTask::Inhabitable(t), unknowns, |mut slf| {
            slf.check_inhabitable_i(t)
        })
    }

    fn prepare(&self, unks: Option<Solutions>, t: Term) -> (Solutions, Term) {
        self.wrap_none(unks, |mut slf| {
            let (sols, r) = slf.prepare(t, None);
            slf.merge_solutions(sols);
            r
        })
    }
    fn revert_prepare(&self, t: Term) -> Term {
        self.wrap_none(None, |slf| slf.revert_prepare(t)).1
    }

    fn check_morphism(&self, m: &Morphism) -> Vec<CheckResult> {
        let mut ret = Vec::new();
        let Some(map) = m.elaboration.get_map() else {
            ret.push(CheckResult::Missing(m.uri.clone().into_module()));
            return ret;
        };
        for d in m.declarations() {
            match d {
                AnyDeclarationRef::Symbol(s) if map.values().any(|(u, b)| *b && *u == s.uri) => {
                    // TODO:
                    // - check that a *refined type* is a subtype of the original type's translation
                    // - check that an *assigned definies* is ???? the original definiens
                    if let Some(r) = self.check_components(&s.data.tp, &s.data.df, true) {
                        ret.push(CheckResult::Content(ContentCheckResult::Symbol(
                            s.uri.clone(),
                            r,
                        )));
                    }
                }
                _ => (),
            }
        }
        ret
    }

    /// #### Errors
    pub fn add_modules(&mut self, modules: Vec<Module>) -> Result<(), ModuleUri> {
        let mut todos = Vec::new();
        for m in modules {
            let _ = self.modules.remove(&m);
            todos.push(m.uri.clone());
            self.modules.insert(m);
        }
        self.set_context(todos)?;
        Ok(())
    }

    /// #### Errors
    pub fn set_context(&mut self, m: Vec<ModuleUri>) -> Result<(), ModuleUri> {
        let new = self.sort(m);
        for i in self.context.len() - new..self.context.len() {
            let uri = &self.context[i];
            let m = match self.get_module(uri).map_err(|()| uri.clone()) {
                Ok(m) => m,
                Err(m) => {
                    if uri.is_top() {
                        return Err(m);
                    }
                    continue;
                }
            };
            if let Err(m) = m.initialize(&mut |uri| self.get_module_like(uri).ok()) {
                if m.is_top() {
                    return Err(m);
                }
                continue;
            }
            self.load_context(&m);
        }
        Ok(())
    }

    fn load_context(&mut self, m: &Module) {
        //, todos: &mut Vec<ModuleUri>) {
        tracing::trace!("Loading: {}", m.uri);
        for d in m.dfs() {
            match d {
                /*
                AnyDeclarationRef::Import { uri: m, .. } if !self.modules.contains(m) => {
                    if m.is_top() {
                        todos.push(m.clone());
                    } else {
                        let uri = !m.clone();
                        if !self.modules.contains(&uri) {
                            todos.push(uri);
                        }
                    }
                } */
                AnyDeclarationRef::Symbol(s) => {
                    let markers = self.rules.marker().len();
                    for e in Split::SYMBOL_EXTRACTORS {
                        e(s, &mut self.rules);
                    }
                    if self.rules.marker().len() > markers {
                        self.set_hoas();
                    }
                    self.add_fact(s);
                }
                AnyDeclarationRef::Rule { id, parameters, .. } => {
                    tracing::debug!("Rule: {id}");
                    let markers = self.rules.marker().len();
                    if let Some(rule) = Split::RULE_EXTRACTORS
                        .iter()
                        .find_map(|(s, f)| if id.as_ref() == *s { Some(f) } else { None })
                    {
                        let parameters = parameters
                            .iter()
                            .map(|t| self.prepare(None, t.clone()).1)
                            .collect::<Vec<_>>();
                        rule(&parameters, &mut self.rules);
                    }
                    if self.rules.marker().len() > markers {
                        self.set_hoas();
                    }
                }
                _ => (),
            }
        }
    }

    fn sort(&mut self, modules: Vec<ModuleUri>) -> usize {
        let mut ctx = std::mem::take(&mut self.context);
        let new = Module::topo_sort(modules, &mut ctx, |uri| self.get_module(uri).ok());
        self.context = ctx;
        new
    }
}
