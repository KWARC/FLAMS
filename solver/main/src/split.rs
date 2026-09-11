use std::hint::unreachable_unchecked;

use ftml_solver_trace::traceref;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smallvec::SmallVec;

use crate::{
    CheckRef,
    rules::{
        CheckerRule,
        extractors::{RuleExtractor, SymbolRuleExtractor},
    },
    trace::{CheckingTask, RefCheckLog},
};

pub trait Cancellation: Default + Send + Sync {
    fn is_cancelled(&self) -> bool;
    fn cancel(&self);
}
#[derive(Default)]
pub struct CancelToken<'a, C: Cancellation> {
    cancelled: C,
    parent: Option<&'a Self>,
}
impl Cancellation for std::sync::atomic::AtomicBool {
    #[inline]
    fn is_cancelled(&self) -> bool {
        self.load(std::sync::atomic::Ordering::Acquire)
    }
    #[inline]
    fn cancel(&self) {
        self.store(true, std::sync::atomic::Ordering::Release);
    }
}
impl<C: Cancellation> CancelToken<'_, C> {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.is_cancelled()
            || self.parent.as_ref().is_some_and(|tk| {
                tk.is_cancelled() && {
                    self.cancelled.cancel();
                    true
                }
            })
    }
    #[inline]
    pub fn cancel(&self) {
        self.cancelled.cancel();
    }
    pub fn derive(&self) -> CancelToken<'_, C> {
        CancelToken {
            cancelled: C::default(),
            parent: Some(self),
        }
    }
}
impl Cancellation for () {
    #[inline]
    fn is_cancelled(&self) -> bool {
        false
    }
    #[inline(always)]
    fn cancel(&self) {}
}

pub trait SplitStrategy:
    Send + Sync + Sized + 'static + Copy + Clone + Default + std::fmt::Debug + PartialEq + Eq
{
    type CancelToken: Cancellation;
    const SYMBOL_EXTRACTORS: &[SymbolRuleExtractor<Self>] =
        { super::rules::extractors::all_symbol_extractors() };

    const RULE_EXTRACTORS: &[RuleExtractor<Self>] =
        { super::rules::extractors::all_rule_extractors() };

    fn strategies<'t, A, B, R>(
        solver: &mut CheckRef<'t, '_, Self>,
        strategy_a: &'static str,
        oper_a: A,
        strategy_b: &'static str,
        oper_b: B,
    ) -> Option<R>
    where
        A: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        B: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        R: Send + std::fmt::Debug + Clone;

    /// ### Errors
    #[allow(clippy::result_large_err)]
    fn split_i<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>, //smallvec::SmallVec<&Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Result<R, smallvec::SmallVec<RefCheckLog<'t>, 2>>;

    fn split<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>, //smallvec::SmallVec<&Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Option<R> {
        match Self::split_i(slf, msg, rules, then) {
            Ok(r) => Some(r),
            Err(ls) => {
                for e in ls {
                    slf.add_msg(e.into());
                }
                None
            }
        }
    }

    // -----------------------------

    fn strategies_st<'t, A, B, R>(
        solver: &mut CheckRef<'t, '_, Self>,
        strategy_a: &'static str,
        oper_a: A,
        strategy_b: &'static str,
        oper_b: B,
    ) -> Option<R>
    where
        A: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        B: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        R: Send + std::fmt::Debug + Clone,
    {
        let l1 = match solver
            .branch_traced(CheckingTask::Strategy(strategy_a), |mut c| oper_a(&mut c))
        {
            Ok(r) => return Some(r),
            Err(l) => l,
        };
        match solver.traced(CheckingTask::Strategy(strategy_b), oper_b) {
            Ok(r) => Some(r),
            Err(l) => {
                solver.add_msg(l1.into());
                solver.add_msg(l.into());
                None
            }
        }
    }

    /// ### Errors
    #[allow(clippy::result_large_err)]
    fn split_i_st<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>, //smallvec::SmallVec<&Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Result<R, smallvec::SmallVec<RefCheckLog<'t>, 2>> {
        //let mut rules = rules.peekable();
        if rules.is_empty() {
            return Err(if msg {
                smallvec::smallvec![traceref!(FAIL "No rule applicable (4)")]
            } else {
                smallvec::SmallVec::default()
            });
        }
        let mut failures = SmallVec::<_, 2>::new();
        for rule in rules {
            match slf.branch_traced(CheckingTask::Rule(rule.as_dyn()), |slf| {
                tracing::debug!("Applying rule {rule:?}");
                then(slf, rule)
            }) {
                Ok(r) => {
                    return Ok(r);
                }
                Err(l) => failures.push(l),
            }
        }
        Err(failures)
    }

    fn strategies_mt<'t, A, B, R>(
        solver: &mut CheckRef<'t, '_, Self>,
        strategy_a: &'static str,
        oper_a: A,
        strategy_b: &'static str,
        oper_b: B,
    ) -> Option<R>
    where
        A: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        B: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        R: Send + std::fmt::Debug + Clone,
    {
        //solver.comment("Splitting");
        solver.cancellable(|solver| {
            let mut top1 = solver.copied();
            let mut top2 = solver.copied();
            let (a, b) = rayon::join(
                || {
                    let mut solver = top1.get_ref();
                    solver
                        .traced(CheckingTask::Strategy(strategy_a), oper_a)
                        .inspect(|_| solver.cancel.cancel())
                },
                || {
                    let mut solver = top2.get_ref();
                    solver
                        .traced(CheckingTask::Strategy(strategy_b), oper_b)
                        .inspect(|_| solver.cancel.cancel())
                },
            );
            match (a, b) {
                (Ok(r), b) => {
                    drop(b);
                    //solver.comment("1 Succeeded");
                    top1.close(solver);
                    Some(r)
                }
                (a, Ok(r)) => {
                    drop(a);
                    //solver.comment("2 Succeeded");
                    top2.close(solver);
                    Some(r)
                }
                (Err(i1), Err(i2)) => {
                    solver.add_msg(i1.into());
                    solver.add_msg(i2.into());
                    None
                }
            }
        })
    }

    /// ### Errors
    #[allow(clippy::result_large_err)]
    fn split_i_mt<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Result<R, smallvec::SmallVec<RefCheckLog<'t>, 2>> {
        let then = &then;
        macro_rules! then {
            ($rl:ident !) => {{
                let mut top = slf.copied();
                let mut slf = top.get_ref();
                slf.branch_traced(CheckingTask::Rule($rl.as_dyn()), move |slf| then(slf, $rl))
                    .inspect(|_| slf.cancel.cancel())
            }};
            ($rl:expr) => {{
                slf.branch_traced(CheckingTask::Rule($rl.as_dyn()), move |slf| then(slf, $rl))
                    .inspect(|_| slf.cancel.cancel())
            }};
        }

        match rules.len() {
            0 => {
                return Err(if msg {
                    smallvec::smallvec![traceref!(FAIL "No rule applicable (5)")]
                } else {
                    smallvec::SmallVec::default()
                });
            }
            1 => {
                // SAFETY: len == 1
                let rule = unsafe { rules.first().unwrap_unchecked() };
                return slf
                    .branch_traced(CheckingTask::Rule(rule.as_dyn()), |slf| then(slf, rule))
                    .map_err(|l| smallvec::smallvec![l]);
            }
            2 => {
                // SAFETY: len == 2
                let [rule_a, rule_b] = &*rules else {
                    unsafe { unreachable_unchecked() }
                };
                return match rayon::join(|| then!(rule_a!), || then!(rule_b!)) {
                    (Ok(t), _) | (_, Ok(t)) => Ok(t),
                    (Err(i1), Err(i2)) => Err(smallvec::smallvec_inline![i1, i2]),
                };
            }
            _ => (),
        }
        let result = parking_lot::Mutex::new(None);
        let failures = parking_lot::Mutex::new(SmallVec::<_, 2>::new());
        rules.into_vec().into_par_iter().for_each(|rule| {
            if slf.cancel.is_cancelled() {
                return;
            }
            match then!(rule!) {
                Ok(r) => *result.lock() = Some(r),
                Err(l) => failures.lock().push(l),
            }
        });
        result
            .into_inner()
            .map_or_else(|| Err(failures.into_inner()), Ok)
    }
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct SingleThreadedSplit;
impl SplitStrategy for SingleThreadedSplit {
    type CancelToken = ();

    #[inline]
    fn strategies<'t, A, B, R>(
        solver: &mut CheckRef<'t, '_, Self>,
        strategy_a: &'static str,
        oper_a: A,
        strategy_b: &'static str,
        oper_b: B,
    ) -> Option<R>
    where
        A: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        B: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        R: Send + std::fmt::Debug + Clone,
    {
        Self::strategies_st(solver, strategy_a, oper_a, strategy_b, oper_b)
    }

    #[inline]
    fn split_i<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>, //smallvec::SmallVec<&Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Result<R, smallvec::SmallVec<RefCheckLog<'t>, 2>> {
        Self::split_i_st(slf, msg, rules, then)
    }
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct RayonStrategiesDepth<const DEPTH: usize>;
impl<const DEPTH: usize> SplitStrategy for RayonStrategiesDepth<DEPTH> {
    type CancelToken = std::sync::atomic::AtomicBool;
    #[inline]
    fn split_i<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>, //smallvec::SmallVec<&Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Result<R, smallvec::SmallVec<RefCheckLog<'t>, 2>> {
        Self::split_i_st(slf, msg, rules, then)
    }

    #[inline]
    fn strategies<'t, A, B, R>(
        solver: &mut CheckRef<'t, '_, Self>,
        strategy_a: &'static str,
        oper_a: A,
        strategy_b: &'static str,
        oper_b: B,
    ) -> Option<R>
    where
        A: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        B: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        R: Send + std::fmt::Debug + Clone,
    {
        if solver.depth() <= DEPTH {
            Self::strategies_mt(solver, strategy_a, oper_a, strategy_b, oper_b)
        } else {
            Self::strategies_st(solver, strategy_a, oper_a, strategy_b, oper_b)
        }
    }
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct RayonStrategiesOnly;
impl SplitStrategy for RayonStrategiesOnly {
    type CancelToken = std::sync::atomic::AtomicBool;

    #[inline]
    fn strategies<'t, A, B, R>(
        solver: &mut CheckRef<'t, '_, Self>,
        strategy_a: &'static str,
        oper_a: A,
        strategy_b: &'static str,
        oper_b: B,
    ) -> Option<R>
    where
        A: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        B: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        R: Send + std::fmt::Debug + Clone,
    {
        Self::strategies_mt(solver, strategy_a, oper_a, strategy_b, oper_b)
    }

    #[inline]
    fn split_i<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>, //smallvec::SmallVec<&Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Result<R, smallvec::SmallVec<RefCheckLog<'t>, 2>> {
        Self::split_i_st(slf, msg, rules, then)
    }
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct RayonSplit;
impl SplitStrategy for RayonSplit {
    type CancelToken = std::sync::atomic::AtomicBool;

    #[inline]
    fn strategies<'t, A, B, R>(
        solver: &mut CheckRef<'t, '_, Self>,
        strategy_a: &'static str,
        oper_a: A,
        strategy_b: &'static str,
        oper_b: B,
    ) -> Option<R>
    where
        A: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        B: FnOnce(&mut CheckRef<'t, '_, Self>) -> Option<R> + Send,
        R: Send + std::fmt::Debug + Clone,
    {
        Self::strategies_mt(solver, strategy_a, oper_a, strategy_b, oper_b)
    }

    #[inline]
    fn split_i<'t, Rl: CheckerRule + ?Sized, R: Send + std::fmt::Debug + Clone + 'static>(
        slf: &mut CheckRef<'t, '_, Self>,
        msg: bool,
        rules: smallvec::SmallVec<&'t Rl, 2>,
        then: impl Fn(CheckRef<'t, '_, Self>, &Rl) -> Option<R> + Send + Sync,
    ) -> Result<R, smallvec::SmallVec<RefCheckLog<'t>, 2>> {
        Self::split_i_mt(slf, msg, rules, then)
    }
}
