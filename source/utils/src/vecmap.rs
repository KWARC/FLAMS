use std::fmt::Debug;

#[derive(Clone, Hash, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VecMap<K, V>(pub Vec<(K, V)>);

impl<K, V, K2, V2> PartialEq<VecMap<K2, V2>> for VecMap<K, V>
where
    K: PartialEq<K2>,
    V: PartialEq<V2>,
{
    fn eq(&self, other: &VecMap<K2, V2>) -> bool {
        self.0.len() == other.0.len()
            && self
                .iter()
                .zip(other.iter())
                .all(|((k1, v1), (k2, v2))| k1 == k2 && v1 == v2)
    }
}

impl<K, V> Default for VecMap<K, V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl<K, V> FromIterator<(K, V)> for VecMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}
impl<K: Debug, V: Debug> Debug for VecMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.0.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}
impl<K, V> VecMap<K, V> {
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }
    pub fn get<E: ?Sized>(&self, key: &E) -> Option<&V>
    where
        for<'a> &'a E: PartialEq<&'a K>,
    {
        self.0.iter().find(|(k, _)| key == k).map(|(_, v)| v)
    }
    pub fn get_mut<E: ?Sized>(&mut self, key: &E) -> Option<&mut V>
    where
        for<'a> &'a E: PartialEq<&'a K>,
    {
        self.0.iter_mut().find(|(k, _)| key == k).map(|(_, v)| v)
    }
    pub fn get_mut_index(&mut self, i: usize) -> Option<&mut (K, V)> {
        self.0.get_mut(i)
    }
    pub fn get_or_insert_mut(&mut self, key: K, value: impl FnOnce() -> V) -> &mut V
    where
        K: PartialEq,
    {
        let ret = if let Some((i, _)) = self.0.iter().enumerate().find(|(_, (k, _))| k == &key) {
            i
        } else {
            self.0.push((key, value()));
            self.0.len() - 1
        };
        &mut self.0[ret].1
    }
    pub fn insert(&mut self, key: K, value: V)
    where
        K: PartialEq,
    {
        match self.0.iter_mut().find(|(k, _)| k == &key) {
            Some((_, v)) => *v = value,
            None => self.0.push((key, value)),
        }
    }
    pub fn remove<E: ?Sized>(&mut self, key: &E) -> Option<V>
    where
        for<'a> &'a E: PartialEq<&'a K>,
    {
        let index = self.0.iter().position(|(k, _)| key == k)?;
        Some(self.0.swap_remove(index).1)
    }
    pub fn remove_index(&mut self, i: usize) -> (K, V) {
        self.0.swap_remove(i)
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
    pub fn contains_key<E: ?Sized>(&self, key: &E) -> bool
    where
        for<'a> &'a E: PartialEq<&'a K>,
    {
        self.0.iter().any(|(k, _)| key == k)
    }
}
impl<K, V> IntoIterator for VecMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K, V> From<Vec<(K, V)>> for VecMap<K, V> {
    fn from(v: Vec<(K, V)>) -> Self {
        Self(v)
    }
}

#[derive(Clone, Hash, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VecSet<V>(pub Vec<V>);

impl<V, V2> PartialEq<VecSet<V2>> for VecSet<V>
where
    V: PartialEq<V2>,
{
    fn eq(&self, other: &VecSet<V2>) -> bool {
        self.0.len() == other.0.len() && self.iter().zip(other.iter()).all(|(v1, v2)| v1 == v2)
    }
}

impl<V> Default for VecSet<V> {
    #[inline]
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl<V: PartialEq<V>> FromIterator<V> for VecSet<V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let mut v = Vec::new();
        for i in iter {
            if !v.contains(&i) {
                v.push(i);
            }
        }
        Self(v)
    }
}
impl<V: Debug> Debug for VecSet<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter()).finish()
    }
}
impl<V> IntoIterator for VecSet<V> {
    type Item = V;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<V: PartialEq<V>> From<Vec<V>> for VecSet<V> {
    #[inline]
    fn from(v: Vec<V>) -> Self {
        v.into_iter().collect()
    }
}

impl<V> VecSet<V> {
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &V> {
        self.0.iter()
    }
    pub fn insert(&mut self, value: V)
    where
        V: PartialEq,
    {
        if !self.0.contains(&value) {
            self.0.push(value);
        }
    }
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }
}

impl<V: Clone> VecSet<V> {
    pub fn insert_clone(&mut self, value: &V)
    where
        V: PartialEq,
    {
        if !self.0.contains(value) {
            self.0.push(value.clone());
        }
    }
}

#[derive(Clone, Hash, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrdSet<V: Ord>(pub Vec<V>);

impl<V, V2: Ord> PartialEq<OrdSet<V2>> for OrdSet<V>
where
    V: Ord + PartialEq<V2>,
{
    fn eq(&self, other: &OrdSet<V2>) -> bool {
        self.0.len() == other.0.len() && self.iter().zip(other.iter()).all(|(v1, v2)| v1 == v2)
    }
}

impl<V: Ord> Default for OrdSet<V> {
    #[inline]
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl<V: PartialEq<V> + Ord> FromIterator<V> for OrdSet<V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let mut v = Vec::new();
        for elem in iter {
            if let Err(i) = v.binary_search(&elem) {
                v.insert(i, elem);
            }
        }
        Self(v)
    }
}
impl<V: Debug + Ord> Debug for OrdSet<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter()).finish()
    }
}
impl<V: Ord> IntoIterator for OrdSet<V> {
    type Item = V;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<V: PartialEq<V> + Ord> From<Vec<V>> for OrdSet<V> {
    #[inline]
    fn from(v: Vec<V>) -> Self {
        v.into_iter().collect()
    }
}

impl<V: Ord> OrdSet<V> {
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &V> {
        self.0.iter()
    }
    pub fn insert(&mut self, value: V)
    where
        V: PartialEq,
    {
        if let Err(i) = self.0.binary_search(&value) {
            self.0.insert(i, value);
        }
    }
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    #[must_use]
    pub fn contains(&self, v: &V) -> bool {
        self.0.binary_search(v).is_ok()
    }
}

impl<V: Ord + Clone> OrdSet<V> {
    pub fn insert_clone(&mut self, value: &V) {
        if let Err(i) = self.0.binary_search(value) {
            self.0.insert(i, value.clone());
        }
    }
}
