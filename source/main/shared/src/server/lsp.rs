use flams_database::{DBBackend, DBUser};
use flams_lsp::{async_lsp::ClientSocket, state::LSPState};
use flams_router_dashboard::LoginState;

struct WSLSPServer {
    client: ClientSocket,
    state: LSPState,
}

impl flams_lsp::FLAMSLSPServer for WSLSPServer {
    #[inline]
    fn client_mut(&mut self) -> &mut ClientSocket {
        &mut self.client
    }
    #[inline]
    fn client(&self) -> &ClientSocket {
        &self.client
    }
    #[inline]
    fn state(&self) -> &LSPState {
        &self.state
    }
}

pub(crate) async fn register(
    auth_session: axum_login::AuthSession<DBBackend>,
    ws: axum::extract::WebSocketUpgrade,
) -> axum::response::Response {
    let login = match &auth_session.backend.admin {
        None => LoginState::NoAccounts,
        Some(_) => match auth_session.user {
            None => LoginState::None,
            Some(DBUser {
                id: 0, username, ..
            }) if username == "admin" => LoginState::Admin,
            Some(u) => LoginState::User {
                name: u.username,
                avatar: u.avatar_url.unwrap_or_default(),
                is_admin: u.is_admin,
            },
        },
    };
    match login {
        LoginState::NoAccounts | LoginState::Admin => flams_lsp::ws::upgrade(ws, |c| WSLSPServer {
            client: c,
            state: LSPState::default(),
        }),
        _ => {
            let mut res = axum::response::Response::new(axum::body::Body::empty());
            *(res.status_mut()) = http::StatusCode::UNAUTHORIZED;
            res
        }
    }
}

#[tokio::test]
//#[cfg(test)]
async fn linter() {
    /*
    tracing_subscriber::fmt().init();
    let _ce = color_eyre::install();
    let mut spec = flams_system::settings::SettingsSpec::default();
    spec.lsp = true;
    flams_system::settings::Settings::initialize(spec);
    flams_system::backend::GlobalBackend::initialize();
    //flams_system::initialize(spec);
    let state = LSPState::default();
    let _ = GLOBAL_STATE.set(state.clone());
    tracing::info!("Waiting for stex to load...");
    std::thread::sleep(std::time::Duration::from_secs(3));
    tracing::info!("Go!");
    let (_, t) = measure(move || {
        tracing::info!("Loading all archives");
        let mut files = Vec::new();
        for a in GlobalBackend::get().all_archives().iter() {
            if let Archive::Local(a) = a {
                a.with_sources(|d| {
                    for e in <_ as TreeChildIter<SourceDir>>::dfs(d.children.iter()) {
                        match e {
                            SourceEntry::File(f) => files.push((
                                f.relative_path
                                    .split('/')
                                    .fold(a.source_dir(), |p, s| p.join(s))
                                    .into(),
                                DocumentUri::from_archive_relpath(
                                    a.uri().owned(),
                                    &f.relative_path,
                                ),
                            )),
                            _ => {}
                        }
                    }
                })
            }
        }
        let len = files.len();
        tracing::info!("Linting {len} files");
        state.load_all(
            files.into_iter(), /*.enumerate().map(|(i,(path,uri))| {
                                 tracing::info!("{}/{len}: {}",i+1,path.display());
                                 (path,uri)
                               })*/
            |_, _| {},
        );
    });
    tracing::info!("initialized after {t}");
     */
}
