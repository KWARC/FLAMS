#![cfg_attr(docsrs, feature(doc_cfg))]
//#![feature(file_buffered)]
//#![feature(lazy_type_alias)]

pub mod backend;
//pub mod formats;
pub mod building;
#[cfg(feature = "tokio")]
pub mod logging;
pub mod settings;
pub use flams_math_archives::FlamsExtension;

pub fn span_capture<R>(f: impl FnOnce() -> R) -> (String, R) {
    use tracing_subscriber::{fmt, layer::SubscriberExt};
    #[derive(Clone)]
    struct Buffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for &Buffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if let Ok(mut l) = self.0.lock() {
                l.extend_from_slice(buf);
            }
            Ok(buf.len())
        }
        #[inline]
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> fmt::MakeWriter<'a> for Buffer {
        type Writer = &'a Self;
        #[inline]
        fn make_writer(&'a self) -> Self::Writer {
            self
        }
    }
    let buffer = Buffer(std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new())));
    let bufcl = buffer.clone();
    let sub = tracing_subscriber::registry().with(
        fmt::layer()
            .with_writer(bufcl)
            .with_ansi(false)
            .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
    );
    let ret = tracing::subscriber::with_default(sub, f);
    let s = buffer
        .0
        .lock()
        .map(|mut l| String::from_utf8_lossy(&std::mem::take(&mut *l)).into_owned())
        .unwrap_or_default();
    (s, ret)
}

#[cfg(feature = "zip")]
pub mod zip;

use building::queue_manager::QueueManager;
use flams_math_archives::{utils::AsyncEngine, LocalArchive};
use settings::SettingsSpec;

pub use inventory::iter;
pub use inventory::submit as register_exension;
use tracing_subscriber::Layer;

#[cfg(feature = "tokio")]
pub struct TokioEngine;
#[cfg(feature = "tokio")]
impl AsyncEngine for TokioEngine {
    fn background(f: impl FnOnce() + Send + 'static) {
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || span.in_scope(f));
    }
    async fn block_on<R: Send + Sync + 'static>(
        f: impl FnOnce() -> R + Send + Sync + 'static,
    ) -> R {
        tokio::task::spawn_blocking(f).await.expect("this is a bug")
    }
    fn exec_after(delay: std::time::Duration, f: impl FnOnce() + Send + 'static) {
        tokio::task::spawn(async move {
            tokio::time::sleep(delay).await;
            f();
        });
    }
}

/// #### Panics
pub fn initialize<A: AsyncEngine>(settings: SettingsSpec, rdf: bool) {
    settings::Settings::initialize(settings);
    let settings = settings::Settings::get();
    #[cfg(feature = "rocksdb")]
    if let Some(p) = &settings.rdf_database {
        flams_math_archives::backend::set_global(p);
    }
    if settings.lsp {
        use tracing::Level;
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::Layer;
        #[cfg(feature = "tokio")]
        let logger = logging::LogStore::new();
        let debug = settings.debug;
        let level = if debug { Level::TRACE } else { Level::INFO };

        let l = tracing_subscriber::fmt::layer()
            //.with_max_level(Level::INFO)//(if debug {Level::TRACE} else {Level::INFO})
            .with_ansi(false)
            .with_target(true)
            .with_writer(std::io::stderr)
            .with_filter(tracing::level_filters::LevelFilter::from(Level::INFO)); //.init();
        #[cfg(feature = "tokio")]
        let sub = tracing_subscriber::registry()
            .with(logger.with_filter(tracing::level_filters::LevelFilter::from(level)))
            .with(l);
        #[cfg(not(feature = "tokio"))]
        let sub = tracing_subscriber::registry().with(l);
        tracing::subscriber::set_global_default(sub)
            .expect("Failed to set global default logging subscriber");
    } else {
        #[cfg(feature = "tokio")]
        logging::LogStore::initialize();
    }
    tracing::info_span!(target:"initializing",parent:None,"initializing").in_scope(move || {
        #[cfg(feature = "gitlab")]
        {
            if let Some(url) = &settings.gitlab_url {
                let cfg = flams_git::gl::GitlabConfig::new(
                    url.clone(),
                    settings.gitlab_token.as_ref().map(ToString::to_string),
                    settings.gitlab_app_id.as_ref().map(ToString::to_string),
                    settings.gitlab_app_secret.as_ref().map(ToString::to_string),
                );
                flams_git::gl::GLInstance::global().clone().load(cfg);
            }
        }
        backend::initialize::<A>(rdf);
        QueueManager::initialize(settings.num_threads);
        for e in inventory::iter::<FlamsExtension>() {
            A::background(|| {
                tracing::info_span!("Initializing", extension = e.name).in_scope(|| (e.on_start)());
            });
        }
    });
}

#[cfg(feature = "gitlab")]
pub trait LocalArchiveExt {
    fn is_managed(&self) -> Option<&flams_git::GitUrl>;
}

#[cfg(feature = "gitlab")]
impl LocalArchiveExt for LocalArchive {
    fn is_managed(&self) -> Option<&flams_git::GitUrl> {
        let gl = crate::settings::Settings::get().gitlab_url.as_ref()?;
        self.git_url(gl)
    }
}
