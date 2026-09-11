use super::StringPosition;

/// INVARIANT: end >= start
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct StringRange<P: StringPosition> {
    pub start: P,
    pub end: P,
}
impl<P: StringPosition> std::fmt::Display for StringRange<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}--{}", self.start, self.end)
    }
}

impl<P: StringPosition> StringRange<P> {
    #[inline]
    pub fn contains(self, pos: P) -> bool {
        self.start <= pos && pos <= self.end
    }
}

#[allow(clippy::inline_always)]
impl<P: StringPosition> PartialEq<P> for StringRange<P> {
    #[inline(always)]
    fn eq(&self, other: &P) -> bool {
        self.contains(*other)
    }
}
impl<P: StringPosition> PartialOrd<P> for StringRange<P> {
    fn partial_cmp(&self, other: &P) -> Option<std::cmp::Ordering> {
        if self.end <= *other {
            Some(std::cmp::Ordering::Less)
        } else if self.start >= *other {
            Some(std::cmp::Ordering::Greater)
        } else if self.start == self.end && self.start == *other {
            Some(std::cmp::Ordering::Equal)
        } else {
            None
        }
    }
}

impl<P: StringPosition> StringRange<P> {
    #[allow(clippy::inline_always)]
    #[inline(always)]
    #[must_use]
    pub fn from_other<O: StringPosition>(other: StringRange<O>, source: &str) -> Self {
        let mut conv = P::from_many(source);
        Self {
            start: conv.next(other.start),
            end: conv.next(other.end),
        }
    }
    #[allow(clippy::inline_always)]
    #[inline(always)]
    #[must_use]
    pub fn into_other<O: StringPosition>(self, source: &str) -> StringRange<O> {
        StringRange::from_other(self, source)
    }
}

impl<P: StringPosition> StringRange<P> {
    #[must_use]
    pub const fn until(self, rhs: P) -> Self {
        Self {
            start: self.start,
            end: rhs,
        }
    }
}
