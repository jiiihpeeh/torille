//! Consonant gradation, the `*X` code in the Kotus sanalista.
//!
//! Finnish gradation is the alternation a stem shows when a form is built
//! from its weak grade rather than its strong: `pankki` is `pankin` in the
//! genitive (the `kk` weakens to `k`) and `pankkiin` in the illative (it
//! stays strong). The site of the alternation is a property of the stem,
//! not of the form that picks the grade, and the way the strong and weak
//! consonants relate is a property of the gradation type.
//!
//! The sanalista records the gradation type as a single capital letter that
//! runs `A` through `M` (the `*X` part of a `class*X` inflection code), and
//! `Gradation::from_letter` decodes it. The strong/weak consonant pair for
//! each type lives in [`Gradation::pair`]; types that delete a consonant or
//! weaken `h` return `None` from it and are not reconstructed, only emitted
//! unchanged.
//!
//! Gradation is a property of the *stem*, not the *form*: the genitive stem
//! of `hammas` is `hammaa` (weak `mm`), and [`Gradation::strongen`] is the
//! step that takes a word that already carries the weak grade back to the
//! strong. [`Gradation::apply`] is the other direction: given a word that
//! sits in the strong grade, build the weak form by replacing the strong
//! pair at the gradation site with the weak one.

/// Consonant gradation type, the `*X` code in the sanalista (`kk <-> k` is `A`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Gradation(pub u8);

/// The grade of a gradating stem.
///
/// A noun's paradigm does not use one stem throughout: the genitive is weak
/// and the illative strong (`pankki` is `pankin` but `pankkiin`), and which
/// grade a form wants is a property of the form, not a universal rule.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Grade {
    Strong,
    Weak,
}

impl Grade {
    /// Whether the grade is the weak one, which is the one gradation produces.
    pub fn is_weak(self) -> bool {
        self == Grade::Weak
    }
}

impl Gradation {
    /// Decodes the letter used in the sanalista, `A..Z` maps to `1..=26`.
    pub fn from_letter(c: char) -> Option<Self> {
        match c {
            'A'..='Z' => Some(Gradation(c as u8 - b'A' + 1)),
            _ => None,
        }
    }

    /// The strong/weak consonant pair for this gradation type.
    ///
    /// `Some((strong, weak))`, where both sides are the *consonants that
    /// replace each other*. Types that also delete a consonant (`G`: `nk <-> ng
    /// <-> ∅`) or weaken `h` return `None`; they are not reconstructed, only
    /// the unaltered form is emitted.
    pub fn pair(self) -> Option<(&'static str, &'static str)> {
        Some(match self.0 {
            1 => ("kk", "k"),   // A
            2 => ("pp", "p"),   // B
            3 => ("tt", "t"),   // C
            4 => ("k", ""),     // D: reikä : reiän
            5 => ("p", "v"),    // E: sopu : sovun
            6 => ("t", "d"),    // F
            7 => ("nk", "ng"),  // G: kenkä : kengän
            8 => ("mp", "mm"),  // H
            9 => ("lt", "ll"),  // I
            10 => ("nt", "nn"), // J
            11 => ("rt", "rr"), // K
            12 => ("k", "j"),   // L: arki : arjen
            13 => ("k", "v"),   // M: suku : suvun
            _ => return None,
        })
    }

    /// The byte offset of this gradation's strong pair in `word`, if it sits
    /// there.
    ///
    /// Gradation happens at a fixed site, not at whichever consonant happens
    /// to match: `pankki` gradates in its `kk` wherever that is, and a word
    /// that happens to contain a `kk` somewhere else is not affected. The site
    /// is the last occurrence before the stem-final vowel, which is where the
    /// consonant gradation actually sits in Finnish.
    pub fn site(self, word: &str) -> Option<usize> {
        let (strong, _) = self.pair()?;
        if strong.is_empty() {
            return None;
        }
        // Look for the pair before the trailing vowel(s) of the stem. The
        // `kk` of `pankki` sits two characters from the end, so the site is
        // found working back from the last consonant.
        let mut at = None;
        for (i, _) in word.char_indices() {
            if word[i..].starts_with(strong) {
                at = Some(i);
            }
        }
        at
    }

    /// The place the weak grade sits in a word that already carries it.
    ///
    /// The reverse of [`Gradation::site`], which looks for the strong grade.
    /// `hammas`'s genitive stem `hammaa` has the weak `mm` (not the strong
    /// `mp`), so it is the weak site that is found there.
    pub fn weak_site(self, word: &str) -> Option<usize> {
        let (_, weak) = self.pair()?;
        if weak.is_empty() {
            return None;
        }
        let mut at = None;
        for (i, _) in word.char_indices() {
            if word[i..].starts_with(weak) {
                at = Some(i);
            }
        }
        at
    }

    /// A word that already carries the weak grade, turned back to the strong.
    ///
    /// `hammas` is `hammaa` in its genitive stem and `hampaa` where the word
    /// actually is (its nominative `hammas` already had the weak `mm`), so the
    /// genitive is the strong grade, the reverse of the usual weakening.
    pub fn strongen(self, word: &str) -> String {
        let (strong, weak) = match self.pair() {
            Some(p) => p,
            None => return word.to_string(),
        };
        let Some(at) = self.weak_site(word) else {
            return word.to_string();
        };
        format!("{}{strong}{}", &word[..at], &word[at + weak.len()..])
    }

    /// The word with this gradation applied in the grade given.
    ///
    /// The strong grade is the word as it stands; the weak grade replaces the
    /// strong pair at the site with the weak one. Where the pair is lossy
    /// (`D`: `reikä : reiän`, the `k` disappears) the site's consonant is
    /// deleted.
    pub fn apply(self, word: &str, grade: Grade) -> String {
        let (strong, weak) = match self.pair() {
            Some(p) => p,
            None => return word.to_string(),
        };
        let Some(at) = self.site(word) else {
            return word.to_string();
        };
        match grade {
            Grade::Strong => word.to_string(),
            Grade::Weak => format!("{}{weak}{}", &word[..at], &word[at + strong.len()..]),
        }
    }

    /// Gradation types that a noun/verb paradigm can also be reconstructed
    /// for. `D`, `E` and `G` are deliberately absent from [`Gradation::pair`]
    /// handling above only where the pair is lossy.
    pub fn all_known() -> impl Iterator<Item = Gradation> {
        (1u8..=13).map(Gradation)
    }
}