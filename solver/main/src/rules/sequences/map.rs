use std::hint::unreachable_unchecked;

use ftml_ontology::terms::{
    ApplicationTerm, Argument, BindingTerm, BoundArgument, ComponentVar, MaybeSequence, Term,
    Variable,
};
use ftml_solver_trace::SizedSolverRule;
use ftml_uris::SymbolUri;

use crate::{
    rules::{
        InferenceRule, InhabitableRule, SimplificationRule,
        operators::numbers::{NumberRule, NumberType},
        sequences::{Sequence, SequenceType},
    },
    split::SplitStrategy,
};

/*
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapInferenceRule(pub SymbolUri);
impl SizedSolverRule for MapSimplificationRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ s:T1*, x:T1, f(x):T2 } ⊢ ",&self.0, "([x1,...,xn],f) :=> T2*")
    }
}
impl<Split: SplitStrategy> InferenceRule<Split> for MapInferenceRule {

}
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapArgumentSimplificationRule(pub SymbolUri);
impl SizedSolverRule for MapArgumentSimplificationRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.0, "([x1,...,xn],f) ==> [f(x1),..,f(xn)]")
    }
    fn priority(&self) -> isize {
        1000
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for MapArgumentSimplificationRule {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term {
            app.arguments.iter().any(|a| {
                if let Argument::Sequence(MaybeSequence::One(t)) = a {
                    MapSimplificationRule::applicable_i(&self.0, t)
                } else {
                    false
                }
            })
        } else if let Term::Bound(app) = term {
            app.arguments.iter().any(|a| {
                if let BoundArgument::Sequence(MaybeSequence::One(t)) = a {
                    MapSimplificationRule::applicable_i(&self.0, t)
                } else {
                    false
                }
            })
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        mut checker: crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        if let Term::Application(app) = term {
            Ok(Term::Application(ApplicationTerm::new(
                app.head.clone(),
                app.arguments
                    .iter()
                    .map(|a| match a {
                        Argument::Sequence(MaybeSequence::One(t))
                            if MapSimplificationRule::applicable_i(&self.0, t) =>
                        {
                            MapSimplificationRule::apply_i(&mut checker, t).map_or_else(
                                |_| a.clone(),
                                |v| Argument::Sequence(MaybeSequence::Seq(v.into_boxed_slice())),
                            )
                        }
                        _ => a.clone(),
                    })
                    .collect(),
                app.presentation.clone(),
            )))
        } else if let Term::Bound(app) = term {
            Ok(Term::Bound(BindingTerm::new(
                app.head.clone(),
                app.arguments
                    .iter()
                    .map(|a| match a {
                        BoundArgument::Sequence(MaybeSequence::One(t))
                            if MapSimplificationRule::applicable_i(&self.0, t) =>
                        {
                            MapSimplificationRule::apply_i(&mut checker, t).map_or_else(
                                |_| a.clone(),
                                |v| {
                                    BoundArgument::Sequence(MaybeSequence::Seq(
                                        v.into_boxed_slice(),
                                    ))
                                },
                            )
                        }
                        _ => a.clone(),
                    })
                    .collect(),
                app.presentation.clone(),
            )))
        } else {
            Err(None)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapIndexSimplificationRule(pub SymbolUri);
impl SizedSolverRule for MapIndexSimplificationRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.0, "(s,f)(i) ==> f(s(i))")
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for MapIndexSimplificationRule {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term
            && let [Argument::Simple(_)] = &*app.arguments
            && let Term::Application(map) = &app.head
            && let Term::Symbol { uri, .. } = &map.head
            && *uri == self.0
            && let [_, Argument::Simple(_)] = &*map.arguments
        {
            true
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        _: crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        let Term::Application(app) = term else {
            return Err(None);
        };
        let [Argument::Simple(idx)] = &*app.arguments else {
            return Err(None);
        };
        let Term::Application(map) = &app.head else {
            return Err(None);
        };
        let [a, Argument::Simple(f)] = &*map.arguments else {
            return Err(None);
        };
        let s = match a {
            Argument::Simple(t) | Argument::Sequence(MaybeSequence::One(t)) => t.clone(),
            Argument::Sequence(MaybeSequence::Seq(ts)) => Term::into_seq(ts.iter().cloned()),
        };
        Ok(Term::Application(ApplicationTerm::new(
            f.clone(),
            Box::new([Argument::Simple(Term::Application(ApplicationTerm::new(
                s,
                Box::new([Argument::Simple(idx.clone())]),
                None,
            )))]),
            None,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSimplificationRule(pub SymbolUri);
impl SizedSolverRule for MapSimplificationRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(&self.0, "([x1,...,xn],f) ==> [f(x1),..,f(xn)]")
    }
}
impl MapSimplificationRule {
    fn apply_i<'t, Split: SplitStrategy>(
        checker: &mut crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Vec<Term>, Option<ftml_ontology::terms::termpaths::TermPath>> {
        let Term::Application(app) = term else {
            return Err(None);
        };
        let [a, Argument::Simple(f)] = &*app.arguments else {
            return Err(None);
        };
        match a {
            Argument::Simple(s) | Argument::Sequence(MaybeSequence::One(s)) => {
                let Some(ts) = s.as_sequence().and_then(|s| s.to_concrete()) else {
                    return Err(None);
                };
                Ok(ts
                    .into_iter()
                    .map(|arg| {
                        let t = Term::Application(ApplicationTerm::new(
                            f.clone(),
                            Box::new([Argument::Simple(arg)]),
                            None,
                        ));
                        /*checker
                        .scoped(|checker| {
                            checker.simplify_full(
                                crate::impls::simplify::Expansion::NoDefinitionExpansion,
                                &t,
                            )
                        })
                        .unwrap_or(t)*/
                        t
                    })
                    .collect())
            }
            Argument::Sequence(MaybeSequence::Seq(ts)) => Ok(ts
                .iter()
                .map(|arg| {
                    let t = Term::Application(ApplicationTerm::new(
                        f.clone(),
                        Box::new([Argument::Simple(arg.clone())]),
                        None,
                    ));
                    /*
                    checker
                        .scoped(|checker| {
                            checker.simplify_full(
                                crate::impls::simplify::Expansion::NoDefinitionExpansion,
                                &t,
                            )
                        })
                        .unwrap_or(t) */
                    t
                })
                .collect()),
        }
    }
    fn applicable_i(sym: &SymbolUri, term: &Term) -> bool {
        if let Term::Application(app) = term
            && let Term::Symbol { uri, .. } = &app.head
            && *uri == *sym
            && let [a, Argument::Simple(_)] = &*app.arguments
        {
            if let Argument::Simple(s) | Argument::Sequence(MaybeSequence::One(s)) = a {
                s.as_sequence().is_some_and(|seq| seq.is_concrete())
            } else {
                true
            }
        } else {
            false
        }
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for MapSimplificationRule {
    fn applicable(&self, term: &Term) -> bool {
        Self::applicable_i(&self.0, term)
    }
    fn apply<'t>(
        &self,
        mut checker: crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        Self::apply_i::<Split>(&mut checker, term).map(|ts| Term::into_seq(ts.into_iter()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapInhabitableRule(pub SymbolUri);
impl SizedSolverRule for MapInhabitableRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ x:A, INH f(x), s:A* } ⊢ INH ", &self.0, "(s,f)")
    }
}

impl<Split: SplitStrategy> InhabitableRule<Split> for MapInhabitableRule {
    fn applicable(&self, term: &ftml_ontology::terms::Term) -> bool {
        if let Term::Application(app) = term
            && let Term::Symbol { uri, .. } = &app.head
        {
            *uri == self.0 && app.arguments.len() == 2
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        mut checker: crate::CheckRef<'t, '_, Split>,
        term: &'t ftml_ontology::terms::Term,
    ) -> Option<bool> {
        let Term::Application(app) = term else {
            return None;
        };
        let [Argument::Sequence(seq), Argument::Simple(f)] = &*app.arguments else {
            checker.failure("arguments don't match");
            return None;
        };
        let seqtp = match seq {
            MaybeSequence::One(t)
                if matches!(t.as_sequence(), Some(Sequence::SequenceExpression(_))) =>
            {
                // SAFETY: pattern match
                let Some(Sequence::SequenceExpression(args)) = t.as_sequence() else {
                    unsafe { unreachable_unchecked() }
                };
                let mut curr = None;
                for t in args {
                    if !checker.scoped::<Option<bool>>(|checker| {
                        let r = checker.infer_type(t)?;
                        if let Some(c) = &curr {
                            if !checker.scoped(|checker| checker.check_equality(c, &r))? {
                                return None;
                            }
                        } else {
                            curr = Some(r);
                        }
                        Some(true)
                    })? {
                        return None;
                    }
                }
                curr?
            }
            MaybeSequence::One(t) => checker.infer_type(t)?,
            MaybeSequence::Seq(ts) => {
                let mut curr = None;
                for t in ts {
                    if !checker.scoped::<Option<bool>>(|checker| {
                        let r = checker.infer_type(t)?;
                        if let Some(c) = &curr {
                            if !checker.scoped(|checker| checker.check_equality(c, &r))? {
                                return None;
                            }
                        } else {
                            curr = Some(r);
                        }
                        Some(true)
                    })? {
                        return None;
                    }
                }
                curr?
            }
        };
        let seqtp = match seqtp.as_sequence_type() {
            Some(SequenceType::SeqType(t, _)) => t.clone(),
            Some(SequenceType::Map(seq, f)) => {
                let nat = checker.rules().marker().iter().rev().find_map(|rl| {
                    rl.as_any().downcast_ref::<NumberRule>().and_then(|rl| {
                        if rl.typ == NumberType::Naturals {
                            Some(rl.sym.clone())
                        } else {
                            None
                        }
                    })
                })?;
                let vname = Variable::Name {
                    name: unsafe { "index".parse().unwrap_unchecked() },
                    notated: None,
                };
                let nv = ComponentVar {
                    var: vname.clone(),
                    tp: Some(nat.into()),
                    df: None,
                };
                checker.extend_context(nv);
                let arg = Term::Application(ApplicationTerm::new(
                    seq.to_term(),
                    Box::new([Argument::Simple(vname.into())]),
                    None,
                ));
                Term::Application(ApplicationTerm::new(
                    f.clone(),
                    Box::new([Argument::Simple(arg)]),
                    None,
                ))
            }
            Some(_) => {
                let nat = checker.rules().marker().iter().rev().find_map(|rl| {
                    rl.as_any().downcast_ref::<NumberRule>().and_then(|rl| {
                        if rl.typ == NumberType::Naturals {
                            Some(rl.sym.clone())
                        } else {
                            None
                        }
                    })
                })?;
                let vname = Variable::Name {
                    name: unsafe { "index".parse().unwrap_unchecked() },
                    notated: None,
                };
                let nv = ComponentVar {
                    var: vname.clone(),
                    tp: Some(nat.into()),
                    df: None,
                };
                checker.extend_context(nv);
                Term::Application(ApplicationTerm::new(
                    seqtp,
                    Box::new([Argument::Simple(vname.into())]),
                    None,
                ))
            }
            _ => seqtp,
        };
        let (v, _) = f.fresh_variable(&crate::DUMMY, None);
        checker.extend_context(ComponentVar {
            var: v.clone(),
            tp: Some(seqtp),
            df: None,
        });
        let nt = Term::Application(ApplicationTerm::new(
            f.clone(),
            Box::new([Argument::Simple(Term::Var {
                variable: v,
                presentation: None,
            })]),
            None,
        ));
        checker.scoped(|checker| checker.check_inhabitable(&nt))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapInferenceRule(pub SymbolUri);
impl SizedSolverRule for MapInferenceRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("{ x:A, f(x):T, s:A* } ⊢ ", &self.0, "(s,f): T*")
    }
}

impl<Split: SplitStrategy> InferenceRule<Split> for MapInferenceRule {
    fn applicable(&self, term: &ftml_ontology::terms::Term) -> bool {
        if let Term::Application(app) = term
            && let Term::Symbol { uri, .. } = &app.head
        {
            *uri == self.0 && app.arguments.len() == 2
        } else {
            false
        }
    }
    fn infer<'t>(
        &self,
        mut checker: crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Option<Term> {
        let Term::Application(app) = term else {
            return None;
        };
        let [Argument::Sequence(seq), Argument::Simple(f)] = &*app.arguments else {
            checker.failure("arguments don't match");
            return None;
        };
        let seqtp = match seq {
            MaybeSequence::One(t)
                if matches!(t.as_sequence(), Some(Sequence::SequenceExpression(_))) =>
            {
                // SAFETY: pattern match
                let Some(Sequence::SequenceExpression(args)) = t.as_sequence() else {
                    unsafe { unreachable_unchecked() }
                };
                let mut curr = None;
                for t in args {
                    if !checker.scoped::<Option<bool>>(|checker| {
                        let r = checker.infer_type(t)?;
                        if let Some(c) = &curr {
                            if !checker.scoped(|checker| checker.check_equality(c, &r))? {
                                return None;
                            }
                        } else {
                            curr = Some(r);
                        }
                        Some(true)
                    })? {
                        return None;
                    }
                }
                curr?
            }
            MaybeSequence::One(t) => checker.infer_type(t)?,
            MaybeSequence::Seq(ts) => {
                let mut curr = None;
                for t in ts {
                    if !checker.scoped::<Option<bool>>(|checker| {
                        let r = checker.infer_type(t)?;
                        if let Some(c) = &curr {
                            if !checker.scoped(|checker| checker.check_equality(c, &r))? {
                                return None;
                            }
                        } else {
                            curr = Some(r);
                        }
                        Some(true)
                    })? {
                        return None;
                    }
                }
                curr?
            }
        };
        let seqtp = match seqtp.as_sequence_type() {
            Some(SequenceType::SeqType(t, _)) => t.clone(),
            Some(SequenceType::Map(seq, f)) => {
                let nat = checker.rules().marker().iter().rev().find_map(|rl| {
                    rl.as_any().downcast_ref::<NumberRule>().and_then(|rl| {
                        if rl.typ == NumberType::Naturals {
                            Some(rl.sym.clone())
                        } else {
                            None
                        }
                    })
                })?;
                let vname = Variable::Name {
                    name: unsafe { "index".parse().unwrap_unchecked() },
                    notated: None,
                };
                let nv = ComponentVar {
                    var: vname.clone(),
                    tp: Some(nat.into()),
                    df: None,
                };
                checker.extend_context(nv);
                let arg = Term::Application(ApplicationTerm::new(
                    seq.to_term(),
                    Box::new([Argument::Simple(vname.into())]),
                    None,
                ));
                Term::Application(ApplicationTerm::new(
                    f.clone(),
                    Box::new([Argument::Simple(arg)]),
                    None,
                ))
            }
            Some(_) => {
                let nat = checker.rules().marker().iter().rev().find_map(|rl| {
                    rl.as_any().downcast_ref::<NumberRule>().and_then(|rl| {
                        if rl.typ == NumberType::Naturals {
                            Some(rl.sym.clone())
                        } else {
                            None
                        }
                    })
                })?;
                let vname = Variable::Name {
                    name: unsafe { "index".parse().unwrap_unchecked() },
                    notated: None,
                };
                let nv = ComponentVar {
                    var: vname.clone(),
                    tp: Some(nat.into()),
                    df: None,
                };
                checker.extend_context(nv);
                Term::Application(ApplicationTerm::new(
                    seqtp,
                    Box::new([Argument::Simple(vname.into())]),
                    None,
                ))
            }
            _ => seqtp,
        };
        let (v, _) = f.fresh_variable(&crate::DUMMY, None);
        checker.extend_context(ComponentVar {
            var: v.clone(),
            tp: Some(seqtp),
            df: None,
        });
        let nt = Term::Application(ApplicationTerm::new(
            f.clone(),
            Box::new([Argument::Simple(Term::Var {
                variable: v,
                presentation: None,
            })]),
            None,
        ));
        checker
            .scoped(|checker| checker.infer_type(&nt))
            .map(Term::into_seq_type)
    }
}
