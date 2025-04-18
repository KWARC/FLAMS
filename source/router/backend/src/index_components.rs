use flams_ontology::{
    archive_json::{ArchiveIndex, Institution},
    uris::DocumentURI,
};
use flams_web_utils::components::wait_and_then_fn;
use leptos::prelude::*;
use thaw::{
    Body1, Caption1, Card, CardFooter, CardHeader, CardHeaderAction, CardHeaderDescription,
    CardPreview, Scrollbar,
};

#[component]
pub fn Index() -> impl IntoView {
    flams_web_utils::inject_css(
        "flams-index-card",
        ".flams-index-card{max-width:400px !important;margin:10px !important;}",
    );
    wait_and_then_fn(super::server_fns::index, |(is, idxs)| {
        let mut libraries = Vec::new();
        let mut books = Vec::new();
        let mut papers = Vec::new();
        let mut courses = Vec::new();
        let mut self_studies = Vec::new();
        for e in idxs {
            match e {
                e @ ArchiveIndex::Library { .. } => libraries.push(e),
                e @ ArchiveIndex::Book { .. } => books.push(e),
                e @ ArchiveIndex::Paper { .. } => papers.push(e),
                e @ ArchiveIndex::Course { .. } => courses.push(e),
                e @ ArchiveIndex::SelfStudy { .. } => self_studies.push(e),
            }
        }
        view! {
          {do_books(books)}
          {do_papers(papers)}
          {do_self_studies(self_studies)}
          {do_courses(courses,&is)}
          {do_libraries(libraries)}
        }
    })
}

fn wrap_list<V: IntoView + 'static>(
    ttl: &'static str,
    i: impl FnOnce() -> V,
) -> impl IntoView + 'static {
    use thaw::Divider;
    view! {
      <h2 style="color:var(--colorBrandForeground1)">{ttl}</h2>
      <div style="display:flex;flex-flow:wrap;">
      {i()}
      </div>
      <Divider/>
    }
}

fn link_doc<V: IntoView + 'static, T: FnOnce() -> V>(
    uri: &DocumentURI,
    i: T,
) -> impl IntoView + 'static + use<V, T> {
    view! {
      <a target="_blank" href=format!("/?uri={}",urlencoding::encode(&uri.to_string())) style="color:var(--colorBrandForeground1)">
        {i()}
      </a>
    }
}

fn do_img(url: String) -> impl IntoView {
    view!(<div style="width:100%"><div style="width:min-content;margin:auto;">
    <img src=url style="max-width:350px;max-height:150px;"/>
  </div></div>)
}

fn do_teaser(txt: String) -> impl IntoView {
    view!(<div style="margin:5px;"><Scrollbar style="max-height: 100px;"><Body1>
    <span inner_html=txt style="font-size:smaller;"/>
  </Body1></Scrollbar></div>)
}

fn do_books(books: Vec<ArchiveIndex>) -> impl IntoView {
    if books.is_empty() {
        return None;
    }
    Some(wrap_list("Books", move || {
        books.into_iter().map(book).collect_view()
    }))
}

fn book(book: ArchiveIndex) -> impl IntoView {
    let ArchiveIndex::Book {
        title,
        authors,
        file,
        teaser,
        thumbnail,
    } = book
    else {
        unreachable!()
    };
    view! {<Card class="flams-index-card">
      <CardHeader>
        {link_doc(&file,|| view!(<Body1><b inner_html=title.to_string()/></Body1>))}
        <CardHeaderDescription slot><Caption1>
          {if authors.is_empty() {None} else {Some(IntoIterator::into_iter(authors).map(|a| view!{{a.to_string()}<br/>}).collect_view())}}
        </Caption1>
        </CardHeaderDescription>
      </CardHeader>
      <CardPreview>
        {thumbnail.map(|t| do_img(t.to_string()))}
        {teaser.map(|t| do_teaser(t.to_string()))}
      </CardPreview>
    </Card>}
}

fn do_papers(papers: Vec<ArchiveIndex>) -> impl IntoView {
    if papers.is_empty() {
        return None;
    }
    Some(wrap_list("Papers", move || {
        papers.into_iter().map(paper).collect_view()
    }))
}

fn paper(paper: ArchiveIndex) -> impl IntoView {
    let ArchiveIndex::Paper {
        title,
        authors,
        file,
        teaser,
        thumbnail,
        venue,
        venue_url,
    } = paper
    else {
        unreachable!()
    };
    view! {<Card class="flams-index-card">
      <CardHeader>
        {link_doc(&file,|| view!(<Body1><b inner_html=title.to_string()/></Body1>))}
        <CardHeaderDescription slot><Caption1>
          {if authors.is_empty() {None} else {Some(IntoIterator::into_iter(authors).map(|a| view!{{a.to_string()}<br/>}).collect_view())}}
        </Caption1>
        </CardHeaderDescription>
        <CardHeaderAction slot>
        {venue.map(|v| {
          if let Some(url) = venue_url {
            leptos::either::Either::Left(view!(
              <a target="_blank" href=url.to_string() style="color:var(--colorBrandForeground1)">
                <b>{v.to_string()}</b>
              </a>
            ))
          } else {
            leptos::either::Either::Right(view!(<b>{v.to_string()}</b>))
          }
        })}
        </CardHeaderAction>
      </CardHeader>
      <CardPreview>
        {thumbnail.map(|t| do_img(t.to_string()))}
        {teaser.map(|t| do_teaser(t.to_string()))}
      </CardPreview>
    </Card>}
}

fn do_self_studies(sss: Vec<ArchiveIndex>) -> impl IntoView {
    if sss.is_empty() {
        return None;
    }
    Some(wrap_list("Self-Study Courses", move || {
        sss.into_iter().map(self_study).collect_view()
    }))
}

fn self_study(ss: ArchiveIndex) -> impl IntoView {
    let ArchiveIndex::SelfStudy {
        title,
        landing,
        acronym,
        notes,
        slides,
        thumbnail,
        teaser,
    } = ss
    else {
        unreachable!()
    };
    view! {<Card class="flams-index-card">
      <CardHeader>
        {link_doc(&landing,|| view!(
          <Body1><b><span inner_html=title.to_string()/>{acronym.map(|s| format!(" ({s})"))}</b></Body1>
        ))}
      </CardHeader>
      <CardPreview>
        {thumbnail.map(|t| do_img(t.to_string()))}
        {teaser.map(|t| do_teaser(t.to_string()))}
      </CardPreview>
      <div style="margin-top:auto;"/>
      <CardFooter>
        <Caption1>
          {link_doc(&notes,|| "Notes")}
          {slides.map(|s| view!(", "{link_doc(&s,|| "Slides")}))}
        </Caption1>
      </CardFooter>
    </Card>}
}

fn do_courses(
    courses: Vec<ArchiveIndex>,
    insts: &[Institution],
) -> impl IntoView + 'static + use<> {
    if courses.is_empty() {
        return None;
    }
    let r = courses.into_iter().map(|c| course(c, insts)).collect_view();
    Some(wrap_list("Courses", move || r))
}

fn course(course: ArchiveIndex, insts: &[Institution]) -> impl IntoView + 'static + use<> {
    let ArchiveIndex::Course {
        title,
        landing,
        acronym,
        instructors,
        institution,
        notes,
        slides,
        thumbnail,
        teaser,
        //quizzes,
        //homeworks,
        //instances,
        ..
    } = course
    else {
        unreachable!()
    };
    let inst = insts.iter().find(|i| i.acronym() == &*institution).cloned();
    view! {<Card class="flams-index-card">
      <CardHeader>
        {link_doc(&landing,|| view!(
          <Body1><b><span inner_html=title.to_string()/>{acronym.map(|s| format!(" ({s})"))}</b></Body1>
        ))}
        <CardHeaderDescription slot><Caption1>
          {if instructors.is_empty() {None} else {Some(IntoIterator::into_iter(instructors).map(|a| view!{{a.to_string()}<br/>}).collect_view())}}
        </Caption1>
        </CardHeaderDescription>
        <CardHeaderAction slot>{
          {inst.map(|inst| view!(
            <img style="max-width:50px;max-height:30px;" src=inst.logo().to_string() title=inst.title().to_string()/>
          ))}
        }</CardHeaderAction>
      </CardHeader>
      <CardPreview>
        {thumbnail.map(|t| do_img(t.to_string()))}
        {teaser.map(|t| do_teaser(t.to_string()))}
      </CardPreview>
      <div style="margin-top:auto;"/>
      <CardFooter>
        <Caption1>
          {link_doc(&notes,|| "Notes")}
          {slides.map(|s| view!(", "{link_doc(&s,|| "Slides")}))}
        </Caption1>
      </CardFooter>
    </Card>}
}

fn do_libraries(libs: Vec<ArchiveIndex>) -> impl IntoView {
    if libs.is_empty() {
        return None;
    }
    Some(wrap_list("Libraries", move || {
        libs.into_iter().map(library).collect_view()
    }))
}

fn library(lib: ArchiveIndex) -> impl IntoView {
    let ArchiveIndex::Library {
        archive,
        title,
        teaser,
        thumbnail,
    } = lib
    else {
        unreachable!()
    };
    view! {<Card class="flams-index-card">
      <CardHeader>
        <Body1><b inner_html=title.to_string()/></Body1>
        <CardHeaderDescription slot><Caption1>
          {archive.to_string()}
        </Caption1></CardHeaderDescription>
        /*{link_doc(&landing,|| view!(
          <Body1><b><span inner_html=title.to_string()/>{acronym.map(|s| format!(" ({s})"))}</b></Body1>
        ))}*/
      </CardHeader>
      <CardPreview>
        {thumbnail.map(|t| do_img(t.to_string()))}
        {teaser.map(|t| do_teaser(t.to_string()))}
      </CardPreview>
      <div style="margin-top:auto;"/>
    </Card>}
}
