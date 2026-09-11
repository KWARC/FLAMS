use std::borrow::Cow;

use ftml_ontology::terms::ComponentVar;
use smallvec::SmallVec;

use crate::{
    Checker,
    facts::{Fact, GlobalOrLocal, LocalFacts},
    hoas::HOASSymbols,
    split::SplitStrategy,
};

pub(crate) const CONTEXT_LEN: usize = 4;

pub trait CowLike<'a> {
    fn into_cow(self) -> Cow<'a, ComponentVar>;
}
impl<'a> CowLike<'a> for ComponentVar {
    #[inline]
    fn into_cow(self) -> Cow<'a, ComponentVar> {
        Cow::Owned(self)
    }
}
impl<'a> CowLike<'a> for &'a ComponentVar {
    #[inline]
    fn into_cow(self) -> Cow<'a, ComponentVar> {
        Cow::Borrowed(self)
    }
}

#[derive(Clone)]
pub(crate) struct ContextBase<'c> {
    //hoas: Option<HOASSymbols>,
    ctx: SmallVec<Cow<'c, ComponentVar>, { CONTEXT_LEN }>,
    blocked: Vec<GlobalOrLocal>,
    facts: LocalFacts,
    goal_counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
impl<'c> ContextBase<'c> {
    pub fn new() -> Self {
        Self {
            ctx: SmallVec::new(),
            blocked: Vec::new(),
            facts: LocalFacts::default(),
            goal_counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)), //hoas: HOASSymbols::get(top),
        }
    }
    #[inline]
    pub const fn get_ref(&mut self) -> ContextWrap<'c, '_> {
        ContextWrap(self)
    }
}
impl<'c> AsRef<[Cow<'c, ComponentVar>]> for ContextBase<'c> {
    #[inline]
    fn as_ref(&self) -> &[Cow<'c, ComponentVar>] {
        &self.ctx
    }
}

pub struct ContextWrap<'c, 's>(&'s mut ContextBase<'c>);
impl<'c> ContextWrap<'c, '_> {
    pub(crate) fn clone_base(&self) -> ContextBase<'c> {
        self.0.clone()
    }
    pub(crate) fn pop(&mut self, len: usize) {
        for _ in 0..len {
            self.0.ctx.pop();
        }
        let newlen = self.0.ctx.len();
        while let Some((i, _)) = self.0.facts.facts.last()
            && *i >= newlen
        {
            self.0.facts.facts.pop();
        }
    }

    pub(crate) fn block_fact(&mut self, fact: GlobalOrLocal) {
        self.0.blocked.push(fact);
    }
    pub(crate) fn blocked(&self) -> &[GlobalOrLocal] {
        &self.0.blocked
    }
    pub(crate) const fn facts(&self) -> &LocalFacts {
        &self.0.facts
    }
    pub(crate) fn goal_counter(&self) -> &std::sync::atomic::AtomicUsize {
        &self.0.goal_counter
    }
    pub(crate) fn push<Split: SplitStrategy>(
        &mut self,
        top: &Checker<Split>,
        var: Cow<'c, ComponentVar>,
    ) {
        /*
        if let Some(tp) = var.tp.as_ref()
            && let Some(hoas) = top.hoas()
            && let Some(fact) = Fact::from_tp(hoas, tp, top)
        {
            self.0.facts.facts.push((self.0.ctx.len(), fact));
        }
        */
        tracing::trace!("Adding");
        self.0.ctx.push(var);
    }
    pub(crate) fn take(&mut self) -> ContextBase<'c> {
        std::mem::replace(self.0, ContextBase::new())
    }
    pub(crate) fn set(&mut self, base: ContextBase<'c>) {
        *self.0 = base;
    }
    pub(crate) const fn duplicate(&mut self) -> ContextWrap<'c, '_> {
        ContextWrap(self.0)
    }
}
impl<'c> AsRef<[Cow<'c, ComponentVar>]> for ContextWrap<'c, '_> {
    #[inline]
    fn as_ref(&self) -> &[Cow<'c, ComponentVar>] {
        &self.0.ctx
    }
}
impl std::fmt::Debug for ContextWrap<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ls = f.debug_list();
        for cv in &self.0.ctx {
            ls.entry(&**cv);
        }
        ls.finish()
    }
}
