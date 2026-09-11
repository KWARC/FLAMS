use std::{borrow::Cow, hint::unreachable_unchecked, sync::LazyLock};

use ftml_ontology::terms::{
    ApplicationTerm, Argument, ArgumentMode, BindingTerm, BoundArgument, ComponentVar,
    MaybeSequence, Term, Variable, eq::Alpha, patterns::Pattern,
};
use ftml_solver_trace::{CheckLogCow, Displayable, RefCheckLog, SizedSolverRule};
use ftml_uris::{DocumentUri, Id, ModuleUri, SymbolUri};
use smallvec::SmallVec;

use crate::{
    CheckRef,
    impls::solving::{is_solvable_id, is_solvable_var},
    rules::{RuleSet, implicits::ImplicitExtApp},
    split::SplitStrategy,
};

macro_rules! uri {
    ($(  $name:ident  $(  : $t:ty := $l:literal)?   $( = $lb:literal )?  ),* $(,)?) => {
        $(
            uri!{@go
                $name $( : $t := $l )? $( = $lb )?
            }
        )*
    };
    (@go $name:ident = $l:literal) => {
            pub static $name: LazyLock<SymbolUri> = LazyLock::new(||
                URI.clone() | $l.parse::<ftml_uris::UriName>().expect("Is a valid URI")
            );
    };
    (@go $name:ident : $t:ty := $l:literal) => {
            pub static $name: LazyLock<$t> = LazyLock::new(||
                $l.parse().expect("Is a valid URI")
            );
    }
}

pub static NAMESPACE: &str = "http://mathhub.info?a=FTML/meta";
uri! {
    DOC_URI:DocumentUri := "http://mathhub.info?a=FTML/meta&d=Judgments&l=en",
    URI:ModuleUri := "http://mathhub.info?a=FTML/meta&m=Judgments",

    INH = "inhabitable",
    UNIV = "universe",
    HAS_PROOF = "has proof",
    HAS_TYPE = "has type",
    SUBTYPE = "subtype of",
    SIMPLIFY = "simplifies to",
    EQUAL = "equal to",
    BINDS = "binds",
    PROOF_BARRIER = "proof barrier"
}

pub trait GenericJudgment: SizedSolverRule {
    fn parse<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
        parse(params, rules);
    }
    fn vars(&self) -> &[Id];
    fn premises(&self) -> &[Premise];
}
pub fn parse<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    /*
    println!("Here!");
    for p in params {
        println!(" - {:?}", p.debug_short());
    }
    */
    let Some(Term::Application(concl)) = params.last() else {
        return;
    };
    let mut vars = Vec::new();
    let mut premises = Vec::with_capacity(params.len() - 1);
    for p in &params[..params.len() - 1] {
        if let Some(p) = Premise::parse(p, &mut vars) {
            premises.push(p);
        } else {
            tracing::debug!("Invalid premise: {:?}", p.debug_short());
            return;
        }
    }

    if let [Argument::Simple(t)] = &*concl.arguments {
        if concl.head.is(&*INH) {
            Pattern::from_with_vars(t, true, &mut vars);
            tracing::debug!("New dynamic inhabitability rule: {:?}", t.debug_short());
            rules.push_inhabitable(Box::new(GenericInhabitable {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: t.clone(),
            }));
        } else if concl.head.is(&*UNIV) {
            Pattern::from_with_vars(t, true, &mut vars);
            tracing::debug!("New dynamic universe rule: {:?}", t.debug_short());
            rules.push_universe(Box::new(GenericUniverse {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: t.clone(),
            }));
        } else if concl.head.is(&*HAS_PROOF) {
            Pattern::from_with_vars(t, true, &mut vars);
            tracing::debug!("New dynamic provability rule: {:?}", t.debug_short());
            rules.push_proof(Box::new(GenericProof {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: t.clone(),
            }));
        }
    } else if let [Argument::Simple(a), Argument::Simple(b)] = &*concl.arguments {
        if concl.head.is(&*HAS_TYPE) {
            Pattern::from_with_vars(a, true, &mut vars);
            Pattern::from_with_vars(b, true, &mut vars);
            tracing::debug!(
                "New dynamic typing rule: {:?}  :  {:?}",
                a.debug_short(),
                b.debug_short()
            );
            rules.push_checking(Box::new(GenericTyping {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: (a.clone(), b.clone()),
            }));
        } else if concl.head.is(&*SUBTYPE) {
            Pattern::from_with_vars(a, true, &mut vars);
            Pattern::from_with_vars(b, true, &mut vars);
            tracing::debug!(
                "New dynamic subtyping rule: {:?}  <:  {:?}",
                a.debug_short(),
                b.debug_short()
            );
            rules.push_subtyping(Box::new(GenericSubtyping {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: (a.clone(), b.clone()),
            }));
        } else if concl.head.is(&*SIMPLIFY) {
            Pattern::from_with_vars(a, true, &mut vars);
            Pattern::from_with_vars(b, true, &mut vars);
            tracing::debug!(
                "New dynamic simplification rule: {:?}  -->  {:?}",
                a.debug_short(),
                b.debug_short()
            );
            rules.push_simplification(Box::new(GenericSimplification {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: (a.clone(), b.clone()),
            }));
        } else if concl.head.is(&*EQUAL) {
            Pattern::from_with_vars(a, true, &mut vars);
            Pattern::from_with_vars(b, true, &mut vars);
            tracing::debug!(
                "New dynamic equality rule: {:?}  ==  {:?}",
                a.debug_short(),
                b.debug_short()
            );
            rules.push_equality(Box::new(GenericEquality {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: (a.clone(), b.clone()),
            }));
        } else if concl.head.is(&*BINDS)
            && let Term::Var { variable, .. } = a
        {
            let b = undo_implicits(b).unwrap_or_else(|| b.clone());
            Pattern::from_with_vars(&b, true, &mut vars);
            tracing::debug!("New dynamic preparation rule: {:?}", b.debug_short());
            rules.push_preparation(Box::new(GenericBindPrep {
                vars: vars.into_boxed_slice(),
                premises: premises.into_boxed_slice(),
                concl: (variable.clone(), b),
            }));
        }
    }
}

fn undo_implicits(term: &Term) -> Option<Term> {
    if let Some((head, _)) = term.unapply_implicits(false) {
        Some(head.clone())
    } else if let Term::Application(app) = term
        && let Some((head, _)) = app.head.unapply_implicits(false)
    {
        Some(Term::Application(ApplicationTerm::new(
            head.clone(),
            app.arguments.clone(),
            app.presentation.clone(),
        )))
    } else if let Term::Bound(app) = term
        && let Some((head, _)) = app.head.unapply_implicits(false)
    {
        Some(Term::Bound(BindingTerm::new(
            head.clone(),
            app.arguments.clone(),
            app.presentation.clone(),
        )))
    } else {
        None
    }
}

macro_rules! judg {
    ($($name:ident($concl:ty $(>$prec:literal)?):$rl:ident { $($impl:tt)* } => $slf:ident{$($disp:tt)*} ),*$(,)?) => {
        $(
            #[derive(Clone,Debug,PartialEq,Eq)]
            struct $name {
                vars: Box<[Id]>,
                premises: Box<[Premise]>,
                concl: $concl,
            }
            impl SizedSolverRule for $name {
                #[allow(unused_variables)]
                fn display(&self) -> Vec<ftml_solver_trace::Displayable> {
                    let $slf = self;
                    $($disp)*
                }
                $(
                    fn priority(&self) -> isize {
                        $prec
                    }
                )?
            }
            impl GenericJudgment for $name {
                fn vars(&self) -> &[Id] { &self.vars}
                fn premises(&self) -> &[Premise] {&self.premises}
            }
            impl<Split:SplitStrategy> crate::rules::$rl<Split> for $name {
                $($impl)*
            }
        )*
    };
}
judg! {
    GenericInhabitable(Term): InhabitableRule {
        fn applicable(&self, term: &Term) -> bool {
            Pattern::r#match(term, &self.concl, &self.vars, true).is_some()
        }
        fn apply<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool> {
            let conc = Pattern::r#match(term,&self.concl,&self.vars,true)?;
            let mut conc = self.vars.iter().cloned().zip(conc.into_iter().map(|o| o.map(Cow::into_owned))).collect::<Vec<_>>();
            for p in &self.premises {
                if p.check(&mut conc, &mut checker) != Some(true) { return None;}
            }
            if conc.iter().any(|(_,o)| o.is_none()) {None} else {
                Some(true)
            }
        }
    } => slf { ftml_solver_trace::trace!("Dynamic Rule") },

    GenericUniverse(Term): UniverseRule {
        fn applicable(&self, term: &Term) -> bool {
            Pattern::r#match(term, &self.concl, &self.vars, true).is_some()
        }
        fn apply<'t>(&self, mut checker: CheckRef<'t, '_, Split>, term: &'t Term) -> Option<bool> {
            let conc = Pattern::r#match(term,&self.concl,&self.vars,true)?;
            let mut conc = self.vars.iter().cloned().zip(conc.into_iter().map(|o| o.map(Cow::into_owned))).collect::<Vec<_>>();
            for p in &self.premises {
                if p.check(&mut conc, &mut checker) != Some(true) { return None;}
            }
            if conc.iter().any(|(_,o)| o.is_none()) {None} else {
                Some(true)
            }
        }
    } => slf { ftml_solver_trace::trace!("Dynamic Rule") },

    GenericProof(Term): ProofRule {
        fn applicable(&self, term: &Term) -> bool {
            Pattern::r#match(term, &self.concl, &self.vars, true).is_some()
        }
        fn prove<'t>(&self, mut checker: CheckRef<'t, '_, Split>, goal: &'t Term) -> Option<Term> {
            let conc = Pattern::r#match(goal,&self.concl,&self.vars,true)?;
            let mut conc = self.vars.iter().cloned().zip(conc.into_iter().map(|o| o.map(Cow::into_owned))).collect::<Vec<_>>();
            for p in &self.premises {
                if p.check(&mut conc, &mut checker) != Some(true) { return None;}
            }
            if conc.iter().any(|(_,o)| o.is_none()) {None} else {
                Some(ftml_uris::metatheory::AUTO_PROVE.clone().into())
            }
        }
    } => slf { ftml_solver_trace::trace!("Dynamic Rule") },

    GenericBindPrep((Variable,Term) > 100_000): PreparationRule {
        fn applicable(&self, checker: &CheckRef<'_, '_, Split>, t: &Term) -> bool {
            let Term::Bound(b) = t else {
                //tracing::trace!("Not bound");
                return false;
            };
            let Some(head) = checker.get_head(t) else {
                return false;
            };
            let spec = head.as_ref().either(|s| &s.data.arity, |v| &v.data.arity);
            if spec.num() as usize != b.arguments.len() {
                //tracing::trace!("Arguments don't match: {spec:?} != {:?}", b.arguments);
                return false;
            }
            spec.iter()
                .zip(b.arguments.iter())
                .any(|(a, b)| match (a, b) {
                    (ArgumentMode::BoundVariable, BoundArgument::Simple(t)) | (
                        ArgumentMode::BoundVariableSequence,
                        BoundArgument::Sequence(MaybeSequence::One(t)),
                    ) => {
                        Pattern::r#match(t, &self.concl.1, &self.vars, true).is_some()
                    },
                    (
                        ArgumentMode::BoundVariableSequence,
                        BoundArgument::Sequence(MaybeSequence::Seq(ts)),
                    ) => ts.iter().any(|t| {
                        Pattern::r#match(t, &self.concl.1, &self.vars, true).is_some()
                    }),
                    _ => false,
                })
        }
        fn apply(
            &self,
            checker: &mut CheckRef<'_, '_, Split>,
            t: Term,
            path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
        ) -> std::ops::ControlFlow<Term, Term> {
            let Some(head) = checker.get_head(&t) else {
                return std::ops::ControlFlow::Continue(t);
            };
            let Term::Bound(b) = t else {
                unreachable!("wut");
            };
            let spec = head.as_ref().either(|s| &s.data.arity, |v| &v.data.arity);

            let nargs = spec
                .iter()
                .zip(b.arguments.clone())
                .map(|(m, a)| match (m, a) {
                    (ArgumentMode::BoundVariable, BoundArgument::Simple(t)) => {
                        if Pattern::r#match(&t, &self.concl.1, &self.vars, true).is_some() {
                            // SAFETY: is_app
                            let var/*(mut v, tp)*/ = unsafe { self.get_var(&t) };
                            BoundArgument::Bound(ComponentVar {
                                var,
                                tp: None,//Some(tp),
                                df: None,
                            })
                            /*if v.len() == 1 {
                                // SAFETY: len==1
                                let var = unsafe { v.pop().unwrap_unchecked() };
                                BoundArgument::Bound(ComponentVar {
                                    var,
                                    tp: Some(tp),
                                    df: None,
                                })
                            } else {
                                BoundArgument::Simple(t)
                            }*/
                        } else {
                            BoundArgument::Simple(t)
                        }
                    }
                    (
                        ArgumentMode::BoundVariableSequence,
                        BoundArgument::Sequence(MaybeSequence::Seq(s)),
                    ) => {
                        let mut works = true;
                        //let mut types = Vec::new();
                        let mut ns = s
                            .into_iter()
                            .flat_map(|t| {
                                if Pattern::r#match(&t, &self.concl.1, &self.vars, true).is_some() {
                                    // SAFETY: is_app
                                    let v = unsafe { self.get_var(&t) };
                                    /*for _ in &v {
                                        types.push(tp.clone());
                                    }*/
                                    vec![v.into()]
                                    /*v.into_iter()
                                        .map(|v| Term::Var {
                                            variable: v,
                                            presentation: None,
                                        })
                                        .collect::<Vec<_>>()*/
                                } else {
                                    works = false;
                                    vec![t]
                                }
                            })
                            .collect::<Vec<_>>();
                        if works {
                            BoundArgument::BoundSeq(if ns.len() == 1 && ns[0].is_sequence() {
                                let Some(Term::Var { variable, .. }) = ns.pop() else {
                                    // SAFETY: works == true
                                    unsafe { unreachable_unchecked() }
                                };
                                MaybeSequence::One(ComponentVar {
                                    var: variable,
                                    tp: None,//Some(tp),
                                    df: None,
                                })
                            }else {
                            MaybeSequence::Seq(
                                ns.into_iter()
                                    //.zip(types)
                                    .map(|t|{//(t, tp)| {
                                        let Term::Var { variable, .. } = t else {
                                            // SAFETY: works == true
                                            unsafe { unreachable_unchecked() }
                                        };
                                        ComponentVar {
                                            var: variable,
                                            tp: None,//Some(tp),
                                            df: None,
                                        }
                                    })
                                    .collect(),
                            )
                            })
                        } else {
                            BoundArgument::Sequence(MaybeSequence::Seq(ns.into_boxed_slice()))
                        }
                    }
                    (m, a) => {
                        //tracing::trace!("Other: {m:?} = {a:?}");
                        a
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            std::ops::ControlFlow::Continue(Term::Bound(BindingTerm::new(
                b.head.clone(),
                nargs,
                b.presentation.clone(),
            )))
        }
        fn applicable_revert(&self, _: &CheckRef<'_, '_, Split>, _: &Term) -> bool {
            false
        }
        fn revert(&self, _: &CheckRef<'_, '_, Split>, t: Term) -> std::ops::ControlFlow<Term, Term> {
            std::ops::ControlFlow::Continue(t)
        }

    } => slf { ftml_solver_trace::trace!("Dynamic Rule") },

    GenericTyping((Term,Term)): CheckingRule {
        fn applicable(&self, _: &CheckRef<'_, '_, Split>, term: &Term, tp: &Term) -> bool {
            let (a,b) = &self.concl;
            Pattern::r#match(term, a, &self.vars, true).is_some() &&
            Pattern::r#match(tp, b, &self.vars, true).is_some()
        }
        fn apply<'t>(
                &self,
                mut checker: CheckRef<'t, '_, Split>,
                term: &'t Term,
                tp: &'t Term,
        ) -> Option<bool> {
            let (a,b) = &self.concl;
            let mut vars: Vec<Option<Cow<'t, _>>> = vec![None; self.vars.len()];
            Pattern::match_i(term,a,&self.vars,true,&mut vars,&mut Alpha::new())?;
            Pattern::match_i(tp,b,&self.vars,true,&mut vars,&mut Alpha::new())?;
            let mut conc = self.vars.iter().cloned().zip(vars.into_iter().map(|o| o.map(Cow::into_owned))).collect::<Vec<_>>();
            for p in &self.premises {
                if p.check(&mut conc, &mut checker) != Some(true) { return None;}
            }
            if conc.iter().any(|(_,o)| o.is_none()) {None} else {
                Some(true)
            }
        }
    } => slf { ftml_solver_trace::trace!("Dynamic Rule") },

    GenericSubtyping((Term,Term)): SubtypeRule {
        fn applicable(&self, _: &CheckRef<'_, '_, Split>, sub: &Term, sup: &Term) -> bool {
            let (a,b) = &self.concl;
            Pattern::r#match(sub, a, &self.vars, true).is_some() &&
            Pattern::r#match(sup, b, &self.vars, true).is_some()
        }
        fn apply<'t>(
            &self,
            mut checker: CheckRef<'t, '_, Split>,
            sub: &'t Term,
            sup: &'t Term,
        ) -> Option<bool> {
            let (a,b) = &self.concl;
            let mut vars: Vec<Option<Cow<'t, _>>> = vec![None; self.vars.len()];
            Pattern::match_i(sub,a,&self.vars,true,&mut vars,&mut Alpha::new())?;
            Pattern::match_i(sup,b,&self.vars,true,&mut vars,&mut Alpha::new())?;
            let mut conc = self.vars.iter().cloned().zip(vars.into_iter().map(|o| o.map(Cow::into_owned))).collect::<Vec<_>>();
            for p in &self.premises {
                if p.check(&mut conc, &mut checker) != Some(true) { return None;}
            }
            if conc.iter().any(|(_,o)| o.is_none()) {None} else {
                Some(true)
            }
        }
    } => slf { ftml_solver_trace::trace!("Dynamic Rule") },

    GenericEquality((Term,Term)): EqualityRule {
        fn applicable(&self, lhs: &Term, rhs: &Term) -> bool {
            let (a,b) = &self.concl;
            Pattern::r#match(lhs, a, &self.vars, true).is_some() &&
            Pattern::r#match(rhs, b, &self.vars, true).is_some()
        }
        fn apply<'t>(
            &self,
            mut checker: CheckRef<'t, '_, Split>,
            lhs: &'t Term,
            rhs: &'t Term,
        ) -> Option<bool> {
            let (a,b) = &self.concl;
            let mut vars: Vec<Option<Cow<'t, _>>> = vec![None; self.vars.len()];
            Pattern::match_i(lhs,a,&self.vars,true,&mut vars,&mut Alpha::new())?;
            Pattern::match_i(rhs,b,&self.vars,true,&mut vars,&mut Alpha::new())?;
            let mut conc = self.vars.iter().cloned().zip(vars.into_iter().map(|o| o.map(Cow::into_owned))).collect::<Vec<_>>();
            for p in &self.premises {
                if p.check(&mut conc, &mut checker) != Some(true) { return None;}
            }
            if conc.iter().any(|(_,o)| o.is_none()) {None} else {
                Some(true)
            }
        }
    } => slf { ftml_solver_trace::trace!("Dynamic Rule") },

    GenericSimplification((Term,Term)): SimplificationRule {
        fn applicable(&self, term: &Term) -> bool {
            Pattern::r#match(term, &self.concl.0, &self.vars, true).is_some()
        }
        fn apply<'t>(
            &self,
            mut checker: CheckRef<'t, '_, Split>,
            term: &'t Term,
        ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
            let (a,b) = &self.concl;
            let mut vars: Vec<Option<Cow<'t, _>>> = vec![None; self.vars.len()];
            Pattern::match_i(term,a,&self.vars,true,&mut vars,&mut Alpha::new()).ok_or(None)?;
            //Pattern::match_i(rhs,b,&self.vars,true,&mut vars,&mut Alpha::new()).ok_or(None)?;
            let mut conc = self.vars.iter().cloned().zip(vars.into_iter().map(|o| o.map(Cow::into_owned))).collect::<Vec<_>>();
            for (i,p) in self.premises.iter().enumerate() {
                checker.comment(format!("Premise {}",i+1));
                if p.check(&mut conc, &mut checker) != Some(true) { return Err(None);}
            }
            if conc.iter().any(|(v,o)| !is_solvable_id(v) && o.is_none()) {return Err(None)}
            let subst = conc.iter().filter_map(|(v,o)| if is_solvable_id(v) && o.is_none() {
                Some((v.as_ref(),Cow::Owned(checker.new_solvable())))
            } else {o.as_ref().map(|t| (v.as_ref(),Cow::Borrowed(t)))}).collect::<Vec<_>>();
            let ret = b.clone() / subst.as_slice();
            checker.add_msg(CheckLogCow::Borrowed(RefCheckLog::Msg(vec!["Result: ".into(),ret.clone().into()], ftml_solver_trace::MessageLevel::Comment)));
            //println!("HERE RETURN: {:?}",(b.clone() / subst.as_slice()).debug_short());
            Ok(ret)
        }
    } => slf {
        let mut ret = vec!["Dynamic rule: ".into()];
        if !slf.premises.is_empty() {
            ret.push("[ ".into());
            let mut first = true;
            for p in &slf.premises {
                if !first {
                    ret.push(", ".into());
                }
                first = false;
                p.disps(&mut ret);
            }
            ret.push(" ] ".into());
        }
        ret.extend(["⊢ ".into(),slf.concl.0.clone().into(),"  ==>  ".into(),slf.concl.1.clone().into()]);
        ret
    },
}

impl GenericBindPrep {
    /// Only safe to call iff `self.is_app(&t)`
    unsafe fn get_var(&self, t: &Term) -> Variable {
        let Some(mut matches) = Pattern::r#match(t, &self.concl.1, &self.vars, true) else {
            unreachable!("bug!");
        };
        self.vars
            .iter()
            .position(|v| v.as_ref() == self.concl.0.name())
            .map_or_else(
                || self.concl.0.clone(),
                |i| {
                    if let Some(Term::Var { variable, .. }) = matches.remove(i).map(Cow::into_owned)
                    {
                        variable
                    } else {
                        self.concl.0.clone()
                    }
                },
            )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Premise {
    Inhabitable(Term),
    Universe(Term),
    HasProof(Term),
    HasType(Term, Term),
    Subtype(Term, Term),
    Equal(Term, Term),
}
impl Premise {
    fn disps(&self, target: &mut Vec<Displayable>) {
        match self {
            Self::Inhabitable(t) => target.extend(["⊢ INH ".into(), t.clone().into()]),
            Self::Universe(t) => target.extend(["⊢ UNIV ".into(), t.clone().into()]),
            Self::HasProof(t) => target.extend(["⊢ PROOF ".into(), t.clone().into()]),
            Self::HasType(t1, t2) => target.extend([
                "⊢".into(),
                t1.clone().into(),
                " : ".into(),
                t2.clone().into(),
            ]),
            Self::Subtype(t1, t2) => target.extend([
                "⊢".into(),
                t1.clone().into(),
                " <: ".into(),
                t2.clone().into(),
            ]),
            Self::Equal(t1, t2) => target.extend([
                "⊢".into(),
                t1.clone().into(),
                " == ".into(),
                t2.clone().into(),
            ]),
        }
    }
    fn parse(t: &Term, vars: &mut Vec<Id>) -> Option<Self> {
        let Term::Application(p) = t else { return None };
        if let [Argument::Simple(t)] = &*p.arguments {
            if p.head.is(&*INH) {
                Pattern::from_with_vars(t, true, vars);
                Some(Self::Inhabitable(t.clone()))
            } else if p.head.is(&*UNIV) {
                Pattern::from_with_vars(t, true, vars);
                Some(Self::Universe(t.clone()))
            } else if p.head.is(&*HAS_PROOF) {
                Pattern::from_with_vars(t, true, vars);
                Some(Self::HasProof(t.clone()))
            } else {
                None
            }
        } else if let [Argument::Simple(a), Argument::Simple(b)] = &*p.arguments {
            if p.head.is(&*HAS_TYPE) {
                Pattern::from_with_vars(a, true, vars);
                Pattern::from_with_vars(b, true, vars);
                Some(Self::HasType(a.clone(), b.clone()))
            } else if p.head.is(&*SUBTYPE) {
                Pattern::from_with_vars(a, true, vars);
                Pattern::from_with_vars(b, true, vars);
                Some(Self::Subtype(a.clone(), b.clone()))
            } else if p.head.is(&*EQUAL) {
                Pattern::from_with_vars(a, true, vars);
                Pattern::from_with_vars(b, true, vars);
                Some(Self::Equal(a.clone(), b.clone()))
            } else {
                None
            }
        } else {
            None
        }
    }
    pub fn check<Split: SplitStrategy>(
        &self,
        context: &mut [(Id, Option<Term>)],
        checker: &mut CheckRef<'_, '_, Split>,
    ) -> Option<bool> {
        fn has_free(t: &Term, ctx: &[(Id, Option<Term>)]) -> bool {
            t.free_variables().iter().any(|v| {
                ctx.iter()
                    .all(|(a, b)| a.as_ref() != v.name() || b.is_none())
            })
        }
        macro_rules! check {
            ($c:ident.$f:ident($($t:ident),+) ) => {{
                let subst = context.iter().filter_map(|(id,o)| o.as_ref().map(|t| (id.as_ref(),t))).collect::<Vec<_>>();
                $(
                    let $t = $t / subst.as_slice();
                )*
                checker.scoped(|$c| $c.$f($(&$t),*))
            }}
        }
        match self {
            Self::Inhabitable(t) => check!(c.check_inhabitable(t)),
            Self::Universe(t) => check!(c.check_universe(t)),
            Self::HasProof(t) => check!(c.prove(t)).map(|_| true),
            Self::Subtype(a, b) => check!(c.check_subtype(a, b)),
            Self::Equal(a, b) => {
                if has_free(b, context) && !has_free(a, context) {
                    let subst = context
                        .iter()
                        .filter_map(|(id, o)| o.as_ref().map(|t| (id.as_ref(), t)))
                        .collect::<Vec<_>>();
                    let na = a / subst.as_slice();
                    let nb = b / subst.as_slice();
                    let all_vars = nb
                        .free_variables()
                        .iter()
                        .map(|v| v.name_id().into_owned())
                        .collect::<SmallVec<_, 2>>();
                    checker.comment(format!(
                        "matching {:?} against {:?}",
                        na.debug_short(),
                        nb.debug_short()
                    ));
                    let r = checker.scoped(|ch| {
                        ch.simplify_until(&na, |_, t| {
                            #[allow(clippy::option_if_let_else)]
                            if let Some(r) = Pattern::r#match(t, &nb, &all_vars, true) {
                                for (name, nt) in all_vars.iter().zip(r) {
                                    if let Some((_, t)) =
                                        context.iter_mut().find(|(a, _)| *a == *name)
                                    {
                                        *t = nt.map(Cow::into_owned);
                                    }
                                }
                                true
                            } else {
                                false
                            }
                        })
                        .is_some()
                    });
                    if r { Some(true) } else { None }
                } else {
                    check!(c.check_equality(a, b))
                }
            }
            Self::HasType(a, b) => {
                if let Term::Var {
                    variable: Variable::Name { name, .. },
                    ..
                } = b
                    && context.iter().any(|(p, o)| p == name && o.is_none())
                {
                    let nt = check!(c.infer_type(a))?;
                    context.iter_mut().find(|(a, _)| *a == *name)?.1 = Some(nt);
                    Some(true)
                } else if !has_free(a, context) && !has_free(b, context) {
                    check!(c.check_type(a, b))
                } else {
                    None
                }
            }
        }
    }
}
