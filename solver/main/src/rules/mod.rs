pub mod extractors;
pub mod fixity;
pub mod implicits;
pub mod operators;
pub mod sequences;
pub mod symbols;
pub mod unknowns;
pub use ftml_solver_trace::{CheckerRule, SizedSolverRule};
use ftml_uris::SymbolUri;

use crate::{CheckRef, rules::operators::typing, split::SplitStrategy};
use ftml_ontology::{
    domain::{
        SharedDeclaration,
        declarations::{SharedSymbolLike, morphisms::Morphism},
    },
    terms::{Argument, Term, helpers::IntoTerm, termpaths::TermPath},
};
use std::{fmt::Debug, ops::ControlFlow};

macro_rules! rules{
    ($($name:ident = $tp:ident $(($($e:expr),* $(,)?))? ),*$(,)?) => {
        #[derive(Debug)]
        pub struct RuleSet<Split: SplitStrategy> {
            $(
                $name:Vec<Box<dyn $tp<Split>>>
            ),*
        }

        paste::paste!{
            $(
            trait [< ClonableDyn $tp>]<Split:SplitStrategy>:$tp<Split> {
                fn [<to_dyn_ $name>](&self) -> Box<dyn $tp<Split>>;
            }
            impl<Split:SplitStrategy,T:$tp<Split>+Sized+Clone> [< ClonableDyn $tp>]<Split> for T {
                fn [<to_dyn_ $name>](&self) -> Box<dyn $tp<Split>> {
                    Box::new(self.clone()) as _
                }
            }
            )*
        }


        impl<Split:SplitStrategy> Default for RuleSet<Split> {
            paste::paste!{
                fn default() -> Self {
                    Self {
                        $(
                            $name: Self::[<DEFAULT_ $name:snake:upper >].iter().map(|r| r.[<to_dyn_ $name>]()).collect()
                        ),*
                    }
                }
            }
        }

        impl<Split: SplitStrategy> RuleSet<Split> {
            paste::paste!{
                $(

                    const [<DEFAULT_ $name:snake:upper >] : &[&dyn [< ClonableDyn $tp>]<Split>] = &[
                        $($(
                            &$e as _
                        ),*)?
                    ];

                    pub fn [<push_ $name>](&mut self,rule:Box<dyn $tp<Split>>) {
                        if !self.$name.iter().any(|v| v.eq(&*rule)) {
                            let i = self
                                .$name
                                .binary_search_by_key(&(-rule.priority()), |e| -e.priority())
                                .map_or_else(|i| i, |i| i);
                            self.$name.insert(i, rule);
                        }
                    }
                    #[inline]
                    #[must_use]
                    pub fn $name(&self) -> &[Box<dyn $tp<Split>>] {
                        &self.$name
                    }
                )*
            }
        }
    }
}

rules! {
    inference = InferenceRule(
        sequences::SeqIndexRule,
        sequences::SeqInferenceRule,
        sequences::fold::FoldInferenceRule,
        sequences::SeqConcatInferenceRule,
        operators::numbers::NumberTypes,
        implicits::ImplicitRule,
        unknowns::UnknownsRule,
        CommentRule,
        MorphismRule,
        super::impls::records::FieldRule,
        ProofBarrier
    ),
    subtyping = SubtypeRule(
        operators::numbers::NumberTypes,
        super::impls::records::RecordRule,
        CommentRule
    ),
    checking = CheckingRule(operators::numbers::NumberTypes),
    inhabitable = InhabitableRule(
        sequences::SeqUniverseRule,
        super::impls::records::RecordRule,
        CommentRule,
    ),
    equality = EqualityRule(
        operators::numbers::NumberTypes,
        CommentRule,
        ProofBarrier
        //sequences::SeqTypeEqRule
    ),
    universe = UniverseRule(sequences::SeqUniverseRule,CommentRule),
    preparation = PreparationRule,
    simplification = SimplificationRule(
        unknowns::UnknownsRule,
        typing::InferredTypeSimplificationRule,
        CommentRule,
        MorphismRule,
        super::impls::records::FieldRule
    ),
    marker = MarkerRule,
    proof = ProofRule
}

pub trait SimplificationRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, term: &Term) -> bool;
    fn apply<'t>(
        &self,
        checker: CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<TermPath>>;
}

pub trait EqualityRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, lhs: &Term, rhs: &Term) -> bool;
    fn apply<'t>(
        &self,
        checker: CheckRef<'t, '_, Split>,
        lhs: &'t Term,
        rhs: &'t Term,
    ) -> Option<bool>;
}

pub trait InferenceRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, term: &Term) -> bool;
    fn infer<'t>(&self, checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term>;
}

pub trait CheckingRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, checker: &CheckRef<'_, '_, Split>, term: &Term, tp: &Term) -> bool;
    fn apply<'t>(
        &self,
        checker: CheckRef<'t, '_, Split>,
        term: &'t Term,
        tp: &'t Term,
    ) -> Option<bool>;
}

pub trait InhabitableRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, term: &Term) -> bool;
    fn apply<'t>(&self, checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool>;
}

pub trait UniverseRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, term: &Term) -> bool;
    fn apply<'t>(&self, checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool>;
}

pub trait SubtypeRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, checker: &CheckRef<'_, '_, Split>, sub: &Term, sup: &Term) -> bool;
    fn apply<'t>(
        &self,
        checker: CheckRef<'t, '_, Split>,
        sub: &'t Term,
        sup: &'t Term,
    ) -> Option<bool>;
}

pub trait PreparationRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, checker: &CheckRef<'_, '_, Split>, t: &Term) -> bool;
    fn apply(
        &self,
        checker: &mut CheckRef<'_, '_, Split>,
        t: Term,
        path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> ControlFlow<Term, Term>;
    fn applicable_revert(&self, checker: &CheckRef<'_, '_, Split>, t: &Term) -> bool;
    fn revert(&self, checker: &CheckRef<'_, '_, Split>, t: Term) -> ControlFlow<Term, Term>;
}
pub trait MarkerRule<Split: SplitStrategy>: CheckerRule {}

pub trait ProofRule<Split: SplitStrategy>: CheckerRule {
    fn applicable(&self, term: &Term) -> bool;
    fn prove<'t>(&self, checker: CheckRef<'t, '_, Split>, goal: &'t Term) -> Option<Term>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsJudgmentRule(pub SymbolUri);
impl SizedSolverRule for IsJudgmentRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.0, "is a judgment")
    }
}
impl<Split: SplitStrategy> MarkerRule<Split> for IsJudgmentRule {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HOASRule {
    pub lambda: SymbolUri,
    pub pi: SymbolUri,
    pub apply: Option<SymbolUri>,
}
impl SizedSolverRule for HOASRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("HOAS using ", &self.lambda, " and ", &self.pi)
    }
}
impl<Split: SplitStrategy> MarkerRule<Split> for HOASRule {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommentRule;
impl SizedSolverRule for CommentRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("comment")
    }
    fn priority(&self) -> isize {
        1_000_000
    }
}

fn as_comment(term: &Term) -> Option<&Term> {
    if let Term::Application(app) = term
        && app.head.is(&*ftml_uris::metatheory::COMMENTED)
        && let [Argument::Simple(r), Argument::Simple(_)] = &*app.arguments
    {
        Some(r)
    } else {
        None
    }
}

impl<Split: SplitStrategy> SimplificationRule<Split> for CommentRule {
    fn applicable(&self, term: &Term) -> bool {
        as_comment(term).is_some()
    }
    fn apply<'t>(
        &self,
        _: CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<TermPath>> {
        as_comment(term).cloned().ok_or(None)
    }
}

impl<Split: SplitStrategy> InferenceRule<Split> for CommentRule {
    fn applicable(&self, term: &Term) -> bool {
        as_comment(term).is_some()
    }
    fn infer<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        as_comment(term).and_then(|t| checker.infer_type(t))
    }
}

impl<Split: SplitStrategy> InhabitableRule<Split> for CommentRule {
    fn applicable(&self, term: &Term) -> bool {
        as_comment(term).is_some()
    }
    fn apply<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool> {
        as_comment(term).and_then(|t| checker.check_inhabitable(t))
    }
}

impl<Split: SplitStrategy> UniverseRule<Split> for CommentRule {
    fn applicable(&self, term: &Term) -> bool {
        as_comment(term).is_some()
    }
    fn apply<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool> {
        as_comment(term).and_then(|t| checker.check_universe(t))
    }
}
impl<Split: SplitStrategy> SubtypeRule<Split> for CommentRule {
    fn applicable(&self, _: &CheckRef<'_, '_, Split>, sub: &Term, sup: &Term) -> bool {
        as_comment(sub).is_some() || as_comment(sup).is_some()
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        sub: &'t Term,
        sup: &'t Term,
    ) -> Option<bool> {
        // one of them is different, because .applicable()
        let sub = as_comment(sub).unwrap_or(sub);
        let sup = as_comment(sup).unwrap_or(sup);
        checker.check_subtype(sub, sup)
    }
}

impl<Split: SplitStrategy> EqualityRule<Split> for CommentRule {
    fn applicable(&self, lhs: &Term, rhs: &Term) -> bool {
        as_comment(lhs).is_some() || as_comment(rhs).is_some()
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        lhs: &'t Term,
        rhs: &'t Term,
    ) -> Option<bool> {
        // one of them is different, because .applicable()
        let lhs = as_comment(lhs).unwrap_or(lhs);
        let rhs = as_comment(rhs).unwrap_or(rhs);
        checker.check_subtype(lhs, rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MorphismRule;
impl SizedSolverRule for MorphismRule {
    fn priority(&self) -> isize {
        100_000
    }
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("morphism rule")
    }
}
impl MorphismRule {
    pub fn is_morphism_appl<'t, Split: SplitStrategy>(
        t: &'t Term,
        checker: &mut CheckRef<'_, '_, Split>,
    ) -> Option<(SharedDeclaration<Morphism>, &'t Term)> {
        Morphism::unapply(t, &mut |head| {
            checker
                .top
                .get_symbol_like(head, |t| checker.prepare(t, None).1)
                .ok()
        })
    }
}
impl<Split: SplitStrategy> InferenceRule<Split> for MorphismRule {
    fn applicable(&self, term: &Term) -> bool {
        match term {
            Term::Application(app) if app.arguments.len() == 1 => {
                matches!(&app.head, Term::Symbol { .. })
                    && matches!(app.arguments.first(), Some(Argument::Simple(_)))
            }
            _ => false,
        }
    }
    fn infer<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        let (m, arg) = Self::is_morphism_appl(term, &mut checker)?;
        let arg_tp = checker.infer_type(arg)?;
        m.apply(&arg_tp, &mut |s| {
            checker
                .top
                .get_symbol_like(s, |t| checker.prepare(t, None).1)
                .ok()
        })
        .ok()
        .map(std::borrow::Cow::into_owned)
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for MorphismRule {
    fn applicable(&self, t: &Term) -> bool {
        match t {
            Term::Application(app) if app.arguments.len() == 1 => {
                matches!(&app.head, Term::Symbol { .. })
                    && matches!(app.arguments.first(), Some(Argument::Simple(_)))
            }
            _ => false,
        }
    }
    fn apply(
        &self,
        mut checker: CheckRef<'_, '_, Split>,
        t: &Term,
    ) -> Result<Term, Option<TermPath>> {
        tracing::debug!("Morphism? {:?}", t.debug_short());
        let (m, arg) = Self::is_morphism_appl(t, &mut checker).ok_or(None)?;
        tracing::debug!("Applying morphism to {:?}", arg.debug_short());
        m.apply(arg, &mut |s| {
            checker
                .top
                .get_symbol_like(s, |t| checker.prepare(t, None).1)
                .ok()
        })
        .map_or(Err(None), |a| {
            if *a == *t {
                //println!("Not applicable: {} to {:?}", m.uri, arg.debug_short());
                Err(None)
            } else {
                tracing::debug!("Result: {:?}", a.debug_short());
                Ok(a.into_owned())
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProofBarrier;
impl SizedSolverRule for ProofBarrier {
    fn priority(&self) -> isize {
        100_000
    }
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("proof barrier")
    }
}
impl ProofBarrier {
    pub fn proof_for(t: &Term) -> Option<&Term> {
        if let Term::Application(t) = t
            && let Term::Symbol { uri, .. } = &t.head
            && *uri == *symbols::PROOF_BARRIER
            && let [Argument::Simple(_), Argument::Simple(prop)] = &*t.arguments
        {
            return Some(prop);
        }
        None
    }
    pub fn apply(df: Term, tp: Term) -> Term {
        symbols::PROOF_BARRIER.clone().apply_tms([df, tp])
    }
}
impl<Split: SplitStrategy> InferenceRule<Split> for ProofBarrier {
    fn applicable(&self, term: &Term) -> bool {
        Self::proof_for(term).is_some()
    }
    fn infer<'t>(&self, _: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        Self::proof_for(term).cloned()
    }
}
impl<Split: SplitStrategy> EqualityRule<Split> for ProofBarrier {
    // Proof Irrelevance
    fn applicable(&self, lhs: &Term, rhs: &Term) -> bool {
        Self::proof_for(lhs).is_some() && Self::proof_for(rhs).is_some()
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        lhs: &'t Term,
        rhs: &'t Term,
    ) -> Option<bool> {
        let lhs = Self::proof_for(lhs)?;
        let rhs = Self::proof_for(rhs)?;
        Some(checker.check_equality(lhs, rhs) == Some(true))
    }
}
