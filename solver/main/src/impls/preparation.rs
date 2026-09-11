use crate::{
    CheckRef,
    impls::{records::Record, solving::Solutions},
    rules::implicits::{ImplicitExtApp, ImplicitExtBound, ImplicitExtTerm},
    split::SplitStrategy,
};
use either::Either;
use ftml_ontology::{
    domain::{
        SharedDeclaration,
        declarations::{morphisms::Morphism, symbols::Symbol},
    },
    narrative::{SharedDocumentElement, elements::VariableDeclaration},
    terms::{
        ApplicationTerm, Argument, BindingTerm, BoundArgument, ComponentVar, IsTerm, MaybeSequence,
        OpaqueTerm, Term, Variable, helpers::IntoTerm, sequences::Sequence, termpaths::TermPath,
    },
};

impl<Split: SplitStrategy> CheckRef<'_, '_, Split> {
    #[inline]
    #[must_use]
    pub const fn rules(&self) -> &crate::rules::RuleSet<Split> {
        &self.top.rules
    }

    pub(crate) fn prepare(&self, t: Term, path: Option<&mut TermPath>) -> (Solutions, Term) {
        let mut cp = self.copied();
        let mut ncp = cp.get_ref();
        let old = ncp.context.take();
        let r = ncp.prepare_i(t, path.map(|p| (p.inner_mut(), 0)));
        ncp.context.set(old);
        drop(ncp);
        let sols = std::mem::take(&mut cp.solutions);
        (sols, r)
    }

    pub(crate) fn revert_prepare(&self, t: Term) -> Term {
        tracing::trace!("reverting preparation {:?}", t.debug_short());
        let mut cp = self.copied();
        let mut ncp = cp.get_ref();
        let old = ncp.context.take();
        let r = ncp.revert_i(t);
        ncp.context.set(old);
        r
    }

    /*
    pub(crate) fn bind_implicits(&mut self, nt: Term) -> Term {
        tracing::trace!("Binding implicits for {:?}", nt.debug_short());
        let mut allvars = nt
            .free_variables()
            .into_iter()
            .cloned()
            .collect::<SmallVec<_, 4>>();
        let mut curr_idx = 0;
        if allvars.is_empty() {
            return nt;
        }
        while curr_idx < allvars.len() {
            if let Variable::Ref { declaration, .. } = &allvars[curr_idx]
                && let Ok(v) = self.get_variable(declaration)
                && let Some((tp, _)) = v.data.tp.checked_or_parsed()
            {
                let vars = tp.free_variables();
                let mut changed = false;
                for v in vars {
                    if !allvars[..curr_idx].contains(v) {
                        allvars.insert(curr_idx, v.clone());
                        changed = true;
                    }
                }
                if changed {
                    continue;
                }
            }
            curr_idx += 1;
        }
        tracing::trace!("All variables: {allvars:?}");

        let mut ctx = smallvec::SmallVec::<_, 4>::new();
        for v in allvars {
            if !ctx.iter().any(|(var, _)| *var == v) {
                let name = self.new_solvable();
                let Variable::Name { name: id, .. } = &name else {
                    // SAFETY: new_solvable always returns Variable::Name
                    unsafe { unreachable_unchecked() }
                };
                let tp = self.infer_var_type_i(&v);
                if let Some(tp) = tp {
                    self.comment(format!("Solving type of {id}"));
                    self.solve_type(id.clone(), tp / ctx.as_slice());
                }

                ctx.push((
                    v,
                    Term::Var {
                        variable: name,
                        presentation: None,
                    },
                ));
            }
        }
        if ctx.is_empty() {
            return nt;
        }
        tracing::trace!("New context: {ctx:?}");
        let n = nt / ctx.as_slice();
        tracing::trace!("Implicitified: {:?}", n.debug_short());
        n
    }
     */

    pub fn get_head(
        &self,
        t: &Term,
    ) -> Option<Either<SharedDeclaration<Symbol>, SharedDocumentElement<VariableDeclaration>>> {
        let head = t.head()?;
        Some(match head {
            Either::Left(uri) => Either::Left(self.get_symbol(uri).ok()?),
            Either::Right(Variable::Ref { declaration, .. }) => {
                Either::Right(self.get_variable(declaration).ok()?)
            }
            either::Right(Variable::Name { .. }) => return None,
        })
    }

    /*
    fn push_down_implicits(term: Term) -> Term {
        if let Term::Application(ref app) = term
            && app.head.is(&*ftml_uris::metatheory::APPLY_IMPLICIT)
            && let [
                Argument::Simple(f @ (Term::Application(_) | Term::Bound(_))),
                Argument::Sequence(MaybeSequence::Seq(args)),
            ] = &*app.arguments
        {
            let mut iter = args.iter();
            let next = if let Term::Application(fapp) = f {
                let napp = fapp
                    .head
                    .clone()
                    .apply_implicits(args.len(), |_| iter.next().expect("bug").clone());
                Term::Application(ApplicationTerm::new(
                    napp,
                    fapp.arguments.clone(),
                    fapp.presentation.clone(),
                ))
            } else if let Term::Bound(fapp) = f {
                let napp = fapp
                    .head
                    .clone()
                    .apply_implicits(args.len(), |_| iter.next().expect("bug").clone());
                Term::Bound(BindingTerm::new(
                    napp,
                    fapp.arguments.clone(),
                    fapp.presentation.clone(),
                ))
            } else {
                unreachable!("bug");
            };
            Self::push_down_implicits(next)
        } else {
            term
        }
    }
     */

    fn prepare_i(
        &mut self,
        mut t: Term,
        mut path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> Term {
        tracing::trace!("preparing {:?}", t.debug_short());
        //let mut t = Self::prepare_seqs(t);
        //let mut t = Self::push_down_implicits(t);
        if t.unapply_implicits(false).is_some() {
            return t;
        }
        match &t {
            Term::Field(f) => {
                if let Some(r) = self.scoped(|slf| {
                    if let Ok(r) = Record::from_term(&f.record, f.record_type.as_ref(), slf)
                    && let Some(sym) = r.get_type().get_symbol(&f.key)
                    //&& sym.data.tp.has_checked()
                    && let Some(Some(vars)) = sym
                        .data
                        .tp
                        .with_checked(|t| Some(t.get_bound_implicits()?.1.len()))
                    {
                        Some(t.clone().apply_implicits(vars, |_| slf.new_solvable()))
                    } else {
                        None
                    }
                }) {
                    return r;
                }
            }
            Term::Symbol { uri, presentation } => {
                return if let Ok(sym) = self.get_symbol(uri)
                    && sym.data.tp.has_checked()
                {
                    sym.data
                        .tp
                        .with_checked(|t| {
                            let (_, vars) = t.get_bound_implicits()?;
                            Some(
                                Term::Symbol {
                                    uri: uri.clone(),
                                    presentation: presentation.clone(),
                                }
                                .apply_implicits(vars.len(), |_| self.new_solvable()),
                            )
                        })
                        .flatten()
                        .unwrap_or(t)
                } else {
                    t
                };
            }
            Term::Var { .. } => return t,
            _ => (),
        }

        for rl in self.top.rules.preparation() {
            if !rl.applicable(self, &t) {
                continue;
            }
            let path = match &mut path {
                Some((p, t)) => Some((&mut **p, *t)),
                _ => None,
            };
            match rl.apply(self, t, path) {
                std::ops::ControlFlow::Break(t) => return t,
                std::ops::ControlFlow::Continue(tm) => {
                    t = tm;
                }
            }
            tracing::trace!("Rule {rl:?} applied; result: {:?}", t.debug_short());
        }
        self.prepare_recurse(t, |s, t, p| s.prepare_i(t, p), path)
    }

    fn revert_i(&mut self, t: Term) -> Term {
        match &t {
            Term::Application(b) if b.head.unapply_implicits(true).is_some() => {
                // SAFETY: pattern match
                let (t, args) = unsafe { b.head.unapply_implicits(true).unwrap_unchecked() };
                {
                    return self.revert_i(
                        Term::Application(ApplicationTerm::new(
                            t.clone(),
                            b.arguments.clone(),
                            b.presentation.clone(),
                        ))
                        .apply_implicits(args.len(), |i| args[i].clone()),
                    );
                }
            }
            Term::Bound(b) if b.head.unapply_implicits(true).is_some() => {
                // SAFETY: pattern match
                let (t, args) = unsafe { b.head.unapply_implicits(true).unwrap_unchecked() };
                {
                    return self.revert_i(
                        Term::Bound(BindingTerm::new(
                            t.clone(),
                            b.arguments.clone(),
                            b.presentation.clone(),
                        ))
                        .apply_implicits(args.len(), |i| args[i].clone()),
                    );
                }
            }

            Term::Application(a) if a.unapply_implicits(true).is_some() => {
                // SAFETY: pattern match
                let (t, _) = unsafe { a.unapply_implicits(true).unwrap_unchecked() };
                return t.clone();
            }
            Term::Symbol { .. } | Term::Var { .. } => return t,
            _ => (),
        }

        let mut tm = t;

        for rl in self.top.rules.preparation().iter().rev() {
            // SAFETY: not yet replaced
            if !rl.applicable_revert(self, &tm) {
                continue;
            }
            tm = match rl.revert(self, tm) {
                std::ops::ControlFlow::Break(t) => return t,
                std::ops::ControlFlow::Continue(t) => t,
            };
            tracing::trace!("Rule {rl:?} applied; result: {:?}", tm.debug_short());
        }
        self.prepare_recurse(tm, |s, t, _| s.revert_i(t), None)
    }

    fn prepare_recurse(
        &mut self,
        term: Term,
        then: fn(
            &mut CheckRef<'_, '_, Split>,
            Term,
            Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
        ) -> Term,
        mut path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> Term {
        tracing::trace!("Recursing {:?}", term.debug_short());
        match term {
            Term::Opaque(ot) => Term::Opaque(OpaqueTerm::new(
                ot.node.clone(),
                ot.terms
                    .iter()
                    .enumerate()
                    .map(|(i, t)| then(self, t.clone(), get_path(&mut path, i)))
                    .collect(),
            )),
            Term::Application(a) => Term::Application(ApplicationTerm::new(
                then(self, a.head.clone(), get_path(&mut path, 0)),
                {
                    let mut idx = 0;
                    a.arguments
                        .iter()
                        .map(|arg| match arg {
                            Argument::Simple(t) => Argument::Simple(then(
                                self,
                                t.clone(),
                                get_path(&mut path, {
                                    idx += 1;
                                    idx
                                }),
                            )),
                            Argument::Sequence(MaybeSequence::One(t)) => {
                                Argument::Sequence(MaybeSequence::One(then(
                                    self,
                                    t.clone(),
                                    get_path(&mut path, {
                                        idx += 1;
                                        idx
                                    }),
                                )))
                            }
                            Argument::Sequence(MaybeSequence::Seq(ts)) => {
                                idx += 1;
                                let mut npath = get_path(&mut path, idx);
                                Argument::Sequence(MaybeSequence::Seq(
                                    ts.iter()
                                        .cloned()
                                        .enumerate()
                                        .map(|(i, t)| then(self, t, get_path(&mut npath, i)))
                                        .collect(),
                                ))
                            }
                        })
                        .collect()
                },
                a.presentation.clone(),
            )),
            Term::Bound(b) => Term::Bound(BindingTerm::new(
                then(self, b.head.clone(), get_path(&mut path, 0)),
                self.scoped(|slf| {
                    let mut idx = 0;
                    b.arguments
                        .iter()
                        .map(|arg| match arg {
                            BoundArgument::Simple(t) => BoundArgument::Simple(then(
                                slf,
                                t.clone(),
                                get_path(&mut path, {
                                    idx += 1;
                                    idx
                                }),
                            )),
                            BoundArgument::Sequence(MaybeSequence::One(t)) => {
                                BoundArgument::Sequence(MaybeSequence::One(then(
                                    slf,
                                    t.clone(),
                                    get_path(&mut path, {
                                        idx += 1;
                                        idx
                                    }),
                                )))
                            }
                            BoundArgument::Sequence(MaybeSequence::Seq(ts)) => {
                                idx += 1;
                                let mut npath = get_path(&mut path, idx);
                                BoundArgument::Sequence(MaybeSequence::Seq(
                                    ts.iter()
                                        .cloned()
                                        .enumerate()
                                        .map(|(i, t)| then(slf, t, get_path(&mut npath, i)))
                                        .collect(),
                                ))
                            }
                            BoundArgument::Bound(cv) => BoundArgument::Bound(slf.prepare_cv(
                                cv,
                                then,
                                get_path(&mut path, {
                                    idx += 1;
                                    idx
                                }),
                            )),
                            BoundArgument::BoundSeq(MaybeSequence::One(cv)) => {
                                BoundArgument::BoundSeq(MaybeSequence::One(slf.prepare_cv(
                                    cv,
                                    then,
                                    get_path(&mut path, {
                                        idx += 1;
                                        idx
                                    }),
                                )))
                            }
                            BoundArgument::BoundSeq(MaybeSequence::Seq(vars)) => {
                                idx += 1;
                                let mut npath = get_path(&mut path, idx);
                                BoundArgument::BoundSeq(MaybeSequence::Seq(
                                    vars.iter()
                                        .enumerate()
                                        .map(|(i, cv)| {
                                            slf.prepare_cv(cv, then, get_path(&mut npath, i))
                                        })
                                        .collect(),
                                ))
                            }
                        })
                        .collect()
                }),
                b.presentation.clone(),
            )),
            t => t,
        }
    }

    #[allow(clippy::single_match_else)]
    fn prepare_cv(
        &mut self,
        cv: &ComponentVar,
        then: fn(&mut Self, Term, Option<(&mut smallvec::SmallVec<u8, 16>, usize)>) -> Term,
        mut path: Option<(&mut smallvec::SmallVec<u8, 16>, usize)>,
    ) -> ComponentVar {
        tracing::trace!("preparing bound variable {}", cv.var.name());
        let mut next = 0;
        let tp = match &cv.tp {
            Some(t) => {
                next += 1;
                Some(then(self, t.clone(), get_path(&mut path, 0)))
            }
            None => {
                let r = self.infer_var_type_i(&cv.var);
                if r.is_some()
                    && let Some((p, i)) = &mut path
                    && let Some(i) = p.get_mut(*i)
                {
                    *i += 1;
                    next += 1;
                }
                r
            }
        };
        let df = match &cv.df {
            Some(t) => Some(then(self, t.clone(), get_path(&mut path, next))),
            None => {
                let r = self.get_var_definiens(&cv.var);
                if r.is_some()
                    && let Some((p, i)) = &mut path
                    && let Some(i) = p.get_mut(*i)
                {
                    *i += 1;
                    next += 1;
                }
                r
            }
        };
        let cv = ComponentVar {
            var: cv.var.clone(),
            tp,
            df,
        };
        tracing::trace!("Extending context");
        self.extend_context(cv.clone());
        tracing::trace!("Done");
        cv
    }
}

fn get_path<'a, 'b>(
    op: &'a mut Option<(&'b mut smallvec::SmallVec<u8, 16>, usize)>,
    idx: usize,
) -> Option<(&'a mut smallvec::SmallVec<u8, 16>, usize)> {
    op.as_mut().and_then(|(s, i)| {
        if s.get(*i).copied() == Some(idx as u8) {
            Some((&mut **s, *i + 1))
        } else {
            None
        }
    })
}
