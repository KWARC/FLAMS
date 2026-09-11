use std::borrow::Cow;

use ftml_ontology::terms::{ApplicationTerm, Argument, BoundArgument, MaybeSequence, Term};
use ftml_solver_trace::SizedSolverRule;

use crate::{
    impls::solving::TermExtSolvable,
    rules::{InferenceRule, SimplificationRule},
    split::SplitStrategy,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct UnknownsRule;
impl SizedSolverRule for UnknownsRule {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("bound unknowns")
    }
    fn priority(&self) -> isize {
        100_000
    }
}

fn unsolved<'t, Split: SplitStrategy>(
    mut checker: crate::CheckRef<'t, '_, Split>,
    app: &'t ApplicationTerm,
) -> Option<Term> {
    let [Argument::Sequence(MaybeSequence::Seq(args))] = &*app.arguments else {
        checker.failure("argument is not simple");
        return None;
    };
    let tp = checker.infer_type(&app.head)?;
    let Term::Bound(b) = tp else {
        checker.failure("not a binder");
        return None;
    };
    if !b.head.is(&*ftml_uris::metatheory::BIND_UNKNOWNS) {
        checker.failure("Not an unknown binder");
        return None;
    }
    let [
        BoundArgument::BoundSeq(MaybeSequence::Seq(vs)),
        BoundArgument::Simple(body),
    ] = &*b.arguments
    else {
        checker.failure("arguments don't match");
        return None;
    };
    if vs.len() != args.len() {
        checker.failure("lengths don't match");
        return None;
    }
    let mut substs = smallvec::SmallVec::<_, 2>::new();
    for (v, arg) in vs.iter().zip(args.iter()) {
        if let Some(tp) = v.tp.as_ref() {
            let tp = tp / &*substs;
            if !checker.scoped(|slf| slf.check_type(arg, &tp))? {
                return None;
            }
            substs.push((v.var.name(), arg));
        }
    }
    Some(body.clone() / &*substs)
}
impl<Split: SplitStrategy> SimplificationRule<Split> for UnknownsRule {
    fn applicable(&self, term: &ftml_ontology::terms::Term) -> bool {
        if let Term::Application(app) = term
            && let Term::Bound(b) = &app.head
            && b.head.is(&*ftml_uris::metatheory::BIND_UNKNOWNS)
            && let [
                BoundArgument::BoundSeq(MaybeSequence::Seq(vs)),
                BoundArgument::Simple(_),
            ] = &*b.arguments
            && let [Argument::Sequence(MaybeSequence::Seq(args))] = &*app.arguments
        {
            vs.len() == args.len()
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        // TODO should check types
        checker: crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        match beta_unknowns_cow(term) {
            Cow::Owned(term) => Ok(term),
            _ => Err(None),
        }
    }
}

impl<Split: SplitStrategy> InferenceRule<Split> for UnknownsRule {
    fn applicable(&self, term: &ftml_ontology::terms::Term) -> bool {
        let Term::Application(app) = term else {
            return false;
        };
        if app.head.is_solvable().is_some() {
            return true;
        }
        if let Term::Bound(b) = &app.head
            && b.head.is(&*ftml_uris::metatheory::BIND_UNKNOWNS)
            && let [
                BoundArgument::BoundSeq(MaybeSequence::Seq(vs)),
                BoundArgument::Simple(_),
            ] = &*b.arguments
            && let [Argument::Sequence(MaybeSequence::Seq(args))] = &*app.arguments
        {
            vs.len() == args.len()
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
        if app.head.is_solvable().is_some() {
            unsolved(checker, app)
        } else if let Cow::Owned(t) = beta_unknowns_cow(term) {
            // TODO should check types ?
            checker.scoped(|slf| slf.infer_type(&t))
        } else {
            None
        }
    }
}

pub(crate) fn beta_unknowns(t: Term) -> Term {
    if let Cow::Owned(t) = beta_unknowns_cow(&t) {
        t
    } else {
        t
    }
}

pub(crate) fn beta_unknowns_cow(t: &Term) -> Cow<'_, Term> {
    //tracing::warn!("Applying beta to {:?}", t.debug_short());
    let r = t.modify(|t| {
        if let Term::Application(app) = t
            && let Term::Bound(b) = &app.head
            && b.head.is(&*ftml_uris::metatheory::BIND_UNKNOWNS)
            && let [
                BoundArgument::BoundSeq(MaybeSequence::Seq(vs)),
                BoundArgument::Simple(body),
            ] = &*b.arguments
            && let [Argument::Sequence(MaybeSequence::Seq(args))] = &*app.arguments
            && vs.len() == args.len()
        {
            let substs = vs
                .iter()
                .map(|i| i.var.name())
                .zip(args.iter())
                .collect::<smallvec::SmallVec<_, 2>>();
            let r = body.clone() / &*substs;
            /*tracing::warn!(
                "Substituting: {:?}\n        {:?}",
                t.debug_short(),
                r.debug_short()
            );*/
            Some(std::ops::ControlFlow::Continue(r))
        } else {
            None
        }
    });
    /*match &r {
        Cow::Owned(t) => tracing::warn!("Changed: {:?}", t.debug_short()),
        _ => tracing::warn!("unchanged"),
    }*/
    r
}
