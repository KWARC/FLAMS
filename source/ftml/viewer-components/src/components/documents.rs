use super::Gotto;
use super::TOCSource;
use crate::iterate;
use crate::FTMLDocumentSetup;
use flams_ontology::uris::NarrativeURI;
use flams_ontology::uris::{DocumentElementURI, DocumentURI, NameStep};
use flams_web_utils::components::wait_local;
use flams_web_utils::do_css;
use leptos::prelude::*;
use leptos_posthoc::DomStringCont;

#[cfg(feature = "omdoc")]
#[component]
pub fn DocumentFromURI(
    uri: DocumentURI,
    #[prop(optional, into)] toc: TOCSource,
    #[prop(optional, into)] gottos: Vec<Gotto>,
    #[prop(optional)] omdoc: crate::components::omdoc::OMDocSource,
) -> impl IntoView {
    wait_local(
        move || {
            tracing::info!("fetching {uri}");
            let fut = crate::remote::server_config.full_doc(uri.clone());
            async move { fut.await.ok() }
        },
        move |(uri, css, html)| {
            for c in css {
                do_css(c);
            }
            view!(<DocumentString html uri toc=toc.clone() gottos=gottos.clone() omdoc=omdoc.clone()/>)
        },
        "Error loading document reference".to_string(),
    )
}

#[component]
pub fn FragmentFromURI(uri: DocumentElementURI) -> impl IntoView {
    let uricl = uri.clone();
    wait_local(
        move || {
            tracing::info!("fetching {uri}");
            let fut = crate::remote::server_config.paragraph(uri.clone());
            async move { fut.await.ok() }
        },
        move |(_, css, html)| {
            for c in css {
                do_css(c);
            }
            view!(<FragmentString html uri=uricl.clone()/>)
        },
        "Error loading document fragment".to_string(),
    )
}

#[cfg(not(feature = "omdoc"))]
#[component]
pub fn DocumentFromURI(
    uri: DocumentURI,
    #[prop(optional, into)] toc: TOCSource,
    #[prop(optional, into)] gottos: Vec<Gotto>,
) -> impl IntoView {
    wait_local(
        move || {
            tracing::info!("fetching {uri}");
            let fut = crate::remote::server_config.full_doc(uri.clone());
            async move { fut.await.ok() }
        },
        move |(uri, css, html)| {
            for c in css {
                do_css(c);
            }
            view!(<DocumentString html uri gottos=gottos.clone() toc=toc.clone()/>)
        },
        "Error loading document reference".to_string(),
    )
}

#[component]
pub fn FragmentString(
    html: String,
    #[prop(optional)] uri: Option<DocumentElementURI>,
) -> impl IntoView {
    use leptos::context::Provider;
    use leptos::either::EitherOf3;
    let name = uri.as_ref().map(|uri| uri.name().last_name().clone());
    let needs_suffix = uri
        .as_ref()
        .map(|uri| uri.name().steps().len() > 1)
        .unwrap_or_default();
    let doc = uri
        .as_ref()
        .map_or_else(DocumentURI::no_doc, |d| d.document().clone());
    view! {<FTMLDocumentSetup uri=doc>{
        match name {
            Some(name) if needs_suffix => {
                let nuri = NarrativeURI::Element(flams_utils::unwrap!(uri).parent());
                EitherOf3::A(view!{
                    <Provider value=ForcedName(Some(name))>
                    <Provider value=nuri>
                        <DomStringCont html cont=iterate/>
                    </Provider>
                    </Provider>
                })
            },
            Some(name) => EitherOf3::B(view!{
                <Provider value=ForcedName(Some(name))>
                    <DomStringCont html cont=iterate/>
                </Provider>
            }),
            _ => EitherOf3::C(view!{
                <DomStringCont html cont=iterate/>
            })
        }
    }</FTMLDocumentSetup>}
}

#[derive(Clone, Debug, Default)]
pub struct ForcedName(Option<NameStep>);
impl ForcedName {
    pub fn update(&self, uri: &DocumentElementURI) -> DocumentElementURI {
        match self.0.as_ref() {
            Some(n) => {
                let name = uri.name().clone();
                let doc = uri.document().clone();
                doc & name.with_last_name(n.clone())
            }
            _ => uri.clone(),
        }
    }
}

#[cfg(feature = "omdoc")]
#[component]
pub fn DocumentString(
    html: String,
    #[prop(optional)] uri: Option<DocumentURI>,
    #[prop(optional, into)] toc: TOCSource,
    #[prop(optional, into)] gottos: Vec<Gotto>,
    #[prop(optional)] omdoc: crate::components::omdoc::OMDocSource,
) -> impl IntoView {
    let uri = uri.unwrap_or_else(DocumentURI::no_doc);
    let burger = !matches!(
        (&toc, &omdoc),
        (TOCSource::None, crate::components::omdoc::OMDocSource::None)
    );
    view! {<FTMLDocumentSetup uri>
        {
            if burger {Some(
                do_burger(toc,gottos,omdoc)
            )}
            else { None }
        }
        <DomStringCont html cont=iterate/>
    </FTMLDocumentSetup>}
}

#[cfg(not(feature = "omdoc"))]
#[component]
pub fn DocumentString(
    html: String,
    #[prop(optional)] uri: Option<DocumentURI>,
    #[prop(optional, into)] toc: TOCSource,
    #[prop(optional, into)] gottos: Vec<Gotto>,
) -> impl IntoView {
    let uri = uri.unwrap_or_else(DocumentURI::no_doc);
    let burger = !matches!(toc, TOCSource::None);
    view! {<FTMLDocumentSetup uri>
        {if burger {Some(
            do_burger(toc,gottos)
        )}
        else { None }}
        <DomStringCont html cont=iterate/>
    </FTMLDocumentSetup>}
}

#[cfg(feature = "omdoc")]
fn do_burger(
    toc: TOCSource,
    gottos: Vec<Gotto>,
    omdoc: crate::components::omdoc::OMDocSource,
) -> impl IntoView {
    use flams_web_utils::components::Burger;
    //use flams_web_utils::components::ClientOnly;
    crate::components::do_toc(toc, gottos, move |v| {
        view! {
            /*<ClientOnly>
                <div style="width:0;height:0;margin-left:auto;">
                    <div style="position:fixed">
                        {crate::components::omdoc::do_omdoc(omdoc)}
                        <div style="width:fit-content;height:fit-content;">{v}</div>
                    </div>
                </div>
            </ClientOnly>*/
            <Burger>{crate::components::omdoc::do_omdoc(omdoc)}{v}</Burger>
        }
    })
}

#[cfg(not(feature = "omdoc"))]
fn do_burger(toc: crate::components::TOCSource, gottos: Vec<Gotto>) -> impl IntoView {
    use flams_web_utils::components::Burger;
    //use flams_web_utils::components::ClientOnly;
    crate::components::do_toc(toc, gottos, move |v| {
        view! {
           //<ClientOnly> <div>{v}</div></ClientOnly>
            <Burger>{v}</Burger>
        }
    })
}
