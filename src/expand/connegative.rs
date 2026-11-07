//! The bare verb connegatives.
//!
//! A connegative is the verb's own word in a negative clause: `en syöne`,
//! `älkää ajatelko`. The engine emits them only inside the multiword
//! phrase (it does not generate a bare `syöne` -- see the note on
//! `expand::verbs::compound`), but the tokeniser sees the two words
//! separately, so the reverse lookup has to accept the bare one.
//!
//! The set is derived from the oracle's own attested phrases by
//! `tools/oracle/build_verb_lexicon.py` and embedded by `build.rs` from
//! `data/verb_connegatives.csv`, the same way the adverb and
//! adjective-degree tables are. Only the potential and the imperative
//! are listed: the indicative present is the 2sg imperative, the
//! indicative past is the past participle, and the conditional
//! connegative is the affirmative third person, so those surfaces are
//! already generated.

mod table {
    use crate::expand::static_table::StemMap;

    /// Bare verb connegatives: form -> lemma, derived by
    /// `tools/oracle/build_verb_lexicon.py`.
    pub static CONNEGATIVES: StemMap = StemMap::new(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/connegatives.bin",
    )));
}

/// The lemma of a bare connegative, or `None` when `word` is not one.
///
/// `word` is expected lowercase, as everywhere in the walks.
pub fn base(word: &str) -> Option<&'static str> {
    table::CONNEGATIVES.bases(word).next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reported_connegatives_resolve() {
        assert_eq!(base("syöne"), Some("syödä"));
        assert_eq!(base("ajatelko"), Some("ajatella"));
        assert_eq!(base("hätäilkö"), Some("hätäillä"));
    }

    #[test]
    fn an_ordinary_word_is_not_a_connegative() {
        for word in ["talo", "syödä", "syönee", "koira", "ymmärtää"] {
            assert_eq!(base(word), None, "{word}");
        }
    }
}
