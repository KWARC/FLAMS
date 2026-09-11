mod read;
mod string;

pub use read::ReadParser;
pub use string::StrParser;

use crate::sourcerefs::StringPosition;

/// A wrapper around a [`memchr::memmem::Finder`] that is guaranteed by construction to
/// be a valid utf-8 [`str`]
pub struct Needle<'a>(memchr::memmem::Finder<'a>);

#[allow(clippy::inline_always)]
impl<'a> Needle<'a> {
    #[inline(always)]
    #[must_use]
    pub fn new(needle: &'a str) -> Self {
        Self(memchr::memmem::Finder::new(needle))
    }
    #[inline(always)]
    #[must_use]
    pub fn as_str(&self) -> &str {
        // SAFETY: constructed from a str
        unsafe { str::from_utf8_unchecked(self.0.needle()) }
    }
    #[inline(always)]
    #[must_use]
    pub fn find_in(&self, haystack: &str) -> Option<usize> {
        self.0.find(haystack.as_bytes())
    }
    #[inline(always)]
    #[must_use]
    pub fn find_in_bytes(&self, haystack: &[u8]) -> Option<usize> {
        self.0.find(haystack)
    }
}

pub trait SourceParser<'a>: 'a {
    type Pos: StringPosition;
    type Str: StringOrStr<'a>;
    type Source;
    fn curr_pos(&self) -> Self::Pos;
    fn next_char(&mut self) -> Option<char>;
    fn read_until_line_end(&mut self) -> (Self::Str, Self::Pos);
    fn trim_start(&mut self);
    fn starts_with(&mut self, c: char) -> bool;
    fn peek_head(&mut self) -> Option<char>;
    fn read_n(&mut self, i: usize) -> Self::Str;
    fn read_while(&mut self, pred: impl FnMut(char) -> bool) -> Self::Str;
    #[inline]
    fn read_until(&mut self, mut pred: impl FnMut(char) -> bool) -> Self::Str {
        self.read_while(|c| !pred(c))
    }
    fn read_until_inclusive(&mut self, pred: impl FnMut(char) -> bool) -> Self::Str;
    fn read_until_str(&mut self, s: &str) -> Self::Str;
    fn read_until_needle(&mut self, s: &Needle) -> Self::Str;
    fn read_until_byte(&mut self, b: u8) -> Self::Str;
    fn read_until_char(&mut self, c: char) -> Self::Str;

    fn starts_with_str(&mut self, s: &str) -> bool;
    fn drop_prefix(&mut self, s: &str) -> bool;
    fn read_until_with_brackets(
        &mut self,
        open: char,
        close: char,
        pred: impl FnMut(char) -> bool,
    ) -> Self::Str;
    fn read_until_byte_with_brackets(&mut self, b: u8, open: u8, close: u8) -> Self::Str;
    fn skip(&mut self, i: usize);
}

pub trait StringOrStr<'a>:
    AsRef<str>
    + From<&'a str>
    + std::fmt::Debug
    + std::fmt::Display
    + Eq
    + std::hash::Hash
    + Clone
    + for<'b> PartialEq<&'b str>
    + Into<std::borrow::Cow<'a, str>>
{
    /// Self == &str
    const IS_STR: bool;
    /// # Errors
    ///
    /// Will return `Err` if self does not start with prefix.
    fn strip_prefix(self, s: &str) -> Result<Self, Self>;
    #[must_use]
    fn split_n(self, n: usize) -> (Self, Self);
    fn trim_ws(&mut self);
    fn split_noparens_bytes(
        &'a self,
        open: u8,
        close: u8,
        split_char: u8,
    ) -> impl Iterator<Item = &'a str>;
    fn split_noparens(
        &'a self,
        open: char,
        close: char,
        split_char: char,
    ) -> impl Iterator<Item = &'a str>;
    fn as_cow(&self) -> std::borrow::Cow<'a, str>;
}

impl<'a> StringOrStr<'a> for &'a str {
    const IS_STR: bool = true;
    #[inline]
    fn strip_prefix(self, s: &str) -> Result<Self, Self> {
        str::strip_prefix(self, s).map(str::trim_start).ok_or(self)
    }
    #[inline]
    fn split_n(self, n: usize) -> (Self, Self) {
        (&self[..n], &self[n..])
    }
    #[inline]
    fn trim_ws(&mut self) {
        *self = self.trim();
    }
    fn split_noparens_bytes(
        &'a self,
        open: u8,
        close: u8,
        split_char: u8,
    ) -> impl Iterator<Item = &'a str> {
        struct It<'a> {
            open: u8,
            close: u8,
            split_char: u8,
            in_brackets: u8,
            s: &'a str,
        }
        impl<'a> Iterator for It<'a> {
            type Item = &'a str;
            fn next(&mut self) -> Option<Self::Item> {
                if self.s.is_empty() {
                    return None;
                }
                loop {
                    if self.in_brackets == 0 {
                        if let Some(i) =
                            memchr::memchr2(self.split_char, self.open, self.s.as_bytes())
                        {
                            let prefix = &self.s[..i];
                            let b = self.s.as_bytes()[i];
                            self.s = if prefix.len() == self.s.len() {
                                ""
                            } else {
                                &self.s[i + 1..]
                            };
                            if b == self.split_char {
                                return Some(prefix);
                            }
                            self.in_brackets += 1;
                        } else {
                            let r = self.s;
                            self.s = "";
                            return Some(r);
                        }
                    } else if let Some(i) =
                        memchr::memchr2(self.open, self.close, self.s.as_bytes())
                    {
                        let prefix = &self.s[..i];
                        let b = self.s.as_bytes()[i];
                        self.s = if prefix.len() == self.s.len() {
                            ""
                        } else {
                            &self.s[i + 1..]
                        };
                        if b == self.open {
                            self.in_brackets += 1;
                        } else {
                            self.in_brackets -= 1;
                        }
                    } else {
                        let r = self.s;
                        self.s = "";
                        return Some(r);
                    }
                }
            }
        }

        It {
            open,
            close,
            split_char,
            in_brackets: 0,
            s: self,
        }
    }
    fn split_noparens(
        &'a self,
        open: char,
        close: char,
        split_char: char,
    ) -> impl Iterator<Item = &'a str> {
        let mut depth = 0;
        self.split(move |c: char| {
            if c == open {
                depth += 1;
                false
            } else if c == close && depth > 0 {
                depth -= 1;
                false
            } else if depth > 0 {
                false
            } else {
                c == split_char
            }
        })
    }
    #[inline]
    fn as_cow(&self) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Borrowed(self)
    }
}

impl<'a> StringOrStr<'a> for String {
    const IS_STR: bool = false;

    #[allow(clippy::option_if_let_else)]
    fn strip_prefix(self, s: &str) -> Result<Self, Self> {
        match str::strip_prefix(&self, s) {
            Some(s) => Ok(s.trim_start().to_string()),
            None => Err(self),
        }
    }
    #[inline]
    fn trim_ws(&mut self) {
        if let Some(i) = self.find(|c: char| !c.is_ascii_whitespace())
            && i > 0
        {
            *self = self.split_off(i);
        }
        if let Some(i) = self.rfind(|c: char| !c.is_ascii_whitespace())
            && i < self.len()
        {
            let _ = self.split_off(i);
        }
    }
    fn split_n(mut self, n: usize) -> (Self, Self) {
        let r = self.split_off(n);
        (self, r)
    }
    fn split_noparens_bytes(
        &'a self,
        open: u8,
        close: u8,
        split_char: u8,
    ) -> impl Iterator<Item = &'a str> {
        self.split_noparens(open as _, close as _, split_char as _)
    }
    fn split_noparens(
        &'a self,
        open: char,
        close: char,
        split_char: char,
    ) -> impl Iterator<Item = &'a str> {
        let mut depth = 0;
        self.split(move |c: char| {
            if c == open {
                depth += 1;
                false
            } else if c == close && depth > 0 {
                depth -= 1;
                false
            } else if depth > 0 {
                false
            } else {
                c == split_char
            }
        })
    }
    #[inline]
    fn as_cow(&self) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Owned(self.clone())
    }
}
