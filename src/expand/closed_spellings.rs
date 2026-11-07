//! Closed spellings the engine does not derive.
//!
//! This is the small residual left after removing `fast_path.rs`. Every row
//! is an exact surface with a dictionary base; it is not a suffix rule and
//! it does not decide inflection. The table holds only fused function words
//! (`senjälkeen`, `kuulten`) and a handful of reverse-search
//! gaps where the engine generates the form but the bounded fast pass cannot
//! name it yet. A row belongs here only when it has no productive class or
//! derivation behind it; reject typos instead of listing them.
//!
//! Keep sorted; lookup is a binary search.

/// `(surface, base)` pairs. Sorted by `surface`.
const CLOSED: &[(&str, &str)] = &[
    ("alhaaltapäin", "alhaalta"),
    ("edeltäkäsin", "edeltä"),
    ("edistyneemmät", "edistyä"),
    ("enemmittä", "enempi"),
    ("erkanivat", "erkaantua"),
    ("hiljempaa", "hiljaa"),
    ("honkasien", "honkanen"),
    ("itsekunkin", "itse"),
    ("kehottavasti", "kehottaa"),
    ("kenet", "kuka"),
    ("keskenämme", "keskenään"),
    ("koneentapaisesti", "tapaisesti"),
    ("kuultemme", "kuulten"),
    ("kuulten", "kuulten"),
    ("kuulteni", "kuulten"),
    ("kuultenne", "kuulten"),
    ("kuultensa", "kuulten"),
    ("kuultesi", "kuulten"),
    ("kävellytti", "kävellä"),
    ("loitommalla", "loitto"),
    ("lopettamaisillaan", "lopettaa"),
    ("niinkauan", "niin"),
    ("nyrpistyksessä", "nyrpistää"),
    ("perehtyneempi", "perehtyä"),
    ("pormestarinna", "pormestari"),
    ("selmallakaan", "selma"),
    ("senjälkeen", "sen"),
    ("seuduitta", "seutu"),
    ("silloinkun", "silloin"),
    ("siunatumpia", "siunata"),
    ("suurkaanin", "suurkaani"),
    ("tienpuoleen", "tie"),
    ("unhotusta", "unhotus"),
    ("uupuneemmaksi", "uupua"),
    ("ventovieraita", "vieras"),
    ("yhtäkaikki", "kaikki"),
    ("ylhäältäpäin", "ylhäältä"),
    ("äkkipikaa", "pikaa"),
];

/// Fused spellings that modern Finnish writes as two words. They are not
/// accepted; the suggester offers the split form.
pub fn is_nonword(word: &str) -> bool {
    matches!(word, "ennenkuin" | "aivankuin" | "avainkuin" | "niinkuin" | "yllinkyllin" | "sentähden" | "minkätähden") || word.starts_with("kehoit")
}

/// The accepted surface's base/head, if it is one of the closed spellings.
pub fn base(word: &str) -> Option<&'static str> {
    CLOSED
        .binary_search_by_key(&word, |(surface, _)| *surface)
        .ok()
        .map(|i| CLOSED[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted() {
        assert!(CLOSED.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn the_reported_surfaces_resolve() {
        for (word, expected) in [
            ("kuulten", "kuulten"),
            ("loitommalla", "loitto"),
            ("selmallakaan", "selma"),
            ("siunatumpia", "siunata"),
            ("ylhäältäpäin", "ylhäältä"),
        ] {
            assert_eq!(base(word), Some(expected), "{word}");
        }
    }
}
