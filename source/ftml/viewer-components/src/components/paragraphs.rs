use flams_ontology::{narration::paragraphs::ParagraphKind, uris::{DocumentElementURI, DocumentURI, Name}};
use flams_web_utils::inject_css;
use leptos::{prelude::*,context::Provider};

use crate::{components::counters::{LogicalLevel, SectionCounters}, ts::{ParagraphContinuation, SlideContinuation}};

use super::{TOCElem, TOCIter};

pub(super) fn paragraph<V:IntoView+'static>(kind:ParagraphKind,uri:DocumentElementURI,styles:Box<[Name]>,children:impl FnOnce() -> V + Send + 'static) -> impl IntoView {
  inject_css("ftml-sections", include_str!("sections.css"));
  let mut counters : SectionCounters = expect_context();
  let style = counters.get_para(kind,&styles);
  let prefix = match kind {
    ParagraphKind::Assertion => Some("ftml-assertion"),
    ParagraphKind::Definition => Some("ftml-definition"),
    ParagraphKind::Example => Some("ftml-example"),
    ParagraphKind::Paragraph => Some("ftml-paragraph"),
    _ => None
  };
  let cls = prefix.map(|p| {
    let mut s = String::new();
    s.push_str(p);
    for style in styles {
      s.push(' ');
      s.push_str(p);
      s.push('-');
      s.push_str(style.first_name().as_ref());
    }
    s
  });

  view!{
    <Provider value=counters>
    <div class=cls style=style>{ParagraphContinuation::wrap(&(uri,kind) ,children())}</div>
    </Provider>
  }
}

pub(super) fn slide<V:IntoView+'static>(uri:DocumentElementURI,children:impl FnOnce() -> V + Send + 'static) -> impl IntoView {
  inject_css("ftml-slide", include_str!("slides.css"));
  let counters = SectionCounters::slide_inc();
  view!(<Provider value=counters>
      <div class="ftml-slide">{SlideContinuation::wrap(&uri ,children())}</div>
  </Provider>)
}

pub(super) fn slide_number() -> impl IntoView {
  let v = SectionCounters::get_slide();
  move || v.get()
}
/*
pub(super) fn skip_slides(uri:&DocumentURI,id:&str) -> Option<u32> {
  let ctw = expect_context::<RwSignal::<Option<Vec<TOCElem>>>>();
  ctw.with(|v| v.as_ref().and_then(|v| {
    leptos::logging::log!("TOC: {v:?}");
    v.iter_elems().find_map(|e| 
      if let TOCElem::Inputref{uri:u,id:i,children,..} = e {
        if u == uri && i == id {
          Some(children.iter_elems().filter(|e| matches!(e ,TOCElem::Slide{..})).count() as u32)
        } else { None }
      } else { None }
    )
}))
}
 */