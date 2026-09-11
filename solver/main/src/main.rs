use std::{
    io::{Read, Write},
    path::Path,
};

use flams_math_archives::{
    Archive,
    backend::{AnyBackend, GlobalBackend, LocalBackend},
    source_files::SourceEntry,
    utils::AllSyncEngine,
};
use ftml_ontology::{
    domain::modules::{Module, ModuleLike},
    narrative::elements::DocumentElementRef,
    terms::Term,
    utils::{RefTree, time::measure},
};
use ftml_solver::{
    Checker, CheckerCache, SubtermCheckResult,
    results::{CheckResult, DocumentCheckResult},
    split::{
        RayonSplit, RayonStrategiesDepth, RayonStrategiesOnly, SingleThreadedSplit, SplitStrategy,
    },
};
use ftml_uris::{ArchiveId, DocumentUri, NamedUri, UriWithArchive, UriWithPath};

flams_math_archives::source_format!(STEX {
    name: "stex",
    description: "(Semantically annotated) LaTeX",
    targets: &[],
    file_extensions: &["tex", "ltx"],
    dependencies: |_| Vec::new()
});

fn main() {
    let _ = enable_ansi_support::enable_ansi_support();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    GlobalBackend::initialize::<AllSyncEngine>(false);
    /*let _ = std::thread::Builder::new()
    .stack_size(6 * 1024 * 1024)
    .spawn(move || {*/
    //pause();
    let (i, t) = measure(check_selected); //(topo_sort); //(check_all); //(check_subterm); //
    println!("Checked {i} documents in {t}");
    /*println!(
        "minimal stack: {}",
        bytesize::ByteSize::b(minimal_stack() as _)
            .display()
            .iec_short()
    );*/
    /*    })
    .expect("wut")
    .join();*/
}

fn topo_sort() -> usize {
    let alldocs = all_documents(|a| a.as_ref().starts_with("FTML/partial-orders"));
    let mods = alldocs.iter().flat_map(|d| {
        GlobalBackend
            .get_document(d)
            .expect("wut")
            .dfs()
            .filter_map(|d| {
                if let DocumentElementRef::Module { module, .. } = d {
                    Some(module.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    });
    let mut sorted = Vec::new();
    Module::topo_sort(mods.collect(), &mut sorted, |m| {
        if let Ok(ModuleLike::Module(m)) = GlobalBackend.get_module(m) {
            Some(m)
        } else {
            panic!("wut")
        }
    });
    let mut ret = 0;
    for s in sorted {
        let doc = GlobalBackend.get().with_local_archive(s.archive_id(), |a| {
            let a = a.expect("wut");
            a.document_of(s.path(), s.name())
        });
        if let Some(s) = doc {
            ret += 1;
            println!("\"{s}\",");
        }
    }
    ret
}

fn check_subterm() -> usize {
    /*
    const TOP: &str = r#"{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=FTML/math&p=propositions&m=equivalence&s=equivalence","presentation":null}},"arguments":[{"Simple":{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&m=subset&s=fuzzy subset","presentation":null}},"arguments":[{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&d=subset&l=en&e=vA","is_sequence":false}},"presentation":null}}},{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&d=subset&l=en&e=vB","is_sequence":false}},"presentation":null}}}],"presentation":null}}},{"Simple":{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&m=lift&s=less than or equal to","presentation":null}},"arguments":[{"Sequence":{"Seq":[{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&m=fuzzyset&s=membership function","presentation":null}},"arguments":[{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&d=subset&l=en&e=vA","is_sequence":false}},"presentation":null}}}],"presentation":null}},{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&m=fuzzyset&s=membership function","presentation":null}},"arguments":[{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&d=subset&l=en&e=vB","is_sequence":false}},"presentation":null}}}],"presentation":null}}]}}],"presentation":null}}}],"presentation":null}}"#;
    const SUB: &str = r#"{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&m=subset&s=fuzzy subset","presentation":null}},"arguments":[{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&d=subset&l=en&e=vA","is_sequence":null}},"presentation":null}}},{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Papers/26-Intelligencer-sTeX&p=mod&d=subset&l=en&e=vB","is_sequence":null}},"presentation":null}}}],"presentation":null}}"#; */
    const TOP: &str = r#"{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=Alonzo&m=test&s=(quasi-)function type","presentation":null}},"arguments":[{"Sequence":{"Seq":[{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Alonzo&d=test&l=en&e=A","is_sequence":false}},"presentation":null}}]}},{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Alonzo&d=test&l=en&e=B","is_sequence":false}},"presentation":null}}}],"presentation":null}}"#;
    const SUB: &str = r#"{"Application":{"head":{"Symbol":{"uri":"http://mathhub.info?a=Alonzo&m=test&s=(quasi-)function type","presentation":null}},"arguments":[{"Sequence":{"Seq":[{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Alonzo&d=test&l=en&e=A","is_sequence":null}},"presentation":null}}]}},{"Simple":{"Var":{"variable":{"Ref":{"declaration":"http://mathhub.info?a=Alonzo&d=test&l=en&e=B","is_sequence":null}},"presentation":null}}}],"presentation":null}}"#;
    let top: Term = serde_json::from_str(TOP).unwrap();
    let sub: Term = serde_json::from_str(SUB).unwrap();
    let mut global_context = rustc_hash::FxHashSet::default();
    for m in top.full_context(&mut |u| AnyBackend::Global.get_document(u).ok()) {
        global_context.insert(m);
    }
    let mut checker =
        Checker::<SingleThreadedSplit /*RayonStrategiesDepth<4>*/>::new(AnyBackend::Global);
    checker.set_context(global_context.into_iter().collect());
    let SubtermCheckResult {
        simplified,
        inferred_type,
        context,
        mut log,
    } = checker.check_subterm_term(top, sub).expect("failed");

    log.filter_failures(false);
    println!("{}", log.colored());
    println!(
        "{:?}\n  : {:?}",
        simplified.debug_short(),
        inferred_type.as_ref().map(Term::debug_short)
    );
    1
}

fn check_selected() -> usize {
    thread_local! {
        static CACHE: std::cell::Cell<CheckerCache> = std::cell::Cell::new(CheckerCache::default());
    }
    //for _ in 0..1 {
    macro_rules! check {
            ($($s:literal),* $(,)?) => {
                {
                    let mut i = 0;
                    $(
                        i += 1;
                        let mut solver = Checker::<SingleThreadedSplit/*RayonStrategiesDepth<4>*/>::new(AnyBackend::Global);
                        solver.set_cache(CACHE.take());
                        check(&mut solver,$s);
                        CACHE.set(solver.into_cache());
                    )*
                    i
                }
            }
        }
    check!(
        "http://mathhub.info?a=FTML/meta&d=Judgments&l=en",
        "http://mathhub.info?a=FTML/meta&d=Metatheory&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers/nat&d=nat&l=en",
        "http://mathhub.info?a=FTML/math&p=sets&d=set&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers/nat&d=natrange&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers&d=nat&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers/int&d=int&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers&d=int&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers/real&d=real&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=prop&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=negation&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=conjunction&l=en",
        "http://mathhub.info?a=FTML/math&p=sets&d=inset&l=en",
        "http://mathhub.info?a=FTML/math&p=functions&d=function&l=en",
        "http://mathhub.info?a=FTML/math&d=functions&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=equal&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers/real&d=order&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers/real&d=intervals&l=en",
        "http://mathhub.info?a=FTML/math&p=numbers&d=real&l=en",
        "http://mathhub.info?a=FTML/math&d=numbers&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=addition&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=subtraction&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=multiplication&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=division&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=exponentiation&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=logarithm&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs&d=judgment&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=exists&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs&d=choice-operator&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=sqrt&l=en",
        "http://mathhub.info?a=FTML/math&p=arithmetics&d=absolute-value&l=en",
        "http://mathhub.info?a=FTML/math&d=arithmetics&l=en",
        "http://mathhub.info?a=FTML/math&p=sets&d=cons&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=forall&l=en",
        "http://mathhub.info?a=FTML/math&p=sets&d=subset&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs&d=axiom&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs&d=inference-rule&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=true-introduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=false-elimination&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=implication&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=implication-introduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=implication-elimination&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=negation-introduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=negation-elimination&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=conjunction-introduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=conjunction-elimination&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=disjunction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=disjunction-introduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=disjunction-elimination&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=forall-introduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=forall-elimination&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=exists-introduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=exists-elimination&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=equality-reflexive&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=equality-congruence&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs&d=intuitionistic-natural-deduction&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction/extensions&d=conj-swap&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction&d=tertium-non-datur&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs&d=natural-deduction&l=en",
        "http://mathhub.info?a=FTML/math&d=proofs&l=en",
        "http://mathhub.info?a=FTML/math&p=sets&d=comprehension&l=en",
        "http://mathhub.info?a=FTML/math&p=sets&d=powerset&l=en",
        "http://mathhub.info?a=FTML/math&p=sets&d=cartesian-product&l=en",
        "http://mathhub.info?a=FTML/math&d=sets&l=en",
        "http://mathhub.info?a=FTML/math&d=si&l=en",
        "http://mathhub.info?a=FTML/math&p=relations&d=binary-relation&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=equivalence&l=en",
        "http://mathhub.info?a=FTML/math&p=propositions&d=unique&l=en",
        "http://mathhub.info?a=FTML/math&d=propositions&l=en",
        "http://mathhub.info?a=FTML/math&p=relations&d=reflexive&l=en",
        "http://mathhub.info?a=FTML/math&p=relations&d=symmetric&l=en",
        "http://mathhub.info?a=FTML/math&p=relations&d=transitive&l=en",
        "http://mathhub.info?a=FTML/math&p=relations&d=directed-graph&l=en",
        "http://mathhub.info?a=FTML/math&p=relations&d=graph&l=en",
        "http://mathhub.info?a=FTML/math&p=relations&d=antisymmetric&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra/operations&d=operation&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra&d=magma&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra/operations&d=associative&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra&d=semigroup&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra/operations&d=commutative&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra/operations&d=idempotent&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra&d=semigroup-exts&l=en",
        "http://mathhub.info?a=FTML/math&d=relations&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction/extensions&d=equality-transitive&l=en",
        "http://mathhub.info?a=FTML/math&p=proofs/natural-deduction/extensions&d=equality-symmetric&l=en",
        "http://mathhub.info?a=FTML/math&d=math&l=en",
        "http://mathhub.info?a=FTML/math&d=defeq&l=en",
        "http://mathhub.info?a=FTML/math&d=prelude&l=en",
        "http://mathhub.info?a=FTML/math&p=functions&d=pointwise&l=en",
        "http://mathhub.info?a=FTML/math&p=functions/reals&d=pointwise-max&l=en",
        "http://mathhub.info?a=FTML/math&p=categories&d=category&l=en",
        "http://mathhub.info?a=FTML/math&p=categories&d=functor&l=en",
        "http://mathhub.info?a=FTML/math&p=categories&d=natural-transformation&l=en",
        "http://mathhub.info?a=FTML/math&p=categories&d=Cat&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra/operations&d=unit&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra/operations&d=unit-is-unique&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra&d=unital&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra&d=monoid&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra/operations&d=inverses&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra&d=operations&l=en",
        "http://mathhub.info?a=FTML/math&d=algebra&l=en",
        "http://mathhub.info?a=FTML/math&p=algebra&d=group&l=en",
        //
        "http://mathhub.info?a=FTML/partial-orders&d=bounds&l=en",
        "http://mathhub.info?a=FTML/partial-orders&d=dual-partial-order&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=glb-unique&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=lub-unique&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices&d=lattice&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=lub-idempotent&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices&d=dual-lattice&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=lub-glb-absorption-1&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=lub-glb-absorption-2&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=lub-commutative&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=glb-associative&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=lub-associative&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=glb-idempotent&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lemmata&d=glb-commutative&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=semilattice&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=lattice&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices&d=algebraic-lattice&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=join-has-lub&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=join-implies-meet&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=meet-implies-join&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=join-has-glb&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=order-lattice&l=en",
        "http://mathhub.info?a=FTML/partial-orders&p=lattices/algebraic&d=dual-lattice&l=en",
        //
        "http://mathhub.info?a=FTML/demos&p=sqrt2/numbertheory&d=divisibility&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/numbertheory&d=coprimality&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/rationals&d=rational&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/rationals&d=real-mult-laws&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/rationals&d=rational-square-bridge&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/peano&d=zero-not-successor&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/peano&d=successor-injective&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/peano&d=induction&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/peano&d=add-recursion&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=add-associative&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=add-zero-left&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=add-succ-left&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=add-commutative&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/peano&d=mult-recursion&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=mult-zero-left&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=mult-succ-left&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=mult-commutative&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=distributivity&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=cancellation&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=mult-two-is-doubling&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=two-is-not-one&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/numbertheory&d=parity&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/numbertheory&d=parity-totality&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/numbertheory&d=odd-square-odd&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=even-odd-disjoint&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/numbertheory&d=square-even-implies-even&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/numbertheory&d=coprime-descent&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/rationals&d=rational-square-contradiction&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2&d=sqrt2&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=nat-case-split&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2/arithmetic&d=mult-associative&l=en",
        "http://mathhub.info?a=FTML/demos&p=sqrt2&d=all&l=en",
    )
    //}
}

fn check<Split: SplitStrategy>(solver: &mut Checker<Split>, s: &str) {
    println!("Checking {s}");
    let d = GlobalBackend
        .get_document(&s.parse().expect("uri wut"))
        .expect("wut");
    let ((mut v, _), t) = measure(|| solver.check_document(&d).expect("dependency missing"));
    let failures = count_fails(&v);

    //let mut linewise = EveryLine::new();
    /*for mut c in v.checks {
        if let CheckResult::Term { .. } = c {
            continue;
        }
        c.filter_failures();
        println!("{}", c.colored()); //write!(linewise, "{}", c.colored());
        if !c.success() {
            pause(); //break;
        }
    }*/

    //let outfile = Path::new("/home/jazzpirate/work/Software/FlexiFormal/FLAMS/solver/out.txt");
    //let mut outfile = std::fs::File::create(outfile).expect("wut");

    //v.filter_failures(true);
    v.checks = v
        .checks
        .into_iter()
        .filter(|c| !matches!(c, CheckResult::Term { .. }))
        /* .map(|mut c| {
            c.filter_failures();
            c
        })*/
        .collect();
    for mut c in v.checks {
        c.filter_failures(true);
        //writeln!(outfile, "{}", c.display::<()>()).expect("fuck");
        println!("{}", c.colored());
        if !c.success() {
            pause();
        }
    }
    //println!("{}", v.colored());
    //pause();

    //v.filter_failures();
    //println!("{}", v.colored());
    //println!("Checked after {t}");

    /*
    if failures == 0 {
        println!("Checked after {t}");
    } else {
        //v.filter_failures();
        println!("{}", v.colored());
        println!("Checked after {t}");
        pause();
    }
    */
}

fn check_all() -> usize {
    static PATH: &str = "/home/jazzpirate/work/Software/FlexiFormal/FLAMS/solver/foo.txt";
    const SAVE: bool = false;
    const LOAD: bool = true;
    let read_file = if LOAD {
        std::fs::read_to_string(PATH)
            .expect("möp")
            .split("\n\n%%%%%%%%%%\n\n")
            .filter_map(|s| {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s.trim().to_string())
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut out_file = if SAVE {
        Some(std::io::BufWriter::new(
            std::fs::File::create(PATH).expect("möp"),
        ))
    } else {
        None
    };
    let alldocs = all_documents(|a| a.as_ref().starts_with("FTML/math"));
    let mut dones = 0;
    let mut failures = 0;

    /* TOPO sort
    let mods = alldocs.iter().flat_map(|d| {
        GlobalBackend
            .get_document(&d)
            .expect("wut")
            .dfs()
            .filter_map(|d| {
                if let DocumentElementRef::Module { module, .. } = d {
                    Some(module.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    });
    let mut sorted = Vec::new();
    Module::topo_sort(mods.collect(), &mut sorted, |m| {
        if let Ok(ModuleLike::Module(m)) = GlobalBackend.get_module(m) {
            Some(m)
        } else {
            panic!("wut")
        }
    });
    for s in sorted {
        println!("{s}");
    }
    std::process::exit(0);
    */

    for d in alldocs {
        let mut solver =
            Checker::</*SingleThreadedSplit */ RayonStrategiesDepth<4>>::new(AnyBackend::Global);
        let Ok(d) = GlobalBackend.get_document(&d) else {
            continue;
        };
        println!("{}", d.uri);
        let ((mut v, _), t) = measure(|| solver.check_document(&d).expect("dependency missing"));
        let fails = count_fails(&v);
        failures += fails;
        if fails != 0 {
            v.filter_failures(false);
            let vs = v.display::<()>().to_string();
            if let Some(f) = out_file.as_mut() {
                let _ = f.write_all(format!("{vs}\n\n%%%%%%%%%%\n\n").as_bytes());
            }
            if !read_file.contains(&vs) {
                println!("{}", v.colored());
            }
        }
        println!("Checked after {t}");
        dones += 1;
    }
    println!("Total failures: {failures}");
    dones
}

fn count_fails(res: &DocumentCheckResult) -> usize {
    res.checks.iter().filter(|c| !c.success()).count()
}

fn pause() {
    use std::io::{Read, Write};
    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    write!(
        stdout,
        r"
        --------------------------------------------------------------------------
        Press any key to continue..

    "
    )
    .expect("wut");
    let _ = stdout.flush();
    let _ = stdin.read(&mut [0u8]);
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
}

#[allow(clippy::needless_collect)]
pub fn all_documents(filter: fn(&ArchiveId) -> bool) -> Vec<DocumentUri> {
    use flams_math_archives::MathArchive;
    let backend = flams_math_archives::backend::GlobalBackend.get();
    let uris = backend
        .all_archives()
        .iter()
        .filter_map(|a| {
            let Archive::Local(a) = a else { return None };
            if !filter(a.id()) {
                return None;
            }
            Some(a.with_sources(|src| {
                src.dfs()
                    .filter_map(|d| {
                        let SourceEntry::File(f) = d else { return None };
                        Some(a.source_dir().join(f.relative_path.as_ref()))
                    })
                    .collect::<Vec<_>>()
            }))
        })
        .flatten()
        .collect::<Vec<_>>();
    let uris = uris
        .into_iter()
        .filter_map(|f| backend.uri_of(&f))
        .collect::<Vec<_>>();
    tracing::info!("{} documents", uris.len());
    uris
}

struct EveryLine(std::io::Stdout, std::io::Stdin);
impl EveryLine {
    fn new() -> Self {
        Self(std::io::stdout(), std::io::stdin())
    }
}
impl std::io::Write for EveryLine {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut lines = buf.split(|b| *b == b'\n');
        let last = lines.next_back();
        for l in lines {
            self.0.write(l)?;
            self.0.flush()?;
            let _ = self.1.read(&mut [0]);
        }
        if let Some(last) = last {
            self.0.write(last)?;
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

fn get_module(s: &str) -> Module {
    let ModuleLike::Module(m) = GlobalBackend
        .get()
        .get_module(&s.parse().expect("wut"))
        .expect("wut")
    else {
        panic!("wut")
    };
    m
}
