use std::borrow::{Borrow, Cow};

use ftml_ontology::terms::{
    ApplicationTerm, Argument, BindingTerm, BoundArgument, ComponentVar, MaybeSequence, Term,
    Variable,
};
use ftml_uris::Id;
use smallvec::SmallVec;

use crate::{
    CheckRef, Checker,
    rules::unknowns::beta_unknowns,
    split::SplitStrategy,
    utils::{Merge, MutableRefList},
};

const PREFIX: &str = "SOLVE!";

pub trait TermExtSolvable {
    fn is_solvable(&self) -> Option<&Id>;
    fn has_solvable(&self) -> bool;
    fn solvables(&self) -> SmallVec<&Variable, 2>;
}

/*
impl TermExtSolvable for Variable {
    fn is_solvable(&self) -> Option<&Id> {
        let Self::Name { name, .. } = self else {
            return None;
        };
        if name.as_ref().starts_with(PREFIX)
            && name.as_ref().as_bytes()[PREFIX.len()..]
                .iter()
                .all(u8::is_ascii_digit)
        {
            Some(name)
        } else {
            None
        }
    }
    #[inline]
    fn has_solvable(&self) -> bool {
        self.is_solvable().is_some()
    }
}
 */

pub fn is_solvable_id(name: &Id) -> bool {
    name.as_ref().starts_with(PREFIX)
        && name.as_ref().as_bytes()[PREFIX.len()..]
            .iter()
            .all(u8::is_ascii_digit)
}

pub fn is_solvable_var(var: &Variable) -> Option<&Id> {
    let Variable::Name { name, .. } = var else {
        return None;
    };
    if is_solvable_id(name) {
        Some(name)
    } else {
        None
    }
}

impl TermExtSolvable for Term {
    fn is_solvable(&self) -> Option<&Id> {
        if let Self::Var { variable, .. } = self {
            is_solvable_var(variable)
        } else if let Self::Application(app) = self
            && let Self::Var { variable, .. } = &app.head
        {
            is_solvable_var(variable)
        } else {
            None
        }
    }
    fn has_solvable(&self) -> bool {
        self.has_free_such_that(|v| is_solvable_var(v).is_some())
    }
    fn solvables(&self) -> SmallVec<&Variable, 2> {
        self.free_variables()
            .into_iter()
            .filter(|s| is_solvable_var(s).is_some())
            .collect()
    }
}

#[derive(Clone)]
pub enum BoundedValue {
    None,
    Solved(Term),
    Bounded(Option<Term>, Option<Term>),
}
impl std::fmt::Debug for BoundedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("(None)"),
            Self::Solved(t) => t.debug_short().fmt(f),
            Self::Bounded(a, b) => write!(
                f,
                "({:?} <= _ <= {:?})",
                a.as_ref().map(Term::debug_short),
                b.as_ref().map(Term::debug_short)
            ),
        }
    }
}

#[derive(Clone)]
pub struct Solvable {
    pub(crate) name: Id,
    solution: BoundedValue,
    context: Vec<ComponentVar>,
    tp: BoundedValue,
}
impl Solvable {
    pub(crate) fn new(name: Id, context: impl Iterator<Item = ComponentVar>) -> Self {
        Self {
            name,
            solution: BoundedValue::None,
            tp: BoundedValue::None,
            context: context.collect(),
        }
    }

    pub const fn solution(&self) -> Option<&Term> {
        if let BoundedValue::Solved(t) = &self.solution {
            Some(t)
        } else {
            None
        }
    }
}
impl std::fmt::Debug for Solvable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct CtxWrap<'i>(&'i [ComponentVar]);
        impl std::fmt::Debug for CtxWrap<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                for ComponentVar { var, tp, df } in self.0 {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    var.name().fmt(f)?;
                    if let Some(tp) = tp {
                        write!(f, " : {:?}", tp.debug_short())?;
                    }
                    if let Some(df) = df {
                        write!(f, " := {:?}", df.debug_short())?;
                    }
                }
                Ok(())
            }
        }
        write!(
            f,
            "{} := {{{:?}}} {:?} (: {:?})",
            self.name,
            CtxWrap(&self.context),
            self.solution,
            self.tp
        )
    }
}
impl PartialEq for Solvable {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Solvable {}
impl Borrow<Id> for Solvable {
    #[inline]
    fn borrow(&self) -> &Id {
        &self.name
    }
}
impl std::hash::Hash for Solvable {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Solutions(pub(crate) rustc_hash::FxHashSet<Solvable>);
impl Merge for Solutions {
    fn merge(&mut self, other: Self) {
        for e in other.0 {
            self.0.remove(&e);
            self.0.insert(e);
        }
    }
}
impl std::hash::Hash for Solutions {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for s in self.0.iter() {
            s.hash(state);
        }
    }
}

impl MutableRefList<'_, Solutions> {
    #[inline]
    fn get_solvable<'s>(&'s self, name: &Id) -> Option<&'s Solvable> {
        fn get_i<'s>(
            slf: &'s MutableRefList<Solutions>,
            name: &Id,
            first: Option<&Id>,
        ) -> Option<&'s Solvable> {
            let s = slf.find(|s| s.0.get(name))?;
            if let BoundedValue::Solved(s) = &s.solution
                && let Some(s) = s.is_solvable()
                && first != Some(s)
            {
                get_i(slf, s, Some(first.unwrap_or(name)))
            } else {
                Some(s)
            }
        }
        get_i(self, name, None)
    }

    fn set_solution(&mut self, name: Id, solution: Term) {
        let (tp, context) = self
            .get_solvable(&name)
            .map_or((BoundedValue::None, Vec::new()), |e| {
                (e.tp.clone(), e.context.clone())
            });
        self.0.remove(&name);

        let ne = Solvable {
            name,
            solution: BoundedValue::Solved(solution),
            tp,
            context,
        };
        self.0.insert(ne);
    }
    fn set_type(&mut self, name: Id, tp: Term) {
        let (solution, context) = self
            .get_solvable(&name)
            .map_or((BoundedValue::None, Vec::new()), |e| {
                (e.solution.clone(), e.context.clone())
            });
        let ne = Solvable {
            name,
            solution,
            tp: BoundedValue::Solved(tp),
            context,
        };

        self.0.remove(&ne);
        self.0.insert(ne);
    }
    #[must_use]
    pub fn subst(&self, term: Term) -> Term {
        fn subst_i(slf: &MutableRefList<Solutions>, term: Term) -> Term {
            /*
            beta_unknowns(
                term.modify(|t| {
                    if let Term::Var { variable, .. } = t
                        && let Some(var) = is_solvable_var(variable)
                    {
                        self.iter().flat_map(|s| s.0.iter()).find_map(|s| {
                            if s.name == *var
                                && let BoundedValue::Solved(t) = &s.solution
                            {
                                Some(std::ops::ControlFlow::Continue(t.clone()))
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
                .into_owned(),
            )
            */
            let freevars = term.free_variables();
            if freevars.is_empty() {
                drop(freevars);
                return term;
            }
            let mut ret = smallvec::SmallVec::<_, 2>::new();
            for v in slf.iter().flat_map(|m| m.0.iter()) {
                if let BoundedValue::Solved(tm) = &v.solution
                    && freevars.iter().any(|f| f.name() == v.name.as_ref())
                {
                    ret.push((v.name.to_string(), slf.subst(tm.clone()))); //get(curr.0, curr.1, tm))); //tm.clone()));
                }
            }
            drop(freevars);

            //term / ret.as_slice()
            match &term / ret.as_slice() {
                Cow::Borrowed(_) => term,
                Cow::Owned(t) if t.has_solvable() => {
                    tracing::trace!(
                        "substituted {:?}\n  =>  {:?}",
                        term.debug_short(),
                        t.debug_short()
                    );
                    subst_i(slf, t)
                }
                Cow::Owned(t) => t,
            }
        }
        beta_unknowns(subst_i(self, term))
    }
}

impl<Split: SplitStrategy> Checker<Split> {
    pub(crate) fn new_solvable(&self) -> Id {
        let i = self
            .implicits
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // SAFETY: is a valid Id
        unsafe { format!("{PREFIX}{i}").parse().unwrap_unchecked() }
    }
}

fn apply_solvable(name: Id, ctx: impl ExactSizeIterator<Item = Variable>) -> Term {
    let head: Term = Variable::Name {
        name,
        notated: None,
    }
    .into();
    if ctx.len() == 0 {
        head
    } else {
        Term::Application(ApplicationTerm::new(
            head,
            Box::new([Argument::Sequence(MaybeSequence::Seq(
                ctx.map(Into::into).collect(),
            ))]),
            None,
        ))
    }
}

impl<Split: SplitStrategy> CheckRef<'_, '_, Split> {
    #[must_use]
    pub fn new_solvable(&mut self) -> Term {
        let name = self.top.new_solvable();
        self.add_solvable(name.clone());
        apply_solvable(name, self.context.as_ref().iter().map(|cv| cv.var.clone()))
    }
    pub(crate) fn solve_equality(&mut self, unk: &Id, solution: &Term) -> Option<bool> {
        self.comment(format!("solving unknown {unk}"));
        let Some(unks) = self.get_solvable(unk) else {
            self.failure("Unknown unknown!");
            return Some(false);
        };
        let solution = self.subst(solution.clone());

        if let BoundedValue::Solved(tm) = &unks.solution {
            let tm = tm.clone();
            let ctx = unks
                .context
                .iter()
                .cloned()
                .map(cleanup_cv)
                .collect::<Vec<_>>();
            self.comment("already solved");
            let solution = if ctx.is_empty() {
                solution
            } else {
                Term::Bound(BindingTerm::new(
                    (*ftml_uris::metatheory::BIND_UNKNOWNS).clone().into(),
                    Box::new([
                        BoundArgument::BoundSeq(MaybeSequence::Seq(ctx.into_boxed_slice())),
                        BoundArgument::Simple(solution),
                    ]),
                    None,
                ))
            };
            return self.scoped(|slf| slf.check_equality(&solution, &tm));
        }
        self.solve(unk.clone(), solution)?;
        Some(true)
    }

    pub(crate) fn solve_upper_bound(&mut self, unk: &Id, bound: &Term) -> Option<bool> {
        tracing::debug!("Solving upper bound");
        self.comment(format!("solving boundaries of unknown {unk}"));
        let Some(unks) = self.get_solvable(unk) else {
            self.failure("Unknown unknown!");
            return Some(false);
        };
        let bound = self.subst(bound.clone());

        if let BoundedValue::Solved(tm) = &unks.solution {
            let tm = tm.clone();
            let ctx = unks
                .context
                .iter()
                .cloned()
                .map(cleanup_cv)
                .collect::<Vec<_>>();
            self.comment("already solved");
            let bound = if ctx.is_empty() {
                bound
            } else {
                Term::Bound(BindingTerm::new(
                    (*ftml_uris::metatheory::BIND_UNKNOWNS).clone().into(),
                    Box::new([
                        BoundArgument::BoundSeq(MaybeSequence::Seq(ctx.into_boxed_slice())),
                        BoundArgument::Simple(bound),
                    ]),
                    None,
                ))
            };
            return self.scoped(|slf| slf.check_subtype(&tm, &bound));
        }
        /*
        if let BoundedValue::Bounded(lower, upper) = &unks.solution {
            let lower = lower.clone();
            let upper = upper.clone();
            drop(unks);
            return context.in_branch(|context| {
                if let Some(lower) = lower {
                    trace.comment("Checking against previous lower bound");
                    let r = self.check_subtype(trace, context, &lower, sup);
                    if r != Some(true) {
                        return r;
                    }
                }
                if let Some(upper) = upper {
                    trace.comment("Checking against previous upper bound");
                    let r = self.check_subtype(trace, context, &upper, sup);
                    if r != Some(true) {
                        return r;
                    }
                }
            });
        }

        trace.comment(format!("Solving upper type bound of {unk}"));
         */
        self.solve(unk.clone(), bound)?;
        Some(true)
    }

    pub(crate) fn solve_lower_bound(&mut self, unk: &Id, bound: &Term) -> Option<bool> {
        tracing::debug!("Solving lower bound");
        self.comment(format!("solving boundaries of unknown {unk}"));
        let Some(unks) = self.get_solvable(unk) else {
            self.failure("Unknown unknown!");
            return Some(false);
        };

        let bound = self.subst(bound.clone());

        // todo boundary checks
        //
        if let BoundedValue::Solved(tm) = &unks.solution {
            let tm = tm.clone();
            let ctx = unks
                .context
                .iter()
                .cloned()
                .map(cleanup_cv)
                .collect::<Vec<_>>();
            self.comment("already solved");
            let bound = if ctx.is_empty() {
                bound
            } else {
                Term::Bound(BindingTerm::new(
                    (*ftml_uris::metatheory::BIND_UNKNOWNS).clone().into(),
                    Box::new([
                        BoundArgument::BoundSeq(MaybeSequence::Seq(ctx.into_boxed_slice())),
                        BoundArgument::Simple(bound),
                    ]),
                    None,
                ))
            };
            return self.scoped(|slf| slf.check_subtype(&bound, &tm));
        }
        self.solve(unk.clone(), bound)?;
        Some(true)
    }

    #[inline]
    pub(crate) fn merge_solutions(&mut self, solutions: Solutions) {
        self.solutions.merge(solutions);
    }
    fn add_solvable(&mut self, name: Id) {
        self.solutions.0.insert(Solvable::new(
            name,
            self.context.as_ref().iter().map(|v| v.clone().into_owned()),
        ));
    }

    pub fn get_solution(&self, name: &Id) -> Option<Term> {
        self.get_solvable(name)
            .and_then(Solvable::solution)
            .map(|t| self.subst(t.clone()))
    }

    fn get_solvable<'s>(&'s self, name: &Id) -> Option<&'s Solvable> {
        self.solutions.get_solvable(name)
        /*
        fn get<'i>(
            sols: &'i rustc_hash::FxHashSet<Solvable>,
            anc: Option<Ancestor<'i>>,
            name: &Id,
        ) -> Option<&'i Solvable> {
            sols.get(name)
                .or_else(|| anc.and_then(|Ancestor { p, gp }| get(p, gp.copied(), name)))
        }
        fn get_solvable_i<'s, Split: SplitStrategy>(
            slf: &'s CheckRef<Split>,
            name: &Id,
            first: Option<&Id>,
        ) -> Option<&'s Solvable> {
            let s = get(slf.solutions, slf.parent_solutions, name)?;
            if let BoundedValue::Solved(s) = &s.solution
                && let Some(s) = s.is_solvable()
                && first != Some(s)
            {
                get_solvable_i(slf, s, Some(first.unwrap_or(name)))
            } else {
                Some(s)
            }
        }
        get_solvable_i(self, name, None)
         */
    }

    pub(crate) fn get_solvable_type(&mut self, name: &Id) -> Term {
        let ctx = if let Some(s) = self.get_solvable(name) {
            if let BoundedValue::Solved(t) = &s.tp {
                return t.clone();
            }
            Some(
                s.context
                    .iter()
                    .cloned()
                    .map(cleanup_cv)
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let ctx = ctx.unwrap_or(Vec::new());
        let tp_name = self.top.new_solvable();
        self.solutions
            .0
            .insert(Solvable::new(tp_name.clone(), ctx.iter().cloned()));
        let tp: Term = if ctx.is_empty() {
            tp_name.into()
        } else {
            let body = apply_solvable(tp_name, ctx.iter().map(|cv| cv.var.clone()));
            Term::Bound(BindingTerm::new(
                (*ftml_uris::metatheory::BIND_UNKNOWNS).clone().into(),
                Box::new([
                    BoundArgument::BoundSeq(MaybeSequence::Seq(ctx.into_boxed_slice())),
                    BoundArgument::Simple(body),
                ]),
                None,
            ))
        };
        self.solve_type(name.clone(), tp.clone());
        tp
    }

    fn solve(&mut self, name: Id, solution: Term) -> Option<()> {
        let Some((ctx, tp)) = self.get_solvable(&name).map(|s| {
            (
                s.context
                    .iter()
                    .cloned()
                    .map(cleanup_cv)
                    .collect::<Vec<_>>(),
                (), /*s.tp.clone()*/
            )
        }) else {
            self.failure("Unknown not found");
            return None;
        };
        let solution = self.subst(solution);
        if solution.has_free_such_that(|v| v.name() == name.as_ref()) {
            tracing::debug!("Circular solution! {:?}", solution.debug_short());
            self.failure(format!("Circular solution: {:?}", solution.debug_short()));
            return None;
        }
        let solution = if ctx.is_empty() {
            solution
        } else {
            Term::Bound(BindingTerm::new(
                (*ftml_uris::metatheory::BIND_UNKNOWNS).clone().into(),
                Box::new([
                    BoundArgument::BoundSeq(MaybeSequence::Seq(ctx.into_boxed_slice())),
                    BoundArgument::Simple(solution),
                ]),
                None,
            ))
        };
        tracing::debug!("solving {name} as {:?}", solution.debug_short());
        self.comment(format!("Solved {name} as {:?}", solution.debug_short()));
        /*if let BoundedValue::Solved(tp) = tp {
            self.comment("Checking against previous type solution");
            self.scoped(|slf| slf.check_type(&solution, &tp))?;
        }*/
        self.solutions.set_solution(name, solution);
        Some(())
    }
    fn solve_type(&mut self, name: Id, tp: Term) {
        let tp = self.subst(tp);
        self.solutions.set_type(name, tp);
    }

    pub(crate) fn subst(&self, term: Term) -> Term {
        self.solutions.subst(term) //Self::subst_map(term, self.solutions, self.parent_solutions)
    }
}

fn cleanup_cv(cv: ComponentVar) -> ComponentVar {
    ComponentVar {
        var: cv.var,
        tp: None,
        df: None,
    }
}
