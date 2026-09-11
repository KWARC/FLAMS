use crate::{
    CheckRef,
    impls::solving::TermExtSolvable,
    rules::{
        CheckingRule, InferenceRule, InhabitableRule, MarkerRule, PreparationRule,
        SimplificationRule, SizedSolverRule, SubtypeRule, UniverseRule,
        operators::numbers::{NumberRule, NumberType},
    },
    split::SplitStrategy,
};
use ftml_ontology::terms::{
    ApplicationTerm, Argument, BindingTerm, BoundArgument, ComponentVar, MaybeSequence, Term,
    Variable, sequences::SequenceType,
};
use ftml_solver_trace::traceref;
use ftml_uris::SymbolUri;
use smallvec::SmallVec;
use std::{borrow::Cow, hint::unreachable_unchecked};

#[allow(unpredictable_function_pointer_comparisons)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiExtensionRule<Split: SplitStrategy> {
    pub extension: SymbolUri,
    pub pi: SymbolUri,
    pub applicable: fn(&Self, &Term, &Argument) -> bool,
    pub infer: for<'t> fn(
        &Self,
        &super::pi::PiInferenceRule,
        &mut CheckRef<'t, '_, Split>,
        &Term,
        &'t [Argument],
        &mut usize,
    ) -> Option<Term>,
}
impl<Split: SplitStrategy> SizedSolverRule for PiExtensionRule<Split> {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.extension, "is extension of", &self.pi)
    }
}
impl<Split: SplitStrategy> MarkerRule<Split> for PiExtensionRule<Split> {}

macro_rules! ret_i {
    ($(;)?) => {};
    (& $e:expr; $($tk:tt)*) => {
        if !$e {
            return None;
        }
        ret_i!($($tk)*)
    };
    ($p:pat = $e:expr; $($tk:tt)*) => {
        let $p = $e else {
            return None;
        };
        ret_i!($($tk)*)
    };

}
macro_rules! ret {
    ( $($tk:tt)* ) => {
        ret_i!($($tk)*;)
    };
}

pub(crate) fn destruct_binder<'t>(
    t: &'t Term,
    head: &SymbolUri,
) -> Option<(either::Either<&'t ComponentVar, &'t ComponentVar>, &'t Term)> {
    ret!(
        Term::Bound(b) = t;
        & b.arguments.len() == 2;
        & matches!(&b.head,Term::Symbol { uri, .. } if *uri == *head);
        //Some(BoundArgument::Bound(v)) = b.arguments.first();
        Some(BoundArgument::Simple(body)) = b.arguments.get(1);
    );
    let v = if let Some(BoundArgument::Bound(v)) = b.arguments.first() {
        either::Left(v)
    } else if let Some(BoundArgument::BoundSeq(MaybeSequence::One(s))) = b.arguments.first() {
        either::Right(s)
    } else {
        return None;
    };
    Some((v, body))
}
fn construct_binder(var: ComponentVar, body: Term, head: &SymbolUri) -> Term {
    Term::Bound(BindingTerm::new(
        Term::Symbol {
            uri: head.clone(),
            presentation: None,
        },
        Box::new([BoundArgument::Bound(var), BoundArgument::Simple(body)]),
        None,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowRule {
    pub arrow: SymbolUri,
    pub pi: SymbolUri,
}
impl SizedSolverRule for ArrowRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.arrow, " is simple version of ", &self.pi)
    }
    fn priority(&self) -> isize {
        100_000_000 // SimpleTypeOperatorRule::priority() * 100
    }
}
impl ArrowRule {
    fn go<Split: SplitStrategy>(
        &self,
        t: &Term,
        mut path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> Option<Term> {
        let Term::Application(app) = t else {
            return None;
        };
        let Some(Argument::Simple(ret)) = app.arguments.last() else {
            return None;
        };
        let args = &app.arguments[..app.arguments.len() - 1];
        if args.is_empty() {
            return Some(ret.clone());
        }
        let mut count = 0u16;
        let args = args.iter().map(|a| match a {
            Argument::Simple(t) => {
                if let Some((v, i)) = path.as_mut()
                    && v.get(*i).copied() == Some(count as u8)
                {
                    v.insert(*i + 1, 0);
                }
                let (v, idx) = ret.fresh_variable(&crate::DUMMY, Some(count));
                if let Some(idx) = idx {
                    count = idx + 1;
                } else {
                    count += 1;
                }
                BoundArgument::Bound(ComponentVar {
                    var: v,
                    tp: Some(t.clone()),
                    df: None,
                })
            }
            Argument::Sequence(MaybeSequence::One(t)) => {
                if let Some((v, i)) = path.as_mut()
                    && v.get(*i).copied() == Some(count as u8)
                {
                    v.insert(*i + 1, 0);
                }
                let (v, idx) = ret.fresh_variable(&crate::DUMMY, Some(count));
                if let Some(idx) = idx {
                    count = idx + 1;
                } else {
                    count += 1;
                }
                BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                    var: v,
                    tp: Some(t.clone()),
                    df: None,
                }))
            }
            Argument::Sequence(MaybeSequence::Seq(ts)) => {
                if let Some((v, i)) = path.as_mut()
                    && v.get(*i).copied() == Some(count as u8)
                {
                    v.insert(*i + 2, 0);
                }
                BoundArgument::BoundSeq(MaybeSequence::Seq(
                    ts.iter()
                        .map(|t| {
                            let (v, idx) = ret.fresh_variable(&crate::DUMMY, Some(count));
                            if let Some(idx) = idx {
                                count = idx + 1;
                            } else {
                                count += 1;
                            }
                            ComponentVar {
                                var: v,
                                tp: Some(t.clone()),
                                df: None,
                            }
                        })
                        .collect(),
                ))
            }
        });
        let ret = Term::Bound(BindingTerm::new(
            Term::Symbol {
                uri: self.pi.clone(),
                presentation: None,
            },
            args.chain([BoundArgument::Simple(ret.clone())]).collect(),
            None,
        ));
        Some(ret)
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for ArrowRule {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term {
            matches!(&app.head,Term::Symbol{uri,..} if *uri == self.arrow)
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        _: CheckRef<'t, '_, Split>,
        t: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        self.go::<Split>(t, None).ok_or(None)
    }
}
impl<Split: SplitStrategy> PreparationRule<Split> for ArrowRule {
    fn applicable(&self, _: &CheckRef<'_, '_, Split>, t: &Term) -> bool {
        <Self as SimplificationRule<Split>>::applicable(self, t)
    }
    fn applicable_revert(&self, _: &CheckRef<'_, '_, Split>, t: &Term) -> bool {
        if let Term::Bound(bind) = t {
            matches!(&bind.head,Term::Symbol { uri, .. } if *uri == self.pi)
        } else {
            false
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn apply(
        &self,
        _: &mut CheckRef<'_, '_, Split>,
        t: Term,
        path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> std::ops::ControlFlow<Term, Term> {
        std::ops::ControlFlow::Continue(self.go::<Split>(&t, path).unwrap_or(t))
    }

    fn revert(&self, _: &CheckRef<'_, '_, Split>, t: Term) -> std::ops::ControlFlow<Term, Term> {
        let Term::Bound(app) = t else {
            return std::ops::ControlFlow::Continue(t);
        };
        let Some(BoundArgument::Simple(ret)) = app.arguments.last() else {
            return std::ops::ControlFlow::Continue(Term::Bound(app));
        };
        let args = &app.arguments[..app.arguments.len() - 1];
        let mut free_vars = args.iter().map(|a| a.free_variables()).collect::<Vec<_>>();
        free_vars.push(ret.free_variables());

        let args: Result<Vec<Argument>, ()> = args
            .iter()
            .enumerate()
            .map(|(i, a)| match a {
                BoundArgument::Bound(ComponentVar {
                    var,
                    tp: Some(tp),
                    df: None,
                }) if !free_vars[i + 1..]
                    .iter()
                    .flatten()
                    .any(|v| v.name() == var.name()) =>
                {
                    Ok(Argument::Simple(tp.clone()))
                }
                BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                    var,
                    tp: Some(tp),
                    df: None,
                })) if !free_vars[i + 1..]
                    .iter()
                    .flatten()
                    .any(|v| v.name() == var.name()) =>
                {
                    Ok(Argument::Sequence(MaybeSequence::One(tp.clone())))
                }
                BoundArgument::BoundSeq(MaybeSequence::Seq(vs)) => {
                    let seq: Result<Box<[Term]>, ()> = vs
                        .iter()
                        .map(|v| {
                            if let ComponentVar {
                                var,
                                tp: Some(tp),
                                df: None,
                            } = v
                                && !free_vars[i + 1..]
                                    .iter()
                                    .flatten()
                                    .any(|v| v.name() == var.name())
                            {
                                Ok(tp.clone())
                            } else {
                                Err(())
                            }
                        })
                        .collect();
                    Ok(Argument::Sequence(MaybeSequence::Seq(seq?)))
                }
                _ => Err(()),
            })
            .collect();
        drop(free_vars);
        std::ops::ControlFlow::Continue(match args {
            Err(()) => Term::Bound(app),
            Ok(mut args) => {
                args.push(Argument::Simple(ret.clone()));
                Term::Application(ApplicationTerm::new(
                    Term::Symbol {
                        uri: self.arrow.clone(),
                        presentation: None,
                    },
                    args.into_boxed_slice(),
                    None,
                ))
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeedsTypeRule(pub SymbolUri);
impl SizedSolverRule for NeedsTypeRule {
    fn display(&self) -> Vec<ftml_solver_trace::Displayable> {
        ftml_solver_trace::trace!(&self.0, "needs typed variables")
    }
}
impl<Split: SplitStrategy> PreparationRule<Split> for NeedsTypeRule {
    fn applicable(&self, _: &CheckRef<'_, '_, Split>, t: &Term) -> bool {
        if let Term::Bound(b) = t
            && let Term::Symbol { uri, .. } = &b.head
            && *uri == self.0
        {
            b.arguments.iter().any(|a| matches!(a,BoundArgument::Bound(ComponentVar { tp:None, .. })|BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar { tp:None, .. })))
                || matches!(a,BoundArgument::BoundSeq(MaybeSequence::Seq(seq)) if seq.iter().any(|a| matches!(a,ComponentVar { tp:None, .. }))))
        } else {
            false
        }
    }
    fn applicable_revert(&self, _: &CheckRef<'_, '_, Split>, _: &Term) -> bool {
        false
    }
    fn apply(
        &self,
        checker: &mut CheckRef<'_, '_, Split>,
        t: Term,
        path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> std::ops::ControlFlow<Term, Term> {
        let Term::Bound(b) = t else {
            return std::ops::ControlFlow::Continue(t);
        };
        let arguments = &b.arguments;
        let ret = checker.scoped(|checker| {
            let mut changed = false;
            let nargs = arguments
                .iter()
                .map(|a| match a {
                    BoundArgument::Bound(ComponentVar { var, tp: None, df }) => {
                        if checker.infer_var_type(var).is_none() {
                            changed = true;
                            let v = checker.new_solvable();
                            Cow::Owned(BoundArgument::Bound(ComponentVar {
                                var: var.clone(),
                                tp: Some(v),
                                df: df.clone(),
                            }))
                        } else {
                            Cow::Borrowed(a)
                        }
                    }
                    BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                        var,
                        tp: None,
                        df,
                    })) => {
                        if checker.infer_var_type(var).is_none() {
                            changed = true;
                            let v = checker.new_solvable();
                            Cow::Owned(BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                                var: var.clone(),
                                tp: Some(v),
                                df: df.clone(),
                            })))
                        } else {
                            Cow::Borrowed(a)
                        }
                    }
                    BoundArgument::BoundSeq(MaybeSequence::Seq(vs)) => {
                        let nvs = vs
                            .iter()
                            .map(|cv @ ComponentVar { var, tp, df }| {
                                if tp.is_none() && checker.infer_var_type(var).is_none() {
                                    changed = true;
                                    let v = checker.new_solvable();
                                    Cow::Owned(ComponentVar {
                                        var: var.clone(),
                                        tp: Some(v),
                                        df: df.clone(),
                                    })
                                } else {
                                    Cow::Borrowed(cv)
                                }
                            })
                            .collect::<Vec<_>>();
                        if changed {
                            Cow::Owned(BoundArgument::BoundSeq(MaybeSequence::Seq(
                                nvs.into_iter().map(Cow::into_owned).collect(),
                            )))
                        } else {
                            Cow::Borrowed(a)
                        }
                    }
                    _ => Cow::Borrowed(a),
                })
                .collect::<Vec<_>>();
            if changed {
                Some(Term::Bound(BindingTerm::new(
                    b.head.clone(),
                    nargs.into_iter().map(Cow::into_owned).collect(),
                    b.presentation.clone(),
                )))
            } else {
                None
            }
        });
        std::ops::ControlFlow::Continue(ret.unwrap_or(Term::Bound(b)))
    }

    fn revert(&self, _: &CheckRef<'_, '_, Split>, t: Term) -> std::ops::ControlFlow<Term, Term> {
        std::ops::ControlFlow::Continue(t)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiVarianceRule(pub SymbolUri);

impl SizedSolverRule for PiVarianceRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(
            &self.0,
            " is contravariant in arguments and covariant in its return type"
        )
    }
}
impl<Split: SplitStrategy> SubtypeRule<Split> for PiVarianceRule {
    fn applicable(&self, _: &CheckRef<'_, '_, Split>, sub: &Term, sup: &Term) -> bool {
        if let Term::Bound(b) = sub
            && let Term::Bound(b2) = sup
            && let Term::Symbol { uri, .. } = &b.head
            && let Term::Symbol { uri: uri2, .. } = &b2.head
        {
            *uri == self.0 && *uri2 == self.0 && b.arguments.len() == b2.arguments.len()
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        sub: &'t Term,
        sup: &'t Term,
    ) -> Option<bool> {
        let Term::Bound(sub) = sub else { return None };
        let Term::Bound(sup) = sup else { return None };
        let Some(BoundArgument::Simple(subret)) = sub.arguments.last() else {
            return None;
        };
        let Some(BoundArgument::Simple(supret)) = sup.arguments.last() else {
            return None;
        };
        let mut currsub = Vec::<(&str, Term)>::new();
        for (sub, sup) in sub.arguments[..sub.arguments.len() - 1]
            .iter()
            .zip(sup.arguments[..sup.arguments.len() - 1].iter())
        {
            match (sub, sup) {
                (
                    BoundArgument::Bound(ComponentVar {
                        var: varsub,
                        tp: Some(sub),
                        ..
                    }),
                    BoundArgument::Bound(ComponentVar {
                        var: varsup,
                        tp: Some(sup),
                        ..
                    }),
                ) => {
                    let sup = sup / &*currsub;
                    if !checker.scoped(|checker| checker.check_subtype(&sup, sub))? {
                        return None;
                    }
                    currsub.push((varsup.name(), varsub.clone().into()));
                    checker.extend_context(ComponentVar {
                        var: varsub.clone(),
                        tp: Some(sub.clone()),
                        df: None,
                    });
                }
                (
                    BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                        var: varsub,
                        tp: Some(sub),
                        ..
                    })),
                    BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                        var: varsup,
                        tp: Some(sup),
                        ..
                    }))
                    | BoundArgument::Bound(ComponentVar {
                        var: varsup,
                        tp: Some(sup),
                        ..
                    }),
                ) => {
                    let sup = sup / &*currsub;
                    if !checker.scoped(|checker| checker.check_subtype(&sup, sub))? {
                        return None;
                    }
                    currsub.push((varsup.name(), varsub.clone().into()));
                    checker.extend_context(ComponentVar {
                        var: varsub.clone(),
                        tp: Some(sub.clone()),
                        df: None,
                    });
                }
                _ => {
                    checker.failure("argument not bound single variable or variable sequence");
                    //checker.failure(format!("{sub:?}  vs  {sup:?}"));
                    return None;
                }
            }
        }
        let supret = supret / &*currsub;
        checker.scoped(|checker| checker.check_subtype(subret, &supret))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LambdaPiInferenceRule {
    pub lambda: SymbolUri,
    pub pi: SymbolUri,
}
impl SizedSolverRule for LambdaPiInferenceRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(
            "{ x:A, B(x):T } ⊢ (",
            &self.lambda,
            " x:A. B) :=> ",
            &self.pi,
            " x:A. T"
        )
    }
}
impl LambdaPiInferenceRule {
    fn infer_simple<'t, Split: SplitStrategy>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        var: &'t ComponentVar,
        body: &'t Term,
    ) -> Option<Term> {
        let btp = match &var.tp {
            None => {
                if checker.infer_var_type(&var.var).is_some() {
                    checker.scoped(|checker| {
                        checker.extend_context(var);
                        checker.infer_type(body)
                    })?
                } else {
                    let nvar = ComponentVar {
                        var: var.var.clone(),
                        tp: Some(checker.new_solvable()),
                        df: var.df.clone(),
                    };
                    checker.scoped(|checker| {
                        checker.extend_context(nvar);
                        checker.infer_type(body)
                    })?
                }
            }
            Some(tp) if tp.is_solvable().is_some() => {
                let inf = checker.scoped(|checker| {
                    checker.extend_context(var);
                    checker.infer_type(body)
                })?;
                ret!(&checker.check_inhabitable(tp) == Some(true));
                inf
            }
            Some(tp) => {
                ret!(&checker.check_inhabitable(tp) == Some(true));
                checker.scoped(|checker| {
                    checker.extend_context(var);
                    checker.infer_type(body)
                })?
            }
        };
        Some(construct_binder(var.clone(), btp, &self.pi))
    }

    fn infer_sequence<'t, Split: SplitStrategy>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        var: &'t ComponentVar,
        body: &'t Term,
    ) -> Option<Term> {
        let btp = match &var.tp {
            None => {
                if let Some(tp) = checker.infer_var_type(&var.var) {
                    checker.scoped(|checker| {
                        checker.extend_context(var);
                        checker.infer_type(body)
                    })?
                } else {
                    let nvar = ComponentVar {
                        var: var.var.clone(),
                        tp: Some(checker.new_solvable()),
                        df: var.df.clone(),
                    };
                    checker.scoped(|checker| {
                        checker.extend_context(nvar);
                        checker.infer_type(body)
                    })?
                }
            }
            Some(tp) if tp.is_solvable().is_some() => {
                let inf = checker.scoped(|checker| {
                    checker.extend_context(var);
                    checker.infer_type(body)
                })?;
                ret!(&checker.check_inhabitable(tp) == Some(true));
                inf
            }
            Some(tp) => {
                ret!(&checker.check_inhabitable(tp) == Some(true));
                checker.scoped(|checker| {
                    checker.extend_context(var);
                    checker.infer_type(body)
                })?
            }
        };
        let r = Term::Bound(BindingTerm::new(
            Term::Symbol {
                uri: self.pi.clone(),
                presentation: None,
            },
            Box::new([
                BoundArgument::BoundSeq(MaybeSequence::One(var.clone())),
                BoundArgument::Simple(btp),
            ]),
            None,
        ));
        Some(r)
    }
}
impl<Split: SplitStrategy> InferenceRule<Split> for LambdaPiInferenceRule {
    fn applicable(&self, term: &Term) -> bool {
        destruct_binder(term, &self.lambda).is_some()
    }
    fn infer<'t>(&self, checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        let (var, body) = destruct_binder(term, &self.lambda)?;
        match var {
            either::Left(var) => self.infer_simple(checker, var, body),
            either::Right(var) => self.infer_sequence(checker, var, body),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LambdaPiCheckingRule {
    pub lambda: SymbolUri,
    pub pi: SymbolUri,
}
impl SizedSolverRule for LambdaPiCheckingRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(
            "{ x:A, B(x):T } ⊢ (",
            &self.lambda,
            " x:A. B) : ",
            &self.pi,
            " y:A. T"
        )
    }
}
impl<Split: SplitStrategy> CheckingRule<Split> for LambdaPiCheckingRule {
    fn applicable(&self, _: &CheckRef<'_, '_, Split>, term: &Term, tp: &Term) -> bool {
        destruct_binder(term, &self.lambda).is_some_and(|(v, _)| v.is_left())
            && destruct_binder(tp, &self.pi).is_some_and(|(v, _)| v.is_left())
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        term: &'t Term,
        tp: &'t Term,
    ) -> Option<bool> {
        let (var, lambda_body) = destruct_binder(term, &self.lambda)?;
        let var = var.expect_left("applicable");
        let (pivar, pi_body) = destruct_binder(tp, &self.pi)?;
        let pivar = pivar.expect_left("applicable");
        let pi_tp = match &pivar.tp {
            None => Cow::Owned(checker.infer_var_type(&var.var)?),
            Some(tp) => {
                //ret!(&checker.check_inhabitable(trace, context.branch(), tp) == Some(true));
                Cow::Borrowed(tp)
            }
        };
        let lam_tp = match &var.tp {
            None => Cow::Owned(checker.infer_var_type(&var.var)?),
            Some(tp) => {
                //ret!(&checker.check_inhabitable(trace, context.branch(), tp) == Some(true));
                Cow::Borrowed(tp)
            }
        };
        ret!(&checker.scoped(|checker| { checker.check_subtype(&pi_tp, &lam_tp) }) == Some(true));
        let ntp = pi_body
            / (
                pivar.var.name(),
                &Term::Var {
                    variable: var.var.clone(),
                    presentation: None,
                },
            );
        checker.scoped(|checker| {
            checker.extend_context(var);
            checker.check_type(lambda_body, &ntp)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiInhabitableRule(pub SymbolUri);
impl SizedSolverRule for PiInhabitableRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ INH A, x:A, INH B(x) } ⊢ INH ", &self.0, " x:A. B")
    }
}

impl<Split: SplitStrategy> InhabitableRule<Split> for PiInhabitableRule {
    fn applicable(&self, term: &Term) -> bool {
        matches!(term,Term::Bound(b) if matches!(&b.head,Term::Symbol { uri, .. } if *uri == self.0))
    }
    fn apply<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool> {
        let Term::Bound(b) = term else { return None };
        let Some(BoundArgument::Simple(body)) = b.arguments.last() else {
            return None;
        };
        let previous = &b.arguments[..&b.arguments.len() - 1];
        let mut deferred = Vec::new();
        //checker.add_msg(traceref!("Here:", body.clone()).into());
        for arg in previous {
            match arg {
                BoundArgument::Simple(t) | BoundArgument::Sequence(MaybeSequence::One(t)) => {
                    let _ = checker.infer_type(t)?;
                }
                BoundArgument::Sequence(MaybeSequence::Seq(ts)) => {
                    for t in ts {
                        let _ = checker.infer_type(t)?;
                    }
                }
                BoundArgument::Bound(cv @ ComponentVar { var, tp, .. })
                | BoundArgument::BoundSeq(MaybeSequence::One(cv @ ComponentVar { var, tp, .. })) => {
                    if let Some(tp) = tp {
                        if tp.has_solvable() {
                            deferred.push(tp);
                        } else if !checker.check_inhabitable(tp)? {
                            return Some(false);
                        }
                    } else {
                        let _ = checker.infer_var_type(var)?;
                    }
                    checker.extend_context(cv);
                }
                BoundArgument::BoundSeq(MaybeSequence::Seq(vars)) => {
                    for cv @ ComponentVar { var, tp, .. } in vars {
                        if let Some(tp) = tp {
                            if tp.has_solvable() {
                                deferred.push(tp);
                            } else if !checker.check_inhabitable(tp)? {
                                return Some(false);
                            }
                        } else {
                            let _ = checker.infer_var_type(var)?;
                        }
                        checker.extend_context(cv);
                    }
                }
            }
        }
        if !checker.check_inhabitable(body)? {
            return None;
        }
        for d in deferred {
            if !checker.check_inhabitable(d)? {
                return None;
            }
        }
        Some(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiUniverseRule(pub SymbolUri);
impl SizedSolverRule for PiUniverseRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ INH A, x:A, UNIV B(x) } ⊢ UNIV ", &self.0, " x:A. B")
    }
}

impl<Split: SplitStrategy> UniverseRule<Split> for PiUniverseRule {
    fn applicable(&self, term: &Term) -> bool {
        matches!(term,Term::Bound(b) if matches!(&b.head,Term::Symbol { uri, .. } if *uri == self.0))
    }
    fn apply<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool> {
        let Term::Bound(b) = term else { return None };
        let Some(BoundArgument::Simple(body)) = b.arguments.last() else {
            return None;
        };
        let previous = &b.arguments[..&b.arguments.len() - 1];
        let mut deferred = Vec::new();
        //checker.add_msg(traceref!("Here:", body.clone()).into());
        for arg in previous {
            match arg {
                BoundArgument::Simple(t) | BoundArgument::Sequence(MaybeSequence::One(t)) => {
                    let _ = checker.infer_type(t)?;
                }
                BoundArgument::Sequence(MaybeSequence::Seq(ts)) => {
                    for t in ts {
                        let _ = checker.infer_type(t)?;
                    }
                }
                BoundArgument::Bound(cv @ ComponentVar { var, tp, .. })
                | BoundArgument::BoundSeq(MaybeSequence::One(cv @ ComponentVar { var, tp, .. })) => {
                    if let Some(tp) = tp {
                        if tp.has_solvable() {
                            deferred.push(tp);
                        } else if !checker.check_inhabitable(tp)? {
                            return Some(false);
                        }
                    } else {
                        let _ = checker.infer_var_type(var)?;
                    }
                    checker.extend_context(cv);
                }
                BoundArgument::BoundSeq(MaybeSequence::Seq(vars)) => {
                    for cv @ ComponentVar { var, tp, .. } in vars {
                        if let Some(tp) = tp {
                            if tp.has_solvable() {
                                deferred.push(tp);
                            } else if !checker.check_inhabitable(tp)? {
                                return Some(false);
                            }
                        } else {
                            let _ = checker.infer_var_type(var)?;
                        }
                        checker.extend_context(cv);
                    }
                }
            }
        }
        if !checker.check_universe(body)? {
            return None;
        }
        for d in deferred {
            if !checker.check_inhabitable(d)? {
                return None;
            }
        }
        Some(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiInferenceRule(pub SymbolUri);
impl SizedSolverRule for PiInferenceRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ f: ", &self.0, " x:A.B(x), t:A } ⊢ f(t) :=> B(t)")
    }
}

impl<Split: SplitStrategy> InferenceRule<Split> for PiInferenceRule {
    fn applicable(&self, term: &Term) -> bool {
        matches!(term, Term::Application(_)) // | Term::Bound(_))
    }
    fn infer<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        if let Term::Application(app) = term {
            if app.arguments.is_empty() {
                return None;
            }
            let tp = checker.infer_type(&app.head)?;
            if let Some(SequenceType::SeqType(tp, _)) = tp.as_sequence_type()
                && let [Argument::Simple(idx)] = &*app.arguments
            {
                checker.comment("is sequence type");
                let nat = checker.rules().marker().iter().rev().find_map(|rl| {
                    rl.as_any().downcast_ref::<NumberRule>().and_then(|rl| {
                        if rl.typ == NumberType::Naturals {
                            Some(rl.sym.clone())
                        } else {
                            None
                        }
                    })
                })?;
                let ntp: Term = nat.into();
                if checker.scoped(|checker| checker.check_type(idx, &ntp)) != Some(true) {
                    return None;
                }
                Some(tp.clone())
            } else {
                self.type_apply(&mut checker, tp, &app.arguments)
            }
        } else {
            checker.failure("Not an application");
            None
        }
    }
}

impl PiInferenceRule {
    // INVARIANT: return has 2 arguments, the second one being simple
    pub fn deconstruct_tp<Split: SplitStrategy>(
        bind_uri: &SymbolUri,
        checker: &mut CheckRef<'_, '_, Split>,
        tp: Term,
    ) -> Result<BindingTerm, Term> {
        let Some(nret) = checker.scoped(|checker| {
            match checker.simplify_until(&tp, |_, t| {
                if let Term::Bound(b) = t
                    && let Term::Symbol { uri, .. } = &b.head
                {
                    *uri == *bind_uri
                        && b.arguments.len() == 2
                        && matches!(b.arguments.get(1), Some(BoundArgument::Simple(_)))
                } else {
                    false
                }
            })? {
                Cow::Borrowed(_) => Some(None),
                Cow::Owned(tp) => Some(Some(tp)),
            }
        }) else {
            checker.add_msg(traceref!(FAIL "type is not a binder: ",tp.clone()).into());
            return Err(tp);
        };
        let Term::Bound(b) = nret.unwrap_or(tp) else {
            // SAFETY: simplify_until above would have returned None otherwise
            unsafe { unreachable_unchecked() }
        };
        if !matches!(&b.head,Term::Symbol { uri, .. } if *uri == *bind_uri)
            || b.arguments.len() != 2
            || !matches!(b.arguments.get(1), Some(BoundArgument::Simple(_)))
        {
            checker.failure("Type is not a Π anymore");
            return Err(Term::Bound(b));
        }
        Ok(b)
    }

    // INVARIANT: tp.is_concrete_sequence()
    pub(super) fn flatten_sequence<Split: SplitStrategy>(
        checker: &mut CheckRef<'_, '_, Split>,
        tp: &Term,
        b: &BindingTerm,
        body: Term,
    ) -> Term {
        let Some(new_args) = tp.as_sequence().and_then(|s| s.to_concrete()) else {
            // SAFETY: tp.is_concrete_sequence()
            unsafe { unreachable_unchecked() }
        };
        checker.comment("Flattening concrete sequence argument");
        new_args.into_iter().rfold(body.clone(), |body, arg| {
            Term::Bound(BindingTerm::new(
                b.head.clone(),
                Box::new([
                    BoundArgument::Bound(ComponentVar {
                        var: Variable::Name {
                            name: crate::DUMMY.clone(),
                            notated: None,
                        },
                        tp: Some(arg),
                        df: None,
                    }),
                    BoundArgument::Simple(body),
                ]),
                b.presentation.clone(),
            ))
        })
    }

    // INVARIANT: !seq.is_empty()
    pub(super) fn recurse_seq_args<'t, Split: SplitStrategy>(
        uri: &SymbolUri,
        checker: &mut CheckRef<'t, '_, Split>,
        b: &BindingTerm,
        seq: &'t [Term],
        body: &Term,
    ) -> Option<Term> {
        // SAFETY: !seq.is_empty()
        let first = unsafe { seq.first().unwrap_unchecked() };
        let rest = &seq[1..];
        let mut ret = Self::simple_apply(checker, b, first, body)?;
        for arg in rest {
            let b = Self::deconstruct_tp(uri, checker, ret).ok()?;
            let [_, BoundArgument::Simple(body)] = &*b.arguments else {
                // SAFETY: invariant of deconstruct_tp
                unsafe { unreachable_unchecked() }
            };
            ret = Self::simple_apply(checker, &b, arg, body)?;
        }
        Some(ret)
    }

    fn try_extension<'t, Split: SplitStrategy>(
        &self,
        checker: &mut CheckRef<'t, '_, Split>,
        tp: &'t Term,
        args: &'t [Argument],
        index: &mut usize,
    ) -> Option<Term> {
        let exts = checker
            .rules()
            .marker()
            .iter()
            .rev()
            .filter_map(|rl| rl.as_any().downcast_ref::<PiExtensionRule<Split>>())
            .cloned()
            .collect::<SmallVec<_, 1>>();
        let arg = &args[*index];
        let Some(ntp) =
            checker.simplify_until(tp, |_, t| exts.iter().any(|rl| (rl.applicable)(rl, t, arg)))
        else {
            checker.failure("type is not a pi");
            return None;
        };
        for rl in exts {
            if (rl.applicable)(&rl, &ntp, arg)
                && let Some(t) = (rl.infer)(&rl, self, checker, &ntp, args, index)
            {
                return Some(t);
            }
        }
        None
    }

    pub fn type_apply<'t, Split: SplitStrategy>(
        &self,
        checker: &mut CheckRef<'t, '_, Split>,
        tp: Term,
        args: &'t [Argument],
    ) -> Option<Term> {
        let mut ret = tp;
        let mut i = 0;
        loop {
            let Some(arg) = args.get(i) else {
                return Some(ret);
            };
            let dec = Self::deconstruct_tp(&self.0, checker, ret);
            let b = match dec {
                Ok(b) => b,
                Err(t) => {
                    ret =
                        checker.scoped(|checker| self.try_extension(checker, &t, args, &mut i))?;
                    continue;
                }
            };
            let [first, BoundArgument::Simple(body)] = &*b.arguments else {
                // SAFETY: invariant of deconstruct_tp
                unsafe { unreachable_unchecked() }
            };
            match (first, arg) {
                (
                    BoundArgument::Bound(ComponentVar {
                        var,
                        tp: Some(tp),
                        df: None,
                    }),
                    Argument::Sequence(seq),
                ) if !body.has_free_such_that(|v| v.name() == var.name())
                    && tp.as_sequence().is_some_and(|s| s.is_concrete()) =>
                {
                    ret = Self::flatten_sequence(checker, tp, &b, body.clone());
                }
                (
                    BoundArgument::Bound(ComponentVar {
                        var,
                        tp: Some(tp),
                        df: None,
                    }),
                    Argument::Sequence(MaybeSequence::Seq(seq)),
                ) if !seq.is_empty() => {
                    i += 1;
                    checker.counter("(a) Checking Argument ", i);
                    ret = Self::recurse_seq_args(&self.0, checker, &b, seq, body)?;
                }
                (_, Argument::Simple(arg)) => {
                    i += 1;
                    checker.counter("(b) Checking Argument ", i);
                    ret = Self::simple_apply(checker, &b, arg, body)?;
                }
                (_, Argument::Sequence(arg)) => {
                    i += 1;
                    checker.counter("(c) Checking Argument ", i);
                    ret = checker.scoped(|checker| Self::seq_apply(checker, &b, arg, body))?;
                }
            }
        }
    }

    pub(super) fn simple_apply<'t, Split: SplitStrategy>(
        checker: &mut CheckRef<'t, '_, Split>,
        b: &BindingTerm,
        arg: &'t Term,
        body: &Term,
    ) -> Option<Term> {
        let headvar = match b.arguments.first() {
            Some(BoundArgument::Bound(headvar)) => headvar,
            Some(BoundArgument::BoundSeq(MaybeSequence::Seq(vs))) => {
                if let [headvar] = &**vs {
                    headvar
                } else {
                    checker.failure("First argument is not a bound variable");
                    return None;
                }
            }
            Some(BoundArgument::BoundSeq(MaybeSequence::One(v))) if arg.is_sequence() => v,
            Some(BoundArgument::BoundSeq(MaybeSequence::One(_))) => {
                let seq = Term::into_seq(std::iter::once(arg.clone()));
                return checker.scoped(|slf| Self::simple_apply(slf, b, &seq, body));
            }
            _ => {
                checker.failure("First argument is not a bound variable");
                /*checker.comment(format!(
                    "Here: {:?}{:?}  <-- {:?}",
                    b.head.debug_short(),
                    b.arguments,
                    arg.debug_short()
                ));*/
                return None;
            }
        };

        let (varname, vartp) = match headvar {
            ComponentVar {
                var, tp: Some(tp), ..
            } => (var.name(), tp.clone()),
            ComponentVar { var, .. } => (
                var.name(),
                checker.scoped(|checker| {
                    checker
                        .infer_var_type(var)
                        .unwrap_or_else(|| checker.new_solvable())
                }),
            ),
        };

        if checker
            .scoped(|checker| checker.check_type(arg, &vartp))
            .is_none_or(|b| !b)
        {
            return None;
        }
        Some((body / (varname, arg)).into_owned())
    }

    pub(super) fn seq_apply<'t, Split: SplitStrategy>(
        checker: &mut CheckRef<'t, '_, Split>,
        b: &'t BindingTerm,
        arg: &'t MaybeSequence<Term>,
        body: &Term,
    ) -> Option<Term> {
        if let Some(BoundArgument::Bound(ComponentVar {
            var,
            tp: Some(tp),
            df: None,
        })) = b.arguments.first()
            && let Some(tpargs) = tp.as_sequence().and_then(|s| s.to_concrete())
            && !body
                .free_variables()
                .into_iter()
                .any(|v| v.name() == var.name())
            && let MaybeSequence::Seq(args) = arg
            && args.len() == tpargs.len()
        {
            for (a, t) in args.iter().zip(tpargs.iter()) {
                if !checker.scoped(|checker| checker.check_type(a, t))? {
                    return None;
                }
            }
            return Some(body.clone());
        }

        let Some(BoundArgument::BoundSeq(MaybeSequence::One(headvar))) = b.arguments.first() else {
            checker.failure("First argument is not a bound variable sequence");
            /*checker.comment(format!(
                "Here: {:?}{:?}  <-- {arg:?}",
                b.head.debug_short(),
                b.arguments
            ));*/
            return None;
        };

        let (varname, vartp) = match headvar {
            ComponentVar {
                var, tp: Some(tp), ..
            } => (var.name(), tp.clone()),
            ComponentVar { var, .. } => (
                var.name(),
                checker.scoped(|checker| {
                    checker
                        .infer_var_type(var)
                        .unwrap_or_else(|| checker.new_solvable())
                }),
            ),
        };

        match arg {
            MaybeSequence::One(arg) => {
                if checker
                    .scoped(|checker| checker.check_type(arg, &vartp))
                    .is_none_or(|b| !b)
                {
                    return None;
                }
                Some((body / (varname, arg)).into_owned())
            }
            MaybeSequence::Seq(arg) => {
                let narg = Term::into_seq(arg.iter().cloned());
                if let Some(SequenceType::SeqType(vartp, _)) = vartp.as_sequence_type() {
                    if !checker.scoped(|checker| {
                        for a in arg {
                            if !checker.check_type(a, vartp)? {
                                return None;
                            }
                        }
                        Some(true)
                    })? {
                        return None;
                    }
                } else if checker
                    .scoped(|checker| checker.check_type(&narg, &vartp))
                    .is_none_or(|b| !b)
                {
                    return None;
                }

                Some((body / (varname, &narg)).into_owned())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtaRule(pub SymbolUri);
impl SizedSolverRule for EtaRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.0, " x1:A1,...,xn:An. s(x1,...,xn) ==> s")
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for EtaRule {
    fn applicable(&self, term: &Term) -> bool {
        fn applicable_inner(idx: usize, term: &Term, slf: &SymbolUri) -> bool {
            if let Term::Bound(op) = &term
                && let Term::Symbol { uri, .. } = &op.head
                && *uri == *slf
                && op.arguments.len() == 2
                && let Some(BoundArgument::Bound(v)) = op.arguments.first()
                && let Some(BoundArgument::Simple(a)) = op.arguments.get(1)
            {
                matches!(a,Term::Application(app) if matches!(app.arguments.get(idx),Some(Argument::Simple(Term::Var { variable, .. })) if variable.name() == v.var.name()))
                    || applicable_inner(idx + 1, a, slf)
            } else {
                false
            }
        }
        applicable_inner(0, term, &self.0)
    }
    fn apply<'t>(
        &self,
        checker: CheckRef<'t, '_, Split>,
        mut term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        let mut vars = Vec::new();
        loop {
            let Term::Bound(op) = term else {
                return Err(None);
            };
            let Term::Symbol { uri, .. } = &op.head else {
                return Err(None);
            };
            if *uri != self.0 {
                return Err(None);
            };
            let [BoundArgument::Bound(v), BoundArgument::Simple(body)] = &*op.arguments else {
                return Err(None);
            };
            vars.push(&v.var);
            match body {
                Term::Application(app) if app.arguments.len() >= vars.len() => {
                    let idx = app.arguments.len() - vars.len();
                    let margs = &app.arguments[idx..];
                    for (a, v) in margs.iter().zip(vars.iter()) {
                        if let Argument::Simple(Term::Var { variable, .. }) = a
                            && variable.name() == v.name()
                        {
                            // all good
                        } else {
                            return Err(None);
                        }
                    }
                    if idx == 0 {
                        return Ok(app.head.clone());
                    }
                    return Ok(Term::Application(ApplicationTerm::new(
                        app.head.clone(),
                        app.arguments.iter().take(idx).cloned().collect(),
                        None,
                    )));
                }
                _ => term = body,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BetaRule(pub SymbolUri);
impl SizedSolverRule for BetaRule {
    fn priority(&self) -> isize {
        100_000_000
    }
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ a:A } (", &self.0, " x:A. t)(a) ==> t[x/a]")
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for BetaRule {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term
            && let Term::Bound(op) = &app.head
            && let Term::Symbol { uri, .. } = &op.head
        {
            *uri == self.0 && app.arguments.len() >= op.arguments.len() - 1
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
        let Term::Bound(fun) = &app.head else {
            return Err(None);
        };
        let Some(BoundArgument::Simple(ret)) = fun.arguments.last() else {
            return Err(None);
        };
        let funargs = &fun.arguments[..fun.arguments.len() - 1];
        let actual_args = &app.arguments[..funargs.len()];
        let rest_args = &app.arguments[funargs.len()..];
        let mut ret = Cow::Borrowed(ret);
        for (i, (v, a)) in funargs.iter().zip(actual_args).enumerate() {
            checker.counter("Checking argument ", i + 1);
            match (v, a) {
                (
                    BoundArgument::Bound(ComponentVar {
                        var,
                        tp: Some(tp),
                        df: None,
                    }),
                    Argument::Simple(a),
                ) => {
                    if !checker.check_type(a, tp).ok_or(None)? {
                        return Err(None);
                    }
                    if let Cow::Owned(nr) = &*ret / (var.name(), a) {
                        ret = Cow::Owned(nr);
                    }
                }
                (
                    BoundArgument::Bound(ComponentVar {
                        var,
                        tp: None,
                        df: None,
                    }),
                    Argument::Simple(a),
                ) => {
                    let Some(tp) = checker.infer_var_type(var) else {
                        checker.add_msg(traceref!(FAIL "untyped variable ",var).into());
                        return Err(None);
                    };
                    if checker.scoped(|checker| checker.check_type(a, &tp)) != Some(true) {
                        return Err(None);
                    }
                    if let Cow::Owned(nr) = &*ret / (var.name(), a) {
                        ret = Cow::Owned(nr);
                    }
                }
                (
                    BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                        var,
                        tp: Some(tp),
                        df: None,
                    })),
                    Argument::Sequence(MaybeSequence::Seq(ts)),
                ) => {
                    let Some(SequenceType::SeqType(tp, _)) = tp.as_sequence_type() else {
                        checker.failure("Type of sequence variable is not a sequence type");
                        //checker.comment(format!("Here: {:?} <-> {ts:?}", tp.debug_short()));
                        return Err(None);
                    };
                    for t in ts {
                        if !checker.check_type(t, tp).ok_or(None)? {
                            return Err(None);
                        }
                    }
                    if let Cow::Owned(nr) =
                        &*ret / (var.name(), &Term::into_seq(ts.iter().cloned()))
                    {
                        ret = Cow::Owned(nr);
                    }
                }
                (
                    BoundArgument::BoundSeq(MaybeSequence::One(ComponentVar {
                        var,
                        tp: Some(tp),
                        df: None,
                    })),
                    Argument::Simple(t),
                ) => {
                    if tp.as_sequence_type().is_none() {
                        checker.failure("Type of sequence variable is not a sequence type");
                        //checker.comment(format!("Here: {:?} <-> {ts:?}", tp.debug_short()));
                        return Err(None);
                    }
                    if !checker.check_type(t, tp).ok_or(None)? {
                        return Err(None);
                    }
                    if let Cow::Owned(nr) = &*ret / (var.name(), t) {
                        ret = Cow::Owned(nr);
                    }
                }

                (
                    BoundArgument::Bound(ComponentVar {
                        var,
                        tp: None,
                        df: None,
                    }),
                    Argument::Sequence(MaybeSequence::Seq(args)),
                ) => {
                    let Some(tp) = checker.infer_var_type(var) else {
                        checker.add_msg(traceref!(FAIL "untyped variable ",var).into());
                        return Err(None);
                    };
                    let Some(tp) = checker.scoped(|checker| {
                        checker
                            .simplify_until(&tp, |_, t| {
                                t.as_sequence().is_some_and(|s| s.is_concrete())
                            })
                            .map(Cow::into_owned)
                    }) else {
                        checker.failure("Not a concrete sequence");
                        return Err(None);
                    };
                    let vartps = tp.as_sequence().and_then(|s| s.to_concrete()).ok_or(None)?;
                    if vartps.len() != args.len() {
                        checker.failure("sequence lengths don't match");
                        return Err(None);
                    }
                    for (a, tp) in args.iter().zip(vartps) {
                        if !checker
                            .scoped(|checker| checker.check_type(a, &tp))
                            .ok_or(None)?
                        {
                            return Err(None);
                        }
                    }
                }
                (
                    BoundArgument::Bound(ComponentVar {
                        var,
                        tp: Some(tp),
                        df: None,
                    }),
                    Argument::Sequence(MaybeSequence::Seq(args)),
                ) => {
                    let ntp = checker
                        .scoped(|checker| {
                            checker
                                .simplify_until(tp, |_, t| {
                                    t.as_sequence().is_some_and(|s| s.is_concrete())
                                })
                                .map(Cow::into_owned)
                        })
                        .ok_or(None)?;
                    let Some(vartps) = ntp.as_sequence().and_then(|s| s.to_concrete()) else {
                        checker.failure("type of bound variable not a concrete sequence");
                        return Err(None);
                    };
                    if vartps.len() != args.len() {
                        checker.failure("sequence lengths don't match");
                        return Err(None);
                    }
                    for (a, tp) in args.iter().zip(vartps) {
                        if !checker
                            .scoped(|checker| checker.check_type(a, &tp))
                            .ok_or(None)?
                        {
                            return Err(None);
                        }
                    }
                }

                (a, b) => {
                    checker.failure(format!("TODO: {a:?} <-> {b:?}"));
                    return Err(None);
                }
            }
        }
        if rest_args.is_empty() {
            Ok(ret.into_owned())
        } else {
            Ok(Term::Application(ApplicationTerm::new(
                ret.into_owned(),
                rest_args.iter().cloned().collect(),
                None,
            )))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyRule(pub SymbolUri);
impl SizedSolverRule for ApplyRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ a:A } ", &self.0, "(f,a*) ==> f(a*)")
    }
    fn priority(&self) -> isize {
        isize::MAX
    }
}
impl<Split: SplitStrategy> PreparationRule<Split> for ApplyRule {
    fn applicable(&self, _: &CheckRef<'_, '_, Split>, t: &Term) -> bool {
        if let Term::Application(app) = t
            && let [Argument::Simple(_), Argument::Sequence(_)] = &*app.arguments
            && let Term::Symbol { uri, .. } = &app.head
        {
            *uri == self.0
        } else {
            false
        }
    }
    fn apply(
        &self,
        _: &mut CheckRef<'_, '_, Split>,
        t: Term,
        path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> std::ops::ControlFlow<Term, Term> {
        let Term::Application(app) = t else {
            return std::ops::ControlFlow::Continue(t);
        };
        let [Argument::Simple(f), Argument::Sequence(a)] = &*app.arguments else {
            return std::ops::ControlFlow::Continue(Term::Application(app));
        };
        let ret = Term::Application(ApplicationTerm::new(
            f.clone(),
            match a {
                MaybeSequence::One(o) => {
                    Box::new([Argument::Sequence(MaybeSequence::One(o.clone()))])
                }
                MaybeSequence::Seq(ts) => ts.iter().map(|t| Argument::Simple(t.clone())).collect(),
            },
            /*Box::new([Argument::Sequence(match a {
                MaybeSequence::One(o) => MaybeSequence::One(o.clone()),
                MaybeSequence::Seq(ts) => MaybeSequence::Seq(ts.clone()),
            })]),*/
            app.presentation.clone(),
        ));
        if let Some((p, i)) = path
            && let Some(j) = p.get_mut(i)
        {
            *j = j.saturating_sub(1);
        }
        std::ops::ControlFlow::Continue(ret)
    }
    fn applicable_revert(&self, _: &CheckRef<'_, '_, Split>, _: &Term) -> bool {
        false
    }
    fn revert(&self, _: &CheckRef<'_, '_, Split>, t: Term) -> std::ops::ControlFlow<Term, Term> {
        std::ops::ControlFlow::Continue(t)
    }
}
