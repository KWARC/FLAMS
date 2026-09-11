use std::{
    hint::unreachable_unchecked,
    num::NonZeroU32,
    path::{Path, PathBuf},
    str::FromStr,
};

use either::Either;
use flams_math_archives::{
    backend::AnyBackend,
    formats::{BuildSpec, BuildTargetId, TaskDependency, TaskRef},
};
use flams_utils::{
    prelude::{TreeChild, TreeLike},
    triomphe::Arc,
    vecmap::{VecMap, VecSet},
};
use ftml_ontology::utils::time::Eta;
use ftml_uris::{ArchiveId, ArchiveUri, DocumentUri, Language, ModuleUri, UriPath, UriWithArchive};
use parking_lot::RwLock;

pub mod graph;
pub mod queue;
pub mod queue_manager;
pub use queue::QueueName;
pub mod queueing;

pub(crate) static BUILD_QUEUE_SPAN: std::sync::LazyLock<tracing::Span> = std::sync::LazyLock::new(
    || tracing::info_span!(target:"build queue",parent:None,"Build Queue"),
);

#[cfg(all(test, feature = "tokio"))]
mod tests;

pub use queue::Queue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum TaskState {
    Running = 0,
    Queued = 1,
    Blocked = 2,
    Done = 3,
    Failed = 4,
    None = 5,
}
// I don't Understand this complicated atomictaskstate?
// read that this is useful for sharing across threads but in this case why ?
#[derive(Debug)]
pub struct AtomicTaskState(std::sync::atomic::AtomicU8);
impl AtomicTaskState {
    pub fn new(state: TaskState) -> Self {
        Self(std::sync::atomic::AtomicU8::new(state as _))
    }
    /// Sets the state to `state`, but only if it's currently `if_is` -
    /// otherwise a no-op. Callers use this to reset *some* of a task's
    /// steps (e.g. "every step that's still Blocked, back to Queued")
    /// without disturbing steps already past that point (Done/Failed/
    /// Running/etc.) - the CAS failing just means this particular step
    /// wasn't the one being targeted, not an error.
    pub fn set_if_is(&self, if_is: TaskState, state: TaskState) {
        let _ = self.0.compare_exchange(
            if_is as _,
            state as _,
            std::sync::atomic::Ordering::Release,
            std::sync::atomic::Ordering::Acquire,
        );
    }
    pub fn get(&self) -> TaskState {
        let b = self.0.load(std::sync::atomic::Ordering::Acquire);
        match b {
            0 => TaskState::Running,
            1 => TaskState::Queued,
            2 => TaskState::Blocked,
            3 => TaskState::Done,
            4 => TaskState::Failed,
            5 => TaskState::None,
            // SAFETY: impossible b< construction
            _ => unsafe { unreachable_unchecked() },
        }
    }
    pub fn set(&self, state: TaskState) {
        self.0
            .store(state as _, std::sync::atomic::Ordering::Release);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dependency {
    Physical {
        task: TaskRef,
        strict: bool,
    },
    Logical {
        uri: ModuleUri,
        strict: bool,
    },
    Resolved {
        task: BuildTask,
        step: BuildTargetId,
        strict: bool,
    },
}
impl From<TaskDependency> for Dependency {
    fn from(value: TaskDependency) -> Self {
        match value {
            TaskDependency::Logical { uri, strict } => Self::Logical { uri, strict },
            TaskDependency::Physical { task, strict } => Self::Physical { task, strict },
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuildTaskId(NonZeroU32);
impl From<BuildTaskId> for u32 {
    #[inline]
    fn from(id: BuildTaskId) -> Self {
        id.0.get()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct BuildTaskI {
    id: BuildTaskId,
    uri: DocumentUri,
    steps: Box<[BuildStep]>,
    source: Either<PathBuf, String>,
    rel_path: UriPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTask(Arc<BuildTaskI>);
impl BuildTask {
    #[inline]
    /// # Errors
    pub fn new(
        id: BuildTaskId,
        archive: ArchiveUri,
        steps: Box<[BuildStep]>,
        source: Either<PathBuf, String>,
        rel_path: UriPath,
    ) -> eyre::Result<Self> {
        let uri = DocumentUri::from_archive_relpath(archive, rel_path.as_ref())
            .map_err(eyre::Report::new)?;
        Ok(Self(Arc::new(BuildTaskI {
            uri,
            id,
            steps,
            source,
            rel_path,
        })))
    }

    #[must_use]
    #[inline]
    pub fn document_uri(&self) -> &DocumentUri {
        &self.0.uri
    }

    #[must_use]
    pub fn as_build_spec<'a>(&'a self, backend: &'a AnyBackend) -> BuildSpec<'a> {
        BuildSpec {
            uri: &self.0.uri,
            source: self.source(),
            backend,
            rel_path: self.rel_path(),
        }
    }

    #[must_use]
    pub fn as_task_ref(&self, target: BuildTargetId) -> TaskRef {
        TaskRef {
            archive: self.0.uri.archive_id().clone(),
            rel_path: self.0.rel_path.clone(),
            target,
        }
    }

    pub fn get_id(&self) -> BuildTaskId {
        self.0.id
    }

    #[inline]
    #[must_use]
    pub fn source(&self) -> Either<&Path, &str> {
        match &self.0.source {
            Either::Left(p) => Either::Left(p),
            Either::Right(s) => Either::Right(s),
        }
    }

    #[inline]
    #[must_use]
    pub fn archive(&self) -> &ArchiveUri {
        self.0.uri.archive_uri()
    }

    #[inline]
    #[must_use]
    pub fn rel_path(&self) -> &UriPath {
        &self.0.rel_path
    }

    #[inline]
    #[must_use]
    pub fn steps(&self) -> &[BuildStep] {
        &self.0.steps
    }

    #[inline]
    #[must_use]
    pub fn get_step(&self, target: BuildTargetId) -> Option<&BuildStep> {
        self.0.steps.iter().find(|s| s.0.target == target)
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn as_message(&self) -> QueueEntry {
        /*let idx = self.steps().iter().enumerate().find(|s|
            matches!(&*s.1.0.state.read(),TaskState::Running | TaskState::Queued | TaskState::Blocked | TaskState::Failed)
        );
        let idx = if let Some((idx,_)) = idx {(idx - 1) as u8} else {self.steps().len() as u8};
        */
        QueueEntry {
            id: self.0.id,
            archive: self.0.uri.archive_id().clone(),
            rel_path: self.0.rel_path.clone(),
            steps: self
                .steps()
                .iter()
                .map(|s| (s.0.target, s.0.state.get()))
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct BuildStepI {
    //task:std::sync::Weak<BuildTaskI>,
    pub target: BuildTargetId,
    pub state: AtomicTaskState,
    //yields:RwLock<Vec<ModuleUri>>,
    pub requires: RwLock<VecSet<Dependency>>,
    pub dependents: RwLock<Vec<(BuildTaskId, BuildTargetId)>>,
}
impl PartialEq for BuildStepI {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target
    }
}

impl Eq for BuildStepI {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStep(Arc<BuildStepI>);
impl BuildStep {
    pub fn add_dependency(&self, dep: Dependency) {
        self.0.requires.write().insert(dep);
    }
    /*
    #[must_use]
    pub fn get_task(&self) -> BuildTask {
        BuildTask(self.0.task.upgrade().unwrap_or_else(|| unreachable!()))
    }
    */
}

impl std::ops::Deref for BuildStep {
    type Target = BuildStepI;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
/*
pub trait BuildArtifact: Any + 'static {
    fn get_type_id() -> BuildArtifactTypeId
    where
        Self: Sized;
    /// #### Errors
    fn load(p: &Path) -> Result<Self, std::io::Error>
    where
        Self: Sized;
    fn get_type(&self) -> BuildArtifactTypeId;
    /// ### Errors
    fn write(&self, path: &Path) -> Result<(), std::io::Error>;
    fn as_any(&self) -> &dyn Any;
}

pub enum BuildResultArtifact {
    File(BuildArtifactTypeId, PathBuf),
    Data(Box<dyn BuildArtifact>),
    None,
}

pub struct BuildResult {
    pub log: Either<String, PathBuf>,
    pub result: Result<BuildResultArtifact, Vec<Dependency>>,
}
impl BuildResult {
    #[must_use]
    #[inline]
    pub const fn empty() -> Self {
        Self {
            log: Either::Left(String::new()),
            result: Ok(BuildResultArtifact::None),
        }
    }
    #[must_use]
    #[inline]
    pub const fn err() -> Self {
        Self {
            log: Either::Left(String::new()),
            result: Err(Vec::new()),
        }
    }

    #[inline]
    pub fn with_err(s: String) -> Self {
        Self {
            log: Either::Left(s),
            result: Err(Vec::new()),
        }
    }
}

*/

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub id: BuildTaskId,
    pub archive: ArchiveId,
    pub rel_path: UriPath,
    pub steps: VecMap<BuildTargetId, TaskState>,
}

#[derive(Debug, Clone)]
pub enum QueueMessage {
    Idle(Vec<QueueEntry>),
    Started {
        running: Vec<QueueEntry>,
        queue: Vec<QueueEntry>,
        blocked: Vec<QueueEntry>,
        failed: Vec<QueueEntry>,
        done: Vec<QueueEntry>,
    },
    Finished {
        failed: Vec<QueueEntry>,
        done: Vec<QueueEntry>,
    },
    TaskStarted {
        id: BuildTaskId,
        target: BuildTargetId,
    },
    TaskSuccess {
        id: BuildTaskId,
        target: BuildTargetId,
        eta: Eta,
    },
    TaskFailed {
        id: BuildTaskId,
        target: BuildTargetId,
        eta: Eta,
    },
    TaskBlocked {
        id: BuildTaskId,
        target: BuildTargetId,
        eta: Eta,
    },
}
