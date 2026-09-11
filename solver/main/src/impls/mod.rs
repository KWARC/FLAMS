#![allow(clippy::absurd_extreme_comparisons)]

pub mod backend;
pub mod equality;
mod inference;
pub mod preparation;
pub mod proving;
pub mod records;
pub mod simplify;
pub mod solving;
mod typing;

use crate::{
    CheckRef, Checker,
    context::{ContextBase, CowLike},
    impls::solving::Solutions,
    judgment_cache::{JudgmentCache, JudgmentCacheBase, Judgments},
    split::{CancelToken, SplitStrategy},
    trace::{CheckLogCow, CheckingTask, PreCheckLog, RefCheckLog},
    utils::{Merge, MutableRefList},
};
use ftml_ontology::terms::ComponentVar;
use ftml_solver_trace::traceref;
use proving::ProverState;
use smallvec::SmallVec;
use std::borrow::Cow;

const DEPTH_LIMIT: usize = 64;

impl<Split: SplitStrategy> Checker<Split> {
    pub(crate) fn wrap_task<'t, R: std::fmt::Debug + Clone + 'static, F>(
        &'t self,
        task: CheckingTask<'t>,
        unknowns: Option<Solutions>,
        then: F,
    ) -> (Option<R>, Solutions, PreCheckLog)
    where
        F: FnOnce(CheckRef<'t, '_, Split>) -> Option<R>,
    {
        let mut context = ContextBase::new();
        let mut solutions = unknowns.unwrap_or_default();
        let mut messages = SmallVec::new();
        let cancel = CancelToken::default();
        let proof_state = ProverState::default();
        let mut judgment_cache = JudgmentCacheBase::default();
        let rf = CheckRef {
            top: self,
            messages: &mut messages,
            cancel: &cancel,
            context: context.get_ref(),
            judgment_cache: judgment_cache.make_ref(),
            proof_state: &proof_state,
            added: 0,
            solutions: MutableRefList::new(&mut solutions),
            traced: true,
        };
        let r = then(rf);
        let rl = MutableRefList::new(&mut solutions);
        let line = task
            .close(r.as_ref(), messages.into_boxed_slice(), context.as_ref())
            .into_owned(&|t| rl.subst(t));
        tracing::trace!("Solutions:{solutions:#?}");
        (r, solutions, line)
    }

    pub(crate) fn wrap_none<'t, R: std::fmt::Debug + Clone + 'static, F>(
        &'t self,
        unknowns: Option<Solutions>,
        then: F,
    ) -> (Solutions, R)
    where
        F: FnOnce(CheckRef<'t, '_, Split>) -> R,
    {
        let mut context = ContextBase::new();
        let mut solutions = unknowns.unwrap_or_default();
        let mut messages = SmallVec::new();
        let cancel = CancelToken::default();
        let proof_state = ProverState::default();
        let mut judgment_cache = JudgmentCacheBase::default();
        let rf = CheckRef {
            top: self,
            messages: &mut messages,
            cancel: &cancel,
            judgment_cache: judgment_cache.make_ref(),
            context: context.get_ref(),
            proof_state: &proof_state,
            added: 0,
            solutions: MutableRefList::new(&mut solutions),
            traced: true,
        };
        let r = then(rf);
        (solutions, r)
    }
}

impl<'c, 'i, Split: SplitStrategy> CheckRef<'c, 'i, Split> {
    pub fn extend_context<C: CowLike<'c>>(&mut self, var: C) {
        self.added += 1;
        self.context.push(self.top, var.into_cow());
    }

    pub fn add_msg(&mut self, line: CheckLogCow<'c>) {
        self.messages.push(line);
    }
    pub fn comment(&mut self, msg: impl Into<Cow<'static, str>>) {
        self.messages.push(CheckLogCow::Owned(PreCheckLog::Msg(
            vec![msg.into().into_owned().into()],
            crate::trace::MessageLevel::Comment,
        )));
    }
    pub fn counter(&mut self, msg: &'static str, num: usize) {
        self.messages.push(CheckLogCow::Owned(PreCheckLog::Msg(
            vec![msg.into(), num.into()],
            crate::trace::MessageLevel::Comment,
        )));
    }
    pub fn failure(&mut self, msg: impl Into<Cow<'static, str>>) {
        self.messages.push(CheckLogCow::Owned(PreCheckLog::Msg(
            vec![msg.into().into_owned().into()],
            crate::trace::MessageLevel::Failure,
        )));
    }
    #[inline]
    pub(crate) fn split(&mut self) -> (&[Cow<'c, ComponentVar>], Trace<'c, '_>) {
        (self.context.as_ref(), Trace(self.messages))
    }

    #[must_use]
    pub fn iter_context(&self) -> impl ExactSizeIterator<Item = &ComponentVar> {
        self.context.as_ref().iter().rev().map(|c| &**c)
    }

    pub(crate) fn traced<R: Clone>(
        &mut self,
        tsk: CheckingTask<'c>,
        f: impl FnOnce(&mut Self) -> Option<R>,
    ) -> Result<R, RefCheckLog<'c>> {
        if self.depth() >= DEPTH_LIMIT {
            //self.failure("Depth Limit Reached!");
            return Err(traceref!(FAIL "Depth Limit Reached!"));
        }
        let (r, l) = self.traced_inner(tsk, f);
        if let Some(r) = r {
            self.messages.push(CheckLogCow::Borrowed(l));
            Ok(r)
        } else {
            Err(l)
        }
    }

    pub(crate) fn untraced<R: Clone>(
        &mut self,
        task: CheckingTask<'c>,
        f: impl FnOnce(&mut Self) -> Option<R>,
    ) -> Option<R> {
        if self.depth() >= DEPTH_LIMIT {
            self.failure("Depth Limit Reached!");
            return None;
        }
        let old_msg = std::mem::replace(self.messages, SmallVec::new());
        let ret = f(self);
        *self.messages = old_msg;
        let ctx = self.context.as_ref();
        let ctx = &ctx[ctx.len() - self.added as usize..ctx.len()];
        let line = task.close(ret.as_ref(), Box::new([]), ctx);
        ret
    }

    pub(crate) fn traced_inner<R: Clone>(
        &mut self,
        task: CheckingTask<'c>,
        f: impl FnOnce(&mut Self) -> Option<R>,
    ) -> (Option<R>, RefCheckLog<'c>) {
        if self.depth() >= DEPTH_LIMIT {
            //self.failure("Depth Limit Reached!");
            return (None, traceref!(FAIL "Depth Limit Reached!"));
        }

        if self.judgment_cache.running(&task) {
            return (None, traceref!(FAIL "Recursion in Task check"));
        }
        if let Some(r) = self.judgment_cache.has::<R>(&task, &self.context) {
            let l = task.close(
                r.as_ref(),
                Box::new([if r.is_some() {
                    traceref!("previously proven")
                } else {
                    traceref!(FAIL "previously failed already")
                }
                .into()]),
                &[],
            );
            return (r, l);
        }
        self.judgment_cache.push(task);
        let old_msg = std::mem::replace(self.messages, SmallVec::new());
        let ret = f(self);
        let msgs = std::mem::replace(self.messages, old_msg);
        let ctx = self.context.as_ref();
        let ctx = &ctx[ctx.len() - self.added as usize..ctx.len()];
        let task = self
            .judgment_cache
            .pop(&ret, self.context.as_ref(), &self.solutions)
            .expect("wut");
        let line = if crate::TRUNCATE_PROOFS && ret.is_some() {
            task.close(ret.as_ref(), Box::new([]), ctx)
        } else {
            task.close(ret.as_ref(), msgs.into_boxed_slice(), ctx)
        };
        (ret, line)
    }

    #[inline]
    pub const fn depth(&self) -> usize {
        self.solutions.depth()
    }

    pub(crate) fn branch_traced<R: Clone>(
        &mut self,
        task: CheckingTask<'c>,
        f: impl FnOnce(CheckRef<'c, '_, Split>) -> Option<R>,
    ) -> Result<R, RefCheckLog<'c>> {
        if self.depth() >= DEPTH_LIMIT {
            //self.failure("Depth Limit Reached!");
            return Err(traceref!(FAIL "Depth Limit Reached!"));
        }
        if self.judgment_cache.running(&task) {
            return Err(traceref!(FAIL "Recursion in Task check"));
        }
        if let Some(r) = self.judgment_cache.has::<R>(&task, &self.context) {
            return match r {
                Some(r) => {
                    self.messages.push(
                        task.close(
                            Some(&r),
                            Box::new([traceref!("previously proven").into()]),
                            &[],
                        )
                        .into(),
                    );
                    Ok(r)
                }
                None => Err(traceref!(FAIL "previously failed already")),
            };
        }

        self.judgment_cache.push(task);

        let mut messages = SmallVec::<CheckLogCow<'c>, _>::new();
        let mut solutions = Solutions::default();
        let inner = CheckRef {
            messages: &mut messages,
            context: self.context.duplicate(),
            proof_state: self.proof_state,
            judgment_cache: self.judgment_cache.copied(),
            solutions: MutableRefList::new_with_parent(&mut solutions, &self.solutions),
            top: self.top,
            cancel: self.cancel,
            added: 0,
            traced: self.traced,
        };
        let ret = f(inner);
        let ctx = self.context.as_ref();
        let ctx = &ctx[ctx.len() - self.added as usize..ctx.len()];

        let task = self
            .judgment_cache
            .pop(&ret, self.context.as_ref(), &solutions)
            .expect("wut");

        let line = if crate::TRUNCATE_PROOFS && ret.is_some() {
            task.close(ret.as_ref(), Box::new([]), ctx)
        } else {
            task.close(ret.as_ref(), messages.into_boxed_slice(), ctx)
        };
        if let Some(r) = ret {
            self.merge_solutions(solutions);
            self.messages.push(line.into());
            Ok(r)
        } else {
            Err(line)
        }
    }

    pub fn scoped<'nt, R: Send + Sync + 'static>(
        &'nt mut self,
        f: impl FnOnce(&mut CheckRef<'nt, '_, Split>) -> R,
    ) -> R {
        let old_added = std::mem::take(&mut self.added);
        let old_msgs = std::mem::take(self.messages);

        // SAFETY:
        // - all variables added in `f` with lifetime 'nt are popped again before we return
        // - all messages added in `f` with lifetime 'nt are turned into owned ones
        //   before readding
        let muted = unsafe {
            std::mem::transmute::<&mut CheckRef<'c, '_, Split>, &mut CheckRef<'nt, '_, Split>>(self)
        };
        let r = f(muted);
        self.context
            .pop(std::mem::replace(&mut self.added, old_added) as usize);
        for m in std::mem::replace(self.messages, old_msgs) {
            self.messages.push(m.into_owned(&|t| self.subst(t)).into());
        }
        r
    }

    pub(crate) fn cancellable<R: Send>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let old = self.cancel;
        let cancel = old.derive();
        let rf = &cancel;
        let rf: &'c CancelToken<'c, Split::CancelToken> = unsafe { std::mem::transmute(rf) };
        self.cancel = rf;
        let r = f(self);
        self.cancel = old;
        drop(cancel);
        r
    }

    pub(crate) fn wrap_check<R: Clone>(
        &mut self,
        task: CheckingTask<'c>,
        f: impl FnOnce(&mut Self) -> Option<R>,
    ) -> Option<R> {
        if self.depth() >= DEPTH_LIMIT {
            self.failure("Depth Limit Reached!");
            return None;
        }
        if self.cancel.is_cancelled() {
            self.failure("CANCELLED!");
            return None;
        }
        match self.traced(task, f) {
            Ok(r) => Some(r),
            Err(l) => {
                self.add_msg(l.into());
                None
            }
        }
    }

    pub(crate) fn copied(&self) -> CheckRefBranch<'c, 'i, Split> {
        CheckRefBranch {
            context: self.context.clone_base(),
            proof_state: self.proof_state,
            messages: SmallVec::new(),
            // --------------
            top: self.top,
            cache_base: self.judgment_cache.new_base(),
            solutions: Solutions::default(),
            // SAFETY: will not live longer than 'i; only immutable borrows, 'i is only stack reference
            // to parent
            parent_solutions: unsafe { std::mem::transmute(&self.solutions) },
            cancel: self.cancel,
            traced: self.traced,
        }
    }
}

pub struct CheckRefBranch<'c, 'i, Split: SplitStrategy> {
    top: &'c Checker<Split>,
    context: ContextBase<'c>,
    proof_state: &'i ProverState,
    cache_base: JudgmentCacheBase<'c>,
    solutions: Solutions,
    parent_solutions: &'i MutableRefList<'i, Solutions>,
    cancel: &'i CancelToken<'i, Split::CancelToken>,
    messages: SmallVec<CheckLogCow<'c>, 2>,
    traced: bool,
}
impl<'c, Split: SplitStrategy> CheckRefBranch<'c, '_, Split> {
    pub fn get_ref(&mut self) -> CheckRef<'c, '_, Split> {
        CheckRef {
            top: self.top,
            cancel: self.cancel,
            messages: &mut self.messages,
            proof_state: self.proof_state,
            judgment_cache: self.cache_base.make_ref(),
            context: self.context.get_ref(),
            added: 0,
            solutions: MutableRefList::new_with_parent(&mut self.solutions, self.parent_solutions),
            traced: self.traced,
        }
    }
    pub fn close(self, checker: &mut CheckRef<'c, '_, Split>) {
        checker.merge_solutions(self.solutions);
        checker.messages.extend(self.messages);
    }
}

impl<Split: SplitStrategy> Drop for CheckRef<'_, '_, Split> {
    fn drop(&mut self) {
        self.context.pop(self.added as usize);
    }
}

pub struct Trace<'c, 'i>(&'i mut SmallVec<CheckLogCow<'c>, 2>);
impl<'c> Trace<'c, '_> {
    pub fn add_msg(&mut self, line: CheckLogCow<'c>) {
        self.0.push(line);
    }
    pub fn comment(&mut self, msg: impl Into<Cow<'static, str>>) {
        self.0.push(CheckLogCow::Owned(PreCheckLog::Msg(
            vec![msg.into().into_owned().into()],
            crate::trace::MessageLevel::Comment,
        )));
    }
    pub fn failure(&mut self, msg: impl Into<Cow<'static, str>>) {
        self.0.push(CheckLogCow::Owned(PreCheckLog::Msg(
            vec![msg.into().into_owned().into()],
            crate::trace::MessageLevel::Failure,
        )));
    }
}
