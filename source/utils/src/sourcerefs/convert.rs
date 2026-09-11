#![allow(clippy::cast_possible_truncation)]
use std::marker::PhantomData;

use super::{ByteOffset, CharOffset, LineCol, OffsetPosition, StringPosition, Utf16Offset};

pub fn char_to_byte(c: CharOffset, source: &str) -> ByteOffset {
    ByteOffset(source.chars().take(c.0).map(char::len_utf8).sum())
}

pub fn utf16_to_byte(c: Utf16Offset, source: &str) -> ByteOffset {
    let mut curr = 0;
    let mut ret = 0;
    for u in source.chars() {
        if curr >= c.0 {
            break;
        }
        curr += u.len_utf16();
        ret += u.len_utf8();
    }
    ByteOffset(ret)
}

pub fn linecolbyte_to_byte(c: LineCol<ByteOffset>, source: &str) -> ByteOffset {
    let off = skip_lines(source, c.line as _);
    ByteOffset(off + (c.col as usize))
}

pub fn linecolchar_to_byte(c: LineCol<CharOffset>, source: &str) -> ByteOffset {
    let off = skip_lines(source, c.line as _) as _;
    let source = &source[off..];
    ByteOffset(off) + char_to_byte(CharOffset(c.col as usize), source)
}

pub fn linecolutf16_to_byte(c: LineCol<Utf16Offset>, source: &str) -> ByteOffset {
    let off = skip_lines(source, c.line as _);
    let source = &source[off..];
    ByteOffset(off) + utf16_to_byte(Utf16Offset(c.col as _), source)
}

pub fn byte_to_char(c: ByteOffset, source: &str) -> CharOffset {
    CharOffset(source[..c.0].chars().count())
}

pub fn utf16_to_char(c: Utf16Offset, source: &str) -> CharOffset {
    let mut curr = 0;
    let mut ret = 0;
    for u in source.chars() {
        if curr >= c.0 {
            break;
        }
        curr += u.len_utf16();
        ret += 1;
    }
    CharOffset(ret)
}

pub fn linecolbyte_to_char(lc: LineCol<ByteOffset>, source: &str) -> CharOffset {
    // is it more efficient to iterate over chars directly?
    let off = skip_lines(source, lc.line as _);
    CharOffset(source[..off + (lc.col as usize)].chars().count())
}

pub fn linecolchar_to_char(lc: LineCol<CharOffset>, source: &str) -> CharOffset {
    // is it more efficient to iterate over chars directly?
    let off = skip_lines(source, lc.line as _);
    CharOffset(source[..off].chars().count() + (lc.col as usize))
}

pub fn linecolutf16_to_char(lc: LineCol<Utf16Offset>, source: &str) -> CharOffset {
    // is it more efficient to iterate over chars directly?
    let off = skip_lines(source, lc.line as _);
    let chars = source[..off].chars().count();
    let source = &source[off..];
    utf16_to_char(Utf16Offset(lc.col as _), source) + CharOffset(chars)
}

pub fn byte_to_utf16(c: ByteOffset, source: &str) -> Utf16Offset {
    Utf16Offset(source[..c.0].chars().map(char::len_utf16).sum())
}

pub fn char_to_utf16(c: CharOffset, source: &str) -> Utf16Offset {
    Utf16Offset(source.chars().take(c.0).map(char::len_utf16).sum())
}

pub fn linecolbyte_to_utf16(lc: LineCol<ByteOffset>, source: &str) -> Utf16Offset {
    // is it more efficient to iterate over chars directly?
    let off = skip_lines(source, lc.line as _);
    Utf16Offset(
        source[..off + (lc.col as usize)]
            .chars()
            .map(char::len_utf16)
            .sum(),
    )
}

pub fn linecolchar_to_utf16(lc: LineCol<CharOffset>, source: &str) -> Utf16Offset {
    // is it more efficient to iterate over chars directly?
    let off = skip_lines(source, lc.line as _);
    Utf16Offset(
        source[..off].chars().map(char::len_utf16).sum::<usize>()
            + source[off..]
                .chars()
                .take(lc.col as _)
                .map(char::len_utf16)
                .sum::<usize>(),
    )
}

pub fn linecolutf16_to_utf16(lc: LineCol<Utf16Offset>, source: &str) -> Utf16Offset {
    let off = skip_lines(source, lc.line as _);
    Utf16Offset(source[..off].chars().map(char::len_utf16).sum::<usize>() + (lc.col as usize))
}

// -----------------------------------------------------

pub fn byte_to_linecol_byte(c: ByteOffset, source: &str) -> LineCol<ByteOffset> {
    let source = &source[..c.0];
    let (lines, off) = count_lines(source);
    LineCol {
        line: lines as _,
        col: (source.len() - (off as usize)) as _,
        __phantom: PhantomData,
    }
}

pub fn char_to_linecol_byte(mut co: CharOffset, source: &str) -> LineCol<ByteOffset> {
    let mut bytes = 0;
    let mut line = 0;
    let mut char_iter = source.chars().peekable();
    while co.0 != 0
        && let Some(c) = char_iter.next()
    {
        co.0 -= 1;
        if c == '\n' {
            bytes = 0;
            line += 1;
        } else if c == '\r' {
            bytes = 0;
            line += 1;
            if char_iter.peek().copied() == Some('\n') {
                let _ = char_iter.next();
                co.0 -= 1;
            }
        } else {
            bytes += c.len_utf8();
        }
    }
    LineCol {
        line,
        col: bytes as _,
        __phantom: PhantomData,
    }
}

pub fn utf16_to_linecol_byte(mut co: Utf16Offset, source: &str) -> LineCol<ByteOffset> {
    let mut bytes = 0;
    let mut line = 0;
    let mut char_iter = source.chars().peekable();
    while co.0 != 0
        && let Some(c) = char_iter.next()
    {
        co.0 -= c.len_utf16();
        if c == '\n' {
            bytes = 0;
            line += 1;
        } else if c == '\r' {
            bytes = 0;
            line += 1;
            if char_iter.peek().copied() == Some('\n') {
                let _ = char_iter.next();
                co.0 -= 1;
            }
        } else {
            bytes += c.len_utf8();
        }
    }
    LineCol {
        line,
        col: bytes as _,
        __phantom: PhantomData,
    }
}

pub fn byte_to_linecol_char(c: ByteOffset, source: &str) -> LineCol<CharOffset> {
    let source = &source[..c.0];
    let (line, off) = count_lines(source);
    LineCol {
        line,
        col: source[off as usize..].chars().count() as _,
        __phantom: PhantomData,
    }
}

pub fn char_to_linecol_char(c: CharOffset, source: &str) -> LineCol<CharOffset> {
    let (line, bytes, total) = count_lines_chars(source, c.0);
    LineCol {
        line,
        col: source[total - bytes as usize..total].chars().count() as _,
        __phantom: PhantomData,
    }
}

pub fn utf16_to_linecol_char(c: Utf16Offset, source: &str) -> LineCol<CharOffset> {
    let (line, bytes, total) = count_lines_utf16(source, c.0);
    LineCol {
        line,
        col: source[total - bytes as usize..total].chars().count() as _,
        __phantom: PhantomData,
    }
}

pub fn byte_to_linecol_utf16(c: ByteOffset, source: &str) -> LineCol<Utf16Offset> {
    let source = &source[..c.0];
    let (line, offset) = count_lines(source);
    LineCol {
        line,
        col: source[offset as usize..]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum(),
        __phantom: PhantomData,
    }
}

pub fn char_to_linecol_utf16(c: CharOffset, source: &str) -> LineCol<Utf16Offset> {
    let (line, bytes, total) = count_lines_chars(source, c.0);
    LineCol {
        line,
        col: source[total - bytes as usize..total]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum(),
        __phantom: PhantomData,
    }
}

pub fn utf16_to_linecol_utf16(c: Utf16Offset, source: &str) -> LineCol<Utf16Offset> {
    let (line, bytes, total) = count_lines_utf16(source, c.0);
    LineCol {
        line,
        col: source[total - bytes as usize..total]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum(),

        __phantom: PhantomData,
    }
}

// -----------------------------------------------------

pub fn linecol_to_linecol<P1: OffsetPosition, P2: OffsetPosition>(
    lc: LineCol<P1>,
    source: &str,
) -> LineCol<P2>
where
    LineCol<P1>: StringPosition,
    LineCol<P2>: StringPosition,
{
    let zero = LineCol {
        line: lc.line,
        col: 0,
        __phantom: PhantomData,
    };
    let ByteOffset(offset) = linecolbyte_to_byte(zero, source);
    let source = &source[offset..];
    LineCol {
        line: lc.line,
        col: P2::len(source).into() as _,
        __phantom: PhantomData,
    }
}

// -----------------------------------------------------

// line, byte-offset after last line break,total-bytes
pub fn count_lines_chars(source: &str, mut chars: usize) -> (u32, u32, usize) {
    let mut bytes: u32 = 0;
    let mut line = 0;
    let mut total: usize = 0;
    let mut char_iter = source.chars().peekable();
    while chars != 0
        && let Some(c) = char_iter.next()
    {
        chars -= 1;
        if c == '\n' {
            total += (bytes + 1) as usize;
            bytes = 0;
            line += 1;
        } else if c == '\r' {
            total += (bytes + 1) as usize;
            bytes = 0;
            line += 1;
            if char_iter.peek().copied() == Some('\n') {
                total += 1;
                let _ = char_iter.next();
                chars -= 1;
            }
        } else {
            bytes += c.len_utf8() as u32;
        }
    }
    total += bytes as usize;
    (line, bytes, total)
}

// line, byte-offset after last line break,total-bytes
pub fn count_lines_utf16(source: &str, mut utfs: usize) -> (u32, u32, usize) {
    let mut bytes: u32 = 0;
    let mut line: u32 = 0;
    let mut total: usize = 0;
    let mut char_iter = source.chars().peekable();
    while utfs != 0
        && let Some(c) = char_iter.next()
    {
        utfs -= c.len_utf16();
        if c == '\n' {
            total += (bytes + 1) as usize;
            bytes = 0;
            line += 1;
        } else if c == '\r' {
            total += (bytes + 1) as usize;
            bytes = 0;
            line += 1;
            if char_iter.peek().copied() == Some('\n') {
                total += 1;
                let _ = char_iter.next();
                utfs -= 1;
            }
        } else {
            bytes += c.len_utf8() as u32;
        }
    }
    total += bytes as usize;
    (line, bytes, total)
}

// (lines,offset of last line start)
pub fn count_lines(source: &str) -> (u32, u32) {
    let source = source.as_bytes();
    let mut le_iter = memchr::memchr2_iter(b'\r', b'\n', source);
    let mut off = 0;
    let mut lines = 0;
    while let Some(i) = le_iter.next() {
        lines += 1;
        let first = source[i];
        if first == b'\r' && source.get(i + 1).copied() == Some(b'\n') {
            off = i + 2;
            let _ = le_iter.next();
        } else {
            off = i + 1;
        }
    }
    (lines, off as _)
}

pub fn skip_lines(source: &str, mut lines: u32) -> usize {
    let source = source.as_bytes();
    let mut le_iter = memchr::memchr2_iter(b'\r', b'\n', source);
    let mut off = 0;
    while lines > 0
        && let Some(i) = le_iter.next()
    {
        lines -= 1;
        let first = source[i];
        if first == b'\r' && source.get(i + 1).copied() == Some(b'\n') {
            off = i + 2;
            let _ = le_iter.next();
        } else {
            off = i + 1;
        }
    }
    off
}
