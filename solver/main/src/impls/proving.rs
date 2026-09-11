use std::borrow::Cow;

use ftml_ontology::terms::{
    ApplicationTerm, Argument, ComponentVar, Term, Variable, eq::Alpha, helpers::IntoTerm,
};
use ftml_solver_trace::CheckingTask;
use ftml_uris::{Id, SymbolUri};

const MAX_DEPTH: usize = 8;

use crate::{
    CheckRef,
    facts::{Fact, GlobalOrLocal, GoalPremise, LocalFacts},
    hoas::HOASSymbols,
    rules::ProofRule,
    split::SplitStrategy,
};

#[derive(Default)]
pub(crate) struct ProverState(
    parking_lot::RwLock<ProverStateI>,
    std::sync::atomic::AtomicUsize,
);
impl std::fmt::Debug for ProverState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.read().fmt(f)
    }
}
impl ProverState {
    fn add_goal<Split: SplitStrategy>(
        &self,
        t: &Term,
        checker: &CheckRef<'_, '_, Split>,
    ) -> Result<Option<Term>, GoalId> {
        let mut lock = self.0.write();
        if let Some((i, (_, _))) = lock
            .goals
            .iter()
            .enumerate()
            .find(|(_, (e, _))| e.alpha_equal(t))
        {
            tracing::debug!("Already exists");
            if let Some(d) = lock.get_solution(GoalId(i), checker) {
                Ok(Some(d))
            } else {
                drop(lock);
                // avoid recursion
                Ok(None) //Err(GoalId(i))
            }
        } else {
            tracing::debug!("New goal");
            let len = lock.goals.len();
            let mut strategies = Vec::new();
            for (strat, premises) in checker.facts_for(t) {
                let mut done = true;
                let mut npremises = Vec::new();
                for p in premises {
                    if let Some((i, old)) = lock
                        .premises
                        .iter()
                        .enumerate()
                        .find(|(_, e)| e.matches_goal(&p, &mut Alpha::new()))
                    {
                        match old.proven() {
                            Some(true) => {
                                npremises.push(PremiseId(i));
                            }
                            Some(false) => return Ok(None),
                            None => {
                                done = false;
                                npremises.push(PremiseId(i));
                            }
                        }
                    } else {
                        done = false;
                        npremises.push(PremiseId(lock.premises.len()));
                        lock.premises.push(p.into());
                    }
                }
                if done {
                    return Ok(
                        lock.close_done(strat.into_term(checker.context.as_ref()), &npremises)
                    );
                }
                let id = lock.strategies.len();
                strategies.push(StrategyId(id));
                lock.strategies.push((GoalId(len), strat, npremises));
            }
            if strategies.is_empty() {
                Ok(None)
            } else {
                lock.goals.push((t.clone(), GoalState::Running(strategies)));
                drop(lock);
                Err(GoalId(len))
            }
        }
    }
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GoalId(usize);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StrategyId(usize);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PremiseId(usize);

enum GoalState {
    Running(Vec<StrategyId>),
    //Done(Term),
}

#[derive(Default)]
struct ProverStateI {
    goals: Vec<(Term, GoalState)>,
    strategies: Vec<(GoalId, GlobalOrLocal, Vec<PremiseId>)>,
    premises: Vec<Premise>,
}

enum Premise {
    Typing {
        elem: Term,
        tp: Term,
        is_sequence: bool,
        done: Option<bool>,
    },
    Proof {
        goal: Term,
        proof: Option<Option<Term>>,
    },
    NeedSuchThat {
        name: Variable,
        tp: Term,
        is_sequence: bool,
        premises: Vec<Self>,
        found: Option<Term>,
    },
}
impl From<GoalPremise> for Premise {
    fn from(value: GoalPremise) -> Self {
        match value {
            GoalPremise::Typing {
                elem,
                tp,
                is_sequence,
            } => Self::Typing {
                elem,
                tp,
                is_sequence,
                done: None,
            },
            GoalPremise::Proof(t) => Self::Proof {
                goal: t,
                proof: None,
            },
            GoalPremise::NeedSuchThat {
                name,
                tp,
                is_sequence,
                premises,
            } => Self::NeedSuchThat {
                name: Variable::Name {
                    name,
                    notated: None,
                },
                tp,
                is_sequence,
                premises: premises.into_iter().map(Into::into).collect(),
                found: None,
            },
        }
    }
}
impl Premise {
    fn proven(&self) -> Option<bool> {
        match self {
            Self::Typing { done, .. } => *done,
            Self::Proof { proof, .. } => proof.as_ref().map(|_| true),
            Self::NeedSuchThat { found, .. } => found.as_ref().map(|_| true),
        }
    }
    fn matches_goal<'s>(&'s self, other: &'s GoalPremise, alpha: &mut Alpha<'s>) -> bool {
        match (self, other) {
            (
                Self::Typing {
                    elem,
                    tp,
                    is_sequence,
                    ..
                },
                GoalPremise::Typing {
                    elem: elem2,
                    tp: tp2,
                    is_sequence: is_sequence2,
                },
            ) => {
                *is_sequence == *is_sequence2
                    && elem2.alpha_equal_under(elem, alpha)
                    && tp2.alpha_equal_under(tp, alpha)
            }
            (Self::Proof { goal, .. }, GoalPremise::Proof(goal2)) => {
                goal2.alpha_equal_under(goal, alpha)
            }
            (
                Self::NeedSuchThat {
                    name,
                    tp,
                    is_sequence,
                    premises,
                    ..
                },
                GoalPremise::NeedSuchThat {
                    name: name2,
                    tp: tp2,
                    is_sequence: is_sequence2,
                    premises: premises2,
                },
            ) => {
                *is_sequence == *is_sequence2 && tp2.alpha_equal_under(tp, alpha) && {
                    alpha.push((name2.as_ref(), name));
                    if premises2
                        .iter()
                        .all(|p2| premises.iter().any(|p| p.matches_goal(p2, alpha)))
                    {
                        alpha.pop();
                        true
                    } else {
                        false
                    }
                }
            }
            _ => false,
        }
    }
}

impl ProverState {
    /*pub fn new(hoas: &HOASSymbols) -> Self {
        ProverState {
            goals: Vec::new(), //subs: Vec::new(),
                               //main: Vec::new(),
                               //attempts: Vec::new(),
                               //used: ctx.len(),
        }
    }*/
    /*pub fn clean(&mut self, context: &[Cow<'_, ComponentVar>]) {
        TODO
    }*/
}

impl ProverStateI {
    fn close_done(&self, head: Term, premises: &[PremiseId]) -> Option<Term> {
        let Ok(args) = premises
            .iter()
            .filter_map(|i| {
                let Some(p) = self.premises.get(i.0) else {
                    return Some(Err(()));
                };
                match p {
                    Premise::Typing {
                        done: Some(true), ..
                    } => None,
                    Premise::Proof {
                        proof: Some(Some(proof)),
                        ..
                    }
                    | Premise::NeedSuchThat {
                        found: Some(proof), ..
                    } => Some(Ok(Argument::Simple(proof.clone()))),
                    _ => Some(Err(())),
                }
            })
            .collect::<Result<Vec<_>, ()>>()
        else {
            return None;
        };
        Some(if args.is_empty() {
            head
        } else {
            Term::Application(ApplicationTerm::new(head, args.into_boxed_slice(), None))
        })
    }
    fn get_solution<Split: SplitStrategy>(
        &self,
        id: GoalId,
        checker: &CheckRef<Split>,
    ) -> Option<Term> {
        match &self.goals.get(id.0)?.1 {
            //GoalState::Done(t) => Some(t.clone()),
            GoalState::Running(v) => {
                for strat in v {
                    if let Some(t) = self.strategy_done(*strat, checker) {
                        return Some(t);
                    }
                }
                None
            }
        }
    }
    fn strategy_done<Split: SplitStrategy>(
        &self,
        id: StrategyId,
        checker: &CheckRef<Split>,
    ) -> Option<Term> {
        let strat = self.strategies.get(id.0)?;
        if strat.2.iter().all(|e| {
            self.premises
                .get(e.0)
                .is_some_and(|e| e.proven() == Some(true))
        }) {
            self.close_done(
                strat.1.clone().into_term(checker.context.as_ref()),
                &strat.2,
            )
        } else {
            None
        }
    }
}

impl<'t, Split: SplitStrategy> CheckRef<'t, '_, Split> {
    pub fn prove(&mut self, goal: &'t Term) -> Option<Term> {
        //self.untraced(CheckingTask::Proving(goal), |slf| slf.prove_i(goal))
        self.wrap_check(CheckingTask::Proving(goal), |slf| slf.prove_i(goal))
    }
    pub(crate) fn prove_i(&mut self, goal: &'t Term) -> Option<Term> {
        let judgment = self.top.hoas.as_ref()?.judgment.as_ref()?;
        //tracing::debug!("Facts: {:#?}", self.facts);
        //
        let r = if let Some([Argument::Simple(goal)]) = goal.unapply(judgment) {
            self.backchain(goal)
        } else {
            tracing::debug!("Does not match {judgment}: {:?}", goal.debug_short());
            self.simplify_rules(
                self.top.rules.proof(),
                goal,
                ProofRule::applicable,
                |slf, rl, t| rl.prove(slf, t),
            )
        };
        r.map(|t| self.subst(t))
    }

    fn backchain(&mut self, goal: &'t Term) -> Option<Term> {
        tracing::debug!(
            "Proving {:?}", // in context:\n{:#?}",
            goal.debug_short(),
            //self.context
        );

        //println!("{}", std::backtrace::Backtrace::force_capture());

        let sgoal = self.simplify_full(crate::impls::simplify::Expansion::Full, goal);
        let goal = sgoal.as_ref().unwrap_or(goal);

        /*tracing::debug!(
            "Facts: \n Global: {:#?}\n Local: {:#?}",
            self.top.facts,
            self.context.facts()
        );*/

        let proof_state = self.proof_state;
        let goal_id = match proof_state.add_goal(goal, self) {
            Ok(r) => return r,
            Err(i) => i,
        };
        //tracing::warn!("Proof State: {proof_state:#?}");

        let dp = proof_state
            .1
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        if dp > MAX_DEPTH {
            self.failure("exceeded maximum depth");
            proof_state
                .1
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            return None;
        }

        let mut lock = proof_state.0.read();
        loop {
            let Some((i, next)) =
                lock.premises.iter().enumerate().find(|(_, e)| {
                    !matches!(*e, Premise::NeedSuchThat { .. }) && e.proven().is_none()
                })
            else {
                drop(lock);
                self.failure("No more premises");
                //self.comment(format!("{proof_state:#?}"));
                proof_state
                    .1
                    .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                return None;
            };
            let idx = PremiseId(i);
            match next {
                Premise::Typing {
                    elem,
                    tp,
                    is_sequence,
                    ..
                } => {
                    let tm = elem.clone();
                    let tp = tp.clone();
                    drop(lock);
                    let r = self.scoped(|slf| slf.check_type(&tm, &tp)) == Some(true);
                    let mut lock = proof_state.0.write();
                    if let Some(Premise::Typing { done, .. }) = lock.premises.get_mut(idx.0) {
                        *done = Some(r);
                    }
                    drop(lock);
                }
                Premise::Proof { goal, .. } => {
                    let goal = goal.clone();
                    drop(lock);
                    let goal = self.top.hoas.as_ref()?.judgment.clone()?.apply_tms([goal]);
                    let r = self.scoped(|slf| slf.prove(&goal));
                    let mut lock = proof_state.0.write();
                    if let Some(Premise::Proof { proof, .. }) = lock.premises.get_mut(idx.0) {
                        *proof = Some(r);
                    }
                }
                _ => todo!(),
            }
            lock = proof_state.0.read();
            // lock is dropped!
            if let Some(proof) = lock.get_solution(goal_id, self) {
                proof_state
                    .1
                    .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                return Some(proof);
            }

            // TODO check if done
        }

        /*
        let mut local_facts = LocalFacts::default();
        for (i, c) in self.context.0.iter().enumerate() {
            if let Some(tp) = c.tp.as_ref()
                && let Some(fact) = Fact::from_tp(hoas, tp)
            {
                local_facts.facts.push((i, fact));
            }
        }

        let mut state = ProofState {
            local: local_facts,
            subs: Vec::new(),
            main: Vec::new(),
            attempts: Vec::new(),
        };
        if !self.backchain_next(&mut state, goal) {
            self.failure("No applicable strategies");
            return None;
        }
        std::mem::swap(&mut state.main, &mut state.attempts);
        tracing::warn!("Here: {state:#?}");

        /*tracing::debug!(
            "Facts: \n Global: {:#?}\n Local: {:#?}",
            self.top.facts,
            local_facts
        );*/

        None
         */
    }

    /*
    fn backchain_next(&mut self, state: &mut ProverState, next: &'t Term) -> bool {
        let mut facts = Vec::new();
        let Some(t) = self.simplify_until(next, |slf, t| {
            facts = slf.facts_for(t).collect();
            !facts.is_empty()
        }) else {
            self.failure("No applicable proof rules");
            return false;
        };
        self.counter("Facts found", facts.len());
        let mut added = false;
        for (f, gls) in facts {
            let mut changed = false;
            let r = gls
                .into_iter()
                .map(|g| {
                    if let Some(sg) = state.subs.iter().position(|e| *e == g) {
                        sg
                    } else {
                        changed = true;
                        state.subs.push(g.into());
                        state.subs.len() - 1
                    }
                })
                .collect();
            if changed {
                state.attempts.push(ProofAttempt { rf: f, subgoals: r });
                added = true;
            }
            //tracing::warn!("{f}: {gls:?}");
        }
        added
    }
     */
}

impl std::fmt::Debug for ProverStateI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = f.debug_struct("ProverState");
        out.field("goals", &GoalStateDebug(&self.goals))
            .field("strategies", &StrategiesDebug(&self.strategies))
            .field("premises", &self.premises)
            .finish()
    }
}
impl std::fmt::Debug for Premise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Typing {
                elem,
                tp,
                is_sequence,
                done,
            } => write!(
                f,
                "{:?}  :  {:?}{}  ({:?})",
                elem.debug_short(),
                tp.debug_short(),
                if *is_sequence { "*" } else { "" },
                done
            ),
            Self::Proof { goal, proof } => {
                write!(
                    f,
                    "⊢ {:?}  :=  {:?}",
                    goal.debug_short(),
                    proof
                        .as_ref()
                        .and_then(|t| t.as_ref().map(Term::debug_short))
                )
            }
            Self::NeedSuchThat {
                name,
                tp,
                is_sequence,
                premises,
                found,
            } => write!(
                f,
                "SOME {name} : {:?}{}  := {:?} {:?}",
                tp.debug_short(),
                if *is_sequence { "*" } else { "" },
                found.as_ref().map(Term::debug_short),
                premises
            ),
        }
    }
}

struct GoalStateDebug<'s>(&'s [(Term, GoalState)]);
impl std::fmt::Debug for GoalStateDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter().map(GSPair)).finish()
    }
}
struct GSPair<'s>(&'s (Term, GoalState));
impl std::fmt::Debug for GSPair<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {:?}", self.0.0.debug_short(), &self.0.1)
    }
}
struct StrategiesDebug<'s>(&'s [(GoalId, GlobalOrLocal, Vec<PremiseId>)]);
impl std::fmt::Debug for StrategiesDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(StratLine))
            .finish()
    }
}
struct StratLine<'s>(&'s (GoalId, GlobalOrLocal, Vec<PremiseId>));

impl std::fmt::Debug for StratLine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {} {:?}", self.0.0, self.0.1, self.0.2)
    }
}
impl std::fmt::Debug for GoalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running(v) => v.fmt(f), //f.debug_tuple("Running").field(v).finish(),
                                          //Self::Done(t) => t.debug_short().fmt(f),
        }
    }
}
impl std::fmt::Debug for GoalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::fmt::Debug for StrategyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::fmt::Debug for PremiseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/*
struct ProofAttempt {
    rf: GlobalOrLocal,
    subgoals: Vec<usize>,
}
impl std::fmt::Debug for ProofAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} subgoals)", self.rf, self.subgoals.len())
    }
}

#[derive(Hash)]
enum ProofProgress {
    None,
    InProgress(Vec<usize>),
    Done(Term),
}
impl std::fmt::Debug for ProofProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("not yet started"),
            Self::InProgress(_) => f.write_str("attempted"),
            Self::Done(t) => write!(f, "SUCCESS: {:?}", t.debug_short()),
        }
    }
}

enum SubgoalI {
    Typing {
        elem: Term,
        tp: Term,
        checked: Option<bool>,
        is_sequence: bool,
    },
    Premise {
        goal: Term,
        found: ProofProgress,
    },
    Need {
        name: Id,
        tp: Term,
        is_sequence: bool,
        df: Option<Term>,
    },
}
impl PartialEq<Subgoal> for SubgoalI {
    fn eq(&self, other: &Subgoal) -> bool {
        match (self, other) {
            (
                Self::Typing {
                    elem,
                    tp,
                    is_sequence,
                    ..
                },
                Subgoal::Typing {
                    elem: e2,
                    tp: t2,
                    is_sequence: i2,
                },
            ) => alpha_equal(elem, e2) && alpha_equal(tp, t2) && is_sequence == i2,
            (Self::Premise { goal, .. }, Subgoal::Premise(g2)) => alpha_equal(goal, g2),
            (
                Self::Need {
                    name,
                    tp,
                    is_sequence,
                    ..
                },
                Subgoal::Need {
                    name: n2,
                    tp: t2,
                    is_sequence: i2,
                },
            ) => name == n2 && tp == t2 && is_sequence == i2,
            _ => false,
        }
    }
}
impl From<Subgoal> for SubgoalI {
    fn from(value: Subgoal) -> Self {
        match value {
            Subgoal::Typing {
                elem,
                tp,
                is_sequence,
            } => Self::Typing {
                elem,
                tp,
                is_sequence,
                checked: None,
            },
            Subgoal::Premise(tm) => Self::Premise {
                goal: tm,
                found: ProofProgress::None,
            },
            Subgoal::Need {
                name,
                tp,
                is_sequence,
            } => Self::Need {
                name,
                tp,
                is_sequence,
                df: None,
            },
        }
    }
}
impl PartialEq for SubgoalI {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Typing {
                    elem,
                    tp,
                    is_sequence,
                    ..
                },
                Self::Typing {
                    elem: elem2,
                    tp: tp2,
                    is_sequence: is_sequence2,
                    ..
                },
            ) => elem == elem2 && tp == tp2 && is_sequence == is_sequence2,
            (Self::Premise { goal, .. }, Self::Premise { goal: goal2, .. }) => goal == goal2,
            _ => false,
        }
    }
}

 */
