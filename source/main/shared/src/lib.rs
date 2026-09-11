#![recursion_limit = "256"]
//#![feature(let_chains)]
/*! Foo Bar
 *
 * See [endpoints] for public API endpoints
*/
#![cfg_attr(docsrs, feature(doc_cfg))]

/*#[cfg(any(
    all(feature = "ssr", feature = "hydrate", not(doc)),
    not(any(feature = "ssr", feature = "hydrate"))
))]
compile_error!("exactly one of the features \"ssr\" or \"hydrate\" must be enabled");
*/

#[cfg(feature = "ssr")]
pub mod server;

#[cfg(feature = "ssr")]
pub fn main(settings: flams_utils::settings::SettingsSpec) {
    #[allow(unused_imports)]
    use flams_ftml::FTML;
    #[allow(unused_imports)]
    #[cfg(feature = "tantivy")]
    use flams_search::TANTIVY;
    #[allow(unused_imports)]
    use flams_stex::STEX;

    use flams_system::settings::SettingsSpec;
    fn exit() {
        flams_system::building::queue_manager::QueueManager::clear();
        let _ = flams_system::settings::Settings::get().close();
        std::process::exit(0)
    }

    #[allow(clippy::future_not_send)]
    async fn run(settings: SettingsSpec) {
        let lsp = settings.lsp;
        let _ce = color_eyre::install();
        flams_system::initialize::<flams_system::TokioEngine>(settings, true);
        if lsp {
            let (sender, recv) = tokio::sync::watch::channel(None);
            tokio::select! {
            () = crate::server::run(Some(sender)) => {},
            () = flams_lsp::start_lsp(recv) => {},
            _ = tokio::signal::ctrl_c() => exit()
            }
        } else {
            tokio::select! {
            () = crate::server::run(None) => {},
            _ = tokio::signal::ctrl_c() => exit()
            }
        }
    }

    let mut rt = tokio::runtime::Builder::new_multi_thread();
    rt.enable_all();
    if let Some(mb) = settings.stack_size
        && mb > 2
    {
        rt.thread_stack_size((mb as usize) * 1024 * 1024);
    } else {
        if settings.lsp {
            rt.thread_stack_size(4 * 1024 * 1024);
        }
        #[cfg(debug_assertions)]
        {
            rt.thread_stack_size(if settings.lsp { 6 } else { 4 } * 1024 * 1024);
        }
    }

    rt.build()
        .expect("Failed to initialize Tokio runtime")
        .block_on(run(settings));
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use tracing_subscriber::prelude::*;
    fn filter(lvl: tracing::Level) -> tracing_subscriber::filter::Targets {
        tracing_subscriber::filter::Targets::new()
            .with_target("ftml_dom", lvl)
            .with_target("ftml_components", lvl)
            .with_target("ftml_parser", lvl)
            .with_target("ftml_backend", lvl)
            .with_target("ftml_ontology", lvl)
            .with_target("ssr_example", lvl)
            //.with_target("flams_flodown", lvl)
            .with_target("flams_router_base", lvl)
            .with_target(
                "leptos_posthoc",
                tracing_subscriber::filter::LevelFilter::ERROR,
            )
    }
    console_error_panic_hook::set_once();
    tracing_subscriber::registry()
        .with(tracing_wasm::WASMLayer::default())
        .with(filter(tracing::Level::WARN))
        .init();
    ftml_components::set_backend::<flams_router_content::backend::FtmlBackend>();
    ftml_components::set_continuation(&flams_router_content::Continuations);
    #[cfg(debug_assertions)]
    {
        leptos::mount::hydrate_body(flams_router_dashboard::Main);
    }
    #[cfg(not(debug_assertions))]
    {
        leptos::mount::hydrate_lazy(flams_router_dashboard::Main);
    }
}
