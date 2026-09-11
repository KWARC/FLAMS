use crate::quickparse::tokens::TeXToken;
use flams_utils::{
    parsing::{SourceParser, StrParser},
    sourcerefs::{StringPosition, StringRange},
};

use super::stex::DiagnosticLevel;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Mode {
    Text,
    Math { display: bool },
}

pub struct TeXTokenizer<'a, Pos: StringPosition> {
    pub reader: StrParser<'a, Pos>,
    pub letters: String,
    pub mode: Mode,
    pub(crate) err: &'a mut dyn FnMut(String, StringRange<Pos>, DiagnosticLevel),
}

impl<'a, Pos: StringPosition> Iterator for TeXTokenizer<'a, Pos> {
    type Item = TeXToken<Pos, &'a str>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.read_next()
    }
}

impl<'a, Pos: StringPosition> TeXTokenizer<'a, Pos> {
    pub(crate) fn new(
        input: &'a str,
        err: &'a mut dyn FnMut(String, StringRange<Pos>, DiagnosticLevel),
    ) -> Self {
        TeXTokenizer {
            reader: StrParser::new(input),
            mode: Mode::Text,
            err,
            letters: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(),
        }
    }
    fn read_next(&mut self) -> Option<TeXToken<Pos, &'a str>> {
        self.reader.trim_start();
        let start = self.reader.curr_pos();
        match self.reader.peek_head() {
            None => None,
            Some('%') => {
                self.reader.next_char();
                Some(self.read_comment(start))
            }
            Some('{') => {
                self.reader.next_char();
                Some(TeXToken::BeginGroupChar(start))
            }
            Some('}') => {
                self.reader.next_char();
                Some(TeXToken::EndGroupChar(start))
            }
            Some('$') => {
                self.reader.next_char();
                match self.mode {
                    Mode::Math { display: true } => {
                        if self.reader.starts_with('$') {
                            self.reader.next_char();
                        } else {
                            self.problem(
                                start,
                                "Missing $ closing display math",
                                DiagnosticLevel::Error,
                            );
                        }
                        self.close_math();
                        Some(TeXToken::EndMath { start })
                    }
                    Mode::Math { .. } => {
                        self.close_math();
                        Some(TeXToken::EndMath { start })
                    }
                    Mode::Text => {
                        if self.reader.starts_with('$') {
                            self.reader.next_char();
                            self.open_math(true);
                            Some(TeXToken::BeginMath {
                                display: true,
                                start,
                            })
                        } else {
                            self.open_math(false);
                            Some(TeXToken::BeginMath {
                                display: false,
                                start,
                            })
                        }
                    }
                }
            }
            Some('\\') => {
                self.reader.next_char();
                let name = match self.reader.peek_head() {
                    Some(c) if self.letters.contains(c) => {
                        self.reader.read_while(|c| self.letters.contains(c))
                    }
                    None => "".into(),
                    _ => self.reader.read_n(1),
                };
                Some(TeXToken::ControlSequence { start, name })
            }
            _ => {
                let text = self.reader.read_while(|c| !"%{}$\\".contains(c));
                Some(TeXToken::Text {
                    range: StringRange {
                        start,
                        end: self.reader.curr_pos(),
                    },
                    text,
                })
            }
        }
    }

    #[inline]
    pub const fn open_math(&mut self, display: bool) {
        self.mode = Mode::Math { display };
    }
    #[inline]
    pub const fn close_math(&mut self) {
        self.mode = Mode::Text;
    }

    #[inline]
    pub fn problem(&mut self, start: Pos, msg: impl std::fmt::Display, level: DiagnosticLevel) {
        (self.err)(
            msg.to_string(),
            StringRange {
                start,
                end: self.reader.curr_pos(),
            },
            level,
        );
    }

    fn read_comment(&mut self, start: Pos) -> TeXToken<Pos, &'a str> {
        let (c, end) = self.reader.read_until_line_end();
        c.strip_prefix("%STEXIDE").map_or_else(
            || TeXToken::Comment(StringRange { start, end }),
            TeXToken::Directive,
        )
    }
}

/*
#[test]
fn test() {
    use std::path::PathBuf;
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    );
    let path = PathBuf::from("/home/jazzpirate/work/MathHub/courses/FAU/IWGS/problems/source/regex/prob/regex_scientific.de.tex");
    let str = std::fs::read_to_string(&path).unwrap();
    let reader = flams_utils::parsing::ParseStr::<flams_utils::sourcerefs::LSPLineCol>::new(&str);
    let tokenizer = TeXTokenizer::new(reader, Some(&path),|e,p| tracing::error!("Error {e} ({p:?})"));
    for tk in tokenizer {
        tracing::info!("{tk:?}");
    }
}
*/
