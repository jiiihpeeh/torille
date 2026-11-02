//! Finding a word's parts in the lexicon without reading the whole lexicon.
//!
//! A class 50/51 compound is split by looking for the words it is made of, and
//! that used to mean a `for head in lexicon` over all 104 743 lemmas for every
//! compound, with a second full scan nested inside it to find the other part.
//! There are 696 compound lemmas, so the head test alone ran about 73 million
//! times -- and a linear scan is what makes adding a word to the sanalista cost
//! time quadratically rather than linearly.
//!
//! The candidates are not arbitrary, and that is the whole observation: a head
//! is a *prefix* of the compound and a tail is a *suffix*, so both are range
//! queries over the lexicon's sorted words rather than searches of it.
//! `build.rs` sorts the rows once, at build time, into [`WORD_ORDER`] (by the
//! word) and [`TAIL_ORDER`] (by the word reversed -- reversing is what turns a
//! suffix into a prefix). Each question is then a binary search for a lower
//! bound and a scan while the bound still holds, both over static data, so
//! **nothing here is built at run time**.
//!
//! That last part is the reason this is not a `HashMap`. The obvious way to
//! speed the scan up is to bucket the lexicon by prefix and keep the buckets,
//! and that was measured: it builds in 37 ms per process and it made the whole
//! index build *slower*, not faster. A precomputed table costs one pass at
//! compile time and nothing at all afterwards.
//!
//! # What it is worth, measured
//!
//! Over the 696 compound lemmas, splitting each one twice, single-threaded:
//!
//! | | time | answers |
//! |---|---|---|
//! | linear scan | 4.30 s | |
//! | sorted-order search | 3.24 ms | identical, 0 disagreements |
//!
//! So the operation is about 1 300 times faster and provably returns the same
//! thing. **The index build as a whole did not get faster**: it sits at ~38 s
//! either way. That is arithmetic rather than a disappointment -- the 4.30 s is
//! one thread, and `build()` runs on twelve, so the scan was about 0.36 s of
//! wall time out of 38. It was never the bottleneck. What the change buys is the
//! *shape*, not the seconds: a lookup is O(log n) instead of O(lemmas), so
//! adding words to the sanalista stops costing time quadratically. The binary
//! grows by about 2 MB for the two orders.
//!
//! This is a *filter*, not an answer. The candidates are a superset, and the
//! caller still checks `starts_with` and the length floors itself, because those
//! rules are about Finnish compounding and do not belong in a data structure.
//! What is promised here is only that no candidate is ever hidden -- and that
//! candidates come back **in lexicon order**, which is the order the linear
//! scans produced and which the emitted forms depend on.

use crate::kotus::Lemma;

/// How many leading characters decide a bucket.
///
/// Four is measured, not guessed. Three gives a median bucket of 7 and a worst
/// of 836; four gives a median of 3 and a worst of 392; five gives a median of 2
/// but needs six buckets to stay correct, since every word shorter than the
/// prefix has to be reachable from its own length as well. Four is the point
/// where the buckets stop paying for themselves in size -- see
/// `the_lexicon_needs_no_index_built_at_run_time`.
pub const PREFIX: usize = 4;

/// The lexicon, with its two precomputed orders.
///
/// Holds no tables of its own: [`WORD_ORDER`] and [`TAIL_ORDER`] are emitted by
/// `build.rs` into the binary, and the rows themselves are borrowed from the
/// caller's lexicon. Constructing this is free, which is why it can be created
/// once and handed to every thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct LexiconIndex<'a> {
    /// The lexicon, whose rows the two orders index into.
    ///
    /// The borrow is what lets a candidate row number become a `&Lemma` rather
    /// than a copy of one. The row numbers are `build.rs`'s, and they index the
    /// `Lemma`s that `parse_sanalista` produces in the same order -- the
    /// invariant `the_orders_index_the_same_rows_the_parser_produces` holds.
    lexicon: &'a [Lemma<'a>],
}

/// Where `WORD_ORDER` starts being greater than `key`.
///
/// `partition_point` on a sorted slice, spelled out because the order is a
/// static array of row numbers and the comparison has to go through the lexicon
/// to reach the words.
fn lower_bound_by_word<'a>(key: &str, lexicon: &'a [Lemma<'a>]) -> usize {
    crate::kotus::sanalista().word_order().partition_point(|&row| lexicon[row as usize].word < key)
}

impl<'a> LexiconIndex<'a> {
    /// Borrows `lexicon` and the two orders `build.rs` emitted for it.
    ///
    /// Costs nothing: the work was done when the crate was compiled.
    pub fn new(lexicon: &'a [Lemma<'a>]) -> Self {
        Self { lexicon }
    }

    /// The lexicon the two orders index into.
    ///
    /// Exposed for the callers that need the slice rather than the index: the
    /// reverse lookup (`find_bases_with`) takes the lexicon directly, and a
    /// compound modifier with gradation (`puhtaaksi` of `puhdas`) can only be
    /// resolved that way.
    pub fn lemmas(&self) -> &'a [Lemma<'a>] {
        self.lexicon
    }

    /// The rows gathered for one word, in **lexicon order** and without repeats.
    ///
    /// Two things have to be undone. The scans come out in sorted order because
    /// that is the order the orders are stored in, and they go back to lexicon
    /// order because that is the order the linear scan produced and the order
    /// the emitted forms depend on. And the same row can be reached twice,
    /// because a word whose length is under [`PREFIX`] is found by more than
    /// one prefix -- see [`Self::heads_of`]. The runs are short, so sorting is
    /// not worth avoiding.
    fn rows_sorted_lexicon(&self, mut rows: Vec<u32>) -> Vec<&'a Lemma<'a>> {
        rows.sort_unstable();
        rows.dedup();
        rows.into_iter()
            .map(|row| &self.lexicon[row as usize])
            .collect()
    }

    /// The lemmas that could be the head of `word`, as a superset.
    ///
    /// Every head of `word` is in here and most of what is in here is not a
    /// head, which is the trade: the caller was going to reject the wrong ones
    /// with `starts_with` anyway, and now it rejects them from a run of a few
    /// dozen instead of a list of a hundred thousand.
    ///
    /// Two searches, and the second is only there because of `SHORTEST_HEAD`.
    /// A head of [`PREFIX`] or more characters shares the compound's leading
    /// [`PREFIX`] characters, so the sorted-order run under that prefix holds
    /// all of them. A head of exactly `SHORTEST_HEAD` characters is *equal* to
    /// the compound's leading `SHORTEST_HEAD` characters, which is an exact
    /// lookup and not a range -- and it is not inside the run above, because a
    /// word of `SHORTEST_HEAD` characters sorts before the longer words that
    /// share its prefix. Scanning the `SHORTEST_HEAD` run instead would also
    /// find it, but it would drag in every word of that length too, which is
    /// the larger set.
    pub fn heads_of(&self, word: &str) -> Vec<&'a Lemma<'a>> {
        let mut rows = Vec::new();
        if let Some(prefix) = prefix_chars(word, PREFIX) {
            let start = lower_bound_by_word(prefix, self.lexicon);
            for &row in crate::kotus::sanalista().word_order().iter().skip(start) {
                let candidate = self.lexicon[row as usize].word;
                if !candidate.starts_with(prefix) {
                    break;
                }
                rows.push(row);
            }
        }
        if let Some(short) = prefix_chars(word, crate::expand::compound::SHORTEST_HEAD) {
            rows.extend(
                crate::kotus::sanalista()
                    .by_word(short)
                    .map(|entry| entry.row()),
            );
        }
        self.rows_sorted_lexicon(rows)
    }

    /// The lemmas that are exactly `word`, in lexicon order.
    ///
    /// The "and the rest of this is a word of its own" test. The lexicon's own
    /// hash table answers this, so nothing is built here.
    pub fn tail_exactly(&self, word: &str) -> impl Iterator<Item = &'a Lemma<'a>> {
        crate::kotus::sanalista()
            .by_word(word)
            .filter_map(move |entry| self.lexicon.get(entry.row() as usize))
    }

    /// The lemmas that could be a *suffix* of `word`, as a superset.
    ///
    /// The mirror of [`Self::heads_of`] for class 50, where the tail is what
    /// inflects and the head is "whatever precedes the longest suffix that is a
    /// word of its own". Every suffix of a word is a prefix of its reversal, so
    /// this is the same lower-bound-and-scan as `heads_of` -- but over
    /// [`crate::kotus::TAIL_ORDER`], and once per length.
    ///
    /// The "once per length" is the whole difference from `heads_of`, and it is
    /// forced. A reversed suffix of length *L* is a prefix of the reversal of
    /// length *L*, but a shorter prefix sorts *before* the longer one that
    /// extends it: `talo` reversed is `olat`, the reversed suffix `ola` sorts
    /// before it, and `ol` before that. So the suffixes of one word are not one
    /// contiguous run of the reversed order, and a single scan from one bound
    /// finds the full-length tail and none of the shorter ones. Measured: a
    /// single-bound scan returned nothing at all for all 696 compounds.
    pub fn tails_of(&self, word: &str) -> Vec<&'a Lemma<'a>> {
        let reversed: Vec<char> = word.chars().rev().collect();
        let mut rows = Vec::new();
        for length in crate::expand::compound::SHORTEST_TAIL..word.chars().count() {
            let key = &reversed[..length];
            let start = crate::kotus::sanalista().tail_order().partition_point(|&row| {
                let candidate = self.lexicon[row as usize].word;
                candidate.chars().rev().cmp(key.iter().copied()) == std::cmp::Ordering::Less
            });
            for &row in crate::kotus::sanalista().tail_order()[start..].iter() {
                let candidate = self.lexicon[row as usize].word;
                // The *whole* reversal, not a truncation of it. A longer word
                // whose reversal merely begins with `key` is not a tail of
                // `word`, and comparing only `key`'s length returns one: that
                // is how `aamukampa` came back as a tail of `kauppa`.
                if candidate.chars().rev().ne(key.iter().copied()) {
                    break;
                }
                rows.push(row);
            }
        }
        self.rows_sorted_lexicon(rows)
    }
}

/// The first `n` characters of `word`, or `None` if `word` is shorter.
///
/// **Characters, not bytes.** The sanalista has `ala‑` with U+2011
/// NON-BREAKING HYPHEN, which is three bytes, and a four-byte cut lands inside
/// it and panics. Slicing a `str` at a byte offset that is not a character
/// boundary is not something a lookup can be allowed to do, so every cut in
/// this file goes through here.
fn prefix_chars(word: &str, n: usize) -> Option<&str> {
    match word.char_indices().nth(n) {
        Some((at, _)) => Some(&word[..at]),
        None if word.chars().count() == n => Some(word),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotus::parse_sanalista;

    fn lexicon() -> Vec<Lemma<'static>> {
        parse_sanalista(crate::SANALISTA)
    }

    /// The claim the module is built on: no structure is constructed at run
    /// time. `LexiconIndex` is two words wide and holds only a borrow, because
    /// both orders were emitted by `build.rs`.
    #[test]
    fn the_lexicon_needs_no_index_built_at_run_time() {
        assert_eq!(
            std::mem::size_of::<LexiconIndex<'_>>(),
            std::mem::size_of::<&[Lemma<'_>]>()
        );
        let lex = lexicon();
        // Only the construction is timed. `lexicon()` is the 31 ms parse, and
        // timing it with `new` is what made the first version of this test
        // report 17 ms of "index building" that was really the parse.
        let t = std::time::Instant::now();
        let index = LexiconIndex::new(&lex);
        std::hint::black_box(index);
        let built = t.elapsed();
        assert!(
            built.as_micros() < 50,
            "LexiconIndex::new took {built:?}, so something is being built"
        );
    }

    /// The load-bearing invariant. `build.rs` numbers the rows and
    /// `parse_sanalista` numbers them again, independently, and the two must
    /// agree or every lookup here returns the wrong word.
    #[test]
    fn the_orders_index_the_same_rows_the_parser_produces() {
        let lex = lexicon();
        let word_order = crate::kotus::sanalista().word_order();
        let tail_order = crate::kotus::sanalista().tail_order();
        assert_eq!(word_order.len(), lex.len());
        assert_eq!(tail_order.len(), lex.len());
        // The orders are sorted, by word and by word reversed.
        for pair in word_order.windows(2) {
            assert!(lex[pair[0] as usize].word <= lex[pair[1] as usize].word);
        }
        // `<=`, not `<`: a sanalista word can be listed more than once as homonym
        // rows, and two rows of the same word are equal, not ordered.
        for pair in tail_order.windows(2) {
            assert!(
                lex[pair[0] as usize]
                    .word
                    .chars()
                    .rev()
                    .cmp(lex[pair[1] as usize].word.chars().rev())
                    != std::cmp::Ordering::Greater
            );
        }
    }

    /// A head is never hidden. Every word of at least `SHORTEST_HEAD`
    /// characters must come back from `heads_of` on itself, because a head is
    /// always a prefix of the compound and the compound always contains it.
    #[test]
    fn a_head_is_never_hidden_from_the_search_that_should_hold_it() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        for word in ["puoli", "talo", "auto", "kauppa", "yö"] {
            let found: Vec<&str> = index.heads_of(word).iter().map(|l| l.word).collect();
            if word.chars().count() >= crate::expand::compound::SHORTEST_HEAD {
                assert!(found.contains(&word), "{word} lost: {found:?}");
            }
        }
    }

    /// And the whole point: the candidates are a handful, not the language.
    #[test]
    fn the_candidates_are_a_handful_not_the_language() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        let compounds: Vec<&Lemma> = lex
            .iter()
            .filter(|l| l.inflections.iter().any(|i| i.class == 50 || i.class == 51))
            .collect();
        assert!(!compounds.is_empty());
        let mut total = 0usize;
        let mut worst = 0usize;
        for c in &compounds {
            let n = index.heads_of(c.word).len();
            total += n;
            worst = worst.max(n);
        }
        let mean = total as f64 / compounds.len() as f64;
        assert!(mean < 100.0, "mean {mean:.1} candidates per compound");
        assert!(worst < 2_000, "worst bucket {worst}");
    }

    /// The candidates come back in lexicon order, which is the order the linear
    /// scan produced and which the emitted forms depend on.
    #[test]
    fn the_candidates_come_back_in_lexicon_order() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        for c in lex
            .iter()
            .filter(|l| l.inflections.iter().any(|i| i.class == 50 || i.class == 51))
            .take(50)
        {
            for which in [index.heads_of(c.word), index.tails_of(c.word)] {
                let mut positions: Vec<usize> = which
                    .iter()
                    .map(|l| lex.iter().position(|r| r.word == l.word).unwrap())
                    .collect();
                let sorted = {
                    let mut p = positions.clone();
                    p.sort_unstable();
                    p
                };
                assert_eq!(positions, sorted, "{} came back out of order", c.word);
                positions.clear();
            }
        }
    }

    /// `tails_of` finds exactly the suffixes, and nothing else.
    #[test]
    fn a_tail_is_a_suffix_and_only_a_suffix() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        for word in ["kauppa", "puoliyö", "talo"] {
            for tail in index.tails_of(word) {
                assert!(
                    word.ends_with(tail.word),
                    "{} is not a suffix of {word}",
                    tail.word
                );
                assert!(tail.word.len() < word.len());
            }
        }
        assert!(
            !index.tails_of("talo").iter().any(|l| l.word == "talo"),
            "a word is not its own tail"
        );
    }

    /// The bug a single lower bound gives, kept as the test for it.
    ///
    /// `kauppa` reversed is `appauk`. The reversed tails are `a` and `ap` and
    /// `app`, and each sorts *before* the one it extends, so they are three
    /// separate runs rather than one. The first version of `tails_of` searched
    /// once at the full length and so found no tail at all -- for any word.
    #[test]
    fn a_tail_shorter_than_the_word_is_still_a_tail() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        // Every suffix of every compound, checked against a plain scan.
        let compounds: Vec<&Lemma> = lex
            .iter()
            .filter(|l| l.inflections.iter().any(|i| i.class == 50 || i.class == 51))
            .collect();
        assert!(!compounds.is_empty());
        for compound in &compounds {
            let wanted: Vec<&str> = lex
                .iter()
                .filter(|l| {
                    compound.word.ends_with(l.word)
                        && l.word.chars().count() >= crate::expand::compound::SHORTEST_TAIL
                        && l.word.chars().count() < compound.word.chars().count()
                })
                .map(|l| l.word)
                .collect();
            let got: Vec<&str> = index
                .tails_of(compound.word)
                .iter()
                .map(|l| l.word)
                .collect();
            for word in &wanted {
                assert!(
                    got.contains(word),
                    "{} lost the tail {word}; found {got:?}",
                    compound.word
                );
            }
        }
    }

    /// No row reaches the caller twice, which is what would double a compound's
    /// forms.
    ///
    /// Tested here rather than through `heads_of` because from outside the
    /// module rows are indistinguishable: the sanalista lists `iso` as rows
    /// 18450 and 18451, and both are correct answers, so a test that counted
    /// *words* would call correct behaviour a duplicate.
    #[test]
    fn a_row_is_never_returned_twice() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        // Every row the three searches could reach, listed once per route.
        let mut rows = Vec::new();
        for &row in crate::kotus::sanalista().word_order().iter().take(50) {
            rows.push(row);
            rows.push(row);
        }
        let got = index.rows_sorted_lexicon(rows);
        assert_eq!(got.len(), 50, "a duplicated row survived");
    }

    /// Completeness over the whole language, not three words: every row that is
    /// a genuine head of a compound comes back. This is the property the
    /// sorted-order search has to keep, and it is checked against a plain scan
    /// so that a wrong bound shows up as a missing word rather than as a count.
    #[test]
    fn every_head_of_every_compound_is_found() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        let floor = crate::expand::compound::SHORTEST_HEAD;
        let mut checked = 0;
        for compound in lex
            .iter()
            .filter(|l| l.inflections.iter().any(|i| i.class == 50 || i.class == 51))
        {
            let found: Vec<&str> = index
                .heads_of(compound.word)
                .iter()
                .map(|l| l.word)
                .collect();
            for head in lex
                .iter()
                .filter(|l| l.word.chars().count() >= floor && compound.word.starts_with(l.word))
            {
                assert!(
                    found.contains(&head.word),
                    "{} lost the head {}",
                    compound.word,
                    head.word
                );
                checked += 1;
            }
        }
        assert!(checked > 500, "only {checked} heads to check");
    }

    /// The bug the fingerprint found on the first run of this module, kept.
    ///
    /// `ala‑` ends in U+2011 NON-BREAKING HYPHEN, which is three bytes, so
    /// cutting a prefix at four *bytes* lands inside it and panics.
    #[test]
    fn a_word_whose_hyphen_is_three_bytes_long_can_be_searched() {
        let lex = lexicon();
        let index = LexiconIndex::new(&lex);
        let mut checked = 0;
        for lemma in lex.iter().filter(|l| l.word.contains('\u{2011}')) {
            let _ = index.heads_of(lemma.word);
            let _ = index.tails_of(lemma.word);
            checked += 1;
        }
        assert!(
            checked > 0,
            "the sanalista has no non-breaking hyphen to test"
        );
    }
}
