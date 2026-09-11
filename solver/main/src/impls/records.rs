use std::{borrow::Cow, ops::ControlFlow};

use ftml_ontology::{
    domain::{
        HasDeclarations, SharedDeclaration,
        declarations::{AnyDeclarationRef, SharedSymbolLike, symbols::Symbol},
        modules::ModuleLike,
    },
    terms::{ApplicationTerm, Argument, MaybeSequence, RecordFieldTerm, Term},
};
use ftml_solver_trace::SizedSolverRule;
use ftml_uris::{DomainUriRef, Id, ModuleUri, SymbolUri, UriName};

use crate::{
    CheckRef,
    rules::{InferenceRule, InhabitableRule, SimplificationRule, SubtypeRule},
    split::SplitStrategy,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FieldRule;
impl SizedSolverRule for FieldRule {
    fn priority(&self) -> isize {
        100_000
    }
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("record field rule")
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for FieldRule {
    fn applicable(&self, term: &Term) -> bool {
        matches!(term, Term::Field(_))
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        let Term::Field(app) = term else {
            unreachable!("by applicability")
        };
        let record = Record::from_term(&app.record, app.record_type.as_ref(), &mut checker)
            .map_err(|_| None)?;
        record.get_def(&app.key).ok_or(None)
    }
}
impl<Split: SplitStrategy> InferenceRule<Split> for FieldRule {
    fn applicable(&self, term: &Term) -> bool {
        matches!(term, Term::Field(_))
    }
    fn infer<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        let Term::Field(app) = term else {
            unreachable!("by applicability")
        };
        let record = Record::from_term(&app.record, app.record_type.as_ref(), &mut checker).ok()?;
        record.get_tp(&app.key)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct RecordRule;
impl SizedSolverRule for RecordRule {
    fn priority(&self) -> isize {
        100_000
    }
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!("record rule")
    }
}
impl RecordRule {
    fn is_sub(sub: &AnyRecordType, sup: &AnyRecordType) -> bool {
        match (sub, sup) {
            (AnyRecordType::Structure(sub), AnyRecordType::Structure(sup)) => {
                sup.domain.iter().all(|d| sub.domain.contains(d))
            }
            (AnyRecordType::Ext(sub), AnyRecordType::Structure(sup)) => {
                sup.domain.iter().all(|d| {
                    sub.elems.iter().any(|e| {
                        if let AnyRecordType::Structure(sub) = e {
                            sub.domain.contains(d)
                        } else {
                            false
                        }
                    })
                })
            }
            _ => false,
        }
    }
}
impl<Split: SplitStrategy> InhabitableRule<Split> for RecordRule {
    fn applicable(&self, term: &Term) -> bool {
        AnyRecordType::may_be(term)
    }
    fn apply<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool> {
        AnyRecordType::from_term(term, &mut checker).ok()?;
        Some(true)
    }
}
impl<Split: SplitStrategy> SubtypeRule<Split> for RecordRule {
    fn applicable(&self, checker: &CheckRef<'_, '_, Split>, sub: &Term, sup: &Term) -> bool {
        AnyRecordType::may_be(sub) && AnyRecordType::may_be(sup)
    }
    fn apply<'t>(
        &self,
        mut checker: CheckRef<'t, '_, Split>,
        sub: &'t Term,
        sup: &'t Term,
    ) -> Option<bool> {
        let sub = AnyRecordType::from_term(sub, &mut checker).ok()?;
        let sup = AnyRecordType::from_term(sup, &mut checker).ok()?;
        if Self::is_sub(&sub, &sup) {
            Some(true)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordUniverse(pub Term);
impl SizedSolverRule for RecordUniverse {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(self.0.clone(), "is universe for records")
    }
}
impl<Split: SplitStrategy> InferenceRule<Split> for RecordUniverse {
    fn applicable(&self, term: &Term) -> bool {
        AnyRecordType::may_be(term)
    }
    fn infer<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<Term> {
        fn is<Split: SplitStrategy>(checker: &CheckRef<Split>, term: &Term) -> bool {
            match term {
                Term::Symbol { uri, .. } => {
                    matches!(
                        checker
                            .top
                            .get_symbol_like(uri, |t| checker.prepare(t, None).1),
                        Ok(SharedSymbolLike::MathStructure(_))
                    )
                }
                Term::Application(app)
                    if app.head.is(&*ftml_uris::metatheory::ANONYMOUS_RECORD) =>
                {
                    let [Argument::Sequence(MaybeSequence::Seq(omls))] = &*app.arguments else {
                        return false;
                    };
                    omls.iter().all(|o| matches!(o, Term::Label { .. }))
                }
                Term::Application(app)
                    if app.head.is(&*ftml_uris::metatheory::RECORD_TYPE_MERGE) =>
                {
                    let [Argument::Sequence(MaybeSequence::Seq(recs))] = &*app.arguments else {
                        return false;
                    };
                    // TODO check that record is well-typed
                    recs.iter().all(|r| is(checker, r))
                }
                _ => false,
            }
        }
        if is(&checker, term) {
            let r = AnyRecordType::from_term(term, &mut checker).ok()?;
            if r.is_complete() {
                match r {
                    // return the first structure as the type
                    AnyRecordType::Ext(e)
                        if e.elems.iter().any(|e| matches!(e, AnyRecordType::Ano(_)))
                            && e.elems
                                .iter()
                                .any(|e| matches!(e, AnyRecordType::Structure(_))) =>
                    {
                        let first = e.elems.iter().find_map(|e| {
                            if let AnyRecordType::Structure(s) = e {
                                s.rec_type.clone().into_top_symbol()
                            } else {
                                None
                            }
                        })?;
                        Some(first.into())
                    }
                    _ => Some(self.0.clone()),
                }
            } else {
                Some(self.0.clone())
            }
        } else {
            None
        }
    }
}

pub struct Record {
    term: Term,
    typ: AnyRecordType,
}
impl Record {
    #[inline]
    pub const fn get_type(&self) -> &AnyRecordType {
        &self.typ
    }
    pub fn from_term<'t, Split: SplitStrategy>(
        t: &'t Term,
        tp: Option<&'t Term>,
        checker: &mut CheckRef<'t, '_, Split>,
    ) -> Result<Self, Option<ModuleUri>> {
        /*let t = checker
        .simplify_until(t, |_, t| AnyRecordType::may_be(t))
        .unwrap_or_else(|| Cow::Borrowed(t));*/
        if let Some(tp) = tp
            && checker.check_type(t, tp) != Some(true)
        {
            return Err(None);
        }

        let tp = /*if let Some(tp) = tp {
            if checker.check_type(t, tp) != Some(true) {
                return Err(None);
            }
            AnyRecordType::from_term(tp, checker)?
        } else*/ {
            let Some(tp) = checker.infer_type(t) else {
                checker.failure("Could not determine record type");
                return Err(None);
            };
            checker.scoped(|checker| AnyRecordType::from_term(&tp, checker))?
        };
        Ok(Self {
            term: t.clone(),
            typ: tp,
        })
    }

    pub fn get_def(&self, field: &UriName) -> Option<Term> {
        self.typ.get_def(&self.term, field)
    }
    pub fn get_tp(&self, field: &UriName) -> Option<Term> {
        self.typ.get_tp(&self.term, field)
    }
}

pub enum AnyRecordType {
    Structure(ModuleType),
    Ano(AnonymousRecord),
    Ext(RecordExtension),
}
impl AnyRecordType {
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Structure(s) => s.is_complete(),
            Self::Ano(a) => a.is_complete(),
            Self::Ext(e) => e.is_complete(),
        }
    }
    pub fn get_symbol(&self, name: &UriName) -> Option<SharedDeclaration<Symbol>> {
        match self {
            Self::Structure(mt) => mt
                .fields
                .iter()
                .rev()
                .find(|sd| name_fits(sd, name))
                .cloned(),
            Self::Ano(_) => None,
            Self::Ext(e) => e
                .elems
                .iter()
                .rev()
                .filter_map(|e| {
                    if let Self::Structure(m) = e {
                        Some(m)
                    } else {
                        None
                    }
                })
                .flat_map(|m| m.fields.iter().rev())
                .find(|sd| name_fits(sd, name))
                .cloned(),
        }
    }
    pub fn may_be(term: &Term) -> bool {
        match term {
            Term::Symbol { .. } => true,
            Term::Application(app) => {
                (app.head.is(&*ftml_uris::metatheory::ANONYMOUS_RECORD)
                    || app.head.is(&*ftml_uris::metatheory::RECORD_TYPE_MERGE))
                    && matches!(&*app.arguments, [Argument::Sequence(MaybeSequence::Seq(_))])
            }
            _ => false,
        }
    }
    pub fn from_term<'t, Split: SplitStrategy>(
        t: &'t Term,
        checker: &mut CheckRef<'t, '_, Split>,
    ) -> Result<Self, Option<ModuleUri>> {
        let t = checker
            .simplify_until(t, |_, t| Self::may_be(t))
            .unwrap_or_else(|| Cow::Borrowed(t));
        match &*t {
            Term::Symbol { uri, .. } => ModuleType::new(uri.clone(), checker).map(Self::Structure),
            Term::Application(app) => checker.scoped(|checker| {
                RecordExtension::from_app(app, checker)
                    .map(Self::Ext)
                    .or_else(|_| AnonymousRecord::from_app(app, checker).map(Self::Ano))
            }),
            _ => Err(None),
        }
    }

    pub fn get_def(&self, record: &Term, field: &UriName) -> Option<Term> {
        match self {
            Self::Structure(s) => s.get_def(record, field),
            Self::Ext(s) => s.get_def(record, field),
            Self::Ano(s) => s.get_def(record, field),
        }
    }
    pub fn get_tp(&self, record: &Term, field: &UriName) -> Option<Term> {
        match self {
            Self::Structure(s) => s.get_tp(record, field),
            Self::Ext(s) => s.get_tp(record, field),
            Self::Ano(s) => s.get_tp(record, field),
        }
    }
}

struct Field {
    name: UriName,
    id: Option<Id>,
    tp: Option<Term>,
    df: Option<Term>,
}

pub struct AnonymousRecord {
    fields: Vec<Field>,
}
impl AnonymousRecord {
    pub fn is_complete(&self) -> bool {
        self.fields.iter().all(|f| f.df.is_some())
    }
    pub fn from_app<'t, Split: SplitStrategy>(
        app: &'t ApplicationTerm,
        _: &mut CheckRef<'t, '_, Split>,
    ) -> Result<Self, Option<ModuleUri>> {
        if !app.head.is(&*ftml_uris::metatheory::ANONYMOUS_RECORD) {
            return Err(None);
        }
        let [Argument::Sequence(MaybeSequence::Seq(omls))] = &*app.arguments else {
            return Err(None);
        };
        let mut fields = Vec::new();
        for o in omls {
            let Term::Label { name, df, tp } = o else {
                return Err(None);
            };
            fields.push(Field {
                name: name.clone(),
                id: None,
                tp: tp.as_ref().map(|t| (**t).clone()),
                df: df.as_ref().map(|t| (**t).clone()),
            });
        }
        Ok(Self { fields })
    }

    pub fn get_def(&self, _: &Term, field: &UriName) -> Option<Term> {
        self.fields.iter().find_map(|f| {
            if name_fits_i(field, &f.name, f.id.as_ref()) {
                f.df.clone()
            } else {
                None
            }
        })
    }
    pub fn get_tp(&self, _: &Term, field: &UriName) -> Option<Term> {
        self.fields.iter().find_map(|f| {
            if name_fits_i(field, &f.name, f.id.as_ref()) {
                f.tp.clone()
            } else {
                None
            }
        })
    }
}

pub struct ModuleType {
    //term: &'s Term,
    rec_type: ModuleUri,
    domain: Vec<ModuleUri>,
    fields: Vec<SharedDeclaration<Symbol>>,
}
impl ModuleType {
    pub fn is_complete(&self) -> bool {
        self.fields.iter().all(|f| f.data.df.is_some())
    }
    pub fn new<Split: SplitStrategy>(
        structure: SymbolUri,
        checker: &mut CheckRef<Split>,
    ) -> Result<Self, Option<ModuleUri>> {
        let as_mod = structure.into_module();
        let mut mods = Vec::new();
        let mut err = None;
        ModuleLike::topo_sort(vec![as_mod.clone()], &mut mods, |u| {
            if let Ok(r) = checker.top.get_module_like(u) {
                Some(r)
            } else {
                err = Some(u.clone());
                None
            }
        });
        if let Some(err) = err {
            return Err(Some(err));
        }
        let mut fields = Vec::new();
        let mut domain = Vec::new();
        for m in mods {
            domain.push(match m.domain_uri() {
                DomainUriRef::Module(m) => m.clone(),
                DomainUriRef::Symbol(s) => s.clone().into_module(),
            });
            if let ModuleLike::Morphism(m) = &m {
                domain.push(m.domain.clone());
            }
            for d in m.declarations() {
                if let AnyDeclarationRef::Symbol(sym) = d {
                    unsafe {
                        match &m {
                            ModuleLike::Module(_) | ModuleLike::Nested(_) => (),
                            ModuleLike::Structure(m) => fields.push(m.inherit_unsafe(sym)),
                            ModuleLike::Extension(m) => fields.push(m.inherit_unsafe(sym)),
                            ModuleLike::Morphism(m) => fields.push(m.inherit_unsafe(sym)),
                        }
                    };
                }
            }
        }
        Ok(Self {
            rec_type: as_mod,
            domain,
            fields,
        })
    }
    fn subst<'t>(&self, record: &Term, tm: &'t Term) -> Cow<'t, Term> {
        tm.modify(|t| {
            let Term::Symbol { uri, .. } = t else {
                return None;
            };
            self.fields.iter().find_map(|s| {
                if s.uri.equivalent(uri) {
                    // SAFTEY: last names are valie
                    let name = unsafe { s.uri.name.last().parse().unwrap_unchecked() };
                    // SAFTEY: self.rec_type is a structure => nested URI
                    let tp = unsafe { self.rec_type.clone().into_top_symbol().unwrap_unchecked() };
                    Some(ControlFlow::Break(Term::Field(RecordFieldTerm::new(
                        record.clone(),
                        name,
                        Some(tp.into()),
                        None,
                    ))))
                } else {
                    None
                }
            })
        })
    }
    pub fn get_def(&self, record: &Term, field: &UriName) -> Option<Term> {
        self.fields
            .iter()
            .rev()
            .find(|f| name_fits(f, field))
            .and_then(|f| {
                f.data
                    .df
                    .checked_or_parsed()
                    .map(|(t, _)| self.subst(record, &t).into_owned())
            })
    }
    pub fn get_tp(&self, record: &Term, field: &UriName) -> Option<Term> {
        self.fields
            .iter()
            .rev()
            .find(|f| name_fits(f, field))
            .and_then(|f| {
                f.data
                    .tp
                    .checked_or_parsed()
                    .map(|(t, _)| self.subst(record, &t).into_owned())
            })
    }
}

pub struct RecordExtension {
    elems: Vec<AnyRecordType>,
}
impl RecordExtension {
    pub fn is_complete(&self) -> bool {
        self.elems.iter().all(|e| match e {
            AnyRecordType::Structure(s) => s.fields.iter().all(|f| {
                f.data.df.is_some()
                    || self.elems.iter().rev().any(|e| match e {
                        AnyRecordType::Structure(s) => s.fields.iter().any(|e| {
                            name_fits_i(&f.uri.name, &e.uri.name, e.data.macroname.as_ref())
                        }),
                        AnyRecordType::Ano(a) => a.fields.iter().any(|nf| name_fits(f, &nf.name)),
                        AnyRecordType::Ext(_) => unreachable!("eliminated by construction"),
                    })
            }),
            AnyRecordType::Ano(a) => a.fields.iter().all(|f| {
                f.df.is_some()
                    || self.elems.iter().rev().any(|e| match e {
                        AnyRecordType::Structure(s) => {
                            s.fields.iter().any(|e| name_fits(e, &f.name))
                        }
                        AnyRecordType::Ano(a) => a.fields.iter().any(|nf| {
                            nf.name == f.name
                                || nf
                                    .id
                                    .as_ref()
                                    .is_some_and(|id| f.id.as_ref().is_some_and(|f| f == id))
                        }),
                        AnyRecordType::Ext(_) => unreachable!("eliminated by construction"),
                    })
            }),
            AnyRecordType::Ext(_) => unreachable!("eliminated by construction"),
        })
    }
    pub fn from_app<'t, Split: SplitStrategy>(
        app: &'t ApplicationTerm,
        checker: &mut CheckRef<'t, '_, Split>,
    ) -> Result<Self, Option<ModuleUri>> {
        if !app.head.is(&*ftml_uris::metatheory::RECORD_TYPE_MERGE) {
            return Err(None);
        }
        let [Argument::Sequence(MaybeSequence::Seq(recs))] = &*app.arguments else {
            return Err(None);
        };
        let mut elems = Vec::new();
        for r in recs {
            match AnyRecordType::from_term(r, checker)? {
                AnyRecordType::Ext(Self { elems: new }) => elems.extend(new),
                AnyRecordType::Ano(AnonymousRecord { mut fields }) => {
                    for f in &mut fields {
                        if let Some(r) = elems
                            .iter()
                            .rev()
                            .filter_map(|e| {
                                if let AnyRecordType::Structure(m) = e {
                                    Some(m)
                                } else {
                                    None
                                }
                            })
                            .flat_map(|m| m.fields.iter().rev())
                            .find(|s| name_fits(s, &f.name))
                        {
                            f.name = r.uri.name.clone();
                            if let Some(m) = &r.data.macroname {
                                f.id = Some(m.clone());
                            }
                        }
                    }
                    elems.push(AnyRecordType::Ano(AnonymousRecord { fields }));
                }
                o => elems.push(o),
            }
        }
        // TODO check fields against base
        Ok(Self { elems })
    }
    /*fn subst<'t>(&self, record: &Term, tm: &'t Term) -> Cow<'t, Term> {
        self.base.subst(record, tm)
    }*/

    pub fn get_def(&self, record: &Term, field: &UriName) -> Option<Term> {
        self.elems
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, e)| match e {
                AnyRecordType::Ano(e) => e.get_def(record, field),
                AnyRecordType::Ext(_) => unreachable!("Filtered above"),
                AnyRecordType::Structure(e) => e
                    .fields
                    .iter()
                    .rev()
                    .find(|f| name_fits(f, field))
                    .and_then(|f| {
                        f.data
                            .df
                            .checked_or_parsed()
                            .map(|(t, _)| self.subst(record, &t, idx).into_owned())
                    }),
            })
        //.find_map(|r| r.get_def(record, field))
    }
    pub fn get_tp(&self, record: &Term, field: &UriName) -> Option<Term> {
        self.elems
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, e)| match e {
                AnyRecordType::Ano(e) => e.get_tp(record, field),
                AnyRecordType::Ext(_) => unreachable!("Filtered above"),
                AnyRecordType::Structure(e) => e
                    .fields
                    .iter()
                    .rev()
                    .find(|f| name_fits(f, field))
                    .and_then(|f| {
                        f.data
                            .tp
                            .checked_or_parsed()
                            .map(|(t, _)| self.subst(record, &t, idx).into_owned())
                    }),
            })
        //.find_map(|r| r.get_tp(record, field))
    }
    fn subst<'t>(&self, record: &Term, tm: &'t Term, idx: usize) -> Cow<'t, Term> {
        tm.modify(|t| {
            let Term::Symbol { uri, .. } = t else {
                return None;
            };
            self.elems
                .iter()
                .take(idx + 1)
                .flat_map(|e| {
                    if let AnyRecordType::Structure(e) = e {
                        either::Left(e.fields.iter().map(|f| (&e.rec_type, f)))
                    } else {
                        either::Right(std::iter::empty())
                    }
                })
                .find_map(|(rec_type, s)| {
                    if s.uri.equivalent(uri) {
                        // SAFTEY: last names are valie
                        let name = unsafe { s.uri.name.last().parse().unwrap_unchecked() };
                        // SAFTEY: self.rec_type is a structure => nested URI
                        let tp = unsafe { rec_type.clone().into_top_symbol().unwrap_unchecked() };
                        Some(ControlFlow::Break(Term::Field(RecordFieldTerm::new(
                            record.clone(),
                            name,
                            Some(tp.into()),
                            None,
                        ))))
                    } else {
                        None
                    }
                })
        })
    }
}

fn name_fits(s: &Symbol, name: &UriName) -> bool {
    name_fits_i(name, s.uri.name(), s.data.macroname.as_ref())
}
fn name_fits_i(name: &UriName, symname: &UriName, macroname: Option<&Id>) -> bool {
    macroname
        .as_ref()
        .is_some_and(|s| s.as_ref() == name.as_ref())
        || symname
            .as_ref()
            .strip_suffix(name.as_ref())
            .is_some_and(|r| r.is_empty() || r.ends_with('/'))
}
