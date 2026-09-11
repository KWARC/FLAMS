use std::borrow::Cow;

use ftml_ontology::terms::{ComponentVar, Term};
use ftml_solver_trace::CheckingTask;

use crate::{
    context::ContextWrap,
    impls::solving::{Solutions, is_solvable_var},
};

pub type Judgments<'c> = rustc_hash::FxHashSet<(Vec<ComponentVar>, CachedJudgment)>;
pub type Simplifications = rustc_hash::FxHashMap<Term, Vec<(Vec<ComponentVar>, Term)>>;

#[derive(Default, Debug)]
pub struct JudgmentCacheBase<'c> {
    current: Vec<CheckingTask<'c>>,
    cached: std::sync::Arc<parking_lot::Mutex<Judgments<'c>>>,
    simplifications: std::sync::Arc<parking_lot::Mutex<Simplifications>>,
}
impl<'c> JudgmentCacheBase<'c> {
    pub fn make_ref(&mut self) -> JudgmentCache<'c, '_> {
        JudgmentCache {
            current: &mut self.current,
            cached: self.cached.clone(),
            simplifications: self.simplifications.clone(),
        }
    }
}

#[derive(Debug)]
pub struct JudgmentCache<'c, 'i> {
    current: &'i mut Vec<CheckingTask<'c>>,
    pub(crate) cached: std::sync::Arc<parking_lot::Mutex<Judgments<'c>>>,
    simplifications: std::sync::Arc<parking_lot::Mutex<Simplifications>>,
}
impl<'c> JudgmentCache<'c, '_> {
    pub fn copied(&mut self) -> JudgmentCache<'c, '_> {
        JudgmentCache {
            current: &mut self.current,
            cached: self.cached.clone(),
            simplifications: self.simplifications.clone(),
        }
    }
    pub fn new_base(&self) -> JudgmentCacheBase<'c> {
        JudgmentCacheBase {
            current: self.current.clone(),
            cached: self.cached.clone(),
            simplifications: self.simplifications.clone(),
        }
    }
    pub fn add_simplification(&self, from: &Term, to: &Term, ctx: &[Cow<ComponentVar>]) {
        return ();
        let mut vars = from.free_variables();

        let mut nctx = Vec::new();
        while let Some(v) = vars.pop() {
            if is_solvable_var(v).is_some() {
                return;
            }
            if let Some(v) = ctx.iter().rev().find(|vd| vd.var.name() == v.name()) {
                nctx.push(v.clone().into_owned());
                //vars.push(&v.var);
                if let Some(tp) = v.tp.as_ref() {
                    for v in tp.free_variables() {
                        if !vars.contains(&v) && !nctx.iter().any(|nv| nv.var.name() == v.name()) {
                            vars.push(v);
                        }
                    }
                }
                if let Some(df) = v.df.as_ref() {
                    for v in df.free_variables() {
                        if !vars.contains(&v) && !nctx.iter().any(|nv| nv.var.name() == v.name()) {
                            vars.push(v);
                        }
                    }
                }
            }
        }
        nctx.reverse();
        let mut simps = self.simplifications.lock();
        match simps.entry(from.clone()) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert_entry(vec![(nctx, to.clone())]);
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                let o = o.get_mut();
                o.push((nctx, to.clone()));
            }
        }
    }
    pub fn get_simplification(&self, from: &Term, ctx: &[Cow<ComponentVar>]) -> Option<Term> {
        return None;
        let simps = self.simplifications.lock();
        simps.get(from).and_then(|v| {
            v.iter().rev().find_map(|(c, t)| {
                if c.iter().all(|v| {
                    ctx.iter()
                        .rev()
                        .find(|w| w.var.name() == v.var.name())
                        .is_some_and(|w| w.tp == v.tp && w.df == v.df)
                }) {
                    Some(t.clone())
                } else {
                    None
                }
            })
        })
    }

    pub fn push(&mut self, task: CheckingTask<'c>) {
        self.current.push(task);
    }
    pub fn pop<R: Clone>(
        &mut self,
        ret: &Option<R>,
        ctx: &[Cow<ComponentVar>],
        solutions: &Solutions,
    ) -> Option<CheckingTask<'c>> {
        fn filter_context(
            ctx: &[Cow<ComponentVar>],
            task: &CheckingTask,
        ) -> Option<Vec<ComponentVar>> {
            let mut vars: Vec<_> = match task {
                CheckingTask::Equality(a, b)
                | CheckingTask::HasType(a, b)
                | CheckingTask::Subtype(a, b) => a
                    .free_variables()
                    .into_iter()
                    .chain(b.free_variables())
                    .collect(),
                CheckingTask::Inference(t)
                | CheckingTask::Inhabitable(t)
                | CheckingTask::Proving(t)
                | CheckingTask::Universe(t) => t.free_variables().into_iter().collect(),
                CheckingTask::Rule(_)
                | CheckingTask::Simplify(_)
                | CheckingTask::Strategy(_)
                | CheckingTask::VariableInference(_) => {
                    return None;
                }
            };
            let mut ret = Vec::new();
            while let Some(v) = vars.pop() {
                if is_solvable_var(v).is_some() {
                    return None;
                }
                if let Some(v) = ctx.iter().find(|vd| vd.var.name() == v.name()) {
                    ret.push(v.clone().into_owned());
                    //vars.push(&v.var);
                    if let Some(tp) = v.tp.as_ref() {
                        for v in tp.free_variables() {
                            if is_solvable_var(v).is_some() {
                                return None;
                            }
                            if !vars.contains(&v) && !ret.iter().any(|nv| nv.var.name() == v.name())
                            {
                                vars.push(v);
                            }
                        }
                    }
                    if let Some(df) = v.df.as_ref() {
                        for v in df.free_variables() {
                            if is_solvable_var(v).is_some() {
                                return None;
                            }
                            if !vars.contains(&v) && !ret.iter().any(|nv| nv.var.name() == v.name())
                            {
                                vars.push(v);
                            }
                        }
                    }
                }
            }
            ret.reverse();
            Some(ret)
        }
        fn has_unsolveds(ctx: &[Cow<ComponentVar>], task: &CheckingTask) -> bool {
            let vars = match task {
                CheckingTask::Equality(a, b)
                | CheckingTask::HasType(a, b)
                | CheckingTask::Subtype(a, b) => {
                    either::Left(a.free_variables().into_iter().chain(b.free_variables()))
                }
                CheckingTask::Inference(t)
                | CheckingTask::Inhabitable(t)
                | CheckingTask::Proving(t)
                | CheckingTask::Universe(t) => either::Right(t.free_variables().into_iter()),
                CheckingTask::Rule(_)
                | CheckingTask::Simplify(_)
                | CheckingTask::Strategy(_)
                | CheckingTask::VariableInference(_) => {
                    return true;
                }
            };
            for v in vars {
                if is_solvable_var(v).is_some() {
                    return true;
                }
                if let Some(v) = ctx.iter().find(|vd| vd.var.name() == v.name())
                    && (v
                        .tp
                        .as_ref()
                        .is_some_and(|t| t.has_free_such_that(|v| is_solvable_var(v).is_some()))
                        || v.df.as_ref().is_some_and(|t| {
                            t.has_free_such_that(|v| is_solvable_var(v).is_some())
                        }))
                {
                    return true;
                }
            }
            false
        }
        let task = self.current.pop()?;
        //return Some(task);
        if !solutions.0.is_empty()
        //|| ret.is_none()
        //|| has_unsolveds(ctx, &task)
        /*|| !filter_context(ctx, &task)
        || {
            std::mem::size_of::<R>() == std::mem::size_of::<Term>() && {
                let ot: &Option<Term> = unsafe {
                    std::ptr::from_ref(ret)
                        .cast::<Option<Term>>()
                        .as_ref_unchecked()
                };
                ot.as_ref()
                    .is_some_and(|t| t.has_free_such_that(|v| is_solvable_var(v).is_some()))
            }
        } */
        {
            return Some(task);
        }

        let Some(ctx) = filter_context(ctx, &task) else {
            return Some(task);
        };

        macro_rules! insert {
            ($e:expr) => {{
                let mut store = self.cached.lock();
                //let ctx = ctx.iter().map(|vd| vd.clone().into_owned()).collect();
                store.insert((ctx, $e));
            }};
        }
        match task {
            CheckingTask::Rule(_)
            | CheckingTask::Strategy(_)
            | CheckingTask::Simplify(_)
            | CheckingTask::VariableInference(_) => (),
            CheckingTask::Inference(t) => {
                //println!("Here: {:?}\n in context {:?}", t.debug_short(), ctx);
                //println!("");
                insert!(CachedJudgment::Inference {
                    term: t.clone(),
                    result: mutate(ret),
                });
            }
            CheckingTask::Proving(t) => {
                insert!(CachedJudgment::Proof {
                    goal: t.clone(),
                    result: mutate(ret),
                });
            }
            CheckingTask::Inhabitable(t) => {
                insert!(CachedJudgment::Inhabitable {
                    term: t.clone(),
                    result: mutate(ret),
                });
            }
            CheckingTask::Universe(t) => {
                insert!(CachedJudgment::Universe {
                    term: t.clone(),
                    result: mutate(ret),
                });
            }
            CheckingTask::Equality(lhs, rhs) => {
                insert!(CachedJudgment::Equal {
                    lhs: lhs.clone(),
                    rhs: rhs.clone(),
                    result: mutate(ret),
                });
            }
            CheckingTask::Subtype(sub, sup) => {
                insert!(CachedJudgment::Subtype {
                    sub: sub.clone(),
                    sup: sup.clone(),
                    result: mutate(ret),
                });
            }
            CheckingTask::HasType(tm, tp) => {
                insert!(CachedJudgment::HasType {
                    tm: tm.clone(),
                    tp: tp.clone(),
                    result: mutate(ret),
                });
            }
        }
        //

        Some(task)
    }

    pub fn running(&self, task: &CheckingTask<'c>) -> bool {
        self.current.contains(task)
    }
    pub fn has<R: Clone>(&self, task: &CheckingTask, context: &ContextWrap) -> Option<Option<R>> {
        //return None;
        let curr = &*self.cached.lock();
        for c in curr {
            if let Some(r) = c.1.is(task, context, &c.0) {
                /*println!(
                    "Here:\n - Task: {task:?}\n - Context: {context:?}\n - Cached context: {:?}\n - Cached Judgment: {:?}",
                    c.0, c.1
                );*/
                return Some(r);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CachedJudgment {
    Proof {
        // ctx:SmallVec<Cow<'c, ComponentVar>, { CONTEXT_LEN }>,
        goal: Term,
        result: Option<Term>,
    },
    Inference {
        term: Term,
        result: Option<Term>,
    },
    Inhabitable {
        term: Term,
        result: Option<bool>,
    },
    Universe {
        term: Term,
        result: Option<bool>,
    },
    Subtype {
        sub: Term,
        sup: Term,
        result: Option<bool>,
    },
    HasType {
        tm: Term,
        tp: Term,
        result: Option<bool>,
    },
    Equal {
        lhs: Term,
        rhs: Term,
        result: Option<bool>,
    },
}
impl CachedJudgment {
    pub fn is<R: Clone>(
        &self,
        task: &CheckingTask,
        ctx: &ContextWrap,
        cached_ctx: &[ComponentVar],
    ) -> Option<Option<R>> {
        fn contexts(ctx: &ContextWrap, cached_ctx: &[ComponentVar]) -> bool {
            cached_ctx.iter().all(|v| {
                ctx.as_ref()
                    .iter()
                    .rev()
                    .find(|w| w.var.name() == v.var.name())
                    .is_some_and(|w| w.tp == v.tp && w.df == v.df)
            })
        }
        match (task, self) {
            (CheckingTask::Proving(t), Self::Proof { goal, result })
                if goal.alpha_equal(t) && contexts(ctx, cached_ctx) =>
            {
                Some(mutate(result))
            }
            (CheckingTask::Inference(t), Self::Inference { term, result })
                if t.alpha_equal(term) && contexts(ctx, cached_ctx) =>
            {
                Some(mutate(result))
            }
            (CheckingTask::Inhabitable(t), Self::Inhabitable { term, result })
                if t.alpha_equal(term) && contexts(ctx, cached_ctx) =>
            {
                Some(mutate(result))
            }
            (CheckingTask::Universe(t), Self::Universe { term, result })
                if t.alpha_equal(term) && contexts(ctx, cached_ctx) =>
            {
                Some(mutate(result))
            }
            (CheckingTask::Subtype(asub, asup), Self::Subtype { sub, sup, result })
                if asub.alpha_equal(sub) && asup.alpha_equal(sup) && contexts(ctx, cached_ctx) =>
            {
                Some(mutate(result))
            }
            (CheckingTask::HasType(a, b), Self::HasType { tm, tp, result })
                if a.alpha_equal(tm) && b.alpha_equal(tp) && contexts(ctx, cached_ctx) =>
            {
                Some(mutate(result))
            }
            (CheckingTask::Equality(l, r), Self::Equal { lhs, rhs, result })
                if l.alpha_equal(lhs) && r.alpha_equal(rhs) && contexts(ctx, cached_ctx) =>
            {
                Some(mutate(result))
            }
            _ => None,
        }
    }
}

#[allow(clippy::ref_option)]
fn mutate<R: Clone, T>(res: &Option<T>) -> Option<R> {
    assert_eq!(std::mem::size_of::<R>(), std::mem::size_of::<T>());
    unsafe {
        std::ptr::from_ref(res)
            .cast::<Option<R>>()
            .as_ref()
            .expect("wut")
            .clone()
    }
}
