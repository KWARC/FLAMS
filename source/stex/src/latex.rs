use std::{ffi::OsStr, path::Path};

pub fn clean(path: &Path) {
    const EXTENSIONS: &[&str] = &[
        "aux",
        "log",
        "bbl",
        "toc",
        "upa",
        "upb",
        "blg",
        "out",
        "idx",
        "ilg",
        "ind",
        "mw",
        "nav",
        "snm",
        "vrb",
        "sms",
        "sms2",
        "hd",
        "glo",
        "bcf",
        "blg",
        "fdb_latexmk",
        "fls",
        //"sref",
        "run.xml",
        "synctex.gz",
    ];
    for ext in EXTENSIONS {
        let p = path.with_extension(ext);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
    }
    // remove "x-blx.bib"
    let Some(stem) = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string() + "-blx.bib")
    else {
        return;
    };
    let p = path.with_file_name(stem);
    if p.exists() {
        let _ = std::fs::remove_file(p);
    }
}

pub fn pdflatex_and_bib<S: AsRef<std::ffi::OsStr>, I: IntoIterator<Item = (S, S)>>(
    path: &Path,
    envs: I,
) -> Result<(), ()> {
    pdflatex(path, envs)?;
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return Err(());
    };
    let Some(parent) = path.parent() else {
        return Err(());
    };
    let bib = path.with_extension("bcf");
    if bib.exists() {
        let _ = run_command(
            "biber",
            std::iter::once(stem),
            parent,
            std::iter::empty::<(String, String)>(),
        );
    } else {
        let _ = run_command(
            "bibtex",
            std::iter::once(stem),
            parent,
            std::iter::empty::<(String, String)>(),
        );
    }
    Ok(())
}

pub fn pdflatex<S: AsRef<std::ffi::OsStr>, I: IntoIterator<Item = (S, S)>>(
    path: &Path,
    envs: I,
) -> Result<(), ()> {
    let Some(parent) = path.parent() else {
        return Err(());
    };
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return Err(());
    };
    //tracing::info!("Running: pdflatex {stem} in {}", parent.display());
    let out = run_command(
        "pdflatex",
        ["-interaction", "nonstopmode", "-halt-on-error", stem],
        parent,
        envs,
    )?;
    if !out.status.success() {
        //let out = String::from_utf8_lossy(out.stdout.as_slice());
        //let err = out
        //    .find("Fatal error")
        //    .map_or("unknown error",|i| &out[i..]);
        //tracing::error!("pdflatex failed: {} ({})", err,path.display());
        return Err(());
    }
    Ok(())
}

fn run_command<
    A: AsRef<OsStr>,
    S: AsRef<OsStr>,
    Env: IntoIterator<Item = (S, S)>,
    Args: IntoIterator<Item = A>,
>(
    cmd: &str,
    args: Args,
    in_path: &Path,
    with_envs: Env,
) -> Result<std::process::Output, ()> {
    let mut proc = std::process::Command::new(cmd);
    let mut process = proc
        .args(args)
        .current_dir(in_path)
        .env("FLAMS_ADMIN_PWD", "NOPE");
    for (k, v) in with_envs {
        process = process.env(k, v);
    }
    match process
        // Without this, stdin is inherited from the parent. pdflatex/bibtex/
        // biber routinely hit non-fatal warnings that print "Type <return>
        // to continue." and block reading a line from stdin even under
        // -halt-on-error - if the parent's stdin is a pipe/pty that never
        // sends EOF (a background service, this build pipeline's own test
        // harness, ...), that read blocks forever. Closing stdin makes such
        // a prompt read EOF immediately instead, so pdflatex treats it the
        // same as an empty answer and moves on to the real fatal error.
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c.wait_with_output().map_err(|_e| {
            //tracing::error!("Error executing command {cmd}: {e}");
        }),
        Err(_e) => {
            //tracing::error!("Error executing command {cmd}: {e}");
            Err(())
        }
    }
}
