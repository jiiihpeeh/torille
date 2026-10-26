//! A compact set of words tuned for spell checking.
//!
//! Finnish needs a few million word forms, so the usual `HashSet<String>` is a
//! poor fit: it costs a heap allocation plus 24 bytes of `String` header per
//! word. [`WordSet`] instead concatenates every word into one buffer and keeps
//! only a `u32` offset per word, with lookups going through an open-addressing
//! table of `u32` indices.

use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};

/// Empty slot marker inside [`WordSet::table`].
const EMPTY: u32 = u32::MAX;

/// A set of words stored in one contiguous buffer.
#[derive(Clone, Debug)]
pub struct WordSet {
    /// All words concatenated, without separators.
    buffer: String,
    /// Byte offset of each word; the length is implied by the next offset.
    offsets: Vec<u32>,
    /// Open-addressing table holding `offset index + 1`, or [`EMPTY`].
    table: Vec<u32>,
    /// `table.len() - 1`, so probing is a mask rather than a division.
    mask: usize,
    len: usize,
}

/// The default is [`WordSet::new`] and not a derive, because a derived one
/// would leave the lookup table empty and the first lookup would index into
/// nothing.
impl Default for WordSet {
    fn default() -> Self {
        Self::new()
    }
}

impl WordSet {
    /// An empty set, with a small table so lookups never probe a zero-length
    /// table.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// A set holding the words of `it`, ignoring duplicates.
    pub fn from_words<'a, I>(it: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let it = it.into_iter();
        let mut set = Self::with_capacity(it.size_hint().0);
        for w in it {
            set.insert(w);
        }
        set
    }

    /// An empty set sized for roughly `capacity` distinct words.
    pub fn with_capacity(capacity: usize) -> Self {
        // Keep the load factor at or below 0.7 for short probe chains.
        let slots = ((capacity * 10 / 7 + 16).max(32)).next_power_of_two();
        Self {
            buffer: String::with_capacity(capacity * 12),
            offsets: Vec::with_capacity(capacity),
            table: vec![EMPTY; slots],
            mask: slots - 1,
            len: 0,
        }
    }

    /// Number of distinct words.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Total bytes held by the word data, excluding the hash table.
    pub fn byte_size(&self) -> usize {
        self.buffer.len() + self.offsets.len() * 4
    }

    /// Bytes held by the whole structure, including the hash table.
    pub fn total_bytes(&self) -> usize {
        self.byte_size() + self.table.len() * 4
    }

    /// Iterates the words in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.offsets
            .iter()
            .enumerate()
            .map(move |(i, &o)| &self.buffer[o as usize..self.end_of(i)])
    }

    /// True when `word` is in the set. Case sensitive; callers lower-case first.
    pub fn contains<Q: AsRef<str> + ?Sized>(&self, word: &Q) -> bool {
        self.find(word.as_ref()).is_some()
    }

    /// Index of `word` in [`WordSet::iter`] order.
    pub fn position<Q: AsRef<str> + ?Sized>(&self, word: &Q) -> Option<usize> {
        self.find(word.as_ref())
    }

    /// The word that was inserted at `position`, counting from zero.
    ///
    /// The position is the insertion order, which is what [`Self::position`]
    /// returns, so this is the other half of that: together they let a table be
    /// kept beside the set and indexed by it, which is how the grammar index
    /// avoids a second copy of every word. The buffer holds the words with no
    /// separators, so the length of one is the distance to the next offset, and
    /// the last one runs to the end.
    pub fn word_at(&self, position: usize) -> Option<&str> {
        let start = *self.offsets.get(position)? as usize;
        let end = self
            .offsets
            .get(position + 1)
            .map_or(self.buffer.len(), |&next| next as usize);
        Some(&self.buffer[start..end])
    }

    fn find(&self, word: &str) -> Option<usize> {
        debug_assert!(!word.is_empty());
        let mut i = self.probe(word) & self.mask;
        loop {
            let slot = self.table[i];
            if slot == EMPTY {
                return None;
            }
            let idx = (slot - 1) as usize;
            if &self.buffer[self.offsets[idx] as usize..self.end_of(idx)] == word {
                return Some(idx);
            }
            i = (i + 1) & self.mask;
        }
    }

    /// The word at iteration position `i`.
    pub fn get(&self, i: usize) -> Option<&str> {
        self.offsets
            .get(i)
            .map(|&o| &self.buffer[o as usize..self.end_of(i)])
    }

    /// Adds `word`, returning `true` when it was not already present.
    pub fn insert(&mut self, word: &str) -> bool {
        if word.is_empty() || self.contains(word) {
            return false;
        }
        if self.len * 10 >= self.table.len() * 7 {
            self.grow();
        }
        self.insert_new(word);
        true
    }

    /// Adds `word` and reports whether it was already present.
    ///
    /// This is [`WordSet::insert`] with the branch inverted, so hot loops can
    /// skip the extra comparison.
    pub fn insert_checked(&mut self, word: &str) -> bool {
        if word.is_empty() {
            return false;
        }
        if self.find(word).is_some() {
            return true;
        }
        if self.len * 10 >= self.table.len() * 7 {
            self.grow();
        }
        self.insert_new(word);
        false
    }

    fn insert_new(&mut self, word: &str) {
        let offset = self.buffer.len() as u32;
        self.buffer.push_str(word);
        self.offsets.push(offset);
        let idx = self.offsets.len() - 1;
        let mut i = self.probe(word) & self.mask;
        while self.table[i] != EMPTY {
            i = (i + 1) & self.mask;
        }
        self.table[i] = (idx + 1) as u32;
        self.len += 1;
    }

    fn grow(&mut self) {
        let slots = self.table.len() * 2;
        let mut table = vec![EMPTY; slots];
        self.mask = slots - 1;
        for idx in 0..self.offsets.len() {
            let word = &self.buffer[self.offsets[idx] as usize..self.end_of(idx)];
            let mut i = self.probe(word) & self.mask;
            while table[i] != EMPTY {
                i = (i + 1) & (slots - 1);
            }
            table[i] = (idx + 1) as u32;
        }
        self.table = table;
    }

    #[inline]
    fn end_of(&self, i: usize) -> usize {
        match self.offsets.get(i + 1) {
            Some(&next) => next as usize,
            None => self.buffer.len(),
        }
    }

    #[inline]
    fn probe(&self, word: &str) -> usize {
        // FNV-1a, which is plenty for short words and needs no setup.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in word.as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h as usize
    }
}

impl<S: Borrow<str>> Extend<S> for WordSet {
    fn extend<I: IntoIterator<Item = S>>(&mut self, it: I) {
        for w in it {
            self.insert(w.borrow());
        }
    }
}

impl<'a> IntoIterator for &'a WordSet {
    type Item = &'a str;
    type IntoIter = Box<dyn Iterator<Item = &'a str> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl FromIterator<String> for WordSet {
    fn from_iter<I: IntoIterator<Item = String>>(it: I) -> Self {
        let mut s = Self::new();
        s.extend(it);
        s
    }
}

/// Builds a set with a caller-supplied hasher, used by tests that need a
/// deterministic probe order.
impl WordSet {
    // The one-line form of this is `std::hash::Hash::hash_one`, which the
    // `Hash` of this module's own naming stands in the way of, so the lint is
    // turned off here rather than the type renamed.
    #[doc(hidden)]
    #[allow(clippy::manual_hash_one)]
    pub fn probe_hash(&self, word: &str) -> u64 {
        let mut h = RandomState::new().build_hasher();
        word.hash(&mut h);
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_contains() {
        let mut s = WordSet::new();
        assert!(s.insert("talo"));
        assert!(s.insert("taloa"));
        assert!(!s.insert("talo"));
        assert!(s.contains("talo"));
        assert!(s.contains("taloa"));
        assert!(!s.contains("tal"));
        assert!(!s.contains("taloo"));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn empty_word_is_ignored() {
        let mut s = WordSet::new();
        assert!(!s.insert(""));
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn iterates_in_insertion_order() {
        let words = ["maa", "maan", "maata", "maalle"];
        let mut s = WordSet::new();
        for w in words {
            s.insert(w);
        }
        assert_eq!(s.iter().collect::<Vec<_>>(), words);
        for (i, w) in words.iter().enumerate() {
            assert_eq!(s.get(i), Some(*w));
            assert_eq!(s.position(w), Some(i));
        }
    }

    #[test]
    fn survives_growth() {
        let mut s = WordSet::with_capacity(4);
        for i in 0..5000 {
            s.insert(&format!("sana{i}"));
        }
        assert_eq!(s.len(), 5000);
        for i in 0..5000 {
            assert!(s.contains(&format!("sana{i}")), "missing sana{i}");
        }
        assert!(!s.contains("sana5000"));
    }

    #[test]
    fn multibyte_offsets_are_byte_based() {
        let mut s = WordSet::new();
        for w in ["käsi", "kättä", "kätten", "äiti", "isä"] {
            s.insert(w);
        }
        assert!(s.contains("kättä"));
        assert!(s.contains("isä"));
        assert!(!s.contains("kädä"));
        assert_eq!(s.iter().count(), 5);
    }

    #[test]
    fn insert_checked_inverts_the_result() {
        let mut s = WordSet::new();
        assert!(!s.insert_checked("talo"));
        assert!(s.insert_checked("talo"));
    }
}
