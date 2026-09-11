#![allow(clippy::inline_always)]

use super::{LineCol, PositionKind, StringPosition};

pub trait OffsetPosition: StringPosition + From<usize> + Into<usize>
where
    LineCol<Self>: StringPosition,
{
    fn offset(self) -> usize;
}

/// Number of bytes before this position
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct ByteOffset(pub usize);
impl crate::__private::Sealed for ByteOffset {}
impl std::ops::Add for ByteOffset {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl std::ops::AddAssign for ByteOffset {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl std::ops::Sub for ByteOffset {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}
impl std::ops::SubAssign for ByteOffset {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0);
    }
}
impl std::fmt::Display for ByteOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl StringPosition for ByteOffset {
    const KIND: PositionKind = PositionKind::Byte;
    #[inline(always)]
    fn len(s: &str) -> Self {
        Self(s.len())
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn inc_by(&mut self, c: char) {
        self.0 += c.len_utf8();
    }
    fn inc_newline(&mut self, crlf: bool) {
        if crlf {
            self.0 += 2;
        } else {
            self.0 += 1;
        }
    }

    #[inline]
    fn inc_offset_by(&mut self, text: &str) {
        self.0 += text.len();
    }

    #[inline(always)]
    fn from_other<P: StringPosition>(p: P, source: &str) -> Self {
        use PositionKind as K;
        unsafe {
            match P::KIND {
                K::None => Self::default(),
                K::Byte => std::mem::transmute_copy(&p),
                K::Char => super::convert::char_to_byte(std::mem::transmute_copy(&p), source),
                K::Utf16 => super::convert::utf16_to_byte(std::mem::transmute_copy(&p), source),
                K::LineColByte => {
                    super::convert::linecolbyte_to_byte(std::mem::transmute_copy(&p), source)
                }
                K::LineColChar => {
                    super::convert::linecolchar_to_byte(std::mem::transmute_copy(&p), source)
                }
                K::LineColUtf16 => {
                    super::convert::linecolutf16_to_byte(std::mem::transmute_copy(&p), source)
                }
            }
        }
    }
}
impl From<usize> for ByteOffset {
    #[inline(always)]
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<ByteOffset> for usize {
    #[inline(always)]
    fn from(value: ByteOffset) -> Self {
        value.0
    }
}
impl OffsetPosition for ByteOffset {
    #[inline(always)]
    fn offset(self) -> usize {
        self.0
    }
}

/// Number of characters before this position
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct CharOffset(pub usize);
impl crate::__private::Sealed for CharOffset {}
impl std::ops::Add for CharOffset {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl std::ops::AddAssign for CharOffset {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl std::ops::Sub for CharOffset {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}
impl std::ops::SubAssign for CharOffset {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0);
    }
}
impl std::fmt::Display for CharOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl StringPosition for CharOffset {
    const KIND: PositionKind = PositionKind::Char;

    #[inline]
    fn len(s: &str) -> Self {
        Self(s.chars().count())
    }

    #[inline]
    fn inc_by(&mut self, _: char) {
        self.0 += 1;
    }
    #[inline]
    fn inc_offset_by(&mut self, text: &str) {
        self.0 += text.chars().count();
    }

    fn inc_newline(&mut self, crlf: bool) {
        if crlf {
            self.0 += 2;
        } else {
            self.0 += 1;
        }
    }

    #[inline(always)]
    fn from_other<P: StringPosition>(p: P, source: &str) -> Self {
        use PositionKind as K;
        unsafe {
            match P::KIND {
                K::None => Self::default(),
                K::Char => std::mem::transmute_copy(&p),
                K::Byte => super::convert::byte_to_char(std::mem::transmute_copy(&p), source),
                K::Utf16 => super::convert::utf16_to_char(std::mem::transmute_copy(&p), source),
                K::LineColByte => {
                    super::convert::linecolbyte_to_char(std::mem::transmute_copy(&p), source)
                }
                K::LineColChar => {
                    super::convert::linecolchar_to_char(std::mem::transmute_copy(&p), source)
                }
                K::LineColUtf16 => {
                    super::convert::linecolutf16_to_char(std::mem::transmute_copy(&p), source)
                }
            }
        }
    }
}
impl From<usize> for CharOffset {
    #[inline(always)]
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<CharOffset> for usize {
    #[inline(always)]
    fn from(value: CharOffset) -> Self {
        value.0
    }
}
impl OffsetPosition for CharOffset {
    #[inline(always)]
    fn offset(self) -> usize {
        self.0
    }
}

/// Number of UTF-16 codepoints before this position
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Utf16Offset(pub usize);
impl crate::__private::Sealed for Utf16Offset {}
impl std::ops::Add for Utf16Offset {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl std::ops::AddAssign for Utf16Offset {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl std::ops::Sub for Utf16Offset {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}
impl std::ops::SubAssign for Utf16Offset {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0);
    }
}
impl std::fmt::Display for Utf16Offset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl StringPosition for Utf16Offset {
    const KIND: PositionKind = PositionKind::Utf16;

    #[inline]
    fn len(s: &str) -> Self {
        Self(s.chars().map(char::len_utf16).sum())
    }

    #[inline]
    fn inc_by(&mut self, c: char) {
        self.0 += c.len_utf16();
    }

    #[inline]
    fn inc_offset_by(&mut self, text: &str) {
        self.0 += text.chars().map(char::len_utf16).sum::<usize>();
    }

    fn inc_newline(&mut self, crlf: bool) {
        if crlf {
            self.0 += 2;
        } else {
            self.0 += 1;
        }
    }

    #[inline(always)]
    fn from_other<P: StringPosition>(p: P, source: &str) -> Self {
        use PositionKind as K;
        unsafe {
            match P::KIND {
                K::None => Self::default(),
                K::Utf16 => std::mem::transmute_copy(&p),
                K::Byte => super::convert::byte_to_utf16(std::mem::transmute_copy(&p), source),
                K::Char => super::convert::char_to_utf16(std::mem::transmute_copy(&p), source),
                K::LineColByte => {
                    super::convert::linecolbyte_to_utf16(std::mem::transmute_copy(&p), source)
                }
                K::LineColChar => {
                    super::convert::linecolchar_to_utf16(std::mem::transmute_copy(&p), source)
                }
                K::LineColUtf16 => {
                    super::convert::linecolutf16_to_utf16(std::mem::transmute_copy(&p), source)
                }
            }
        }
    }
}

impl From<usize> for Utf16Offset {
    #[inline(always)]
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<Utf16Offset> for usize {
    #[inline(always)]
    fn from(value: Utf16Offset) -> Self {
        value.0
    }
}
impl OffsetPosition for Utf16Offset {
    #[inline(always)]
    fn offset(self) -> usize {
        self.0
    }
}
