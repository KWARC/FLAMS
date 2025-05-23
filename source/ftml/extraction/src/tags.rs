use crate::extractor::FTMLExtractor;
use crate::open::OpenFTMLElement;
use crate::rules::FTMLExtractionRule;
use flams_ontology::ftml::FTMLKey;
use paste::paste;

macro_rules! do_tags {
    ($($tag:ident $(@$f:ident)?),*) => {
        paste! {
            //impl FTMLTagExt for FTMLKey {
                #[must_use]#[inline]
                pub const fn all_rules<E:FTMLExtractor>() -> [FTMLExtractionRule<E>;flams_ontology::ftml::NUM_RULES] {[$(
                    rule(FTMLKey::$tag)
                ),*]}
                #[must_use]#[inline]
                pub const fn rule<E:FTMLExtractor>(key:FTMLKey) -> FTMLExtractionRule<E> {
                    match key {$(
                        FTMLKey::$tag =>
                            FTMLExtractionRule::new(key,FTMLKey::$tag.attr_name(),do_tags!(@FUN $tag $($f)?))
                    ),*}
                }
            //}
        }

    };
    (@FUN $tag:ident None) => {no_op};
    (@FUN $tag:ident $i:ident) => {super::rules::rules::$i};
    (@FUN $tag:ident ) => {|a,b,c| todo(a,b,c,FTMLTag::$tag)}
}

do_tags! {
    Module                      @ module,
    MathStructure               @ mathstructure,
    Morphism                    @ morphism,
    Section                     @ section,
    SkipSection                 @ skipsection,

    Definition                  @ definition,
    Paragraph                   @ paragraph,
    Assertion                   @ assertion,
    Example                     @ example,
    Problem                     @ problem,
    SubProblem                  @ subproblem,
    Slide                       @ slide,
    SlideNumber                 @ slide_number,

    DocTitle                    @ doctitle,
    Title                       @ title,
    ProofTitle                  @ prooftitle,
    SubproofTitle               @ subprooftitle,

    Symdecl                     @ symdecl,
    Vardef                      @ vardecl,
    Varseq                      @ varseq,

    Notation                    @ notation,
    NotationComp                @ notationcomp,
    NotationOpComp              @ notationopcomp,
    Definiendum                 @ definiendum,

    Type                        @ r#type,
    Conclusion                  @ conclusion,
    Definiens                   @ definiens,
    Rule                        @ mmtrule,

    ArgSep                      @ argsep,
    ArgMap                      @ argmap,
    ArgMapSep                   @ argmapsep,

    Term                        @ term,
    Arg                         @ arg,
    HeadTerm                    @ headterm,

    ImportModule                @ importmodule,
    UseModule                   @ usemodule,
    InputRef                    @ inputref,

    SetSectionLevel             @ setsectionlevel,

    Style                       @ style_rule,
    CounterParent               @ counter_parent,
    Counter                     @ counter_parent,


    Proof                       @ proof,
    SubProof                    @ subproof,
    ProofMethod                 @ no_op /* TODO */,
    ProofSketch                 @ no_op /* TODO */,
    ProofTerm                   @ no_op /* TODO */,
    ProofBody                   @ proofbody,
    ProofAssumption             @ no_op /* TODO */,
    ProofHide                   @ no_op,
    ProofStep                   @ no_op /* TODO */,
    ProofStepName               @ no_op /* TODO */,
    ProofEqStep                 @ no_op /* TODO */,
    ProofPremise                @ no_op /* TODO */,
    ProofConclusion             @ no_op /* TODO */,

    PreconditionDimension       @ precondition,
    PreconditionSymbol          @ no_op,
    ObjectiveDimension          @ objective,
    ObjectiveSymbol             @ no_op,
    ProblemMinutes              @ no_op /* TODO */,

    ProblemFillinsol            @ fillinsol,
    ProblemFillinsolWidth       @ no_op,
    ProblemFillinsolCase        @ fillinsol_case,
    ProblemFillinsolCaseValue   @ no_op,
    ProblemFillinsolCaseVerdict @ no_op,

    ProblemNote                 @ no_op /* TODO */,
    ProblemSolution            @ solution,
    ProblemHint                @ problem_hint,
    ProblemGradingNote         @ gnote,

    ProblemMultipleChoiceBlock  @ multiple_choice_block,
    ProblemSingleChoiceBlock    @ single_choice_block,
    ProblemChoice               @ problem_choice,
    ProblemChoiceVerdict        @ problem_choice_verdict,
    ProblemChoiceFeedback       @ problem_choice_feedback,

    AnswerClass                 @ answer_class,
    AnswerclassFeedback         @ ac_feedback,
    AnswerClassPts              @ no_op,

    Comp                        @ comp,
    VarComp                     @ comp,
    MainComp                    @ maincomp,
    DefComp                     @ defcomp,

    Invisible                   @ invisible,

    IfInputref                  @ ifinputref,
    ReturnType                  @ no_op /* TODO */,
    ArgTypes                    @ no_op /* TODO */,

    SRef                        @ no_op /* TODO */,
    SRefIn                      @ no_op /* TODO */,
    Slideshow                   @ no_op /* TODO */,
    SlideshowSlide              @ no_op /* TODO */,
    CurrentSectionLevel         @ no_op /* TODO */,
    Capitalize                  @ no_op /* TODO */,

    Assign                      @ assign,
    Rename                      @ no_op /* TODO */,
    RenameTo                    @ no_op /* TODO */,
    AssignMorphismFrom          @ no_op /* TODO */,
    AssignMorphismTo            @ no_op /* TODO */,

    AssocType                   @ no_op,
    ArgumentReordering          @ no_op,
    ArgNum                      @ no_op,
    Bind                        @ no_op,
    ProblemPoints               @ no_op,
    Autogradable                @ no_op,
    MorphismDomain              @ no_op,
    MorphismTotal               @ no_op,
    ArgMode                     @ no_op,
    NotationId                  @ no_op,
    Head                        @ no_op,
    Language                    @ no_op,
    Metatheory                  @ no_op,
    Signature                   @ no_op,
    Args                        @ no_op,
    Macroname                   @ no_op,
    Inline                      @ no_op,
    Fors                        @ no_op,
    Id                          @ no_op,
    NotationFragment            @ no_op,
    Precedence                  @ no_op,
    Role                        @ no_op,
    Styles                      @ no_op,
    Argprecs                    @ no_op
}
#[allow(dead_code)]
pub const fn ignore<E: FTMLExtractor>(key: FTMLKey) -> FTMLExtractionRule<E> {
    FTMLExtractionRule::new(key, key.attr_name(), super::rules::rules::no_op)
}
#[allow(dead_code)]
pub const fn no_op<E: FTMLExtractor>(
    _extractor: &mut E,
    _attrs: &mut E::Attr<'_>,
    _nexts: &mut super::rules::rules::SV<E>,
) -> Option<OpenFTMLElement> {
    None
}

#[allow(dead_code)]
pub fn todo<E: FTMLExtractor>(
    _extractor: &mut E,
    _attrs: &mut E::Attr<'_>,
    _nexts: &mut super::rules::rules::SV<E>,
    tag: FTMLKey,
) -> Option<OpenFTMLElement> {
    todo!("Tag {}", tag.as_str())
}
