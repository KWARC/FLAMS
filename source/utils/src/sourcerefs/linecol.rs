use std::marker::PhantomData;

use super::{ByteOffset, CharOffset, OffsetPosition, PositionKind, StringPosition, Utf16Offset};

/// (line,column)-pair; where column is an offset
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct LineCol<Off: OffsetPosition = ByteOffset>
where
    Self: StringPosition,
{
    pub line: u32,
    pub col: u32,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) __phantom: PhantomData<Off>,
}
impl<Off: OffsetPosition> std::fmt::Display for LineCol<Off>
where
    Self: StringPosition,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{})", self.line, self.col)
    }
}
impl<Off: OffsetPosition> std::fmt::Debug for LineCol<Off>
where
    Self: StringPosition,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("LineCol")
            .field(&self.line)
            .field(&self.col)
            .finish()
    }
}
impl<Off: OffsetPosition> LineCol<Off>
where
    Self: StringPosition,
{
    #[inline]
    #[must_use]
    pub const fn new(line: u32, col: u32) -> Self {
        Self {
            line,
            col,
            __phantom: PhantomData,
        }
    }
}
impl<Off: OffsetPosition> crate::__private::Sealed for LineCol<Off> where Self: StringPosition {}
impl<Off: OffsetPosition> std::ops::Add for LineCol<Off>
where
    Self: StringPosition,
{
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            line: self.line + rhs.line,
            col: if rhs.line == 0 {
                self.col + rhs.col
            } else {
                rhs.col
            },
            __phantom: PhantomData,
        }
    }
}
impl<Off: OffsetPosition> std::ops::AddAssign for LineCol<Off>
where
    Self: StringPosition,
{
    fn add_assign(&mut self, rhs: Self) {
        self.line += rhs.line;
        if rhs.line == 0 {
            self.col += rhs.col;
        } else {
            self.col = rhs.col;
        }
    }
}
impl<Off: OffsetPosition> std::ops::Sub for LineCol<Off>
where
    Self: StringPosition,
{
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        if rhs.line == self.line {
            Self {
                line: 0,
                col: self.col - rhs.col,
                __phantom: PhantomData,
            }
        } else {
            Self {
                line: self.line.saturating_sub(rhs.line),
                col: self.col,
                __phantom: PhantomData,
            }
        }
    }
}
impl<Off: OffsetPosition> std::ops::SubAssign for LineCol<Off>
where
    Self: StringPosition,
{
    fn sub_assign(&mut self, rhs: Self) {
        if rhs.line == self.line {
            self.line = 0;
        } else {
            self.line = self.line.saturating_sub(rhs.line);
            self.col -= rhs.col;
        }
    }
}
impl<Off: OffsetPosition> Ord for LineCol<Off>
where
    Self: StringPosition,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.line.cmp(&other.line) {
            std::cmp::Ordering::Equal => self.col.cmp(&other.col),
            o => o,
        }
    }
}

#[allow(clippy::inline_always)]
impl<Off: OffsetPosition> PartialOrd for LineCol<Off>
where
    Self: StringPosition,
{
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl StringPosition for LineCol<ByteOffset> {
    const KIND: PositionKind = PositionKind::LineColByte;

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn inc_by(&mut self, c: char) {
        self.col += c.len_utf8() as u32;
    }
    fn inc_newline(&mut self, _: bool) {
        self.col = 0;
        self.line += 1;
    }

    #[allow(clippy::cast_possible_truncation)]
    fn inc_offset_by(&mut self, text: &str) {
        self.col += text.len() as u32;
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn len(s: &str) -> Self {
        let (line, off) = super::convert::count_lines(s);
        Self {
            line: line as _,
            col: (s.len() - (off as usize)) as _,
            __phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn from_other<P: StringPosition>(p: P, source: &str) -> Self {
        use PositionKind as K;
        unsafe {
            match P::KIND {
                K::None => Self::default(),
                K::Byte => {
                    super::convert::byte_to_linecol_byte(std::mem::transmute_copy(&p), source)
                }
                K::Char => {
                    super::convert::char_to_linecol_byte(std::mem::transmute_copy(&p), source)
                }
                K::Utf16 => {
                    super::convert::utf16_to_linecol_byte(std::mem::transmute_copy(&p), source)
                }
                K::LineColByte => std::mem::transmute_copy(&p),
                K::LineColChar => super::convert::linecol_to_linecol::<CharOffset, _>(
                    std::mem::transmute_copy(&p),
                    source,
                ),
                K::LineColUtf16 => super::convert::linecol_to_linecol::<Utf16Offset, _>(
                    std::mem::transmute_copy(&p),
                    source,
                ),
            }
        }
    }
}

impl StringPosition for LineCol<CharOffset> {
    const KIND: PositionKind = PositionKind::LineColChar;

    #[inline]
    fn inc_by(&mut self, _: char) {
        self.col += 1;
    }
    fn inc_newline(&mut self, _: bool) {
        self.col = 0;
        self.line += 1;
    }

    #[allow(clippy::cast_possible_truncation)]
    fn inc_offset_by(&mut self, text: &str) {
        self.col += text.chars().count() as u32;
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn len(s: &str) -> Self {
        let (line, off) = super::convert::count_lines(s);
        Self {
            line: line as _,
            col: s[off as usize..].chars().count() as _,
            __phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn from_other<P: StringPosition>(p: P, source: &str) -> Self {
        use PositionKind as K;
        unsafe {
            match P::KIND {
                K::None => Self::default(),
                K::Byte => {
                    super::convert::byte_to_linecol_char(std::mem::transmute_copy(&p), source)
                }
                K::Char => {
                    super::convert::char_to_linecol_char(std::mem::transmute_copy(&p), source)
                }
                K::Utf16 => {
                    super::convert::utf16_to_linecol_char(std::mem::transmute_copy(&p), source)
                }
                K::LineColChar => std::mem::transmute_copy(&p),
                K::LineColByte => super::convert::linecol_to_linecol::<ByteOffset, _>(
                    std::mem::transmute_copy(&p),
                    source,
                ),
                K::LineColUtf16 => super::convert::linecol_to_linecol::<Utf16Offset, _>(
                    std::mem::transmute_copy(&p),
                    source,
                ),
            }
        }
    }
}

impl StringPosition for LineCol<Utf16Offset> {
    const KIND: PositionKind = PositionKind::LineColUtf16;

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn inc_by(&mut self, c: char) {
        self.col += c.len_utf16() as u32;
    }
    fn inc_newline(&mut self, _: bool) {
        self.col = 0;
        self.line += 1;
    }

    #[allow(clippy::cast_possible_truncation)]
    fn inc_offset_by(&mut self, text: &str) {
        self.col += text.chars().map(|c| c.len_utf16() as u32).sum::<u32>();
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn len(s: &str) -> Self {
        let (line, off) = super::convert::count_lines(s);
        Self {
            line: line as _,
            col: s[off as usize..]
                .chars()
                .map(|c| char::len_utf16(c) as u32)
                .sum::<u32>(),
            __phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn from_other<P: StringPosition>(p: P, source: &str) -> Self {
        use PositionKind as K;
        unsafe {
            match P::KIND {
                K::None => Self::default(),
                K::Byte => {
                    super::convert::byte_to_linecol_utf16(std::mem::transmute_copy(&p), source)
                }
                K::Char => {
                    super::convert::char_to_linecol_utf16(std::mem::transmute_copy(&p), source)
                }
                K::Utf16 => {
                    super::convert::utf16_to_linecol_utf16(std::mem::transmute_copy(&p), source)
                }
                K::LineColUtf16 => std::mem::transmute_copy(&p),
                K::LineColByte => super::convert::linecol_to_linecol::<ByteOffset, _>(
                    std::mem::transmute_copy(&p),
                    source,
                ),
                K::LineColChar => super::convert::linecol_to_linecol::<CharOffset, _>(
                    std::mem::transmute_copy(&p),
                    source,
                ),
            }
        }
    }
}
