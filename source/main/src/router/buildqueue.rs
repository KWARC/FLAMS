use std::num::NonZeroU32;

use flams_ontology::uris::ArchiveId;
use flams_utils::{time::{Delta, Eta}, vecmap::VecMap};
use flams_web_utils::inject_css;
use leptos::{either::EitherOf4, prelude::*};
use leptos_router::hooks::use_params_map;

use crate::{users::LoginState, utils::{from_server_clone, from_server_copy, ws::WebSocket}};

#[derive(Debug,Clone,serde::Serialize,serde::Deserialize)]
pub struct QueueInfo {
  pub id:NonZeroU32,
  pub name:String,
  pub archives:Option<Vec<RepoInfo>>
}
#[derive(Debug,Clone,serde::Serialize,serde::Deserialize)]
pub enum RepoInfo {
  Copy(ArchiveId),
  Git{
    id:ArchiveId,
    remote:String,
    branch:String,
    commit:flams_git::Commit,
    //updates:Vec<(String,flams_git::Commit)>
  }
}

#[derive(Debug,Clone,serde::Serialize,serde::Deserialize)]
pub enum FormatOrTarget {
  Format(String),
  Targets(Vec<String>)
}

#[cfg(feature="ssr")]
impl LoginState {
  pub(crate) fn with_queue<R>(&self,id:NonZeroU32,f:impl FnOnce(&flams_system::building::Queue) -> R) -> Result<R,String> {
    use flams_system::building::QueueName;
    let qm = flams_system::building::queue_manager::QueueManager::get();
    match self {
      LoginState::None | LoginState::Loading => return Err(format!("Not logged in: {self:?}")),
      LoginState::Admin | LoginState::NoAccounts | LoginState::User{is_admin:true,..} => (),
      LoginState::User{name,..} => {
        return qm.with_queue(id.into(), move |q| {
          if let Some(q) = q {
            if matches!(q.name(),QueueName::Sandbox{name:qname,..} if &**qname == name) {
              Ok(f(q))
            } else {
              Err(format!("Not allowed to run queue {id}"))
            }
          } else { Err(format!("Queue {id} not found")) }
        });
      }
    }
    qm.with_queue(id.into(),move |q|
      if let Some(q) = q {
        Ok(f(q))
      } else { Err(format!("Queue {id} not found")) }
    )
  }

  pub(crate) fn with_opt_queue<R>(&self,id:Option<NonZeroU32>,f:impl FnOnce(flams_system::building::queue_manager::QueueId,&flams_system::building::Queue) -> R) -> Result<R,String> {
    use flams_system::building::QueueName;
    let qm = flams_system::building::queue_manager::QueueManager::get();
    match (self,id) {
      (LoginState::None | LoginState::Loading,_) => return Err(format!("Not logged in: {self:?}")),
      (LoginState::User{name,..},Some(id)) => {
        qm.with_queue(id.into(), move |q| {
          if let Some(q) = q {
            if matches!(q.name(),QueueName::Sandbox{name:qname,..} if &**qname == name) {
              Ok(f(id.into(),q))
            } else {
              Err(format!("Not allowed to run queue {id}"))
            }
          } else { Err(format!("Queue {id} not found")) }
        })
      }
      (LoginState::Admin,Some(id)) => {
        qm.with_queue(id.into(),|q| {
          if let Some(q) = q {
            Ok(f(id.into(),q))
          } else {
            Err(format!("Queue {id} not found"))
          }
        })
      }
      (LoginState::User{name,..},_) => {
        let queue = qm.new_queue(name);
        qm.with_queue(queue,|q| {
          let Some(q) = q else {unreachable!()};
          Ok(f(queue,q))
        })
      }
      (LoginState::Admin,_) => {
        let queue = qm.new_queue("admin");
        qm.with_queue(queue,|q| {
          let Some(q) = q else {unreachable!()};
          Ok(f(queue,q))
        })
      }
      (LoginState::NoAccounts,_) => {
        qm.with_global(|q| Ok(f(flams_system::building::queue_manager::QueueId::global(),q)))
      }
    }
  }
}


#[server(
  prefix="/api/buildqueue",
  endpoint="get_queues",
)]
#[allow(clippy::unused_async)]
pub async fn get_queues() -> Result<Vec<QueueInfo>,ServerFnError<String>> {
  use flams_system::building::queue_manager::QueueManager;
  use flams_system::backend::{AnyBackend,SandboxedRepository};
  let login = LoginState::get_server();
  let oauth = super::git::get_oauth().ok();
  tokio::task::spawn_blocking(move || {
    let ls = match login {
      LoginState::None | LoginState::Loading => return Err(format!("Not logged in: {login:?}").into()),
      LoginState::NoAccounts | LoginState::Admin | LoginState::User{is_admin:true,..} =>
        QueueManager::get().all_queues(),
      LoginState::User{name,..} =>
        QueueManager::get().queues_for_user(&name)
    };
    let mut ret = Vec::new();
    for (k,v,d) in ls {
      let archives = if let Some(d) = d {
        let mut archives = Vec::new();
        for ri in d { match ri {
          SandboxedRepository::Copy(id) => archives.push(RepoInfo::Copy(id)),
          SandboxedRepository::Git { id, branch, commit, remote } => {
            /*let updates = if let Some((oauth,secret)) = &oauth {
              let gitlab_url = &**flams_system::settings::Settings::get().gitlab_url.as_ref().unwrap_or_else(|| unreachable!());
              let Some(path) = QueueManager::get().with_queue(k, |q| {
                let q = q?;
                let AnyBackend::Sandbox(be) = q.backend() else {return None};
                Some(be.path_for(&id))
              }) else {continue};
              flams_git::repos::GitRepo::open(path).ok().and_then(|git| {
                git.get_origin_url().ok().and_then(|url| {
                  if url.starts_with(gitlab_url) {
                    git.get_new_commits_with_oauth(secret).ok()
                  } else { None }
                })
              }).unwrap_or_default()
            } else { Vec::new()};*/
            archives.push(RepoInfo::Git { 
              id,branch:branch.to_string(),commit,remote:remote.to_string()//,updates
            })
          }
        }}
        Some(archives)
      } else { None };

      ret.push(QueueInfo {
        id:k.into(),name:v.to_string(),archives
      })
    }
    Ok(ret)
  }).await.unwrap_or_else(|e| Err(e.to_string().into()))
}

#[server(
  prefix="/api/buildqueue",
  endpoint="run",
)]
#[allow(clippy::unused_async)]
pub async fn run(id:NonZeroU32) -> Result<(),ServerFnError<String>> {
  use flams_system::building::{queue_manager::QueueManager,QueueName};
  let login = LoginState::get_server();
  tokio::task::spawn_blocking(move || {
    login.with_queue(id,|_| ())?;
    QueueManager::get().start_queue(id.into()).map_err(|()| "Queue does not exist".to_string())?;
    Ok(())
  }).await.map_err(|e| e.to_string())?
}

#[server(
  prefix="/api/buildqueue",
  endpoint="requeue",
)]
#[allow(clippy::unused_async)]
pub async fn requeue(id:NonZeroU32) -> Result<(),ServerFnError<String>> {
  use flams_system::building::{queue_manager::QueueManager,QueueName};
  let login = LoginState::get_server();
  tokio::task::spawn_blocking(move || {
    login.with_queue(id,|q| q.requeue_failed())
  }).await.map_err(|e| e.to_string())?.map_err(Into::into)
}

#[server(
  prefix="/api/buildqueue",
  endpoint="enqueue"
)]
#[allow(clippy::unused_async)]
pub async fn enqueue(archive:ArchiveId,
  target:FormatOrTarget,
  path:Option<String>,stale_only:Option<bool>,
  queue:Option<NonZeroU32>
) -> Result<usize,ServerFnError<String>> {
  use flams_system::{formats::FormatOrTargets,building::queue_manager::QueueManager};
  use flams_system::backend::archives::ArchiveOrGroup as AoG;
  use flams_system::formats::{SourceFormat,BuildTarget};

  let login = LoginState::get_server();

  tokio::task::spawn_blocking(move || 
    login.with_opt_queue::<Result<_,ServerFnError<String>>>(queue, |id,queue| {
      let stale_only = stale_only.unwrap_or(true);

      #[allow(clippy::option_if_let_else)]
      let tgts: Vec<_> = match &target {
        FormatOrTarget::Targets(t) => {
          let Some(v) = t.iter().map(|s| BuildTarget::get_from_str(s)).collect::<Option<Vec<_>>>() else {
            return Err(ServerFnError::MissingArg("Invalid target".into()))
          };
          v
        }
        FormatOrTarget::Format(_) => Vec::new()
      };

      let fot = match target {
        FormatOrTarget::Format(f) => FormatOrTargets::Format(
          SourceFormat::get_from_str(&f).map_or_else(
            || Err(ServerFnError::MissingArg("Invalid format".into())),
            Ok
          )?
        ),
        FormatOrTarget::Targets(_) => FormatOrTargets::Targets(tgts.as_slice())
      };

      let group = flams_system::backend::GlobalBackend::get().with_archive_tree(|tree| -> Result<bool,ServerFnError<String>>
        {match tree.find(&archive) {
          Some(AoG::Archive(_)) => Ok(false),
          Some(AoG::Group(_)) => Ok(true),
          None => Err(format!("Archive {archive} not found").into()),
        }}
      )?;

      if group && path.is_some() {
        return Err(ServerFnError::MissingArg("Must specify either an archive with optional path or a group".into())) 
      }

      if group { 
        Ok(queue.enqueue_group(&archive, fot, stale_only))
      } else {
        Ok(queue.enqueue_archive(&archive, fot, stale_only,path.as_deref()))
      }
    })
  ).await.map_err(|e| e.to_string())??
}

#[server(
  prefix="/api/buildqueue",
  endpoint="log",
)]
#[allow(clippy::unused_async)]
pub async fn get_log(queue:NonZeroU32,archive:ArchiveId,rel_path:String,target:String) -> Result<String,ServerFnError<String>> {
  use crate::users::LoginState;
  use std::path::PathBuf;
  use flams_system::backend::{Backend,GlobalBackend};
  use flams_system::{formats::FormatOrTargets,building::{QueueName,queue_manager::QueueManager}};
  
  let Some(target) = flams_system::formats::BuildTarget::get_from_str(&target) else {
    return Err(format!("Target {target} not found").into())
  };
  let login = LoginState::get_server();
  let id = archive.clone();
  let Some(path) = tokio::task::spawn_blocking(move ||
    login.with_queue(queue, |q|
      q.backend().with_archive(&id, |a| 
        a.map(|a| a.get_log(&rel_path,target))
      )
    )
  ).await.map_err(|e| e.to_string())?? else {
    return Err(format!("Archive {archive} not found").into())
  };
  let v = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
  Ok(String::from_utf8_lossy(&v).to_string())
}


#[server(
  prefix="/api/buildqueue",
  endpoint="migrate"
)]
#[allow(clippy::unused_async)]
pub async fn migrate(queue:NonZeroU32) -> Result<usize,ServerFnError<String>> {
  use flams_system::building::{queue_manager::QueueManager,QueueName};
  use flams_system::backend::{Backend,SandboxedRepository,archives::Archive};
  let login = LoginState::get_server();
  if matches!(login,LoginState::NoAccounts) {
    return Err("Migration only makes sense in public mode".to_string().into())
  }
  let oauth = super::git::get_oauth().ok();
  tokio::task::spawn_blocking(move || {
    login.with_queue(queue, |_| ())?;
    let (_,n) = flams_system::building::queue_manager::QueueManager::get().migrate::<(),String>(queue.into(),|sandbox| {
      if let Some((_,secret)) = oauth {
        //let mut js = tokio::task::JoinSet::new();
        sandbox.with_repos(|repos| {
          for r in repos {
            if let SandboxedRepository::Git { id,.. } = r {
              sandbox.with_archive(id, |a| {
                /*let Some(Archive::Local(a)) = a else { return };
                let p = a.path().to_path_buf();
                let secret = secret.clone();
                let _ = js.spawn_blocking(move || {
                  let repo = flams_git::repos::GitRepo::open(&p)?;
                  repo.add_dir(&p,false)?;
                  repo.add_dir(&p.join(".flams"),true)?;
                  let _ = repo.commit_all("migrating")?;
                  repo.push_with_oauth(&secret)
                  //repo.mark_managed().map_err(|e| e.to_string())?;
                });*///.map_err(|e| e.to_string())?;.map_err(|e| e.to_string())?;.map_err(|e| e.to_string())?;
                //.map_err(|e| e.to_string())?;
                //.map_err(|e| e.to_string())?;
                //Ok(())
              });
              }
            }
        });
        /*while !js.is_empty() {
          if let Some(r) = js.try_join_next() {
            r.map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
          } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
          };
        }*/
        Ok(())
      } else { Ok(()) }
    })?;
    Ok(n)
  }).await.unwrap_or_else(|e| Err(e.to_string().into()))
}

#[server(
  prefix="/api/buildqueue",
  endpoint="delete"
)]
#[allow(clippy::unused_async)]
pub async fn delete(queue:NonZeroU32) -> Result<(),ServerFnError<String>> {
  use flams_system::building::{queue_manager::QueueManager,QueueName};
  let login = LoginState::get_server();
  tokio::task::spawn_blocking(move || {
    login.with_queue(queue, |_| ())?;
    QueueManager::get().delete(queue.into());
    Ok(())
  }).await.unwrap_or_else(|e| Err(e.to_string().into()))
}

#[derive(Copy,Clone)]
struct UpdateQueues(RwSignal<()>);

#[component]
pub fn QueuesTop() -> impl IntoView {
  use thaw::{TabList,Tab,Divider,Layout};
  use flams_web_utils::components::Spinner;

  let update = UpdateQueues(RwSignal::new(()));
  provide_context(update);
  move || {
    let _ = update.0.get();
    let params = use_params_map();
    let id = move || params.read().get("queue");

    from_server_copy(true,get_queues,move |v| {
      if v.is_empty() {
        return leptos::either::Either::Left(view!(<div>"(No running queues)"</div>))
      }
      let queues = AllQueues::new(v);
      if let Some(id) = id() {
        if let Ok(id) = id.parse() {
          queues.selected.update_untracked(|v| *v = id);
        }
      }
      provide_context(queues);
      let selected_value = RwSignal::new(queues.selected.get_untracked().to_string());
      let _ = Effect::new(move |_| {
        let value = selected_value.get();
        let selected = queues.selected.get_untracked();
        let value = value.parse().unwrap_or_else(|_| unreachable!());
        if selected != value {
          queues.selected.set(value);
        }
      });
      inject_css("flams-fullscreen", ".flams-fullscreen { width:100%; height:calc(100% - 44px - 21px) }");
      leptos::either::Either::Right(view!{
        <TabList selected_value>
          <For each=move || queues.queues.get() key=|e| e.0 children=move |(i,_)| view!{
            <Tab value=i.to_string()>{
              queues.queue_names.get().get(&i).unwrap_or_else(|| unreachable!()).clone()
            }</Tab>
          }/>
        </TabList>
        <div style="margin:10px"><Divider/></div>
        <Layout class="flams-fullscreen">{move || {
          let curr = queues.selected.get();
          queues.show.update_untracked(|v| *v = false);
          QueueSocket::run(queues);
          move || view! {
            <Show when=move || queues.show.get() fallback=|| view!(<Spinner/>)>{
              let ls = *queues.queues.get_untracked().get(&curr).unwrap_or_else(|| unreachable!());
              move || match ls.get() {
                QueueData::Idle(v) => {
                    EitherOf4::A(idle(curr,v))
                },
                QueueData::Running(r) => {
                    EitherOf4::B(running(curr,r))
                },
                QueueData::Finished(failed,done) => EitherOf4::C(finished(curr,failed,done)),
                QueueData::Empty => EitherOf4::D(view!(<div>"Other"</div>))
              }
            }</Show>
          }
        }}</Layout>
      })
    })
  }
}

fn repos(queue_id:NonZeroU32,allowed:bool) -> impl IntoView {
  use flams_web_utils::components::{Collapsible,Header};
  use thaw::{Caption1Strong,Table,TableBody,TableHeader,TableRow,TableHeaderCell,TableCell,TableCellLayout};
  if matches!(LoginState::get(),LoginState::NoAccounts) { return None }
  let queues : AllQueues = expect_context();
  let repos = queues.queue_repos.with_untracked(|v| v.get(&queue_id).cloned()).flatten();
  let Some(repos) = repos else { return None };
  if repos.is_empty() { return None }
  let style = if allowed { "" } else { "color:gray;" };
  inject_css("flams-repo-table", include_str!("repo-table.css"));
  Some(view!{<div style="margin-left:45px;width:fit-content;"><Collapsible>
    <Header slot><Caption1Strong>"Archives"</Caption1Strong></Header>
    <Table class="flams-repo-table">
      <TableHeader><TableRow>
        <TableHeaderCell><Caption1Strong>"Archive"</Caption1Strong></TableHeaderCell>
        <TableHeaderCell><Caption1Strong>"Branch"</Caption1Strong></TableHeaderCell>
        <TableHeaderCell><Caption1Strong>"Commit"</Caption1Strong></TableHeaderCell>
      </TableRow></TableHeader>
      <TableBody>{
        repos.into_iter().map(|d| match d {
          RepoInfo::Copy(id) => leptos::either::Either::Left(view!{
            <TableRow>
              <TableCell><TableCellLayout><span style=style>{id.to_string()}</span></TableCellLayout></TableCell>
              <TableCell><TableCellLayout>"(Copied from MathHub)"</TableCellLayout></TableCell>
            </TableRow>
          }),
          RepoInfo::Git{id,branch,commit,remote/*,updates */} => leptos::either::Either::Right({
            let updates = RwSignal::<Option<Vec<(String,flams_git::Commit)>>>::new(None);
            let style = move || if allowed {
              updates.with(|updates| {
                if let Some(updates) = updates {
                  if updates.is_empty() {
                    "color:green;"
                  } else {
                    "color:yellowgreen;"
                  }
                } else {""}
              })
            } else {style};
            let idstr = id.to_string();
            view!{
              <TableRow>
                <TableCell><TableCellLayout><span style=style>{idstr}</span></TableCellLayout></TableCell>
                <TableCell><TableCellLayout>{branch}</TableCellLayout></TableCell>
                <TableCell><TableCellLayout>
                  {commit.id[..8].to_string()}" at "{commit.created_at.to_string()}" by "{commit.author_name}
                  {if allowed {Some(move || updates.with(|up| {
                    let Some(up) = up else {
                      let aid = id.clone();
                      let toaster = thaw::ToasterInjection::expect_context();
                      let get_updates = Action::new(move |()| {
                        let id = aid.clone();
                        async move {
                          match super::git::get_new_commits(Some(queue_id),id).await {
                            Ok(v) => updates.set(Some(v)),
                            Err(e) => flams_web_utils::components::error_with_toaster(e, toaster),
                          }
                        }
                      });
                      return leptos::either::EitherOf3::A(view!{
                        <button on:click=move |_| {get_updates.dispatch(());}>"Check for updates"</button>
                      });
                    };
                    if up.is_empty() {
                      return leptos::either::EitherOf3::B(view!(<span style=style>" (already up-to-date)"</span>))
                    }
                    let updates = up.clone();

                    leptos::either::EitherOf3::C({
                      use thaw::{Button,ButtonSize,Combobox,ComboboxOption,ToasterInjection};
                      let first = updates.first().map(|(name,_)| name.clone()).unwrap_or_default();
                      let branch = RwSignal::new(first.clone());
                      let _ = Effect::new(move || if branch.with(|s| s.is_empty()) {
                        branch.set(first.clone());
                      });
                      let update : UpdateQueues = expect_context();
                      let toaster = ToasterInjection::expect_context();
                      let commit_map:VecMap<_,_> = updates.clone().into();
                      let archive = id.clone();
                      let remote = remote.clone();
                      let act = flams_web_utils::components::message_action(
                        move |()| super::git::update_from_branch(Some(queue_id),archive.clone(),remote.clone(),branch.get_untracked()),
                        move |(i,_)| {
                          update.0.set(());
                          format!("{i} jobs queued")
                        }
                      );
                      view!{
                        <span style="color:green">
                          " Updates available: "
                        </span>
                        <div style="margin-left:10px">
                          <Button size=ButtonSize::Small on_click=move |_| {act.dispatch(());}>"Update"</Button>
                          " from branch: "
                          <div style="display:inline-block;"><Combobox selected_options=branch>{
                            updates.into_iter().map(|(name,commit)| {let vname = name.clone(); view!{
                              <ComboboxOption text=vname.clone() value=vname>
                                {name}<span style="font-size:x-small">" (Last commit "{commit.id[..8].to_string()}" at "{commit.created_at.to_string()}" by "{commit.author_name}")"</span>
                              </ComboboxOption>
                            }}).collect_view()
                          }</Combobox></div>
                        </div>
                      }
                    })
                  }))} else {None}}
                </TableCellLayout></TableCell>
              </TableRow>
            }
          }),
        }).collect_view()
      }</TableBody>
    </Table>
  </Collapsible></div>})
}

fn delete_action(id:NonZeroU32) -> Action<(),()> {
  use thaw::ToasterInjection;
  let update : UpdateQueues = expect_context();
  let toaster = ToasterInjection::expect_context();
  Action::new(move |()| async move {
    match delete(id).await {
      Ok(()) => update.0.set(()),
      Err(e) => 
        flams_web_utils::components::error_with_toaster(e.to_string(), toaster),
    }
  })
}

fn idle(id:NonZeroU32,ls:RwSignal<Vec<Entry>>) -> impl IntoView {
  use thaw::Button;
  let act = Action::<(),Result<(),ServerFnError<String>>>::new(move |()| run(id));
  let del = delete_action(id);
  view!{
    <div style="width:100%"><div style="position:fixed;right:20px">
        <Button on_click=move |_| {act.dispatch(());}>"Run"</Button>
        <Button on_click=move |_| {del.dispatch(());}>"Delete"</Button>
    </div></div>
    {repos(id,true)}
    <ol reversed style="margin-left:30px">
      <For each=move || ls.get() key=|e| e.id children=|e| e.as_view()/>
    </ol>
  }
}

fn running(id:NonZeroU32,queue:RunningQueue) -> impl IntoView {
  use flams_web_utils::components::{AnchorLink,Anchor,Header};
  use thaw::{Layout,Button};
  let del = delete_action(id);
  let RunningQueue {running,queue,blocked,failed,done,eta} = queue;
  view!{
    <div style="position:fixed;right:20px;z-index:5"><Anchor>
        <AnchorLink href="#running"><Header slot>"Running"</Header></AnchorLink>
        <AnchorLink href="#queued"><Header slot>"Queued"</Header></AnchorLink>
        <AnchorLink href="#blocked"><Header slot>"Blocked"</Header></AnchorLink>
        <AnchorLink href="#failed"><Header slot>"Failed"</Header></AnchorLink>
        <AnchorLink href="#finished"><Header slot>"Finished"</Header></AnchorLink>
    </Anchor></div>
    {repos(id,false)}
    <Layout content_style="text-align:left;">
        {eta.into_view()}
        <div style="width:100%"><div style="position:fixed;right:20px">
            <Button on_click=move |_| {del.dispatch(());}>"Abort and Delete"</Button>
        </div></div>
        <h3 id="running">"Running ("{move || running.with(Vec::len)}")"</h3>
        <ul style="margin-left:30px"><For each=move || running.get() key=|e| e.id children=|e| e.as_view()/></ul>
        <h3 id="queued">"Queued ("{move || queue.with(Vec::len)}")"</h3>
        <ul style="margin-left:30px"><For each=move || queue.get() key=|e| e.id children=|e| e.as_view()/></ul>
        <h3 id="blocked">"Blocked ("{move || blocked.with(Vec::len)}")"</h3>
        <ul style="margin-left:30px"><For each=move || blocked.get() key=|e| e.id children=|e| e.as_view()/></ul>
        <h3 id="failed">"Failed ("{move || failed.with(Vec::len)}")"</h3>
        <ul style="margin-left:30px"><For each=move || failed.get() key=|e| e.id children=|e| e.as_view()/></ul>
        <h3 id="finished">"Finished ("{move || done.with(Vec::len)}")"</h3>
        <ul style="margin-left:30px"><For each=move || done.get() key=|e| e.id children=|e| e.as_view()/></ul>
    </Layout>
  }
}

fn finished(id:NonZeroU32,failed:Vec<Entry>,done:Vec<Entry>) -> impl IntoView {
  use flams_web_utils::components::{AnchorLink,Anchor,Header};
  use thaw::{Button,Layout};
  let requeue = Action::new(move |()| requeue(id));
  let num_failed = failed.len();
  let num_done = done.len(); 
  let del = delete_action(id);
  view!{
    <div style="width:100%"><div style="position:fixed;right:120px;z-index:10">
        {if num_failed > 0 {Some(view!(
          <Button on_click=move |_| {requeue.dispatch(());}>"Requeue Failed"</Button>
        ))} else { None }}
        {migrate_button(id,num_failed)}
        <Button on_click=move |_| {del.dispatch(());}>"Delete"</Button>
    </div></div>
    <div style="position:fixed;right:20px;z-index:5"><Anchor>
        <AnchorLink href="#failed"><Header slot>"Failed"</Header></AnchorLink>
        <AnchorLink href="#finished"><Header slot>"Finished"</Header></AnchorLink>
    </Anchor></div>
    {repos(id,true)}
    <Layout content_style="text-align:left;">
        <h3 id="failed">"Failed ("{num_failed}")"</h3>
        <ul style="margin-left:30px">{
          failed.iter().map(Entry::as_view).collect_view()
        }</ul>
        <h3 id="finished">"Finished ("{num_done}")"</h3>
        <ul style="margin-left:30px">{
          done.iter().map(Entry::as_view).collect_view()
        }</ul>
    </Layout>
  }
}

fn migrate_button(id:NonZeroU32,num_failed:usize) -> impl IntoView {
  use leptos::either::EitherOf3;
  use thaw::{Button,Dialog,DialogSurface,DialogBody,DialogContent,Caption1Strong,Divider};
  if matches!(LoginState::get(),LoginState::NoAccounts) { return EitherOf3::A(()) }
  let update : UpdateQueues = expect_context();
  let migrate = flams_web_utils::components::message_action(
    move |()| migrate(id), 
    move |i| {
      update.0.set(());
      format!("{i} archives migrated")
    }
  );
  if num_failed == 0 { EitherOf3::B(view!{
    <Button on_click=move |_| {migrate.dispatch(());}>"Migrate"</Button>
  })} else {
    let clicked = RwSignal::new(false);
    EitherOf3::C(view!{
      <Button on_click=move |_| {clicked.set(true);}>"Migrate"</Button>
      <Dialog open=clicked><DialogSurface><DialogBody><DialogContent>
        <Caption1Strong><span style="color:red">WARNING</span></Caption1Strong>
        <Divider/>
        <p>{num_failed}" jobs have failed to build!"<br/>"Migrate anyway?"</p>
        <div>
          <div style="width:min-content;margin-left:auto;">
            <Button on_click=move |_| {migrate.dispatch(());}>"Force Migration"</Button>
          </div>
        </div>
      </DialogContent></DialogBody></DialogSurface></Dialog>
    })
  }
}

#[derive(Clone)]
pub struct QueueSocket {
  #[cfg(feature="ssr")]
  #[cfg_attr(docsrs, doc(cfg(feature = "ssr")))]
  listener: Option<flams_utils::change_listener::ChangeListener<flams_system::building::QueueMessage>>,
  #[cfg(all(not(doc),feature="hydrate"))]
  socket: leptos::web_sys::WebSocket,
  #[cfg(doc)]
  socket: (),
  #[cfg(feature="hydrate")]
  _running:RwSignal<bool>
}
impl WebSocket<NonZeroU32,QueueMessage> for QueueSocket {
  const SERVER_ENDPOINT: &'static str = "/ws/queue";
}

#[cfg(feature="ssr")]
#[cfg_attr(docsrs, doc(cfg(feature = "ssr")))]
#[async_trait::async_trait]
impl crate::utils::ws::WebSocketServer<NonZeroU32,QueueMessage> for QueueSocket {
    async fn new(account:crate::users::LoginState,_db:crate::server::db::DBBackend) -> Option<Self> {
        use crate::users::LoginState;
        match account {
            LoginState::Admin | LoginState::NoAccounts | LoginState::User{is_admin:true,..} => {
                let listener = None;//flams_system::logger().listener();
                Some(Self {
                    listener,
                    #[cfg(feature="hydrate")] _running:RwSignal::new(false),
                    #[cfg(feature="hydrate")] socket:unreachable!()
                })
            }
            _ => None
        }
    }
    async fn next(&mut self) -> Option<QueueMessage> {
      loop {
        match &mut self.listener {
          None => tokio::time::sleep(tokio::time::Duration::from_secs_f32(0.5)).await,
          Some(l) => return l.read().await.map(Into::into),
        }
      }
    }
    async fn handle_message(&mut self,msg:NonZeroU32) -> Option<QueueMessage> {
      let (lst,msg) = flams_system::building::queue_manager::QueueManager::get()
        .with_queue(msg.into(), |q| 
          q.map(|q| (q.listener(),q.state_message()))
      )?;
      self.listener = Some(lst);
      Some(msg.into())
    }
    async fn on_start(&mut self,_:&mut axum::extract::ws::WebSocket) {}
}

#[cfg(feature="hydrate")]
#[cfg_attr(docsrs, doc(cfg(feature = "hydrate")))]
impl crate::utils::ws::WebSocketClient<NonZeroU32,QueueMessage> for QueueSocket {
    fn new(ws: leptos::web_sys::WebSocket) -> Self { Self{
        #[cfg(not(doc))]
        socket:ws,
        #[cfg(doc)]
        socket:(),
        _running:RwSignal::new(false),
        #[cfg(feature="ssr")] listener:unreachable!()
    } }
    fn socket(&mut self) -> &mut leptos::web_sys::WebSocket {&mut self.socket }
    fn on_open(&self) -> Option<Box<dyn FnMut()>> {
      let running = self._running;
      Some(Box::new(move || {
        running.set(true);
      }))
    }
}

#[cfg(all(feature="ssr",not(feature="hydrate")))]
impl QueueSocket {
  fn run(_:AllQueues) {
    Self::force_start_server();
  }
}

#[cfg(feature="hydrate")]
impl QueueSocket {
  fn run(queues:AllQueues) {
    use crate::utils::ws::WebSocketClient;
    Self::force_start_client(move |msg| {
      //tracing::warn!("Starting!");
      let current = queues.selected.get_untracked();
      queues.queues.with_untracked(|queues| {
        let Some(queue) = queues.get(&current) else {
          tracing::error!("Queue not found: {current}");
          return
        };
        Self::do_msg(*queue, msg);
      });
      if !queues.show.get_untracked() {
        queues.show.set(true);
      }
      None
    },move |mut socket| {
      Effect::new(move |_| {
        if socket._running.get() {
          let current = queues.selected.get_untracked();
          socket.send(&current);
        }
      });
    });
  }
  fn do_msg(queue:RwSignal<QueueData>,msg:QueueMessage) {
    match msg {
      QueueMessage::Idle(entries) =>
        queue.set(QueueData::Idle(RwSignal::new(entries))),
      QueueMessage::Started { running, queue:actual_queue, blocked, failed, done } =>
        queue.set(QueueData::Running(RunningQueue {
          running:RwSignal::new(running),
          queue:RwSignal::new(actual_queue),
          blocked:RwSignal::new(blocked),
          failed:RwSignal::new(failed),
          done:RwSignal::new(done),
          eta:WrappedEta(RwSignal::new(Eta::default()))
        })),
      QueueMessage::Finished { failed, done } =>
        queue.set(QueueData::Finished(failed, done)),
      QueueMessage::TaskStarted { id, target } =>
        queue.with_untracked(|queue| 
          if let QueueData::Running(RunningQueue {queue,running,..}) = queue {
            queue.update(|v| {
              let Some((i,_)) = v.iter().enumerate().find(|(_,e)| e.id == id) else {return};
              let e = v.remove(i);
              e.steps.update(|m| m.insert(target,TaskState::Running));
              running.update(|running| running.push(e));
            });
          }
        ),
      QueueMessage::TaskSuccess { id, target, eta } =>
        queue.with_untracked(|queue| 
          if let QueueData::Running(RunningQueue {queue,running,done,eta:etasignal,..}) = queue {
            etasignal.0.set(eta);
            running.update(|v| {
              let Some((i,_)) = v.iter().enumerate().find(|(_,e)| e.id == id) else {return};
              let e = v.remove(i);
              e.steps.update(|m| m.insert(target,TaskState::Done));
              if e.steps.with_untracked(|v| v.iter().any(
                |(_,v)| *v == TaskState::Queued || *v == TaskState::Blocked
              )) {
                queue.update(|v| v.push(e));
              } else {
                done.update(|v| v.push(e));
              }
            });
          }
        ),
      QueueMessage::TaskFailed { id, target, eta } =>
        queue.with_untracked(|queue| 
          if let QueueData::Running(RunningQueue {running,failed,eta:etasignal,..}) = queue {
            etasignal.0.set(eta);
            running.update(|v| {
              let Some((i,_)) = v.iter().enumerate().find(|(_,e)| e.id == id) else {return};
              let e = v.remove(i);
              e.steps.update(|m| m.insert(target,TaskState::Failed));
              failed.update(|v| v.push(e));
            });
          }
        )
    }
  }
}


#[derive(Clone,Copy)]
struct AllQueues {
    show:RwSignal<bool>,
    selected:RwSignal<NonZeroU32>,
    queue_names:RwSignal<VecMap<NonZeroU32,String>>,
    queue_repos:RwSignal<VecMap<NonZeroU32,Option<Vec<RepoInfo>>>>,
    queues:RwSignal<VecMap<NonZeroU32,RwSignal<QueueData>>>
}

impl AllQueues {
  fn new(ids:Vec<QueueInfo>) -> Self {
    let queues = RwSignal::new(ids.iter().map(|v| (v.id,RwSignal::new(QueueData::Empty))).collect());
    let selected = ids.first().map_or_else(||NonZeroU32::new(1).unwrap_or_else(|| unreachable!()),|v| v.id);
    let mut queue_names = VecMap::default();
    let mut queue_repos = VecMap::default();
    for d in ids {
      queue_names.insert(d.id,d.name);
      queue_repos.insert(d.id,d.archives)
    }
    Self {
      show:RwSignal::new(false),
      selected:RwSignal::new(selected),
      queues,
      queue_names:RwSignal::new(queue_names),
      queue_repos:RwSignal::new(queue_repos)
    }
  }
}

#[derive(Clone)]
#[allow(dead_code)]
enum QueueData {
    Idle(RwSignal<Vec<Entry>>),
    Running(RunningQueue),
    Empty,
    Finished(Vec<Entry>,Vec<Entry>)
}

#[derive(Clone,Copy)]//,serde::Serialize,serde::Deserialize)]
#[allow(dead_code)]
struct RunningQueue {
    running:RwSignal<Vec<Entry>>,
    queue:RwSignal<Vec<Entry>>,
    blocked:RwSignal<Vec<Entry>>,
    failed:RwSignal<Vec<Entry>>,
    done:RwSignal<Vec<Entry>>,
    eta:WrappedEta
}

#[derive(Clone,Copy)]
struct WrappedEta(RwSignal<Eta>);

#[allow(clippy::cast_precision_loss)]
impl WrappedEta {
  fn into_view(self) -> impl IntoView {
    use thaw::ProgressBar;
    inject_css("flams-eta", r"
.flams-progress-bar {height:10px;}
    ");
    let pctg = Memo::new(move |_| {
      let eta = self.0.get();
      ((eta.done as f64 / eta.total as f64) * 1000.0).round() / 1000.0
    });
    let time_left = move || {
      let eta = self.0.get();
      if eta.time_left == Delta::default() {
        "N/A".to_string()
      } else {
        eta.time_left.max_seconds().to_string()
      }
    };
    view!{
      <div style="width:500px;"><ProgressBar class="flams-progress-bar" value=pctg/>
        {move || (pctg.get() * 100.0).to_string().chars().take(4).collect::<String>()} "%; ca. "{time_left}" remaining"
      </div>
    }
  }
}

#[derive(Clone,Debug,serde::Serialize,serde::Deserialize,PartialEq,Eq)]
pub struct Entry{
  id:u32,
  archive:ArchiveId,
  rel_path:String,
  #[cfg(feature="hydrate")]
  steps:RwSignal<VecMap<String,TaskState>>,
  #[cfg(not(feature="hydrate"))]
  steps:VecMap<String,TaskState>,
}

impl Entry {

  #[cfg(not(feature="hydrate"))]
  fn as_view(&self) -> impl IntoView {
    view!{
      <li>{format!("[{}]{}",self.archive,self.rel_path)}</li>
    }
  }

  #[cfg(feature="hydrate")]
  fn as_view(&self) -> impl IntoView {
    use flams_web_utils::components::{Collapsible,Header};
    let title=format!("[{}]{}",self.archive,self.rel_path);
    let total = self.steps.with_untracked(|v| v.0.len());
    let steps = self.steps;
    let current = move || steps.with(|v| {
      v.iter().enumerate().find_map(|(i,(e,s))| if *s == TaskState::Done {None} else {
        Some((i+1,e.clone()))
      }).unwrap_or_else(|| (total,"Done".to_string()))
    });
    let rel_path = self.rel_path.clone();
    let archive = self.archive.clone();
    view!{
      <li><Collapsible>
        <Header slot>
          <b>{title}{move || {let (i,s) = current(); format!(" ({i}/{total}) {s}")}}</b>
        </Header>
        <ol>
        {let rel_path = rel_path.clone();
          let archive = archive.clone();
          move || steps.get().iter().map(|(t,e)|
          view!(<li>{e.into_view(t.clone(),&archive,&rel_path)}</li>)
        ).collect_view()}
        </ol>
      </Collapsible></li>
    }
  }
}

#[cfg(feature="ssr")]
impl From<flams_system::building::QueueEntry> for Entry {
  fn from(e:flams_system::building::QueueEntry) -> Self {
    #[cfg(feature="hydrate")]
    {unreachable!()}
    #[cfg(not(feature="hydrate"))]
    Self {
      id:e.id.into(),
      archive:e.archive,
      rel_path:e.rel_path.to_string(),
      steps:e.steps.into_iter().map(|(k,v)| (k.to_string(),v.into())).collect()
    }
  }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
pub enum TaskState {
    Running,Queued,Blocked,Done,Failed,None
}
impl TaskState {
  fn into_view(self,t:String,archive:&ArchiveId,rel_path:&str) -> impl IntoView {
    use flams_web_utils::components::{LazyCollapsible,Header};
    use thaw::Scrollbar;
    match self {
      Self::Running => EitherOf4::A(view!{<i style="color:yellow">{t}" (Running)"</i>}),
      Self::Queued | Self::Blocked | Self::None => EitherOf4::B(view!{<span style="color:gray">{t}" (...)"</span>}),
      Self::Done => {
        let archive = archive.clone();
        let rel_path = rel_path.to_string();
        let tc = t.clone();
        EitherOf4::C(view!{
          <LazyCollapsible>
            <Header slot><span style="color:green">{t}" (Done)"</span></Header>
            {
              let archive = archive.clone();
              let rel_path = rel_path.clone();
              let tc = tc.clone();
              let queue = expect_context::<AllQueues>().selected.get_untracked();
              from_server_clone(true, move || get_log(queue,archive.clone(),rel_path.to_string(),tc.clone()), |s| {
              view!{<Scrollbar style="max-height: 160px;max-width:80vw;border:2px solid black;padding:5px;">
                <pre style="width:fit-content;font-size:smaller;">{s}</pre>
              </Scrollbar>}
            })}
          </LazyCollapsible>
        })
      },
      Self::Failed => {
        let archive = archive.clone();
        let rel_path = rel_path.to_string();
        let tc = t.clone();
        EitherOf4::D(view!{
          <LazyCollapsible>
            <Header slot><span style="color:red">{t}" (Failed)"</span></Header>
            {
              let archive = archive.clone();
              let rel_path = rel_path.clone();
              let tc = tc.clone();
              let queue = expect_context::<AllQueues>().selected.get_untracked();
              from_server_clone(true, move || get_log(queue,archive.clone(),rel_path.to_string(),tc.clone()), |s| {
              view!{<Scrollbar style="max-height: 160px;max-width:80vw;border:2px solid black;padding:5px;">
                <pre style="width:fit-content;font-size:smaller;">{s}</pre>
              </Scrollbar>}
            })}
          </LazyCollapsible>
        })
      }
    }
  }
}
#[cfg(feature="ssr")]
impl From<flams_system::building::TaskState> for TaskState {
  fn from(e:flams_system::building::TaskState) -> Self {
    use flams_system::building::TaskState;
    match e {
      TaskState::Running => Self::Running,
      TaskState::Queued => Self::Queued,
      TaskState::Blocked => Self::Blocked,
      TaskState::Done => Self::Done,
      TaskState::Failed => Self::Failed,
      TaskState::None => Self::None
    }
  }
}

#[derive(Debug,Clone,serde::Serialize,serde::Deserialize)]
pub enum QueueMessage {
    Idle(Vec<Entry>),
    Started {running:Vec<Entry>,queue:Vec<Entry>,blocked:Vec<Entry>,failed:Vec<Entry>,done:Vec<Entry>},
    Finished { failed:Vec<Entry>, done:Vec<Entry> },
    TaskStarted {id:u32,target:String},
    TaskSuccess {id:u32,target:String,eta:Eta},
    TaskFailed {id:u32,target:String,eta:Eta}
}
#[cfg(feature="ssr")]
impl From<flams_system::building::QueueMessage> for QueueMessage {
  fn from(e:flams_system::building::QueueMessage) -> Self {
    use flams_system::building::QueueMessage;
    match e {
      QueueMessage::Idle(v) => Self::Idle(v.into_iter().map(Into::into).collect()),
      QueueMessage::Started {running,queue,blocked,failed,done} => Self::Started {
        running:running.into_iter().map(Into::into).collect(),
        queue:queue.into_iter().map(Into::into).collect(),
        blocked:blocked.into_iter().map(Into::into).collect(),
        failed:failed.into_iter().map(Into::into).collect(),
        done:done.into_iter().map(Into::into).collect()
      },
      QueueMessage::Finished { failed, done } =>
        Self::Finished {
          failed:failed.into_iter().map(Into::into).collect(),
          done:done.into_iter().map(Into::into).collect()
        },
      QueueMessage::TaskStarted {id,target} => Self::TaskStarted {id:id.into(),target:target.to_string()},
      QueueMessage::TaskSuccess {id,target,eta} => Self::TaskSuccess {id:id.into(),target:target.to_string(),eta},
      QueueMessage::TaskFailed {id,target,eta} => Self::TaskFailed {id:id.into(),target:target.to_string(),eta}
    }
  }
}