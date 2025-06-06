use crate::uris::errors::URIParseError;
use crate::uris::macros::debugdisplay;
use crate::uris::{sealed, URIOrRefTrait, URIRef, URIRefTrait, URITrait, URI};
use either::Either;
use flams_utils::gc::{TArcInterned, TArcInterner};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::fmt::Display;
use std::str::{FromStr, Split};
use triomphe::Arc;

lazy_static! {
    static ref BASE_URIS: Arc<Mutex<TArcInterner<str, 4, 100>>> =
        Arc::new(Mutex::new(TArcInterner::default()));
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BaseURI(TArcInterned<str>);
impl BaseURI {
    #[must_use]
    #[inline]
    pub fn new_unchecked(s: &str) -> Self {
        Self(BASE_URIS.lock().get_or_intern(s))
    }
    /// ### Errors
    /// Returns an error if the string is not a valid URL, or has a query or fragment.
    pub fn new_checked(s: &str) -> Result<Self, url::ParseError> {
        use url::{ParseError, Url};
        let url = Url::parse(s)?;
        if url.cannot_be_a_base() {
            return Err(ParseError::RelativeUrlWithoutBase);
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(ParseError::RelativeUrlWithoutBase);
        }
        Ok(Self::new_unchecked(s))
    }
    fn new_checked_partially(s: &str) -> Result<Self, url::ParseError> {
        use url::{ParseError, Url};
        let url = Url::parse(s)?;
        if url.cannot_be_a_base() {
            return Err(ParseError::RelativeUrlWithoutBase);
        }
        Ok(Self::new_unchecked(s))
    }

    pub(super) fn pre_parse(s: &str) -> Result<Either<Self, (Self, Split<char>)>, URIParseError> {
        #[inline]
        fn do_base(s: &str) -> Result<BaseURI, URIParseError> {
            Ok(BaseURI::new_checked_partially(s)?)
        }

        let Some((base, rest)) = s.split_once('?') else {
            return do_base(s).map(Either::Left);
        };
        let base = do_base(base)?;
        Ok(if rest.is_empty() {
            Either::Left(base)
        } else {
            Either::Right((base, rest.split('&')))
        })
    }
}
impl AsRef<str> for BaseURI {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl Display for BaseURI {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}
debugdisplay!(BaseURI);
impl URIOrRefTrait for BaseURI {
    #[inline]
    fn base(&self) -> &Self {
        self
    }
    fn as_uri(&self) -> URIRef {
        URIRef::Base(self)
    }
}
impl URITrait for BaseURI {
    type Ref<'a> = &'a Self;
}
impl<'a> URIRefTrait<'a> for &'a BaseURI {
    type Owned = BaseURI;
    #[inline]
    fn owned(self) -> BaseURI {
        self.clone()
    }
}

impl FromStr for BaseURI {
    type Err = URIParseError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new_checked(s).map_err(Into::into)
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::BaseURI;
    use crate::uris::serialize;
    serialize!(as BaseURI);
    impl<'de> serde::Deserialize<'de> for BaseURI {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let s = String::deserialize(deserializer)?;
            Self::new_checked(&s).map_err(serde::de::Error::custom)
        }
    }
}
