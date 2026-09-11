use std::num::NonZeroU32;

use crate::{FormatOrTarget, QueueInfo};
use ftml_uris::ArchiveId;
use leptos::prelude::*;

#[server(prefix = "/api/buildqueue", endpoint = "get_queues")]
pub async fn get_queues() -> Result<Vec<QueueInfo>, ServerFnError<String>> {
    server::get_queues().await
}

#[server(prefix = "/api/buildqueue", endpoint = "run")]
pub async fn run(id: NonZeroU32) -> Result<(), ServerFnError<String>> {
    server::run(id).await
}

#[server(prefix = "/api/buildqueue", endpoint = "requeue")]
pub async fn requeue(id: NonZeroU32) -> Result<(), ServerFnError<String>> {
    server::requeue(id).await
}

#[server(prefix = "/api/buildqueue", endpoint = "enqueue")]
pub async fn enqueue(
    archive: Option<ArchiveId>,
    target: FormatOrTarget,
    path: Option<String>,
    stale_only: Option<bool>,
    queue: Option<NonZeroU32>,
    clean: bool,
) -> Result<usize, ServerFnError<String>> {
    server::enqueue(archive, target, path, stale_only, queue, clean).await
}

#[server(prefix = "/api/buildqueue", endpoint = "log")]
pub async fn get_log(
    queue: NonZeroU32,
    archive: ArchiveId,
    rel_path: String,
    target: String,
) -> Result<either::Either<String, String>, ServerFnError<String>> {
    let is_check = target == ftml_solver::CHECK.name;
    let log = server::get_log(queue, archive, rel_path, target).await?;
    if is_check {
        return Ok(either::Right(log));
        /*return ftml_solver_trace::results::DocumentCheckResult::from_json(&log).map_or_else(
            |err| {
                Ok(either::Left(format!(
                    "Error deserializing type check log: {err}\n\n{log}"
                )))
            },
            |l| Ok(either::Right(l)),
        );*/
    }
    Ok(either::Left(log))
}

#[server(prefix = "/api/buildqueue", endpoint = "migrate")]
pub async fn migrate(queue: NonZeroU32) -> Result<usize, ServerFnError<String>> {
    server::migrate(queue).await
}

#[server(prefix = "/api/buildqueue", endpoint = "delete")]
pub async fn delete(queue: NonZeroU32) -> Result<(), ServerFnError<String>> {
    server::delete(queue).await
}

#[cfg(feature = "ssr")]
pub mod server {
    use std::num::NonZeroU32;

    use crate::{FormatOrTarget, LoginQueue, QueueInfo, RepoInfo};
    use flams_math_archives::backend::SandboxedRepository;
    use flams_math_archives::utils::path_ext::RelPath;
    use flams_router_base::LoginState;
    use flams_system::building::Queue;
    use flams_system::building::queue_manager::QueueManager;
    use flams_web_utils::blocking_server_fn;
    use ftml_uris::ArchiveId;
    use leptos::prelude::*;

    /// #### Errors
    pub(super) async fn get_queues() -> Result<Vec<QueueInfo>, ServerFnError<String>> {
        let login = LoginState::get_server();
        //let oauth = get_oauth().ok();
        blocking_server_fn(move || {
            let ls = match login {
                LoginState::None | LoginState::Loading => {
                    return Err(format!("Not logged in: {login:?}"));
                }
                LoginState::NoAccounts
                | LoginState::Admin
                | LoginState::User { is_admin: true, .. } => QueueManager::get().all_queues(),
                LoginState::User { name, .. } => QueueManager::get().queues_for_user(&name),
            };
            let mut ret = Vec::new();
            for (k, v, d) in ls {
                let archives = d.map(|d| {
                    let mut archives = Vec::new();
                    for ri in d {
                        match ri {
                            SandboxedRepository::Copy(id) => archives.push(RepoInfo::Copy(id)),
                            SandboxedRepository::Git {
                                id,
                                branch,
                                commit,
                                remote,
                            } => {
                                archives.push(RepoInfo::Git {
                                    id,
                                    branch: branch.to_string(),
                                    commit,
                                    remote: remote.to_string(), //,updates
                                });
                            }
                        }
                    }
                    archives
                });

                ret.push(QueueInfo {
                    id: k.into(),
                    name: v.to_string(),
                    archives,
                });
            }
            Ok(ret)
        })
        .await
    }

    /// #### Errors
    pub(super) async fn run(id: NonZeroU32) -> Result<(), ServerFnError<String>> {
        use flams_system::building::queue_manager::QueueManager;
        let login = LoginState::get_server();
        blocking_server_fn(move || {
            login.with_queue(id, |_| ())?;
            QueueManager::get()
                .start_queue(id.into())
                .map_err(|()| "Queue does not exist".to_string())?;
            Ok(())
        })
        .await
    }

    pub(super) async fn requeue(id: NonZeroU32) -> Result<(), ServerFnError<String>> {
        let login = LoginState::get_server();
        blocking_server_fn(move || login.with_queue(id, Queue::requeue_failed)).await
    }

    pub(super) async fn enqueue(
        archive: Option<ArchiveId>,
        target: FormatOrTarget,
        path: Option<String>,
        stale_only: Option<bool>,
        queue: Option<NonZeroU32>,
        clean: bool,
    ) -> Result<usize, ServerFnError<String>> {
        use flams_math_archives::formats::BuildTarget;
        use flams_math_archives::formats::FormatOrTargets;
        use flams_math_archives::formats::SourceFormat;
        use flams_math_archives::manager::ArchiveOrGroup as AoG;

        let login = LoginState::get_server();

        blocking_server_fn(move || {
            login.with_opt_queue(queue, |_, queue| {
                let stale_only = stale_only.unwrap_or(true);

                // only used to have tgts.as_slice() later
                #[allow(clippy::option_if_let_else)]
                let tgts: Vec<_> = match &target {
                    FormatOrTarget::Targets(t) => {
                        let Some(v) = t
                            .iter()
                            .map(|s| BuildTarget::get(s))
                            .collect::<Option<Vec<_>>>()
                        else {
                            return Err("Invalid target".to_string());
                        };
                        v
                    }
                    FormatOrTarget::Format(f) => Vec::new(),
                };

                let fot = match target {
                    FormatOrTarget::Format(f) => FormatOrTargets::Format(
                        SourceFormat::get(&f)
                            .map_or_else(|| Err("Invalid format".to_string()), Ok)?,
                    ),
                    FormatOrTarget::Targets(_) => FormatOrTargets::Targets(tgts.as_slice()),
                };

                let Some(archive) = archive else {
                    return Ok(queue.enqueue_all(fot, stale_only, clean));
                };

                let group = flams_math_archives::backend::GlobalBackend.with_tree(
                    |tree| -> Result<bool, String> {
                        match tree.get_group_or_archive(&archive) {
                            Some(AoG::Archive(_)) => Ok(false),
                            Some(AoG::Group(_)) => Ok(true),
                            None => Err(format!("Archive {archive} not found")),
                        }
                    },
                )?;

                if group && path.is_some() {
                    return Err(
                        "Must specify either an archive with optional path or a group".to_string(),
                    );
                }

                if group {
                    Ok(queue.enqueue_group(&archive, fot, stale_only, clean))
                } else {
                    Ok(queue.enqueue_archive(
                        &archive,
                        fot,
                        stale_only,
                        path.as_deref().map(RelPath::new),
                        clean && path.is_none(),
                    ))
                }
            })?
        })
        .await
    }

    pub(super) async fn get_log(
        queue: NonZeroU32,
        archive: ArchiveId,
        rel_path: String,
        target: String,
    ) -> Result<String, ServerFnError<String>> {
        use flams_math_archives::BuildableArchive;
        use flams_math_archives::backend::LocalBackend;

        let Some(target) = flams_math_archives::formats::BuildTarget::get(&target) else {
            return Err(format!("Target {target} not found").into());
        };
        let login = LoginState::get_server();
        let id = archive.clone();
        let Some(path) = tokio::task::spawn_blocking(move || {
            login.with_queue(queue, |q| {
                q.backend()
                    .with_local_archive(&id, |a| a.map(|a| a.get_log(&rel_path, target)))
            })
        })
        .await
        .map_err(|e| e.to_string())??
        else {
            return Err(format!("Archive {archive} not found").into());
        };
        let v = tokio::fs::read(&path)
            .await
            .map_err(|e| format!("{e}: {}", path.display()))?;
        Ok(String::from_utf8_lossy(&v).to_string())
    }

    pub(super) async fn migrate(queue: NonZeroU32) -> Result<usize, ServerFnError<String>> {
        let login = LoginState::get_server();
        if matches!(login, LoginState::NoAccounts) {
            return Err("Migration only makes sense in public mode"
                .to_string()
                .into());
        }
        //let oauth = get_oauth().ok();
        blocking_server_fn(move || {
            login.with_queue(queue, |_| ())?;
            let ((), n) = flams_system::building::queue_manager::QueueManager::get()
                .migrate::<()>(queue.into(), |_| Ok(()))
                .map_err(|r| format!("{r:#}"))?;
            Ok(n)
        })
        .await
    }

    pub(super) async fn delete(queue: NonZeroU32) -> Result<(), ServerFnError<String>> {
        use flams_system::building::queue_manager::QueueManager;
        let login = LoginState::get_server();
        blocking_server_fn(move || {
            login.with_queue(queue, |_| ())?;
            QueueManager::get().delete(queue.into());
            Ok(())
        })
        .await
    }
}
