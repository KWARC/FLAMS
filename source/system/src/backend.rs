use flams_math_archives::{
    backend::{AnyBackend, GlobalBackend, TemporaryBackend},
    utils::AsyncEngine,
};

static MAIN_BACKEND: std::sync::OnceLock<AnyBackend> = std::sync::OnceLock::new();
pub fn backend() -> &'static AnyBackend {
    MAIN_BACKEND.get_or_init(|| AnyBackend::Global)
}

pub fn initialize<A: AsyncEngine>(rdf: bool) {
    let settings = crate::settings::Settings::get();
    if settings.lsp {
        MAIN_BACKEND.get_or_init(|| AnyBackend::Temp(TemporaryBackend::new(AnyBackend::Global)));
    }
    GlobalBackend.load(settings.mathhubs());
    if rdf {
        A::background(|| {
            GlobalBackend
                .triple_store()
                .load_archives(&GlobalBackend.all_archives());
        });
    }
}
