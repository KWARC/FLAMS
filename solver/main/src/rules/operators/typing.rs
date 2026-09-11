use crate::{
    CheckRef,
    rules::{InferenceRule, PreparationRule, SimplificationRule, SizedSolverRule, SubtypeRule},
    split::SplitStrategy,
};
use ftml_ontology::terms::{
    Argument, ArgumentMode, BindingTerm, BoundArgument, ComponentVar, IsTerm, MaybeSequence, Term,
    Variable, patterns::Pattern,
};
use ftml_uris::SymbolUri;
use std::{hint::unreachable_unchecked, ops::ControlFlow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleTypeOperatorRule(pub SymbolUri);

impl SizedSolverRule for SimpleTypeOperatorRule {
    fn priority(&self) -> isize {
        100_000
    }
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.0, " is a typing operator")
    }
}

impl SimpleTypeOperatorRule {
    fn is_app(&self, t: &Term) -> bool {
        //ftml_ontology::matchtm!(app({sym(=self.0)},[]) = t);
        matches!(
            t,
            Term::Application(app)
            if matches!(
                &app.head,
                Term::Symbol{uri,..} if *uri == self.0
            ) && (matches!(
                app.arguments.first(),
                Some(Argument::Simple(Term::Var{..}))
            ) || matches!(
                app.arguments.first(),
                Some(Argument::Sequence(MaybeSequence::Seq(s)))
                if s.iter().all(|t| matches!(t,Term::Var{..}))
            )) && matches!(app.arguments.get(1),Some(Argument::Simple(_)))
        )
    }
    /// Only safe to call iff `self.is_app(&t)`
    unsafe fn get_var(t: &Term) -> (Vec<Variable>, Term) {
        let Term::Application(a) = t else {
            unsafe { unreachable_unchecked() }
        };
        let Argument::Simple(tp) = a.arguments[1].clone() else {
            unsafe { unreachable_unchecked() }
        };
        (
            match a.arguments[0].clone() {
                Argument::Simple(Term::Var { variable, .. }) => vec![variable],
                Argument::Sequence(MaybeSequence::Seq(vars)) => vars
                    .into_iter()
                    .map(|v| {
                        let Term::Var { variable, .. } = v else {
                            unsafe { unreachable_unchecked() }
                        };
                        variable
                    })
                    .collect(),
                _ => unsafe { unreachable_unchecked() },
            },
            tp,
        )
    }
}
impl<Split: SplitStrategy> PreparationRule<Split> for SimpleTypeOperatorRule {
    fn applicable(&self, checker: &crate::CheckRef<'_, '_, Split>, t: &Term) -> bool {
        let Term::Bound(b) = t else {
            //tracing::trace!("Not bound");
            return false;
        };
        let Some(head) = checker.get_head(t) else {
            return false;
        };
        let spec = head.as_ref().either(|s| &s.data.arity, |v| &v.data.arity);
        if spec.num() as usize != b.arguments.len() {
            //tracing::trace!("Arguments don't match: {spec:?} != {:?}", b.arguments);
            return false;
        }
        spec.iter()
            .zip(b.arguments.iter())
            .any(|(a, b)| match (a, b) {
                (ArgumentMode::BoundVariable, BoundArgument::Simple(t)) => t
                    .head()
                    .is_some_and(|v| matches!(v,either::Left(uri) if *uri == self.0)),
                (
                    ArgumentMode::BoundVariableSequence,
                    BoundArgument::Sequence(MaybeSequence::One(t)),
                ) => t
                    .head()
                    .is_some_and(|v| matches!(v,either::Left(uri) if *uri == self.0)),
                (
                    ArgumentMode::BoundVariableSequence,
                    BoundArgument::Sequence(MaybeSequence::Seq(ts)),
                ) => ts.iter().any(|t| {
                    t.head()
                        .is_some_and(|v| matches!(v,either::Left(uri) if *uri == self.0))
                }),
                _ => false,
            })
    }
    fn apply(
        &self,
        checker: &mut CheckRef<'_, '_, Split>,
        t: Term,
        _: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> ControlFlow<Term, Term> {
        let Some(head) = checker.get_head(&t) else {
            return ControlFlow::Continue(t);
        };
        let Term::Bound(b) = t else {
            unreachable!("wut");
        };
        let spec = head.as_ref().either(|s| &s.data.arity, |v| &v.data.arity);

        let nargs = spec
            .iter()
            .zip(b.arguments.clone())
            .map(|(m, a)| match (m, a) {
                (ArgumentMode::BoundVariable, BoundArgument::Simple(t)) => {
                    if self.is_app(&t) {
                        // SAFETY: is_app
                        let (mut v, tp) = unsafe { Self::get_var(&t) };
                        if v.len() == 1 {
                            // SAFETY: len==1
                            let var = unsafe { v.pop().unwrap_unchecked() };
                            BoundArgument::Bound(ComponentVar {
                                var,
                                tp: Some(tp),
                                df: None,
                            })
                        } else {
                            BoundArgument::Simple(t)
                        }
                    } else {
                        BoundArgument::Simple(t)
                    }
                }
                (
                    ArgumentMode::BoundVariableSequence,
                    BoundArgument::Sequence(MaybeSequence::Seq(s)),
                ) => {
                    let mut works = true;
                    let mut types = Vec::new();
                    let ns = s
                        .into_iter()
                        .flat_map(|t| {
                            if self.is_app(&t) {
                                // SAFETY: is_app
                                let (v, tp) = unsafe { Self::get_var(&t) };
                                for _ in &v {
                                    types.push(tp.clone());
                                }
                                v.into_iter()
                                    .map(|v| Term::Var {
                                        variable: v,
                                        presentation: None,
                                    })
                                    .collect::<Vec<_>>()
                            } else {
                                works = false;
                                vec![t]
                            }
                        })
                        .collect::<Vec<_>>();
                    if works {
                        BoundArgument::BoundSeq(MaybeSequence::Seq(
                            ns.into_iter()
                                .zip(types)
                                .map(|(t, tp)| {
                                    let Term::Var { variable, .. } = t else {
                                        // SAFETY: works == true
                                        unsafe { unreachable_unchecked() }
                                    };
                                    ComponentVar {
                                        var: variable,
                                        tp: Some(tp),
                                        df: None,
                                    }
                                })
                                .collect(),
                        ))
                    } else {
                        BoundArgument::Sequence(MaybeSequence::Seq(ns.into_boxed_slice()))
                    }
                }
                (m, a) => {
                    //tracing::trace!("Other: {m:?} = {a:?}");
                    a
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        ControlFlow::Continue(Term::Bound(BindingTerm::new(
            b.head.clone(),
            nargs,
            b.presentation.clone(),
        )))
    }
    fn applicable_revert(&self, _: &CheckRef<'_, '_, Split>, _: &Term) -> bool {
        false
    }
    fn revert(&self, _: &CheckRef<'_, '_, Split>, t: Term) -> ControlFlow<Term, Term> {
        ControlFlow::Continue(t)
    }
}

impl<Split: SplitStrategy> SimplificationRule<Split> for SimpleTypeOperatorRule {
    fn applicable(&self, term: &Term) -> bool {
        <Self as InferenceRule<Split>>::applicable(&self, term)
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        let Term::Application(app) = term else {
            return Err(None);
        };
        let Term::Symbol { uri, .. } = &app.head else {
            return Err(None);
        };
        if *uri != self.0 {
            return Err(None);
        }
        let (tm, tp) = match &*app.arguments {
            [Argument::Simple(tm), Argument::Simple(tp)] => (tm, tp),
            [
                Argument::Sequence(MaybeSequence::Seq(tms)),
                Argument::Simple(tp),
            ] => match &**tms {
                [tm] => (tm, tp),
                _ => return Err(None),
            },
            _ => return Err(None),
        };
        if checker.check_type(tm, tp) != Some(true) {
            return Err(None);
        }
        Ok(tm.clone())
    }
}

impl<Split: SplitStrategy> InferenceRule<Split> for SimpleTypeOperatorRule {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term
            && let Term::Symbol { uri, .. } = &app.head
            && *uri == self.0
        {
            matches!(&*app.arguments, [Argument::Simple(_), Argument::Simple(_)])
                || matches!(&*app.arguments,[Argument::Sequence(MaybeSequence::Seq(v)),Argument::Simple(_)] if v.len() == 1)
        } else {
            false
        }
    }
    fn infer<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        let Term::Application(app) = term else {
            return None;
        };
        let Term::Symbol { uri, .. } = &app.head else {
            return None;
        };
        if *uri != self.0 {
            return None;
        }
        let (tm, tp) = match &*app.arguments {
            [Argument::Simple(tm), Argument::Simple(tp)] => (tm, tp),
            [
                Argument::Sequence(MaybeSequence::Seq(tms)),
                Argument::Simple(tp),
            ] => match &**tms {
                [tm] => (tm, tp),
                _ => return None,
            },
            _ => return None,
        };
        if checker.check_type(tm, tp) != Some(true) {
            return None;
        }
        Some(tp.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtyping {
    pub sub: Pattern,
    pub sup: Pattern,
}
impl SizedSolverRule for Subtyping {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(
            self.sub.body.clone(),
            "is a subtype of",
            self.sup.body.clone()
        )
    }
}
impl<Split: SplitStrategy> SubtypeRule<Split> for Subtyping {
    fn applicable(&self, checker: &CheckRef<'_, '_, Split>, sub: &Term, sup: &Term) -> bool {
        /*println!(
            "Applicable? ({:?}   <:   {:?})\n {:?}   <:   {:?}",
            self.sub.body.debug_short(),
            self.sup.body.debug_short(),
            sub.debug_short(),
            sup.debug_short()
        );*/
        let Some(sub) = self.sub.matches(sub) else {
            return false;
        };
        let Some(sup) = self.sup.matches(sup) else {
            return false;
        };
        for (i, j) in self
            .sub
            .vars
            .iter()
            .enumerate()
            .filter_map(|(i, v)| Some((i, self.sup.vars.iter().position(|v2| v2 == v)?)))
        {
            let Some(sub) = sub.get(i) else { return false };
            let Some(sup) = sup.get(j) else { return false };
            if !sub.alpha_equal(sup) {
                return false;
            }
        }
        //println!("Applicable!");
        true
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        sub: &'t Term,
        sup: &'t Term,
    ) -> Option<bool> {
        // sanity check
        checker.check_inhabitable(sub)?;
        checker.check_inhabitable(sup)?;
        Some(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InferredTypeSimplificationRule;

impl SizedSolverRule for InferredTypeSimplificationRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&*ftml_uris::metatheory::TYPE_OF, " infers type")
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for InferredTypeSimplificationRule {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term
            && app.head.is(&*ftml_uris::metatheory::TYPE_OF)
            && let [Argument::Simple(_)] = &*app.arguments
        {
            true
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        let Term::Application(app) = term else {
            return Err(None);
        };
        let [Argument::Simple(t)] = &*app.arguments else {
            return Err(None);
        };
        checker.infer_type(t).ok_or(None)
    }
}
