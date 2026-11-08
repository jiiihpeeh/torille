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
///
/// The 2nd infinitive instructive of `kuulla` is `kuullen`; the class-67
/// engine generates it, and the `kuulte-`/`kuulten` spellings are not in the
/// paradigm (en.wiktionary, `kuulla`).
const CLOSED: &[(&str, &str)] = &[
    ("alhaaltapäin", "alhaalta"),
    ("enemmittä", "enempi"),
    ("hiljempaa", "hiljaa"),
    ("keskenämme", "keskenään"),
    ("loitommalla", "loitto"),
    ("pormestarinna", "pormestari"),
    ("suurkaanin", "suurkaani"),
    ("tienpuoleen", "tienpuoli"),
    ("unhotusta", "unhotus"),
    ("ylhäältäpäin", "ylhäältä"),
    ("äkkipikaa", "pikaa"),
];

/// Fused spellings that modern Finnish writes as two words. They are not
/// accepted; the suggester offers the split form.
///
/// Kotus, *Yhdyssana vai ei: nurin päin vai nurinpäin?* (Kielitoimiston
/// ohjepankki), gives `edeltä käsin`, `itse kunkin`, `sen tähden` family,
/// `sitten kun` and `yhtä kaikki` as two words. The `niin kauan`,
/// `silloin kun` and `tien puoleen` rows follow the same pronoun/particle +
/// adverb/postposition rule.
pub fn is_nonword(word: &str) -> bool {
    matches!(
        word,
        "aivankuin"
            | "avainkuin"
            | "edeltäkäsin"
            | "ennenkuin"
            | "itsekunkin"
            | "minkätähden"
            | "niinkauan"
            | "niinkuin"
            | "senjälkeen"
            | "sentähden"
            | "silloinkun"
            | "yhtäkaikki"
            | "yllinkyllin"
    ) || word.starts_with("kehoit")
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
            ("loitommalla", "loitto"),
                    ("ylhäältäpäin", "ylhäältä"),
            ("äkkipikaa", "pikaa"),
        ] {
            assert_eq!(base(word), Some(expected), "{word}");
        }
    }

    /// `ventovieraita` is a form of the classless compound `ventovieras`;
    /// the modifier `vento` has no lemma row, so the compound layer needs
    /// the opaque-modifier entry.
    #[test]
    fn an_opaque_compound_modifier_resolves() {
        use crate::expand::check_normal::{NormalizedOutcome, check_normal, check_normal_fast};
        for outcome in [
            check_normal("ventovieraita", false),
            check_normal_fast("ventovieraita", false),
        ] {
            assert!(
                matches!(outcome, NormalizedOutcome::Inflected { .. }),
                "ventovieraita should be a known compound form"
            );
        }
        assert_eq!(base("ventovieraita"), None);
    }

    /// The 2nd-infinitive instructive is `kuullen`; the `kuulten`/`kuulte-`
    /// spellings are not in the paradigm.
    #[test]
    fn the_second_infinitive_instructive_is_kuullen() {
        use crate::expand::check_normal::{NormalizedOutcome, check_normal, check_normal_fast};

        for outcome in [
            check_normal("kuullen", false),
            check_normal_fast("kuullen", false),
        ] {
            assert!(
                matches!(outcome, NormalizedOutcome::Inflected { .. }),
                "kuullen should resolve to kuulla"
            );
        }
        for word in [
            "kuulten",
            "kuulteni",
            "kuultesi",
            "kuultemme",
            "kuultenne",
            "kuultensa",
        ] {
            assert_eq!(base(word), None, "{word}");
            assert_eq!(
                check_normal(word, false),
                NormalizedOutcome::Unknown,
                "full path: {word}"
            );
            assert_eq!(
                check_normal_fast(word, false),
                NormalizedOutcome::Unknown,
                "fast path: {word}"
            );
        }
    }

    /// Kotus writes these as two words, so the fused spelling is rejected and
    /// no longer shadows the split form.
    #[test]
    fn two_word_spellings_are_rejected() {
        for word in [
            "edeltäkäsin",
            "itsekunkin",
            "niinkauan",
            "senjälkeen",
            "silloinkun",
            "yhtäkaikki",
        ] {
            assert_eq!(base(word), None, "{word} must not be a closed spelling");
            assert!(is_nonword(word), "{word} must be a nonword");
        }
    }

    /// A lowercase proper-name form is not a closed spelling; the name gate
    /// only accepts capitalised forms.
    #[test]
    fn lowercase_name_rows_are_gone() {
        for word in ["honkasien", "selmallakaan"] {
            assert_eq!(base(word), None);
            assert!(!is_nonword(word), "{word} is a name, not a fused phrase");
        }
    }

    /// Rows removed because the engine/derivation now derives them must still
    /// resolve on both the full and the fast path.
    #[test]
    fn removed_engine_rows_resolve_on_both_paths() {
        use crate::expand::check_normal::{NormalizedOutcome, check_normal, check_normal_fast};

        fn base(outcome: NormalizedOutcome) -> Option<String> {
            match outcome {
                NormalizedOutcome::Inflected { base, .. } => Some(base),
                _ => None,
            }
        }

        for (word, expected) in [
            ("kehottavasti", "kehottaa"),
            ("koneentapaisesti", "koneentapainen"),
            ("perehtyneempi", "perehtynyt"),
            ("nyrpistyksessä", "nyrpistys"),
            ("seuduitta", "seutu"),
            ("kävelytti", "kävelyttää"),
        ] {
            assert_eq!(
                base(check_normal(word, false)).as_deref(),
                Some(expected),
                "full path: {word}"
            );
            assert_eq!(
                base(check_normal_fast(word, false)).as_deref(),
                Some(expected),
                "fast path: {word}"
            );
        }
    }

    /// `kävelyttää` is the causative; its past third person is `kävelytti`
    /// (weak grade, one `l`). The double-`l` spelling is not a form of it.
    #[test]
    fn the_double_l_causative_is_not_a_word() {
        use crate::expand::check_normal::{NormalizedOutcome, check_normal, check_normal_fast};
        for outcome in [
            check_normal("kävellytti", false),
            check_normal_fast("kävellytti", false),
        ] {
            assert_eq!(outcome, NormalizedOutcome::Unknown);
        }
        assert_eq!(base("kävellytti"), None);
    }
}
