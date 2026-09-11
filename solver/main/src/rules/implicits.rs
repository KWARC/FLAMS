use std::borrow::Cow;

use ftml_ontology::terms::{
    ApplicationTerm, Argument, BindingTerm, BoundArgument, ComponentVar, MaybeSequence, Term,
    Variable,
};
use ftml_solver_trace::SizedSolverRule;
use ftml_uris::{Id, SymbolUri};
use smallvec::SmallVec;

use crate::{
    Checker,
    rules::{EqualityRule, InferenceRule, SimplificationRule},
    split::SplitStrategy,
};

pub trait ImplicitExtBound {
    fn get_bound_implicits(&self) -> Option<(&Term, &[ComponentVar])>;
}
pub trait ImplicitExtApp: Sized {
    fn unapply_implicits(&self, in_prepare_revert: bool) -> Option<(&Term, &[Term])>;
}
pub trait ImplicitExtTerm: ImplicitExtBound + ImplicitExtApp {
    fn apply_implicits(self, num: usize, new: impl FnMut(usize) -> Term) -> Self;
}
impl ImplicitExtApp for ApplicationTerm {
    // invariant: return.0 matches Term::Symbol {..} or Term::Field(_)
    fn unapply_implicits(&self, in_prepare_revert: bool) -> Option<(&Term, &[Term])> {
        if !self.head.is(&*ftml_uris::metatheory::APPLY_IMPLICIT) {
            return None;
        }
        if let [
            Argument::Simple(t @ (Term::Symbol { .. } | Term::Field(_))),
            Argument::Sequence(MaybeSequence::Seq(bound)),
        ] = &*self.arguments
        {
            Some((t, bound))
        } else if in_prepare_revert {
            None
        } else {
            //panic!("WWWWEEEEIIIIRRRD: {:#?}", self.arguments);
            None
        }
    }
}
impl ImplicitExtBound for BindingTerm {
    fn get_bound_implicits(&self) -> Option<(&Term, &[ComponentVar])> {
        if !self.head.is(&*ftml_uris::metatheory::IMPLICIT_BIND) {
            return None;
        }
        if let [
            BoundArgument::BoundSeq(MaybeSequence::Seq(impls)),
            BoundArgument::Simple(body),
        ] = &*self.arguments
        {
            Some((body, impls))
        } else {
            None
        }
    }
}
impl ImplicitExtBound for Term {
    fn get_bound_implicits(&self) -> Option<(&Term, &[ComponentVar])> {
        if let Self::Bound(b) = self {
            b.get_bound_implicits()
        } else {
            None
        }
    }
}
impl ImplicitExtApp for Term {
    fn unapply_implicits(&self, in_prepare_revert: bool) -> Option<(&Term, &[Term])> {
        if let Self::Application(app) = self {
            app.unapply_implicits(in_prepare_revert)
        } else {
            None
        }
    }
}
impl ImplicitExtTerm for Term {
    fn apply_implicits(self, num: usize, mut new: impl FnMut(usize) -> Self) -> Self {
        if num == 0 {
            return self;
        }
        let mut index = 0;
        Self::Application(ApplicationTerm::new(
            (*ftml_uris::metatheory::APPLY_IMPLICIT).clone().into(),
            Box::new([
                Argument::Simple(self),
                Argument::Sequence(MaybeSequence::Seq(
                    (0..num)
                        .map(|_| {
                            let r = new(index);
                            index += 1;
                            r
                        })
                        .collect(),
                )),
            ]),
            None,
        ))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ImplicitRule;
impl SizedSolverRule for ImplicitRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("implicit arguments")
    }
    fn priority(&self) -> isize {
        100_000
    }
}
/*
impl<Split: SplitStrategy> SimplificationRule<Split> for ImplicitRule {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term
            && let Some((bd, vars)) = app.head.get_bound_implicits()
        {
            app.arguments.len() == vars.len()
        } else {
            false
        }
    }
}
 */
impl<Split: SplitStrategy> InferenceRule<Split> for ImplicitRule {
    fn applicable(&self, term: &Term) -> bool {
        term.unapply_implicits(false).is_some()
    }
    fn infer<'t>(
        &self,
        mut checker: crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Option<Term> {
        let (body, args) = term.unapply_implicits(false)?;
        let btp = checker.infer_type(body)?;
        let (tpbody, bounds) = btp.get_bound_implicits()?;
        if bounds.len() != args.len() {
            return None;
        }
        let mut substs = Vec::new();
        for (ComponentVar { var, tp, .. }, arg) in bounds.iter().zip(args) {
            if let Some(tp) = tp {
                let tp: Cow<Term> = tp / &*substs;
                if checker.scoped(|checker| checker.check_type(arg, &tp)) != Some(true) {
                    return None;
                }
                substs.push((var.name(), arg));
            }
        }
        Some((tpbody / &*substs).into_owned())
    }
}

impl<Split: SplitStrategy> Checker<Split> {
    pub(crate) fn bind_implicits(&self, nt: &Term) -> Option<Term> {
        self.collect_implicits(nt).map(|(cvs, t)| {
            if cvs.is_empty() {
                t
            } else {
                Term::Bound(BindingTerm::new(
                    ftml_uris::metatheory::IMPLICIT_BIND.clone().into(),
                    Box::new([
                        BoundArgument::BoundSeq(MaybeSequence::Seq(cvs.into_boxed_slice())),
                        BoundArgument::Simple(t),
                    ]),
                    None,
                ))
            }
        })
    }
    fn collect_implicits(&self, nt: &Term) -> Option<(Vec<ComponentVar>, Term)> {
        fn new_name(i: usize) -> Id {
            // SAFETY: valid ID
            unsafe { format!("IMPL_{i}").parse().unwrap_unchecked() }
        }
        tracing::trace!("Collecting implicits for {:?}", nt.debug_short());
        let mut allvars = nt
            .free_variables()
            .into_iter()
            .cloned()
            .collect::<SmallVec<_, 4>>();
        if allvars.is_empty() {
            return None;
        }

        let mut curr_idx = 0;
        while curr_idx < allvars.len() {
            if let Variable::Ref { declaration, .. } = &allvars[curr_idx]
                && let Ok(v) = self.get_variable(declaration)
                && let Some((tp, _)) = v.data.tp.checked_or_parsed()
            {
                let vars = tp.free_variables();
                let mut changed = false;
                for v in vars {
                    if !allvars[..curr_idx].contains(v) {
                        allvars.insert(curr_idx, v.clone());
                        changed = true;
                    }
                }
                if changed {
                    continue;
                }
            }
            curr_idx += 1;
        }
        tracing::trace!("All variables: {allvars:?}");

        //let mut counter = 1;

        let mut subst = smallvec::SmallVec::<(&str, Term), 4>::new();
        let mut dones = smallvec::SmallVec::<&str, 4>::new();
        let mut ret = Vec::new();
        for v in &allvars {
            if !dones.iter().any(|var| *var == v.name()) {
                //let name = new_name(counter);
                //counter += 1;

                let tp = if let Variable::Ref { declaration, .. } = v {
                    let var = self.get_variable(declaration).ok();
                    if let Some(df) = var
                        .as_ref()
                        .and_then(|v| v.data.df.checked_or_parsed().map(|t| t.0 / &*subst))
                    {
                        subst.push((v.name(), df));
                        continue;
                    }
                    var.and_then(|v| v.data.tp.checked_or_parsed().map(|t| t.0 / &*subst))
                } else {
                    None
                };

                dones.push(v.name());
                //subst.push((v.name(), name.clone().into()));
                ret.push(ComponentVar {
                    var: v.name_id().into_owned().into(), //name.into(),
                    df: None,
                    tp,
                });
            }
        }
        let n = nt / &*subst;
        tracing::trace!("Implicitified: {:?}", n.debug_short());
        Some((ret, n.into_owned()))
    }
}
