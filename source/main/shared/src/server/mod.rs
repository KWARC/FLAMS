pub mod files;
pub mod lsp;

use std::future::IntoFuture;

use axum::{
    Router,
    error_handling::HandleErrorLayer,
    extract,
    response::{IntoResponse, Redirect},
};
use axum_login::AuthManagerLayerBuilder;
use axum_macros::FromRef;
use flams_database::DBBackend;
use flams_git::gl::auth::GitLabOAuth;
use flams_router_base::ws::WSServerSocket;
use flams_system::settings::Settings;
use http::{StatusCode, Uri};
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};
use leptos_meta::HashedStylesheet;
use tower::ServiceBuilder;
use tower_sessions::{Expiry, MemoryStore};
use tracing::{Instrument, instrument};

use flams_router_dashboard::{
    Main,
    ws::{self, WebSocketServer},
};

lazy_static::lazy_static! {
    static ref SERVER_SPAN:tracing::Span = {
        //println!("Here!");
        tracing::info_span!(target:"server",parent:None,"server")
    };
}

#[inline]
pub async fn run(port_channel: Option<tokio::sync::watch::Sender<Option<u16>>>) {
    run_i(port_channel).instrument(SERVER_SPAN.clone()).await;
}

/// ### Panics
#[instrument(level = "info", target = "server", name = "run", skip_all)]
async fn run_i(port_channel: Option<tokio::sync::watch::Sender<Option<u16>>>) {
    let mut state = ServerState::new().in_current_span().await;
    let mut addr = state.options.site_addr;
    let mut changed = false;
    let mut listener = None;
    //let span = tracing::info_span!(target:"server","request");
    for p in addr.port()..65535 {
        addr.set_port(p);
        if let Ok(l) = tokio::net::TcpListener::bind(addr)
            //.instrument(span.clone())
            .await
        {
            listener = Some(l);
            break;
        }
        changed = true;
    }
    let listener = listener.expect("Could not bind to any port");

    // avoid error: reactive_graph-0.1.7/src/owner/arena.rs:60:29, the `sandboxed-arenas` feature is active, but no Arena is active
    leptos::prelude::Owner::new().set();

    if changed {
        if port_channel.is_some() {
            tracing::warn!("Port already in use; used {} instead", addr.port());
        } else {
            println!("Port already in use; used {} instead", addr.port());
        }
        flams_system::settings::Settings::get()
            .port
            .store(addr.port(), std::sync::atomic::Ordering::Relaxed);
        state.options.site_addr = addr;
    }

    ftml_components::set_backend::<flams_router_content::backend::FtmlBackend>();
    ftml_components::set_continuation(&flams_router_content::Continuations);

    let session_store = MemoryStore::default();
    let session_layer = tower_sessions::SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::days(5),
        ))
        .with_secure(false)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    let auth_layer = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_| async {
            http::StatusCode::BAD_REQUEST
        }))
        .layer(AuthManagerLayerBuilder::new(state.db.clone(), session_layer).build());

    let routes = generate_route_list(Main);

    let has_gl = state.oauth.is_some();

    let mut app = axum::Router::<ServerState>::new()
        .route("/ws/log", axum::routing::get(ws::LogSocket::ws_handler))
        .route("/ws/queue", axum::routing::get(ws::QueueSocket::ws_handler))
        //.route("/ws/mathjx", axum::routing::get(ws::TeXSocket::handler))
        .route("/ws/lsp", axum::routing::get(crate::server::lsp::register));

    if has_gl {
        app = app //.route("/gl_login", axum::routing::get(gl::gl_login))
            .route("/gitlab_login", axum::routing::get(gl_cont));
    }

    let app = app
        .route(
            "/api/{*fn_name}",
            axum::routing::get(server_fn_handle).post(server_fn_handle),
        )
        .route(
            "/content/{*fn_name}",
            axum::routing::get(server_fn_handle).post(server_fn_handle),
        )
        .route(
            "/domain/{*fn_name}",
            axum::routing::get(server_fn_handle).post(server_fn_handle),
        )
        .leptos_routes_with_handler(
            routes,
            axum::routing::get(|a, b, c| routes_handler(a, b, c)), //.in_current_span()),
        )
        .route("/img", axum::routing::get(files::img_handler))
        .route("/doc", axum::routing::get(files::doc_handler))
        .route("/aux", axum::routing::get(files::aux_handler))
        .fallback(file_and_error_handler)
        .layer(auth_layer)
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_methods([http::Method::GET, http::Method::POST])
                .allow_origin(tower_http::cors::Any)
                //.allow_credentials(true)
                .allow_headers([http::header::COOKIE, http::header::SET_COOKIE]),
        )
        .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(SpanLayer));
    let app: Router<()> = app.with_state(state);

    if let Some(channel) = port_channel {
        channel
            .send(Some(addr.port()))
            .expect("Error sending port address");
    }

    //crate::fns::init();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .into_future()
    //.instrument(span)
    //.in_current_span()
    .await
    .unwrap_or_else(|e| panic!("{e}"));
}

async fn gl_cont(
    extract::Query(params): extract::Query<flams_git::gl::auth::AuthRequest>,
    extract::State(state): extract::State<ServerState>,
    mut auth_session: axum_login::AuthSession<DBBackend>,
) -> Result<axum::response::Response, StatusCode> {
    let oauth = state.oauth.as_ref().unwrap_or_else(|| unreachable!());
    let token = oauth
        .callback(params)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let gl = flams_git::gl::GLInstance::global()
        .get()
        .await
        .unwrap_or_else(|| unreachable!());
    let user = gl
        .get_oauth_user(&token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Ok(Some(u)) = state.db.add_user(user, token.secret().clone()).await {
        let _ = auth_session.login(&u).await;
    }
    Ok(Redirect::to("/dashboard").into_response())
}

async fn routes_handler(
    auth_session: axum_login::AuthSession<DBBackend>,
    extract::State(ServerState {
        db, options, oauth, ..
    }): extract::State<ServerState>,
    request: http::Request<axum::body::Body>,
) -> Result<impl IntoResponse, StatusCode> {
    use futures::future::FutureExt;
    let handler = leptos_axum::render_app_to_stream_with_context(
        move || {
            provide_context(auth_session.clone());
            provide_context(db.clone());
            provide_context(oauth.clone());
        },
        move || shell(options.clone()),
    );
    std::panic::AssertUnwindSafe(handler(request))
        .catch_unwind()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn server_fn_handle(
    auth_session: axum_login::AuthSession<DBBackend>,
    extract::State(ServerState {
        db, options, oauth, ..
    }): extract::State<ServerState>,
    request: http::Request<axum::body::Body>,
) -> impl IntoResponse {
    leptos_axum::handle_server_fns_with_context(
        move || {
            provide_context(auth_session.clone());
            provide_context(options.clone());
            provide_context(db.clone());
            provide_context(oauth.clone());
        },
        request,
    )
    //.in_current_span()
    .await
}

async fn file_and_error_handler(
    mut uri: Uri,
    extract::State(state): extract::State<ServerState>,
    request: http::Request<axum::body::Body>,
) -> axum::response::Response {
    (leptos_axum::file_and_error_handler(shell))(uri, extract::State(state), request).await
    /*
    let r = leptos_axum::file_and_error_handler(shell);
    if uri.path().ends_with("flams_bg.wasm") {
        // change to "flams.wasm"
        uri = Uri::builder()
            .path_and_query("/pkg/flams.wasm")
            .build()
            .unwrap_or_else(|_| unreachable!());
    }
    r(uri, extract::State(state), request)
        //.in_current_span()
        .await
    */
}

#[derive(Clone)]
struct SpanLayer;
impl<A> tower_http::trace::MakeSpan<A> for SpanLayer {
    fn make_span(&mut self, r: &http::Request<A>) -> tracing::Span {
        //println!("Here: {},{}",r.method(),r.uri());
        tracing::span!(
            parent:&*SERVER_SPAN,
            tracing::Level::INFO,
            "request",
            method = %r.method(),
            uri = %r.uri(),
            version = ?r.version(),
        )
    }
}

#[derive(Clone, FromRef)]
pub(crate) struct ServerState {
    options: LeptosOptions,
    db: DBBackend,
    pub(crate) images: files::ImageStore,
    pub(crate) oauth: Option<GitLabOAuth>,
}

impl ServerState {
    async fn new() -> Self {
        let leptos_cfg = Self::setup_leptos();
        let redirect = Settings::get().gitlab_redirect_url.as_ref();
        let oauth = if let Some(redirect) = redirect {
            flams_git::gl::GLInstance::global()
                .get()
                .await
                .and_then(|gl| gl.new_oauth(&format!("{redirect}/gitlab_login")).ok())
        } else {
            None
        };
        let db = DBBackend::new().in_current_span().await;
        Self {
            options: leptos_cfg.leptos_options,
            db,
            images: files::ImageStore::default(),
            oauth,
        }
    }

    fn setup_leptos() -> ConfFile {
        let basepath = Self::get_basepath();
        let mut leptos_cfg =
            leptos::prelude::get_configuration(None).expect("Failed to get leptos config");
        leptos_cfg.leptos_options.site_root = basepath.into();
        //leptos_cfg.leptos_options.output_name = "flams".into();
        /*#[cfg(debug_assertions)]
        {
            leptos_cfg.leptos_options.hash_files = false;
        }
        #[cfg(not(debug_assertions))]
        {
            leptos_cfg.leptos_options.hash_files = true;
        }*/
        leptos_cfg.leptos_options.hash_files = true;

        let settings = Settings::get();
        let ip = settings.ip;
        let port = settings.port();
        leptos_cfg.leptos_options.site_addr = std::net::SocketAddr::new(ip, port);
        //println!("Config: {:?}", leptos_cfg.leptos_options);
        leptos_cfg
    }

    #[cfg(debug_assertions)]
    fn get_basepath() -> String {
        /*if std::env::var("LEPTOS_OUTPUT_NAME").is_err() {
            unsafe { std::env::set_var("LEPTOS_OUTPUT_NAME", "flams") };
        }*/
        if Settings::get().lsp {
            let Ok(p) = std::env::current_exe()
                .expect("Error setting current web-dir path")
                .parent()
                .expect("Error setting current web-dir path")
                .parent()
                .expect("Error setting current web-dir path")
                .join("web")
                .canonicalize()
            else {
                panic!("Failed to canonicalize path");
            };
            p.display().to_string()
        } else {
            "target/web".to_string()
        }
    }
    #[cfg(not(debug_assertions))]
    fn get_basepath() -> String {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("web")))
            .expect("Failed to determine executable path")
            .display()
            .to_string()
    }
}

fn shell(options: LeptosOptions) -> impl IntoView {
    ftml_component_utils::ssr_wrap(move || {
        view! {
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="utf-8"/>
                    <meta name="viewport" content="width=device-width, initial-scale=1"/>
                    {
                        #[cfg(debug_assertions)]
                        {view!(<AutoReload options=options.clone() />)}
                    }
                    <HashedStylesheet id="leptos" options=options.clone()/>
                    <HydrationScripts options />//islands=true/>
                    <leptos_meta::MetaTags/>
                </head>
                <body>
                    <Main/>
                </body>
            </html>
        }
    })
}
