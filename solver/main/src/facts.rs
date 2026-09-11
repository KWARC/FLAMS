use std::borrow::Cow;

use ftml_ontology::{
    domain::declarations::symbols::Symbol,
    narrative::{SharedDocumentElement, elements::VariableDeclaration},
    terms::{Argument, ComponentVar, Term, Variable, helpers::Bound, patterns::Pattern},
};
use ftml_uris::{DocumentElementUri, Id, SymbolUri};
use smallvec::SmallVec;

use crate::{
    CheckRef, Checker, hoas::HOASSymbols, impls::solving::TermExtSolvable,
    rules::implicits::ImplicitExtBound, split::SplitStrategy,
};

impl<Split: SplitStrategy> Checker<Split> {
    pub(crate) fn add_fact(&mut self, s: &Symbol) {
        // SAFETY: valid ID
        static NOAUTO: std::sync::LazyLock<Id> =
            std::sync::LazyLock::new(|| unsafe { "noauto".parse().unwrap_unchecked() });
        if !s.data.role.contains(&*NOAUTO)
            && let Some(hoas) = self.hoas()
            && let Some(fact) = Fact::from_symbol(hoas, s, self, |uri| {
                crate::impls::backend::get_variable(
                    &self.backend,
                    &self.documents,
                    &self.current,
                    uri,
                    |t| self.prepare(None, t).1,
                )
            })
        {
            self.facts.facts.push((s.uri.clone(), fact));
            /*self.facts.add(hoas, s, );*/
        }
    }
}

impl<Split: SplitStrategy> CheckRef<'_, '_, Split> {
    pub fn facts_for(
        &self,
        goal: &Term,
    ) -> impl Iterator<Item = (GlobalOrLocal, Vec<GoalPremise>)> {
        self.context
            .facts()
            .find_applicable(self.context.goal_counter(), goal)
            .chain(self.top.facts.find_applicable(
                goal,
                self.context.goal_counter(),
                self.context.blocked(),
            ))
    }
}
#[derive(Clone)]
pub enum GlobalOrLocal {
    Global(SymbolUri),
    Local(usize),
}
impl GlobalOrLocal {
    pub(crate) fn into_term(self, ctx: &[Cow<'_, ComponentVar>]) -> Term {
        match self {
            Self::Global(s) => s.into(),
            Self::Local(i) => ctx[i].var.clone().into(),
        }
    }
}
impl std::fmt::Display for GlobalOrLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global(uri) => uri.name().fmt(f),
            Self::Local(i) => i.fmt(f),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct GlobalFacts {
    facts: Vec<(SymbolUri, Fact)>,
}
impl GlobalFacts {
    pub fn find_applicable(
        &self,
        goal: &Term,
        counter: &std::sync::atomic::AtomicUsize,
        blocked: &[GlobalOrLocal],
    ) -> impl Iterator<Item = (GlobalOrLocal, Vec<GoalPremise>)> {
        self.facts.iter().filter_map(|(uri, fact)| {
            if blocked.iter().any(|b| {
                if let GlobalOrLocal::Global(b) = b {
                    b == uri
                } else {
                    false
                }
            }) {
                return None;
            }
            fact.applies(goal, counter)
                .map(|r| (GlobalOrLocal::Global(uri.clone()), r))
        })
    }
}

#[derive(Default, Clone)]
pub(crate) struct LocalFacts {
    pub(crate) facts: Vec<(usize, Fact)>,
}
impl LocalFacts {
    pub fn find_applicable(
        &self,
        counter: &std::sync::atomic::AtomicUsize,
        goal: &Term,
    ) -> impl Iterator<Item = (GlobalOrLocal, Vec<GoalPremise>)> {
        self.facts.iter().filter_map(|(idx, fact)| {
            fact.applies(goal, counter)
                .map(|r| (GlobalOrLocal::Local(*idx), r))
        })
    }
}

impl std::fmt::Debug for LocalFacts {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ls = f.debug_list();
        for f in &self.facts {
            ls.entry(f);
        }
        ls.finish()
    }
}

impl std::fmt::Debug for GlobalFacts {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ls = f.debug_list();
        for f in &self.facts {
            ls.entry(&(f.0.name(), &f.1));
        }
        ls.finish()
    }
}

#[derive(Clone)]
pub(crate) struct TypeGuard {
    pub name: Id,
    pub tp: Term,
    pub is_sequence: bool,
    pub is_premise: bool,
}
impl std::fmt::Debug for TypeGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_premise {
            write!(
                f,
                "Premise({}: ({:?}){})",
                self.name,
                self.tp.debug_short(),
                if self.is_sequence { "*" } else { "" }
            )
        } else {
            write!(
                f,
                "{}: ({:?}){}",
                self.name,
                self.tp.debug_short(),
                if self.is_sequence { "*" } else { "" }
            )
        }
    }
}

#[derive(Clone)]
pub(crate) struct Fact {
    pub type_guards: Box<[TypeGuard]>,
    //pub judgment: SymbolUri,
    pub pattern: Pattern,
}
#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for Fact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fact")
            .field("guards", &self.type_guards)
            .field("pattern", &self.pattern)
            .finish()
    }
}
impl Fact {
    pub fn applies(
        &self,
        goal: &Term,
        counter: &std::sync::atomic::AtomicUsize,
    ) -> Option<Vec<GoalPremise>> {
        fn get_name(counter: &std::sync::atomic::AtomicUsize) -> Id {
            let i = counter.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            // SAFETY: valid ID
            unsafe { format!("SGL_{i}").parse().unwrap_unchecked() }
        }
        fn do_tg<'s>(
            slf: &'s Fact,
            mut tgs: impl Iterator<Item = &'s TypeGuard>,
            ret: &mut Vec<GoalPremise>,
            subst: &mut Vec<(&'s str, Cow<'s, Term>)>,
            ass: &'s [Cow<'s, Term>],
            counter: &std::sync::atomic::AtomicUsize,
        ) {
            while let Some(tg) = tgs.next() {
                if let Some(idx) = slf.pattern.vars.iter().position(|v| *v == tg.name) {
                    let p = &*ass[idx];
                    ret.push(GoalPremise::Typing {
                        elem: p.clone(),
                        tp: tg.tp.clone() / &**subst,
                        is_sequence: tg.is_sequence,
                    });
                    subst.push((tg.name.as_ref(), Cow::Borrowed(p)));
                } else if tg.is_premise {
                    ret.push(GoalPremise::Proof(tg.tp.clone() / &**subst));
                } else {
                    let name = get_name(counter);
                    let tp = tg.tp.clone() / &**subst;
                    let is_sequence = tg.is_sequence;
                    let mut premises = Vec::new();
                    let idx = ret.len();
                    subst.push((tg.name.as_ref(), Cow::Owned(name.clone().into())));
                    do_tg(slf, tgs, ret, subst, ass, counter);
                    let mut curr = idx;
                    while let Some(e) = ret.get(curr) {
                        if e.uses(&name) {
                            let e = ret.remove(curr);
                            premises.push(e)
                        } else {
                            curr += 1;
                        }
                    }
                    ret.insert(
                        idx,
                        GoalPremise::NeedSuchThat {
                            name,
                            tp,
                            is_sequence,
                            premises,
                        },
                    );
                    return;
                }
            }
        }
        let ass = self.pattern.matches(goal)?;
        let mut ret = Vec::new();
        let mut subst = Vec::new();
        do_tg(
            self,
            self.type_guards.iter(),
            &mut ret,
            &mut subst,
            &ass,
            counter,
        );
        Some(ret)
    }

    pub fn from_tp<Split: SplitStrategy>(
        hoas: &HOASSymbols,
        tp: &Term,
        checker: &Checker<Split>,
    ) -> Option<Self> {
        return None;
        if tp.has_solvable() {
            return None;
        }
        tracing::trace!("Fact?");
        let Some(judg) = hoas.judgment.as_ref() else {
            //tracing::warn!("No judgment");
            return None;
        };
        let mut type_guards = Vec::new();
        let stp = checker
            .wrap_none(None, |mut slf| {
                slf.simplify_full(crate::impls::simplify::Expansion::Full, tp)
            })
            .1;
        let mut curr = stp.as_ref().unwrap_or(tp);
        loop {
            if let Some([Argument::Simple(prop)]) = curr.unapply(judg) {
                let pat = Pattern::from(prop.clone(), true);
                return Some(Self {
                    type_guards: type_guards.into_boxed_slice(),
                    //judgment: judg,
                    pattern: pat,
                });
            }
            if let Some(Bound {
                var,
                tp,
                body,
                is_sequence,
            }) = curr.unbind(&hoas.pi)
            {
                let name = var.name_id().into_owned();
                let tg = if !body.has_free_such_that(|v| v.name() == name.as_ref())
                    && let Some([Argument::Simple(premise)]) = tp.unapply(judg)
                {
                    TypeGuard {
                        name,
                        tp: premise.clone(),
                        is_sequence,
                        is_premise: true,
                    }
                } else {
                    TypeGuard {
                        name,
                        tp: tp.clone(),
                        is_sequence,
                        is_premise: false,
                    }
                };
                type_guards.push(tg);
                curr = body;
            } else {
                //tracing::warn!("Type does not match: {:?} ({:?})", curr.debug_short(), curr);
                return None;
            }
        }
    }

    pub fn from_symbol<Split: SplitStrategy>(
        hoas: &HOASSymbols,
        s: &Symbol,
        checker: &Checker<Split>,
        get: impl Fn(&DocumentElementUri) -> Result<SharedDocumentElement<VariableDeclaration>, ()>,
    ) -> Option<Self> {
        let Some(judg) = hoas.judgment.as_ref() else {
            //tracing::warn!("No judgment");
            return None;
        };
        let Some(tp) = s.data.tp.checked_or_parsed() else {
            //tracing::warn!("No type");
            return None;
        };
        if !tp.1 {
            return None;
        }
        let tp =
            tp.0.get_bound_implicits()
                .map(|(t, _)| t.clone())
                .unwrap_or(tp.0);
        /*let tp = checker
        .wrap_none(None, |mut slf| slf.simplify_full(true, &tp))
        .1
        .unwrap_or(tp);*/
        let allvars = tp.free_variables();
        let mut type_guards = Vec::new();
        let mut curr = &tp;
        loop {
            if let Some([Argument::Simple(prop)]) = curr.unapply(judg) {
                let pat = Pattern::from(prop.clone(), true);

                let mut allvars = allvars.into_iter().cloned().collect::<SmallVec<_, 4>>();
                let mut curr_idx = 0;
                let mut added_since = 0;
                let mut insert_idx = 0;
                while curr_idx < allvars.len() {
                    if let Variable::Ref { declaration, .. } = &allvars[curr_idx]
                        && let Ok(v) = get(declaration)
                        && let Some((tp, _)) = v.data.tp.checked_or_parsed()
                    {
                        let vars = tp.free_variables();
                        let mut changed = false;
                        for v in vars {
                            if matches!(v, Variable::Ref { .. }) && !allvars[..curr_idx].contains(v)
                            {
                                allvars.insert(curr_idx, v.clone());
                                changed = true;
                            }
                        }
                        if !type_guards
                            .iter()
                            .any(|tg: &TypeGuard| tg.name.as_ref() == v.uri.name().last())
                        {
                            let name = allvars[curr_idx].name_id().into_owned();
                            let is_sequence = v.data.is_seq;
                            let tg = if !pat.vars.contains(&name)
                                && let Some([Argument::Simple(premise)]) = tp.unapply(judg)
                            {
                                TypeGuard {
                                    name,
                                    tp: premise.clone(),
                                    is_sequence,
                                    is_premise: true,
                                }
                            } else {
                                TypeGuard {
                                    name,
                                    tp,
                                    is_sequence,
                                    is_premise: false,
                                }
                            };
                            type_guards.insert(insert_idx, tg);
                            added_since += 1;
                        }
                        if changed {
                            continue;
                        }
                    }
                    insert_idx += added_since;
                    added_since = 0;
                    curr_idx += 1;
                }

                let ret = Self {
                    type_guards: type_guards.into_boxed_slice(),
                    //judgment: judg,
                    pattern: pat,
                };

                if ret.pattern.body.has_solvable()
                    || ret.type_guards.iter().any(|g| g.tp.has_solvable())
                {
                    return None;
                }

                return Some(ret);
            }
            if let Some(Bound {
                var,
                tp,
                body,
                is_sequence,
            }) = curr.unbind(&hoas.pi)
            {
                let name = var.name_id().into_owned();
                let tg = if !body.has_free_such_that(|v| v.name() == name.as_ref())
                    && let Some([Argument::Simple(premise)]) = tp.unapply(judg)
                {
                    TypeGuard {
                        name,
                        tp: premise.clone(),
                        is_sequence,
                        is_premise: true,
                    }
                } else {
                    TypeGuard {
                        name,
                        tp: tp.clone(),
                        is_sequence,
                        is_premise: false,
                    }
                };
                type_guards.push(tg);
                curr = body;
            } else {
                //tracing::warn!("Type does not match: {:?} ({:?})", curr.debug_short(), curr);
                return None;
            }
        }
    }
}

pub enum GoalPremise {
    Typing {
        elem: Term,
        tp: Term,
        is_sequence: bool,
    },
    Proof(Term),
    NeedSuchThat {
        name: Id,
        tp: Term,
        is_sequence: bool,
        premises: Vec<Self>,
    },
}
impl GoalPremise {
    fn uses(&self, name: &Id) -> bool {
        match self {
            Self::Typing { elem, tp, .. } => {
                elem.has_free_such_that(|v| v.name() == name.as_ref())
                    || tp.has_free_such_that(|v| v.name() == name.as_ref())
            }
            Self::Proof(t) => t.has_free_such_that(|v| v.name() == name.as_ref()),
            Self::NeedSuchThat { tp, premises, .. } => {
                tp.has_free_such_that(|v| v.name() == name.as_ref())
                    || premises.iter().any(|p| p.uses(name))
            }
        }
    }
}
impl std::fmt::Debug for GoalPremise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Typing {
                elem,
                tp,
                is_sequence,
            } => write!(
                f,
                "{:?}  :  {:?}{}",
                elem.debug_short(),
                tp.debug_short(),
                if *is_sequence { "*" } else { "" }
            ),
            Self::Proof(tm) => write!(f, "⊢ {:?}", tm.debug_short()),
            Self::NeedSuchThat {
                name,
                tp,
                is_sequence,
                premises,
            } => {
                write!(
                    f,
                    "SOME {name} : {:?}{}",
                    tp.debug_short(),
                    if *is_sequence { "*" } else { "" }
                )?;
                if !premises.is_empty() {
                    f.write_str(" such that [")?;
                    for p in premises {
                        p.fmt(f)?;
                    }
                    f.write_str("]")?;
                }
                Ok(())
            }
        }
    }
}
