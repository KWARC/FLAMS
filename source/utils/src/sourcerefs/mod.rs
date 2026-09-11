mod convert;
mod linecol;
//mod lineendings;
mod offsets;
mod ranges;

pub use linecol::*;
//pub use lineendings::*;
pub use offsets::*;
pub use ranges::*;

use crate::CondSerialize;
use std::fmt::Debug;

pub type LSPLineCol = LineCol<Utf16Offset>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionKind {
    Byte,
    Char,
    Utf16,
    LineColByte,
    LineColChar,
    LineColUtf16,
    None,
}

#[allow(clippy::inline_always)]
pub trait StringPosition:
    std::fmt::Debug
    + std::fmt::Display
    + Clone
    + Copy
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + Default
    + std::hash::Hash
    + std::ops::Add<Output = Self>
    + std::ops::AddAssign
    + std::ops::Sub<Output = Self>
    + std::ops::SubAssign
    + crate::__private::Sealed
    + CondSerialize
    + 'static
{
    const KIND: PositionKind;
    fn from_other<P: StringPosition>(p: P, source: &str) -> Self;

    #[inline(always)]
    #[must_use]
    fn from_many<P: StringPosition>(source: &str) -> PositionConverter<'_, P, Self> {
        PositionConverter::new(source)
    }

    fn len(s: &str) -> Self;

    #[inline(always)]
    fn into_other<P: StringPosition>(self, source: &str) -> P {
        P::from_other(self, source)
    }

    #[inline(always)]
    #[must_use]
    fn into_many<P: StringPosition>(source: &str) -> PositionConverter<'_, Self, P> {
        PositionConverter::new(source)
    }
    fn get_range(start: Self, end: Self, text: &str) -> Option<&str> {
        let off =
            PositionConverter::<Self, ByteOffset>::new(text).next_range(StringRange { start, end });
        text.get(off.start.0..off.end.0)
    }

    fn inc_offset_by(&mut self, text: &str);
    fn inc_by(&mut self, c: char);
    fn inc_newline(&mut self, crlf: bool);
}

/// Convert multiple successive positions or ranges in a string.
///
/// More performant
/// than calling [`StringPosition::from_other`] for every single one, since
/// we can only need to consider the not already covered part of the string.
pub struct PositionConverter<'s, From: StringPosition, To: StringPosition> {
    source: &'s str,
    from: From,
    to: To,
}
impl<'s, From: StringPosition, To: StringPosition> PositionConverter<'s, From, To> {
    #[must_use]
    #[inline]
    pub fn new(source: &'s str) -> Self {
        Self {
            source,
            from: From::default(),
            to: To::default(),
        }
    }

    #[inline]
    pub const fn current_position(&self) -> To {
        self.to
    }

    #[inline]
    pub fn next_range(&mut self, range: StringRange<From>) -> StringRange<To> {
        StringRange {
            start: self.next(range.start),
            end: self.next(range.end),
        }
    }

    #[inline]
    pub fn next(&mut self, pos: From) -> To {
        let r = unsafe {
            match (From::KIND, To::KIND) {
                (p, o) if p == o => {
                    self.from = pos;
                    self.to = std::mem::transmute_copy(&pos);
                    return self.to;
                }
                (PositionKind::Byte, _) => {
                    self.next_i(std::mem::transmute_copy::<_, ByteOffset>(&(pos - self.from)).0)
                }
                (_, PositionKind::Byte) => {
                    let r: ByteOffset = (pos - self.from).into_other(self.source);
                    self.source = &self.source[r.0..];
                    std::mem::transmute_copy(&r)
                }
                _ => self.next_i((pos - self.from).into_other::<ByteOffset>(self.source).0),
            }
        };
        self.from = pos;
        self.to += r;
        self.to
    }
    fn next_i(&mut self, offset: usize) -> To {
        let r = To::len(&self.source[..offset]);
        self.source = &self.source[offset..];
        r
    }
}
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct NoPosition;
impl crate::__private::Sealed for NoPosition {}
impl std::fmt::Display for NoPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
impl StringPosition for NoPosition {
    const KIND: PositionKind = PositionKind::None;
    #[inline]
    fn from_other<P: StringPosition>(_: P, _: &str) -> Self {
        Self
    }
    #[inline]
    fn len(_: &str) -> Self {
        Self
    }
    #[inline(always)]
    fn inc_by(&mut self, _: char) {}
    #[inline(always)]
    fn inc_newline(&mut self, _: bool) {}
    #[inline(always)]
    fn inc_offset_by(&mut self, _: &str) {}
}
impl std::ops::Add for NoPosition {
    type Output = Self;
    #[inline]
    fn add(self, _: Self) -> Self::Output {
        Self
    }
}
impl std::ops::AddAssign for NoPosition {
    #[inline(always)]
    fn add_assign(&mut self, _: Self) {}
}
impl std::ops::Sub for NoPosition {
    type Output = Self;
    #[inline]
    fn sub(self, _: Self) -> Self::Output {
        Self
    }
}
impl std::ops::SubAssign for NoPosition {
    #[inline(always)]
    fn sub_assign(&mut self, _: Self) {}
}

/*
pub trait StringPosition:
    Clone + Copy + Default + Debug + PartialOrd + Ord + 'static + CondSerialize
{
    fn inc_by(&mut self, c: char);
    fn inc_newline(&mut self, crlf: bool);
    //fn update_str_no_newline(&mut self, s: &str);
    //fn update_str_maybe_newline(&mut self, s: &str);
    //fn get_range(start: Self, end: Self, text: &str) -> Option<&str>;
}
impl StringPosition for () {
    #[inline]
    fn inc_by(&mut self, _: char) {}
    #[inline]
    fn inc_newline(&mut self, _: bool) {}
    /*#[inline]
    fn update_str_no_newline(&mut self, _: &str) {}
    #[inline]
    fn update_str_maybe_newline(&mut self, _: &str) {}
    #[inline]
    fn get_range(start: Self, end: Self, text: &str) -> Option<&str> {
        None
    }*/
}

#[derive(Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ByteOffset(usize);
impl Display for ByteOffset {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl Debug for ByteOffset {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}
impl StringPosition for ByteOffset {
    #[inline]
    fn inc_by(&mut self, c: char) {
        self.offset += c.len_utf8();
    }
    #[inline]
    fn inc_newline(&mut self, rn: bool) {
        self.offset += if rn { 2 } else { 1 };
    }
    /*
    #[inline]
    fn update_str_no_newline(&mut self, s: &str) {
        self.offset += s.len();
    }
    #[inline]
    fn update_str_maybe_newline(&mut self, s: &str) {
        self.update_str_no_newline(s);
    }
    fn get_range(start: Self, end: Self, text: &str) -> Option<&str> {
        text.get(start.offset..end.offset)
    }
     */
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LSPLineCol {
    pub line: u32,
    pub col: u32,
}
impl Ord for LSPLineCol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}
impl PartialOrd for LSPLineCol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for LSPLineCol {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "l. {} c. {}", self.line, self.col)
    }
}
impl Debug for LSPLineCol {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}
impl LSPLineCol {
    // TODO use UTF16 instead of UTF8
    pub fn get_range_offsets(start: Self, end: Self, text: &str) -> (usize, usize) {
        let Self {
            line: mut start_line,
            col: startc,
        } = start;
        let Self {
            line: mut end_line,
            col: mut endc,
        } = end;
        if start_line == end_line {
            endc -= startc;
        }
        end_line -= start_line;
        let mut start = 0;
        let mut rest = text;
        while start_line > 0 {
            if let Some(i) = rest.find(['\n', '\r']) {
                start += i + 1;
                if rest.as_bytes()[i] == b'\r' && rest.as_bytes().get(i + 1) == Some(&b'\n') {
                    start += 1;
                    rest = &rest[i + 2..];
                } else {
                    rest = &rest[i + 1..];
                }
                start_line -= 1;
            } else {
                start = text.len();
                rest = "";
                end_line = 0;
                break;
            }
        }

        let next = rest
            .chars()
            .take(startc as usize)
            .map(char::len_utf8)
            .sum::<usize>();
        start += next;
        rest = &rest[next..];

        let mut end = start;
        while end_line > 0 {
            if let Some(i) = rest.find(['\n', '\r']) {
                end += i + 1;
                if rest.as_bytes()[i] == b'\r' && rest.as_bytes().get(i + 1) == Some(&b'\n') {
                    end += 1;
                    rest = &rest[i + 2..];
                } else {
                    rest = &rest[i + 1..];
                }
                end_line -= 1;
            } else {
                end = text.len();
                rest = "";
                break;
            }
        }
        end += rest
            .chars()
            .take(endc as usize)
            .map(char::len_utf8)
            .sum::<usize>();
        (start, end)
    }
}
impl StringPosition for LSPLineCol {
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn inc_by(&mut self, c: char) {
        self.col += c.len_utf16() as u32;
    }
    #[inline]
    fn inc_newline(&mut self, _: bool) {
        self.line += 1;
        self.col = 0;
    }
    /*
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn update_str_no_newline(&mut self, s: &str) {
        self.col += s.chars().map(|c| char::len_utf16(c) as u32).sum::<u32>();
    }

    #[allow(clippy::cast_possible_truncation)]
    fn update_str_maybe_newline(&mut self, s: &str) {
        let s = s.split("\r\n").flat_map(|s| s.split(['\n', '\r']));
        let mut last = "";
        let mut first = true;
        for l in s {
            if first {
                first = false;
            } else {
                self.line += 1;
                self.col = 0;
            }
            last = l;
        }
        self.col += last.chars().map(|c| char::len_utf16(c) as u32).sum::<u32>();
    }
    fn get_range(start: Self, end: Self, text: &str) -> Option<&str> {
        let (nstart, nend) = Self::get_range_offsets(start, end, text);
        if nstart == nend {
            Some("")
        } else if nend < text.len() {
            text.get(nstart..nend)
        } else {
            None
        }
    }
     */
}

impl<A: StringPosition, B: StringPosition> StringPosition for (A, B) {
    #[inline]
    fn inc_by(&mut self, c: char) {
        self.0.inc_by(c);
        self.1.inc_by(c);
    }
    #[inline]
    fn inc_newline(&mut self, rn: bool) {
        self.0.inc_newline(rn);
        self.1.inc_newline(rn);
    }
    /*
    #[inline]
    fn update_str_no_newline(&mut self, s: &str) {
        self.0.update_str_no_newline(s);
        self.1.update_str_no_newline(s);
    }
    #[inline]
    fn update_str_maybe_newline(&mut self, s: &str) {
        self.0.update_str_maybe_newline(s);
        self.1.update_str_maybe_newline(s);
    }
    fn get_range(start: Self, end: Self, text: &str) -> Option<&str> {
        A::get_range(start.0, end.0, text)
    }
     */
}
 */

/*
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StringRange<P: StringPosition> {
    pub start: P,
    pub end: P,
}
impl<P: StringPosition> Display for StringRange<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:#?}-{:#?}", self.start, self.end)
    }
}
impl<P: StringPosition> Debug for StringRange<P> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}
impl<P: StringPosition> StringRange<P> {
    pub fn contains(&self, pos: P) -> bool {
        self.start <= pos && pos <= self.end
    }
}
 */

#[test]
fn test() {
    let str = "\n\n";
    let len = str
        .split("\r\n")
        .flat_map(|s| s.split(['\r', '\n']))
        .count();
    assert_eq!(len, 3);
}
