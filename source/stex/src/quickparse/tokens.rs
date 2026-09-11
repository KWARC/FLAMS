use flams_utils::sourcerefs::{StringPosition, StringRange};

#[derive(Debug)]
pub enum TeXToken<P: StringPosition, S> {
    Comment(StringRange<P>),
    BeginGroupChar(P),
    EndGroupChar(P),
    BeginMath { display: bool, start: P },
    EndMath { start: P },
    ControlSequence { start: P, name: S },
    Text { range: StringRange<P>, text: S },
    Directive(S),
}
