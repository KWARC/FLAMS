use std::borrow::Cow;

use crate::{
    CheckRef, impls::solving::TermExtSolvable, rules::implicits::ImplicitExtApp,
    split::SplitStrategy, trace::CheckingTask,
};
use ftml_ontology::terms::{
    ApplicationTerm, Argument, BindingTerm, BoundArgument, MaybeSequence, Term, eq::Alpha,
};
use ftml_solver_trace::traceref;

fn same_shape(lhs: &Term, rhs: &Term) -> bool {
    if lhs.is_solvable().is_some() || rhs.is_solvable().is_some() {
        return true;
    }
    match (lhs, rhs) {
        (Term::Symbol { uri: a, .. }, Term::Symbol { uri: b, .. }) if *a == *b => true,
        (Term::Var { .. }, Term::Var { .. })
        | (Term::Label { .. }, Term::Label { .. })
        | (Term::Number(_), Term::Number(_))
        | (Term::Field(_), Term::Field(_)) => true,
        (Term::Application(a), Term::Application(b)) if a.arguments.len() == b.arguments.len() => {
            same_shape(&a.head, &b.head)
        }
        (Term::Bound(a), Term::Bound(b)) if a.arguments.len() == b.arguments.len() => {
            same_shape(&a.head, &b.head)
        }
        _ => false,
    }
    /*matches!(
        (lhs, rhs),
        (Term::Symbol { .. }, Term::Symbol { .. })
            | (Term::Var { .. }, Term::Var { .. })
            | (Term::Field(_), Term::Field(_))
            | (Term::Label { .. }, Term::Label { .. })
            | (Term::Number(_), Term::Number(_))
            | (Term::Application(_), Term::Application(_))
            | (Term::Bound(_), Term::Bound(_))
    )*/
    /*
    if r {
        tracing::warn!(
            "Same shape:\n  - {:?}\n  - {:?}",
            lhs.debug_short(),
            rhs.debug_short()
        );
    } else {
        tracing::error!(
            "Not same shape:\n  - {:?}\n  - {:?}",
            lhs.debug_short(),
            rhs.debug_short()
        );
    }
    r
     */
}

impl<'t, Split: SplitStrategy> CheckRef<'t, '_, Split> {
    pub fn check_equality(&mut self, lhs: &'t Term, rhs: &'t Term) -> Option<bool> {
        tracing::debug!(
            "Checking equality {:?}   ==   {:?}",
            lhs.debug_short(),
            rhs.debug_short()
        );
        self.wrap_check(CheckingTask::Equality(lhs, rhs), |slf| {
            slf.check_equality_i(lhs, rhs)
        })
    }
    pub(crate) fn check_equality_i(&mut self, lhs: &'t Term, rhs: &'t Term) -> Option<bool> {
        if lhs.alpha_equal(rhs) {
            self.comment("trivial");
            return Some(true);
        }
        if let Some(unk) = lhs.is_solvable() {
            return self.solve_equality(unk, rhs);
        }
        if let Some(unk) = rhs.is_solvable() {
            return self.solve_equality(unk, lhs);
        }
        let lhs = self.subst(lhs.clone());
        let rhs = self.subst(rhs.clone());
        self.scoped(|slf| {
            match slf.simplify_rules_two(
                slf.top.rules.equality(),
                &lhs,
                &rhs,
                |_, rl, lhs, rhs| rl.applicable(lhs, rhs),
                |slf, rl, lhs, rhs| rl.apply(slf, lhs, rhs),
                |lhs, rhs| {
                    lhs.alpha_equal(rhs)
                        || lhs.is_solvable().is_some()
                        || rhs.is_solvable().is_some()
                },
            ) {
                either::Left(Some(opt)) => {
                    if !opt {
                        slf.failure("Disproven");
                    }
                    return Some(opt);
                }
                either::Right((lhs, rhs)) => {
                    if lhs.alpha_equal(&rhs) {
                        slf.comment("trivial");
                        return Some(true);
                    }
                    if let Some(unk) = lhs.is_solvable() {
                        return slf.solve_equality(unk, &rhs);
                    }
                    if let Some(unk) = rhs.is_solvable() {
                        return slf.solve_equality(unk, &lhs);
                    }
                    slf.comment("Trying congruence");
                    return slf.scoped(|slf| slf.congruence(&lhs, &rhs));
                }
                either::Left(None) => (),
            }

            slf.comment("Trying congruence");
            slf.congruence(&lhs, &rhs)
        })
    }

    fn congruence(&mut self, lhs: &'t Term, rhs: &'t Term) -> Option<bool> {
        tracing::debug!("Trying congruence");
        let Some((lhs, rhs)) = self.simplify_until_two(lhs, rhs, |_, lhs, rhs| {
            lhs.unapply_implicits(false).is_some()
                || rhs.unapply_implicits(false).is_some()
                || same_shape(lhs, rhs)
        }) else {
            return self.congruence_cont(lhs, rhs);
        };
        match (lhs, rhs) {
            (Cow::Borrowed(lhs), Cow::Borrowed(rhs)) => self.congruence_i(lhs, rhs),
            (lhs, rhs) => self.scoped(|slf| slf.congruence_i(&lhs, &rhs)),
        }
    }

    fn congruence_i(&mut self, lhs: &'t Term, rhs: &'t Term) -> Option<bool> {
        if lhs.unapply_implicits(false).is_some() || rhs.unapply_implicits(false).is_some() {
            let nlhs = self
                .simplify_implicit(lhs)
                .ok()
                .flatten()
                .map_or(Cow::Borrowed(lhs), Cow::Owned);
            let nrhs = self
                .simplify_implicit(rhs)
                .ok()
                .flatten()
                .map_or(Cow::Borrowed(rhs), Cow::Owned);
            if !lhs.alpha_equal(&nlhs) || !rhs.alpha_equal(&nrhs) {
                return self.scoped(|slf| slf.congruence(&nlhs, &nrhs));
            }
        }
        let r = match (lhs, rhs) {
            (Term::Application(l), Term::Application(r))
                if l.arguments.len() == r.arguments.len() =>
            {
                match self.traced(CheckingTask::Strategy("Trying congruence"), |slf| {
                    slf.congruence_app(l, r)
                }) {
                    Ok(r) => Some(r),
                    Err(l) => {
                        self.add_msg(l.into());
                        self.congruence_cont(lhs, rhs)
                    }
                }
            }
            (Term::Bound(l), Term::Bound(r)) if l.arguments.len() == r.arguments.len() => {
                match self.traced(CheckingTask::Strategy("Trying congruence"), |slf| {
                    slf.congruence_bind(l, r)
                }) {
                    Ok(r) => Some(r),
                    Err(l) => {
                        self.add_msg(l.into());
                        self.congruence_cont(lhs, rhs)
                    }
                }
            }
            (Term::Field(a), Term::Field(b)) if a.key == b.key => {
                self.congruence_cont(&a.record, &b.record)
            }
            (Term::Field(_), Term::Field(_)) => Some(false),
            (Term::Number(a), Term::Number(b)) => Some(a == b),
            _ => self.congruence_cont(lhs, rhs),
        };
        if r.is_some() {
            return r;
        }
        if let Some(nlhs) = self.simplify_one(super::simplify::Expansion::Full, lhs) {
            return self.scoped(|slf| slf.congruence(rhs, &nlhs));
        }
        if let Some(nrhs) = self.simplify_one(super::simplify::Expansion::Full, rhs) {
            return self.scoped(|slf| slf.congruence(&nrhs, lhs));
        }
        None
    }

    fn congruence_cont(&mut self, lhs: &'t Term, rhs: &'t Term) -> Option<bool> {
        self.add_msg(traceref!("shapes don't match: ", lhs, " and ", rhs).into());
        /*
        // LAST RESORT
        let nlhs = self
            .simplify_full(super::simplify::Expansion::Full, lhs)
            .map_or(Cow::Borrowed(lhs), Cow::Owned);
        let nrhs = self
            .simplify_full(super::simplify::Expansion::Full, rhs)
            .map_or(Cow::Borrowed(rhs), Cow::Owned);
        if *lhs != *nlhs || *rhs != *nrhs {
            self.scoped(|slf| slf.check_equality_i(&nlhs, &nrhs))
        } else {
            None
        }
         */
        None
        /*
        // todo: preserve logs on recursive fail
        if let Some(lhs) = self.simplify_one(true, lhs) {
            if self.alpha_equal(&lhs, rhs) {
                self.comment("trivial");
                return Some(true);
            }
            return self.scoped(|slf| slf.check_equality_i(&lhs, rhs));
        }
        if let Some(rhs) = self.simplify_one(true, rhs) {
            if self.alpha_equal(lhs, &rhs) {
                self.comment("trivial");
                return Some(true);
            }
            return self.scoped(|slf| slf.check_equality_i(lhs, &rhs));
        }
        None
         */
    }

    // invariant: lhs.arguments.len() == rhs.arguments.len()
    fn congruence_app(
        &mut self,
        lhs: &'t ApplicationTerm,
        rhs: &'t ApplicationTerm,
    ) -> Option<bool> {
        tracing::trace!("Comparing operators");
        self.comment("Comparing operators");
        if !self.check_equality(&lhs.head, &rhs.head)? {
            return None;
        }
        for (i, (a, b)) in lhs.arguments.iter().zip(&rhs.arguments).enumerate() {
            tracing::trace!("Comparing argument {}", i + 1);
            self.counter("Comparing arguments ", i + 1);
            if let (Argument::Simple(a), Argument::Simple(b)) = (a, b) {
                if !self.check_equality(a, b)? {
                    return None;
                }
            } else if let (
                Argument::Sequence(MaybeSequence::One(a)),
                Argument::Sequence(MaybeSequence::One(b)),
            ) = (a, b)
            {
                if !self.check_equality(a, b)? {
                    return None;
                }
            } else if let (
                Argument::Sequence(MaybeSequence::Seq(a)),
                Argument::Sequence(MaybeSequence::Seq(b)),
            ) = (a, b)
            {
                if a.len() != b.len() {
                    return None;
                }
                for (a, b) in a.iter().zip(b.iter()) {
                    if !self.check_equality(a, b)? {
                        return None;
                    }
                }
            } else {
                self.failure("Arguments don't match");
                return None;
            }
        }
        Some(true)
    }

    // invariant: lhs.arguments.len() == rhs.arguments.len()
    fn congruence_bind(&mut self, lhs: &'t BindingTerm, rhs: &'t BindingTerm) -> Option<bool> {
        self.comment("Comparing operators");
        if !self.check_equality(&lhs.head, &rhs.head)? {
            return None;
        }
        let mut substs = Alpha::new();
        macro_rules! maybe_subst {
            ($a:expr,$b:expr) => {
                if substs.is_empty()
                    || !$a.has_free_such_that(|av| substs.iter().any(|(v, _)| *v == av.name()))
                {
                    if !self.check_equality($a, $b)? {
                        return None;
                    }
                } else {
                    let subst = substs
                        .iter()
                        .map(|(n, v)| {
                            (
                                n,
                                Term::Var {
                                    variable: (*v).clone(),
                                    presentation: None,
                                },
                            )
                        })
                        .collect::<smallvec::SmallVec<_, 2>>();
                    let r = match $a / &*subst {
                        Cow::Borrowed(_) => self.check_equality($a, $b)?,
                        Cow::Owned(a) => self.scoped(|slf| slf.check_equality(&a, $b))?,
                    };
                    if !r {
                        return None;
                    }
                }
            };
        }
        for (i, (a, b)) in lhs.arguments.iter().zip(&rhs.arguments).enumerate() {
            self.counter("Comparing arguments ", i + 1);
            match (a, b) {
                (BoundArgument::Simple(a), BoundArgument::Simple(b)) => {
                    maybe_subst!(a, b);
                }
                (BoundArgument::Bound(a), BoundArgument::Bound(b))
                | (
                    BoundArgument::BoundSeq(MaybeSequence::One(a)),
                    BoundArgument::BoundSeq(MaybeSequence::One(b)),
                ) => {
                    match (a.tp.as_ref(), b.tp.as_ref()) {
                        (Some(a), Some(b)) => {
                            maybe_subst!(a, b);
                        }
                        (None, None) => (),
                        _ => return None,
                    }
                    match (a.df.as_ref(), b.df.as_ref()) {
                        (Some(a), Some(b)) => {
                            maybe_subst!(a, b);
                        }
                        (None, None) => (),
                        _ => return None,
                    }
                    self.extend_context(b);
                    if a.var.name() != b.var.name() {
                        substs.push((a.var.name(), &b.var));
                    }
                }
                (
                    BoundArgument::BoundSeq(MaybeSequence::Seq(sa)),
                    BoundArgument::BoundSeq(MaybeSequence::Seq(sb)),
                ) if sa.len() == sb.len() => {
                    for (a, b) in sa.iter().zip(sb.iter()) {
                        match (a.tp.as_ref(), b.tp.as_ref()) {
                            (Some(a), Some(b)) => {
                                maybe_subst!(a, b);
                            }
                            (None, None) => (),
                            _ => return None,
                        }
                        match (a.df.as_ref(), b.df.as_ref()) {
                            (Some(a), Some(b)) => {
                                maybe_subst!(a, b);
                            }
                            (None, None) => (),
                            _ => return None,
                        }
                        self.extend_context(b);
                        if a.var.name() != b.var.name() {
                            substs.push((a.var.name(), &b.var));
                        }
                    }
                }
                _ => {
                    self.failure(format!("Arguments do not match: {a:?}  <-->  {b:?}"));
                    return None;
                }
            }
        }
        Some(true)
    }
}

/*
*
// -----------------------------------------------------------

pub fn alpha_equal_traced(lhs: &Term, rhs: &Term) -> bool {
    alpha_equal_with_traced(lhs, rhs, &mut Alpha::default())
}

const CHECK: bool = true;

macro_rules! rep_eq {
    (false@$lhs:expr,$rhs:expr) => {
        if CHECK {
            ::tracing::error!(
                "Not equal ({},{}): {:?}    and   {:?}",
                line!(),
                column!(),
                $lhs,
                $rhs
            );
            false
        } else {
            false
        }
    };
    ($lhs:expr,$rhs:expr) => {
        if CHECK {
            $lhs == $rhs || {
                ::tracing::error!(
                    "Not equal ({},{}): {:?}    and   {:?}",
                    line!(),
                    column!(),
                    $lhs,
                    $rhs
                );
                false
            }
        } else {
            $lhs == $rhs
        }
    };
}

pub fn alpha_equal_with_traced<'t>(lhs: &'t Term, rhs: &'t Term, alpha: &mut Alpha<'t>) -> bool {
    if lhs == rhs {
        return true;
    }
    match (lhs, rhs) {
        (Term::Var { variable: v1, .. }, Term::Var { variable: v2, .. }) => {
            rep_eq!(v1.name(), v2.name())
                || alpha.iter().any(|(a, b)| {
                    (*a == v1.name() && b.name() == v2.name())
                        || (b.name() == v1.name() && *a == v2.name())
                })
        }
        (Term::Application(a), Term::Application(b))
            if rep_eq!(a.arguments.len(), b.arguments.len()) =>
        {
            alpha_equal_with_traced(&a.head, &b.head, alpha)
                && a.arguments
                    .iter()
                    .zip(b.arguments.iter())
                    .all(|(a, b)| alpha_arg_traced(a, b, alpha))
        }
        (Term::Bound(a), Term::Bound(b)) if rep_eq!(a.arguments.len(), b.arguments.len()) => {
            let mut pop = 0;
            if !alpha_equal_with_traced(&a.head, &b.head, alpha)
                || a.arguments.iter().zip(b.arguments.iter()).any(|(a, b)| {
                    alpha_barg_traced(a, b, alpha)
                        .inspect(|i| pop += i)
                        .is_none()
                })
            {
                return false;
            }
            for _ in 0..pop {
                alpha.pop();
            }
            true
        }
        (Term::Field(a), Term::Field(b)) => {
            alpha_equal_with_traced(&a.record, &b.record, alpha) && a.key == b.key
        }
        (
            Term::Label {
                name: na,
                df: da,
                tp: ta,
            },
            Term::Label {
                name: nb,
                df: db,
                tp: tb,
            },
        ) if *na == *nb => {
            match (da, db) {
                (Some(a), Some(b)) => {
                    if !alpha_equal_with_traced(a, b, alpha) {
                        return false;
                    }
                }
                (None, None) => (),
                _ => return false,
            }
            match (ta, tb) {
                (Some(a), Some(b)) => alpha_equal_with_traced(a, b, alpha),
                (None, None) => true,
                _ => false,
            }
        }
        (Term::Number(a), Term::Number(b)) => rep_eq!(a, b),
        _ => rep_eq!(false@lhs,rhs),
    }
}

fn alpha_arg_traced<'t>(lhs: &'t Argument, rhs: &'t Argument, alpha: &mut Alpha<'t>) -> bool {
    match (lhs, rhs) {
        (Argument::Simple(lhs), Argument::Simple(rhs))
        | (
            Argument::Sequence(MaybeSequence::One(lhs)),
            Argument::Sequence(MaybeSequence::One(rhs)),
        ) => alpha_equal_with_traced(lhs, rhs, alpha),
        (
            Argument::Sequence(MaybeSequence::Seq(lhs)),
            Argument::Sequence(MaybeSequence::Seq(rhs)),
        ) if lhs.len() == rhs.len() => lhs
            .iter()
            .zip(rhs.iter())
            .all(|(lhs, rhs)| alpha_equal_with_traced(lhs, rhs, alpha)),
        _ => rep_eq!(false@lhs,rhs),
    }
}
fn alpha_barg_traced<'t>(
    lhs: &'t BoundArgument,
    rhs: &'t BoundArgument,
    alpha: &mut Alpha<'t>,
) -> Option<usize> {
    macro_rules! ret {
        ($e:expr) => {
            if $e { Some(0) } else { None }
        };
    }
    match (lhs, rhs) {
        (BoundArgument::Simple(lhs), BoundArgument::Simple(rhs))
        | (
            BoundArgument::Sequence(MaybeSequence::One(lhs)),
            BoundArgument::Sequence(MaybeSequence::One(rhs)),
        ) => ret!(alpha_equal_with_traced(lhs, rhs, alpha)),
        (
            BoundArgument::Sequence(MaybeSequence::Seq(lhs)),
            BoundArgument::Sequence(MaybeSequence::Seq(rhs)),
        ) if rep_eq!(lhs.len(), rhs.len()) => ret!(
            lhs.iter()
                .zip(rhs.iter())
                .all(|(lhs, rhs)| alpha_equal_with_traced(lhs, rhs, alpha))
        ),
        (BoundArgument::Bound(lhs), BoundArgument::Bound(rhs))
        | (
            BoundArgument::BoundSeq(MaybeSequence::One(lhs)),
            BoundArgument::BoundSeq(MaybeSequence::One(rhs)),
        ) => {
            if alpha_cv_traced(lhs, rhs, alpha) {
                Some(1)
            } else {
                None
            }
        }
        (
            BoundArgument::BoundSeq(MaybeSequence::Seq(lhs)),
            BoundArgument::BoundSeq(MaybeSequence::Seq(rhs)),
        ) if rep_eq!(lhs.len(), rhs.len()) => {
            if lhs
                .iter()
                .zip(rhs.iter())
                .all(|(a, b)| alpha_cv_traced(a, b, alpha))
            {
                Some(lhs.len())
            } else {
                None
            }
        }
        _ => None,
    }
}
fn alpha_cv_traced<'t>(
    lhs: &'t ComponentVar,
    rhs: &'t ComponentVar,
    alpha: &mut Alpha<'t>,
) -> bool {
    match (lhs.tp.as_ref(), rhs.tp.as_ref()) {
        (Some(lhs), Some(rhs)) => {
            if !alpha_equal_with_traced(lhs, rhs, alpha) {
                return false;
            }
        }
        (None, None) => (),
        _ => return rep_eq!(false@lhs,rhs),
    }
    match (lhs.df.as_ref(), rhs.df.as_ref()) {
        (Some(lhs), Some(rhs)) => {
            if !alpha_equal_with_traced(lhs, rhs, alpha) {
                return false;
            }
        }
        (None, None) => (),
        _ => return rep_eq!(false@lhs,rhs),
    }
    alpha.push((lhs.var.name(), &rhs.var));
    true
}

// -----------------------------------------------------------
*/
