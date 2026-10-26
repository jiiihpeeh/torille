//! Vowel harmony: which side of the a/ä split a word takes its endings from.

/// Which side of the a/ä split a word harmonises to.
///
/// The vowel `y` is neutral and harmonises to `ä` by convention, which is how
/// `yhteydessä` and `yö` coexist in the corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Harmony {
    Back,
    Front,
}

impl Harmony {
    pub(super) fn a(self) -> char {
        match self {
            Self::Back => 'a',
            Self::Front => 'ä',
        }
    }

    pub(super) fn o(self) -> char {
        match self {
            Self::Back => 'o',
            Self::Front => 'ö',
        }
    }
}

/// The harmony of a word: front if any of `ä`, `ö` or `y` is in it, back if any
/// of `a`, `o` or `u` is, and front for a word of only the neutral vowels.
///
/// This is the reading a generator attaches endings by, and it always has an
/// answer.
pub fn harmony(word: &str) -> Harmony {
    reading(word).unwrap_or(Harmony::Front)
}

/// What a word's own spelling settles about its harmony.
///
/// `Some` is a reading the letters give. The stem's vowels decide it: a word
/// with any of the front vowels `ä`, `ö` or `y` is front, and one with any of
/// the back vowels `a`, `o` or `u` is back, since Finnish words do not mix the
/// two sets. A word of nothing but the neutral vowels `i` and `e` is read
/// front, the convention every such word is read by (`risti`, `vene`, `mies`
/// and `piste` all take front endings). The `e` is neutral in a native theme
/// vowel and in a loanword alike — `huone` and `kone` are back on their `o`,
/// `nainen` is back on its `a`, `kämmen` front on its `ä` — so no `e` complicates
/// the reading.
///
/// [`harmony`] answers the same words the same way this does.
pub fn reading(word: &str) -> Option<Harmony> {
    let mut back = false;
    let mut front = false;
    for c in word.chars() {
        match c {
            'a' | 'o' | 'u' | 'A' | 'O' | 'U' => back = true,
            'ä' | 'ö' | 'y' | 'Ä' | 'Ö' | 'Y' => front = true,
            _ => {}
        }
    }
    if front {
        Some(Harmony::Front)
    } else if back {
        Some(Harmony::Back)
    } else {
        Some(Harmony::Front)
    }
}

/// An ending whose harmony vowel is read from `to` rather than from the model
/// the ending was copied from.
///
/// A class model is a single word, and Finnish puts words of both harmonies in
/// one class: class 3 is modelled on the back-harmony `valtio` and holds the
/// front-harmony `eliö` with it. Taking the ending's vowel from the model gives
/// `eliö` the back-harmony `eliöta` and `eliöita` where Finnish has `eliötä`
/// and `eliöitä`.
///
/// Only the vowels `a`/`ä` and `o`/`ö` are swapped, so an ending that has no
/// harmony vowel in it (`-n`, `-ssa`, `-hin`) and an ending carrying a `u`,
/// which has no front counterpart, are left exactly as they were. The caller
/// decides whether the ending needs reading at all: a lemma that harmonises the
/// same way as the model is already carrying the right vowel, and the model's
/// own forms are what the catalogue tabulates.
pub(crate) fn harmonise<'a>(ending: &'a str, to: Harmony) -> std::borrow::Cow<'a, str> {
    let (a, o) = match to {
        Harmony::Back => ('a', 'o'),
        Harmony::Front => ('ä', 'ö'),
    };
    if !ending.contains('a')
        && !ending.contains('o')
        && !ending.contains('ä')
        && !ending.contains('ö')
    {
        return std::borrow::Cow::Borrowed(ending);
    }
    if ending.contains('u') {
        return std::borrow::Cow::Borrowed(ending);
    }
    std::borrow::Cow::Owned(
        ending
            .chars()
            .map(|c| match c {
                'a' | 'ä' => a,
                'o' | 'ö' => o,
                c => c,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harmony_reads_the_last_vowel() {
        assert_eq!(harmony("kala"), Harmony::Back);
        assert_eq!(harmony("kylmä"), Harmony::Front);
        assert_eq!(harmony("käsi"), Harmony::Front);
        // `i` and `y` are neutral, so the reading comes from further left.
        assert_eq!(harmony("yhteys"), Harmony::Front);
        assert_eq!(harmony("koti"), Harmony::Back);
        // A capitalised lemma reads the same as the lower case one, which
        // matters because the sanalista is not all lower case.
        assert_eq!(harmony("Äiti"), Harmony::Front);
        assert_eq!(harmony("Ötö"), Harmony::Front);
        assert_eq!(harmony("Aamu"), Harmony::Back);
        assert_eq!(harmony("Oma"), Harmony::Back);
    }

    #[test]
    fn the_reading_knows_when_the_spelling_says_nothing() {
        // A vowel of the `a`/`ä` pair decides, and the reading is the harmony
        // the ending takes.
        assert_eq!(reading("talo"), Some(Harmony::Back));
        assert_eq!(reading("kala"), Some(Harmony::Back));
        assert_eq!(reading("maa"), Some(Harmony::Back));
        assert_eq!(reading("kylmä"), Some(Harmony::Front));
        assert_eq!(reading("käsi"), Some(Harmony::Front));
        assert_eq!(reading("isoäiti"), Some(Harmony::Front));
        // `i` and `y` are neutral, so the reading comes from the vowel before
        // them: `lapsi` is a back word on the `a` of `la-`.
        assert_eq!(reading("lapsi"), Some(Harmony::Back));
        assert_eq!(reading("koti"), Some(Harmony::Back));
        // A word of nothing but neutral vowels is read by the convention.
        assert_eq!(reading("risti"), Some(Harmony::Front));
        assert_eq!(reading("vene"), Some(Harmony::Front));
        // An `e` is neutral, not evidence of a loanword: the stem's back or
        // front vowel decides even across it. `huone`, `kone` and `nainen` are
        // back on their `o`/`a`, `kämmen` front on its `ä`.
        assert_eq!(reading("paperi"), Some(Harmony::Back));
        assert_eq!(reading("huone"), Some(Harmony::Back));
        assert_eq!(reading("nainen"), Some(Harmony::Back));
        assert_eq!(reading("kone"), Some(Harmony::Back));
        assert_eq!(reading("kämmen"), Some(Harmony::Front));
        assert_eq!(harmony("paperi"), Harmony::Back);
        assert_eq!(harmony("huone"), Harmony::Back);
    }
}
