pub mod results;

use ftml_ontology::terms::{ComponentVar, Term, Variable};
use ftml_uris::{FtmlUri, Uri, UriRef};
#[cfg(feature = "colors")]
use owo_colors::OwoColorize;
use std::borrow::Cow;
use std::{fmt::Write, marker::PhantomData};

#[cfg(feature = "full")]
pub trait CheckerRule: std::fmt::Debug + Send + Sync + std::any::Any {
    fn priority(&self) -> isize {
        0
    }
    fn display(&self) -> Vec<Displayable>;
    fn as_box_dyn(&self) -> Box<dyn CheckerRule>;
    fn as_dyn(&self) -> &dyn CheckerRule;
    fn as_any(&self) -> &dyn std::any::Any;
    fn eq(&self, o: &dyn CheckerRule) -> bool;
}

#[cfg(feature = "full")]
pub trait SizedSolverRule:
    std::fmt::Debug + Send + Sync + std::any::Any + Clone + Sized + PartialEq + Eq
{
    fn priority(&self) -> isize {
        0
    }
    fn display(&self) -> Vec<Displayable>;
}

#[cfg(feature = "full")]
impl<T: SizedSolverRule> CheckerRule for T {
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn priority(&self) -> isize {
        <Self as SizedSolverRule>::priority(self)
    }

    #[inline]
    fn display(
        &self,
        //f: &mut std::fmt::Formatter,
    ) -> Vec<Displayable> {
        <T as SizedSolverRule>::display(self)
    }

    fn as_box_dyn(&self) -> Box<dyn CheckerRule> {
        Box::new(self.clone()) as _
    }
    fn as_dyn(&self) -> &dyn CheckerRule {
        self as _
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self as _
    }
    fn eq(&self, o: &dyn CheckerRule) -> bool {
        o.as_any().downcast_ref::<T>().is_some_and(|v| v == self)
    }
}

#[cfg(feature = "full")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MessageLevel {
    Failure,
    Comment,
    Header,
    Emph,
}

#[cfg(feature = "full")]
#[derive(Clone, Copy, Default)]
pub struct Indent(pub usize);
#[cfg(feature = "full")]
impl Indent {
    pub const fn increase(&mut self) {
        self.0 += 1;
    }
    pub const fn decrease(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }
}
#[cfg(feature = "full")]
impl std::fmt::Display for Indent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            return Ok(());
        }
        for _ in 0..self.0 - 1 {
            f.write_str("  │")?;
        }
        f.write_str("  ├─")?;
        Ok(())
    }
}

#[cfg(feature = "full")]
#[derive(Debug)]
pub enum CheckLogCow<'t> {
    Owned(PreCheckLog),
    Borrowed(RefCheckLog<'t>),
}
#[cfg(feature = "full")]
impl<'t> From<RefCheckLog<'t>> for CheckLogCow<'t> {
    #[inline]
    fn from(value: RefCheckLog<'t>) -> Self {
        Self::Borrowed(value)
    }
}
#[cfg(feature = "full")]
impl From<PreCheckLog> for CheckLogCow<'_> {
    #[inline]
    fn from(value: PreCheckLog) -> Self {
        Self::Owned(value)
    }
}

macro_rules! tasks {
    (
        $(
            $name:ident($($field:ident : $tp:ident),*) => $res:tt
        ),* $(,)?
    ) => {

        #[cfg(feature = "full")]
        #[derive(Debug)]
        pub enum RefCheckLog<'t> {
            $(
                $name {
                    $($field: tasks!(@TPBORROW $tp),)*
                    steps:Box<[CheckLogCow<'t>]>,
                    context: Box<[Cow<'t, ComponentVar>]>,
                    result: Option<$res>,
                },
            )*
            Rule{
                rule: &'t dyn CheckerRule,
                steps:Box<[CheckLogCow<'t>]>,
            },
            Strategy{
                name: &'static str,
                steps:Box<[CheckLogCow<'t>]>,
                success:bool
            },
            Msg(Vec<DisplayableRef<'t>>, MessageLevel),
        }
        #[cfg(feature = "full")]
        impl RefCheckLog<'_> {
            pub fn into_owned(self,term:&impl Fn(Term) -> Term) -> PreCheckLog {
                match self {
                    $(
                        Self::$name{$($field,)* steps,context,result} => PreCheckLog::$name{
                            $($field:tasks!(@CONV $tp $field term),)*
                            steps: steps.into_iter().map(|t| CheckLogCow::into_owned(t,term)).collect(),
                            context: context.into_iter().map(Cow::into_owned).collect(),
                            result,

                        },
                    )*
                    Self::Msg(txt,lvl) => PreCheckLog::Msg(txt.into_iter().map(|s| s.into_owned(term)).collect(),lvl),
                    Self::Rule{rule,steps} => PreCheckLog::Rule{
                        rule:rule.as_box_dyn(),
                        steps: steps.into_iter().map(|t| CheckLogCow::into_owned(t,term)).collect(),
                    },
                    Self::Strategy{name,steps,success} => PreCheckLog::Strategy{
                        name,
                        steps: steps.into_iter().map(|t| CheckLogCow::into_owned(t,term)).collect(),
                        success
                    }
                }
            }
        }
        #[cfg(feature = "full")]
        #[derive(Debug)]
        pub enum PreCheckLog {
            $(
                $name {
                    $($field: tasks!(@TPOWN $tp),)*
                    steps:Vec<Self>,
                    context:Box<[ComponentVar]>,
                    result:Option<$res>
                },
            )*
            Rule{
                rule:Box<dyn CheckerRule>,
                steps:Vec<Self>,
            },
            Strategy{
                name: &'static str,
                steps:Vec<Self>,
                success:bool
            },
            //Dyn(Box<dyn CheckTraceDisplayable>)
            Msg(Vec<Displayable>, MessageLevel),
            //Count(&'static str,usize)
            //Interpolated(Box<[DisplayableElem]>, MessageLevel),
        }

        #[cfg(feature = "full")]
        impl CheckLog {
            pub fn from_pre(v:PreCheckLog,terms:&mut impl FnMut(Term) -> Term) -> Self {
                use PreCheckLog as P;
                match v {
                    $(
                        P::$name{
                            $($field,)*steps,context,result
                        } => Self::$name{
                            $( $field:tasks!(@FROMPRE $tp $field terms),)*
                            context,result:result.map(|r| tasks!(@FROMPRE $res r terms)),
                            steps:steps.into_iter().map(|e| Self::from_pre(e,terms)).collect()
                        },
                    )*
                    P::Rule{ rule, steps } => Self::Rule {
                        header:Displayable::map(rule.display(),terms),
                        steps:steps.into_iter().map(|e| Self::from_pre(e,terms)).collect()
                    },
                    P::Strategy{ name, steps, success } => Self::Strategy {
                        name:name.to_string(),
                        steps:steps.into_iter().map(|e| Self::from_pre(e,terms)).collect(),
                        success
                    },
                    P::Msg(s, MessageLevel::Comment) => Self::Comment(s),
                    P::Msg(s, MessageLevel::Emph) => Self::Emph(s),
                    P::Msg(s, MessageLevel::Header) => Self::Header(s),
                    P::Msg(s, MessageLevel::Failure) => Self::Fail(s),
                    //P::Count(s, u) =>
                    //    Self::Comment(format!("{s} {u}"))
                }
            }
        }
        #[cfg(feature = "full")]
        impl<'t> CheckingTask<'t> {
            pub fn close<R:Clone>(self,res:Option<&R>,steps:Box<[CheckLogCow<'t>]>,context:&[Cow<'t,ComponentVar>]) -> RefCheckLog<'t> {
                let context = context.iter().map(Cow::clone).collect();
                match self {
                    $(
                        Self::$name( $($field),* ) => RefCheckLog::$name {
                            $($field,)*
                            steps,
                            context,
                            result: res.map(|r| unsafe{&*std::ptr::from_ref(r).cast::<$res>()}.clone() )
                        },
                    )*
                    Self::Strategy(name) => RefCheckLog::Strategy {
                        name,
                        steps,success:res.is_some_and(|v| {
                            if std::mem::size_of::<R>() == 1 {
                                // SAFETY: => boolean
                                unsafe{
                                    std::mem::transmute_copy::<R,bool>(v)
                                }
                            } else {
                                true
                            }
                        })
                    },
                    Self::Rule(rule) => RefCheckLog::Rule { rule, steps }
                }
            }
        }
        #[cfg(feature = "full")]
        impl<'t> CheckLogCow<'t> {
            pub fn into_owned(self,term:&impl Fn(Term) -> Term) -> PreCheckLog {
                match self {
                    Self::Owned(o) => o,
                    Self::Borrowed(b) => b.into_owned(term)
                }
            }
        }

        #[cfg(feature = "full")]
        #[derive(Copy,Clone,Debug)]
        pub enum CheckingTask<'t> {
            $(
                $name($(tasks!(@TPBORROW $tp)),*)
            ),*,
            Rule(&'t dyn CheckerRule),
            Strategy(&'static str),
        }

        #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
        pub enum CheckLog {
            $(
                $name {
                    $($field: tasks!(@TPOWN $tp),)*
                    steps:Vec<Self>,
                    context:Box<[ComponentVar]>,
                    result:Option<$res>
                },
            )*
            Comment(Vec<Displayable>),
            Emph(Vec<Displayable>),
            Header(Vec<Displayable>),
            Fail(Vec<Displayable>),
            Strategy {
                name: String,
                steps: Vec<Self>,
                success: bool,
            },
            Rule {
                header: Vec<Displayable>,
                steps: Vec<Self>,
            },
        }
        #[cfg(feature = "full")]
        impl CheckLog {
            pub(crate) fn display_i(&self,displayer:&mut impl TraceDisplay) -> std::fmt::Result {
                let mut curr = std::slice::from_ref(self).iter();
                let mut stack = Vec::new();
                let mut indent = Indent::default();
                loop {
                    while let Some(next) = curr.next() {
                        if displayer.line(next,indent)? == std::ops::ControlFlow::Continue(()) {
                            match next {
                                $(
                                    Self::$name{ $($field,)* steps, context,result } => {
                                        displayer.task(CheckingTask::$name($($field),*),context,result.is_some())?;
                                        tasks!(@DISPL result displayer $res);
                                        indent.increase();
                                        stack.push(std::mem::replace(&mut curr,steps.iter()));
                                    }
                                )*
                                Self::Rule{header,steps} => {
                                    for e in header {
                                        displayer.displayable(e,None)?;
                                    }
                                    indent.increase();
                                    stack.push(std::mem::replace(&mut curr,steps.iter()));
                                }
                                Self::Strategy{name,steps,success} => {
                                    displayer.strategy(name,&[],*success)?;
                                    indent.increase();
                                    stack.push(std::mem::replace(&mut curr,steps.iter()));
                                }
                                Self::Comment(s) => {
                                    for s in s {
                                        displayer.displayable(s,Some(MessageLevel::Comment))?;
                                    }
                                }
                                Self::Emph(s) => {
                                    for s in s {
                                        displayer.displayable(s,Some(MessageLevel::Emph))?;
                                    }
                                }
                                Self::Header(s) => {
                                    for s in s {
                                        displayer.displayable(s,Some(MessageLevel::Header))?;
                                    }
                                }
                                Self::Fail(s) => {
                                    for s in s {
                                        displayer.displayable(s,Some(MessageLevel::Failure))?;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(next) = stack.pop() {
                        indent.decrease();
                        curr = next;
                    } else {
                        break
                    }
                }
                Ok(())
            }
        }

    };
    (@DISPL $res:ident $disp:ident Term) => {
        if let Some(t) = $res {
            $disp.string(": ",None)?;
            $disp.term(t,None)?;
        }
    };
    (@DISPL $res:ident $disp:ident bool) => {};
    (@TPBORROW Term) => {&'t Term};
    (@TPOWN Term) => {Term};
    (@TPBORROW str) => {&'t str};
    (@TPOWN str) => {Box<str>};
    (@FROMPRE Term $name:ident $f:ident) => {$f($name)};
    (@FROMPRE str $name:ident $f:ident) => {$name};
    (@FROMPRE bool $name:ident $f:ident) => {$name};
    (@CONV Term $name:ident $f:ident) => {$name.clone()};//{ $f($name.clone()) };
    (@CONV str $name:ident $f:ident) => { $name.to_string().into_boxed_str() };
    //(@TPBORROW SolverRule) => {&'t dyn SolverRule};
    //(@TPOWN SolverRule) => {Box<dyn SolverRule>};
    (@CONV SolverRule $name:ident $f:ident) => { $name.as_box_dyn() };
}

#[cfg(feature = "full")]
impl PreCheckLog {
    pub fn push(&mut self, msg: Self) {
        if let Some(steps) = self.steps_mut() {
            steps.push(msg);
        }
    }
    pub const fn steps_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            Self::Equality { steps, .. }
            | Self::HasType { steps, .. }
            | Self::Inference { steps, .. }
            | Self::Inhabitable { steps, .. }
            | Self::Rule { steps, .. }
            | Self::Simplify { steps, .. }
            | Self::Strategy { steps, .. }
            | Self::Subtype { steps, .. }
            | Self::Universe { steps, .. }
            | Self::VariableInference { steps, .. }
            | Self::Proving { steps, .. } => Some(steps),
            Self::Msg(_, _) /*| Self::Count(_, _)*/ => None,
        }
    }
}

impl CheckLog {
    pub fn filter_failures(&mut self, preserve_siblings: bool) {
        let Some(steps) = self.steps_mut() else {
            return;
        };
        if preserve_siblings {
            for s in steps {
                if s.success() {
                    s.filter_failures(false);
                } else {
                    s.filter_failures(true);
                }
            }
        } else {
            *steps = std::mem::take(steps)
                .into_iter()
                .filter(|s| !s.success())
                .map(|mut s| {
                    s.filter_failures(preserve_siblings);
                    s
                })
                .collect();
        }
    }
    pub const fn steps_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            Self::Equality { steps, .. }
            | Self::HasType { steps, .. }
            | Self::Inference { steps, .. }
            | Self::Inhabitable { steps, .. }
            | Self::Rule { steps, .. }
            | Self::Simplify { steps, .. }
            | Self::Strategy { steps, .. }
            | Self::Subtype { steps, .. }
            | Self::Universe { steps, .. }
            | Self::VariableInference { steps, .. }
            | Self::Proving { steps, .. } => Some(steps),
            Self::Comment(_) | Self::Emph(_) | Self::Header(_) | Self::Fail(_) => None,
        }
    }
    pub fn success(&self) -> bool {
        match self {
            Self::Equality { result, .. }
            | Self::HasType { result, .. }
            | Self::Inhabitable { result, .. }
            | Self::Subtype { result, .. }
            | Self::Universe { result, .. } => *result == Some(true),
            Self::Rule { steps, .. } => steps.iter().all(Self::success),
            Self::Inference { result, .. }
            | Self::VariableInference { result, .. }
            | Self::Simplify { result, .. }
            | Self::Proving { result, .. } => result.is_some(),
            Self::Strategy { success, .. } => *success,
            Self::Comment(_) | Self::Header(_) | Self::Emph(_) | Self::Fail(_) => false,
        }
    }
    pub fn add_failure(&mut self, s: &'static str) {
        if let Some(steps) = self.steps_mut() {
            steps.push(Self::Fail(vec![s.to_string().into()]));
        }
    }
}

#[cfg(feature = "full")]
#[derive(Debug, Clone)]
pub enum DisplayableRef<'r> {
    Num(i128),
    //Space,
    String(Cow<'static, str>),
    Term(Cow<'r, Term>),
    Uri(either::Either<UriRef<'r>, Uri>),
    Var(Cow<'r, Variable>),
}
#[cfg(feature = "full")]
impl DisplayableRef<'_> {
    pub fn into_owned(self, term: &impl Fn(Term) -> Term) -> Displayable {
        match self {
            Self::Num(i) => Displayable::Num(i),
            Self::String(s) => Displayable::String(s.to_string()),
            Self::Term(t) => Displayable::Term(term(t.into_owned())),
            Self::Uri(u) => Displayable::Uri(match u {
                either::Left(u) => u.owned(),
                either::Right(u) => u,
            }),
            Self::Var(v) => Displayable::Var(v.into_owned()),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum Displayable {
    //Log(CheckLog),
    Num(i128),
    //Space,
    String(String),
    Term(Term),
    Uri(Uri),
    Var(Variable),
}
impl Displayable {
    fn map(v: Vec<Self>, terms: &mut impl FnMut(Term) -> Term) -> Vec<Self> {
        v.into_iter()
            .map(|e| {
                if let Self::Term(t) = e {
                    Self::Term(terms(t))
                } else {
                    e
                }
            })
            .collect()
    }
}

tasks! {
    Simplify(term:Term) => Term,
    Proving(term:Term) => Term,
    Inference(term: Term) => Term,
    VariableInference(var: str) => Term,
    //Simplify(term:Term) => Term,
    Inhabitable(term: Term) => bool,
    Universe(term:Term) => bool,
    Subtype(sub:Term,sup:Term) => bool,
    HasType(tm:Term,tp:Term) => bool,
    Equality(lhs:Term,rhs:Term) => bool,
}

#[cfg(feature = "full")]
impl<'b> PartialEq<CheckingTask<'b>> for CheckingTask<'_> {
    fn eq(&self, other: &CheckingTask<'b>) -> bool {
        match (self, other) {
            (Self::Simplify(t), CheckingTask::Simplify(t2))
            | (Self::Proving(t), CheckingTask::Proving(t2))
            | (Self::Inference(t), CheckingTask::Inference(t2))
            | (Self::Inhabitable(t), CheckingTask::Inhabitable(t2))
            | (Self::Universe(t), CheckingTask::Universe(t2)) => t.alpha_equal(t2),
            (Self::VariableInference(v), CheckingTask::VariableInference(v2)) => v == v2,
            (Self::Subtype(a, b), CheckingTask::Subtype(a2, b2))
            | (Self::HasType(a, b), CheckingTask::HasType(a2, b2))
            | (Self::Equality(a, b), CheckingTask::Equality(a2, b2)) => {
                a.alpha_equal(a2) && b.alpha_equal(b2)
            }
            _ => false,
        }
    }
}

#[cfg(feature = "full")]
impl CheckLog {
    #[must_use]
    pub fn display<D: FmtTraceDisplay>(&self) -> impl std::fmt::Display + use<'_, D> {
        TraceDisplayer::<'_, D> {
            trace: self,
            d: PhantomData,
        }
    }
    #[cfg(feature = "colors")]
    #[must_use]
    pub fn colored(&self) -> impl std::fmt::Display {
        TraceDisplayer {
            d: PhantomData::<ColorDisplay>,
            trace: self,
        }
    }
}
#[cfg(feature = "full")]
impl std::fmt::Display for CheckLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.display::<()>().fmt(f)
    }
}

#[cfg(feature = "full")]
pub trait FmtTraceDisplay {
    fn new(f: &mut std::fmt::Formatter<'_>) -> impl TraceDisplay;
}

#[cfg(feature = "full")]
pub trait TraceDisplay {
    // /// ### Errors
    //fn rule(&mut self, rule: &dyn CheckerRule) -> std::fmt::Result;

    /// ### Errors
    fn displayable(&mut self, d: &Displayable, lvl: Option<MessageLevel>) -> std::fmt::Result {
        match d {
            Displayable::Num(i) => self.num(*i, lvl),
            //Displayable::Space => self.space(),
            Displayable::String(s) => self.string(s, lvl),
            Displayable::Term(t) => self.term(t, lvl),
            Displayable::Uri(u) => self.uri(u.as_uri(), lvl),
            Displayable::Var(v) => self.variable(v, lvl),
        }
    }

    /// ### Errors
    fn line(
        &mut self,
        _: &CheckLog,
        indent: Indent,
    ) -> Result<std::ops::ControlFlow<()>, std::fmt::Error>; /* {
    f.write_char('\n')?;
    self.indent(indent, None, f)?;
    Ok(std::ops::ControlFlow::Continue(()))
    }*/

    /// ### Errors
    fn task(
        &mut self,
        task: CheckingTask<'_>,
        context: &[ComponentVar],
        success: bool,
    ) -> std::fmt::Result;

    /// ### Errors
    fn strategy(&mut self, name: &str, context: &[ComponentVar], success: bool)
    -> std::fmt::Result;

    /// ### Errors
    fn uri(&mut self, uri: ftml_uris::UriRef, lvl: Option<MessageLevel>) -> std::fmt::Result;

    /// ### Errors
    fn term(&mut self, term: &Term, lvl: Option<MessageLevel>) -> std::fmt::Result;

    /// ### Errors
    fn string(&mut self, s: &str, lvl: Option<MessageLevel>) -> std::fmt::Result;

    /// ### Errors
    fn variable(&mut self, var: &Variable, lvl: Option<MessageLevel>) -> std::fmt::Result;

    /// ### Errors
    fn num(&mut self, num: i128, lvl: Option<MessageLevel>) -> std::fmt::Result;

    /// ### Errors
    fn indent(&mut self, indent: Indent, lvl: Option<MessageLevel>) -> std::fmt::Result;

    /// ### Errors
    fn space(&mut self) -> std::fmt::Result;
}
#[cfg(feature = "full")]
impl FmtTraceDisplay for () {
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn new(f: &mut std::fmt::Formatter<'_>) -> impl TraceDisplay {
        f
    }
}
#[cfg(feature = "full")]
impl TraceDisplay for &mut std::fmt::Formatter<'_> {
    fn line(
        &mut self,
        _: &CheckLog,
        indent: Indent,
        //f: &mut std::fmt::Formatter<'_>,
    ) -> Result<std::ops::ControlFlow<()>, std::fmt::Error> {
        self.write_char('\n')?;
        self.indent(indent, None)?;
        Ok(std::ops::ControlFlow::Continue(()))
    }
    fn space(&mut self) -> std::fmt::Result {
        self.write_char(' ')
    }
    /*
    fn rule(&mut self, rule: &dyn CheckerRule) -> std::fmt::Result {
        self.write_str("Using rule: ")?;
        rule.display(self, None)
    }
     */

    fn strategy(&mut self, name: &str, _: &[ComponentVar], _: bool) -> std::fmt::Result {
        write!(self, "Strategy: {name}")
    }
    fn task(
        &mut self,
        task: CheckingTask<'_>,
        context: &[ComponentVar],
        success: bool,
        //f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        fn do_context(
            context: &[ComponentVar],
            mut f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            if context.is_empty() {
                return Ok(());
            }
            f.write_str("{... ")?;
            let mut first = true;
            for ComponentVar { var, tp, df } in context {
                if first {
                    first = false;
                } else {
                    f.write_str(", ")?;
                }
                f.variable(var, None)?;
                if let Some(tp) = tp {
                    f.write_str(" : ")?;
                    f.term(tp, None)?;
                }
                if let Some(df) = df {
                    f.write_str(" : ")?;
                    f.term(df, None)?;
                }
            }
            f.write_str(" } ")
        }
        if success {
            self.write_str("[SUCCESS] ")?;
        } else {
            self.write_str("[FAILED] ")?;
        }
        match task {
            CheckingTask::Simplify(t) => {
                self.write_str("Simplifying ")?;
                do_context(context, self)?;
                self.term(t, None)
            }
            CheckingTask::Proving(t) => {
                self.write_str("Proving ")?;
                do_context(context, self)?;
                self.term(t, None)
            }
            CheckingTask::Inference(t) => {
                self.write_str("Inferring type of ")?;
                do_context(context, self)?;
                self.term(t, None)
            }
            CheckingTask::VariableInference(t) => {
                self.write_str("Inferring type of variable ")?;
                do_context(context, self)?;
                self.write_str(t)
            }
            CheckingTask::Inhabitable(tm) => {
                self.write_str("Checking inhabitability ")?;
                do_context(context, self)?;
                self.write_str("⊢ INH ")?;
                self.term(tm, None)
            }
            CheckingTask::Universe(tm) => {
                self.write_str("Checking universe ")?;
                do_context(context, self)?;
                self.write_str("⊢ UNIV ")?;
                self.term(tm, None)
            }
            CheckingTask::Subtype(sub, sup) => {
                self.write_str("Checking subtyping ")?;
                do_context(context, self)?;
                self.write_str("⊢ ")?;
                self.term(sub, None)?;
                self.write_str(" <: ")?;
                self.term(sup, None)
            }
            CheckingTask::HasType(tm, tp) => {
                self.write_str("Checking typing ")?;
                do_context(context, self)?;
                self.write_str("⊢ ")?;
                self.term(tm, None)?;
                self.write_str(" : ")?;
                self.term(tp, None)
            }
            CheckingTask::Equality(lhs, rhs) => {
                self.write_str("Checking equality ")?;
                do_context(context, self)?;
                self.write_str("⊢ ")?;
                self.term(lhs, None)?;
                self.write_str(" == ")?;
                self.term(rhs, None)
            }
            CheckingTask::Rule(rl) => {
                for e in rl.display() {
                    self.displayable(&e, None)?;
                }
                Ok(())
            }
            CheckingTask::Strategy(s) => self.strategy(s, context, success),
        }
    }
    fn uri(&mut self, uri: ftml_uris::UriRef, _: Option<MessageLevel>) -> std::fmt::Result {
        match uri {
            ftml_uris::UriRef::Symbol(s) => {
                std::fmt::Display::fmt(&s.module.name, self)?;
                self.write_char('?')?;
                std::fmt::Display::fmt(&s.name, self)
            }
            _ => std::fmt::Display::fmt(&uri, self),
        }
    }
    fn indent(&mut self, indent: Indent, _: Option<MessageLevel>) -> std::fmt::Result {
        write!(self, "{indent} ")
    }
    fn term(&mut self, term: &Term, _: Option<MessageLevel>) -> std::fmt::Result {
        <_ as std::fmt::Debug>::fmt(&term.debug_short(), self)
    }
    fn string(&mut self, s: &str, lvl: Option<MessageLevel>) -> std::fmt::Result {
        if lvl == Some(MessageLevel::Failure) {
            self.write_str("[FAILED] ")?;
        }
        self.write_str(s)
    }
    fn variable(&mut self, var: &Variable, _: Option<MessageLevel>) -> std::fmt::Result {
        self.write_str(var.name())
    }
    fn num(&mut self, num: i128, _: Option<MessageLevel>) -> std::fmt::Result {
        <i128 as std::fmt::Display>::fmt(&num, self)
    }
}

#[cfg(feature = "colors")]
pub struct ColorDisplay<'a, 'b>(&'a mut std::fmt::Formatter<'b>);
#[cfg(feature = "colors")]
impl FmtTraceDisplay for ColorDisplay<'_, '_> {
    fn new(f: &mut std::fmt::Formatter<'_>) -> impl TraceDisplay {
        ColorDisplay(f)
    }
}
#[cfg(feature = "colors")]
impl TraceDisplay for ColorDisplay<'_, '_> {
    fn line(
        &mut self,
        _: &CheckLog,
        indent: Indent,
    ) -> Result<std::ops::ControlFlow<()>, std::fmt::Error> {
        self.0.write_char('\n')?;
        self.indent(indent, None)?;
        Ok(std::ops::ControlFlow::Continue(()))
    }

    fn space(&mut self) -> std::fmt::Result {
        self.0.write_char(' ')
    }

    /*
    fn rule(&mut self, rule: &dyn CheckerRule) -> std::fmt::Result {
        write!(self.0, "{} ", "Using rule: ".italic())?;
        rule.display(self, None)
    }
     */

    fn strategy(&mut self, name: &str, _: &[ComponentVar], _: bool) -> std::fmt::Result {
        write!(self.0, "Strategy: {}", name.italic())
    }

    fn task(
        &mut self,
        task: CheckingTask<'_>,
        context: &[ComponentVar],
        success: bool,
    ) -> std::fmt::Result {
        fn do_context(context: &[ComponentVar], f: &mut ColorDisplay<'_, '_>) -> std::fmt::Result {
            if context.is_empty() {
                return Ok(());
            }
            f.0.write_str("{... ")?;
            let mut first = true;
            for ComponentVar { var, tp, df } in context {
                if first {
                    first = false;
                } else {
                    f.0.write_str(", ")?;
                }
                f.variable(var, None)?;
                if let Some(tp) = tp {
                    f.0.write_str(" : ")?;
                    f.term(tp, None)?;
                }
                if let Some(df) = df {
                    f.0.write_str(" : ")?;
                    f.term(df, None)?;
                }
            }
            f.0.write_str(" } ")
        }
        if success {
            write!(self.0, "{} ", "[SUCCESS]".green())?;
        } else {
            write!(self.0, "{} ", "[FAILED]".red())?;
        }
        match task {
            CheckingTask::Simplify(t) => {
                write!(self.0, "{} ", "Simplifying".bright_white().bold())?;
                do_context(context, self)?;
                self.term(t, None)
            }
            CheckingTask::Proving(t) => {
                write!(self.0, "{} ", "Proving".bright_white().bold())?;
                do_context(context, self)?;
                self.term(t, None)
            }
            CheckingTask::Inference(t) => {
                write!(self.0, "{} ", "Inferring type of".bright_white().bold())?;
                do_context(context, self)?;
                self.term(t, None)
            }
            CheckingTask::VariableInference(t) => {
                write!(
                    self.0,
                    "{} ",
                    "Inferring type of variable".bright_white().bold()
                )?;
                do_context(context, self)?;
                self.0.write_str(t)
            }
            CheckingTask::Inhabitable(tm) => {
                write!(
                    self.0,
                    "{} ",
                    "Checking inhabitability".bright_white().bold()
                )?;
                do_context(context, self)?;
                write!(self.0, "{} ", "⊢ INH".bright_white().bold())?;
                self.term(tm, None)
            }
            CheckingTask::Universe(tm) => {
                write!(self.0, "{} ", "Checking universe".bright_white().bold())?;
                do_context(context, self)?;
                write!(self.0, "{} ", "⊢ UNIV".bright_white().bold())?;
                self.term(tm, None)
            }
            CheckingTask::Subtype(sub, sup) => {
                write!(self.0, "{} ", "Checking subtyping".bright_white().bold())?;
                do_context(context, self)?;
                write!(self.0, "{} ", "⊢".bright_white().bold())?;
                self.term(sub, None)?;
                write!(self.0, " {} ", "<:".bright_white().bold())?;
                self.term(sup, None)
            }
            CheckingTask::HasType(tm, tp) => {
                write!(self.0, "{} ", "Checking typing".bright_white().bold())?;
                do_context(context, self)?;
                write!(self.0, "{} ", "⊢".bright_white().bold())?;
                self.term(tm, None)?;
                write!(self.0, " {} ", ":".bright_white().bold())?;
                self.term(tp, None)
            }
            CheckingTask::Equality(lhs, rhs) => {
                write!(self.0, "{} ", "Checking equality".bright_white().bold())?;
                do_context(context, self)?;
                write!(self.0, "{} ", "⊢".bright_white().bold())?;
                self.term(lhs, None)?;
                write!(self.0, " {} ", "==".bright_white().bold())?;
                self.term(rhs, None)
            }
            CheckingTask::Rule(rl) => {
                for e in rl.display() {
                    self.displayable(&e, None)?;
                }
                Ok(())
            }
            CheckingTask::Strategy(s) => self.strategy(s, context, success),
        }
    }
    fn uri(&mut self, uri: ftml_uris::UriRef, _: Option<MessageLevel>) -> std::fmt::Result {
        match uri {
            ftml_uris::UriRef::Symbol(s) => {
                std::fmt::Display::fmt(&s.module.name, self.0)?;
                self.0.write_char('?')?;
                std::fmt::Display::fmt(&s.name, self.0)
            }
            _ => std::fmt::Display::fmt(&uri, self.0),
        }
    }
    fn indent(&mut self, indent: Indent, _: Option<MessageLevel>) -> std::fmt::Result {
        write!(self.0, "{} ", indent.blue())
    }
    fn term(&mut self, term: &Term, _: Option<MessageLevel>) -> std::fmt::Result {
        write!(self.0, "{:?}", term.debug_short().yellow())
    }
    fn string(&mut self, s: &str, lvl: Option<MessageLevel>) -> std::fmt::Result {
        if lvl == Some(MessageLevel::Failure) {
            write!(self.0, "{} ", "[FAILED]".red())?;
            write!(self.0, "{}", s.red())
        } else {
            write!(self.0, "{}", s.bright_black())
        }
    }
    fn variable(&mut self, var: &Variable, _: Option<MessageLevel>) -> std::fmt::Result {
        self.0.write_str(var.name())
    }
    fn num(&mut self, num: i128, _: Option<MessageLevel>) -> std::fmt::Result {
        <i128 as std::fmt::Display>::fmt(&num, self.0)
    }
}

#[cfg(feature = "full")]
struct TraceDisplayer<'d, D: FmtTraceDisplay> {
    trace: &'d CheckLog,
    d: PhantomData<D>,
}
#[cfg(feature = "full")]
impl<D: FmtTraceDisplay> std::fmt::Display for TraceDisplayer<'_, D> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.trace.display_i(&mut D::new(f))
    }
}

#[cfg(feature = "full")]
impl From<i128> for DisplayableRef<'_> {
    fn from(value: i128) -> Self {
        Self::Num(value)
    }
}
#[cfg(feature = "full")]
impl From<usize> for DisplayableRef<'_> {
    fn from(value: usize) -> Self {
        Self::Num(value as _)
    }
}
#[cfg(feature = "full")]
impl From<&'static str> for DisplayableRef<'_> {
    fn from(value: &'static str) -> Self {
        Self::String(Cow::Borrowed(value))
    }
}
#[cfg(feature = "full")]
impl From<String> for DisplayableRef<'_> {
    fn from(value: String) -> Self {
        Self::String(Cow::Owned(value))
    }
}
#[cfg(feature = "full")]
impl<'r> From<&'r Term> for DisplayableRef<'r> {
    fn from(value: &'r Term) -> Self {
        Self::Term(Cow::Borrowed(value))
    }
}
#[cfg(feature = "full")]
impl From<Term> for DisplayableRef<'_> {
    fn from(value: Term) -> Self {
        Self::Term(Cow::Owned(value))
    }
}
#[cfg(feature = "full")]
impl<'r> From<UriRef<'r>> for DisplayableRef<'r> {
    fn from(value: UriRef<'r>) -> Self {
        Self::Uri(either::Left(value))
    }
}
#[cfg(feature = "full")]
impl From<Uri> for DisplayableRef<'_> {
    fn from(value: Uri) -> Self {
        Self::Uri(either::Right(value))
    }
}
#[cfg(feature = "full")]
impl<'r> From<&'r Variable> for DisplayableRef<'r> {
    fn from(value: &'r Variable) -> Self {
        Self::Var(Cow::Borrowed(value))
    }
}
#[cfg(feature = "full")]
impl From<Variable> for DisplayableRef<'_> {
    fn from(value: Variable) -> Self {
        Self::Var(Cow::Owned(value))
    }
}

impl<T: FtmlUri> From<&T> for Displayable {
    fn from(value: &T) -> Self {
        Self::Uri(value.as_uri().owned())
    }
}
impl From<&str> for Displayable {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}
impl From<String> for Displayable {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}
impl From<Term> for Displayable {
    fn from(value: Term) -> Self {
        Self::Term(value)
    }
}

impl From<i128> for Displayable {
    fn from(value: i128) -> Self {
        Self::Num(value)
    }
}
impl From<usize> for Displayable {
    fn from(value: usize) -> Self {
        Self::Num(value as _)
    }
}

#[cfg(feature = "full")]
#[macro_export]
macro_rules! trace {
    ($($e:expr),* $(,)? ) => {
        {vec![$(
            $e.into()
        ),*]
        }
    }
}

#[cfg(feature = "full")]
#[macro_export]
macro_rules! traceref {
    (FAIL $($e:expr),* $(,)? ) => {
        $crate::RefCheckLog::Msg(
            vec![$( $e.into() ),*],
            $crate::MessageLevel::Failure
        )
    };
    ($($e:expr),* $(,)? ) => {
        $crate::RefCheckLog::Msg(
            vec![$( $e.into() ),*],
            $crate::MessageLevel::Comment
        )
    }
}
