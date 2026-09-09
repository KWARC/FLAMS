#![allow(clippy::must_use_candidate)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![recursion_limit = "256"]

#[cfg(any(
    all(feature = "ssr", feature = "hydrate", not(feature = "docs-only")),
    not(any(feature = "ssr", feature = "hydrate"))
))]
compile_error!("exactly one of the features \"ssr\" or \"hydrate\" must be enabled");

pub mod math;
mod module_picker;

use flams_router_base::maybe_lazy;
use flams_router_content::Views;
use ftml_backend::{FtmlBackend, GlobalBackend};
use ftml_dom::{FtmlViews, utils::css::CssExt};
use ftml_ontology::{
    domain::{HasDeclarations, declarations::AnyDeclarationRef, modules::ModuleLike},
    utils::Css,
};
use ftml_uris::{DocumentUri, Id, ModuleUri, SymbolUri};
use leptos::prelude::*;

maybe_lazy!(FloDownEditor = flodown_editor());

//#[component]
pub fn flodown_editor() -> AnyView {
    #[cfg(feature = "hydrate")]
    math::TeXClient::provide();

    Css::Link("/rustex2.css".to_string().into_boxed_str()).inject();
    Css::Link(
        "https://fonts.googleapis.com/css2?family=STIX+Two+Text"
            .to_string()
            .into_boxed_str(),
    )
    .inject();

    let modules = RwSignal::new(rustc_hash::FxHashSet::default());
    let symbols = RwSignal::new(rustc_hash::FxHashMap::default());
    let action = Action::new(move |v: &rustc_hash::FxHashSet<_>| {
        let v = v.iter().cloned().collect();
        async move {
            let syms = get_symbols(v).await;
            symbols.set(syms);
        }
    });
    let _ = Effect::new(move || {
        modules.with(|modules| {
            action.dispatch(modules.clone());
        });
    });

    view! {
        {editor(symbols)}
        <div style="width:100%;display:flex;flex-direction:row;">
            <div style="width:45%;border:1px solid black;">
                <strong>Symbols:</strong>
                <span>
                {move ||
                    symbols.with(|s| {
                        let mut s = s.iter().collect::<Vec<_>>();
                        s.sort_by_key(|(a,_)| *a);
                        ftml_components::components::content::CommaSep("",
                            s.into_iter().map(|(id,uri)| ftml_components::components::content::symbol_uri(id.to_string(), uri))
                        ).into_view().attr("style", "display:inline;")
                    }   )
                }
                </span>
            </div>
            <div style="width:45%">
                {module_picker::picker(modules)}
            </div>
        </div>
    }.into_any()
}

async fn get_symbols(mut todos: Vec<ModuleUri>) -> rustc_hash::FxHashMap<Id, SymbolUri> {
    let mut dones = rustc_hash::FxHashSet::default();
    let mut ret = rustc_hash::FxHashMap::default();
    while let Some(next) = todos.pop() {
        if dones.contains(&next) {
            continue;
        }
        dones.insert(next.clone());
        if let Ok(ModuleLike::Module(m)) = flams_router_content::backend::FtmlBackend::get()
            .get_module(next)
            .await
        {
            for d in m.declarations() {
                match d {
                    AnyDeclarationRef::Symbol(s) => {
                        ret.insert(
                            unsafe { s.uri.name().as_ref().parse().unwrap_unchecked() },
                            s.uri.clone(),
                        );
                        if let Some(mac) = &s.data.macroname {
                            ret.insert(mac.clone(), s.uri.clone());
                        }
                    }
                    AnyDeclarationRef::MathStructure(s) => {
                        ret.insert(
                            unsafe { s.uri.name().as_ref().parse().unwrap_unchecked() },
                            s.uri.clone(),
                        );
                        if let Some(mac) = &s.macroname {
                            ret.insert(mac.clone(), s.uri.clone());
                        }
                    }
                    AnyDeclarationRef::Import { uri: m, .. } => {
                        todos.push(m.clone());
                    }
                    _ => (),
                }
            }
        }
    }
    ret
}

fn editor(symbols: RwSignal<rustc_hash::FxHashMap<Id, SymbolUri>>) -> AnyView {
    let csr = RwSignal::new(false);
    #[cfg(feature = "hydrate")]
    let _ = Effect::new(move || csr.set(true));
    let checked = RwSignal::new(false);
    let text = RwSignal::new(DEMO.to_string());

    //ftml_components::config::FtmlConfig::set_toc_source(ftml_dom::structure::TocSource::None);
    Views::setup_document(
        DocumentUri::no_doc().clone(),
        ftml_components::SidebarPosition::None,
        false,
        ftml_dom::toc::TocSource::None,
        move || {
            view! {
                <div><input type="checkbox" on:change:target=move |ev| {
                      checked.set(ev.target().checked());
                }/>"LaTeX"</div>
                <div style="width:100%;display:flex;flex-direction:row;">
                    <textarea
                        style="width:48%;max-width:48%;min-width:48%;min-height:200px;"
                        on:input:target=move |ev| {
                            text.set(ev.target().value());
                        }
                        prop:value=text
                    />
                    <div
                        style="width:48%;max-width:48%;min-width:48%;text-align:left;border:1px solid black"
                        //inner_html=move || text.with(|txt| flodown::to_html(txt))
                    >
                        {move ||
                            if csr.get() {
                                Some(if checked.get() {
                                    view!(<pre>{text.with(|txt| flodown::to_latex(txt))}</pre>).into_any()
                                } else {
                                    md_html(text,symbols)
                                })
                            } else {
                                None
                            }
                        }
                    </div>
                </div>
            }.into_any()
        },
    )
}

fn md_html(
    md: RwSignal<String>,
    symbols: RwSignal<rustc_hash::FxHashMap<Id, SymbolUri>>,
) -> AnyView {
    let owner = leptos::prelude::Owner::current().expect("not in a reactive context");

    let actual = RwSignal::new(String::new());
    let signals = RwSignal::new(Vec::<(usize, RwSignal<Option<Result<String, String>>>)>::new());
    Effect::new(move || {
        #[cfg(feature = "hydrate")]
        {
            signals.update_untracked(Vec::clear);
            math::TeXClient::reset();
            let s = md.with(|txt| {
                symbols.with(|symbols| {
                    flodown::to_html_with_math_and_symbols(
                        txt,
                        symbols,
                        |s, out| {
                            use std::fmt::Write;
                            let (i, rs) = owner.with(|| math::TeXClient::inline_math(s));
                            signals.update_untracked(|v| v.push((i, rs)));
                            let _ = write!(out, "<!--math{i}--> ...");
                        },
                        |s, out| {
                            use std::fmt::Write;
                            let (i, rs) = owner.with(|| math::TeXClient::block_math(s));
                            signals.update_untracked(|v| v.push((i, rs)));
                            let _ = write!(out, "<!--math{i}--> ...");
                        },
                    )
                })
            });
            actual.set(s);
            signals.notify();
        }
    });
    Effect::new(move || {
        #[cfg(feature = "hydrate")]
        {
            signals.with(|v| {
                for (i, sig) in v {
                    if let Some(v) = sig.get() {
                        match v {
                            Ok(s) => actual.update_untracked(|a| {
                                *a = a.replace(&format!("<!--math{i}--> ..."), &s)
                            }),
                            Err(e) => actual.update_untracked(|a| {
                                *a = a.replace(
                                    &format!("<!--math{i}--> ..."),
                                    &format!("<span style=\"background-color:red;\">{e}</span>"),
                                );
                            }),
                        }
                    }
                }
            });
            actual.notify();
        }
    });
    (move || actual.with(|txt| Views::render_ftml(txt.clone(), None))).into_any()
}

static DEMO: &str = r#"
::: definition title="Gödel's Incompleteness Theorem"
  foo @[sym](CS) @[sym](CS,computer science)
  @[def](uncertain)
:::

@[definition](this is a @[sym](mind) inline definition for @[def](CS,computer science))

a *b* **blubb** \*bla\* blubb _bla_ __blubb__ ~bla~ ^blubb^ ~~bla~~ ==blubb==
$inline math$ and $$block math$$ and such, and `inline code` and
```javascript
some
  code blocks
    with
  indentation
```

go visit [mathhub](https://mathhub.info)

- foo
  - bar

- blubb

1. btw
1. this works
  1. too
  1. and this
1) and this

foo bar
"#;
