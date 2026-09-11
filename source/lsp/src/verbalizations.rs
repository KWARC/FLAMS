use std::{borrow::Cow};

use flams_utils::{
    CharExt, parsing::{SourceParser, StrParser}, sourcerefs::{StringPosition, StringRange},
};
use ftml_uris::{Language, SymbolUri};
use radix_trie::NibbleVec;
use smallvec::SmallVec;


struct StackElem<P: StringPosition> {
    token:Token<P>,
    merged: Option<(Verbalization, bool)>,
}

struct Token<P:StringPosition> {
    verb: Verbalization,
    range: StringRange<P>,
    byte_range:(usize,usize),
    separator: Option<char>,
    has_singleton: bool,
}

struct Tokenizer<'s,P:StringPosition> {
    inner:StrParser<'s,P>,
    string:&'s str,
    prefix:Vec<Token<P>>,
    off:usize,
    trie:&'s radix_trie::Trie<Verbalization,SmallVec<SymbolUri,1>>,
    language:Language
}
impl<P:StringPosition> Iterator for Tokenizer<'_,P> {
    type Item = Token<P>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.prefix.pop() {
            return Some(next)
        }
        let curr_len = self.inner.rest().len();
        self.inner.trim_start();
        if self.inner.rest().is_empty() {
            return None
        }
        let start = self.inner.pos;
        let start_byte = self.off + curr_len - self.inner.rest().len();
        let next = self.inner.read_until(|c| Verbalization::BREAKS.contains(&c));
        let range = StringRange { start, end:self.inner.pos };
        let end_byte = self.off + curr_len - self.inner.rest().len();
        self.off = end_byte;
        let separator = self.inner.next_char();
        if let Some(sep) = separator {
            self.off += sep.len_utf8();
        }
        let verb = Verbalization::new(next, self.language);
        let has_singleton = self.trie.get(&verb).is_some();
        Some(Token { verb, range, byte_range:(start_byte,end_byte),separator, has_singleton })
    }
}

#[derive(Clone, Debug, Default)]
pub struct VerbalizationTrie(
    pub(crate) std::sync::Arc<parking_lot::Mutex<radix_trie::Trie<Verbalization, SmallVec<SymbolUri, 1>>>>,
);
pub struct UnlockedVerbalizationTrie<'a>(parking_lot::MutexGuard<'a,radix_trie::Trie<Verbalization, SmallVec<SymbolUri, 1>>>);
impl UnlockedVerbalizationTrie<'_> {
    pub fn insert(&mut self, language: Language, text: &str, uri: &SymbolUri) {
        if text.len() <= 3 {return;}
        let stem = Verbalization::new(text, language);
        if let Some(e) = self.0.get_mut(&stem) {
            if !e.contains(uri) {
                e.push(uri.clone());
            }
        } else {
            self.0.insert(stem, smallvec::smallvec_inline![uri.clone()]);
        }
    }
    fn clear<P:StringPosition>(stack:&mut Vec<StackElem<P>>,ret:&mut Vec<(StringRange<P>, SmallVec<(SymbolUri,bool), 1>)>,
        needs_usemodule: &dyn Fn(&SymbolUri) -> bool,
        tokenizer:&mut Tokenizer<P>,
        ignores:&[&str]
    ) {
            if stack.is_empty() {
                return;
            }
            if let Some((i,_)) = stack.iter().enumerate().rev().find(|(_,e)| {
                e.merged.as_ref().is_some_and(|(_,can_merge)| *can_merge)
            }) {
                let new_stack = stack.split_off(i+1);
                let mut initial = std::mem::replace(stack, new_stack);
                let last = initial.pop().expect("bug");
                let Some((final_verb,_)) = last.merged else {
                    unreachable!("bug");
                };
                let first = initial.first().expect("bug");
                let start = first.token.range.start;
                let end = last.token.range.end;
                let text_str = &tokenizer.string[first.token.byte_range.0..last.token.byte_range.1];
                //tracing::warn!("Success: {:?}",final_verb.as_ref());
                if !ignores.iter().any(|s| s.eq_ignore_ascii_case(text_str)) {
                    let uris = tokenizer.trie.get(&final_verb).expect("bug").iter().map(|s| (s.clone(),needs_usemodule(s))).collect();
                    ret.push((StringRange{start,end},uris));
                }
                tokenizer.prefix.extend(std::mem::take(stack).into_iter().rev().map(|se| se.token));
            } else {
                let mut stack_iter = std::mem::take(stack).into_iter();
                let first = stack_iter.next().expect("bug");
                if first.token.has_singleton {
                    let text_str = &tokenizer.string[first.token.byte_range.0..first.token.byte_range.1];
                    if !ignores.iter().any(|s| s.eq_ignore_ascii_case(text_str)) {
                        let uris = tokenizer.trie.get(&first.token.verb).expect("bug").iter().map(|s| (s.clone(),needs_usemodule(s))).collect();
                        //tracing::warn!("Success: {:?}",first.verb.as_ref());
                        ret.push((first.token.range,uris));
                    }
                } else {
                    //tracing::warn!("Dropping: {:?}",first.verb.as_ref());
                }
                tokenizer.prefix.extend(stack_iter.rev().map(|se| se.token));
            }
        }

    pub fn find_all_in<P: StringPosition>(
        &self,
        language: Language,
        text: &str,
        start_pos:P,
        ignores:&[&str],
        needs_usemodule: &dyn Fn(&SymbolUri) -> bool
    ) -> Vec<(StringRange<P>, SmallVec<(SymbolUri,bool), 1>)> {
        use radix_trie::TrieCommon;
        //tracing::warn!("Here: {:?}",text.rest());

        let mut stack: Vec<StackElem<P>> = Vec::with_capacity(1);
        let mut ret = Vec::new();

        let mut parser = StrParser::new(text);
        parser.pos = start_pos;

        let trie = &self.0;
        let mut tokenizer = Tokenizer {
            inner:parser,
            string:text,
            off:0,
            prefix:Vec::new(),
            trie,
            language
        };
        loop {
            let Some(next) = tokenizer.next() else {
                if stack.is_empty() { break }
                Self::clear(&mut stack,&mut ret,needs_usemodule,&mut tokenizer,ignores);
                continue
            };
            //tracing::warn!("Checking {:?}",next.verb.as_ref());
            if let Some(last) = stack.last() {
                let previous_merged = last
                    .merged
                    .as_ref()
                    .map_or_else(|| last.token.verb.clone(), |(m, _)| m.clone());
                let merged = previous_merged.merge(&next.verb, last.token.separator);
                if let Some(sub) = trie.subtrie(&merged)
                    && !sub.is_empty()
                {
                    if sub.is_leaf() {
                        let first = stack.first().expect("bug");
                        let text_str = &tokenizer.string[first.token.byte_range.0..last.token.byte_range.1];
                        if !ignores.iter().any(|s| s.eq_ignore_ascii_case(text_str)) {
                            let Ok(Some(v)) = sub.get(&merged) else { unreachable!("bug")};
                            // SAFETY: known to be non-empty
                            let start =first.token.range.start;
                            //tracing::warn!("Success: {:?}",next.verb.as_ref());
                            ret.push((
                                StringRange {
                                    start,
                                    end: next.range.end,
                                },
                                v.iter().map(|s| (s.clone(),needs_usemodule(s))).collect(),
                            ));
                        }
                        stack.clear();
                        continue;
                    }
                    let can_merge = trie.get(&merged).is_some();
                    stack.push(StackElem {
                        token:next,
                        merged: Some((merged, can_merge)),
                    });
                    continue;
                }
                tokenizer.prefix.push(next);
                Self::clear(&mut stack,&mut ret,needs_usemodule,&mut tokenizer,ignores);
                continue
            }
            if let Some(sub) = trie.subtrie(&next.verb)
                && !sub.is_empty()
            {
                if sub.is_leaf() {
                    let text_str = &tokenizer.string[next.byte_range.0..next.byte_range.1];
                    if !ignores.iter().any(|s| s.eq_ignore_ascii_case(text_str)) {
                        let Ok(Some(v)) = sub.get(&next.verb) else { unreachable!("bug")};
                        //tracing::warn!("Success: {:?}",next.verb.as_ref());
                        ret.push((next.range, v.iter().map(|s| (s.clone(),needs_usemodule(s))).collect()));
                    }
                } else {
                    stack.push(StackElem {
                        token:next,
                        merged: None,
                    });
                }
            }
        }
        ret
    }
}
impl VerbalizationTrie {
    #[must_use]
    pub fn lock(&self) -> UnlockedVerbalizationTrie<'_> {
        UnlockedVerbalizationTrie(self.0.lock())
    }
    pub fn insert(&self, language: Language, text: &str, uri: &SymbolUri) {
        self.lock().insert(language, text, uri);
    }
    //pub fn get()
    pub fn find_all_in<P: StringPosition>(
        &self,
        language: Language,
        text: &str,
        start_pos:P,
        ignore:&[&str],
        needs_usemodule: &dyn Fn(&SymbolUri) -> bool
    ) -> Vec<(StringRange<P>, SmallVec<(SymbolUri,bool), 1>)> {
        self.lock().find_all_in(language, text,start_pos,ignore,needs_usemodule)
    }
    #[allow(clippy::needless_pass_by_value)]
    pub fn merge(&self,other:Self) {
        use radix_trie::TrieCommon;
        let mut trie = self.0.lock();
        for (k,v) in other.0.lock().iter() {
            // not nice :(
            trie.insert(k.clone(), v.clone());
        }
    }
}

#[derive(Clone)]
pub struct Verbalization {
    pub lang: Language,
    inner: Nibble,
}
#[derive(Clone)]
struct Nibble(radix_trie::NibbleVec<[u8; 64]>);
impl Nibble {
    fn push(&mut self, mut c: u8) {
        c.make_ascii_lowercase();
        self.0.push(c >> 4);
        self.0.push(c);
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::hash::Hash for Verbalization {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.0.as_bytes().hash(state);
    }
}
impl AsRef<str> for Verbalization {
    fn as_ref(&self) -> &str {
        // SAFETY: by construction, always valid utf8
        unsafe { std::str::from_utf8_unchecked(self.inner.0.as_bytes()) }
    }
}
impl std::fmt::Display for Verbalization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}
impl std::fmt::Debug for Verbalization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}
impl Verbalization {
    const BREAKS: [char; 17] = [
        ' ', '\t', '\r', '\n', '.', ',', ';', '!', ':', '-', '(', ')', '[', ']', '{', '}', '$',
    ];
    fn merge(mut self, other: &Self, sep: Option<char>) -> Self {
        if let Some(sep) = sep
            && !sep.is_ascii_whitespace()
        {
            for u in sep.into_bytes() {
                self.inner.push(u);
            }
        } else {
            self.inner.push(b' ');
        }
        for b in other.inner.0.as_bytes() {
            self.inner.push(*b);
        }
        self
    }
    fn stem(s: &str, lang: Language) -> Cow<'_, str> {
        Self::stemmer(lang).map_or_else(|| Cow::Borrowed(s), |stem| stem.stem(s))
    }
    fn stemmer(lang: Language) -> Option<waken_snowball::Stemmer> {
        Some(
            (match lang {
                Language::Arabic => waken_snowball::Algorithm::Arabic,
                Language::English => waken_snowball::Algorithm::English,
                Language::Finnish => waken_snowball::Algorithm::Finnish,
                Language::French => waken_snowball::Algorithm::French,
                Language::German => waken_snowball::Algorithm::German,
                Language::Romanian => waken_snowball::Algorithm::Romanian,
                Language::Russian => waken_snowball::Algorithm::Russian,
                Language::Turkish => waken_snowball::Algorithm::Turkish,
                Language::Slovenian | Language::Bulgarian | _ => {
                    return None;
                }
            })
            .stemmer(),
        )
    }
    #[must_use]
    pub fn new(txt: &str, lang: Language) -> Self {
        let mut ret = Nibble(NibbleVec::new());
        let stemmer = Self::stemmer(lang);

        /*let Some(stemmer) = Self::stemmer(lang) else {
            for b in txt.as_bytes() {
                ret.push(*b);
            }
            return Self { lang, inner: ret };
        };*/
        for e in txt.trim().split(|c: char| c.is_ascii_whitespace()) {
            if e.is_empty() {
                continue;
            }
            if !ret.is_empty() {
                ret.push(b' ');
            }
            if let Some(stemmer) = &stemmer {
                for b in stemmer.stem(e).as_bytes() {
                    ret.push(*b);
                }
            } else {
                for b in e.as_bytes() {
                    ret.push(*b);
                }
            }
        }
        Self { lang, inner: ret }
    }
}
impl PartialEq for Verbalization {
    fn eq(&self, other: &Self) -> bool {
        self.inner.0 == other.inner.0
    }
}
impl Eq for Verbalization {}
impl radix_trie::TrieKey for Verbalization {
    fn encode(&self) -> radix_trie::NibbleVec<[u8; 64]> {
        self.inner.0.clone()
    }
}
