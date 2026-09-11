mod check;
#[cfg(any(doc, feature = "docs"))]
pub mod endpoints;

use std::path::{Path, PathBuf};

#[allow(unused_imports)]
use flams_stex::STEX;
use flams_system::settings::{BuildQueueSettings, ServerSettings, SettingsSpec};
use ftml_uris::ArchiveId;

fn in_tokio(f: impl Future<Output = ()>) {
    let mut rt = tokio::runtime::Builder::new_multi_thread();
    rt.enable_all();
    rt.build()
        .expect("Failed to initialize Tokio runtime")
        .block_on(async move {
            tokio::select! {
            () = f => {},
            _ = tokio::signal::ctrl_c() => std::process::exit(0)
            }
        })
}
fn in_tokio_fn(f: impl FnOnce() + Send + 'static) {
    let mut rt = tokio::runtime::Builder::new_multi_thread();
    rt.enable_all();
    rt.build()
        .expect("Failed to initialize Tokio runtime")
        .block_on(async move {
            tokio::select! {
            r = tokio::task::spawn_blocking(f) => {
                r.expect("this is a bug");
            },
            _ = tokio::signal::ctrl_c() => std::process::exit(0)
            }
        })
}

fn main() {
    let mut cli = Cli::get();
    match cli.command.take() {
        Some(Commands::Build {
            archive,
            path,
            persist,
            verbose,
        }) => {
            let mut settings: SettingsSpec = cli.into();
            settings.buildqueue.num_threads = Some(1);
            in_tokio_fn(move || {
                flams_system::settings::Settings::initialize(settings);
                check::check(archive, path, persist, !verbose)
            });
        }
        None => flams_main::main(cli.into()),
    }
}

#[derive(clap::Parser, Debug)]
#[command(name="flams",propagate_version = true, version, about, long_about = Some(
"𝖥𝖫∀𝖬∫ - Flexiformal Annotation Management System\n\
--------------------------------------------------------------------\n\
See the \u{1b}]8;;https://github.com/UniFormal/MMT\u{1b}\\documentation\u{1b}]8;;\u{1b}\\ for details"
))]
pub struct Cli {
    /// a comma-separated list of `MathHub` paths (if not given, the default paths are used
    /// as determined by the MATHHUB system variable or ~/.mathhub/mathhub.path)
    #[arg(short, long)]
    pub(crate) mathhubs: Option<String>,

    /// whether to enable debug logging
    #[arg(short, long)]
    pub(crate) debug: Option<bool>,

    #[arg(short, long)]
    /// The toml config file to use
    pub(crate) config_file: Option<PathBuf>,

    #[arg(short, long)]
    /// The log directory to use
    pub(crate) log_dir: Option<PathBuf>,

    #[arg(long)]
    /// The stack size in MB used for every thread
    pub(crate) stack_size: Option<u8>,

    #[arg(long)]
    /// The directory used for temporary files
    pub(crate) temp_dir: Option<PathBuf>,

    #[arg(long)]
    /// The directory used for embedding models for vector search
    pub(crate) embedding_dir: Option<PathBuf>,

    #[arg(short, long)]
    /// The admin password to use for the server
    pub(crate) admin_pwd: Option<String>,

    /// Network port to use for the server
    #[arg(long,value_parser = clap::value_parser!(u16).range(1..))]
    pub(crate) port: Option<u16>,

    /// Network address to use for the server
    #[arg(long)]
    pub(crate) ip: Option<String>,

    #[arg(long)]
    pub(crate) external_url: Option<String>,

    #[arg(long)]
    /// The database file to use for account management etc.
    pub(crate) db: Option<PathBuf>,

    #[arg(long)]
    /// The directory to use for the rdf triple store.
    pub(crate) rdf_database: Option<PathBuf>,

    /// The number of threads to use for the buildqueue
    #[arg(short, long)]
    pub(crate) threads: Option<u8>,

    /// enter lsp mode
    #[arg(long)]
    pub(crate) lsp: bool,

    #[arg(long)]
    pub(crate) gitlab_url: Option<String>,

    #[arg(long)]
    pub(crate) gitlab_token: Option<String>,
    #[arg(long)]
    pub(crate) gitlab_app_id: Option<String>,
    #[arg(long)]
    pub(crate) gitlab_app_secret: Option<String>,
    #[arg(long)]
    pub(crate) gitlab_redirect_url: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Checks a single file
    Build {
        /// The archive of the file
        #[arg(short, long)]
        archive: ArchiveId,
        /// The file path relative to the archive's source directory
        #[arg(short, long)]
        path: PathBuf,
        /// Whether to write the build to disk (otherwise, results are printed and discarded)
        #[arg(long)]
        persist: bool,
        /// Print entire proof trees; otherwise, successful steps are not expanded
        #[arg(long)]
        verbose: bool,
    },
}

impl From<Cli> for SettingsSpec {
    fn from(cli: Cli) -> Self {
        fn from_file(cfg_file: &Path) -> SettingsSpec {
            let cfg = std::fs::read_to_string(cfg_file).unwrap_or_else(|e| {
                panic!("Could not read config file {}: {e}", cfg_file.display())
            });
            let cfg: SettingsSpec = toml::from_str(&cfg).unwrap_or_else(|e| {
                panic!("Could not parse config file {}: {e}", cfg_file.display())
            });
            cfg
        }
        let (cfg, mut settings) = cli.into();
        settings += SettingsSpec::from_envs();
        if let Some(cfg_file) = cfg {
            if cfg_file.exists() {
                settings += from_file(&cfg_file);
            } else {
                panic!("Could not find config file {}", cfg_file.display());
            }
        } else if let Ok(path) = std::env::current_exe()
            && let Some(path) = path.parent()
        {
            let path = path.join("settings.toml");
            if path.exists() {
                settings += from_file(&path);
            }
        }
        settings
    }
}
impl From<Cli> for (Option<PathBuf>, SettingsSpec) {
    /// #### Panics
    fn from(cli: Cli) -> Self {
        let settings = SettingsSpec {
            mathhubs: cli
                .mathhubs
                .map(|s| s.split(',').map(|s| PathBuf::from(s.trim())).collect())
                .unwrap_or_default(),
            debug: cli.debug,
            stack_size: cli.stack_size,
            database: cli.db.map(PathBuf::into_boxed_path),
            rdf_database: cli.rdf_database.map(PathBuf::into_boxed_path),
            log_dir: cli.log_dir.map(PathBuf::into_boxed_path),
            temp_dir: cli.temp_dir.map(PathBuf::into_boxed_path),
            embedding_dir: cli.embedding_dir.map(PathBuf::into_boxed_path),
            server: ServerSettings {
                port: cli.port.unwrap_or_default(),
                ip: cli.ip.map(|s| s.parse().expect("Illegal ip")),
                admin_pwd: cli.admin_pwd,
                external_url: cli.external_url,
            },
            buildqueue: BuildQueueSettings {
                num_threads: cli.threads,
            },
            gitlab: flams_utils::settings::GitlabSettings {
                url: cli.gitlab_url.map(|s| s.parse().expect("Illegal url")),
                token: cli.gitlab_token.map(Into::into),
                app_id: cli.gitlab_app_id.map(Into::into),
                app_secret: cli.gitlab_app_secret.map(Into::into),
                redirect_url: cli.gitlab_redirect_url.map(Into::into),
            },
            lsp: cli.lsp,
            remotes: std::collections::HashMap::default(),
        };
        (cli.config_file, settings)
    }
}

impl Cli {
    #[must_use]
    #[inline]
    fn get() -> Self {
        use clap::Parser;
        Self::parse()
    }
}
