use crate::{
    CharExt,
    parsing::SourceParser,
    sourcerefs::{PositionKind, StringPosition},
};

pub struct StrParser<'a, P: StringPosition> {
    input: &'a str,
    pub pos: P,
}

impl<'a, P: StringPosition> StrParser<'a, P> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: P::default(),
        }
    }
    #[inline]
    pub const fn rest(&self) -> &'a str {
        self.input
    }

    pub fn read_until_is(&mut self, mut pred: impl FnMut(&'a str) -> bool) -> &'a str {
        let mut curr = self.input;
        while !curr.is_empty() {
            if pred(curr) {
                let ret = &self.input[..self.input.len() - curr.len()];
                self.input = curr;
                self.pos += P::len(ret);
                return ret;
            }
            if let Some(next) = curr.chars().next() {
                curr = &curr[next.len_utf8()..];
            }
        }
        self.pos += P::len(self.input);
        std::mem::take(&mut self.input)
    }

    pub fn preview_until_with_brackets(
        &self,
        open: char,
        close: char,
        mut pred: impl FnMut(char) -> bool,
    ) -> &'a str {
        let mut depth = 0;
        let i = self
            .input
            .find(|c| {
                if c == open {
                    depth += 1;
                    false
                } else if c == close && depth > 0 {
                    depth -= 1;
                    false
                } else {
                    depth == 0 && pred(c)
                }
            })
            .unwrap_or(self.input.len());
        let (l, _r) = self.input.split_at(i);
        l
    }

    pub fn read_until_escaped(&mut self, find: char, escape: char) -> &'a str {
        let mut chars = self.input.chars();
        let mut i: usize = 0;
        while let Some(c) = chars.next() {
            if c == escape {
                if let Some(c) = chars.next() {
                    i += c.len_utf8();
                }
            } else if c == find {
                let (l, r) = self.input.split_at(i);
                self.input = r;
                self.pos += P::len(l);
                return l;
            }
            i += c.len_utf8();
        }
        let ret = self.input;
        self.input = "";
        self.pos += P::len(ret);
        ret
    }
}

impl<'a, P: StringPosition> SourceParser<'a> for StrParser<'a, P> {
    type Pos = P;
    type Str = &'a str;
    type Source = &'a str;

    #[inline]
    fn curr_pos(&self) -> P {
        self.pos
    }

    #[inline]
    fn starts_with_str(&mut self, s: &str) -> bool {
        self.input.starts_with(s)
    }
    fn drop_prefix(&mut self, s: &str) -> bool {
        self.input.starts_with(s) && {
            self.input = &self.input[s.len()..];
            self.pos += P::len(s);
            true
        }
    }

    fn next_char(&mut self) -> Option<char> {
        if self.input.starts_with(['\r', '\n']) {
            if self.input.starts_with("\r\n") {
                self.input = &self.input[2..];
                self.pos.inc_newline(true);
            } else {
                self.input = &self.input[1..];
                self.pos.inc_newline(false);
            }
            return Some('\n');
        }
        self.input.chars().next().inspect(|c| {
            self.pos.inc_by(*c);
            self.input = &self.input[c.len_utf8()..];
        })
    }

    fn read_until_line_end(&mut self) -> (&'a str, P) {
        if let Some(i) = memchr::memchr2(b'\r', b'\n', self.input.as_bytes()) {
            if self.input.as_bytes()[i] == b'\r'
                && self.input.as_bytes().get(i + 1).copied() == Some(b'\n')
            {
                let (l, r) = self.input.split_at(i);
                self.input = &r[2..];
                self.pos.inc_offset_by(l);
                let pos = self.pos;
                self.pos.inc_newline(true);
                return (l, pos);
            }
            let (l, r) = self.input.split_at(i);
            self.input = &r[1..];
            self.pos.inc_offset_by(l);
            let pos = self.pos;
            self.pos.inc_newline(false);
            (l, pos)
        } else {
            let ret = self.input;
            self.pos.inc_offset_by(ret);
            self.input = "";
            (ret, self.pos)
        }
    }

    fn read_until_byte(&mut self, b: u8) -> Self::Str {
        if let Some(i) = memchr::memchr(b, self.input.as_bytes()) {
            let (l, r) = self.input.split_at(i);
            self.input = r;
            self.pos += P::len(l);
            l
        } else {
            let ret = self.input;
            self.pos += P::len(ret);
            self.input = "";
            ret
        }
    }
    fn read_until_char(&mut self, c: char) -> Self::Str {
        let bytes = c.into_bytes();
        let del = bytes[0];
        for i in memchr::memchr_iter(del, self.input.as_bytes()) {
            if self.input.as_bytes()[i..].starts_with(&bytes) {
                let (l, r) = self.input.split_at(i);
                self.input = r;
                self.pos += P::len(l);
                return l;
            }
        }
        let ret = self.input;
        self.pos += P::len(ret);
        self.input = "";
        ret
    }

    fn read_until_byte_with_brackets(&mut self, needle: u8, open: u8, close: u8) -> Self::Str {
        let mut in_brackets = 0;
        let mut curroff = 0;
        let bytes = self.input.as_bytes();
        loop {
            if in_brackets == 0 {
                if let Some(i) = memchr::memchr2(needle, open, &bytes[curroff..]) {
                    curroff += i;
                    let b = bytes[curroff];
                    if b == needle {
                        let (l, r) = self.input.split_at(curroff);
                        self.input = r;
                        self.pos += P::len(l);
                        return l;
                    }
                    curroff += 1;
                    in_brackets += 1;
                } else {
                    let ret = self.input;
                    self.input = "";
                    self.pos += P::len(ret);
                    return ret;
                }
            } else if let Some(i) = memchr::memchr2(open, close, &bytes[curroff..]) {
                curroff += i;
                let b = bytes[curroff];
                curroff += 1;
                if b == open {
                    in_brackets += 1;
                } else {
                    in_brackets -= 1;
                }
            } else {
                let ret = self.input;
                self.input = "";
                self.pos += P::len(ret);
                return ret;
            }
        }
    }

    fn read_until_inclusive(&mut self, pred: impl FnMut(char) -> bool) -> &'a str {
        let i = self.input.find(pred).unwrap_or(self.input.len());
        let (l, r) = self.input.split_at(i + 1);
        self.input = r;
        self.pos += P::len(l);
        l
    }

    fn trim_start(&mut self) {
        if P::KIND == PositionKind::None {
            self.input = self.input.trim_start();
            return;
        }
        if P::KIND == PositionKind::Byte {
            let old = self.input;
            self.input = self.input.trim_start();
            let trimmed = &old[..old.len() - self.input.len()];
            self.pos += P::len(trimmed);
            return;
        }
        while let Some(c) = self.input.chars().next() {
            if c == '\n' {
                self.input = &self.input[1..];
                self.pos.inc_newline(false);
            } else if c == '\r' {
                self.input = &self.input[1..];
                if self.input.starts_with('\n') {
                    self.input = &self.input[1..];
                    self.pos.inc_newline(true);
                } else {
                    self.pos.inc_newline(false);
                }
            } else if c.is_whitespace() {
                self.input = &self.input[c.len_utf8()..];
                self.pos.inc_by(c);
            } else {
                break;
            }
        }
    }
    fn starts_with(&mut self, c: char) -> bool {
        self.input.starts_with(c)
    }
    fn read_while(&mut self, mut pred: impl FnMut(char) -> bool) -> Self::Str {
        let i = self.input.find(|c| !pred(c)).unwrap_or(self.input.len());
        let (l, r) = self.input.split_at(i);
        self.input = r;
        self.pos += P::len(l);
        l
    }
    fn read_until_with_brackets(
        &mut self,
        open: char,
        close: char,
        mut pred: impl FnMut(char) -> bool,
    ) -> Self::Str {
        let mut depth = 0;
        let i = self
            .input
            .find(|c| {
                if c == close && depth > 0 {
                    depth -= 1;
                    false
                } else if c == open {
                    depth += 1;
                    false
                } else {
                    depth == 0 && pred(c)
                }
            })
            .unwrap_or(self.input.len());
        let (l, r) = self.input.split_at(i);
        self.input = r;
        self.pos += P::len(l);
        l
    }
    fn peek_head(&mut self) -> Option<char> {
        self.input.chars().next()
    }
    fn read_n(&mut self, i: usize) -> Self::Str {
        let (l, mut r) = self.input.split_at(i);
        if l.ends_with('\r') && r.starts_with('\n') {
            r = &r[1..];
        }
        self.input = r;
        self.pos += P::len(l);
        l
    }

    fn read_until_str(&mut self, s: &str) -> Self::Str {
        if let Some(i) = self.input.find(s) {
            let (l, r) = self.input.split_at(i);
            self.input = r;
            self.pos += P::len(l);
            l
        } else {
            let ret = self.input;
            self.input = "";
            self.pos += P::len(ret);
            ret
        }
    }

    fn read_until_needle(&mut self, s: &super::Needle) -> Self::Str {
        if let Some(i) = s.find_in(self.input) {
            let (l, r) = self.input.split_at(i);
            self.input = r;
            self.pos += P::len(l);
            l
        } else {
            let ret = self.input;
            self.input = "";
            self.pos += P::len(ret);
            ret
        }
    }

    fn skip(&mut self, i: usize) {
        let (a, b) = self.input.split_at(i);
        self.input = b;
        self.pos += P::len(a);
    }
}
