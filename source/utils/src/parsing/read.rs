use crate::{parsing::SourceParser, sourcerefs::StringPosition};
use std::io::Read;

pub struct ReadParser<R: Read, P: StringPosition> {
    inner: std::io::Bytes<std::io::BufReader<R>>,
    buf: Vec<char>,
    pos: P,
}
impl<R: Read, P: StringPosition> ReadParser<R, P> {
    pub fn new(inner: R) -> Self {
        Self {
            inner: std::io::BufReader::new(inner).bytes(),
            buf: Vec::new(),
            pos: P::default(),
        }
    }
}

impl<'a, R: Read + 'a, P: StringPosition> SourceParser<'a> for ReadParser<R, P> {
    type Pos = P;
    type Str = String;
    type Source = R;
    #[inline]
    fn curr_pos(&self) -> P {
        self.pos
    }
    #[inline]
    fn skip(&mut self, i: usize) {
        for _ in 0..i {
            self.next_char();
        }
    }
    fn next_char(&mut self) -> Option<char> {
        match self.get_char() {
            Some('\n') => {
                self.pos.inc_newline(false);
                Some('\n')
            }
            Some('\r') => {
                match self.get_char() {
                    Some('\n') | None => {
                        self.pos.inc_newline(false);
                    }
                    Some(c) => {
                        self.pos.inc_newline(false);
                        self.push_char(c);
                    }
                }
                Some('\n')
            }
            Some(c) => {
                self.pos.inc_by(c);
                Some(c)
            }
            None => None,
        }
    }
    fn read_until_line_end(&mut self) -> (String, P) {
        let (s, rn) = self.find_line_end();
        self.pos.inc_offset_by(&s);
        let pos = self.pos;
        if let Some(rn) = rn {
            self.pos.inc_newline(rn);
        }
        (s, pos)
    }
    fn trim_start(&mut self) {
        while let Some(c) = self.get_char() {
            if c == '\n' {
                self.pos.inc_newline(false);
            } else if c == '\r' {
                match self.get_char() {
                    Some('\n') => {
                        self.pos.inc_newline(true);
                    }
                    Some(c) => {
                        self.push_char(c);
                        self.pos.inc_newline(false);
                    }
                    None => {
                        self.pos.inc_newline(false);
                        break;
                    }
                }
            } else if c.is_whitespace() {
                self.pos.inc_by(c);
            } else {
                self.push_char(c);
                break;
            }
        }
    }

    #[allow(clippy::unnecessary_map_or)]
    fn starts_with(&mut self, c: char) -> bool {
        self.get_char().map_or(false, |c2| {
            self.push_char(c2);
            c2 == c
        })
    }
    fn read_while(&mut self, mut pred: impl FnMut(char) -> bool) -> Self::Str {
        let mut ret = String::new();
        let mut rn = false;
        while let Some(c) = self.get_char() {
            if !pred(c) {
                self.push_char(c);
                break;
            }
            if rn && c == '\n' {
                self.pos.inc_newline(true);
                rn = false;
                continue;
            }
            if rn {
                self.pos.inc_newline(false);
                rn = false;
            } else if c == '\n' {
                self.pos.inc_newline(false);
                ret.push('\n');
                continue;
            } else if c == '\r' {
                ret.push('\n');
                rn = true;
                continue;
            }
            self.pos.inc_by(c);
            ret.push(c);
        }
        if rn {
            self.pos.inc_newline(false);
        }
        ret
    }

    fn read_until_inclusive(&mut self, pred: impl FnMut(char) -> bool) -> Self::Str {
        let mut r = self.read_until(pred);
        if let Some(c) = self.next_char() {
            r.push(c);
        }
        r
    }

    #[inline]
    fn read_until_byte(&mut self, b: u8) -> Self::Str {
        self.read_until_char(b as char)
    }
    #[inline]
    fn read_until_char(&mut self, ch: char) -> Self::Str {
        let mut ret = String::new();
        let mut rn = false;
        while let Some(c) = self.get_char() {
            if c == ch {
                self.push_char(c);
                break;
            }
            if rn && c == '\n' {
                self.pos.inc_newline(true);
                rn = false;
                continue;
            }
            if rn {
                self.pos.inc_newline(false);
                rn = false;
            } else if c == '\n' {
                self.pos.inc_newline(false);
                ret.push('\n');
                continue;
            } else if c == '\r' {
                ret.push('\n');
                rn = true;
                continue;
            }
            self.pos.inc_by(c);
            ret.push(c);
        }
        if rn {
            self.pos.inc_newline(false);
        }
        ret
    }

    fn read_until_with_brackets(
        &mut self,
        open: char,
        close: char,
        mut pred: impl FnMut(char) -> bool,
    ) -> Self::Str {
        let mut ret = String::new();
        let mut depth = 0;
        let mut rn = false;
        while let Some(c) = self.get_char() {
            if c == open {
                depth += 1;
                self.pos.inc_by(c);
                ret.push(c);
                continue;
            } else if c == close && depth > 0 {
                depth -= 1;
                self.pos.inc_by(c);
                ret.push(c);
                continue;
            } else if depth > 0 {
                if rn && c == '\n' {
                    self.pos.inc_newline(true);
                    rn = false;
                    continue;
                }
                if rn {
                    self.pos.inc_newline(false);
                    rn = false;
                } else if c == '\n' {
                    self.pos.inc_newline(false);
                    ret.push('\n');
                    continue;
                } else if c == '\r' {
                    ret.push('\n');
                    rn = true;
                    continue;
                }
                self.pos.inc_by(c);
                ret.push(c);
                continue;
            }
            if pred(c) {
                self.push_char(c);
                break;
            }
            self.pos.inc_by(c);
            ret.push(c);
        }
        if rn {
            self.pos.inc_newline(false);
        }
        ret
    }

    #[inline]
    fn read_until_byte_with_brackets(&mut self, b: u8, open: u8, close: u8) -> Self::Str {
        self.read_until_with_brackets(open as _, close as _, |c| c == b as char)
    }

    fn peek_head(&mut self) -> Option<char> {
        self.get_char().inspect(|c| {
            self.push_char(*c);
        })
    }
    fn read_n(&mut self, i: usize) -> Self::Str {
        let mut ret = String::new();
        for _ in 0..i {
            if let Some(c) = self.next_char() {
                ret.push(c);
            } else {
                break;
            }
        }
        ret
    }

    fn read_until_str(&mut self, s: &str) -> Self::Str {
        let mut ret = String::with_capacity(32);
        while let Some(c) = self.next_char() {
            ret.push(c);
            if ret.ends_with(s) {
                for _ in 0..s.len() {
                    self.push_char(ret.pop().unwrap_or_else(|| unreachable!()));
                }
                return ret;
            }
        }
        ret
    }

    #[inline]
    fn read_until_needle(&mut self, s: &super::Needle) -> Self::Str {
        self.read_until_str(s.as_str())
    }

    fn starts_with_str(&mut self, s: &str) -> bool {
        let mut read = 0;
        macro_rules! nope {
            () => {{
                for c in s.chars().take(read) {
                    self.push_char(c);
                }
                return false;
            }};
        }
        for c in s.chars() {
            if let Some(c2) = self.next_char() {
                read += 1;
                if c != c2 {
                    nope!()
                }
            } else {
                nope!();
            }
        }
        self.buf.extend(s.chars().rev());
        true
    }

    fn drop_prefix(&mut self, s: &str) -> bool {
        let mut read = 0;
        macro_rules! nope {
            () => {{
                for c in s.chars().take(read) {
                    self.push_char(c);
                }
                return false;
            }};
        }
        for c in s.chars() {
            if let Some(c2) = self.next_char() {
                read += 1;
                if c != c2 {
                    nope!()
                }
            } else {
                nope!();
            }
        }
        true
    }
}

impl<R: Read, P: StringPosition> ReadParser<R, P> {
    #[inline]
    fn get_char(&mut self) -> Option<char> {
        self.buf.pop().or_else(|| self.read_char())
    }
    fn read_char(&mut self) -> Option<char> {
        self.inner.next().and_then(|byte| {
            byte.ok().and_then(|byte| {
                if byte & 0b1110_0000_u8 == 192u8 {
                    // a two byte unicode character
                    let next = self.inner.next().and_then(Result::ok)?;
                    Self::char_from_utf8(&[byte, next])
                } else if byte & 0b1111_0000_u8 == 224u8 {
                    // a three byte unicode character
                    let next1 = self.inner.next().and_then(Result::ok)?;
                    let next2 = self.inner.next().and_then(Result::ok)?;
                    Self::char_from_utf8(&[byte, next1, next2])
                } else if byte & 0b1111_1000_u8 == 240u8 {
                    // a four byte unicode character
                    let next1 = self.inner.next().and_then(Result::ok)?;
                    let next2 = self.inner.next().and_then(Result::ok)?;
                    let next3 = self.inner.next().and_then(Result::ok)?;
                    Self::char_from_utf8(&[byte, next1, next2, next3])
                } else {
                    Some(byte as char)
                }
            })
        })
    }
    fn push_char(&mut self, c: char) {
        self.buf.push(c);
    }
    fn char_from_utf8(buf: &[u8]) -> Option<char> {
        std::str::from_utf8(buf).ok().and_then(|s| s.chars().next())
    }
    fn find_line_end(&mut self) -> (String, Option<bool>) {
        let mut ret = String::new();
        while let Some(c) = self.get_char() {
            if c == '\n' {
                return (ret, Some(false));
            }
            if c == '\r' {
                match self.get_char() {
                    Some('\n') => return (ret, Some(true)),
                    Some(c) => self.push_char(c),
                    None => (),
                }
                return (ret, Some(true));
            }
            ret.push(c);
        }
        (ret, None)
    }
}
