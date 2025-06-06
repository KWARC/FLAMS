//#![feature(string_from_utf8_lossy_owned)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

mod parser;

use either::Either;
use flams_ontology::uris::{ArchiveURITrait, DocumentURI};
use flams_system::{backend::{AnyBackend, Backend}, build_result, build_target, building::{BuildArtifact, BuildResult, BuildResultArtifact, BuildTask}, formats::{BuildArtifactTypeId, OMDocResult, CHECK, UNCHECKED_OMDOC}, source_format};

source_format!(ftml ["html","xhtml","htm"] [FTML_IMPORT => FTML_OMDOC => CHECK] @
  "Flexiformally annotated HTML"
  = |_,_| todo!()
);

build_target!(
  ftml_import [] => [FTML_DOC] 
  @ "Import existing FTML"
  = |_,_| todo!()
);

build_target!(
  ftml_omdoc [FTML_DOC] => [UNCHECKED_OMDOC] 
  @ "Extract OMDoc from FTML" 
  = extract
);

build_result!(ftml_doc @ "Semantically annotated HTML");

fn extract(backend:&AnyBackend,task:&BuildTask) -> BuildResult {
  let html:Result<HTMLString,_> = backend.with_archive(task.archive().archive_id(), |a| {
    let Some(a) = a else {return Err(BuildResult::err())};
    a.load(task.rel_path()).map_err(|e| BuildResult {
      log:Either::Left(format!("Error loading html data for {}/{}: {e}",task.archive().archive_id(),task.rel_path())),
      result:Err(Vec::new())
    })
  });
  let html = match html {
    Err(e) => return e,
    Ok(h) => h
  };
  let uri = match task.document_uri() {
    Ok(uri) => uri,
    Err(e) => return BuildResult::with_err(format!("{e:#}"))
  };
  match build_ftml(backend,&html.0,uri,task.rel_path()) {
    Err(e) => BuildResult {
      log:Either::Left(e),
      result:Err(Vec::new())
    },
    Ok((r,s)) => BuildResult {
      log:Either::Left(s),
      result:Ok(BuildResultArtifact::Data(Box::new(r)))
    }
  }
}

/// #### Errors
#[inline]
pub fn build_ftml(backend:&AnyBackend,html:&str,uri:DocumentURI,rel_path:&str) -> Result<(OMDocResult,String),String> {
  parser::HTMLParser::run(html,uri,rel_path,backend)
}


pub struct HTMLString(pub String);
impl BuildArtifact for HTMLString {
  #[inline] fn get_type_id() -> BuildArtifactTypeId where Self:Sized {
      FTML_DOC
  }
  #[inline]
  fn get_type(&self) -> BuildArtifactTypeId {
      FTML_DOC
  }
  fn write(&self,path:&std::path::Path) -> Result<(),std::io::Error> {
    std::fs::write(path, &self.0)
  }
  fn load(p:&std::path::Path) -> Result<Self,std::io::Error> where Self:Sized {
    let s = std::fs::read_to_string(p)?;
    Ok(Self(s))
  }

  #[inline]
  fn as_any(&self) -> &dyn std::any::Any {self}
}
impl HTMLString {
  #[must_use]
  pub fn create(s:String) -> BuildResultArtifact {
    BuildResultArtifact::Data(Box::new(Self(s)))
  }
}