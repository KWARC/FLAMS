use ::rustex_lib::engine::{CompilationResult, RusTeXEngineExt};
use flams_utils::binary::BinaryWriter;
use parking_lot::Mutex;
use std::io::Write;
use std::path::Path;
use tex_engine::prelude::Mouth;

#[allow(clippy::module_inception)]
mod rustex {
    pub use rustex_lib;
    pub use rustex_lib::engine::commands::{
        register_primitives_postinit, register_primitives_preinit,
    };
    pub use rustex_lib::engine::files::RusTeXFileSystem;
    pub use rustex_lib::engine::output::{OutputCont, RusTeXOutput};
    pub use rustex_lib::engine::stomach::RusTeXStomach;
    pub use rustex_lib::engine::{Extension, RusTeXEngine, RusTeXEngineT, Types};
    pub use rustex_lib::engine::{fonts::Fontsystem, state::RusTeXState};
    pub use tex_engine::commands::{Macro, MacroSignature, TeXCommand};
    pub use tex_engine::engine::filesystem::FileSystem;
    pub use tex_engine::engine::gullet::DefaultGullet;
    pub use tex_engine::engine::mouth::DefaultMouth;
    pub use tex_engine::engine::{DefaultEngine, EngineAux};
    pub use tex_engine::pdflatex::{PDFTeXEngine, nodes::PDFExtension};
    pub use tex_engine::prelude::{CSName, InternedCSName, Token, TokenList};
    pub use tex_engine::tex::tokens::StandardToken;
    pub use tex_engine::tex::tokens::control_sequences::CSInterner;
    pub use tex_engine::{engine::utils::memory::MemoryManager, tex::tokens::CompactToken};
    pub use tracing::{debug, error, instrument, trace, warn};
    pub type RTSettings = rustex_lib::engine::Settings;
    pub use rustex_lib::ImageOptions;
}
pub use rustex::OutputCont;
#[allow(clippy::wildcard_imports)]
use rustex::*;

struct FileOutput(std::cell::RefCell<std::io::BufWriter<std::fs::File>>);
impl FileOutput {
    fn new(path: &Path) -> Self {
        let f = std::fs::File::create(path).expect("This should not happen!");
        let buf = std::io::BufWriter::new(f);
        Self(std::cell::RefCell::new(buf))
    }
}

impl OutputCont for FileOutput {
    fn message(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "{text}");
    }
    fn errmessage(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "{text}");
    }
    fn file_open(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "({text}");
    }
    fn file_close(&self, _text: String) {
        let _ = self.0.borrow_mut().write_string(")\n");
    }
    fn write_18(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "{text}");
    }
    fn write_17(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "{text}");
    }
    fn write_16(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "{text}");
    }
    fn write_neg1(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "{text}");
    }
    fn write_other(&self, text: String) {
        let _ = writeln!(self.0.borrow_mut(), "{text}");
    }
    #[inline]
    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

pub struct TracingOutput;
impl OutputCont for TracingOutput {
    fn message(&self, text: String) {
        debug!(target:"rustex","{}", text);
    }
    fn errmessage(&self, text: String) {
        debug!(target:"rustex","{}", text);
    }
    fn file_open(&self, text: String) {
        trace!(target:"rustex","({}", text);
    }
    fn file_close(&self, _text: String) {
        trace!(target:"rustex",")");
    }
    fn write_18(&self, text: String) {
        trace!(target:"rustex","write18: {}", text);
    }
    fn write_17(&self, text: String) {
        debug!(target:"rustex","{}", text);
    }
    fn write_16(&self, text: String) {
        trace!(target:"rustex","write16: {}", text);
    }
    fn write_neg1(&self, text: String) {
        trace!(target:"rustex","write-1: {}", text);
    }
    fn write_other(&self, text: String) {
        trace!(target:"rustex","write: {}", text);
    }

    #[inline]
    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

#[derive(Clone)]
pub(crate) struct EngineBase {
    state: RusTeXState,
    memory: MemoryManager<CompactToken>,
    font_system: Fontsystem,
}
static ENGINE_BASE: Mutex<Option<Result<EngineBase, ()>>> = Mutex::new(None);

impl EngineBase {
    pub(crate) fn into_engine(self) -> RusTeXEngine {
        self.into_engine_opts(
            std::iter::once(("FLAMS_ADMIN_PWD".to_string(), "NOPE".to_string())),
            TracingOutput,
        )
    }
    fn into_engine_opts<O: OutputCont + 'static, I: IntoIterator<Item = (String, String)>>(
        mut self,
        envs: I,
        out: O,
    ) -> RusTeXEngine {
        //use tex_engine::engine::filesystem::FileSystem;
        use tex_engine::engine::EngineExtension;
        use tex_engine::engine::gullet::Gullet;
        use tex_engine::engine::stomach::Stomach;
        use tex_engine::prelude::ErrorHandler;
        use tex_engine::prelude::Mouth;
        let mut aux = EngineAux {
            outputs: RusTeXOutput::Cont(Box::new(out)),
            error_handler: ErrorThrower::new(),
            start_time: chrono::Local::now(),
            extension: Extension::new(&mut self.memory),
            memory: self.memory,
            jobname: String::new(),
        };
        let mut mouth = DefaultMouth::new(&mut aux, &mut self.state);
        let gullet = DefaultGullet::new(&mut aux, &mut self.state, &mut mouth);
        let mut stomach = RusTeXStomach::new(&mut aux, &mut self.state);
        stomach.continuous = true;
        DefaultEngine {
            state: self.state,
            aux,
            fontsystem: self.font_system,
            filesystem: RusTeXFileSystem::new_with_envs(tex_engine::utils::PWD.to_path_buf(), envs),
            mouth,
            gullet,
            stomach,
        }
    }
    fn get<R, F: FnOnce(&Self) -> R>(f: F) -> Result<R, ()> {
        let mut lock = ENGINE_BASE.lock();
        match &mut *lock {
            Some(Ok(engine)) => Ok(f(engine)),
            Some(_) => Err(()),
            o => {
                *o = Some(Self::initialize());
                o.as_ref()
                    .unwrap_or_else(|| unreachable!())
                    .as_ref()
                    .map(f)
                    .map_err(|()| ())
            }
        }
    }

    pub(crate) fn from_engine(engine: DefaultEngine<Types>) -> Self {
        Self {
            state: engine.state,
            memory: engine.aux.memory,
            font_system: engine.fontsystem,
        }
    }

    #[instrument(level = "info", target = "sTeX", name = "Initializing RusTeX engine")]
    fn initialize() -> Result<Self, ()> {
        use tex_engine::engine::TeXEngine;
        std::panic::catch_unwind(|| {
            let mut engine = DefaultEngine::<Types>::default();
            engine.aux.outputs = RusTeXOutput::Cont(Box::new(TracingOutput));
            register_primitives_preinit(&mut engine);
            match engine.initialize_pdflatex() {
                Ok(()) => {}
                Err(e) => {
                    error!("Error initializing RusTeX engine: {}", e);
                }
            }
            register_primitives_postinit(&mut engine);
            match engine.init_file("rustex_defs.def") {
                Ok(()) => {}
                Err(e) => {
                    error!("Error initializing RusTeX engine: {}", e);
                }
            }
            Self::from_engine(engine)
        })
        .map_err(|a| {
            if let Some(s) = a.downcast_ref::<String>() {
                tracing::error!("Error initializing RusTeX engine: {}", s);
            } else if let Some(s) = a.downcast_ref::<&str>() {
                tracing::error!("Error initializing RusTeX engine: {}", s);
            } else {
                tracing::error!("Error initializing RusTeX engine");
            }
        })
    }
}

pub struct RusTeX(pub(crate) Mutex<EngineBase>);
impl RusTeX {
    /// # Errors
    pub fn get() -> Result<Self, ()> {
        Ok(Self(
            ENGINE_BASE
                .lock()
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .clone()?
                .into(),
        ))
    }
    pub fn initialize() {
        let _ = EngineBase::get(|_| ());
    }
    /// ### Errors
    pub fn run(&self, file: &Path, memorize: bool, out: Option<&Path>) -> Result<String, String> {
        self.run_with_envs(
            file,
            memorize,
            std::iter::once(("FLAMS_ADMIN_PWD".to_string(), "NOPE".to_string())),
            out,
        )
    }

    /// ### Errors
    fn set_up<I: IntoIterator<Item = (String, String)>>(
        &self,
        envs: I,
        out: Option<&Path>,
    ) -> (DefaultEngine<Types>, RTSettings) {
        let e = self.0.lock().clone();
        let engine = match out {
            None => e.into_engine_opts(envs, TracingOutput),
            Some(f) => e.into_engine_opts(envs, FileOutput::new(f)),
        };
        let settings = RTSettings {
            verbose: false,
            sourcerefs: true,
            log: true,
            insert_font_info: false,
            image_options: ImageOptions::AsIs,
        };
        (engine, settings)
    }

    /// ### Errors
    pub fn run_with_envs<I: IntoIterator<Item = (String, String)>>(
        &self,
        file: &Path,
        memorize: bool,
        envs: I,
        out: Option<&Path>,
    ) -> Result<String, String> {
        let (mut engine, settings) = self.set_up(envs, out);
        //println!("Setup done; compiling...");
        let res = engine.run(file.to_str().unwrap_or_else(|| unreachable!()), settings);
        //println!("Done. Memorizing / evaluating result...");

        res.error.as_ref().map_or_else(
            || {
                if memorize {
                    let mut base = self.0.lock();
                    give_back(engine, &mut base, &[b"c_stex_module_", b"c_stex_mathhub_"]);
                }
                Ok(res.to_string())
            },
            |(e, _)| Err(e.to_string()),
        )
    }

    pub fn builder(&self) -> RusTeXRunBuilder<false> {
        RusTeXRunBuilder {
            inner: self.0.lock().clone().into_engine_opts(
                std::iter::once(("FLAMS_ADMIN_PWD".to_string(), "NOPE".to_string())),
                TracingOutput,
            ),
            settings: RTSettings {
                verbose: false,
                sourcerefs: false,
                log: true,
                insert_font_info: false,
                image_options: ImageOptions::AsIs,
            },
        }
    }
}

pub struct RusTeXRunBuilder<const HAS_PATH: bool> {
    inner: DefaultEngine<Types>,
    settings: RTSettings,
}
impl<const HAS_PATH: bool> RusTeXRunBuilder<HAS_PATH> {
    pub fn set_output<O: OutputCont>(mut self, output: O) -> Self {
        self.inner.aux.outputs = RusTeXOutput::Cont(Box::new(output));
        self
    }
    pub const fn set_sourcerefs(mut self, b: bool) -> Self {
        self.settings.sourcerefs = b;
        self
    }
    pub fn set_envs<I: IntoIterator<Item = (String, String)>>(mut self, envs: I) -> Self {
        self.inner.filesystem.add_envs(envs);
        self
    }
    pub const fn set_font_debug_info(mut self, b: bool) -> Self {
        self.settings.insert_font_info = b;
        self
    }
}

pub struct EngineRemnants(DefaultEngine<Types>);

impl EngineRemnants {
    pub fn memorize(self, global: &RusTeX) {
        let mut base = global.0.lock();
        give_back(self.0, &mut base, &[b"c_stex_module_", b"c_stex_mathhub_"]);
    }
    pub fn take_output<O: OutputCont>(&mut self) -> Option<O> {
        match std::mem::replace(&mut self.0.aux.outputs, RusTeXOutput::None) {
            RusTeXOutput::Cont(o) => o.as_any().downcast().ok().map(|b: Box<O>| *b),
            _ => None,
        }
    }
}

impl RusTeXRunBuilder<true> {
    pub fn run(mut self) -> (CompilationResult, EngineRemnants) {
        *self.inner.aux.extension.elapsed() = std::time::Instant::now();
        let res =
            match tex_engine::engine::TeXEngine::run(&mut self.inner, rustex_lib::shipout::shipout)
            {
                Ok(()) => None,
                Err(e) => {
                    self.inner.aux.outputs.errmessage(format!(
                        "{}\n\nat {}",
                        e,
                        self.inner
                            .mouth
                            .current_sourceref()
                            .display(&self.inner.filesystem)
                    ));
                    Some(e)
                }
            };
        let res = self.inner.do_result(res, self.settings);
        (res, EngineRemnants(self.inner))
    }
}
impl RusTeXRunBuilder<false> {
    pub fn set_path(mut self, p: &Path) -> Option<RusTeXRunBuilder<true>> {
        let parent = p.parent()?;
        self.inner.filesystem.set_pwd(parent.to_path_buf());
        self.inner.aux.jobname = p.with_extension("").file_name()?.to_str()?.to_string();
        let f = self.inner.filesystem.get(p.as_os_str().to_str()?);
        self.inner.mouth.push_file(f);
        Some(RusTeXRunBuilder {
            inner: self.inner,
            settings: self.settings,
        })
    }
    pub fn set_string(mut self, in_path: &Path, content: &str) -> Option<RusTeXRunBuilder<true>> {
        let parent = in_path.parent()?;
        self.inner.filesystem.set_pwd(parent.to_path_buf());
        self.inner.aux.jobname = in_path
            .with_extension("")
            .file_name()?
            .to_str()?
            .to_string();
        self.inner
            .filesystem
            .add_file(in_path.to_path_buf(), content);
        let f = self.inner.filesystem.get(in_path.file_name()?.to_str()?);
        self.inner.mouth.push_file(f);
        Some(RusTeXRunBuilder {
            inner: self.inner,
            settings: self.settings,
        })
    }
    pub fn set_string_noaux(
        mut self,
        in_path: &Path,
        content: &str,
    ) -> Option<RusTeXRunBuilder<true>> {
        let aux = in_path.with_extension("aux");
        self.inner.filesystem.add_file(aux, "");
        self.set_string(in_path, content)
    }
}

fn save_macro(
    name: InternedCSName<u8>,
    m: &Macro<CompactToken>,
    oldmem: &CSInterner<u8>,
    newmem: &mut CSInterner<u8>,
    state: &mut RusTeXState,
) {
    let oldname = oldmem.resolve(name);
    let newname = convert_name(name, oldmem, newmem);

    let exp = &m.expansion;
    let newexp: TokenList<_> = exp
        .0
        .iter()
        .map(|x| convert_token(*x, oldmem, newmem))
        .collect();
    let newsig = MacroSignature {
        arity: m.signature.arity,
        params: m
            .signature
            .params
            .0
            .iter()
            .map(|x| convert_token(*x, oldmem, newmem))
            .collect(),
    };
    let newmacro = Macro {
        protected: m.protected,
        long: m.long,
        outer: m.outer,
        signature: newsig,
        expansion: newexp,
    };
    state.set_command_direct(newname, Some(TeXCommand::Macro(newmacro)));
}

fn convert_name(
    oldname: InternedCSName<u8>,
    oldmem: &CSInterner<u8>,
    newmem: &mut CSInterner<u8>,
) -> InternedCSName<u8> {
    newmem.intern(oldmem.resolve(oldname))
}

fn convert_token(
    old: CompactToken,
    oldmem: &CSInterner<u8>,
    newmem: &mut CSInterner<u8>,
) -> CompactToken {
    match old.to_enum() {
        StandardToken::ControlSequence(cs) => {
            CompactToken::from_cs(convert_name(cs, oldmem, newmem))
        }
        _ => old,
    }
}

use tex_engine::tex::tokens::control_sequences::CSNameMap;
use tex_engine::utils::errors::ErrorThrower;

fn give_back(engine: RusTeXEngine, base: &mut EngineBase, prefixes: &'static [&'static [u8]]) {
    let EngineBase {
        state,
        memory,
        font_system,
    } = base;
    *font_system = engine.fontsystem;
    let oldinterner = engine.aux.memory.cs_interner();
    let iter = CommandIterator {
        prefixes, //: &[b"c_stex_module_", b"c_stex_mathhub_"],
        cmds: engine.state.destruct().into_iter(),
        interner: oldinterner,
    };
    for (n, c) in iter.filter_map(|(a, b)| match b {
        TeXCommand::Macro(m) => Some((a, m)),
        _ => None,
    }) {
        save_macro(n, &c, oldinterner, memory.cs_interner_mut(), state);
    }
}

pub struct CommandIterator<'a, I: Iterator<Item = (InternedCSName<u8>, TeXCommand<Types>)>> {
    prefixes: &'static [&'static [u8]],
    cmds: I,
    interner: &'a <InternedCSName<u8> as CSName<u8>>::Handler,
}
impl<I: Iterator<Item = (InternedCSName<u8>, TeXCommand<Types>)>> Iterator
    for CommandIterator<'_, I>
{
    type Item = (InternedCSName<u8>, TeXCommand<Types>);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((name, cmd)) = self.cmds.next() {
                let bname = self.interner.resolve(name);
                if self.prefixes.iter().any(|p| bname.starts_with(p)) {
                    return Some((name, cmd));
                }
            } else {
                return None;
            }
        }
    }
}
