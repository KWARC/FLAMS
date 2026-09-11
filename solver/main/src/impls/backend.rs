use crate::{CheckRef, Checker, impls::solving::is_solvable_var, split::SplitStrategy};
use dashmap::DashSet;
use flams_math_archives::backend::{AnyBackend, LocalBackend};
use ftml_ontology::{
    domain::{
        HasDeclarations, SharedDeclaration,
        declarations::{IsDeclaration, SharedSymbolLike, symbols::Symbol},
        modules::{Module, ModuleLike},
    },
    narrative::{SharedDocumentElement, documents::Document, elements::VariableDeclaration},
    terms::{ComponentVar, Term, Variable},
};
use ftml_uris::{DocumentElementUri, IsDomainUri, IsNarrativeUri, LeafUri, ModuleUri, SymbolUri};
use std::hint::unreachable_unchecked;

pub fn get_variable(
    backend: &AnyBackend,
    documents: &DashSet<Document, rustc_hash::FxBuildHasher>,
    current: &[LeafUri],
    uri: &DocumentElementUri,
    prepare: impl Fn(Term) -> Term,
) -> Result<SharedDocumentElement<VariableDeclaration>, ()> {
    fn get(
        backend: &AnyBackend,
        documents: &DashSet<Document, rustc_hash::FxBuildHasher>,
        uri: &DocumentElementUri,
    ) -> Result<SharedDocumentElement<VariableDeclaration>, ()> {
        let doc = uri.document_uri();
        if let Some(d) = documents.get(doc) {
            return d.get_as(uri.name()).ok_or(());
        }
        let doc = backend.get_document(doc).map_err(|_| ())?;
        documents.insert(doc.clone());
        doc.get_as(uri.name()).ok_or(())
    }

    if current.iter().any(|u| u == uri) {
        return Err(());
    }
    let d = get(backend, documents, uri)?;

    if let Some(tp) = d.data.tp.get_parsed()
        && !d.data.tp.has_checked()
    {
        let tp = prepare(tp.clone());

        if d.data.is_seq && tp.as_sequence_type().is_none() {
            if d.data.sequence_range.is_empty() {
                d.data.tp.set_checked(tp.into_seq_type());
            } else {
                d.data
                    .tp
                    .set_checked(tp.into_ranged_seq_type(d.data.sequence_range.iter().cloned()));
            }
        } else {
            d.data.tp.set_checked(tp);
        }
    }
    if let Some(df) = d.data.df.get_parsed()
        && !d.data.df.has_checked()
    {
        d.data.df.set_checked(prepare(df.clone()));
    }

    Ok(d)
}

impl<Split: SplitStrategy> Checker<Split> {
    pub(crate) fn get_module_like(&self, uri: &ModuleUri) -> Result<ModuleLike, ()> {
        if uri.is_top() {
            if let Some(m) = self.modules.get(uri) {
                return Ok(ModuleLike::Module(m.clone()));
            }
            let ModuleLike::Module(m) = self.backend.get_module(uri).map_err(|_| ())? else {
                // SAFETY: uri.is_top()
                unsafe { unreachable_unchecked() }
            };
            m.initialize(&mut |u| self.get_module_like(u).ok())
                .map_err(|_| ())?;
            self.modules.insert(m.clone());
            Ok(ModuleLike::Module(m))
        } else {
            // SAFETY: !uri.is_top()
            let inner = unsafe { uri.clone().into_top_symbol().unwrap_unchecked() };
            let m = self.get_module(inner.module_uri())?;
            m.as_module_like(inner.name()).ok_or(())
        }
    }
    pub(crate) fn get_module(&self, uri: &ModuleUri) -> Result<Module, ()> {
        if uri.is_top() {
            if let Some(m) = self.modules.get(uri) {
                return Ok(m.clone());
            }
            let ModuleLike::Module(m) = self.backend.get_module(uri).map_err(|_| ())? else {
                // SAFETY: uri.is_top()
                unsafe { unreachable_unchecked() }
            };
            self.modules.insert(m.clone());
            Ok(m)
        } else {
            let uri = !uri.clone();
            if let Some(m) = self.modules.get(&uri) {
                return Ok(m.clone());
            }
            let ModuleLike::Module(m) = self.backend.get_module(&uri).map_err(|_| ())? else {
                // SAFETY: uri = !uri enforces top-level
                unsafe { unreachable_unchecked() }
            };
            self.modules.insert(m.clone());
            Ok(m)
        }
    }
    pub fn get_symbol_like(
        &self,
        uri: &SymbolUri,
        prepare: impl Fn(Term) -> Term,
    ) -> Result<SharedSymbolLike, ()> {
        if self.current.iter().any(|u| u == uri) {
            return Err(());
        }
        let uri = uri.as_simple_module();

        let Some(d) = self
            .get_module(&uri.module)
            .map_err(|_| ())?
            .get_symbol_like(uri.name())
        else {
            return Err(());
        };
        if let SharedSymbolLike::Symbol(d) = &d {
            if let Some(tp) = d.data.tp.get_parsed()
                && !d.data.tp.has_checked()
            {
                d.data.tp.set_checked(prepare(tp.clone()));
            }
            if let Some(df) = d.data.df.get_parsed()
                && !d.data.df.has_checked()
            {
                d.data.df.set_checked(prepare(df.clone()));
            }
        }
        Ok(d)
    }

    pub(crate) fn get_symbol(
        &self,
        uri: &SymbolUri,
        prepare: impl Fn(Term) -> Term,
    ) -> Result<SharedDeclaration<Symbol>, ()> {
        if self.current.iter().any(|u| u == uri) {
            return Err(());
        }
        let uri = uri.as_simple_module();
        let Some(d) = self.get_module(&uri.module)?.get_as::<Symbol>(uri.name()) else {
            return Err(());
        };
        if let Some(tp) = d.data.tp.get_parsed()
            && !d.data.tp.has_checked()
        {
            d.data.tp.set_checked(prepare(tp.clone()));
        }
        if let Some(df) = d.data.df.get_parsed()
            && !d.data.df.has_checked()
        {
            d.data.df.set_checked(prepare(df.clone()));
        }
        Ok(d)
        //.ok_or();
    }

    pub(crate) fn get_variable(
        &self,
        uri: &DocumentElementUri,
    ) -> Result<SharedDocumentElement<VariableDeclaration>, ()> {
        get_variable(&self.backend, &self.documents, &self.current, uri, |t| {
            self.prepare(None, t).1
        })
        /*
        fn get<Split: SplitStrategy>(
            slf: &Checker<Split>,
            uri: &DocumentElementUri,
        ) -> Result<SharedDocumentElement<VariableDeclaration>, ()> {
            let doc = uri.document_uri();
            if let Some(d) = slf.documents.get(doc) {
                return d.get_as(uri.name()).ok_or(());
            }
            let doc = slf.backend.get_document(doc).map_err(|_| ())?;
            slf.documents.insert(doc.clone());
            doc.get_as(uri.name()).ok_or(())
        }
        if self.current.iter().any(|u| u == uri) {
            return Err(());
        }
        let d = get(self, uri)?;

        if let Some(tp) = d.data.tp.get_parsed()
            && !d.data.tp.has_checked()
        {
            let (_, tp) = self.prepare(None, tp.clone());

            if d.data.is_seq && tp.as_sequence_type().is_none() {
                d.data.tp.set_checked(tp.into_seq_type());
            } else {
                d.data.tp.set_checked(tp);
            }
        }
        if let Some(df) = d.data.df.get_parsed()
            && !d.data.df.has_checked()
        {
            d.data.df.set_checked(self.prepare(None, df.clone()).1);
        }

        Ok(d)
         */
    }
}

impl<Split: SplitStrategy> CheckRef<'_, '_, Split> {
    /// ### Errors
    pub(crate) fn get_declaration<T: IsDeclaration>(
        &self,
        uri: &SymbolUri,
    ) -> Result<SharedDeclaration<T>, ()> {
        self.top
            .get_module(&uri.module)?
            .get_as::<T>(uri.name())
            .ok_or(())
    }

    /// ### Errors
    #[inline]
    pub(crate) fn get_symbol(&self, uri: &SymbolUri) -> Result<SharedDeclaration<Symbol>, ()> {
        self.top.get_symbol(uri, |t| self.prepare(t, None).1)
    }

    pub(crate) fn get_symbol_type(&mut self, uri: &SymbolUri) -> Option<Term> {
        let Ok(s) = self.top.get_symbol_like(uri, |t| self.prepare(t, None).1) else {
            self.failure(format!("Symbol not found: {uri}"));
            return None;
        };
        match s {
            SharedSymbolLike::Symbol(s) => s.data.tp.checked_or_parsed().map(|(t, _)| t),
            SharedSymbolLike::MathStructure(_) => {
                self.top.rules.inference().iter().find_map(|rl| {
                    rl.as_any()
                        .downcast_ref::<super::records::RecordUniverse>()
                        .map(|rl| rl.0.clone())
                })
            }
            _ => None,
        }

        /*
        let Ok(s) = self.get_symbol(uri) else {
            self.failure(format!("Symbol not found: {uri}"));
            return None;
        };
        s.data.tp.checked_or_parsed().map(|(t, _)| t)
         */
    }
    pub(crate) fn get_symbol_definiens(&mut self, uri: &SymbolUri) -> Option<Term> {
        let Ok(s) = self.get_symbol(uri) else {
            self.failure(format!("Symbol not found: {uri}"));
            return None;
        };

        /*
        if uri.name.as_ref() == "universal quantifier" {
            use std::io::{Read, Write};
            let bt = std::backtrace::Backtrace::force_capture();
            let bts = bt.to_string();
            if !bts.contains("simplify_implicit") {
                println!("----------------------------------------");
                for l in bts.split('\n').rev() {
                    println!("{l}");
                }
                std::io::stdout().flush();
                let _ = std::io::stdin().read(&mut [0]);
            }
        } */
        let t = s.data.df.checked_or_parsed();
        if t.is_none() {
            self.comment(format!("Symbol has no definiens: {uri}"));
        }
        t.map(|(t, _)| t)
    }

    /// ### Errors
    #[inline]
    pub fn get_variable(
        &self,
        uri: &DocumentElementUri,
    ) -> Result<SharedDocumentElement<VariableDeclaration>, ()> {
        self.top.get_variable(uri)
    }

    pub(crate) fn get_var_definiens(&mut self, var: &Variable) -> Option<Term> {
        if let Some(id) = is_solvable_var(var) {
            return self.get_solution(id);
        }
        for v in self.iter_context() {
            match (v, var) {
                (
                    ComponentVar {
                        var: Variable::Name { name, .. },
                        df,
                        ..
                    },
                    Variable::Name { name: n2, .. },
                ) if *name == *n2 => {
                    return df.clone().map(|t| self.subst(t));
                }
                (
                    ComponentVar {
                        var: Variable::Name { name, .. },
                        df,
                        ..
                    },
                    Variable::Ref { declaration, .. },
                ) if name.as_ref() == declaration.name().last() && df.is_some() => {
                    return df.clone().map(|t| self.subst(t));
                }
                (
                    ComponentVar {
                        var: Variable::Ref { declaration, .. },
                        df,
                        ..
                    },
                    Variable::Name { name, .. },
                ) if name.as_ref() == declaration.name().last() => {
                    return if df.is_some() {
                        df.clone().map(|t| self.subst(t))
                    } else {
                        self.get_variable(declaration)
                            .ok()?
                            .data
                            .df
                            .checked_or_parsed()
                            .map(|(t, _)| t)
                    };
                }
                (
                    ComponentVar {
                        var: Variable::Ref { declaration, .. },
                        df,
                        ..
                    },
                    Variable::Ref {
                        declaration: d2, ..
                    },
                ) if *declaration == *d2 => {
                    return if df.is_some() {
                        df.clone().map(|t| self.subst(t))
                    } else {
                        self.get_variable(declaration)
                            .ok()?
                            .data
                            .df
                            .checked_or_parsed()
                            .map(|(t, _)| t)
                    };
                }
                _ => (),
            }
        }
        if let Variable::Ref { declaration, .. } = var {
            self.get_variable(declaration)
                .ok()?
                .data
                .df
                .checked_or_parsed()
                .map(|(t, _)| t)
        } else {
            None
        }
    }
}
