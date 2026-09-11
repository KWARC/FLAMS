use std::borrow::Cow;

use ftml_ontology::terms::{
    ApplicationTerm, Argument, BindingTerm, BoundArgument, ComponentVar, MaybeSequence, Term,
};
use ftml_solver_trace::{CheckerRule, CheckingTask, traceref};
use ftml_uris::Id;

use crate::{
    CheckRef,
    impls::solving::{TermExtSolvable, is_solvable_var},
    rules::{
        implicits::{ImplicitExtApp, ImplicitExtBound},
        unknowns::beta_unknowns_cow,
    },
    split::SplitStrategy,
};

const LOOP_LIMIT: usize = 64;
const RECURSION_LIMIT: usize = 128;
const TOTAL_LIMIT: usize = 4096 * 512;

static NOEXPAND: std::sync::LazyLock<Id> =
    std::sync::LazyLock::new(|| unsafe { "noexpand".parse().unwrap_unchecked() });

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Expansion {
    Full,
    NoDefinitionExpansion,
    NoNoexpands,
}
impl Expansion {
    fn with_defs(self) -> Self {
        if self == Self::NoNoexpands {
            Self::NoNoexpands
        } else {
            Self::Full
        }
    }
}

impl<'t, Split: SplitStrategy> CheckRef<'t, '_, Split> {
    pub fn simplify_full(&mut self, expand: Expansion, term: &'t Term) -> Option<Term> {
        if matches!(term, Term::Symbol { .. } | Term::Number(_)) {
            return None;
        }
        self./*wrap_check*/untraced(CheckingTask::Simplify(term), |slf| {
            slf.simplify_full_i(expand, term,&mut 0,&mut 0)
        }).inspect(|t| {
            self.add_msg(traceref!("Simplified: ",t.clone()).into());
        })
    }
    fn simplify_full_first(
        &mut self,
        expand: Expansion,
        term: &'t Term,
        recursion_limit: &mut usize,
        total_limit: &mut usize,
    ) -> Option<Cow<'t, Term>> {
        match term {
            Term::Symbol { uri, .. } if expand == Expansion::NoNoexpands => {
                let s = self.get_symbol(uri).ok()?;
                if s.data.role.contains(&NOEXPAND) {
                    None
                } else {
                    self.get_symbol_definiens(uri).map(|t| {
                        self.comment("expanded definition");
                        Some(Cow::Owned(t))
                    })?
                }
            }
            Term::Symbol { uri, .. } /*if expand == Expansion::Full*/ => {
                self.get_symbol_definiens(uri).map(|t| {
                    self.comment("expanded definition");
                    Some(Cow::Owned(t))
                })?
            }
            Term::Var { variable, .. } => {
                if let Some(name) = is_solvable_var(variable)
                    && let Some(t) = self.get_solution(name)
                {
                    Some(Cow::Owned(t))
                } else if
                /*expand != Expansion::NoDefinitionExpansion
                &&*/
                let Some(df) = self.get_var_definiens(variable) {
                    Some(Cow::Owned(df))
                } else {
                    //println!("  : None");
                    None
                }
            }
            Term::Number(_) => {
                //println!("  : None");
                None
            }
            Term::Application(app) => {
                let mut changed = false;
                let nhead = self
                    .simplify_full_i(expand, &app.head, recursion_limit, total_limit)
                    .map_or(Cow::Borrowed(&app.head), |t| {
                        changed = true;
                        Cow::Owned(t)
                    });
                let args = app
                    .arguments
                    .iter()
                    .map(|a| {
                        self.arg_full(expand, a, recursion_limit, total_limit)
                            .map_or(Cow::Borrowed(a), |a| {
                                changed = true;
                                Cow::Owned(a)
                            })
                    })
                    .collect::<Vec<_>>();
                Some(if changed {
                    Cow::Owned(Term::Application(ApplicationTerm::new(
                        nhead.into_owned(),
                        args.into_iter().map(Cow::into_owned).collect(),
                        app.presentation.clone(),
                    )))
                } else {
                    Cow::Borrowed(term)
                })
            }
            Term::Bound(app) => {
                let mut changed = false;
                let nhead = self
                    .simplify_full_i(expand.with_defs(), &app.head, recursion_limit, total_limit)
                    .map_or(Cow::Borrowed(&app.head), |t| {
                        changed = true;
                        Cow::Owned(t)
                    });
                let args = app
                    .arguments
                    .iter()
                    .map(|a| {
                        self.bound_arg_full(expand, a, recursion_limit, total_limit)
                            .map_or(Cow::Borrowed(a), |a| {
                                changed = true;
                                Cow::Owned(a)
                            })
                    })
                    .collect::<Vec<_>>();
                Some(if changed {
                    Cow::Owned(Term::Bound(BindingTerm::new(
                        nhead.into_owned(),
                        args.into_iter().map(Cow::into_owned).collect(),
                        app.presentation.clone(),
                    )))
                } else {
                    Cow::Borrowed(term)
                })
            }
            _ => Some(Cow::Borrowed(term)),
        }
    }
    fn simplify_full_i(
        &mut self,
        expand: Expansion,
        term: &'t Term,
        recursion_limit: &mut usize,
        total_limit: &mut usize,
    ) -> Option<Term> {
        if expand == Expansion::Full
            && let Some(r) = self
                .judgment_cache
                .get_simplification(term, self.context.as_ref())
        {
            return Some(r);
        }
        tracing::debug!(
            "Fully Simplifying {:?} (expand:{expand:?})",
            term.debug_short()
        );
        if
        /*expand != Expansion::NoDefinitionExpansion
        &&*/
        true {
            match self.simplify_implicit(term) {
                Ok(Some(t)) => {
                    let r = self
                        .scoped(|slf| slf.simplify_full_i(expand, &t, recursion_limit, total_limit))
                        .unwrap_or(t);
                    return Some(r);
                }
                Ok(None) => (),
                Err(()) => return None,
            }
        }
        /*if term.unapply_implicits().is_some() {
            return None;
        } else*/
        //self.scoped(|slf| {
        *recursion_limit += 1;
        let Some(mut current) =
            self.simplify_full_first(expand, term, recursion_limit, total_limit)
        else {
            *recursion_limit -= 1;
            return None;
        };
        let mut loop_limit = 0;
        loop {
            loop_limit += 1;
            *total_limit += 1;
            if loop_limit >= LOOP_LIMIT
                || *recursion_limit >= RECURSION_LIMIT
                || *total_limit >= TOTAL_LIMIT
            {
                *recursion_limit -= 1;
                return match current {
                    Cow::Borrowed(_) => {
                        //println!("  : None");
                        None
                    }
                    Cow::Owned(t) => {
                        /*if loop_limit >= LOOP_LIMIT {
                            panic!("LOOP_LIMIT");
                        }
                        if *recursion_limit >= RECURSION_LIMIT {
                            panic!("RECURSION_LIMIT");
                        }
                        if *total_limit >= TOTAL_LIMIT {
                            panic!("TOTAL_LIMIT");
                        }*/
                        if expand == Expansion::Full {
                            self.judgment_cache
                                .add_simplification(term, &t, self.context.as_ref());
                        }
                        //println!("  : {:?}", t.debug_short());
                        Some(t)
                    }
                };
            }
            if let Some(next) = self.scoped(|slf| slf.simplify_one(expand, &current)) {
                current = Cow::Owned(next);
            } else {
                return match current {
                    Cow::Borrowed(_) => {
                        //println!("  : None");
                        *recursion_limit -= 1;
                        None
                    }
                    Cow::Owned(mut t) => {
                        while let Some(r) = self.scoped(|slf| {
                            slf.simplify_full_i(expand, &t, recursion_limit, total_limit)
                        }) {
                            t = r;
                        }
                        //println!("  : {:?}", t.debug_short());

                        if expand == Expansion::Full {
                            self.judgment_cache
                                .add_simplification(term, &t, self.context.as_ref());
                        }
                        *recursion_limit -= 1;
                        Some(t)
                    }
                };
            }
        }
        //})
    }

    pub fn simplify_until(
        &mut self,
        term: &'t Term,
        mut until: impl FnMut(&Self, &Term) -> bool,
    ) -> Option<Cow<'t, Term>> {
        if until(self, term) {
            return Some(Cow::Borrowed(term));
        }
        self.wrap_check(CheckingTask::Simplify(term), |slf| {
            slf.simplify_until_i(term, until, &mut 0, &mut 0)
        })
    }
    fn simplify_until_i(
        &mut self,
        term: &'t Term,
        mut until: impl FnMut(&Self, &Term) -> bool,
        recursion_limit: &mut usize,
        total_limit: &mut usize,
    ) -> Option<Cow<'t, Term>> {
        if until(self, term) {
            return Some(Cow::Borrowed(term));
        }
        let mut current = Cow::<'t, _>::Borrowed(term);
        *recursion_limit += 1;
        let mut loop_limit = 0;
        loop {
            loop_limit += 1;
            *total_limit += 1;
            if *recursion_limit >= RECURSION_LIMIT
                || loop_limit >= LOOP_LIMIT
                || *total_limit >= TOTAL_LIMIT
            {
                *recursion_limit -= 1;
                return None;
            }
            let Some(next) = self.scoped(|slf| slf.simplify_one(Expansion::Full, &current)) else {
                *recursion_limit -= 1;
                //self.comment(format!("Final simplification: {:?}", current.debug_short()));
                return None;
            };
            if until(self, &next) {
                *recursion_limit -= 1;
                return Some(Cow::Owned(next));
            }
            current = Cow::Owned(next);
        }
    }

    pub(crate) fn simplify_rules<Rl: CheckerRule + ?Sized, R>(
        &mut self,
        rules: &'t [Box<Rl>],
        term: &'t Term,
        applicable: impl Fn(&Rl, &Term) -> bool,
        apply: impl for<'s> Fn(CheckRef<'s, '_, Split>, &Rl, &'s Term) -> Option<R> + Send + Sync,
    ) -> Option<R>
    where
        R: Send + Sync + std::fmt::Debug + Clone + 'static,
    {
        let mut applicables = smallvec::SmallVec::<_, 2>::default();
        match self.simplify_until(term, |_, t| {
            applicables = rules
                .iter()
                .filter_map(|rl| {
                    if applicable(&**rl, t) {
                        Some(&**rl)
                    } else {
                        None
                    }
                })
                .collect();
            !applicables.is_empty()
        }) {
            Some(Cow::Borrowed(term)) => {
                if let Some(r) =
                    Split::split(self, true, applicables, |slf, rl| apply(slf, rl, term))
                {
                    return Some(r);
                }
                self.simplify_one(Expansion::Full, term).and_then(|term| {
                    self.scoped(|slf| slf.simplify_rules(rules, &term, applicable, apply))
                })
            }
            Some(Cow::Owned(term)) => self.scoped(|slf| {
                if let Some(r) =
                    Split::split(slf, true, applicables, |slf, rl| apply(slf, rl, &term))
                {
                    return Some(r);
                }
                slf.simplify_one(Expansion::Full, &term).and_then(|term| {
                    slf.scoped(|slf| slf.simplify_rules(rules, &term, applicable, apply))
                })
            }),
            None => {
                self.failure("No rule applicable (1)");
                //println!("Here: {}", term.debug_short());
                None
            }
        }
    }

    pub(crate) fn simplify_until_two(
        &mut self,
        term1: &'t Term,
        term2: &'t Term,
        mut until: impl FnMut(&Self, &Term, &Term) -> bool,
    ) -> Option<(Cow<'t, Term>, Cow<'t, Term>)> {
        let mut left = true;
        let mut right = true;
        let mut next_left = true;
        let mut t1 = Cow::Borrowed(term1);
        let mut t2 = Cow::Borrowed(term2);
        loop {
            if next_left && left {
                if until(self, &t1, &t2) {
                    return Some((t1, t2));
                }
                if right {
                    next_left = false;
                }
                if let Some(next) = self.scoped(|slf| slf.simplify_one(Expansion::Full, &t1)) {
                    t1 = Cow::Owned(next);
                    continue;
                }
                left = false;
            }
            if right {
                if until(self, &t1, &t2) {
                    return Some((t1, t2));
                }
                next_left = true;
                if let Some(next) = self.scoped(|slf| slf.simplify_one(Expansion::Full, &t2)) {
                    t2 = Cow::Owned(next);
                    continue;
                }
                right = false;
                continue;
            }
            break;
        }
        self.add_msg(
            traceref!(FAIL
                "Final simplifications: ",
                t1.into_owned(),
                " and ",
                t2.into_owned()
            )
            .into(),
        );
        None
    }

    pub(crate) fn simplify_rules_two<Rl: CheckerRule + ?Sized + 'static, R>(
        &mut self,
        rules: &'t [Box<Rl>],
        term1: &'t Term,
        term2: &'t Term,
        applicable: impl Fn(&CheckRef<'_, '_, Split>, &Rl, &Term, &Term) -> bool,
        apply: impl for<'s> Fn(CheckRef<'s, '_, Split>, &Rl, &'s Term, &'s Term) -> Option<R> + Sync,
        abort: impl Fn(&Term, &Term) -> bool + Send + Sync,
    ) -> either::Either<Option<R>, (Term, Term)>
    where
        R: Send + Sync + std::fmt::Debug + Clone + 'static,
    {
        //println!("Simplify rules two {{");
        let mut applicables = smallvec::SmallVec::<_, 2>::default();
        let mut left = true;
        let mut right = true;
        let mut next_left = true;
        let mut t1 = Cow::Borrowed(term1);
        let mut t2 = Cow::Borrowed(term2);
        loop {
            macro_rules! set {
                () => {
                    set!(NOBREAK);
                    if !applicables.is_empty() {
                        break;
                    }
                };
                (NOBREAK) => {
                    if abort(&*t1, &*t2) {
                        //println!("}}");
                        return either::Right((t1.into_owned(), t2.into_owned()));
                    }
                    applicables = rules
                        .iter()
                        .filter_map(|rl| {
                            if applicable(self, &**rl, &*t1, &*t2) {
                                Some(&**rl)
                            } else {
                                None
                            }
                        })
                        .collect();
                };
            }
            loop {
                /*let s = t1.debug_short().to_string();
                let s2 = t2.debug_short().to_string();
                if s.contains("\"universal quantifier\"[") || s2.contains("\"universal quantifier\"[") {
                    println!("Here: {s}  ===  {s2}");
                }*/

                if next_left && left {
                    set!();
                    if right {
                        next_left = false;
                    }
                    if let Some(next) = self.scoped(|slf| slf.simplify_one(Expansion::Full, &t1)) {
                        t1 = Cow::Owned(next);
                        continue;
                    }
                    left = false;
                }
                if right {
                    set!();
                    if left {
                        next_left = true;
                    }
                    if let Some(next) = self.scoped(|slf| slf.simplify_one(Expansion::Full, &t2)) {
                        t2 = Cow::Owned(next);
                        continue;
                    }
                    right = false;
                    if left {
                        continue;
                    }
                }
                break;
            }

            if applicables.is_empty() {
                self.failure("No rule applicable (2)");
                self.add_msg(
                    traceref!(FAIL
                        "Final simplifications: ",
                        t1.into_owned(),
                        " and ",
                        t2.into_owned()
                    )
                    .into(),
                );
                //println!("}}");
                return either::Left(None);
            }
            if let Some(r) = self.scoped(|slf| {
                Split::split(slf, true, std::mem::take(&mut applicables), |slf, rl| {
                    apply(slf, rl, &t1, &t2)
                })
            }) {
                //println!("}}");
                return either::Left(Some(r));
            }
            if next_left && left {
                if right {
                    next_left = false;
                }
                if let Some(next) = self.scoped(|slf| slf.simplify_one(Expansion::Full, &t1)) {
                    t1 = Cow::Owned(next);
                    //set!(NOBREAK);
                    continue;
                }
                left = false;
            }
            if right {
                if left {
                    next_left = true;
                }
                if let Some(next) = self.scoped(|slf| slf.simplify_one(Expansion::Full, &t2)) {
                    t2 = Cow::Owned(next);
                    //set!(NOBREAK);
                    continue;
                }
                right = false;
                if left {
                    continue;
                }
            }
            break;
        }
        self.failure("No rule applicable (3)");
        //println!("}}");
        either::Left(None)
    }

    pub(super) fn simplify_one(&mut self, expand: Expansion, term: &'t Term) -> Option<Term> {
        if term.has_solvable() {
            let nterm = self.subst(term.clone());
            if *term != nterm {
                return Some(nterm); //self.scoped(|slf| slf.simplify_one_i(expand, &nterm));
            }
        }
        if let Cow::Owned(nterm) = beta_unknowns_cow(term)
            && *term != nterm
        {
            return Some(nterm); //self.scoped(|slf| slf.simplify_one_i(expand, &nterm));
        }

        /*
        let s = term.debug_short().to_string();
        if s.contains("\"universal quantifier\"[") {
            println!("Here: {:?}", term.debug_short());
        }
        */

        if
        /*expand != Expansion::NoDefinitionExpansion
        &&*/
        true {
            match self.simplify_implicit(term) {
                Ok(Some(t)) => return Some(t),
                Ok(None) => (),
                Err(()) => return None,
            }
        }
        self.simplify_one_i(expand, term)
    }
    fn simplify_one_i(&mut self, expand: Expansion, term: &'t Term) -> Option<Term> {
        /**/
        let applicables = self
            .top
            .rules
            .simplification()
            .iter()
            .filter_map(|rl| {
                if rl.applicable(term) {
                    Some(&**rl)
                } else {
                    None
                }
            })
            .collect::<smallvec::SmallVec<_, 2>>();
        match Split::split(self, false, applicables, |slf, rl| {
            match rl.apply(slf, term) {
                Ok(t) => Some(either::Left(t)),
                Err(Some(v)) => Some(either::Right(v)),
                _ => None,
            }
        }) {
            Some(either::Left(t)) => return Some(t),
            Some(_) => {
                // TODO
            }
            None => (),
        }

        /*
        let s = term.debug_short().to_string();
        if s.contains("\"universal quantifier\"[") {
            println!("here: {s}");
        }
         */

        self.simplify_one_default(expand, term)
    }

    fn simplify_morph(&mut self, expand: Expansion, term: &'t Term) -> Option<Term> {
        let (m, r) = crate::rules::MorphismRule::is_morphism_appl(term, self)?;
        /*if let Some((m2, r2)) = crate::rules::MorphismRule::is_morphism_appl(r, self)
            && let Ok(t) = m2.apply(r2, |s| {
                self.top
                    .get_symbol_like(s, |t| self.prepare(t, None).1)
                    .ok()
            })
            && *t != *r2
        {
            let r = Term::Application(ApplicationTerm::new(
                m.uri.clone().into(),
                Box::new([Argument::Simple(t.into_owned())]),
                None,
            ));
            return self.scoped(|slf| slf.simplify_morph(expand, &r));
        }*/
        if matches!(r, Term::Symbol { .. })
            && let Some(nt) = self.simplify_one_default(expand, r)
            && let Ok(nt) = m.apply(&nt, &mut |s| {
                self.top
                    .get_symbol_like(s, |t| self.prepare(t, None).1)
                    .ok()
            })
            && *nt != *term
        {
            Some(nt.into_owned())
        } else if let Ok(Some(nt)) = self.simplify_implicit(r)
            && let Ok(nt) = m.apply(&nt, &mut |s| {
                self.top
                    .get_symbol_like(s, |t| self.prepare(t, None).1)
                    .ok()
            })
            && *nt != *term
        {
            Some(nt.into_owned())
        } else {
            None
        }
    }

    fn simplify_one_default(&mut self, expand: Expansion, term: &'t Term) -> Option<Term> {
        if
        /*expand != Expansion::NoDefinitionExpansion
        &&*/
        true {
            match self.simplify_implicit(term) {
                Ok(Some(t)) => return Some(t),
                Ok(None) => (),
                Err(()) => return None,
            }
        }
        if let Some(t) = self.simplify_morph(expand, term) {
            return Some(t);
        }
        match term {
            // Definition Expansion
            Term::Symbol { uri, .. } if expand == Expansion::NoNoexpands => {
                let s = self.get_symbol(uri).ok()?;
                if s.data.role.contains(&NOEXPAND) {
                    None
                } else {
                    self.get_symbol_definiens(uri).inspect(|_| {
                        self.comment("expanded definition");
                    })
                }
            }
            Term::Symbol { uri, .. } /*if expand == Expansion::Full*/ => {
                self.get_symbol_definiens(uri).inspect(|_| {
                    self.comment("expanded definition");
                })
            }
            Term::Var { variable, .. }
                if true/*expand != Expansion::NoDefinitionExpansion*/
                    || is_solvable_var(variable).is_some() =>
            {
                self.get_var_definiens(variable).inspect(|_| {
                    self.comment("expanded definition");
                })
            }

            Term::Application(app) if self.top.hoas().is_some_and(|h| h.judgment.as_ref().is_some_and(|j|
                app.head.is(j)
            )) && let [Argument::Simple(p)] = &*app.arguments => {
                self.simplify_one(expand, p).map(|r| Term::Application(ApplicationTerm::new(
                    app.head.clone(),Box::new([Argument::Simple(r)]),app.presentation.clone()
                )))
            }

            Term::Application(app) => self.simplify_one(expand, &app.head).map(|nh| {
                Term::Application(ApplicationTerm::new(
                    nh,
                    app.arguments.clone(),
                    app.presentation.clone(),
                ))
            }),
            Term::Bound(app) => self.simplify_one(expand.with_defs(), &app.head).map(|nh| {
                Term::Bound(BindingTerm::new(
                    nh,
                    app.arguments.clone(),
                    app.presentation.clone(),
                ))
            }),
            _ => None,
        }
    }

    fn arg_full(
        &mut self,
        expand: Expansion,
        arg: &'t Argument,
        recursion_limit: &mut usize,
        total_limit: &mut usize,
    ) -> Option<Argument> {
        match arg {
            Argument::Simple(t) => self
                .simplify_full_i(expand, t, recursion_limit, total_limit)
                .map(Argument::Simple),
            Argument::Sequence(MaybeSequence::One(t)) => self
                .simplify_full_i(expand, t, recursion_limit, total_limit)
                .map(|t| Argument::Sequence(MaybeSequence::One(t))),
            Argument::Sequence(MaybeSequence::Seq(ts)) => {
                let mut changed = false;
                let nts = ts
                    .iter()
                    .map(|t| {
                        self.simplify_full_i(expand, t, recursion_limit, total_limit)
                            .map_or(Cow::Borrowed(t), |a| {
                                changed = true;
                                Cow::Owned(a)
                            })
                    })
                    .collect::<Vec<_>>();
                if changed {
                    Some(Argument::Sequence(MaybeSequence::Seq(
                        nts.into_iter().map(Cow::into_owned).collect(),
                    )))
                } else {
                    None
                }
            }
        }
    }
    fn bound_arg_full(
        &mut self,
        expand: Expansion,
        arg: &'t BoundArgument,
        recusion_limit: &mut usize,
        total_limit: &mut usize,
    ) -> Option<BoundArgument> {
        match arg {
            BoundArgument::Simple(t) => self
                .simplify_full_i(expand, t, recusion_limit, total_limit)
                .map(BoundArgument::Simple),
            BoundArgument::Bound(cv) => self
                .cv_full(expand, cv, recusion_limit, total_limit)
                .map(BoundArgument::Bound),
            BoundArgument::Sequence(MaybeSequence::One(t)) => self
                .simplify_full_i(expand, t, recusion_limit, total_limit)
                .map(|t| BoundArgument::Sequence(MaybeSequence::One(t))),
            BoundArgument::BoundSeq(MaybeSequence::One(cv)) => self
                .cv_full(expand, cv, recusion_limit, total_limit)
                .map(|t| BoundArgument::BoundSeq(MaybeSequence::One(t))),
            BoundArgument::Sequence(MaybeSequence::Seq(ts)) => {
                let mut changed = false;
                let nts = ts
                    .iter()
                    .map(|t| {
                        self.simplify_full_i(expand, t, recusion_limit, total_limit)
                            .map_or(Cow::Borrowed(t), |a| {
                                changed = true;
                                Cow::Owned(a)
                            })
                    })
                    .collect::<Vec<_>>();
                if changed {
                    Some(BoundArgument::Sequence(MaybeSequence::Seq(
                        nts.into_iter().map(Cow::into_owned).collect(),
                    )))
                } else {
                    None
                }
            }
            BoundArgument::BoundSeq(MaybeSequence::Seq(cvs)) => {
                let mut changed = false;
                let ncvs = cvs
                    .iter()
                    .map(|t| {
                        self.cv_full(expand, t, recusion_limit, total_limit).map_or(
                            Cow::Borrowed(t),
                            |a| {
                                changed = true;
                                Cow::Owned(a)
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                if changed {
                    Some(BoundArgument::BoundSeq(MaybeSequence::Seq(
                        ncvs.into_iter().map(Cow::into_owned).collect(),
                    )))
                } else {
                    None
                }
            }
        }
    }

    fn cv_full(
        &mut self,
        expand: Expansion,
        arg: &'t ComponentVar,
        recursion_limit: &mut usize,
        total_limit: &mut usize,
    ) -> Option<ComponentVar> {
        match (arg.tp.as_ref(), arg.df.as_ref()) {
            (None, None) => None,
            (Some(tp), None) => self
                .simplify_full_i(expand, tp, recursion_limit, total_limit)
                .map(|tp| ComponentVar {
                    var: arg.var.clone(),
                    tp: Some(tp),
                    df: None,
                }),
            (None, Some(df)) => self
                .simplify_full_i(expand, df, recursion_limit, total_limit)
                .map(|df| ComponentVar {
                    var: arg.var.clone(),
                    tp: None,
                    df: Some(df),
                }),
            (Some(tp), Some(df)) => {
                let ntp = self.simplify_full_i(expand, tp, recursion_limit, total_limit);
                let ndf = self.simplify_full_i(expand, df, recursion_limit, total_limit);
                if ntp.is_none() && ndf.is_none() {
                    return None;
                }
                Some(ComponentVar {
                    var: arg.var.clone(),
                    tp: Some(ntp.unwrap_or_else(|| tp.clone())),
                    df: Some(ndf.unwrap_or_else(|| df.clone())),
                })
            }
        }
    }

    pub(crate) fn simplify_implicit(&mut self, term: &'t Term) -> Result<Option<Term>, ()> {
        let Some((Term::Symbol { uri, .. }, args)) = term.unapply_implicits(false) else {
            return Ok(None);
        };
        let Some(def) = self.get_symbol_definiens(uri) else {
            self.comment(format!("Symbol has no definiens: {uri}"));
            return Err(());
        };
        let Some((def, vars)) = def.get_bound_implicits() else {
            self.failure("definiens has no implicit arguments");
            return Err(());
        };
        if args.len() != vars.len() {
            self.failure("number of implicit arguments don't match");
            return Err(());
        };
        let mut substs = Vec::new();
        for (ComponentVar { var, tp, .. }, arg) in vars.iter().zip(args) {
            if let Some(tp) = tp {
                let tp: Cow<Term> = tp / &*substs;
                if self.scoped(|checker| checker.check_type(arg, &tp)) != Some(true) {
                    return Err(());
                }
                substs.push((var.name(), arg));
            }
        }
        let r = def / &*substs;
        tracing::debug!("Unapplied implicits: {:?}", r.debug_short());
        Ok(Some(r.into_owned()))
    }
}
