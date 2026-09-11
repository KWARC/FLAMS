//#![feature(ptr_as_ref_unchecked)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod binary;
#[cfg(feature = "async")]
pub mod change_listener;
pub mod escaping;
pub mod gc;
pub mod globals;
pub mod id_counters;
mod inner_arc;
pub mod logs;
pub mod parsing;
//pub mod regex;
pub mod settings;
pub mod sourcerefs;
//pub mod time;
mod treelike;
pub mod vecmap;
//pub mod file_id;

use std::hint::unreachable_unchecked;

pub use parking_lot;
pub use triomphe;

pub mod prelude {
    pub use super::vecmap::{VecMap, VecSet};
    pub type HMap<K, V> = rustc_hash::FxHashMap<K, V>;
    pub type HSet<V> = rustc_hash::FxHashSet<V>;
    pub use crate::inner_arc::InnerArc;
    pub use crate::treelike::*;
}

pub(crate) mod __private {
    pub trait Sealed {}
}

pub trait CharExt {
    fn into_bytes(self) -> arrayvec::ArrayVec<u8, 4>;
}

impl CharExt for char {
    fn into_bytes(self) -> arrayvec::ArrayVec<u8, 4> {
        const TAG_CONT: u8 = 0b1000_0000;
        const TAG_TWO_B: u8 = 0b1100_0000;
        const TAG_THREE_B: u8 = 0b1110_0000;
        const TAG_FOUR_B: u8 = 0b1111_0000;
        let num = self as u32;
        let mut ret = arrayvec::ArrayVec::new();

        // SAFETY: character length is between 1 and 4
        unsafe {
            match self.len_utf8() {
                1 => {
                    #[allow(clippy::cast_possible_truncation)]
                    ret.push_unchecked(num as u8);
                }
                2 => {
                    ret.push_unchecked((num >> 6 & 0x1F) as u8 | TAG_TWO_B);
                    ret.push_unchecked((num & 0x3F) as u8 | TAG_CONT);
                }
                3 => {
                    ret.push_unchecked((num >> 12 & 0x0F) as u8 | TAG_THREE_B);
                    ret.push_unchecked((num >> 6 & 0x3F) as u8 | TAG_CONT);
                    ret.push_unchecked((num & 0x3F) as u8 | TAG_CONT);
                }
                4 => {
                    ret = [
                        (num >> 18 & 0x07) as u8 | TAG_FOUR_B,
                        (num >> 12 & 0x3F) as u8 | TAG_CONT,
                        (num >> 6 & 0x3F) as u8 | TAG_CONT,
                        (num & 0x3F) as u8 | TAG_CONT,
                    ]
                    .into();
                }
                _ => unreachable_unchecked(),
            }
        }
        ret
    }
}

#[cfg(target_family = "wasm")]
type Str = String;
#[cfg(not(target_family = "wasm"))]
type Str = Box<str>;

pub fn hashstr<A: std::hash::Hash>(prefix: &str, a: &A) -> String {
    use std::hash::BuildHasher;
    let h = rustc_hash::FxBuildHasher.hash_one(a);
    format!("{prefix}{h:02x}")
}

#[cfg(feature = "tokio")]
pub fn background<F: FnOnce() + Send + 'static>(f: F) {
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || span.in_scope(f));
}

pub fn in_span<F: FnOnce() -> R, R>(f: F) -> impl FnOnce() -> R {
    let span = tracing::Span::current();
    move || {
        let _span = span.enter();
        f()
    }
}
/*
#[cfg(feature = "serde")]
pub trait Hexable: Sized {
    /// #### Errors
    fn as_hex(&self) -> eyre::Result<String>;
    /// #### Errors
    fn from_hex(s: &str) -> eyre::Result<Self>;
}
#[cfg(feature = "serde")]
impl<T: Sized + serde::Serialize + for<'de> serde::Deserialize<'de>> Hexable for T {
    fn as_hex(&self) -> eyre::Result<String> {
        use std::fmt::Write;
        let bc = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        let mut ret = String::with_capacity(bc.len() * 2);
        for b in bc {
            write!(ret, "{b:02X}")?;
        }
        Ok(ret)
    }
    fn from_hex(s: &str) -> eyre::Result<Self> {
        let bytes: Result<Vec<_>, _> = if s.len().is_multiple_of(2) {
            (0..s.len())
                .step_by(2)
                .filter_map(|i| s.get(i..i + 2))
                .map(|sub| u8::from_str_radix(sub, 16))
                .collect()
        } else {
            return Err(eyre::eyre!("Incompatible string length"));
        };
        bincode::serde::decode_from_slice(&bytes?, bincode::config::standard())
            .map(|(r, _)| r)
            .map_err(Into::into)
    }
}
*/
pub mod fs {
    use std::path::Path;

    use eyre::Context;

    /// #### Errors
    pub fn copy_dir_all(src: &Path, dst: &Path) -> eyre::Result<()> {
        std::fs::create_dir_all(dst)
            .wrap_err_with(|| format!("Error creating {}", dst.display()))?;
        for entry in
            std::fs::read_dir(src).wrap_err_with(|| format!("Error reading {}", src.display()))?
        {
            let entry = entry
                .wrap_err_with(|| format!("Error getting file entry for {}", src.display()))?;
            let ty = entry.file_type().wrap_err_with(|| {
                format!("Error determining file type of {}", entry.path().display())
            })?;
            let target = dst.join(entry.file_name());
            if ty.is_dir() {
                copy_dir_all(&entry.path(), &target)?;
            } else {
                let md = entry.metadata().wrap_err_with(|| {
                    format!("Error obtaining metatada for {}", entry.path().display())
                })?;
                std::fs::copy(entry.path(), &target).wrap_err_with(|| {
                    format!(
                        "Error copying {} to {}",
                        entry.path().display(),
                        target.display()
                    )
                })?;
                let mtime = filetime::FileTime::from_last_modification_time(&md);
                filetime::set_file_mtime(&target, mtime).wrap_err_with(|| {
                    format!(
                        "Error setting file modification time for {}",
                        target.display()
                    )
                })?;
            }
        }
        Ok(())
    }
}
/*
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CSS {
    Link(#[cfg_attr(feature = "wasm", tsify(type = "string"))] Str),
    Inline(#[cfg_attr(feature = "wasm", tsify(type = "string"))] Str),
    Class {
        #[cfg_attr(feature = "wasm", tsify(type = "string"))]
        name: Str,
        #[cfg_attr(feature = "wasm", tsify(type = "string"))]
        css: Str,
    },
}
impl PartialOrd for CSS {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CSS {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn classnum(s: &str) -> u8 {
            match s {
                s if s.starts_with("ftml-subproblem") => 1,
                s if s.starts_with("ftml-problem") => 2,
                s if s.starts_with("ftml-example") => 3,
                s if s.starts_with("ftml-definition") => 4,
                s if s.starts_with("ftml-paragraph") => 5,
                "ftml-subsubsection" => 6,
                "ftml-subsection" => 7,
                "ftml-section" => 8,
                "ftml-chapter" => 9,
                "ftml-part" => 10,
                _ => 0,
            }
        }
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Link(l1), Self::Link(l2)) | (Self::Inline(l1), Self::Inline(l2)) => l1.cmp(l2),
            (Self::Link(_), Self::Inline(_))
            | (Self::Link(_) | Self::Inline(_), Self::Class { .. }) => Ordering::Less,
            (Self::Inline(_), Self::Link(_))
            | (Self::Class { .. }, Self::Inline(_) | Self::Link(_)) => Ordering::Greater,
            (Self::Class { name: n1, css: c1 }, Self::Class { name: n2, css: c2 }) => {
                (classnum(n1), n1, c1).cmp(&(classnum(n2), n2, c2))
            }
        }
    }
}
impl CSS {
    pub fn merge(v: Vec<Self>) -> Vec<Self> {
        use lightningcss::traits::ToCss;
        use lightningcss::{
            printer::PrinterOptions,
            rules::{CssRule, CssRuleList},
            selector::Component,
            stylesheet::{MinifyOptions, ParserOptions, StyleSheet},
        };

        let mut links = Vec::new();
        let mut strings = Vec::new();
        for c in v {
            match c {
                Self::Link(_) => links.push(c),
                Self::Inline(css) | Self::Class { css, .. } => strings.push(css),
            }
        }

        let mut sheet = StyleSheet::new(
            Vec::new(),
            CssRuleList(Vec::new()),
            ParserOptions::default(),
        );
        let inlines = smallvec::SmallVec::<_, 2>::new();
        for s in &strings {
            if let Ok(rs) = StyleSheet::parse(s, ParserOptions::default()) {
                sheet.rules.0.extend(rs.rules.0.into_iter());
            } else {
                tracing::warn!("Not class-able: {s}");
            }
        }
        let _ = sheet.minify(MinifyOptions::default());

        let mut classes = Vec::new();
        for rule in std::mem::take(&mut sheet.rules.0) {
            match rule {
                CssRule::Style(style) => {
                    if style.vendor_prefix.is_empty()
                        && style.selectors.0.len() == 1
                        && style.selectors.0[0].len() == 1
                        && matches!(
                            style.selectors.0[0].iter().next(),
                            Some(Component::Class(_))
                        )
                    {
                        let Some(Component::Class(class_name)) = style.selectors.0[0].iter().next()
                        else {
                            impossible!()
                        };
                        if let Ok(s) = style.to_css_string(PrinterOptions::default()) {
                            classes.push(Self::Class {
                                name: class_name.to_string().into(),
                                css: s.into(),
                            });
                        } else {
                            tracing::warn!("Illegal CSS: {style:?}");
                        }
                    } else {
                        if let Ok(s) = style.to_css_string(PrinterOptions::default()) {
                            tracing::warn!("Not class-able: {s}");
                            links.push(Self::Inline(s.into()));
                        } else {
                            tracing::warn!("Illegal CSS: {style:?}");
                        }
                    }
                }
                rule => {
                    if let Ok(s) = rule.to_css_string(PrinterOptions::default()) {
                        tracing::warn!("Not class-able: {s}");
                        links.push(Self::Inline(s.into()));
                    } else {
                        tracing::warn!("Illegal CSS: {rule:?}");
                    }
                }
            }
        }
        drop(sheet);

        links.extend(inlines.into_iter().map(|i| Self::Inline(strings.remove(i))));
        links.extend(classes);
        links
    }

    #[must_use]
    pub fn split(css: &str) -> Vec<Self> {
        use lightningcss::traits::ToCss;
        use lightningcss::{
            printer::PrinterOptions,
            rules::CssRule,
            selector::Component,
            stylesheet::{ParserOptions, StyleSheet},
        };
        let Ok(ruleset) = StyleSheet::parse(css, ParserOptions::default()) else {
            tracing::warn!("Not class-able: {css}");
            return vec![Self::Inline(css.to_string().into())];
        };
        if ruleset.sources.iter().any(|s| !s.is_empty()) {
            tracing::warn!("Not class-able: {css}");
            return vec![Self::Inline(css.to_string().into())];
        }
        ruleset
            .rules
            .0
            .into_iter()
            .filter_map(|rule| match rule {
                CssRule::Style(style) => {
                    if style.vendor_prefix.is_empty()
                        && style.selectors.0.len() == 1
                        && style.selectors.0[0].len() == 1
                        && matches!(
                            style.selectors.0[0].iter().next(),
                            Some(Component::Class(_))
                        )
                    {
                        let Some(Component::Class(class_name)) = style.selectors.0[0].iter().next()
                        else {
                            impossible!()
                        };
                        style
                            .to_css_string(PrinterOptions::default())
                            .ok()
                            .map(|s| Self::Class {
                                name: class_name.to_string().into(),
                                css: s.into(),
                            })
                    } else {
                        style
                            .to_css_string(PrinterOptions::default())
                            .ok()
                            .map(|s| {
                                tracing::warn!("Not class-able: {s}");
                                Self::Inline(s.into())
                            })
                    }
                }
                o => o.to_css_string(PrinterOptions::default()).ok().map(|s| {
                    tracing::warn!("Not class-able: {s}");
                    Self::Inline(s.into())
                }),
            })
            .collect()
    }
}
 */

#[macro_export]
macro_rules! impossible {
    () => {{
        #[cfg(debug_assertions)]
        {
            unreachable!()
        }
        #[cfg(not(debug_assertions))]
        {
            unsafe { std::hint::unreachable_unchecked() }
        }
    }};
    ($s:literal) => {
        #[cfg(debug_assertions)]
        {
            panic!($s)
        }
        #[cfg(not(debug_assertions))]
        {
            unsafe { std::hint::unreachable_unchecked() }
        }
    };
    (?) => {
        unreachable!()
    };
    (? $s:literal) => {{ panic!($s) }};
}

#[macro_export]
macro_rules! unwrap {
    ($e: expr) => { $e.unwrap_or_else(|| {$crate::impossible!();}) };
    (? $e: expr) => { $e.unwrap_or_else(|| {$crate::impossible!(?);}) };
    ($e: expr;$l:literal) => { $e.unwrap_or_else(|| {$crate::impossible!($l);}) };
    (? $e: expr;$l:literal) => { $e.unwrap_or_else(|| {$crate::impossible!(? $l);}) };
}

#[cfg(feature = "serde")]
pub trait CondSerialize: serde::Serialize {}
#[cfg(feature = "serde")]
impl<T: serde::Serialize> CondSerialize for T {}

#[cfg(not(feature = "serde"))]
pub trait CondSerialize {}
#[cfg(not(feature = "serde"))]
impl<T> CondSerialize for T {}

/*
#[allow(clippy::unwrap_used)]
#[allow(clippy::cognitive_complexity)]
#[allow(clippy::similar_names)]
#[test]
fn css_things() {
    use lightningcss::traits::ToCss;
    use lightningcss::{
        printer::PrinterOptions,
        rules::CssRule,
        selector::Component,
        stylesheet::{MinifyOptions, ParserOptions, StyleSheet},
    };
    tracing_subscriber::fmt().init();
    let css = include_str!("../../../resources/assets/rustex.css");
    let rules = StyleSheet::parse(css, ParserOptions::default()).unwrap();
    let roundtrip = rules.to_css(PrinterOptions::default()).unwrap();
    tracing::info!("{}", roundtrip.code);
    let test = "
        .ftml-paragraph {
            > .ftml-title {
                font-weight: bold;
            }
            margin: 0;
        }
    ";
    let mut ruleset = StyleSheet::parse(test, ParserOptions::default()).unwrap();
    ruleset.minify(MinifyOptions::default()).unwrap();
    assert!(ruleset.sources.iter().all(String::is_empty));
    tracing::info!("Result: {ruleset:#?}");
    for rule in ruleset.rules.0 {
        match rule {
            CssRule::Style(style) => {
                assert!(style.vendor_prefix.is_empty());
                assert!(style.selectors.0.len() == 1);
                assert!(style.selectors.0[0].len() == 1);
                tracing::info!(
                    "Here: {}",
                    style.to_css_string(PrinterOptions::default()).unwrap()
                );
                let sel = style.selectors.0[0].iter().next().unwrap();
                assert!(matches!(sel, Component::Class(_)));
                let Component::Class(cls) = sel else {
                    impossible!()
                };
                let cls_str = &**cls;
                tracing::info!("Class: {cls_str}");
            }
            o => panic!("Unexpected rule: {o:#?}"),
        }
    }
}

pub trait PathExt {
    const PATH_SEPARATOR: char;
    fn as_slash_str(&self) -> String;
    fn same_fs_as<P: AsRef<std::path::Path>>(&self, other: &P) -> bool;
    fn rename_safe<P: AsRef<std::path::Path>>(&self, target: &P) -> eyre::Result<()>;
}
impl<T: AsRef<std::path::Path>> PathExt for T {
    #[cfg(target_os = "windows")]
    const PATH_SEPARATOR: char = '\\';
    #[cfg(not(target_os = "windows"))]
    const PATH_SEPARATOR: char = '/';
    fn as_slash_str(&self) -> String {
        if cfg!(windows) {
            unwrap!(self.as_ref().as_os_str().to_str()).replace('\\', "/")
        } else {
            unwrap!(self.as_ref().as_os_str().to_str()).to_string()
        }
    }
    #[cfg(target_os = "windows")]
    fn same_fs_as<P: AsRef<std::path::Path>>(&self, other: &P) -> bool {
        let Some(p1) = self
            .as_ref()
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
        else {
            return false;
        };
        let Some(p2) = other
            .as_ref()
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
        else {
            return false;
        };
        p1 == p2
    }
    #[cfg(target_arch = "wasm32")]
    fn same_fs_as<P: AsRef<std::path::Path>>(&self, other: &P) -> bool {
        impossible!()
    }

    #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
    fn same_fs_as<P: AsRef<std::path::Path>>(&self, other: &P) -> bool {
        use std::os::unix::fs::MetadataExt;
        fn existent_parent(p: &std::path::Path) -> &std::path::Path {
            if p.exists() {
                return p;
            }
            existent_parent(p.parent().unwrap_or_else(|| unreachable!()))
        }
        let p1 = existent_parent(self.as_ref());
        let p2 = existent_parent(other.as_ref());
        let md1 = p1.metadata().unwrap_or_else(|_| unreachable!());
        let md2 = p2.metadata().unwrap_or_else(|_| unreachable!());
        md1.dev() == md2.dev()
    }
    fn rename_safe<P: AsRef<std::path::Path>>(&self, target: &P) -> eyre::Result<()> {
        Ok(if self.same_fs_as(target) {
            std::fs::rename(self.as_ref(), target.as_ref())?
        } else {
            crate::fs::copy_dir_all(self.as_ref(), target.as_ref())?
        })
    }
}
 */
