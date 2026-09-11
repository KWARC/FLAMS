#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(any(
    all(feature = "ssr", feature = "hydrate", not(feature = "docs-only")),
    not(any(feature = "ssr", feature = "hydrate"))
))]
compile_error!("exactly one of the features \"ssr\" or \"hydrate\" must be enabled");

use flams_router_base::LoginState;
use flams_utils::unwrap;
use ftml_dom::utils::css::inject_css;
use ftml_uris::ArchiveId;
use std::num::NonZeroU32;

pub mod server_fns;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FormatOrTarget {
    Format(String),
    Targets(Vec<String>),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueInfo {
    pub id: NonZeroU32,
    pub name: String,
    pub archives: Option<Vec<RepoInfo>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RepoInfo {
    Copy(ArchiveId),
    Git {
        id: ArchiveId,
        remote: String,
        branch: String,
        commit: flams_backend_types::git::Commit,
        //updates:Vec<(String,flams_git::Commit)>
    },
}

#[cfg(feature = "ssr")]
mod login {
    use std::num::NonZeroU32;

    use flams_router_base::LoginState;

    fn owns(user: &str, queue_name: &str) -> bool {
        queue_name
            .strip_prefix(user)
            .is_some_and(|rest| rest.as_bytes().iter().all(u8::is_ascii_digit))
    }

    pub trait LoginQueue {
        /// #### Errors
        fn with_queue<R>(
            &self,
            id: NonZeroU32,
            f: impl FnOnce(&flams_system::building::Queue) -> R,
        ) -> Result<R, String>;

        /// #### Errors
        fn with_opt_queue<R>(
            &self,
            id: Option<NonZeroU32>,
            f: impl FnOnce(
                flams_system::building::queue_manager::QueueId,
                &flams_system::building::Queue,
            ) -> R,
        ) -> Result<R, String>;
    }
    #[cfg(feature = "ssr")]
    impl LoginQueue for LoginState {
        fn with_queue<R>(
            &self,
            id: NonZeroU32,
            f: impl FnOnce(&flams_system::building::Queue) -> R,
        ) -> Result<R, String> {
            use flams_system::building::QueueName;
            let qm = flams_system::building::queue_manager::QueueManager::get();
            match self {
                Self::None | Self::Loading => {
                    return Err(format!("Not logged in: {self:?}"));
                }
                Self::Admin | Self::NoAccounts | Self::User { is_admin: true, .. } => (),
                Self::User { name, .. } => {
                    return qm.with_queue(id.into(), move |q| q.map_or_else(
                        || Err(format!("Queue {id} not found")),
                        |q| if matches!(q.name(),QueueName::Sandbox{name:qname,..} if owns(name,&qname))
                        {
                            Ok(f(q))
                        } else {
                            Err(format!("Not allowed to run queue {id}"))
                        }
                    ));
                }
            }
            qm.with_queue(id.into(), move |q| {
                q.map_or_else(|| Err(format!("Queue {id} not found")), |q| Ok(f(q)))
            })
        }

        fn with_opt_queue<R>(
            &self,
            id: Option<NonZeroU32>,
            f: impl FnOnce(
                flams_system::building::queue_manager::QueueId,
                &flams_system::building::Queue,
            ) -> R,
        ) -> Result<R, String> {
            use flams_system::building::QueueName;
            let qm = flams_system::building::queue_manager::QueueManager::get();
            match (self, id) {
                (Self::None | Self::Loading, _) =>
                    Err(format!("Not logged in: {self:?}")),
                (Self::User { name, .. }, Some(id)) => qm.with_queue(id.into(), move |q|
                   q.map_or_else(
                       || Err(format!("Queue {id} not found")),
                       |q| if matches!(q.name(),QueueName::Sandbox{name:qname,..} if owns(name,&qname))
                       {
                           Ok(f(id.into(), q))
                       } else {
                           Err(format!("Not allowed to run queue {id}"))
                       }
                   )),
                (Self::Admin, Some(id)) => qm.with_queue(id.into(), |q|
                    q.map_or_else(
                        || Err(format!("Queue {id} not found")),
                        |q| Ok(f(id.into(), q))
                    )),
                (Self::User { name, .. }, _) => {
                    let queue = qm.new_queue(name);
                    qm.with_queue(queue, |q| {
                        let Some(q) = q else { unreachable!() };
                        Ok(f(queue, q))
                    })
                }
                (Self::Admin, _) => {
                    let queue = qm.new_queue("admin");
                    qm.with_queue(queue, |q| {
                        let Some(q) = q else { unreachable!() };
                        Ok(f(queue, q))
                    })
                }
                (Self::NoAccounts, _) => qm.with_global(|q| {
                    Ok(f(
                        flams_system::building::queue_manager::QueueId::global(),
                        q,
                    ))
                }),
            }
        }
    }
}
#[cfg(feature = "ssr")]
pub use login::*;

use leptos::prelude::*;
#[must_use]
pub fn select_queue(queue_id: RwSignal<Option<NonZeroU32>>) -> impl IntoView {
    use flams_web_utils::components::display_error;
    use ftml_component_utils::Spinner;
    move || {
        let user = LoginState::get();
        if matches!(user, LoginState::NoAccounts) {
            return None;
        }
        let r = Resource::new(|| (), move |()| server_fns::get_queues());
        Some(view! {<Suspense fallback = || view!(<Spinner/>)>{move || {
          match r.get() {
            None => leptos::either::EitherOf3::A(view!(<Spinner/>)),
            Some(Err(e)) => leptos::either::EitherOf3::B(display_error(e.to_string().into())),
            Some(Ok(queues)) => leptos::either::EitherOf3::C(view!{<div><div style="width:fit-content;margin-left:auto;">{
                do_queues(queue_id,queues)
            }</div></div>})
          }
        }}</Suspense>})
    }
}

fn do_queues(queue_id: RwSignal<Option<NonZeroU32>>, v: Vec<QueueInfo>) -> impl IntoView {
    use ftml_component_utils::Select;
    inject_css("flams-select-queue", include_str!("select_queue.css"));
    let queues = if v.is_empty() {
        vec![(0u32, "New Build Queue".to_string())]
    } else {
        v.into_iter()
            .map(|q| (q.id.get(), q.name))
            .chain(std::iter::once((0u32, "New Build Queue".to_string())))
            .collect()
    };
    let value = RwSignal::new(unwrap!(queues.first()).clone().1);
    let qc = queues.clone();
    let _ = Effect::new(move |_| {
        let queue = value.get();
        if queue == "New Build Queue" {
            queue_id.update_untracked(|v| *v = None);
        } else {
            let idx = unwrap!(? qc.iter().find_map(|(id,name)| if *name == queue {Some(*id)} else {None}));
            queue_id.update_untracked(|v| *v = Some(unwrap!(NonZeroU32::new(idx))));
        }
    });
    view! {
      <span style="font-style:italic;">"Build Queue: "
      <Select value class="flams-select-queue">{
        queues.into_iter().map(|(_,name)| view!{
          <option value=name>{name.clone()}</option>
        }).collect_view()
      }</Select></span>
    }
}
