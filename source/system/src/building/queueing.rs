use either::Either;
use flams_math_archives::{
    backend::AnyBackend,
    formats::{BuildTargetId, FormatOrTargets},
    source_files::{FileState, SourceFile},
    Archive, MathArchive,
};
use flams_utils::{triomphe::Arc, vecmap::VecSet};
use ftml_ontology::utils::time::Eta;
use ftml_uris::UriWithArchive;
use parking_lot::RwLock;
use petgraph::{
    Direction::{Incoming, Outgoing},
    algo::kosaraju_scc,
    graph::NodeIndex,
};
use std::collections::{HashMap, HashSet, hash_map::Entry};

use crate::building::{AtomicTaskState, QueueMessage};

use super::{
    graph,
    queue::{Queue, QueueState, RunningQueue, TaskMap},
    BuildStep, BuildStepI, BuildTask, BuildTaskId, Dependency, TaskState,
};

impl Queue {
    /// The original weak/strict fixed-point classifier. Superseded by
    /// `sort_graph` (see its doc comment for the behavior differences);
    /// no longer called anywhere, kept around as a reference/rollback
    /// point.
    #[allow(dead_code, clippy::significant_drop_in_scrutinee)]
    pub fn sort(map: &TaskMap, state: &mut RunningQueue) {
        let RunningQueue {
            queue,
            done,
            blocked,
            failed,
            ..
        } = state;
        let mut tasks = map.map.values().cloned().collect::<Vec<_>>();
        let mut weak = true;
        while !tasks.is_empty() {
            let mut changed = false;
            for t in &tasks {
                let mut has_failed = false;
                let Some(step) = t.steps().iter().find(|s| {
                    let state = s.0.state.get();
                    if state == TaskState::Failed {
                        has_failed = true;
                        return false;
                    }
                    !matches!(state, TaskState::Done)
                }) else {
                    if has_failed {
                        failed.push(t.clone());
                    } else {
                        done.push(t.clone());
                    }
                    continue;
                };
                let mut newstate = TaskState::Queued;
                for d in step.0.requires.read().iter() {
                    match d {
                        Dependency::Resolved { task, strict, step } if *strict || weak => {
                            match task
                                .get_step(*step)
                                .unwrap_or_else(|| unreachable!())
                                .0
                                .state
                                .get()
                            {
                                TaskState::Done
                                | TaskState::Queued
                                | TaskState::Failed
                                | TaskState::Running => (),
                                TaskState::Blocked => {
                                    newstate = TaskState::Blocked;
                                }
                                TaskState::None => {
                                    newstate = TaskState::None;
                                    break;
                                }
                            }
                        }
                        // here dependency resolution does not happen what about other deps ?
                        // do small before you dream big
                        // dont try to build steps on air start with foundation
                        // In this case questioning why dep resolution does not happen here ?
                        // Dependency is specified cleary but here only resolved deps are taken in to consideration where does it happen then ?
                        _ => (),
                    }
                }
                let mut found = false;
                if newstate == TaskState::None {
                    continue;
                }
                changed = true;
                for s in t.steps() {
                    if s == step {
                        found = true;
                        s.0.state.set(newstate);
                    } else if found {
                        s.0.state.set(TaskState::Blocked);
                    }
                }
                match newstate {
                    TaskState::Blocked => blocked.push(t.clone()),
                    TaskState::Queued => queue.push_back(t.clone()),
                    _ => (),
                }
            }
            if changed {
                tasks.retain(|t| t.steps().iter().any(|s| s.0.state.get() == TaskState::None));
            } else if weak {
                weak = false;
            } else {
                let tasks = std::mem::take(&mut tasks);
                for t in tasks {
                    for s in t.steps() {
                        s.0.state.set_if_is(TaskState::None, TaskState::Blocked);
                    }
                    blocked.push(t);
                }
            }
        }
    }

    /// Classifies tasks using the SCC-aware dependency graph from
    /// `graph::build_graph` instead of `sort`'s weak/strict fixed-point
    /// scan (kept below as `sort`, now unused - see its doc comment).
    ///
    /// `build_graph` already excludes every step that's `Done`, or that's
    /// `Failed` (or depends - directly, or through the in-task pipeline
    /// order it adds - on a step that's `Failed`), so a step simply being
    /// absent from the graph *is* "this task's pipeline is over, one way or
    /// the other" - no separate `has_failed` bookkeeping needed the way
    /// `sort` needs it. That also means this enforces strict in-task
    /// pipeline order (an earlier failed step blocks every later step of
    /// the same task) where `sort` was more permissive: `sort` only skips
    /// over a `Failed` step when picking which step to evaluate next, so a
    /// later step with no direct `requires` on the failed one could still
    /// run under `sort`. That's a real, deliberate behavior difference,
    /// not an oversight - see `graph::build_graph`'s doc comment.
    ///
    /// Instead of `sort`'s "two passes, then give up and block the whole
    /// remainder", genuine cycles are found with `kosaraju_scc` and the
    /// most-depended-on still-pending member of each is forced ready - the
    /// same heuristic `buildsystem::scheduler::Scheduler::
    /// unblock_one_cycle_node` uses.
    pub fn sort_graph(map: &TaskMap, state: &mut RunningQueue) {
        let RunningQueue {
            queue,
            done,
            blocked,
            failed,
            sccs,
            ..
        } = state;

        let dep_graph = graph::build_graph(map);
        let by_id: HashMap<BuildTaskId, BuildTask> =
            map.map.values().map(|t| (t.get_id(), t.clone())).collect();
        let mut node_of: HashMap<graph::StepId, NodeIndex> = HashMap::new();
        for idx in dep_graph.node_indices() {
            node_of.insert(dep_graph[idx], idx);
        }

        let is_done_step = |tid: BuildTaskId, target: BuildTargetId| {
            by_id
                .get(&tid)
                .and_then(|t| t.get_step(target))
                .is_some_and(|s| s.0.state.get() == TaskState::Done)
        };
        let is_ready = |idx: NodeIndex| {
            dep_graph
                .neighbors_directed(idx, Incoming)
                .all(|dep| {
                    let (tid, target) = dep_graph[dep];
                    is_done_step(tid, target)
                })
        };

        let all_sccs = kosaraju_scc(&dep_graph);
        let mut forced_ready = HashSet::new();
        for scc in &all_sccs {
            if scc.len() < 2 || scc.iter().any(|&n| is_ready(n)) {
                continue;
            }
            if let Some(&n) = scc
                .iter()
                .max_by_key(|&&n| dep_graph.neighbors_directed(n, Outgoing).count())
            {
                let (tid, target) = dep_graph[n];
                if let Some(t) = by_id.get(&tid) {
                    tracing::info!(target:"buildqueue",
                        "sort: breaking cycle (SCC size {}) by force-queuing [{}]{{{}}} :: {target}",
                        scc.len(), t.0.uri.archive_id(), t.0.rel_path);
                }
                forced_ready.insert(n);
            }
        }

        *sccs = Some((
            dep_graph.node_count(),
            all_sccs
                .iter()
                .map(|scc| scc.iter().map(|&n| dep_graph[n]).collect())
                .collect(),
        ));

        for task in map.map.values() {
            let Some(step) = task
                .steps()
                .iter()
                .find(|s| s.0.state.get() != TaskState::Done)
            else {
                done.push(task.clone());
                continue;
            };
            let here = (task.get_id(), step.0.target);
            let newstate = match node_of.get(&here) {
                None => TaskState::Failed, // itself, or a dependency, already failed
                Some(&idx) if is_ready(idx) || forced_ready.contains(&idx) => TaskState::Queued,
                Some(_) => TaskState::Blocked,
            };
            let mut found = false;
            for s in task.steps() {
                if s == step {
                    found = true;
                    s.0.state.set(newstate);
                } else if found {
                    s.0.state.set(TaskState::Blocked);
                }
            }
            match newstate {
                TaskState::Blocked => blocked.push(task.clone()),
                TaskState::Queued => queue.push_back(task.clone()),
                TaskState::Failed => failed.push(task.clone()),
                _ => unreachable!(),
            }
        }
    }

    pub fn get_next(&self) -> Option<(BuildTask, BuildTargetId)> {
        loop {
            match self.get_next_i_graph() {
                Ok(r) => return r,
                Err(()) => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        }
    }

    #[cfg(feature = "tokio")]
    pub async fn get_next_async(&self) -> Option<(BuildTask, BuildTargetId)> {
        loop {
            match self.get_next_i_graph() {
                Ok(r) => return r,
                Err(()) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
            }
        }
    }

    /// The original fallback: once nothing running, nothing in `queue`,
    /// and nothing in `blocked` is immediately runnable, this assumes
    /// that's unrecoverable and fails everything left. Superseded by
    /// `get_next_i_graph` (see its doc comment); no longer called
    /// anywhere, kept around as a reference/rollback point.
    #[allow(dead_code)]
    fn get_next_i(&self) -> Result<Option<(BuildTask, BuildTargetId)>, ()> {
        let mut state = self.0.state.write();
        let QueueState::Running(RunningQueue {
            queue,
            blocked,
            running,
            failed,
            ..
        }) = &mut *state
        else {
            unreachable!()
        };
        if queue.is_empty() && blocked.is_empty() && running.is_empty() {
            return Ok(None);
        }
        if let Some((i, target)) = queue
            .iter()
            .enumerate()
            .find_map(|(next, e)| Self::can_be_next(e).map(|t| (next, t)))
        {
            let Some(task) = queue.remove(i) else {
                unreachable!()
            };
            task.get_step(target)
                .unwrap_or_else(|| unreachable!())
                .0
                .state
                .set(TaskState::Running);
            running.push(task.clone());
            return Ok(Some((task, target)));
        }
        if !running.is_empty() {
            drop(state);
            Err(())
            //std::thread::sleep(std::time::Duration::from_secs(1));
        } else if !blocked.is_empty() {
            if let Some((i, target)) = blocked
                .iter()
                .enumerate()
                .find_map(|(next, e)| Self::can_be_next(e).map(|t| (next, t)))
            {
                let task = blocked.remove(i);
                for s in task.steps() {
                    s.0.state.set_if_is(TaskState::Blocked, TaskState::Queued);
                }
                task.get_step(target)
                    .unwrap_or_else(|| unreachable!())
                    .0
                    .state
                    .set(TaskState::Running);
                running.push(task.clone());
                return Ok(Some((task, target)));
            }
            while let Some(t) = blocked.pop() {
                for s in t.steps() {
                    let state = s.0.state.get();
                    if state != TaskState::Done {
                        s.0.state.set(TaskState::Failed);
                    }
                }
                self.0.sender.lazy_send(|| QueueMessage::TaskFailed {
                    id: t.0.id,
                    target: t.steps().last().expect("???").0.target,
                    eta: Eta::default(),
                });
                failed.push(t);
            }
            Ok(None)
        } else {
            Ok(None)
        }
    }

    /// Identical to `get_next_i` (kept above, now unused) in the
    /// `queue`/`running` fast paths (those aren't about cycle detection at
    /// all - `can_be_next` just avoids racing two steps of the same strict
    /// dependency chain at once). The difference is entirely in the final
    /// fallback: instead of assuming "nothing immediately runnable" is
    /// unrecoverable and failing everything left, this checks first
    /// whether it's actually just an ordinary dependency cycle - in which
    /// case there's a sound way forward: run the most-depended-on member of
    /// the most fundamental unresolved cycle anyway, same heuristic as
    /// `buildsystem::scheduler::Scheduler::unblock_one_cycle_node`. Only
    /// falls through to the original's "fail everything" behavior when
    /// it's genuinely not a cycle.
    fn get_next_i_graph(&self) -> Result<Option<(BuildTask, BuildTargetId)>, ()> {
        let mut state = self.0.state.write();
        let QueueState::Running(RunningQueue {
            queue,
            blocked,
            running,
            failed,
            sccs,
            ..
        }) = &mut *state
        else {
            unreachable!()
        };
        if queue.is_empty() && blocked.is_empty() && running.is_empty() {
            return Ok(None);
        }
        if let Some((i, target)) = queue
            .iter()
            .enumerate()
            .find_map(|(next, e)| Self::can_be_next(e).map(|t| (next, t)))
        {
            let Some(task) = queue.remove(i) else {
                unreachable!()
            };
            task.get_step(target)
                .unwrap_or_else(|| unreachable!())
                .0
                .state
                .set(TaskState::Running);
            tracing::info!(target:"buildqueue","dispatch (ready) [{}]{{{}}} :: {target}",
                task.0.uri.archive_id(), task.0.rel_path);
            running.push(task.clone());
            return Ok(Some((task, target)));
        }
        if !running.is_empty() {
            drop(state);
            return Err(());
        }
        if blocked.is_empty() {
            return Ok(None);
        }
        if let Some((i, target)) = blocked
            .iter()
            .enumerate()
            .find_map(|(next, e)| Self::can_be_next(e).map(|t| (next, t)))
        {
            let task = blocked.remove(i);
            for s in task.steps() {
                s.0.state.set_if_is(TaskState::Blocked, TaskState::Queued);
            }
            task.get_step(target)
                .unwrap_or_else(|| unreachable!())
                .0
                .state
                .set(TaskState::Running);
            tracing::info!(target:"buildqueue","dispatch (unblocked) [{}]{{{}}} :: {target}",
                task.0.uri.archive_id(), task.0.rel_path);
            running.push(task.clone());
            return Ok(Some((task, target)));
        }

        // Genuinely stuck by `can_be_next`'s local check. `build_graph`
        // only ever contains still-pending steps, so any node still
        // present at this point belongs to one of exactly the tasks
        // sitting in `blocked` right now (queue/running are both empty).
        let dep_graph = {
            let map = self.0.map.read();
            graph::build_graph(&map)
        };
        let mut node_of: HashMap<graph::StepId, NodeIndex> = HashMap::new();
        for idx in dep_graph.node_indices() {
            node_of.insert(dep_graph[idx], idx);
        }
        let by_id: HashMap<BuildTaskId, &BuildTask> =
            blocked.iter().map(|t| (t.get_id(), t)).collect();

        // Reuse the cached SCC decomposition (populated here or by
        // `sort_graph`) as long as the graph hasn't grown since it was
        // computed - `kosaraju_scc` is the expensive part of this and
        // should run at most once per stable graph shape, not on every
        // forced-dispatch decision. A shrinking graph (steps completing)
        // never invalidates the cache; only growth (e.g. a fresh
        // `enqueue_archive` on an already-running queue) can introduce
        // cycles the cached decomposition doesn't know about yet.
        let node_count = dep_graph.node_count();
        if !matches!(sccs, Some((n, _)) if *n == node_count) {
            *sccs = Some((
                node_count,
                kosaraju_scc(&dep_graph)
                    .into_iter()
                    .map(|scc| scc.into_iter().map(|n| dep_graph[n]).collect())
                    .collect(),
            ));
        }
        let cached_sccs = &sccs.as_ref().unwrap_or_else(|| unreachable!()).1;

        let forced = cached_sccs.iter().rev().find_map(|scc| {
            if scc.len() < 2 {
                return None;
            }
            scc.iter()
                .filter(|sid| by_id.contains_key(&sid.0))
                .filter_map(|sid| node_of.get(sid).copied())
                .max_by_key(|&n| dep_graph.neighbors_directed(n, Outgoing).count())
        });

        if let Some(node) = forced {
            let (tid, target) = dep_graph[node];
            let i = blocked
                .iter()
                .position(|t| t.get_id() == tid)
                .unwrap_or_else(|| unreachable!());
            let task = blocked.remove(i);
            for s in task.steps() {
                s.0.state.set_if_is(TaskState::Blocked, TaskState::Queued);
            }
            task.get_step(target)
                .unwrap_or_else(|| unreachable!())
                .0
                .state
                .set(TaskState::Running);
            tracing::info!(target:"buildqueue","dispatch (FORCED - breaking cycle) [{}]{{{}}} :: {target}",
                task.0.uri.archive_id(), task.0.rel_path);
            running.push(task.clone());
            return Ok(Some((task, target)));
        }

        // Not a cycle - genuinely unresolvable, same fallback as `get_next_i`.
        while let Some(t) = blocked.pop() {
            for s in t.steps() {
                let st = s.0.state.get();
                if st != TaskState::Done {
                    s.0.state.set(TaskState::Failed);
                }
            }
            self.0.sender.lazy_send(|| QueueMessage::TaskFailed {
                id: t.0.id,
                target: t.steps().last().expect("???").0.target,
                eta: Eta::default(),
            });
            failed.push(t);
        }
        Ok(None)
    }

    // This method is a method which checks for the task state of the requires dependencies of a build tasks build step which is queued or blocked
    // now he checks the dependency state if it is running then he doesnot do anything if none of the deps tasks are running then he selects it
    fn can_be_next(e: &BuildTask) -> Option<BuildTargetId> {
        let step =
            e.0.steps.iter().find(|step| {
                matches!(step.0.state.get(), TaskState::Queued | TaskState::Blocked)
            })?;
        for d in &step.0.requires.read().0 {
            if let Dependency::Resolved { task, step, strict } = d {
                if *strict
                    && task
                        .get_step(*step)
                        .unwrap_or_else(|| unreachable!())
                        .0
                        .state
                        .get()
                        == TaskState::Running
                {
                    return None;
                }
            }
        }
        Some(step.0.target)
    }

    #[deprecated(note = "assumes local archives")]
    pub(super) fn enqueue<'a, I: Iterator<Item = &'a SourceFile>>(
        map: &mut TaskMap,
        backend: &AnyBackend,
        archive: &Archive,
        target: FormatOrTargets,
        stale_only: bool,
        files: I,
    ) -> usize {
        let targets = match target {
            FormatOrTargets::Format(f) => f.targets,
            FormatOrTargets::Targets(t) => t,
        };
        let has_target = |f: &SourceFile, tgt: BuildTargetId| {
            f.target_state
                .iter()
                .find_map(|(k, v)| if *k == tgt { Some(v) } else { None })
                .is_some_and(|t| !stale_only || matches!(t, FileState::Stale(_) | FileState::New))
        };
        let should_queue = |f: &SourceFile| targets.iter().any(|t| has_target(f, *t));
        let mut count = 0;

        for f in files.filter(|f| should_queue(f)) {
            let key = (archive.id().clone(), f.relative_path.clone());
            let task = match map.map.entry(key) {
                Entry::Vacant(e) => {
                    count += 1;
                    let steps = targets
                        .iter()
                        .filter(|t| has_target(f, **t))
                        .map(|t| {
                            BuildStep(Arc::new(BuildStepI {
                                target: *t,
                                state: AtomicTaskState::new(TaskState::None),
                                //yields:RwLock::new(Vec::new()),
                                requires: RwLock::new(VecSet::default()),
                                dependents: RwLock::new(Vec::new()),
                            }))
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice();
                    map.total += steps.len();
                    let id = map.counter;
                    map.counter = map.counter.saturating_add(1);
                    let task = BuildTask::new(
                        BuildTaskId(id),
                        archive.uri().clone(),
                        steps,
                        match archive {
                            Archive::Local(archive) => {
                                Either::Left(archive.source_dir().join(f.relative_path.as_ref()))
                            }
                            Archive::Ext(..) => todo!("foreign archives"),
                        },
                        f.relative_path.clone(),
                    )
                    .expect("this is a bug");
                    e.insert(task.clone());
                    task
                }
                Entry::Occupied(o) => {
                    count += 1;
                    for s in o.get().steps() {
                        s.0.state.set(TaskState::None);
                    }
                    continue;
                }
            };
            let spec = task.as_build_spec(backend);
            if let FormatOrTargets::Format(fmt) = target {
                for (f, d) in (fmt.dependencies)(spec) {
                    if let Some(step) = task.get_step(f) {
                        step.add_dependency(d.into());
                    }
                }
                Self::process_dependencies(&task, map);
            }
        }
        count
    }

    pub fn process_dependencies(task: &BuildTask, map: &mut TaskMap) {
        for s in task.steps() {
            let key = task.as_task_ref(s.0.target);
            if let Some(v) = map.dependents.remove(&key) {
                for (d, i) in v {
                    if let Some(t) = d.get_step(s.0.target) {
                        let mut deps = t.0.requires.write();
                        for d in &mut deps.0 {
                            if let Dependency::Physical {
                                task: ref t,
                                strict,
                            } = d
                            {
                                if *t == key {
                                    *d = Dependency::Resolved {
                                        task: task.clone(),
                                        step: i,
                                        strict: *strict,
                                    };
                                }
                            }
                        }
                    }
                }
            }
            for dep in &mut s.0.requires.write().0 {
                if let Dependency::Physical {
                    task: ref deptask,
                    strict,
                } = dep
                {
                    if deptask.archive == *task.0.uri.archive_id()
                        && deptask.rel_path == task.0.rel_path
                    {
                        continue;
                        // TODO check for more
                    }
                    let key = (deptask.archive.clone(), deptask.rel_path.clone());
                    if let Some(deptasks) = map.map.get(&key) {
                        if let Some(step) =
                            deptasks.steps().iter().find(|bt| bt.0.target == s.0.target)
                        {
                            step.0.dependents.write().push((task.0.id, s.0.target));
                            *dep = Dependency::Resolved {
                                task: deptasks.clone(),
                                step: s.0.target,
                                strict: *strict,
                            };
                            continue;
                        }
                        map.dependents
                            .entry(deptask.clone())
                            .or_default()
                            .push((task.clone(), s.0.target));
                    }
                }
            }
        }
    }
}
