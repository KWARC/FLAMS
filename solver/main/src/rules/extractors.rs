use super::operators::*;
use crate::{
    rules::{RuleSet, SimplificationRule},
    split::SplitStrategy,
};
use ftml_ontology::{
    domain::declarations::symbols::{AssocType, Symbol},
    terms::{
        ApplicationTerm, Argument, BindingTerm, BoundArgument, ComponentVar, Term,
        patterns::Pattern,
    },
};
use ftml_solver_trace::SizedSolverRule;
use ftml_uris::{Id, SymbolUri};

pub type SymbolRuleExtractor<Split> = fn(&Symbol, &mut RuleSet<Split>);
pub type RuleExtractor<Split> = (&'static str, fn(&[Term], &mut RuleSet<Split>));

#[must_use]
pub const fn all_symbol_extractors<Split: SplitStrategy>() -> &'static [SymbolRuleExtractor<Split>]
{
    &[
        prenex,
        binl,
        binr,
        conj,
        pwconj,
        reorder,
        of_type,
        apply,
        lambda,
        pi,
        prop,
        judgment,
        universe,
        inhabitable,
        any,
        implicit,
        conjunction,
        map,
        letrule,
        numnat,
        numposnat,
        numint,
        numnegint,
        numnonzeroint,
        numrat,
        numposrat,
        numnegrat,
        numnonzerorat,
        numreal,
        numposreal,
        numnegreal,
        numnonzeroreal,
        numcomplex,
        addition,
        multiplication,
        exponentiation,
        division,
        subtraction,
        logarithm,
        leq,
        max,
    ]
}
#[must_use]
pub const fn all_rule_extractors<Split: SplitStrategy>() -> &'static [RuleExtractor<Split>] {
    &[
        ("hoas-lambda-pi-apply", hoas_lpa),
        ("arrow-for-pi", arrow_for),
        ("hoas-bindin", bind_in),
        ("intersection-type", intersection),
        ("inhabitable", inhab),
        ("universe", univ),
        ("subtype", subtp),
        ("record-universe", record_universe),
        ("forallEI", forall_ei),
        ("complex", super::symbols::parse),
    ]
}

macro_rules! rules {
    (
        $(
            $v:vis $name:ident $( ($id:expr) )? = ($symbol:ident,$rules:ident) => $b:block
        )*
    ) => {
        $( rules!(@I $v $name $(($id))? = ($symbol,$rules) => $b ); )*
    };
    (@I $v:vis $name:ident($id:expr) = ($symbol:ident,$rules:ident) => $b:block) => {
        #[allow(unused_variables)]
        $v fn $name<Split: SplitStrategy>($symbol:&Symbol,$rules:&mut RuleSet<Split>) {
            static ID: std::sync::LazyLock<Id> =
                std::sync::LazyLock::new(|| unsafe { $id.parse().unwrap_unchecked() });
            if $symbol.data.role.contains(&ID) $b
        }
    };
    (@I $v:vis $name:ident = ($symbol:ident,$rules:ident) => $b:block) => {
        rules!(@I $v $name(stringify!($name)) = ($symbol,$rules) => $b);
    }
}

pub fn subtp<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let [sub, sup] = params else { return };
    rules.push_subtyping(Box::new(typing::Subtyping {
        sub: Pattern::from(sub.clone(), false),
        sup: Pattern::from(sup.clone(), false),
    }));
}

pub fn forall_ei<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let [
        Term::Symbol { uri: foral, .. },
        Term::Symbol { uri: proof, .. },
        Term::Symbol { uri: pi, .. },
    ] = params
    else {
        return;
    };
    rules.push_simplification(Box::new(ForallSimp {
        foral: foral.clone(),
        proof: proof.clone(),
        pi: pi.clone(),
    }));
}

pub fn inhab<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let [term] = params else { return };
    rules.push_inhabitable(Box::new(universe::ComplexInhabitableRule(Pattern::from(
        term.clone(),
        false,
    ))));
}

pub fn record_universe<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let [term] = params else { return };
    rules.push_inference(Box::new(crate::impls::records::RecordUniverse(
        term.clone(),
    )));
}

pub fn univ<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let [term] = params else {
        //tracing::error!("Parameters don't match: {params:?}");
        return;
    };
    let rl = universe::ComplexUniverseRule(Pattern::from(term.clone(), false));
    //tracing::warn!("New universe rule: {rl:?}");
    rules.push_inhabitable(Box::new(rl.clone()));
    rules.push_universe(Box::new(rl));
}

pub fn intersection<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let [
        Term::Symbol { uri: intersect, .. },
        Term::Symbol { uri: lambda, .. },
        Term::Symbol { uri: pi, .. },
    ] = params
    else {
        return;
    };
    rules.push_inhabitable(Box::new(intersection::IntersectionTypeInhabitable(
        intersect.clone(),
    )));
    rules.push_universe(Box::new(intersection::IntersectionTypeInhabitable(
        intersect.clone(),
    )));
    rules.push_marker(Box::new(intersection::intersect_pi_extension(
        intersect.clone(),
        pi.clone(),
    )));
}

pub fn bind_in<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let [
        Term::Symbol { uri: bindin, .. },
        Term::Symbol { uri: bind, .. },
    ] = params
    else {
        return;
    };
    rules.push_inhabitable(Box::new(bindin::BindInInhabitableRule {
        bindin: bindin.clone(),
        bind: bind.clone(),
    }));
    rules.push_preparation(Box::new(pi::NeedsTypeRule(bindin.clone())));
    rules.push_inference(Box::new(bindin::BindInInferenceRule {
        bindin: bindin.clone(),
        bind: bind.clone(),
    }));
    rules.push_inference(Box::new(bindin::BindInApplyRule {
        bindin: bindin.clone(),
        bind: bind.clone(),
    }));
    rules.push_simplification(Box::new(bindin::BindInComputationRule {
        bindin: bindin.clone(),
        bind: bind.clone(),
    }));
}

pub fn arrow_for<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    if let [Term::Symbol { uri: head, .. }, Term::Symbol { uri: pi, .. }] = params {
        let rule = Box::new(pi::ArrowRule {
            arrow: head.clone(),
            pi: pi.clone(),
        });
        rules.push_preparation(rule.clone());
        rules.push_simplification(rule);
    }
}

pub fn hoas_lpa<Split: SplitStrategy>(params: &[Term], rules: &mut RuleSet<Split>) {
    let (lambda, pi, apply) = if let [
        Term::Symbol { uri: lambda, .. },
        Term::Symbol { uri: pi, .. },
        Term::Symbol { uri: apply, .. },
    ] = params
    {
        (lambda, pi, Some(apply))
    } else if let [
        Term::Symbol { uri: lambda, .. },
        Term::Symbol { uri: pi, .. },
    ] = params
    {
        (lambda, pi, None)
    } else {
        return;
    };
    rules.push_inhabitable(Box::new(pi::PiInhabitableRule(pi.clone())));
    rules.push_universe(Box::new(pi::PiUniverseRule(pi.clone())));
    rules.push_inference(Box::new(pi::PiInferenceRule(pi.clone())));
    rules.push_subtyping(Box::new(pi::PiVarianceRule(pi.clone())));
    rules.push_inference(Box::new(pi::LambdaPiInferenceRule {
        lambda: lambda.clone(),
        pi: pi.clone(),
    }));
    rules.push_checking(Box::new(pi::LambdaPiCheckingRule {
        lambda: lambda.clone(),
        pi: pi.clone(),
    }));
    rules.push_simplification(Box::new(pi::BetaRule(lambda.clone())));
    rules.push_simplification(Box::new(pi::EtaRule(lambda.clone())));
    rules.push_preparation(Box::new(pi::NeedsTypeRule(lambda.clone())));
    if lambda != pi {
        rules.push_preparation(Box::new(pi::NeedsTypeRule(pi.clone())));
    }
    rules.push_marker(Box::new(super::HOASRule {
        lambda: lambda.clone(),
        pi: pi.clone(),
        apply: apply.cloned(),
    }));
}

pub fn reorder<Split: SplitStrategy>(sym: &Symbol, rules: &mut RuleSet<Split>) {
    if let Some(perm) = &sym.data.reordering {
        rules.push_preparation(Box::new(super::fixity::ReorderRule {
            symbol: sym.uri.clone(),
            reorder: perm.clone(),
        }));
    }
}

pub fn prenex<Split: SplitStrategy>(sym: &Symbol, rules: &mut RuleSet<Split>) {
    if sym.data.assoctype.is_some_and(|at| at == AssocType::Prenex) {
        rules.push_preparation(Box::new(super::fixity::PrenexRule(sym.uri.clone())));
    }
}

pub fn conj<Split: SplitStrategy>(sym: &Symbol, rules: &mut RuleSet<Split>) {
    if sym
        .data
        .assoctype
        .is_some_and(|at| at == AssocType::Conjunctive)
    {
        rules.push_preparation(Box::new(super::fixity::ConjunctiveRule(sym.uri.clone())));
    }
}

pub fn pwconj<Split: SplitStrategy>(sym: &Symbol, rules: &mut RuleSet<Split>) {
    if sym
        .data
        .assoctype
        .is_some_and(|at| at == AssocType::PairwiseConjunctive)
    {
        rules.push_preparation(Box::new(super::fixity::PairwiseConjunctiveRule(
            sym.uri.clone(),
        )));
    }
}

pub fn binl<Split: SplitStrategy>(sym: &Symbol, rules: &mut RuleSet<Split>) {
    if sym
        .data
        .assoctype
        .is_some_and(|at| at == AssocType::LeftAssociativeBinary)
    {
        rules.push_preparation(Box::new(super::fixity::BinLRule(sym.uri.clone())));
    }
}

pub fn binr<Split: SplitStrategy>(sym: &Symbol, rules: &mut RuleSet<Split>) {
    if sym
        .data
        .assoctype
        .is_some_and(|at| at == AssocType::RightAssociativeBinary)
    {
        rules.push_preparation(Box::new(super::fixity::BinRRule(sym.uri.clone())));
    }
}

rules! {
    pub conjunction = (sym,rules) => {
        rules.push_marker(Box::new(super::fixity::IsConjunctionRule(sym.uri.clone())));
    }
    pub universe = (sym,rules) => {
        rules.push_inhabitable(Box::new(universe::SimpleUniverseRule(sym.uri.clone())));
        rules.push_universe(Box::new(universe::SimpleUniverseRule(sym.uri.clone())));
    }
    pub of_type("oftype") = (sym,rules) => {
        rules.push_preparation(Box::new(typing::SimpleTypeOperatorRule(sym.uri.clone())));
        rules.push_inference(Box::new(typing::SimpleTypeOperatorRule(sym.uri.clone())));
        rules.push_simplification(Box::new(typing::SimpleTypeOperatorRule(sym.uri.clone())));
    }
    pub apply = (sym,rules) => {
        rules.push_preparation(Box::new(pi::ApplyRule(sym.uri.clone())));
    }
    pub lambda = (sym,rules) => {

    }
    pub pi = (sym,rules) => {
        rules.push_inhabitable(Box::new(pi::PiInhabitableRule(sym.uri.clone())));
        rules.push_inference(Box::new(pi::PiInferenceRule(sym.uri.clone())));
    }
    pub prop = (sym,rules) => {

    }
    pub judgment = (sym,rules) => {
        rules.push_marker(Box::new(super::IsJudgmentRule(sym.uri.clone())));
    }
    pub inhabitable = (sym,rules) => {
        rules.push_inhabitable(Box::new(universe::SimpleInhabitableRule(sym.uri.clone(),sym.data.arity.num())));
    }
    pub any = (sym,rules) => {
        rules.push_inhabitable(Box::new(universe::SimpleInhabitableRule(sym.uri.clone(),0)));
        rules.push_subtyping(Box::new(universe::AnyRule(sym.uri.clone())));
        rules.push_checking(Box::new(universe::AnyRule(sym.uri.clone())));
        rules.push_marker(Box::new(universe::AnyRule(sym.uri.clone())));
    }
    pub implicit = (sym,rules) => {

    }
    pub map = (sym,rules) => {
        rules.push_inhabitable(Box::new(super::sequences::map::MapInhabitableRule(sym.uri.clone())));
        rules.push_simplification(Box::new(super::sequences::map::MapSimplificationRule(sym.uri.clone())));
        rules.push_simplification(Box::new(super::sequences::map::MapArgumentSimplificationRule(sym.uri.clone())));
        rules.push_simplification(Box::new(super::sequences::map::MapIndexSimplificationRule(sym.uri.clone())));
        rules.push_inference(Box::new(super::sequences::map::MapInferenceRule(sym.uri.clone())));
    }
    pub letrule("let") = (sym,rules) => {
        rules.push_simplification(Box::new(letin::LetinComputation(sym.uri.clone())));
    }

    pub numnat = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::Naturals,
            sym:sym.uri.clone()
        }));
    }
    pub numposnat = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ: numbers::NumberType::PositiveNaturals,
            sym:sym.uri.clone()
        }));
    }
    pub numint = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::Integers,
            sym:sym.uri.clone()
        }));
    }
    pub numnegint = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::NegativeIntegers,
            sym:sym.uri.clone()
        }));
    }
    pub numnonzeroint = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::NonZeroIntegers,
            sym:sym.uri.clone()
        }));
    }
    pub numrat = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::Rationals,
            sym:sym.uri.clone()
        }));
    }
    pub numposrat = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::PositiveRationals,
            sym:sym.uri.clone()
        }));
    }
    pub numnegrat = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::NegativeRationals,
            sym:sym.uri.clone()
        }));
    }
    pub numnonzerorat = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::NonZeroRationals,
            sym:sym.uri.clone()
        }));
    }
    pub numreal = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::Reals,
            sym:sym.uri.clone()
        }));
    }
    pub numposreal = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::PositiveReals,
            sym:sym.uri.clone()
        }));
    }
    pub numnegreal = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::NegativeReals,
            sym:sym.uri.clone()
        }));
    }
    pub numnonzeroreal = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::NonZeroReals,
            sym:sym.uri.clone()
        }));
    }
    pub numcomplex = (sym,rules) => {
        rules.push_marker(Box::new(numbers::NumberRule{
            typ:numbers::NumberType::Complex,
            sym:sym.uri.clone()
        }));
    }
    pub addition = (sym,rules) => {
        rules.push_simplification(Box::new(numbers::AdditionRule(sym.uri.clone())));
    }
    pub multiplication = (sym,rules) => {
        rules.push_simplification(Box::new(numbers::MultiplicationRule(sym.uri.clone())));
    }
    pub division = (sym,rules) => {
        rules.push_simplification(Box::new(numbers::DivisionRule(sym.uri.clone())));
    }
    pub exponentiation = (sym,rules) => {
        rules.push_simplification(Box::new(numbers::ExponentiationRule(sym.uri.clone())));
    }
    pub subtraction = (sym,rules) => {
        rules.push_simplification(Box::new(numbers::SubtractionRule(sym.uri.clone())));
    }
    pub logarithm = (sym,rules) => {
        rules.push_simplification(Box::new(numbers::Logarithm(sym.uri.clone())));
    }
    pub leq = (sym,rules) => {
        rules.push_proof(Box::new(numbers::LessThan(sym.uri.clone())));
    }
    pub max = (sym,rules) => {
        rules.push_simplification(Box::new(numbers::Max(sym.uri.clone())));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForallSimp {
    foral: SymbolUri,
    proof: SymbolUri,
    pi: SymbolUri,
}
impl SizedSolverRule for ForallSimp {
    fn display(&self) -> Vec<crate::trace::Displayable> {
        ftml_solver_trace::trace!(
            &self.proof,
            "(",
            &self.foral,
            " x:T. P)  ==>  ",
            &self.pi,
            " x:T. ",
            &self.proof,
            " P"
        )
    }
}
impl<Split: SplitStrategy> SimplificationRule<Split> for ForallSimp {
    fn applicable(&self, term: &Term) -> bool {
        if let Term::Application(app) = term
            && let Term::Symbol { uri, .. } = &app.head
            && *uri == self.proof
            && let [Argument::Simple(Term::Bound(forall))] = &*app.arguments
            && let Term::Symbol { uri, .. } = &forall.head
            && *uri == self.foral
            && let [
                BoundArgument::Simple(_), // tp
                BoundArgument::Bound(_),
                BoundArgument::Simple(_),
            ] = &*forall.arguments
        {
            true
        } else {
            false
        }
    }
    fn apply<'t>(
        &self,
        mut checker: crate::CheckRef<'t, '_, Split>,
        term: &'t Term,
    ) -> Result<Term, Option<ftml_ontology::terms::termpaths::TermPath>> {
        if let Term::Application(app) = term
            && let Term::Symbol { uri, .. } = &app.head
            && *uri == self.proof
            && let [Argument::Simple(Term::Bound(forall))] = &*app.arguments
            && let Term::Symbol { uri, .. } = &forall.head
            && *uri == self.foral
            && let [
                BoundArgument::Simple(tp),
                BoundArgument::Bound(v),
                BoundArgument::Simple(bd),
            ] = &*forall.arguments
        {
            if let Some(otp) = &v.tp
                && checker.check_equality(tp, otp) != Some(true)
            {
                return Err(None);
            }

            Ok(Term::Bound(BindingTerm::new(
                self.pi.clone().into(),
                Box::new([
                    BoundArgument::Bound(ComponentVar {
                        var: v.var.clone(),
                        tp: Some(tp.clone()),
                        df: None,
                    }),
                    BoundArgument::Simple(Term::Application(ApplicationTerm::new(
                        self.proof.clone().into(),
                        Box::new([Argument::Simple(bd.clone())]),
                        None,
                    ))),
                ]),
                None,
            )))
        } else {
            Err(None)
        }
    }
}
