use crate::{
    MathArchive,
    backend::{GlobalBackend, LocalBackend},
};
use ftml_ontology::{
    narrative::elements::DocumentElementRef,
    utils::{Css, RefTree},
};
use ftml_uris::{ArchiveId, DocumentUri};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ImagePath<'s> {
    Kpse(&'s str),
    ArchiveRelpath { id: ArchiveId, rel_path: &'s str },
    File(&'s Path),
}

impl<'s> ImagePath<'s> {
    #[must_use]
    pub fn to_path(self) -> Option<PathBuf> {
        let r = match self {
            Self::Kpse(p) => tex_engine::engine::filesystem::kpathsea::KPATHSEA.which(p),
            Self::ArchiveRelpath { id, rel_path } => {
                GlobalBackend.with_local_archive(&id, |a| a.map(|a| a.path().join(&*rel_path)))
            }
            Self::File(p) => Some(p.into()),
        }?;
        if r.extension().is_some_and(|r| r == "pdf") {
            Some(r.with_extension("pdf-rustex.png"))
        } else {
            Some(r)
        }
    }
    #[must_use]
    pub fn get_webp(path: &Path) -> Option<(Box<[u8]>, &str)> {
        static NO_WEBP: &[&str] = &["svg"];
        if let Some(ext) = path.extension().and_then(|s| s.to_str())
            && NO_WEBP.contains(&ext)
        {
            std::fs::read(path)
                .ok()
                .map(|v| (v.into_boxed_slice(), ext))
        } else {
            let img = image::ImageReader::open(path).ok()?.decode().ok()?;
            let mut v = Vec::<u8>::new();
            img.write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(&mut v))
                .ok()?;
            Some((v.into_boxed_slice(), "webp"))
        }
    }
    #[must_use]
    pub fn get(self) -> Option<(Box<[u8]>, String)> {
        Self::get_webp(&self.to_path()?).map(|(b, e)| (b, e.to_string()))
    }
    #[must_use]
    #[allow(clippy::option_if_let_else)]
    pub fn from_query(s: &'s str, allow_file: bool) -> Option<Self> {
        if let Some(s) = s.strip_prefix("kpse=") {
            Some(Self::Kpse(s))
        } else if let Some(f) = s.strip_prefix("file=")
            && allow_file
        {
            Some(Self::File(Path::new(f)))
        } else if let Some(s) = s.strip_prefix("a=")
            && let Some((a, rp)) = s.split_once("&rp=")
        {
            let a = a.parse().unwrap_or_else(|_| unreachable!());
            Some(Self::ArchiveRelpath {
                id: a,
                rel_path: rp,
            })
        } else if let Some(s) = s.strip_prefix("a=")
            && let Some((a, rp)) = s.split_once("&amp;rp=")
        {
            let a = a.parse().unwrap_or_else(|_| unreachable!());
            Some(Self::ArchiveRelpath {
                id: a,
                rel_path: rp,
            })
        } else {
            None
        }
    }
}

struct ExportData<'s> {
    images: &'s mut Vec<Box<Path>>,
    aux_path: &'s Path,
    img_path: &'s Path,
    css_path: &'s Path,
    css: &'s mut Vec<Box<str>>,
    all_documents: &'s mut Vec<DocumentUri>,
    backend: &'s dyn LocalBackend,
}

pub(crate) fn html_export(
    uri: &DocumentUri,
    to: &Path,
    backend: &dyn LocalBackend,
) -> Result<(), String> {
    use std::fmt::Write as SW;
    use std::io::Write;
    let mut images = Vec::new();
    let mut all_documents = Vec::new();
    let inputs = to.join("aux");
    let img_path = to.join("img");
    let css_path = to.join("css");
    std::fs::create_dir_all(&inputs).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&img_path).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&css_path).map_err(|e| e.to_string())?;
    let mut css = Vec::new();
    let mut data = ExportData {
        images: &mut images,
        aux_path: &inputs,
        img_path: &img_path,
        css_path: &css_path,
        css: &mut css,
        all_documents: &mut all_documents,
        backend,
    };
    recurse_export(uri, &mut data)?;

    let htmlstr = backend.get_html_full(uri).map_err(|e| e.to_string())?;
    let htmlstr = subst_img(htmlstr, &mut images, &img_path)?.into_string();
    let mut replaces = String::new();
    for (i, u) in all_documents.into_iter().enumerate() {
        if replaces.is_empty() {
            replaces.push_str(",\nredirects:[");
        } else {
            replaces.push(',');
        }
        let _ = write!(&mut replaces, "[\"{u}\",\"aux/{i}.json\"]");
    }
    if !replaces.is_empty() {
        replaces.push(']');
    }
    let htmlstr = htmlstr.replace(
        "</head>",
        &format!(
            r#"
        <script type="text/javascript" id="ftml">
    	window.FTML_CONFIG = {{
    	  documentUri:"{uri}",
    	  backendUrl:"https://mathhub.info",
    	  logLevel:"WARN"
    	  {replaces}
    	}};
    	</script>
    	<script src="https://mathhub.info/ftml.js"></script>
        </head>"#
        ),
    );
    let index = to.join("index.html");
    let mut out = std::fs::File::create_new(index).map_err(|e| e.to_string())?;
    out.write_all(htmlstr.as_bytes())
        .map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(())
}
fn recurse_export(uri: &DocumentUri, data: &mut ExportData) -> Result<(), String> {
    data.all_documents.push(uri.clone());
    let doc = data.backend.get_document(uri).map_err(|e| e.to_string())?;
    for e in doc.dfs() {
        if let DocumentElementRef::DocumentReference { target, .. } = e
            && !data.all_documents.contains(target)
        {
            let idx = data.all_documents.len();
            let path = data.aux_path.join(format!("{idx}.json"));
            do_document(target, &path, data)?;
            recurse_export(target, data)?;
        }
    }
    Ok(())
}

fn do_document(uri: &DocumentUri, path: &Path, data: &mut ExportData) -> Result<(), String> {
    let (mut css, htmlstr) = data.backend.get_html_body(uri).map_err(|e| e.to_string())?;
    let htmlstr = subst_img(htmlstr, data.images, data.img_path)?;
    for c in &mut css {
        if let Css::Link(opath) = c
            && let Some(path) = opath.strip_prefix("srv:/")
        {
            let idx = if let Some(i) = data.css.iter().position(|p| **p == *path) {
                format!("./css/{i}.css")
            } else {
                let i = data.css.len();
                data.css.push(path.into());
                let f = std::env::current_exe()
                    .map_err(|e| e.to_string())?
                    .parent()
                    .ok_or_else(|| "error getting parent directory of executable".to_string())?
                    .join("web")
                    .join(path);
                let target = data.css_path.join(format!("{i}.css"));
                std::fs::copy(f, target).map_err(|e| e.to_string())?;
                format!("{i}.css")
            };
            *opath = idx.into_boxed_str();
        }
    }
    let out = std::fs::File::create_new(path).map_err(|e| e.to_string())?;
    serde_json::to_writer(std::io::BufWriter::new(out), &(uri, css, htmlstr))
        .map_err(|e| e.to_string())
}

fn subst_img(
    htmlstr: Box<str>,
    images: &mut Vec<Box<Path>>,
    img_path: &Path,
) -> Result<Box<str>, String> {
    thread_local! {
        // "prea", "preb", "srv" = data-ftml-src="(THIS)" "posta", "postb"
        static REGEX: fancy_regex::Regex = fancy_regex::Regex::new(r#"<img\s+(?<prea>(?:(?!src=[\"\'])[^>])*)(?:src=[\"\'][\"\'])?(?<preb>[^>]*)(?<srv>data-ftml-src=[\"\']srv:\/img\?[^\"\']*[\"\'])(?<posta>(?:(?!src=[\"\'])[^>])*)(?:src=[\"\'][\"\'])?(?<postb>[^>]*)>"#).expect("this is a bug");
    }
    let mut failed: Option<String> = None;
    let cow = REGEX.with(|regex| {
        regex.replace_all(&htmlstr, |cap: &fancy_regex::Captures<str>| {
            macro_rules! ret {
                ($name:pat = $e:expr) => {
                    let Some($name) = $e else { ret!() };
                };
                () => {{
                    failed = Some("invalid img tag in html string".to_string());
                    return Cow::Borrowed("");
                }};
            }
            let srv = &cap["srv"];
            let capurl = if let Some(srv) = srv.strip_prefix("data-ftml-src=\"")
                && let Some(s) = srv.strip_suffix("\"")
            {
                //srv.strip_circumfix("data-ftml-src=\"", "\"") {
                s
            } else if let Some(srv) = srv.strip_prefix("data-ftml-src=\'")
                && let Some(s) = srv.strip_suffix("\'")
            {
                // srv.strip_circumfix("data-ftml-src\'", "\'") {
                s
            } else {
                ret!();
            };
            ret!(capurl = capurl.strip_prefix("srv:/img?"));
            ret!(img = ImagePath::from_query(capurl, true));
            ret!(img = img.to_path());
            let filestr = if let Some(i) = images.iter().position(|p| **p == *img) {
                // TODO use correct extension
                format!("./img/{i}.webp")
            } else {
                ret!((imgbytes, ext) = ImagePath::get_webp(&img));
                let i = images.len();
                let out_file = img_path.join(format!("{i}.{ext}"));
                if let Err(e) = std::fs::write(out_file, imgbytes) {
                    failed = Some(e.to_string());
                    return Cow::Borrowed("");
                }
                let s = format!("./img/{i}.{ext}");
                images.push(img.into_boxed_path());
                s
            };
            let pre_a = cap.name("prea").map_or("", |c| c.as_str());
            let pre_b = cap.name("preb").map_or("", |c| c.as_str());
            let post_a = cap.name("posta").map_or("", |c| c.as_str());
            let post_b = cap.name("postb").map_or("", |c| c.as_str());
            Cow::Owned(format!(
                "<img {pre_a}{pre_b}src=\"{filestr}\"{post_a}{post_b}>"
            ))
        })
    });
    if let Some(e) = failed {
        return Err(e);
    }
    match cow {
        Cow::Borrowed(_) => Ok(htmlstr),
        Cow::Owned(s) => Ok(s.into_boxed_str()),
    }
}
