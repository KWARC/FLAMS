use crate::languages::Language;
use crate::uris::{ArchiveId, ArchiveURI, BaseURI, ModuleURI, Name, NameStep, SymbolURI};
use std::ops::{BitAnd, BitOr, Div, Not, Rem};

use super::name::{InvalidURICharacter, INVALID_CHARS};
use super::{DocumentElementURI, DocumentURI, NarrativeURI, PathURI};

impl<'a> Div<&'a str> for Name {
    type Output = Result<Self,InvalidURICharacter>;
    fn div(self, rhs: &'a str) -> Self::Output {
        if rhs.contains(INVALID_CHARS) {
            return Err(InvalidURICharacter);
        }
        let mut steps = self.0;
        if rhs.contains('/') {
            steps.extend(
                rhs.split('/')
                    .map(|s| NameStep(crate::uris::name::NAMES.lock().get_or_intern(s))),
            );
        } else {
            steps.push(NameStep(crate::uris::name::NAMES.lock().get_or_intern(rhs)));
        }
        Ok(Self(steps))
    }
}
impl Div<String> for Name {
    type Output = Result<Self,InvalidURICharacter>;
    #[inline]
    fn div(self, rhs: String) -> Self::Output {
        self / rhs.as_str()
    }
}
impl Div<NameStep> for Name {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: NameStep) -> Self::Output {
        self.0.push(rhs);
        self
    }
}
impl Div<Self> for Name {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: Self) -> Self::Output {
        self.0.extend(rhs.0);
        self
    }
}

impl BitAnd<ArchiveId> for BaseURI {
    type Output = crate::uris::ArchiveURI;
    #[inline]
    fn bitand(self, rhs: ArchiveId) -> Self::Output {
        crate::uris::ArchiveURI {
            base: self,
            archive: rhs,
        }
    }
}
impl BitAnd<&str> for BaseURI {
    type Output = ArchiveURI;
    #[inline]
    fn bitand(self, rhs: &str) -> Self::Output {
        <Self as BitAnd<ArchiveId>>::bitand(self, ArchiveId::new(rhs))
    }
}
impl BitOr<Name> for ArchiveURI {
    type Output = ModuleURI;
    #[inline]
    fn bitor(self, rhs: Name) -> Self::Output {
        ModuleURI {
            path: self.into(),
            name: rhs
        }
    }
}
impl BitOr<&str> for ArchiveURI {
    type Output = Result<ModuleURI,InvalidURICharacter>;
    #[inline]
    fn bitor(self, rhs: &str) -> Self::Output {
        Ok(<Self as BitOr<Name>>::bitor(self, rhs.parse()?))
    }
}

impl BitOr<Name> for ModuleURI {
    type Output = SymbolURI;
    #[inline]
    fn bitor(self, rhs: Name) -> Self::Output {
        SymbolURI {
            module: self,
            name: rhs,
        }
    }
}
impl BitOr<&str> for ModuleURI {
    type Output = Result<SymbolURI,InvalidURICharacter>;
    #[inline]
    fn bitor(self, rhs: &str) -> Self::Output {
        Ok(<Self as BitOr<Name>>::bitor(self, rhs.parse()?))
    }
}
impl BitAnd<Name> for ArchiveURI {
    type Output = DocumentURI;
    #[inline]
    fn bitand(self, rhs: Name) -> Self::Output {
        DocumentURI {
            path: self.into(),
            name: rhs,
            language: Language::default(),
        }
    }
}
impl BitAnd<&str> for ArchiveURI {
    type Output = Result<DocumentURI,InvalidURICharacter>;
    #[inline]
    fn bitand(self, rhs: &str) -> Self::Output {
        Ok(<Self as BitAnd<Name>>::bitand(self, rhs.parse()?))
    }
}

impl Rem<Name> for ArchiveURI {
    type Output = PathURI;
    #[inline]
    fn rem(self, rhs: Name) -> Self::Output {
        PathURI {
            archive: self,
            path: Some(rhs),
        }
    }
}

impl Rem<&str> for ArchiveURI {
    type Output = Result<PathURI,InvalidURICharacter>;
    #[inline]
    fn rem(self, rhs: &str) -> Self::Output {
        Ok(PathURI {
            archive: self,
            path: if rhs.is_empty() {
                None
            } else {
                Some(rhs.parse()?)
            },
        })
    }
}

impl Div<Name> for PathURI {
    type Output = Self;
    fn div(self, rhs: Name) -> Self::Output {
        Self {
            archive: self.archive,
            path: Some(if let Some(p) = self.path {
                p / rhs
            } else {rhs})
        }
    }
}


impl Div<&Name> for PathURI {
    type Output = Self;
    fn div(self, rhs: &Name) -> Self::Output {
        self / rhs.clone()
    }
}

impl Div<&str> for PathURI {
    type Output = Result<Self,InvalidURICharacter>;
    fn div(self, rhs: &str) -> Self::Output {
        if rhs.is_empty() {
            Ok(self)
        } else {
            Ok(self / rhs.parse::<Name>()?)
        }
    }
}

impl BitOr<Name> for PathURI {
    type Output = ModuleURI;
    #[inline]
    fn bitor(self, rhs: Name) -> Self::Output {
        ModuleURI {
            path: self,
            name: rhs
        }
    }
}
impl BitOr<&str> for PathURI {
    type Output = Result<ModuleURI,InvalidURICharacter>;
    #[inline]
    fn bitor(self, rhs: &str) -> Self::Output {
        Ok(<Self as BitOr<Name>>::bitor(self, rhs.parse()?))
    }
}

impl BitAnd<(Name, Language)> for ArchiveURI {
    type Output = DocumentURI;
    #[inline]
    fn bitand(self, rhs: (Name, Language)) -> Self::Output {
        DocumentURI {
            path: self.into(),
            name: rhs.0,
            language: rhs.1,
        }
    }
}
impl BitAnd<(&str, Language)> for ArchiveURI {
    type Output = Result<DocumentURI,InvalidURICharacter>;
    #[inline]
    fn bitand(self, rhs: (&str, Language)) -> Self::Output {
        Ok(<Self as BitAnd<(Name, Language)>>::bitand(self, (rhs.0.parse()?, rhs.1)))
    }
}
impl BitAnd<(Name, Language)> for PathURI {
    type Output = DocumentURI;
    #[inline]
    fn bitand(self, rhs: (Name, Language)) -> Self::Output {
        DocumentURI {
            path: self,
            name: rhs.0,
            language: rhs.1,
        }
    }
}
impl BitAnd<(&str, Language)> for PathURI {
    type Output = Result<DocumentURI,InvalidURICharacter>;
    #[inline]
    fn bitand(self, rhs: (&str, Language)) -> Self::Output {
        Ok(<Self as BitAnd<(Name, Language)>>::bitand(self, (rhs.0.parse()?, rhs.1)))
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl BitAnd<Name> for NarrativeURI {
    type Output = DocumentElementURI;
    #[inline]
    fn bitand(self,rhs:Name) -> Self::Output {
        match self {
            Self::Document(d) => DocumentElementURI {
                document: d,
                name: rhs
            },
            Self::Element(e) => e / rhs
        }
    }
}
impl BitAnd<&str> for NarrativeURI {
    type Output = Result<DocumentElementURI,InvalidURICharacter>;
    #[inline]
    fn bitand(self,rhs:&str) -> Self::Output {
        Ok(self & rhs.parse::<Name>()?)
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl BitAnd<Name> for DocumentURI {
    type Output = DocumentElementURI;
    #[inline]
    fn bitand(self,rhs:Name) -> Self::Output {
        DocumentElementURI {
            document: self,
            name: rhs
        }
    }
}
impl BitAnd<&str> for DocumentURI {
    type Output = Result<DocumentElementURI,InvalidURICharacter>;
    #[inline]
    fn bitand(self,rhs:&str) -> Self::Output {
        Ok(self & rhs.parse::<Name>()?)
    }
}

impl Not for ModuleURI {
    type Output = Self;
    #[inline]
    fn not(self) -> Self::Output {
        if self.name.is_simple() {
            return self;
        }
        let name = self.name.steps().first().unwrap_or_else(|| unreachable!());
        let name = name.clone().into();
        Self {
            path: self.path,
            name,
        }
    }
}


impl<'a> Div<&'a str> for ModuleURI {
    type Output = Result<Self,InvalidURICharacter>;
    fn div(self, rhs: &'a str) -> Self::Output {
        Ok(Self {
            path:self.path,
            name:self.name / rhs.parse::<Name>()?
        })
    }
}
impl Div<String> for ModuleURI {
    type Output = Result<Self,InvalidURICharacter>;
    #[inline]
    fn div(self, rhs: String) -> Self::Output {
        self / rhs.as_str()
    }
}
impl Div<NameStep> for ModuleURI {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: NameStep) -> Self::Output {
        self.name.0.push(rhs);
        self
    }
}
impl Div<Name> for ModuleURI {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: Name) -> Self::Output {
        self.name.0.extend(rhs.0);
        self
    }
}


impl<'a> Div<&'a str> for DocumentElementURI {
    type Output = Result<Self,InvalidURICharacter>;
    fn div(self, rhs: &'a str) -> Self::Output {
        Ok(Self {
            document:self.document,
            name:(self.name / rhs)?
        })
    }
}
impl Div<String> for DocumentElementURI {
    type Output = Result<Self,InvalidURICharacter>;
    #[inline]
    fn div(self, rhs: String) -> Self::Output {
        self / rhs.as_str()
    }
}
impl Div<NameStep> for DocumentElementURI {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: NameStep) -> Self::Output {
        self.name.0.push(rhs);
        self
    }
}
impl Div<Name> for DocumentElementURI {
    type Output = Self;
    #[inline]
    fn div(mut self, rhs: Name) -> Self::Output {
        self.name.0.extend(rhs.0);
        self
    }
}