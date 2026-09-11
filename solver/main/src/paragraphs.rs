use crate::{
    Checker,
    facts::GlobalOrLocal,
    hoas::HOASSymbols,
    impls::solving::{Solutions, TermExtSolvable},
    rules::ProofBarrier,
    split::SplitStrategy,
};
use ftml_ontology::{
    narrative::elements::{LogicalParagraph, paragraphs::ParagraphStep},
    terms::{BindingTerm, BoundArgument, ComponentVar, MaybeSequence, Term, Variable},
    utils::SourceRange,
};
use ftml_solver_trace::{
    CheckLog, CheckingTask, PreCheckLog,
    results::{
        CheckResult, ContentCheckResult, ProofStepCheckResult, ProofStepResult, SymbolCheckResult,
        TypeCheckResult,
    },
};
use ftml_uris::{DocumentElementUri, Id, SymbolUri};

impl<Split: SplitStrategy> Checker<Split> {
    pub fn check_assertion(&mut self, p: &LogicalParagraph) -> Option<Vec<CheckResult>> {
        let hoas = self.hoas()?;
        let mut ret = Vec::new();
        for (target, term) in &p.fors {
            let Ok(target) = self.get_symbol(target, |t| t) else {
                continue;
            };
            let Some(term) = term else { continue };
            let Some(term) = term.get_parsed() else {
                continue;
            };
            let params = p
                .binds_variables
                .iter()
                .filter_map(|uri| {
                    self.get_variable(uri).ok().map(|v| ComponentVar {
                        var: Variable::Ref {
                            declaration: uri.clone(),
                            is_sequence: Some(v.data.is_seq),
                        },
                        tp: v.data.tp.checked_or_parsed().map(|(t, _)| t),
                        df: v.data.df.checked_or_parsed().map(|(t, _)| t),
                    })
                })
                .collect::<Vec<_>>();

            let wrapped = hoas.wrap_types(&p.premises, term);
            let wrapped = if params.is_empty() {
                wrapped.into_owned()
            } else {
                Term::Bound(BindingTerm::new(
                    hoas.pi.clone().into(),
                    Box::new([
                        BoundArgument::BoundSeq(MaybeSequence::Seq(params.into_boxed_slice())),
                        BoundArgument::Simple(wrapped.into_owned()),
                    ]),
                    None,
                ))
            };
            let (unks, tp) = self.prepare(None, wrapped);

            tracing::trace!("Checking assertion for {}", target.uri);
            let (b, unks, mut l) = self.check_inhabitable(Some(unks), &tp);
            let mut tp = self.wrap_none(Some(unks), |slf| slf.subst(tp)).1;

            if tp.has_solvable() {
                l.push(PreCheckLog::Msg(
                    vec!["Unsolved unkowns remain".into()],
                    ftml_solver_trace::MessageLevel::Failure,
                ));
                ret.push(CheckResult::Content(ContentCheckResult::Symbol(
                    target.uri.clone(),
                    SymbolCheckResult::TypeOnly {
                        result: TypeCheckResult {
                            success: false,
                            log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                        },
                    },
                )));
                continue;
            }
            if let Some(t) = self.bind_implicits(&tp) {
                tp = t;
            }

            target
                .data
                .tp
                .set_presentation(self.revert_prepare(tp.clone()));
            target.data.tp.set_checked(tp);
            ret.push(CheckResult::Content(ContentCheckResult::Symbol(
                target.uri.clone(),
                SymbolCheckResult::TypeOnly {
                    result: TypeCheckResult {
                        success: b.unwrap_or(false),
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    },
                },
            )));
        }
        Some(ret)
    }

    pub fn check_definition(&mut self, p: &LogicalParagraph) -> Vec<CheckResult> {
        let Some(hoas) = self.hoas() else {
            return Vec::new();
        };
        let mut ret = Vec::new();
        for (target, term) in &p.fors {
            let Ok(target) = self.get_symbol(target, |t| t) else {
                continue;
            };
            let Some(term) = term else { continue };
            let Some(term) = term.get_parsed() else {
                continue;
            };
            let params = p
                .binds_variables
                .iter()
                .filter_map(|uri| {
                    self.get_variable(uri).ok().map(|v| ComponentVar {
                        var: Variable::Ref {
                            declaration: uri.clone(),
                            is_sequence: Some(v.data.is_seq),
                        },
                        tp: v.data.tp.checked_or_parsed().map(|(t, _)| t),
                        df: v.data.df.checked_or_parsed().map(|(t, _)| t),
                    })
                })
                .collect::<Vec<_>>();

            let df = if params.is_empty() {
                term.clone()
            } else {
                Term::Bound(BindingTerm::new(
                    hoas.lambda.clone().into(),
                    Box::new([
                        BoundArgument::BoundSeq(MaybeSequence::Seq(params.into_boxed_slice())),
                        BoundArgument::Simple(term.clone()),
                    ]),
                    None,
                ))
            };

            tracing::trace!("Checking definiens for {}", target.uri);
            if let Some(tp) = target.data.tp.get_parsed() {
                ret.push(CheckResult::Content(ContentCheckResult::Symbol(
                    target.uri.clone(),
                    self.df_and_tp(&df, &target.data.df, tp, &target.data.tp, true, true),
                )));
            } else {
                ret.push(CheckResult::Content(ContentCheckResult::Symbol(
                    target.uri.clone(),
                    self.df_only(&df, &target.data.df, &target.data.tp, true, true),
                )));
            }
        }
        ret
    }

    pub fn check_proof(&mut self, p: &LogicalParagraph) -> Option<CheckResult> {
        //println!("Here: {:?}", &p.children);
        let mut ret = Vec::new();
        let mut ctx = Vec::new();
        let _ = self.hoas()?;
        let mut state = ProofCheckState {
            context: &mut ctx,
            counter: 0,
            conclusion_df: None,
            conclusion_type: None,
        };
        let for_symbol = p
            .fors
            .first()
            .and_then(|sym| self.get_symbol(&sym.0, |t| self.prepare(None, t).1).ok());
        let block = for_symbol.as_ref().map(|sym| &sym.uri);
        for s in &p.steps {
            if let Some(res) = self.proof_step(s, &mut state, block) {
                let success = res.success();
                ret.push(res);
                if !success {
                    return Some(CheckResult::Proof(p.uri.clone(), ret));
                }
            }
        }
        if matches!(p.steps.last(), Some(ParagraphStep::ProofConclusion { .. })) {
            state.context.pop();
        }
        let (tp, df) = state.make_conclusion(self.hoas()?, 0);
        if (tp.is_some() || df.is_some())
            && let Some(sym) = for_symbol.as_ref()
        {
            let orig_tp = &sym.data.tp;
            let orig_df = &sym.data.df;
            if let Some(tp) = tp {
                if let Some((orig, _)) = orig_tp.checked_or_parsed() {
                    let tp = self.bind_implicits(&tp).unwrap_or(tp.clone());
                    let (b, _, l) = self.check_subtype(None, &tp, &orig);
                    ret.push(ProofStepResult::Conclusion {
                        var: None,
                        result: ProofStepCheckResult::GoalOnly {
                            result: TypeCheckResult {
                                success: b.unwrap_or_default(),
                                log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                            },
                        },
                    });
                } else {
                    orig_tp.set_checked(tp.clone());
                    orig_tp.set_presentation(self.revert_prepare(tp.clone()));
                }
                if let Some(df) = df
                    && orig_df.is_none()
                {
                    let df = ProofBarrier::apply(df, tp);
                    let df = self.bind_implicits(&df).unwrap_or(df);
                    orig_df.set_checked(df.clone());
                    orig_df.set_presentation(self.revert_prepare(df));
                }
            }
        } else if for_symbol.is_some() {
            return Some(CheckResult::Proof(
                p.uri.clone(),
                vec![ProofStepResult::Conclusion {
                    var: None,
                    result: ProofStepCheckResult::ProofOnly {
                        inferred: None,
                        log: CheckLog::Fail(vec![
                            "does not establish the theorem.".to_string().into(),
                        ]),
                    },
                }],
            ));
        }
        Some(CheckResult::Proof(p.uri.clone(), ret))
    }

    fn proof_step(
        &mut self,
        s: &ParagraphStep,
        context: &mut ProofCheckState,
        block: Option<&SymbolUri>,
    ) -> Option<ProofStepResult> {
        match s {
            ParagraphStep::EquationStep => None,
            ParagraphStep::ProofAssumption {
                var_name,
                method,
                justification,
                arguments,
                yields,
            } => self
                .step_data(
                    context,
                    var_name.as_ref(),
                    method.as_ref().map(|(t, _)| t),
                    justification.as_ref().map(|(t, _)| t),
                    arguments,
                    yields.as_ref().map(|(t, _)| t),
                    true,
                    block,
                )
                .map(|r| ProofStepResult::Assumption {
                    var: var_name.clone(),
                    result: r,
                }),
            ParagraphStep::ProofConclusion {
                var_name,
                method,
                justification,
                arguments,
                yields,
            } => self
                .conclusion_step(
                    context,
                    var_name.as_ref(),
                    method.as_ref().map(|(t, _)| t),
                    justification.as_ref().map(|(t, _)| t),
                    arguments,
                    yields.as_ref().map(|(t, _)| t),
                    block,
                )
                .map(|r| ProofStepResult::Conclusion {
                    var: var_name.clone(),
                    result: r,
                }),
            ParagraphStep::ProofStep {
                var_name,
                method,
                justification,
                arguments,
                yields,
            } => self
                .step_data(
                    context,
                    var_name.as_ref(),
                    method.as_ref().map(|(t, _)| t),
                    justification.as_ref().map(|(t, _)| t),
                    arguments,
                    yields.as_ref().map(|(t, _)| t),
                    false,
                    block,
                )
                .map(|r| ProofStepResult::Step {
                    var: var_name.clone(),
                    result: r,
                }),
            ParagraphStep::Subproof {
                uri,
                var_name,
                steps,
                .. /*
                method,
                justification,
                arguments,
                yields,
                */
            } => {
                let curr = context.context.len();
                let mut results = Vec::with_capacity(steps.len());
                for s in steps {
                    if let Some(r) = self.proof_step(s, context, block) {
                        let success = r.success();
                        results.push(r);
                        if !success {
                            return Some(ProofStepResult::Subproof {
                                uri: uri.clone(),
                                var: var_name.clone(),
                                results,
                            })
                        }
                    }
                }

                if matches!(steps.last(), Some(ParagraphStep::ProofConclusion { .. })) {
                    context.context.pop();
                }
                let (tp, df) = context.make_conclusion(self.hoas().expect("checked earlier"),curr);
                let var = var_name.as_ref().map_or_else(
                    || Variable::Name {
                        name: context.dummy(),
                        notated: None,
                    },
                    |uri| Variable::Ref {
                        declaration: uri.clone(),
                        is_sequence: None,
                    },
                );
                if let Some(v) = var_name.as_ref().and_then(|vn| self.get_variable(vn).ok()) {
                    if let Some(tp) = &tp {
                        v.data.tp.set_checked(tp.clone());
                        let pres = self.revert_prepare(tp.clone());
                        v.data.tp.set_presentation(pres);
                    }
                    if let Some(df) = &df {
                        v.data.df.set_checked(df.clone());
                        let pres = self.revert_prepare(df.clone());
                        v.data.df.set_presentation(pres);
                    }
                }
                context.context.push((ComponentVar { var, tp, df }, false));
                // TODO something reasonable
                //r
                Some(ProofStepResult::Subproof {
                    uri: uri.clone(),
                    var: var_name.clone(),
                    results,
                })
            }
        }
    }

    fn step_data_i(
        &self,
        context: &mut ProofCheckState,
        hoas: &HOASSymbols,
        var_name: Option<&DocumentElementUri>,
        method: Option<&Term>,
        justification: Option<&Term>,
        arguments: &[Option<(Term, SourceRange)>],
        yields: Option<&Term>,
        needs_def: bool,
        block: Option<&SymbolUri>,
    ) -> (Option<ProofStepCheckResult>, Option<Term>, Option<Term>) {
        let mut tp = None;
        let mut df = None;
        let mut proof_log = None;
        let var = var_name.and_then(|vn| self.get_variable(vn).ok());
        if let Some(tm) = yields {
            let (unks, tm) = self.prepare(None, tm.clone());
            let (b, nunks, l) = context.check_inhabitable(self, unks.clone(), &tm, block);
            if Some(true) == b {
                proof_log = Some(ProofStepCheckResult::GoalOnly {
                    result: TypeCheckResult {
                        success: b.unwrap_or_default(),
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    },
                });
                tp = Some(self.wrap_none(Some(nunks), |slf| slf.subst(tm)).1);
            } else {
                let tm = hoas.wrap_judg(&tm);
                let (b, unks, l2) = context.check_inhabitable(self, unks, &tm, block);
                proof_log = Some(ProofStepCheckResult::GoalOnly {
                    result: TypeCheckResult {
                        success: b.unwrap_or_default(),
                        log: if Some(true) == b {
                            CheckLog::from_pre(l2, &mut |t| self.revert_prepare(t))
                        } else {
                            CheckLog::Strategy {
                                name: "Checking provability".to_string(),
                                steps: vec![
                                    CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                                    CheckLog::from_pre(l2, &mut |t| self.revert_prepare(t)),
                                ],
                                success: false,
                            }
                        },
                    },
                });
                tp = Some(
                    self.wrap_none(Some(unks), |slf| slf.subst(tm.into_owned()))
                        .1,
                );
            }
        }
        if let Some(tm) = justification {
            let (unks, tm) = self.prepare(None, tm.clone());
            if let Some(tp) = tp.as_ref() {
                let Some(ProofStepCheckResult::GoalOnly { result }) = proof_log.take() else {
                    panic!("bug");
                };
                let (b, unks, l) = context.check_type(self, unks, &tm, tp, block);
                df = Some(self.wrap_none(Some(unks), |slf| slf.subst(tm)).1);
                proof_log = Some(ProofStepCheckResult::Both {
                    inhabitable: result,
                    matches: Some(TypeCheckResult {
                        success: b.unwrap_or_default(),
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    }),
                });
            } else {
                let (r, mut unks, l) = context.infer(self, unks, &tm, block);
                let infed = r.clone().map(|t| self.revert_prepare(t));
                proof_log = Some(ProofStepCheckResult::ProofOnly {
                    inferred: infed,
                    log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                });
                tp = r.map(|t| {
                    let (unks2, t) =
                        self.wrap_none(Some(std::mem::take(&mut unks)), |slf| slf.subst(t));
                    unks = unks2;
                    t
                });
                let d = self.wrap_none(Some(unks), |slf| slf.subst(tm)).1;
                if let Some(tp) = tp.as_ref() {
                    df = Some(ProofBarrier::apply(d, tp.clone()));
                } else {
                    df = Some(d);
                }
            }
        }
        if df.is_none() {
            if let Some(tm) = method {
                let (tm, unks) = hoas.apply(
                    self,
                    tm,
                    arguments.iter().map(|o| o.as_ref().map(|(t, _)| t.clone())),
                );
                let (unks, tm) = self.prepare(Some(unks), tm.into_owned());
                if let Some(tp) = &tp {
                    let Some(ProofStepCheckResult::GoalOnly { result }) = proof_log.take() else {
                        panic!("bug");
                    };
                    let (b, unks, l) = context.check_type(self, unks, &tm, tp, block);
                    let d = self.wrap_none(Some(unks), |slf| slf.subst(tm)).1;
                    df = Some(ProofBarrier::apply(d, tp.clone()));

                    proof_log = Some(ProofStepCheckResult::Both {
                        inhabitable: result,
                        matches: Some(TypeCheckResult {
                            success: b.unwrap_or_default(),
                            log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                        }),
                    });
                } else {
                    let (r, mut unks, l) = context.infer(self, unks, &tm, block);
                    let infed = r.clone().map(|t| self.revert_prepare(t));
                    proof_log = Some(ProofStepCheckResult::ProofOnly {
                        inferred: infed,
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    });
                    tp = r.map(|t| {
                        let (unks2, t) =
                            self.wrap_none(Some(std::mem::take(&mut unks)), |slf| slf.subst(t));
                        unks = unks2;
                        t
                    });

                    let d = self.wrap_none(Some(unks), |slf| slf.subst(tm)).1;
                    if let Some(tp) = tp.as_ref() {
                        df = Some(ProofBarrier::apply(d, tp.clone()));
                    } else {
                        df = Some(d);
                    }
                }
            } else if needs_def && let Some(tp) = tp.as_ref() {
                let Some(ProofStepCheckResult::GoalOnly { result }) = proof_log.take() else {
                    panic!("bug");
                };
                let (r, unks, l) = context.prove(self, Solutions::default(), tp, block);
                df = r.map(|t| {
                    ProofBarrier::apply(
                        self.wrap_none(Some(unks), |slf| slf.subst(t)).1,
                        tp.clone(),
                    )
                });
                proof_log = Some(ProofStepCheckResult::Both {
                    inhabitable: result,
                    matches: Some(TypeCheckResult {
                        success: df.is_some(),
                        log: CheckLog::from_pre(l, &mut |t| self.revert_prepare(t)),
                    }),
                });
            }
        }

        if let Some(v) = var {
            if let Some(tp) = &tp {
                v.data.tp.set_checked(tp.clone());
                let pres = self.revert_prepare(tp.clone());
                v.data.tp.set_presentation(pres);
            }
            if let Some(df) = &df {
                v.data.df.set_checked(df.clone());
                let pres = self.revert_prepare(df.clone());
                v.data.df.set_presentation(pres);
            }
        }
        if needs_def
            && df.is_none()
            && let Some(ProofStepCheckResult::GoalOnly { result }) = proof_log.as_mut()
        {
            result.success = false;
            result.log.add_failure("Unproven goal");
        }
        (proof_log, tp, df)
    }

    fn step_data(
        &mut self,
        context: &mut ProofCheckState,
        var_name: Option<&DocumentElementUri>,
        method: Option<&Term>,
        justification: Option<&Term>,
        arguments: &[Option<(Term, SourceRange)>],
        yields: Option<&Term>,
        is_assumption: bool,
        block: Option<&SymbolUri>,
    ) -> Option<ProofStepCheckResult> {
        /*
        if let Some(vn) = var_name
            && vn.name().as_ref().ends_with("proof/5.")
        {
            println!("HERE: {vn}");
            crate::DEBUG.store(true, std::sync::atomic::Ordering::Relaxed);
            print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
            println!("Debug mode on.");
            crate::pause();
        } */

        let (r, tp, df) = self.step_data_i(
            context,
            self.hoas()?,
            var_name,
            method,
            justification,
            arguments,
            yields,
            !is_assumption,
            block,
        );
        let var = var_name.map_or_else(
            || Variable::Name {
                name: context.dummy(),
                notated: None,
            },
            |uri| Variable::Ref {
                declaration: uri.clone(),
                is_sequence: None,
            },
        );
        let cv = ComponentVar { var, tp, df };
        context.context.push((cv, is_assumption));
        r
    }

    fn conclusion_step(
        &self,
        context: &mut ProofCheckState,
        var_name: Option<&DocumentElementUri>,
        method: Option<&Term>,
        justification: Option<&Term>,
        arguments: &[Option<(Term, SourceRange)>],
        yields: Option<&Term>,
        block: Option<&SymbolUri>,
    ) -> Option<ProofStepCheckResult> {
        let (r, tp, df) = self.step_data_i(
            context,
            self.hoas()?,
            var_name,
            method,
            justification,
            arguments,
            yields,
            true,
            block,
        );
        let var = var_name.map_or_else(
            || Variable::Name {
                name: context.dummy(),
                notated: None,
            },
            |uri| Variable::Ref {
                declaration: uri.clone(),
                is_sequence: None,
            },
        );
        context.conclusion_type.clone_from(&tp);
        context.conclusion_df.clone_from(&df);
        let cv = ComponentVar { var, tp, df };
        context.context.push((cv, false));
        r
    }
}

#[derive(Debug)]
struct ProofCheckState<'c> {
    context: &'c mut Vec<(ComponentVar, bool)>,
    counter: usize,
    conclusion_type: Option<Term>,
    conclusion_df: Option<Term>,
}

impl ProofCheckState<'_> {
    fn dummy(&mut self) -> Id {
        // SAFETY: valid ID
        let r = unsafe {
            format!("DUMMY_{}", self.counter + 1)
                .parse()
                .unwrap_unchecked()
        };
        self.counter += 1;
        r
    }
    fn make_conclusion(&mut self, hoas: &HOASSymbols, off: usize) -> (Option<Term>, Option<Term>) {
        self.context.drain(off..).rev().fold(
            (self.conclusion_type.take(), self.conclusion_df.take()),
            |(tp, df), (v, is_ass)| match (tp, df) {
                (Some(tp), None) if is_ass => (Some(hoas.pi(v, tp)), None),
                (None, Some(df)) if is_ass => (None, Some(hoas.lambda(v, df))),
                (Some(tp), Some(df)) if is_ass => {
                    (Some(hoas.pi(v.clone(), tp)), Some(hoas.lambda(v, df)))
                }
                (Some(tp), None) => (
                    (Some(if tp.has_free_such_that(|v2| v2.name() == v.var.name()) {
                        if let Some(df) = v.df {
                            tp / (v.var.name(), &df)
                        } else {
                            HOASSymbols::let_in(v, tp)
                        }
                    } else {
                        tp
                    })),
                    None,
                ),
                (None, Some(df)) => (
                    None,
                    Some(if let Some(d) = v.df {
                        df / (v.var.name(), &d)
                    } else {
                        HOASSymbols::let_in(v, df)
                    }),
                ),
                (Some(tp), Some(df)) => (
                    (Some(if tp.has_free_such_that(|v2| v2.name() == v.var.name()) {
                        if let Some(df) = &v.df {
                            tp / (v.var.name(), df)
                        } else {
                            HOASSymbols::let_in(v.clone(), tp)
                        }
                    } else {
                        tp
                    })),
                    Some(if let Some(d) = v.df {
                        df / (v.var.name(), &d)
                    } else {
                        HOASSymbols::let_in(v, df)
                    }),
                ),
                (None, None) => (None, None),
            },
        )
    }

    fn forget(&mut self, off: usize) {
        self.context.truncate(off);
    }

    fn check_inhabitable<Split: SplitStrategy>(
        &self,
        checker: &Checker<Split>,
        unks: Solutions,
        t: &Term,
        block: Option<&SymbolUri>,
    ) -> (Option<bool>, Solutions, PreCheckLog) {
        checker.wrap_task(CheckingTask::Inhabitable(t), Some(unks), |mut slf| {
            for (c, _) in &*self.context {
                slf.extend_context(c);
            }
            if let Some(blocked) = block {
                slf.context
                    .block_fact(GlobalOrLocal::Global(blocked.clone()));
            }
            slf.check_inhabitable_i(t)
        })
    }
    fn check_type<Split: SplitStrategy>(
        &self,
        checker: &Checker<Split>,
        unks: Solutions,
        tm: &Term,
        tp: &Term,
        block: Option<&SymbolUri>,
    ) -> (Option<bool>, Solutions, PreCheckLog) {
        checker.wrap_task(CheckingTask::HasType(tm, tp), Some(unks), |mut slf| {
            for (c, _) in &*self.context {
                slf.extend_context(c);
            }
            if let Some(blocked) = block {
                slf.context
                    .block_fact(GlobalOrLocal::Global(blocked.clone()));
            }
            slf.check_type_i(tm, tp)
        })
    }
    fn check_equal<Split: SplitStrategy>(
        &self,
        checker: &Checker<Split>,
        unks: Solutions,
        lhs: &Term,
        rhs: &Term,
        block: Option<&SymbolUri>,
    ) -> (Option<bool>, Solutions, PreCheckLog) {
        checker.wrap_task(CheckingTask::Equality(lhs, rhs), Some(unks), |mut slf| {
            for (c, _) in &*self.context {
                slf.extend_context(c);
            }
            if let Some(blocked) = block {
                slf.context
                    .block_fact(GlobalOrLocal::Global(blocked.clone()));
            }
            slf.check_equality_i(lhs, rhs)
        })
    }
    fn infer<Split: SplitStrategy>(
        &self,
        checker: &Checker<Split>,
        unks: Solutions,
        t: &Term,
        block: Option<&SymbolUri>,
    ) -> (Option<Term>, Solutions, PreCheckLog) {
        checker.wrap_task(CheckingTask::Inference(t), Some(unks), |mut slf| {
            for (c, _) in &*self.context {
                slf.extend_context(c);
            }
            if let Some(blocked) = block {
                slf.context
                    .block_fact(GlobalOrLocal::Global(blocked.clone()));
            }
            slf.infer_type_i(t)
        })
    }
    fn prove<Split: SplitStrategy>(
        &self,
        checker: &Checker<Split>,
        unks: Solutions,
        t: &Term,
        block: Option<&SymbolUri>,
    ) -> (Option<Term>, Solutions, PreCheckLog) {
        checker.wrap_task(CheckingTask::Proving(t), Some(unks), |mut slf| {
            for (c, _) in &*self.context {
                slf.extend_context(c);
            }
            if let Some(blocked) = block {
                slf.context
                    .block_fact(GlobalOrLocal::Global(blocked.clone()));
            }
            slf.prove_i(t)
        })
    }
}
