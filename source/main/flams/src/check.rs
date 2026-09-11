use std::path::PathBuf;

use flams_ftml::{FTML_CONTENT, FtmlResult};
use flams_math_archives::{
    MathArchive,
    artifacts::{ContentResult, ContentUpdate, FileOrString, FtmlString},
    backend::{AnyBackend, GlobalBackend, LocalBackend},
    utils::{AllSyncEngine, path_ext::PathExt},
};
use flams_stex::RusTeX;
use ftml_solver::CHECK;
use ftml_uris::{ArchiveId, DocumentUri};

pub fn check(archive: ArchiveId, path: PathBuf, persist: bool, truncate: bool) {
    let _ = enable_ansi_support::enable_ansi_support();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    GlobalBackend::initialize::<AllSyncEngine>(false);
    let (uri, full_path) = GlobalBackend
        .get()
        .with_local_archive(&archive, |a| {
            a.map(|a| (a.uri().clone(), a.source_dir().join(&path)))
        })
        .expect("File not found in archive");
    let uri =
        DocumentUri::from_archive_relpath(uri, &path.as_slash_str()).expect("Invalid file path");
    let text = std::fs::read_to_string(&full_path).expect("error reading tex file");
    let mh = flams_math_archives::mathhub::mathhubs()
        .iter()
        .copied()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(",");

    tracing::info!("Initializing RusTeX...");
    RusTeX::initialize();

    tracing::info!("Running RusTeX...");
    let rustex = RusTeX::get()
        .expect("Failed to initialize RusTeX")
        .builder()
        .set_envs([
            ("STEX_USESMS".to_string(), "true".to_string()),
            ("MATHHUB".to_string(), mh),
        ])
        .set_sourcerefs(true)
        .set_font_debug_info(true)
        .set_string(&full_path, &text)
        .expect("Failed to initialize RusTeX"); //.set_output(TracingOutput);

    let (res, _) = rustex.run();
    if let Some(e) = &res.error {
        panic!("{e:?}");
    }
    tracing::info!("RusTeX: success");
    let html = res.to_string();
    if persist
        && let Err(e) = GlobalBackend.get().save(
            &uri,
            None,
            FileOrString::Str("".into()),
            flams_stex::RUSTEX.id(),
            Some(Box::new(FtmlString(html.clone().into_boxed_str()))),
        )
    {
        panic!("Error persisting: {e}")
    }
    let r: FtmlResult = match flams_system::logging::ignore_traces(|| {
        flams_ftml::build_ftml(&AnyBackend::Global, &html, uri.clone())
    }) {
        Ok(r) => r,
        Err(e) => panic!("Error: {e}"),
    };
    if !r.errors.is_empty() {
        for e in r.errors {
            tracing::error!("Error {e}");
        }
        panic!("FTML extraction failed");
    }
    tracing::info!("Extracting FTML: success");
    let doc = r.doc.document.clone();
    let modules = r.doc.modules.clone();
    if persist
        && let Err(e) = GlobalBackend.get().save(
            &uri,
            None,
            FileOrString::Str("".into()),
            FTML_CONTENT.id(),
            Some(Box::new(ContentResult {
                document: r.doc.document,
                modules: r.doc.modules,
                data: r.doc.data,
                body: r.body,
                inner_offset: r.inner_offset,
                css: r.css,
                ftml: r.ftml,
                triples: r.doc.triples,
            })),
        )
    {
        panic!("Error persisting: {e}")
    }
    let mut checker =
        ftml_solver::Checker::<ftml_solver::split::SingleThreadedSplit>::new(AnyBackend::Global);

    let _ = checker.add_modules(modules);
    let (mut drs, modules) = match checker.check_document(&doc) {
        Ok((drs, modules)) => (drs, modules),
        Err(e) => {
            panic!("Error checking: Missing module: {e}");
        }
    };
    if persist
        && let Err(e) = GlobalBackend.get().save(
            &uri,
            None,
            FileOrString::Str("".into()),
            CHECK.id(),
            Some(Box::new(ContentUpdate {
                document: Some(doc),
                modules,
            })),
        )
    {
        panic!("Error persisting: {e}")
    }
    if truncate {
        drs.filter_failures(false);
    }
    println!("{}", drs.colored());
}
