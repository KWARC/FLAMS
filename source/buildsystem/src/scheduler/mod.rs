// The scheduler owns a dependency graph of tasks and drives it forward by
// handing "ready" tasks to the executor and reacting to the state updates
// the executor reports back. Crucially: nodes and edges are never removed
// from the graph. Completion is tracked purely as per-node state
// (`ExecutorState`), which is what "ready" is computed from. This mirrors
// what actually happens on the wire: sending a task on `sender` only means
// "the executor accepted it", not "it ran" - the executor reports Running,
// then Finished/Failed, asynchronously and out of order, via
// `state_updater`. Mutating graph structure on send (the old approach)
// conflated those two things and could unblock a task's dependents before
// the dependency had actually finished.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
};

use petgraph::{
    Direction::Outgoing,
    algo::kosaraju_scc,
    graph::{DiGraph, NodeIndex},
    stable_graph::StableDiGraph,
};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::executor::{ExecutorState, State};

/// Result of one run of `Scheduler::drain_acyclic` (the shared Kahn's-only
/// phase behind both `schedule` and `schedule_topo`).
enum DrainOutcome {
    /// Every node reached Finished/Failed.
    Settled,
    /// Nothing ready, nothing in flight, nodes still remain - the rest of
    /// the graph is entirely cycles.
    Stalled,
    /// The executor's side of the channel closed mid-drain.
    ExecutorGone,
}

pub struct Scheduler<T: Clone + Eq + Hash> {
    graph: StableDiGraph<T, ()>,
    // reverse lookup: task identity -> its node, so a `State<T>` coming back
    // from the executor can be applied to the right node.
    index: HashMap<T, NodeIndex>,
    // per-node execution state. Absent == ExecutorState::None (not yet
    // dispatched). This is the thing that changes instead of the graph.
    state: HashMap<NodeIndex, ExecutorState>,
    // Kahn's algorithm, kept incremental instead of rescanned: number of
    // not-yet-Finished dependencies remaining for each node (absent == 0).
    // A node becomes ready the instant this hits 0 - decremented in
    // `apply_state` when a dependency reports Finished, never recomputed
    // from scratch.
    pending: HashMap<NodeIndex, usize>,
    // Nodes with pending == 0 that have never been dispatched - the
    // frontier `dispatch_ready` drains. Populated once per node, either by
    // `seed_ready` (nodes that started with no dependencies) or by
    // `apply_state` (nodes whose last dependency just finished).
    ready: VecDeque<NodeIndex>,
    ready_seeded: bool,
    // Count of nodes that have reached Finished or Failed, so `all_settled`
    // is an O(1) comparison instead of an O(V) scan over every node.
    settled_count: usize,
    sender: Sender<T>,
    state_updater: Receiver<State<T>>,
    // number of nodes dispatched (Queued or Running) but not yet
    // Finished/Failed. Used to tell "nothing ready because we're waiting on
    // in-flight work" apart from "nothing ready because we're deadlocked on
    // a cycle".
    in_flight: usize,
    // Nodes whose real dependency was paid off (pending hit 0) while they
    // were still `Queued`/`Running` on a forced dispatch - i.e. the
    // in-flight run started before it was actually safe to trust. Its
    // report is discarded and it's redispatched immediately instead of
    // being settled (see `apply_state`). This only covers the race where
    // the payoff lands *before* the forced run reports back; the far more
    // common case - payoff arriving *after* the forced run already
    // finished - doesn't need this set at all (handled inline, see below).
    reclose: HashSet<NodeIndex>,
    // Nodes whose one-time Kahn "unblock my dependents" step has already
    // run. A force-dispatched node can report Finished twice (once
    // prematurely, once for real once its own dependency is satisfied,
    // see `apply_state`) - this guards against decrementing its
    // dependents' `pending` twice for the same edge, which would otherwise
    // cause repeated bogus re-closes to ping-pong forever.
    kahn_applied: HashSet<NodeIndex>,
    // One-time cache of the graph's strongly connected components,
    // computed lazily on first use by `unblock_one_cycle_node`. SCC
    // membership is a pure function of graph topology, and topology never
    // changes after construction (nodes/edges are never removed - see the
    // top of this file), so recomputing it on every stall would be pure
    // waste; every stall reuses this same partition instead.
    sccs: Option<Vec<Vec<NodeIndex>>>,
}

impl<T: Clone + Eq + Hash> Scheduler<T> {
    pub fn new(sender: Sender<T>, state_updater: Receiver<State<T>>) -> Self {
        Self {
            graph: StableDiGraph::new(),
            index: HashMap::new(),
            state: HashMap::new(),
            pending: HashMap::new(),
            ready: VecDeque::new(),
            ready_seeded: false,
            settled_count: 0,
            sender,
            state_updater,
            in_flight: 0,
            reclose: HashSet::new(),
            kahn_applied: HashSet::new(),
            sccs: None,
        }
    }

    /// Build a scheduler directly from an already-assembled dependency
    /// graph (e.g. `TaskMap::create_graph`'s output), where an edge
    /// `dep -> dependent` means `dep` must finish before `dependent` runs.
    pub fn from_graph(
        graph: DiGraph<T, ()>,
        sender: Sender<T>,
        state_updater: Receiver<State<T>>,
    ) -> Self {
        let mut scheduler = Self::new(sender, state_updater);
        let mut remap = HashMap::new();
        for old_idx in graph.node_indices() {
            let weight = graph
                .node_weight(old_idx)
                .expect("just iterated it")
                .clone();
            remap.insert(old_idx, scheduler.add_task(weight));
        }
        for edge in graph.raw_edges() {
            scheduler.link(remap[&edge.source()], remap[&edge.target()]);
        }
        scheduler
    }

    fn get_or_insert(&mut self, task: T) -> NodeIndex {
        if let Some(&idx) = self.index.get(&task) {
            return idx;
        }
        let idx = self.graph.add_node(task.clone());
        self.index.insert(task, idx);
        self.state.insert(idx, ExecutorState::None);
        idx
    }

    /// Register a task with no known dependencies/dependents yet.
    pub fn add_task(&mut self, task: T) -> NodeIndex {
        self.get_or_insert(task)
    }

    /// Add the edge `dep_idx -> dependent_idx` if it isn't already present,
    /// keeping `pending` (dependent's not-yet-finished dependency count) in
    /// sync. Only a genuinely new edge changes `pending` - calling this
    /// twice for the same pair is a no-op the second time.
    fn link(&mut self, dep_idx: NodeIndex, dependent_idx: NodeIndex) {
        if self.graph.find_edge(dep_idx, dependent_idx).is_some() {
            return;
        }
        self.graph.add_edge(dep_idx, dependent_idx, ());
        if self.node_state(dep_idx) != ExecutorState::Finished {
            *self.pending.entry(dependent_idx).or_insert(0) += 1;
        }
    }

    /// Record that `dependency` must finish before `dependent` can run.
    /// Idempotent: calling it twice for the same pair doesn't duplicate the
    /// edge.
    pub fn add_dependency(&mut self, dependency: T, dependent: T) {
        let dep_idx = self.get_or_insert(dependency);
        let dependent_idx = self.get_or_insert(dependent);
        self.link(dep_idx, dependent_idx);
    }

    fn node_state(&self, idx: NodeIndex) -> ExecutorState {
        self.state.get(&idx).copied().unwrap_or(ExecutorState::None)
    }

    fn node_pending(&self, idx: NodeIndex) -> usize {
        self.pending.get(&idx).copied().unwrap_or(0)
    }

    /// One-time O(V+E) Kahn setup: every node that started with no
    /// dependencies is ready immediately. Everything after this is
    /// incremental (see `apply_state`), so this only ever runs once.
    fn seed_ready(&mut self) {
        if self.ready_seeded {
            return;
        }
        self.ready_seeded = true;
        for idx in self.graph.node_indices() {
            if self.node_pending(idx) == 0 {
                self.ready.push_back(idx);
            }
        }
    }

    fn all_settled(&self) -> bool {
        self.settled_count == self.graph.node_count()
    }

    /// Send every currently-ready node to the executor. Marks each as
    /// Queued immediately so it can never be enqueued twice - this is the
    /// replacement for removing the node from the graph.
    async fn dispatch_ready(&mut self) {
        self.seed_ready();
        while let Some(idx) = self.ready.pop_front() {
            let weight = self
                .graph
                .node_weight(idx)
                .expect("just iterated it")
                .clone();
            self.sender
                .send(weight)
                .await
                .expect("executor channel closed");
            self.state.insert(idx, ExecutorState::Queued);
            self.in_flight += 1;
        }
    }

    /// Apply one state update coming back from the executor to the node it
    /// refers to. On a first-time transition to Finished, this is also
    /// Kahn's "decrement dependents, enqueue any that just hit zero" step -
    /// done incrementally here instead of rescanning the whole graph.
    fn apply_state(&mut self, update: State<T>) {
        let Some(&idx) = self.index.get(update.task()) else {
            // Update for a task this scheduler never dispatched - ignore
            // rather than panicking, in case the executor is shared.
            return;
        };
        let new_state = update.taskstate();

        // This report belongs to a forced dispatch whose real dependency
        // was paid off *while it was still in flight* (see the
        // `Queued | Running` arm below). That run was a throwaway - its
        // result doesn't count, and it must not unblock its own
        // dependents. Discard it and immediately redispatch for the real,
        // dependency-respecting run instead of ever recording it settled.
        if matches!(new_state, ExecutorState::Finished | ExecutorState::Failed)
            && self.reclose.remove(&idx)
        {
            self.in_flight = self.in_flight.saturating_sub(1);
            self.state.insert(idx, ExecutorState::None);
            self.ready.push_back(idx);
            return;
        }

        let already_settled = matches!(
            self.node_state(idx),
            ExecutorState::Finished | ExecutorState::Failed
        );
        // `settled_count`/`in_flight` track *current* state, not history:
        // a node forced through a premature run and later reset to `None`
        // for its real run is "unsettled" again in between, and this needs
        // to move back and forth with it exactly like the first time.
        if !already_settled && matches!(new_state, ExecutorState::Finished | ExecutorState::Failed)
        {
            self.in_flight = self.in_flight.saturating_sub(1);
            self.settled_count += 1;
        }
        // Kahn's "decrement dependents, enqueue any that just hit zero"
        // step, by contrast, is keyed to the *edge*, not the report: it
        // must run exactly once per node ever, no matter how many times
        // that node reports Finished (a forced node reports it twice - see
        // below). `kahn_applied` is the one-time guard for that, tracked
        // separately from `state` so a later reset back to `None` doesn't
        // make this fire again.
        if new_state == ExecutorState::Finished && self.kahn_applied.insert(idx) {
            let dependents: Vec<NodeIndex> =
                self.graph.neighbors_directed(idx, Outgoing).collect();
            for dep in dependents {
                let p = self.pending.entry(dep).or_insert(0);
                *p = p.saturating_sub(1);
                if *p == 0 {
                    match self.node_state(dep) {
                        ExecutorState::None => self.ready.push_back(dep),
                        ExecutorState::Queued | ExecutorState::Running => {
                            // `dep` is mid-flight on a forced dispatch that
                            // went out before its dependency on `idx` was
                            // satisfied. Flag it: when that run reports
                            // back, discard it and redispatch instead of
                            // settling (see the `reclose` check above).
                            self.reclose.insert(dep);
                        }
                        ExecutorState::Finished | ExecutorState::Failed => {
                            // `dep` already finished its forced run before
                            // we knew it truly depended on `idx`. Undo the
                            // settlement and send it out again - this is
                            // the real run, closing the cycle. Its own
                            // `kahn_applied` entry stays put, so this
                            // second run won't double-decrement whatever
                            // *it* depends on.
                            self.settled_count = self.settled_count.saturating_sub(1);
                            self.state.insert(dep, ExecutorState::None);
                            self.ready.push_back(dep);
                        }
                    }
                }
            }
        }
        self.state.insert(idx, new_state);
    }

    /// Phase 1, shared by `schedule` and `schedule_topo`: pure Kahn's
    /// algorithm, no cycle-breaking. Dispatches whatever is ready, waits
    /// for the executor to report back, updates state, and repeats until
    /// either every node has settled or it stalls (nothing ready, nothing
    /// in flight, nodes still remain - which can only mean a cycle).
    async fn drain_acyclic(&mut self) -> DrainOutcome {
        loop {
            self.dispatch_ready().await;

            if self.all_settled() {
                return DrainOutcome::Settled;
            }

            if self.in_flight == 0 {
                return DrainOutcome::Stalled;
            }

            match self.state_updater.recv().await {
                Some(update) => self.apply_state(update),
                None => return DrainOutcome::ExecutorGone,
            }
        }
    }

    /// Drive every node to Finished/Failed. Runs the acyclic phase first;
    /// if that stalls, the remainder is entirely cycles - break one edge's
    /// worth of deadlock by force-dispatching the most-depended-on
    /// undispatched node in the most "downstream" unresolved SCC, then
    /// hand back to the acyclic phase, which carries on from there (its
    /// dependents unblock normally once it reports Finished). Repeats
    /// across as many separate cycles as the graph has.
    pub async fn schedule(&mut self) {
        loop {
            match self.drain_acyclic().await {
                DrainOutcome::Settled | DrainOutcome::ExecutorGone => return,
                DrainOutcome::Stalled => {
                    if !self.unblock_one_cycle_node().await {
                        // nothing ready, nothing in flight, and no
                        // undispatched node anywhere - genuinely nothing
                        // left to do.
                        return;
                    }
                }
            }
        }
    }

    /// Like `schedule`, but asserts the graph is acyclic instead of trying
    /// to break cycles - use when the caller has already guaranteed there
    /// are none.
    pub async fn schedule_topo(&mut self) {
        if let DrainOutcome::Stalled = self.drain_acyclic().await {
            panic!("schedule_topo: stuck with no ready and no in-flight nodes - graph has a cycle");
        }
    }

    /// Find the most fundamental unresolved cycle (the SCCs are visited in
    /// the same "most downstream first" order Kahn's algorithm would peel
    /// them off in) and force-dispatch its most-depended-on undispatched
    /// member. Returns false if there was nothing left to unblock.
    ///
    /// `kosaraju_scc` only ever runs once per scheduler (see `sccs`) - a
    /// settled node can make a cached component look more connected than
    /// the *live* deadlock actually is (its edges have already had their
    /// effect elsewhere), but that never hides a real candidate: removing
    /// nodes from a graph can only split components apart, never merge
    /// them, so every still-live cycle is entirely contained within one of
    /// these cached components. Filtering each one down to its
    /// undispatched (`None`) members - exactly like before - still finds
    /// it, just without ever re-walking the graph to do so.
    async fn unblock_one_cycle_node(&mut self) -> bool {
        if self.sccs.is_none() {
            self.sccs = Some(kosaraju_scc(&self.graph));
        }
        let candidate = self
            .sccs
            .as_ref()
            .expect("just ensured Some")
            .iter()
            .rev()
            .find_map(|scc| {
                scc.iter()
                    .copied()
                    .filter(|&n| self.node_state(n) == ExecutorState::None)
                    .max_by_key(|&n| self.graph.neighbors_directed(n, Outgoing).count())
            });

        let Some(node) = candidate else {
            return false;
        };
        let weight = self.graph.node_weight(node).expect("in graph").clone();
        self.sender
            .send(weight)
            .await
            .expect("executor channel closed");
        self.state.insert(node, ExecutorState::Queued);
        self.in_flight += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Executor;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    // Runs an Executor on its own task, logging each task id (in the order
    // its blocking work actually completed) to `log`. Returns the join
    // handle so the caller can await clean shutdown after dropping the
    // scheduler (which closes the channel `rx` reads from).
    fn spawn_executor(
        permits: usize,
        rx: Receiver<u32>,
        stx: Sender<State<u32>>,
        log: Arc<Mutex<Vec<u32>>>,
    ) -> tokio::task::JoinHandle<()> {
        let executor = Executor::new(permits, rx, stx);
        tokio::spawn(async move {
            executor
                .run(move |t: u32| {
                    // real (blocking) work, running on tokio's blocking pool
                    std::thread::sleep(Duration::from_millis(5));
                    log.lock().unwrap().push(t);
                    Ok(())
                })
                .await;
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn diamond_dependency_runs_in_dependency_order() {
        // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3  (A -> B, A -> C, B -> D, C -> D)
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(16);
        let (stx, srx) = tokio::sync::mpsc::channel::<State<u32>>(16);
        let mut scheduler = Scheduler::new(tx, srx);
        scheduler.add_dependency(0, 1);
        scheduler.add_dependency(0, 2);
        scheduler.add_dependency(1, 3);
        scheduler.add_dependency(2, 3);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_executor(4, rx, stx, log.clone());

        // If dispatch mutated the graph on *send* instead of on actual
        // completion, 3 could be sent before 1/2's spawn_blocking work had
        // run - this would still terminate but could violate ordering. The
        // timeout guards against the opposite failure mode (deadlock).
        tokio::time::timeout(Duration::from_secs(5), scheduler.schedule())
            .await
            .expect("scheduler should not hang on an acyclic graph");

        drop(scheduler); // closes `tx`, letting the executor's recv loop end
        handle.await.unwrap();

        let log = log.lock().unwrap();
        assert_eq!(log.len(), 4);
        let pos = |t: u32| log.iter().position(|&x| x == t).unwrap();
        assert!(pos(0) < pos(1), "A must finish before B: {log:?}");
        assert!(pos(0) < pos(2), "A must finish before C: {log:?}");
        assert!(pos(1) < pos(3), "B must finish before D: {log:?}");
        assert!(pos(2) < pos(3), "C must finish before D: {log:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_node_cycle_closes_by_rerunning_the_forced_node() {
        // 0 -> 1 -> 0: unresolvable cycle. schedule() force-dispatches one
        // side to break the deadlock, which lets the other side run for
        // real and pay off the forced node's own dependency - so the
        // forced node then runs a *second* time, for real, to actually
        // close the cycle instead of just having its debt forgiven. Net
        // result: one task runs twice, the other exactly once, and the
        // once-only task's run falls strictly between the forced node's
        // two runs.
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(16);
        let (stx, srx) = tokio::sync::mpsc::channel::<State<u32>>(16);
        let mut scheduler = Scheduler::new(tx, srx);
        scheduler.add_dependency(0, 1);
        scheduler.add_dependency(1, 0);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_executor(2, rx, stx, log.clone());

        tokio::time::timeout(Duration::from_secs(5), scheduler.schedule())
            .await
            .expect("scheduler must not deadlock on a cycle");

        drop(scheduler);
        handle.await.unwrap();

        let log = log.lock().unwrap();
        assert_eq!(log.len(), 3, "one task should rerun to close the cycle: {log:?}");
        let count_of = |t: u32| log.iter().filter(|&&x| x == t).count();
        let forced = if count_of(0) == 2 { 0 } else { 1 };
        let other = 1 - forced;
        assert_eq!(count_of(forced), 2, "forced node should run twice: {log:?}");
        assert_eq!(count_of(other), 1, "other node should run once: {log:?}");

        let first_forced_run = log.iter().position(|&x| x == forced).unwrap();
        let other_run = log.iter().position(|&x| x == other).unwrap();
        let second_forced_run = log.iter().rposition(|&x| x == forced).unwrap();
        assert!(
            first_forced_run < other_run && other_run < second_forced_run,
            "closing run must come after the other node, which must come after the forced node's first run: {log:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cycle_unblocking_ignores_unrelated_finished_nodes() {
        // 10, 11: independent, zero-dep, will settle before the cycle is
        // ever considered. 20 -> 21 -> 22 -> 20: a genuine 3-cycle,
        // unrelated to 10/11. Once 10/11 finish, unblock_one_cycle_node's
        // NodeFiltered view must still find and break the cycle correctly
        // even though the graph also contains settled, unrelated nodes.
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(16);
        let (stx, srx) = tokio::sync::mpsc::channel::<State<u32>>(16);
        let mut scheduler = Scheduler::new(tx, srx);
        scheduler.add_task(10);
        scheduler.add_task(11);
        scheduler.add_dependency(20, 21);
        scheduler.add_dependency(21, 22);
        scheduler.add_dependency(22, 20);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_executor(4, rx, stx, log.clone());

        tokio::time::timeout(Duration::from_secs(5), scheduler.schedule())
            .await
            .expect("scheduler must not deadlock or stall");

        drop(scheduler);
        handle.await.unwrap();

        let log = log.lock().unwrap();
        // One member of the 3-cycle is forced early and reruns once its
        // own dependency is paid off by the rest of the cycle, so the
        // cycle contributes 4 runs (not 3) on top of 10/11's one run each.
        assert_eq!(log.len(), 6, "one cycle member should rerun to close the cycle: {log:?}");
        let count_of = |t: u32| log.iter().filter(|&&x| x == t).count();
        for t in [10, 11, 20, 21, 22] {
            assert!(count_of(t) >= 1, "task {t} never ran: {log:?}");
        }
        let reran: Vec<u32> = [10, 11, 20, 21, 22]
            .into_iter()
            .filter(|&t| count_of(t) == 2)
            .collect();
        assert_eq!(
            reran.len(),
            1,
            "exactly one task should rerun to close the cycle: {log:?}"
        );
        assert!(
            [20, 21, 22].contains(&reran[0]),
            "only a cycle member should ever rerun, not {}: {log:?}",
            reran[0]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiple_disjoint_cycles_share_one_cached_scc_pass() {
        // Two independent 2-cycles - 0<->1 and 2<->3 - with no edges
        // between them. `schedule` stalls once per cycle, so
        // `unblock_one_cycle_node` runs twice; both calls must be served
        // from the same cached `sccs` (kosaraju runs at most once for the
        // scheduler's whole lifetime - see its doc comment) and still
        // resolve both cycles correctly.
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(16);
        let (stx, srx) = tokio::sync::mpsc::channel::<State<u32>>(16);
        let mut scheduler = Scheduler::new(tx, srx);
        scheduler.add_dependency(0, 1);
        scheduler.add_dependency(1, 0);
        scheduler.add_dependency(2, 3);
        scheduler.add_dependency(3, 2);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_executor(4, rx, stx, log.clone());

        tokio::time::timeout(Duration::from_secs(5), scheduler.schedule())
            .await
            .expect("scheduler must not deadlock on multiple disjoint cycles");

        drop(scheduler);
        handle.await.unwrap();

        let log = log.lock().unwrap();
        let count_of = |t: u32| log.iter().filter(|&&x| x == t).count();
        for t in [0, 1, 2, 3] {
            assert!(count_of(t) >= 1, "task {t} never ran: {log:?}");
        }
        let reran: Vec<u32> = [0, 1, 2, 3]
            .into_iter()
            .filter(|&t| count_of(t) == 2)
            .collect();
        assert_eq!(
            reran.len(),
            2,
            "exactly one member of each cycle should rerun: {log:?}"
        );
        assert_ne!(
            reran.contains(&0),
            reran.contains(&1),
            "exactly one of {{0,1}} should rerun: {log:?}"
        );
        assert_ne!(
            reran.contains(&2),
            reran.contains(&3),
            "exactly one of {{2,3}} should rerun: {log:?}"
        );
    }
}
