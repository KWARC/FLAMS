use std::borrow::Cow;

use crate::{
    CheckRef,
    impls::solving::TermExtSolvable,
    rules::{InhabitableRule, UniverseRule},
    split::SplitStrategy,
    trace::CheckingTask,
};
use ftml_ontology::terms::Term;
use smallvec::SmallVec;

impl<'t, Split: SplitStrategy> CheckRef<'t, '_, Split> {
    pub fn check_type(&mut self, tm: &'t Term, tp: &'t Term) -> Option<bool> {
        tracing::debug!(
            "Checking Typing {:?}   ::   {:?}",
            tm.debug_short(),
            tp.debug_short()
        );
        //crate::pause();
        self.wrap_check(CheckingTask::HasType(tm, tp), |slf| {
            slf.check_type_i(tm, tp)
        })
    }

    pub fn check_subtype(&mut self, sub: &'t Term, sup: &'t Term) -> Option<bool> {
        tracing::debug!(
            "Checking Subtyping {:?}   <:   {:?}",
            sub.debug_short(),
            sup.debug_short()
        );
        //crate::pause();

        self.wrap_check(CheckingTask::Subtype(sub, sup), |slf| {
            slf.check_subtype_i(sub, sup)
        })
    }

    pub fn check_inhabitable(&mut self, t: &'t Term) -> Option<bool> {
        tracing::debug!("Checking Inhabitability of {:?}", t.debug_short());
        if t.has_solvable() {
            let nt = self.subst(t.clone());
            self.scoped(|slf| {
                slf.wrap_check(CheckingTask::Inhabitable(&nt), |slf| {
                    slf.check_inhabitable_i(&nt)
                })
            })
        } else {
            self.wrap_check(CheckingTask::Inhabitable(t), |slf| {
                slf.check_inhabitable_i(t)
            })
        }
    }

    pub fn check_universe(&mut self, t: &'t Term) -> Option<bool> {
        tracing::debug!("Checking Universe {:?}", t.debug_short());
        //self.wrap_check(CheckingTask::Universe(t), |slf| slf.check_universe_i(t))
        if t.has_solvable() {
            let nt = self.subst(t.clone());
            self.scoped(|slf| {
                slf.wrap_check(CheckingTask::Universe(&nt), |slf| slf.check_universe_i(&nt))
            })
        } else {
            self.wrap_check(CheckingTask::Universe(t), |slf| slf.check_universe_i(t))
        }
    }

    pub(crate) fn check_type_i(&mut self, tm: &'t Term, tp: &'t Term) -> Option<bool> {
        //self.cancellable(|slf| {
        Split::strategies(
            self,
            "Using checking rules",
            |slf| {
                if let either::Left(r) = slf.simplify_rules_two(
                    self.top.rules.checking(),
                    tm,
                    tp,
                    |slf, rl, tm, tp| rl.applicable(slf, tm, tp),
                    |slf, rl, tm, tp| rl.apply(slf, tm, tp),
                    |_, _| false,
                ) {
                    r
                } else {
                    None
                }
                /*
                let rules = slf
                    .top
                    .rules
                    .checking()
                    .iter()
                    .filter_map(|rl| {
                        if rl.applicable(slf, tm, tp) {
                            Some(&**rl)
                        } else {
                            None
                        }
                    })
                    .collect::<smallvec::SmallVec<_, 2>>();
                Split::split(slf, true, rules, |slf, rl| rl.apply(slf, tm, tp))
                */
            },
            "Using type inference",
            |slf| {
                let subtp = slf.infer_type(tm)?;
                slf.scoped(|slf| slf.check_subtype(&subtp, tp))
            },
        )
        //})
    }

    pub(crate) fn check_subtype_i(&mut self, sub: &'t Term, sup: &'t Term) -> Option<bool> {
        if sub.alpha_equal(sup) {
            self.comment("trivial");
            tracing::debug!("trivial");
            return Some(true);
        }
        if let Some(unk) = sub.is_solvable() {
            return self.solve_upper_bound(unk, sup);
        }
        if let Some(unk) = sup.is_solvable() {
            return self.solve_lower_bound(unk, sub);
        }
        match self.simplify_rules_two(
            self.top.rules.subtyping(),
            sub,
            sup,
            |slf, rl, sub, sup| rl.applicable(slf, sub, sup),
            |slf, rl, sub, sup| rl.apply(slf, sub, sup),
            |sub, sup| {
                sub.alpha_equal(sup) || sub.is_solvable().is_some() || sup.is_solvable().is_some()
            },
        ) {
            either::Left(opt) => {
                if opt.is_some() {
                    return opt;
                }
                tracing::debug!("Proving subtyping failed; Falling back to checking equality");
                match self.traced(
                    CheckingTask::Strategy(
                        "Proving subtyping failed; Falling back to checking equality",
                    ),
                    |slf| slf.check_equality_i(sub, sup),
                ) {
                    Ok(r) => Some(r),
                    Err(ls) => {
                        self.add_msg(ls.into());
                        None
                    }
                }
            }
            either::Right((sub, sup)) => {
                if sub.alpha_equal(&sup) {
                    self.comment("trivial");
                    tracing::debug!("trivial");
                    return Some(true);
                }
                self.scoped(|slf| {
                    if let Some(unk) = sub.is_solvable() {
                        tracing::debug!("solving");
                        return slf.solve_upper_bound(unk, &sup);
                    }
                    if let Some(unk) = sup.is_solvable() {
                        tracing::debug!("solving");
                        return slf.solve_lower_bound(unk, &sub);
                    }
                    tracing::debug!("Proving subtyping failed; Falling back to checking equality");
                    match slf.traced(
                        CheckingTask::Strategy(
                            "Proving subtyping failed; Falling back to checking equality",
                        ),
                        |slf| slf.check_equality_i(&sub, &sup),
                    ) {
                        Ok(r) => Some(r),
                        Err(ls) => {
                            slf.add_msg(ls.into());
                            None
                        }
                    }
                })
            }
        }

        /*
        let sub = self
            .simplify_full(false, sub)
            .map_or(Cow::Borrowed(sub), Cow::Owned);
        let sup = self
            .simplify_full(false, sup)
            .map_or(Cow::Borrowed(sup), Cow::Owned);
        if self.alpha_equal(&sub, &sup) {
            self.comment("trivial");
            return Some(true);
        }
        self.scoped(|slf| {
            if let Some(unk) = sub.is_solvable() {
                tracing::debug!("solving");
                return slf.solve_upper_bound(unk, &sup);
            }
            if let Some(unk) = sup.is_solvable() {
                tracing::debug!("solving");
                return slf.solve_lower_bound(unk, &sub);
            }
            let rules = slf
                .top
                .rules
                .subtyping()
                .iter()
                .filter_map(|rl| {
                    if rl.applicable(slf, &sub, &sup) {
                        Some(&**rl)
                    } else {
                        None
                    }
                })
                .collect::<smallvec::SmallVec<_, 2>>();
            let lines = match Split::split_i(slf, true, rules, |slf, rl| rl.apply(slf, &sub, &sup))
            {
                Ok(r) => return Some(r),
                Err(ls) => ls,
            };
            match slf.traced(
                CheckingTask::Strategy(
                    "Proving subtyping failed; Falling back to checking equality",
                ),
                |slf| slf.check_equality_i(&sub, &sup),
            ) {
                Ok(b) => Some(b),
                Err(l) => {
                    for l in lines {
                        slf.add_msg(l.into());
                    }
                    slf.add_msg(l.into());
                    None
                }
            }
        })
         */
    }

    pub(crate) fn check_inhabitable_i(&mut self, tm: &'t Term) -> Option<bool> {
        Split::strategies(
            self,
            "Using type inference",
            |slf| {
                let tp = slf.infer_type(tm)?;
                slf.scoped(|slf| slf.check_universe(&tp))
            },
            "Using inhabitable rules",
            |slf| {
                slf.simplify_rules(
                    slf.top.rules.inhabitable(),
                    tm,
                    InhabitableRule::applicable,
                    |slf, rl, t| rl.apply(slf, t),
                )
            },
        )
    }

    fn check_universe_i(&mut self, tm: &'t Term) -> Option<bool> {
        //self.comment(format!("{:?}", self.top.rules.universe()));
        self.simplify_rules(
            self.top.rules.universe(),
            tm,
            UniverseRule::applicable,
            |slf, rl, t| rl.apply(slf, t),
        )

        /*let rules = self
            .top
            .rules
            .universe()
            .iter()
            .filter_map(|rl| if rl.applicable(t) { Some(&**rl) } else { None });
        Split::split(self, true, rules, |slf, rl| rl.apply(slf, t))*/
    }
}
