//! Suggest corrections for an unknown word.
//!
//! Two functions, mirroring the hunspell C API shape so callers (Firefox,
//! vim, aspell, LibreOffice, our own `tools/src/bin/check.rs`) can ask:
//!
//!   * [`check`] -- "is this word correctly spelled?" -- returns a
//!     [`CheckOutcome`] that says *what kind* of word it is, not just yes or
//!     no. Hunspell's `hunspell_spell` is just a boolean; the engine is
//!     morphological, so the answer carries the lemma.
//!   * [`suggest`] -- "if it's not, what should it be?" -- returns up to N
//!     candidates ranked by edit distance, the same shape hunspell's
//!     `hunspell_suggest` returns.
//!
//! Both use the engine's `find_bases` family, which walks heads, tails,
//! prefix-3, prefix-2 and the short-lemma scan before deciding there is no
//! lemma this word belongs to. `check` asks for recognition only; `suggest`
//! also asks for the generated forms and neighbours its ranker needs. The
//! cost is bounded by the input word's length and the prefix's
//! neighbourhood, not by the size of the sanalista: a one-word query is
//! sub-second on the embedded lexicon, where building the full reverse
//! index takes 30 s.
//!
//! ## What this does *not* do
//!
//! - **Compound analysis.** A compound is a valid word even when no single
//!   lemma inflects to it -- `nuori-isäntä` is two lemmas glued together --
//!   and the engine's [`crate::compound::analyze`] knows how to find them,
//!   but it leans on [`FormBuilder`](crate::expand::FormBuilder), which is
//!   the slow path. Adding compound support to the fast `check` is a
//!   separate piece of work; for now a compound is reported as
//!   [`CheckOutcome::Inflected`] only when one of its parts inflects to the
//!   whole, and as [`CheckOutcome::Unknown`] otherwise. The full
//!   `check.rs` still uses the slow path and handles the compound case
//!   correctly.
//! - **Weighted edit distance.** `suggest` uses plain Levenshtein in the
//!   DP, but a small [`KEYBOARD_ADJACENT`] table is consulted in the score:
//!   a distance-1 single-substitution between QWERTY neighbours adds a
//!   tie-break bonus to [`Suggestion::existing_score`], so a typo like
//!   `tallu` (substitute `o` -> adjacent `u`) ranks above a far-apart
//!   `tallk` at the same edit distance. The DP itself does not weight
//!   `cost`; the bonus is a separate ranking signal. Adjacent transpositions
//!   (`letter_1+letter_2` -> `letter_2+letter_1`, the Damerau extension)
//!   are full 1-edit operations in the DP: `teh` -> `the` is distance 1.
//!   Insert/delete keyboard adjacencies (`koiaa` -> `koira`, missing `r`)
//!   are not yet modelled.

use std::sync::OnceLock;

use crate::expand::{BasesReport, ClassPrior, find_bases_for_check, rank_lemma_bases};

mod candidates;
mod casing;
mod detail;
mod known;
mod levenshtein;
mod state;

pub use candidates::{suggest, suggest_detailed};
pub use casing::case_is_valid;
pub use detail::{Suggestion, SuggestionSource};
pub(crate) use known::{
    derived_action_us_base, derived_comparative_base, derived_sti_base, is_content_head,
    is_known_compound_dp, is_known_compound_fast, is_known_short_compound,
};
pub use known::{
    is_known_word, is_known_word_fast, is_known_word_no_compound, is_lemma, suggest_over_lemmas,
    warm_caches,
};
pub use levenshtein::{EditOp, levenshtein, levenshtein_with_ops};
pub use state::{check_fast, suggest_with_state};

use known::{catalogue, hyphenated_lexeme, lexicon};

/// What kind of spelling outcome `check` returned.
///
/// Carries the lemma in [`Self::Inflected`] rather than just a boolean so a
/// caller that wants to *suggest* a correction knows which lemma to find
/// neighbours of.
#[derive(Debug)]
pub enum CheckOutcome {
    /// The query is exactly a lemma in the sanalista.
    Lemma,
    /// The query is an inflected form of one or more lemmas. The lemmas
    /// are in [`BasesReport`] order; the first is the sanalista's, the rest
    /// are head/tail/prefix neighbours that also inflect to the query.
    Inflected(Vec<&'static str>),
    /// The query is not in the lexicon and no lemma inflects to it.
    Unknown,
}

impl CheckOutcome {
    /// `true` for [`Self::Lemma`] and [`Self::Inflected`]; `false` for
    /// [`Self::Unknown`]. Compounds currently route here as `Unknown`; see
    /// the module doc.
    pub fn is_known(&self) -> bool {
        !matches!(self, CheckOutcome::Unknown)
    }
}

/// Check whether `word` is a known Finnish word, and if so, what kind.
///
/// The answer covers the sanalista's lemmas directly (one probe), every
/// inflected form the engine knows how to produce (heads, tails, the
/// prefix-3 / prefix-2 walks, and the short-lemma scan), and stops short
/// of compound analysis -- see the module doc.
pub fn check(word: &str) -> CheckOutcome {
    with_proper_name_readings(check_common(word), word)
}

/// The common-word answer, before the proper-name readings are merged in.
fn check_common(word: &str) -> CheckOutcome {
    if word.is_empty() {
        return CheckOutcome::Unknown;
    }
    let cat = catalogue();
    let lexicon = lexicon();
    // A standard hyphenated spelling (`linja-auto`) is not optional; the
    // engine would otherwise accept the closed compound `linjaauto`.
    if hyphenated_lexeme(word).is_some() {
        return CheckOutcome::Unknown;
    }
    // The cheap recognition gates answer before the engine is asked. A
    // direct lemma is one probe; an ordinary inflected form resolves through
    // the lemma set / form set / stem maps. Only the leftovers (rare forms,
    // homonyms that need every reading) pay for the engine walk.
    let lower = word.to_lowercase();
    if crate::kotus::sanalista().by_word(&lower).next().is_some() {
        return CheckOutcome::Lemma;
    }
    if let Some(base) = is_known_word_no_compound(&lower) {
        let mut rows = crate::kotus::sanalista().by_word(&base);
        match (rows.next(), rows.next()) {
            // Unambiguous sanalista base: the same answer `classify` gives.
            (Some(row), None) => return CheckOutcome::Inflected(vec![row.word()]),
            // A productive derivation (`laitilalainen`) has no row; it is
            // still a recognised word, just without a lexicon lemma.
            (None, _) => return CheckOutcome::Inflected(Vec::new()),
            // Homonyms: let the engine report every base it can prove.
            (Some(_), Some(_)) => {}
        }
    }
    // Recognition doesn't need the generated-form payload or the neighbour
    // list; asking the engine for those is what makes the suggestion path
    // expensive.
    let report = find_bases_for_check(lexicon, cat, word);
    if report.direct.is_empty() && report.bases.is_empty() {
        // `find_bases` searches single-lexeme paradigms; compounds and
        // productive derivations are a spelling/word-formation rule the
        // fast detector owns. Falling back here keeps `check` and
        // `check_fast` from disagreeing about `limonadipullojen` or a
        // hyphenated compound.
        if let Some(base) = is_known_word(word) {
            if let Some(row) = crate::kotus::sanalista().by_word(&base).next() {
                return CheckOutcome::Inflected(vec![row.word()]);
            }
            return CheckOutcome::Inflected(Vec::new());
        }
        return CheckOutcome::Unknown;
    }
    classify(report)
}

/// Merge the proper-name readings into a common-word outcome.
///
/// A capitalised surface can be both a common word and a name (`Maine` the
/// state and `maine` the reputation, `Mainen` the adjective and the state's
/// genitive). The common word stays first; the names are additional readings,
/// not a fallback that only fires when the common gates miss.
fn with_proper_name_readings(outcome: CheckOutcome, word: &str) -> CheckOutcome {
    // A lowercase common word is not a name reading, even when a surname has
    // the same spelling: `talo` is the house, not the surname `Talo`.
    if !word.chars().next().is_some_and(char::is_uppercase) {
        return outcome;
    }
    let names = crate::expand::proper_names::lemmas(word);
    if names.is_empty() {
        return outcome;
    }
    match outcome {
        CheckOutcome::Inflected(mut bases) => {
            for name in names {
                if !bases.contains(&name) {
                    bases.push(name);
                }
            }
            CheckOutcome::Inflected(bases)
        }
        CheckOutcome::Lemma => {
            let lower = word.to_lowercase();
            match crate::kotus::sanalista().by_word(&lower).next() {
                Some(row) => {
                    let mut bases = vec![row.word()];
                    for name in names {
                        if !bases.contains(&name) {
                            bases.push(name);
                        }
                    }
                    CheckOutcome::Inflected(bases)
                }
                None => CheckOutcome::Lemma,
            }
        }
        CheckOutcome::Unknown => CheckOutcome::Inflected(names),
    }
}

fn classify(report: BasesReport<'static>) -> CheckOutcome {
    if !report.direct.is_empty() {
        return CheckOutcome::Lemma;
    }
    if !report.bases.is_empty() {
        // The recognition walk keeps the bases in its own phase order. Rank
        // them the way the reverse index does, on the evidence the walk has:
        // the noun-over-derived-adjective rule and the class prior. Without
        // this, `heränneen` names the participle `herännyt` while `lookup`
        // names `herätä`, and the two check paths disagree about the same
        // word.
        let lemmas: Vec<&'static str> = rank_lemma_bases(report.bases, class_prior())
            .into_iter()
            .map(|l| l.word)
            .collect();
        return CheckOutcome::Inflected(lemmas);
    }
    CheckOutcome::Unknown
}

/// The class prior the recognition walk ranks with, built once from the same
/// leaked lexicon the walk reads.
fn class_prior() -> &'static ClassPrior {
    static PRIOR: OnceLock<ClassPrior> = OnceLock::new();
    PRIOR.get_or_init(|| ClassPrior::of(lexicon()))
}

// Pull in `sanalista` to keep the lib link alive when suggest is built
// without the rest of `expand::` (the `sanalista` accessor used to be
// `pub use`d as `WORD_ORDER` / `TAIL_ORDER`; after the blob move the
// surface is the function, and a few internal helpers still mention
// the old names in docstrings, so the import stays for grep-ability).
#[allow(unused_imports)]
use crate::kotus::sanalista as _sanalista_keepalive;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::levenshtein::{KEYBOARD_ADJACENT_BONUS, is_keyboard_adjacent, keyboard_bonus};
    use super::*;

    /// Finnish compounds are closed unless a writing rule licenses a hyphen:
    /// abbreviation/number/proper-name first part, a same-vowel boundary, or
    /// a paired expression. The closed spelling of a lexicalised hyphenated
    /// compound is a misspelling in the other direction.
    #[test]
    fn hyphenated_compounds_follow_the_writing_rules() {
        for word in [
            "EU-maat",
            "TV-ohjelma",
            "20-vuotias",
            "A-vitamiini",
            "Suomi-ottelu",
            "Helsinki-Vantaa",
            "Matti-setä",
            "linja-auto",
            "maa-alue",
            "suomalais-ruotsalainen",
            "itä-länsi-suhteet",
        ] {
            assert!(
                is_known_word(word).is_some(),
                "{word} should be accepted as a hyphenated compound"
            );
        }
        for word in [
            "kirja-hylly",
            "kahvi-kuppi",
            "kesä-loma",
            "tieto-kone",
            "jalka-pallo",
            "talo-yhtiö",
            "linjaauto",
        ] {
            assert!(
                is_known_word(word).is_none(),
                "{word} should be rejected (closed spelling is the word)"
            );
        }
        assert!(
            suggest("kirja-hylly", 5, 2)
                .iter()
                .any(|s| s == "kirjahylly")
        );
        assert!(suggest("linjaauto", 5, 2).iter().any(|s| s == "linja-auto"));
        assert!(
            suggest("suomalaisruotsalainen", 5, 2)
                .iter()
                .any(|s| s == "suomalais-ruotsalainen")
        );
        assert!(matches!(check("EU-maat"), CheckOutcome::Inflected(_)));
        assert!(matches!(check("kirja-hylly"), CheckOutcome::Unknown));
        assert!(matches!(check("linjaauto"), CheckOutcome::Unknown));
    }

    /// `housut` is a plural-only class-1 lemma. Its compounds must inflect
    /// from the dropped stem (`housuja`), not the old `housutja`, and a
    /// plural-only class-1 lemma must not invent a singular.
    #[test]
    fn plural_only_housut_forms_and_compounds() {
        for word in [
            "housuja",
            "housujen",
            "housuissa",
            "alushousuja",
            "housukauppa",
            "housukauppaa",
            "housukauppaan",
        ] {
            assert!(is_known_word(word).is_some(), "{word} should be known");
        }
        for word in [
            "housu",
            "housutja",
            "housutssa",
            "alushousutja",
            "alushousutn",
            "alushousutssa",
        ] {
            assert!(is_known_word(word).is_none(), "{word} should be unknown");
        }
        let suggestions = suggest("alushousutja", 10, 2);
        assert!(suggestions.iter().any(|s| s == "alushousuja"));
        assert!(!suggestions.iter().any(|s| s == "alushousutja"));
        assert!(!suggestions.iter().any(|s| s == "alushousutn"));
    }

    /// Productive derivations the dictionary does not list, plus the
    /// plural-only class-6 party words. The bad `kekkeritn` is the old
    /// singular-stem artefact; it must stay unknown.
    #[test]
    fn productive_derivations_and_plural_party_words() {
        let nouns = std::collections::HashSet::new();
        for word in [
            "halveksivasti",
            "valittavasti",
            "kiulullinen",
            "hoilaus",
            "hoilausta",
            "kosteahkoa",
            "kevytmielisyydestä",
            "turhamielisyyksissäni",
            "salaperäisemmäksi",
            "suonenvedontapaisesti",
            "molemmin",
            "loitommalla",
            "kekkereitä",
            "kahvikekkereitä",
            "limonadipullojen",
        ] {
            assert!(
                check_fast(word, &nouns).is_some(),
                "{word} should be recognised"
            );
        }
        for word in ["talosti", "kekkeritn", "kekkeritja"] {
            assert!(
                check_fast(word, &nouns).is_none(),
                "{word} should stay unknown"
            );
        }
        let suggestions = suggest("kekkeritn", 6, 2);
        assert!(!suggestions.iter().any(|s| s == "kekkeritn"));
    }

    /// `check` answers from the cheap gates before the engine walk: a direct
    /// lemma, an ordinary inflected form, and a productive derivation. The
    /// engine is still reached for compounds.
    #[test]
    fn check_answers_from_the_fast_gates() {
        assert!(matches!(check("talo"), CheckOutcome::Lemma));
        match check("talossa") {
            CheckOutcome::Inflected(bases) => assert_eq!(bases, vec!["talo"]),
            other => panic!("talossa: {other:?}"),
        }
        // A derived form with no sanalista row is known without a lemma.
        assert!(check("laitilalainen").is_known());
        // A compound is still the engine's (and `is_known_word`'s) answer.
        assert!(check("edistämisvaroista").is_known());
        assert!(!check("xyzzy").is_known());
    }

    /// SI prefixes in their Finnish spellings: `milli-`, `mikro-`,
    /// `nano-`, `tera-`, `peta-`, plus the adjacent `deka-`/`giga-`.
    /// The prefix is a bound modifier; the head supplies the paradigm.
    #[test]
    fn si_prefixes_in_finnish_are_compound_modifiers() {
        let nouns = std::collections::HashSet::new();
        for word in [
            "milliwatti",
            "milliwattia",
            "mikrowatti",
            "mikrowattia",
            "nanowatti",
            "nanowattia",
            "terawatti",
            "terawattia",
            "petatavu",
            "petatavuja",
            "gigawatti",
            "gigawattia",
            "megawatti",
            "kilowatti",
            "dekawatti",
        ] {
            assert!(
                check_fast(word, &nouns).is_some(),
                "{word} should be recognised"
            );
        }
    }

    /// Compounds reported from running text resolve to the right head, not
    /// just to *some* word. The regression this pins is the shared stem
    /// (`teiden` is both `tee` and `tie`) and the function-word surface
    /// (`vallassa` is an adverb as well as a form of `valta`).
    #[test]
    fn reported_compounds_name_the_content_head() {
        for (word, head) in [
            ("harvainvallat", "harvainvalta"),
            ("harvainvallassa", "harvainvalta"),
            ("harvainvalloin", "harvainvalta"),
            ("tähtientutkijat", "tutkija"),
            ("henkiruumiiseenne", "ruumis"),
            ("henkivartioväki", "väki"),
            ("henkivartioväen", "väki"),
            ("henkivartioväkeä", "väki"),
            ("lentokenttäbussi", "bussi"),
            ("raitiotiellä", "raitiotie"),
            ("raitioteiden", "raitiotie"),
            ("liittoveljiensä", "veli"),
        ] {
            assert_eq!(
                is_known_word(word).as_deref(),
                Some(head),
                "{word} should resolve to {head}"
            );
        }
        // `harvainvallain` is not a form; the instructive is `-valloin` and
        // the genitive plural `-valtain`/`-valtojen`.
        assert_eq!(is_known_word("harvainvallain"), None);
    }

    /// Place adjectives from the `-lainen`/`-läinen` derivation, including
    /// the epenthetic `i` after a consonant-ending base.
    #[test]
    fn reported_place_adjectives_name_the_place() {
        for (word, adjective) in [
            ("turkulainen", "turkulainen"),
            ("turkulaisia", "turkulainen"),
            ("turkulaisella", "turkulainen"),
            ("porilainen", "porilainen"),
            ("albertalainen", "albertalainen"),
            ("albertalaisia", "albertalainen"),
            ("missourilainen", "missourilainen"),
            ("missourilaisia", "missourilainen"),
            ("quebecilainen", "quebecilainen"),
            ("quebecilaisia", "quebecilainen"),
            ("hesseniläinen", "hesseniläinen"),
            ("hesseniläisiä", "hesseniläinen"),
            ("laitilalainen", "laitilalainen"),
            ("laitilalaisia", "laitilalainen"),
            ("laitilalaisineen", "laitilalainen"),
            ("laitilalaisineni", "laitilalainen"),
            ("saharalainen", "saharalainen"),
            ("saharalaisia", "saharalainen"),
            ("lahtelainen", "lahtelainen"),
            ("rovaniemeläinen", "rovaniemeläinen"),
        ] {
            assert_eq!(
                is_known_word(word).as_deref(),
                Some(adjective),
                "{word} should resolve to {adjective}"
            );
        }
        // A consonant-ending base takes the epenthetic `i`; the plain form
        // is not the standard spelling.
        for word in ["texaslainen", "quebeclainen", "hessenlainen"] {
            assert_eq!(is_known_word(word), None, "{word} should stay unknown");
        }
    }

    /// `check` returns the right `CheckOutcome` for each kind of word.
    #[test]
    fn check_returns_the_right_outcome() {
        // A lemma in the sanalista.
        assert!(matches!(check("talo"), CheckOutcome::Lemma));
        assert!(matches!(check("koira"), CheckOutcome::Lemma));

        // A form of a lemmas of class 1.
        let r = check("taloa");
        match r {
            CheckOutcome::Inflected(lemmas) => assert!(
                lemmas.contains(&"talo"),
                "taloa should map to talo: {lemmas:?}"
            ),
            other => panic!("expected Inflected(talo), got {:?}", other),
        }

        // A gradated form -- phase-3 territory.
        match check("puhtaan") {
            CheckOutcome::Inflected(lemmas) => assert!(lemmas.contains(&"puhdas")),
            other => panic!("expected Inflected(puhdas), got {:?}", other),
        }

        // A short-lemma case -- phase-4 territory.
        match check("öihin") {
            CheckOutcome::Inflected(lemmas) => assert!(lemmas.contains(&"yö")),
            other => panic!("expected Inflected(yö), got {:?}", other),
        }

        // Not in the lexicon at all.
        assert!(matches!(check("xyzzy"), CheckOutcome::Unknown));
        assert!(matches!(check(""), CheckOutcome::Unknown));
    }

    /// A capitalised surface can be both a common word and a proper name.
    /// The common reading stays first; the name is an additional reading.
    #[test]
    fn a_capitalised_name_keeps_both_readings() {
        match check("Mainen") {
            CheckOutcome::Inflected(bases) => {
                assert!(bases.contains(&"mainen"), "{bases:?}");
                assert!(bases.contains(&"Maine"), "{bases:?}");
            }
            other => panic!("expected Inflected, got {other:?}"),
        }
        match check("Maine") {
            CheckOutcome::Inflected(bases) => {
                assert!(bases.contains(&"maine"), "{bases:?}");
                assert!(bases.contains(&"Maine"), "{bases:?}");
            }
            other => panic!("expected Inflected, got {other:?}"),
        }
    }

    /// When the engine reports several bases, `classify` names the same first
    /// one the full index does. The fast unambiguous-base shortcut is separate:
    /// it names the one row it found, which is why `check_normal`'s documented
    /// fast-path limit also applies here.
    #[test]
    fn classify_ranks_the_engine_bases_like_lookup() {
        for (word, want) in [
            ("aavistamattoman", "aavistaa"),
            ("ahdistuneen", "ahdistua"),
            ("heränneen", "herätä"),
            ("aamuista", "aamu"),
        ] {
            let report = find_bases_for_check(lexicon(), catalogue(), word);
            match classify(report) {
                CheckOutcome::Inflected(bases) => {
                    assert_eq!(bases.first().copied(), Some(want), "{word}: {bases:?}")
                }
                other => panic!("{word}: expected Inflected, got {other:?}"),
            }
        }
    }

    /// A sanalista row the build script rejects must not be reachable as a
    /// lemma, an inflected form, or a suggestion.
    #[test]
    fn a_rejected_dictionary_row_is_not_a_word() {
        for word in [
            "viti",
            "vitejä",
            "vitinsä",
            "videikseen",
            "vinka",
            "vinkaanne",
            "aivotus",
            "aivotusten",
            "elttaantua",
            "elttaantuneen",
            "elttaantuminen",
            "pingoittuivat",
        ] {
            assert!(
                matches!(check(word), CheckOutcome::Unknown),
                "{word} should be unknown"
            );
            assert!(
                is_known_word_fast(word).is_none(),
                "{word} should miss the fast path"
            );
        }
        assert!(
            !suggest("viti", 8, 2)
                .iter()
                .any(|candidate| candidate == "viti"),
            "a rejected row must not be suggested"
        );
        // The standard lemma is not rejected and is the curated correction
        // for the variant's genitive plural.
        assert!(
            matches!(check("aivoitus"), CheckOutcome::Lemma),
            "aivoitus must stay a lemma"
        );
        assert_eq!(
            suggest("aivotusten", 8, 2).first().map(String::as_str),
            Some("aivoitusten"),
        );
        assert_eq!(
            suggest("elttaantuneen", 8, 2).first().map(String::as_str),
            Some("eltaantuneen"),
        );
        assert_eq!(
            suggest("pingoittuivat", 8, 2).first().map(String::as_str),
            Some("pingottuivat"),
        );
    }

    /// `suggest` returns candidates within `max_distance` for a typo, ranked
    /// by distance.
    #[test]
    fn suggest_returns_close_matches_for_a_typo() {
        // "taloo" is one edit from "talo".
        let s = suggest("taloo", 5, 2);
        assert!(
            s.iter().any(|w| w == "talo"),
            "taloo -> [..., talo, ...]: {s:?}"
        );

        // "koirra" is one edit from "koira".
        let s = suggest("koirra", 5, 2);
        assert!(s.iter().any(|w| w == "koira"), "{s:?}");

        // "jooksee" is one insertion away from "juoksee" (the 3sg
        // present of lemma "juosta"). The lemma starts with "j" but
        // the head prefix "juo" differs from the input's "joo" by
        // one substitution -- the lemma would be missed by the
        // prefix-walk alone. The phase 1.5 head-1 walk + inflection
        // candidate pool catches it.
        let s = suggest("jooksee", 5, 2);
        assert!(
            s.iter().any(|w| w == "juoksee"),
            "jooksee -> [..., juoksee, ...]: {s:?}"
        );

        // Beyond `max_distance` nothing comes back.
        let s = suggest("zzz", 3, 1);
        assert!(s.is_empty(), "{s:?}");
    }

    /// `levenshtein` agrees with the textbook distances on a few inputs.
    #[test]
    fn levenshtein_matches_textbook_distances() {
        assert_eq!(levenshtein("kitten", "sitting", usize::MAX), 3);
        assert_eq!(levenshtein("flaw", "lawn", usize::MAX), 2);
        assert_eq!(levenshtein("", "", usize::MAX), 0);
        assert_eq!(levenshtein("a", "", usize::MAX), 1);
        assert_eq!(levenshtein("abc", "abc", usize::MAX), 0);
    }

    /// `levenshtein` returns `max_distance + 1` when the running minimum
    /// has already blown past the budget -- a guard against the
    /// `O(|a|*|b|)` worst case on long inputs.
    #[test]
    fn levenshtein_bails_out_early() {
        // Two strings with no shared prefix; the row-0 minimum never drops
        // low enough within the budget.
        let d = levenshtein("aaaaaaaaaaaaaaaa", "zzzzzzzzzzzzzzzz", 2);
        assert!(d > 2);
    }

    /// The Damerau extension recognises adjacent transpositions as a
    /// single edit. `teh` -> `the` is the textbook example.
    #[test]
    fn damerau_treats_adjacent_transpositions_as_one_edit() {
        assert_eq!(levenshtein("teh", "the", usize::MAX), 1);
        assert_eq!(levenshtein("ab", "ba", usize::MAX), 1);
        assert_eq!(levenshtein("abc", "bac", usize::MAX), 1);
        // Two swaps, no overlap: still 2.
        assert_eq!(levenshtein("abcd", "badc", usize::MAX), 2);
        // A rotation is two swaps under OSA (no overlap allowed).
        assert_eq!(levenshtein("abc", "cba", usize::MAX), 2);
    }

    /// Standard Levenshtein distances are unchanged when no transposition
    /// is involved, so callers and the existing test corpus that pin
    /// those values keep working.
    #[test]
    fn damerau_preserves_standard_distances() {
        assert_eq!(levenshtein("kitten", "sitting", usize::MAX), 3);
        assert_eq!(levenshtein("flaw", "lawn", usize::MAX), 2);
        assert_eq!(levenshtein("", "", usize::MAX), 0);
        assert_eq!(levenshtein("a", "", usize::MAX), 1);
        assert_eq!(levenshtein("abc", "abc", usize::MAX), 0);
    }

    /// `levenshtein_with_ops` can now actually emit `EditOp::Transpose`,
    /// not just dead-code Transpose branches. `teh` -> `the` is a single
    /// transposition.
    #[test]
    fn levenshtein_with_ops_emits_transpose() {
        let (d, ops) = levenshtein_with_ops("teh", "the");
        assert_eq!(d, 1);
        assert!(
            ops.iter().any(|op| matches!(op, EditOp::Transpose { a, b } if *a == 'e' && *b == 'h' || *a == 'h' && *b == 'e')),
            "expected a Transpose, got {ops:?}"
        );
    }

    /// `is_keyboard_adjacent` resolves neighbours per the QWERTY layout
    /// table. Two letters on adjacent keys are adjacent; a vowel and a
    /// consonant on different rows are not.
    #[test]
    fn is_keyboard_adjacent_recognises_neighbours() {
        // Same row, one key apart.
        assert!(is_keyboard_adjacent('a', 's'));
        assert!(is_keyboard_adjacent('s', 'a'));
        // Diagonal -- 'a' is below 'q' and 'w' in the layout, so 'q' is
        // a neighbour (top row -> home row, one column over).
        assert!(is_keyboard_adjacent('a', 'q'));
        assert!(is_keyboard_adjacent('q', 'a'));
        // 'a' and 'l' are not adjacent (different rows, three columns
        // apart).
        assert!(!is_keyboard_adjacent('a', 'l'));
        // Capital letters are normalised.
        assert!(is_keyboard_adjacent('A', 'S'));
        // 'a' and 'ä' are adjacent in the Finnish row of KEYBOARD_ADJACENT.
        assert!(is_keyboard_adjacent('a', 'ä'));
    }

    /// `keyboard_bonus` returns the tie-break weight for a single adjacent
    /// substitution, and 0 for any other distance-1 edit shape.
    #[test]
    fn keyboard_bonus_only_for_single_adjacent_substitution() {
        // Adjacent single-substitution: positive bonus. `l` and `p` are
        // QWERTY neighbours (home row + top row, same column).
        assert_eq!(keyboard_bonus("talo", "tapo"), KEYBOARD_ADJACENT_BONUS);
        // Far single-substitution: zero. `l` and `x` are not adjacent
        // (different rows, two columns apart).
        assert_eq!(keyboard_bonus("talo", "taxo"), 0.0);
        // Insertion: zero, even if the inserted char is adjacent to its
        // neighbour.
        assert_eq!(keyboard_bonus("talo", "tallo"), 0.0);
        // Deletion: zero.
        assert_eq!(keyboard_bonus("talo", "tal"), 0.0);
        // Two differing positions: zero.
        assert_eq!(keyboard_bonus("talo", "taxu"), 0.0);
    }

    /// `suggest_detailed` preserves the same word order as `suggest` for
    /// every well-known case. Slice-0 contract: the new API is byte-equal
    /// to the old API in the words it surfaces.
    #[test]
    fn suggest_detailed_matches_suggest_words() {
        for word in ["taloo", "koirra", "jooksee", "taloa", "puhtaan"] {
            let words_old: Vec<String> = suggest(word, 5, 2);
            let words_new: Vec<String> = suggest_detailed(word, 5, 2)
                .into_iter()
                .map(|s| s.word)
                .collect();
            assert_eq!(
                words_old, words_new,
                "word order changed for {word}: old={words_old:?} new={words_new:?}"
            );
        }
    }

    /// `suggest_detailed` annotates the `jooksee` case with the lemma the
    /// engine actually used to produce `juoksee`.
    #[test]
    fn suggest_detailed_juoksee_carries_lemma() {
        let out = suggest_detailed("jooksee", 5, 2);
        let juoksee = out
            .iter()
            .find(|s| s.word == "juoksee")
            .expect("juoksee must appear in suggestions for jooksee");
        // The lemma the engine emitted was `juosta`, regardless of which
        // source variant carried it.
        let lemma = juoksee
            .lemma
            .as_deref()
            .expect("juoksee's suggestion must carry a lemma");
        assert_eq!(lemma, "juosta");
        // It must be one of the morphologically-grounded sources — either
        // a generated form (the fast path) or a neighbour lemma. Slice 0
        // uses GeneratedForm; slice 1 may surface NeighbourLemma too.
        assert!(matches!(
            juoksee.source,
            SuggestionSource::GeneratedForm { .. } | SuggestionSource::NeighbourLemma
        ));
        // Edit distance from "jooksee" is 1 (substitute `o` → `u` at index 1).
        assert_eq!(juoksee.edit_distance, 1);
        // And the rank is 0 because nothing closer exists.
        assert_eq!(juoksee.original_rank, 0);
    }

    /// `suggest_detailed` does not crash on an empty input or on a zero
    /// `max_n`. The existing `suggest` also handles both, so the new
    /// entry point has to.
    #[test]
    fn suggest_detailed_handles_empty_inputs() {
        assert!(suggest_detailed("", 5, 2).is_empty());
        assert!(suggest_detailed("talo", 0, 2).is_empty());
        assert!(suggest_detailed("xyzzy", 5, 0).is_empty());
    }

    /// `--lemma-set--`
    #[test]
    fn known_lemma_is_recognised() {
        assert!(is_lemma("talo"));
        assert!(is_lemma("koira"));
        assert!(is_lemma("kissa"));
        assert!(is_lemma("puhdas"));
    }

    #[test]
    fn ascii_capitals_lowercase() {
        assert!(is_lemma("TALO"));
        assert!(is_lemma("Talo"));
    }

    #[test]
    fn inflected_forms_are_not_lemmas() {
        // "taloa" inflects from "talo"; "koiralle" from "koira";
        // "puhtaan" is the genitive of "puhdas" via gradation. None
        // of these are themselves in the sanalista.
        assert!(!is_lemma("taloa"));
        assert!(!is_lemma("koiralle"));
        assert!(!is_lemma("puhtaan"));
    }

    /// A colloquial `-s` (`ymmärrätkös`) is spoken-language: the checker
    /// does not accept it, and the on-click popup offers the s-less host
    /// (`ymmärrätkö`) first. A word that genuinely ends in `-s`
    /// (`kahdeksas`, `ulos`) is a sanalista lemma and stays accepted.
    #[test]
    fn colloquial_s_is_flagged_and_suggests_the_s_less_host() {
        for word in ["ymmärrätkös", "näetkös", "tiedätkös", "mitäs", "kukas"] {
            assert!(
                is_known_word(word).is_none(),
                "{word} is a colloquial -s form and should be flagged"
            );
        }
        for word in ["kahdeksas", "kolmas", "ulos", "alas"] {
            assert!(
                is_known_word(word).is_some(),
                "{word} is a genuine -s lemma and should be accepted"
            );
        }
        assert_eq!(
            suggest_over_lemmas("ymmärrätkös", 4, 2)
                .first()
                .map(String::as_str),
            Some("ymmärrätkö"),
        );
        assert_eq!(
            suggest_over_lemmas("mitäs", 4, 2)
                .first()
                .map(String::as_str),
            Some("mitä"),
        );
    }

    /// `is_known_word` covers the productive noun cases and
    /// the verb personal forms. Each call is at most
    /// `1 + sum_k(|stem map|) * HashSet::contains` queries --
    /// the stripper is the lib's `ENDINGS` / `VERB_ENDINGS`
    /// tables (sourced from the kotus oracles) and the verify
    /// is `inflect_all(base, ..., pos, false)`. Used by the
    /// per-textbox render's first-pass misspell detector so
    /// correctly inflected words don't get flagged as misspell.
    #[test]
    fn known_word_recognises_inflected_via_stem_hashmap() {
        // Inessive: -ssa / -ssä.
        assert_eq!(is_known_word("talossa").as_deref(), Some("talo"));
        assert_eq!(is_known_word("puussa").as_deref(), Some("puu"));
        assert_eq!(is_known_word("puistossa").as_deref(), Some("puisto"));
        // Elative: -sta / -stä.
        assert_eq!(is_known_word("talosta").as_deref(), Some("talo"));
        // Adessive: -lla / -llä.
        assert_eq!(is_known_word("talolla").as_deref(), Some("talo"));
        // Allative: -lle.
        assert_eq!(is_known_word("talolle").as_deref(), Some("talo"));
        // Ablative: -lta / -ltä.
        assert_eq!(is_known_word("talolta").as_deref(), Some("talo"));
        // Trans-lative: -ksi.
        assert_eq!(is_known_word("taloksi").as_deref(), Some("talo"));
        // Class-2 noun (koira -> koirassa): the `-a` partitive
        // gets stripped with the case marker.
        assert_eq!(is_known_word("koirassa").as_deref(), Some("koira"));
        // Plural in -ja / -jä.
        assert_eq!(is_known_word("taloja").as_deref(), Some("talo"));
        // Partitive sg: -ta/-tä.
        assert_eq!(is_known_word("vettä").as_deref(), Some("vesi"));
    }

    /// `is_known_word` still refuses misspellings that happen
    /// to end in a productive suffix.
    #[test]
    fn known_word_still_rejects_typos() {
        assert_eq!(is_known_word("taloo"), None);
        assert_eq!(is_known_word("koirra"), None);
        assert_eq!(is_known_word("xyzzy"), None);
        assert_eq!(is_known_word("fooo"), None);
        // Stems that aren't lemmas either, even if the suffix
        // is real.
        assert_eq!(is_known_word("xyzzyja"), None);
    }

    /// Verb personal forms are recognised by the verb stem
    /// hashmap. Covers class-1 (`istuu`, `puhuu`, `sanoo`,
    /// `lukee`, `onnistuu`), class-2 (`tuntee`), and class-3
    /// gradated (`juoksee`, `menee`, `pesee`) verbs in the
    /// present 3sg. Also covers 1sg / 1pl / past / conditional
    /// / potential / imperative / passive (the verb oracle's
    /// full paradigm set is what the verb stem CSV is built
    /// from).
    #[test]
    fn known_word_covers_verb_personal() {
        // Present 3sg: doubled stem vowel + ending.
        assert_eq!(is_known_word("istuu").as_deref(), Some("istua"));
        assert_eq!(is_known_word("puhuu").as_deref(), Some("puhua"));
        assert_eq!(is_known_word("sanoo").as_deref(), Some("sanoa"));
        assert_eq!(is_known_word("lukee").as_deref(), Some("lukea"));
        assert_eq!(is_known_word("onnistuu").as_deref(), Some("onnistua"));
        // Class 2, present 3sg -ee: infinitive in -ea/-eä.
        assert_eq!(is_known_word("tuntee").as_deref(), Some("tuntea"));
        // Class 3, present 3sg with gradation: doubled
        // consonant + -ee, infinitive has the weak-grade stem.
        assert_eq!(is_known_word("juoksee").as_deref(), Some("juosta"));
        assert_eq!(is_known_word("menee").as_deref(), Some("mennä"));
        // 1sg / past / passive.
        assert_eq!(is_known_word("tiedän").as_deref(), Some("tietää"));
        assert_eq!(is_known_word("tiesin").as_deref(), Some("tietää"));
        assert_eq!(is_known_word("tiedetään").as_deref(), Some("tietää"));
    }
    /// 1sg forms that we do NOT yet cover stay on the slow
    /// `check` path; this is a known gap, not a regression.
    #[test]
    fn known_word_1sg_gaps() {
        // "teen" (1sg of tehdä) -- class 4 -d- drop isn't
        // captured by the stripper. The slow `check` path
        // catches it via the popup.
        // (Asserts commented out; left as a TODO marker.)
    }

    /// `is_known_word` is case-insensitive, matching `is_lemma`.
    #[test]
    fn known_word_is_case_insensitive() {
        assert_eq!(is_known_word("TALOSSA").as_deref(), Some("talo"));
        assert_eq!(is_known_word("Talossa").as_deref(), Some("talo"));
    }

    /// Participle stems in the verb stem map. The bare present
    /// active participle (`muistava` = "remembering"), past
    /// active participle (`tiennyt`), and agent (`lukema`) are
    /// emitted as CSV keys; the runtime noun case stripper peels
    /// genitive/partitive/inessive off them so forms like
    /// `muistavan` (genitive), `muistavaa` (partitive), and
    /// `muistavassa` (inessive) all resolve.
    ///
    /// Note: `tietävä` and `lukema` are themselves lemmas in
    /// the sanalista (adjective / noun) and so resolve to
    /// themselves via the lemma probe, not to their verb base.
    /// The interesting coverage is the case forms that don't
    /// have their own lemma entry.
    #[test]
    fn known_word_recognises_participles() {
        // Bare present active participle (not a separate lemma).
        assert_eq!(is_known_word("muistava").as_deref(), Some("muistaa"));
        // Bare past active participle.
        assert_eq!(is_known_word("lukenut").as_deref(), Some("lukea"));
        // Case forms on the bare participle -- peeled by the noun
        // case stripper at the verb stem map path.
        assert_eq!(is_known_word("muistavan").as_deref(), Some("muistaa"));
        assert_eq!(is_known_word("muistavaa").as_deref(), Some("muistaa"));
        assert_eq!(is_known_word("muistavassa").as_deref(), Some("muistaa"));
        assert_eq!(is_known_word("muistavalle").as_deref(), Some("muistaa"));
        // Past active participle's case forms.
        assert_eq!(is_known_word("lukenutta").as_deref(), Some("lukea"));
        // Agent participle case forms (the bare `lukema` is a
        // lemma so we only test its inflected cases here).
        assert_eq!(is_known_word("lukemaa").as_deref(), Some("lukema"));
    }

    /// Possessive-suffix forms. The case-form underneath the
    /// suffix lands in the lemma set or the noun stem map.
    /// Vowel harmony is covered: both `-nsa` (back) and `-nsä`
    /// (front) strip to a host that resolves.
    #[test]
    fn known_word_recognises_possessives() {
        // 1sg / 2sg / 1pl / 2pl are invariant.
        assert_eq!(is_known_word("taloni").as_deref(), Some("talo"));
        assert_eq!(is_known_word("talossani").as_deref(), Some("talo"));
        assert_eq!(is_known_word("isäni").as_deref(), Some("isä"));
        assert_eq!(is_known_word("talossasi").as_deref(), Some("talo"));
        assert_eq!(is_known_word("talossamme").as_deref(), Some("talo"));
        assert_eq!(is_known_word("talossanne").as_deref(), Some("talo"));
        // 3sg / 3pl: `-nsa` after a back vowel, `-nsä` after
        // a front vowel.
        assert_eq!(is_known_word("talossansa").as_deref(), Some("talo"));
        assert_eq!(is_known_word("isänsä").as_deref(), Some("isä"));
        // The `tuttavuuttanne` shape -- partitive + 2pl
        // possessive on a class-40 noun. Strips `-nne` to
        // `tuttavuutta`, then `-tta` to `tuttavuu` (8 chars),
        // which the noun stem map carries.
        assert_eq!(
            is_known_word("tuttavuuttanne").as_deref(),
            Some("tuttavuus"),
        );
        // `tuttavuutenne` (genitive + 2pl possessive) would
        // need `tuttavuude` + `-n` + `-nne`, but as written
        // it parses as `tuttavuute` + `-nne` and the noun
        // stem map carries the illative-stem form, which
        // the noun case stripper doesn't reach because
        // there is no direct stem-map probe (the verb map
        // has one in path 3a; the noun map does not).
        assert_eq!(is_known_word("tuttavuudenne").as_deref(), Some("tuttavuus"),);
    }

    /// Adverbs and particles the lemma probe accepts directly.
    #[test]
    fn known_word_recognises_adverbs() {
        assert_eq!(is_known_word("niin").as_deref(), Some("niin"));
        assert_eq!(is_known_word("noin").as_deref(), Some("noin"));
    }

    /// Clitic-bearing verb forms. The clitic (`-ko`, `-kin`,
    /// `-kaan`, `-han`, `-pa`) attaches to the inflected verb
    /// form; the fast path strips one layer at a time and
    /// recurses on the host until a lemma or stem-map key
    /// resolves. The host's compound detection stays off so
    /// a single word doesn't trigger two compound walks.
    #[test]
    fn known_word_recognises_clitic_verbs() {
        // 1sg present + clitic -- the clitic strip lands on
        // `nousen` (1sg present of `nousta`), which the verb
        // strip then routes to the verb base.
        assert_eq!(is_known_word("nousenko").as_deref(), Some("nousta"));
        assert_eq!(is_known_word("luenko").as_deref(), Some("lukea"));
        // `sanoin` is ambiguous: it is the 1sg past of
        // `sanoa` AND the instructive plural of `sana` (in
        // the noun form set). The noun strip fires first
        // (`-in` -> `sano` -> noun map -> `sana`) and
        // returns the noun base. Both readings are valid;
        // the demo doesn't care which lemma the spell-check
        // attributes the word to, just that it isn't a
        // misspelling.
        assert_eq!(
            is_known_word("sanoinko").as_deref(),
            Some("sana"),
            "sanoin is also the instructive plural of sana; \
             the noun strip fires first",
        );
        // 3sg past alone (no clitic) -- the bare form, surfaced
        // by the 3sg-past stem map key fix.
        assert_eq!(is_known_word("oli").as_deref(), Some("olla"));
        // 3sg past + clitic.
        assert_eq!(is_known_word("oliko").as_deref(), Some("olla"));
        // Stacked clitics.
        assert_eq!(is_known_word("nousenkin").as_deref(), Some("nousta"));
        // `-kin` after a possessive-bearing form (`isäni` + `-kin`).
        // The host resolves via the noun lemma probe; the
        // outer clitic strip then returns its lemma.
        assert_eq!(is_known_word("isänikin").as_deref(), Some("isä"));
    }

    /// Negation verb finite forms and the words the auxiliary fuses with.
    /// The kotus verb oracle stores negation only as part of a multi-word
    /// form (`ei tule`), so the stripper doesn't see the bare finite
    /// forms, and the fused conjunctions are not a verb paradigm at all.
    /// The closed table is [`crate::expand::negation`], checked at the top
    /// of `is_known_word_with_compound`.
    #[test]
    fn known_word_recognises_negation_verb() {
        assert_eq!(is_known_word("ei").as_deref(), Some("ei"));
        assert_eq!(is_known_word("en").as_deref(), Some("ei"));
        assert_eq!(is_known_word("et").as_deref(), Some("ei"));
        assert_eq!(is_known_word("emme").as_deref(), Some("ei"));
        assert_eq!(is_known_word("ette").as_deref(), Some("ei"));
        assert_eq!(is_known_word("eivät").as_deref(), Some("ei"));
        // Clitic-bearing negation forms: the clitic strip
        // path peels `-ko`/`-kö` off and lands on a finite
        // negation form.
        assert_eq!(is_known_word("eikö").as_deref(), Some("ei"));
        assert_eq!(is_known_word("enkö").as_deref(), Some("ei"));
        assert_eq!(is_known_word("etkö").as_deref(), Some("ei"));
        assert_eq!(is_known_word("eivätkö").as_deref(), Some("ei"));
        // The negative clitic `-kä`.
        assert_eq!(is_known_word("enkä").as_deref(), Some("ei"));
        assert_eq!(is_known_word("eivätkä").as_deref(), Some("ei"));
        // The fused conjunctions name their own lemma.
        assert_eq!(is_known_word("ettet").as_deref(), Some("että"));
        assert_eq!(is_known_word("ellette").as_deref(), Some("ellei"));
    }

    /// The translative stem `-kse-` plus a possessive. The stem is not a
    /// word (`omakse-`, `palkakse-`), so the plain possessive stripper
    /// cannot reach the base; `palkka` is graded (`palka-`) and needs the
    /// noun stem map. Covers a noun, a graded noun, an adjective and the
    /// long 1st infinitive of a verb.
    #[test]
    fn known_word_recognises_translative_possessive() {
        for (word, base) in [
            ("omakseen", "oma"),
            ("palkakseni", "palkka"),
            ("talokseni", "talo"),
            ("taloksesi", "talo"),
            ("ymmärtääkseni", "ymmärtää"),
            ("saattaakseen", "saattaa"),
        ] {
            assert_eq!(is_known_word(word).as_deref(), Some(base), "{word}");
        }
    }

    /// Suggestion: an old-spelling word the lib does not
    /// recognise as a known form. `suggest` should still
    /// find the modern spelling within a small edit distance
    /// and rank it as the top candidate. The case-insensitive
    /// candidate walk in [`find_bases_with`] ensures the
    /// head-1 / prefix walks land on `bar` (lowercase) even
    /// when the input is sentence-initial `Baromeetteri`,
    /// and the case-stable edit-distance check in
    /// [`suggest_detailed`] removes the bonus edit cost the
    /// capital `B` would otherwise add. The returned suggestion keeps the
    /// input's capital (`Barometri`), not the lexicon spelling.
    #[test]
    fn known_word_suggests_modern_spelling() {
        let s = suggest("Baromeetteri", 5, 3);
        assert_eq!(
            s.first().map(|s| s.as_str()),
            Some("Barometri"),
            "expected modern spelling to top the suggestions, got {:?}",
            s,
        );
    }

    /// words joined orthographically; the head is the right end.
    /// Each half must resolve via the lemma HashSet or the noun /
    /// verb stem maps. The head's lemma is returned.
    ///
    /// All examples here are compounds whose halves are in the
    /// stem-map coverage. Cases that need the slow path stay
    /// covered by `check_normal` gate 5 and `crate::compound::
    /// analyze`. Documented gaps include:
    ///
    ///   * **genitive modifiers** whose stem is missing from the
    ///     noun map (`kahdentoista` = `kahden` + `toista`,
    ///     neither is in the noun map as a stem).
    ///   * **possessive suffixes** not in `ENDINGS` (`tuttavuus
    ///     tanne` = `tuttavuus` + `-tanne`).
    ///   * **i-link compounds** where the head's orthographic
    ///     stem differs from the parent string
    ///     (`täysikäisillä` would need `ikäisi` not `käisi`).
    ///   * **nested compounds** deeper than the stack bound
    ///     (`MAX_COMPOUND_DEPTH`), or whose inner modifier is not a
    ///     lemma / stem-map hit on its own.
    #[test]
    fn known_word_recognises_compounds() {
        // `vesimies` and `tietokone` are themselves listed as
        // lemmas in the sanalista -- the lemma probe wins, no
        // compound detection needed. The result is the lemma
        // itself, which is fine.
        assert_eq!(is_known_word("vesimies").as_deref(), Some("vesimies"));
        assert_eq!(is_known_word("tietokone").as_deref(), Some("tietokone"));
        // `kylmäkahvi` is NOT a lemma; the split at byte 5 gives
        // two real lemmas `kylmä` + `kahvi`, the head's lemma
        // `kahvi` is returned.
        assert_eq!(is_known_word("kylmäkahvi").as_deref(), Some("kahvi"));
        // Same for `viinikauppa`.
        assert_eq!(is_known_word("viinikauppa").as_deref(), Some("kauppa"));
        // Three-part compound where the head is a 4th-infinitive
        // nominalisation (lemma in the sanalista). The middle
        // part `tietokone` is itself a listed lemma, so the
        // split lands at the head boundary `tietokone` +
        // `ohjelmointi` rather than recursing three levels.
        assert_eq!(
            is_known_word("tietokoneohjelmointi").as_deref(),
            Some("ohjelmointi")
        );
    }

    /// A compound head may carry the contracted third-person possessive
    /// (`kulultaan` = `kululta` + `-an`, of `kulku`). The fast path's
    /// possessive list must strip it or the whole compound
    /// (`juonen` + `kulultaan`) is rejected even though `check_normal`
    /// resolves the head.
    #[test]
    fn known_word_recognises_a_compound_with_a_contracted_possessive_head() {
        // `kulu` (expense) and `kulku` (course) both have the ablative
        // possessive `kulultaan`, so the fast path may name either; the
        // guarantee under test is that the form is recognised at all.
        let head = is_known_word("kulultaan");
        assert!(
            matches!(head.as_deref(), Some("kulu" | "kulku")),
            "kulultaan -> {head:?}"
        );
        assert!(
            is_known_word("juonenkulultaan").is_some(),
            "juonen + kulultaan was not split"
        );
    }

    /// The possessive suffix belongs to the whole compound, so the head has
    /// to come from the host, not from the possessive's own tail:
    /// `aamujuomanne` is `aamu` + `juoma` + `-nne`, not a word headed by
    /// `manne`.
    #[test]
    fn a_compound_possessive_head_is_not_the_possessive_tail() {
        let head = super::known::is_known_compound("aamujuomanne");
        assert!(head.is_some(), "aamu + juoma + -nne was not split");
        assert_ne!(head.as_deref(), Some("manne"));
    }

    /// Compound detection is bounded: the left modifier may recurse, but
    /// only up to `MAX_COMPOUND_DEPTH`, so a deeply-compounded input that
    /// doesn't fit the fast-path coverage simply returns None rather than
    /// spinning.
    #[test]
    fn compound_recursion_bounded() {
        // Empty halves are skipped -- no panics, no infinite loop.
        assert_eq!(is_known_word(""), None);
        // Single lemmas skip compound detection entirely.
        assert_eq!(is_known_word("talo"), Some("talo".into()));
        // A long compound whose `-inen`/`-mainen` head has a real base now
        // resolves (the derivation layer), and a nonsensical one still
        // returns None -- either way the call is bounded.
        assert!(is_known_word("koirankoppamainen").is_some());
        assert_eq!(is_known_word("xyzzykoppamainen"), None);
    }

    #[test]
    fn short_compounds_and_minen_stems_are_known_in_the_fast_path() {
        // Six/seven-letter compounds are below `is_known_compound_fast`'s
        // length floor; `esi` is a bound modifier.
        assert_eq!(is_known_word_fast("esisana").as_deref(), Some("sana"));
        // A `-minen` action noun's compound stem is `-mis-`.
        assert!(is_known_word("edistämisvaroista").is_some());
        assert!(is_known_word_fast("edistämisvaroista").is_some());
    }

    #[test]
    fn missing_i_verb_forms_are_not_compounds() {
        // `vetäsi`/`kysäsi` are one-letter-short spellings of the
        // conditional `vetäisi`/`kysäisi`. The compound detector used to
        // split them at the final `si` and name the pronoun head `tämä` or
        // `sä`, which made the checker accept a misspelling.
        let nouns = HashSet::new();
        for word in ["vetäsi", "kysäsi"] {
            assert_eq!(is_known_word(word), None, "{word}");
            assert_eq!(is_known_word_fast(word), None, "{word}");
            assert!(check_fast(word, &nouns).is_none(), "{word}");
            assert!(matches!(
                crate::expand::check_normal::check_normal_fast(word, false),
                crate::expand::check_normal::NormalizedOutcome::Unknown
            ));
        }
        // The real conditional forms stay accepted.
        for word in ["vetäisi", "kysäisi"] {
            assert!(is_known_word(word).is_some(), "{word}");
        }
    }

    #[test]
    fn unknown_words_are_unknown() {
        assert!(!is_lemma("xyzzy"));
        assert!(!is_lemma(""));
    }

    /// `--check-fast--`
    #[test]
    fn check_fast_returns_lemma_for_a_known_lemma() {
        let extra: HashSet<String> = HashSet::new();
        let r = check_fast("talo", &extra);
        assert!(matches!(r, Some(CheckOutcome::Lemma)));
    }

    #[test]
    fn check_fast_returns_inflected_for_an_inflected_form() {
        let extra: HashSet<String> = HashSet::new();
        // `taloa` inflects from `talo` via find_bases_with.
        let r = check_fast("taloa", &extra);
        assert!(matches!(r, Some(CheckOutcome::Inflected(_))));
    }

    #[test]
    fn check_fast_returns_none_for_unknown() {
        let extra: HashSet<String> = HashSet::new();
        assert!(check_fast("xyzzy", &extra).is_none());
    }

    #[test]
    fn check_fast_recognises_a_compound_with_a_possessive() {
        let extra: HashSet<String> = HashSet::new();
        // `todistelu` + `kyvynsä`; the engine walk has no compound split,
        // the compound-aware fallback does.
        assert!(check_fast("todistelukykynsä", &extra).is_some());
        assert!(check_fast("todistelukykynnsä", &extra).is_none());
    }

    #[test]
    fn correctly_spelled_compounds_need_no_suggestions() {
        let nouns: HashSet<String> = HashSet::new();
        // These are the long compounds whose head-1 walk used to dominate
        // suggestion latency. Recognition now names the head before the
        // walk, and a correctly written word returns no corrections.
        for word in [
            "kahvikekkereitä",
            "limonadipullojen",
            "turhamielisyyksissäni",
            "henkiruumiiseenne",
            // Three-part compounds: the modifier is itself a compound
            // (`sala` + `kirje` + `vaihto`, `henki` + `vartio` + `väki`).
            "salakirjevaihto",
            "salakirjevaihdossa",
            "henkivartioväki",
            "henkivartioväen",
        ] {
            assert!(check_fast(word, &nouns).is_some(), "{word} should be known");
            assert!(
                suggest(word, 5, 2).is_empty(),
                "{word} is correctly spelled"
            );
        }
        // A non-compound derived degree is not swallowed by the compound
        // short-circuit; its own suggestions still come through.
        assert!(
            suggest("salaperäisemmäksi", 5, 2)
                .iter()
                .any(|s| s == "salaperäisemmäksi")
        );
    }

    #[test]
    fn suggest_expands_oo_to_verb_endings() {
        let s = suggest("kohoo", 10, 2);
        eprintln!("{s:?}");
        assert!(s.contains(&"kohoaa".to_string()), "{s:?}");
        assert!(s.contains(&"kohoa".to_string()), "{s:?}");
    }

    #[test]
    fn suggest_offers_the_standard_genitive_plural() {
        let s = suggest("lammasten", 10, 3);
        assert!(s.contains(&"lampaiden".to_string()), "{s:?}");
    }

    #[test]
    fn suggest_repairs_a_doubled_letter_in_a_compound() {
        let s = suggest("todistelukykynnsä", 5, 2);
        assert_eq!(s.first().map(String::as_str), Some("todistelukykynsä"));
    }

    #[test]
    fn kehoittaa_forms_are_illegal_and_suggest_kehottaa() {
        for (wrong, right) in [
            ("kehoittaa", "kehottaa"),
            ("kehoitan", "kehotan"),
            ("kehoitti", "kehotti"),
            ("kehoittavasti", "kehottavasti"),
            ("kehoitus", "kehotus"),
        ] {
            assert!(matches!(check(wrong), CheckOutcome::Unknown), "{wrong} is not a word");
            assert_eq!(
                suggest(wrong, 5, 2).first().map(String::as_str),
                Some(right),
                "{wrong}"
            );
        }
    }

    #[test]
    fn ennenkuin_is_a_misspelling_of_the_two_word_phrase() {
        assert_eq!(is_known_word("ennenkuin"), None);
        assert_eq!(
            suggest("ennenkuin", 8, 2).first().map(String::as_str),
            Some("ennen kuin"),
        );
        assert_eq!(
            suggest("aivankuin", 8, 2).first().map(String::as_str),
            Some("aivan kuin"),
        );
        assert_eq!(
            suggest("avainkuin", 8, 2).first().map(String::as_str),
            Some("aivan kuin"),
        );
        assert_eq!(
            suggest("niinkuin", 8, 2).first().map(String::as_str),
            Some("niin kuin"),
        );
        assert_eq!(is_known_word("yllinkyllin"), None);
        assert_eq!(
            suggest("yllinkyllin", 8, 2).first().map(String::as_str),
            Some("yltäkyllin"),
        );
        assert_eq!(is_known_word("sentähden"), None);
        assert_eq!(
            suggest("sentähden", 8, 2).first().map(String::as_str),
            Some("sen tähden"),
        );
        assert!(!is_known_word("ennenin").is_some_and(|b| b == "ennen"));
    }

    #[test]
    fn reported_standard_compound_and_degree_forms_are_known() {
        for (word, base) in [
            ("perehtyneempi", "perehtynyt"),
            ("molemmille", "molemmat"),
            ("kyrmyniskoja", "kyrmyniska"),
            ("housuntaskuissa", "housuntasku"),
            ("herttaisinta", "herttainen"),
            ("pihtejä", "pihdit"),
        ] {
            assert_eq!(is_known_word(word).as_deref(), Some(base), "{word}");
            assert_eq!(is_known_word_fast(word).as_deref(), Some(base), "{word}");
        }
    }

    #[test]
    fn the_frequentative_illa_of_avata_is_known() {
        // `availla` is the productive frequentative of `avata` (`FREQ_ILLA`),
        // not a Kotus lemma. The base row carries the flag so the derived
        // class-67 verb and its forms resolve in the fast path too.
        for word in [
            "availla",
            "availlaan",
            "availlen",
            "availee",
            "availeva",
            "availlut",
        ] {
            assert_eq!(is_known_word(word).as_deref(), Some("availla"), "{word}");
            assert_eq!(
                is_known_word_fast(word).as_deref(),
                Some("availla"),
                "{word}"
            );
        }
        assert!(check_fast("availla", &HashSet::new()).is_some());
    }

    #[test]
    fn illative_possessives_of_koti_are_known() {
        // `kotiinsa` is `kotiin` + the contracted 3rd-person possessive.
        // The fast path tried the `kotiin` lemma's paradigm and missed the
        // possessive; the noun stem map behind the illative names `koti`.
        for word in ["kotiinsa", "kotiinne", "kotiimme", "kotiini", "kotiisi"] {
            assert_eq!(is_known_word(word).as_deref(), Some("koti"), "{word}");
            assert_eq!(is_known_word_fast(word).as_deref(), Some("koti"), "{word}");
        }
    }

    #[test]
    fn a_function_word_surface_still_heads_a_content_compound() {
        // `varresta` is a postposition lemma and the elative of `varsi`.
        // The compound detector used to stop at the postposition reading
        // and reject `lankivarresta`/`langanvarresta`, even though
        // `lankivarsi` and `lankivartta` were already known.
        for word in [
            "lankivarresta",
            "lankavarresta",
            "langanvarresta",
            "lankivarteen",
        ] {
            assert_eq!(is_known_word(word).as_deref(), Some("varsi"), "{word}");
            assert_eq!(is_known_word_fast(word).as_deref(), Some("varsi"), "{word}");
        }
    }

    #[test]
    fn a_truncated_uus_noun_is_not_a_suggestion() {
        // `irstaisuus` is a real derived noun (better written `irstailu`),
        // but `irstaisuu` is not a form of it, and the old
        // `irstai` + `suu` compound split used to offer it first.
        assert!(!is_known_word("irstaisuu").is_some());
        let s = suggest("irstaisuus", 12, 5);
        assert!(
            !s.iter().any(|w| w == "irstaisuu"),
            "irstaisuu must not be suggested: {s:?}"
        );
    }

    #[test]
    fn ahertelin_suggests_the_base_verb_past() {
        let s = suggest("ahertelin", 8, 3);
        assert!(s.contains(&"ahersin".to_string()), "{s:?}");
    }

    #[test]
    fn liektiseva_suggests_the_participle() {
        let s = suggest("liektisevä", 8, 3);
        assert!(s.contains(&"liekehtivä".to_string()), "{s:?}");
    }

    #[test]
    fn silmaranpayksen_suggests_the_transposed_compound() {
        let s = suggest("silmäränpäyksen", 8, 2);
        assert_eq!(s.first().map(String::as_str), Some("silmänräpäyksen"));
    }

    #[test]
    fn suggestions_keep_the_input_capitalization() {
        assert_eq!(
            suggest("Tlao", 5, 2).first().map(String::as_str),
            Some("Talo")
        );
        assert_eq!(
            suggest("TALO", 5, 2).first().map(String::as_str),
            Some("TALO")
        );
    }

    #[test]
    fn maiskauttelee_suggests_the_frequentative() {
        assert_eq!(
            suggest("maiskauttelee", 8, 2).first().map(String::as_str),
            Some("maiskuttelee")
        );
    }

    #[test]
    fn kolahdutti_suggests_the_standard_past() {
        assert_eq!(
            suggest("kolahdutti", 8, 2).first().map(String::as_str),
            Some("kolautti")
        );
    }

    #[test]
    fn a_plural_genitive_jen_compound_suggests_the_iden_form() {
        // `vapaakomppania` is a classless compound, so the head/tail walk
        // never generates `vapaakomppanioiden` from the input. The `-jen`
        // -> `-iden` repair supplies the whole compound.
        assert_eq!(
            suggest("vapaakomppaniojen", 8, 2)
                .first()
                .map(String::as_str),
            Some("vapaakomppanioiden")
        );
    }

    #[test]
    fn palvelijainsa_suggests_the_modern_possessive_genitive() {
        // The archaic `-in` plural genitive + `-sa` is far down the ordinary
        // rank; the morphological repair names `palvelijoidensa` directly.
        assert!(is_known_word("palvelijainsa").is_none());
        assert_eq!(
            suggest("palvelijainsa", 8, 3).first().map(String::as_str),
            Some("palvelijoidensa")
        );
    }

    #[test]
    fn asukkasten_suggests_a_legal_genitive() {
        // `asukasten` is a legal plural genitive of `asukas`; the doubled
        // `asukkasten` is not. The class-41 `-sten` cell must use the
        // citation stem, and the suggestion must point at the legal form.
        assert!(is_known_word("asukasten").is_some());
        assert!(is_known_word("asukkasten").is_none());
        assert_eq!(
            suggest("asukkasten", 8, 3).first().map(String::as_str),
            Some("asukasten")
        );
    }

    #[test]
    fn vanhanaikanen_and_salamyhkaa_suggest_the_standard_forms() {
        assert_eq!(
            suggest("vanhanaikanen", 8, 2).first().map(String::as_str),
            Some("vanhanaikainen")
        );
        assert_eq!(
            suggest("salamyhkää", 8, 2).first().map(String::as_str),
            Some("salamyhkäisesti")
        );
    }

    #[test]
    fn supplemental_selkama_noun_is_known_in_the_fast_path() {
        // `selkämä` was added to the sanalista as a class-10 noun after
        // Wiktionary; the Omorfi-derived form/stem snapshots predate it, so
        // the lexical supplement supplies its case grid to the fast pass.
        for word in [
            "selkämä",
            "selkämän",
            "selkämälle",
            "selkämässä",
            "selkämät",
            "selkämiä",
            "selkämine",
        ] {
            assert_eq!(is_known_word(word).as_deref(), Some("selkämä"), "{word}");
            assert_eq!(
                is_known_word_fast(word).as_deref(),
                Some("selkämä"),
                "{word}"
            );
        }
    }

    #[test]
    fn reported_prose_forms_and_place_names_are_known() {
        for (word, base) in [
            // Participle of `närkästyä`; the user's report named the noun
            // `närkästys`, but the surface is the verb participle.
            ("närkästyneenä", "närkästyä"),
            ("viehkeästi", "viehkeästi"),
            // Proper place names are accepted capitalised through the
            // embedded Wikidata/Omorfi set; the `-lainen` derivative then
            // reuses that name.
            ("Lassila", "Lassila"),
            ("Lassilassa", "Lassila"),
            ("lassilalainen", "lassilalainen"),
        ] {
            assert_eq!(is_known_word(word).as_deref(), Some(base), "{word}");
            assert_eq!(is_known_word_fast(word).as_deref(), Some(base), "{word}");
        }
    }

    #[test]
    fn classless_inen_degrees_name_the_positive() {
        for (word, base) in [
            ("salaperäisempi", "salaperäinen"),
            ("salaperäisemmäksi", "salaperäinen"),
            ("salaperäisimmäksi", "salaperäinen"),
            ("salaperäisin", "salaperäinen"),
        ] {
            assert_eq!(is_known_word(word).as_deref(), Some(base), "{word}");
        }
        // `check_fast` used to take the engine's head/tail guess (`empiä`)
        // for the comparative before the fast degree path named the
        // positive.
        let nouns = HashSet::new();
        match check_fast("salaperäisempi", &nouns) {
            Some(CheckOutcome::Inflected(v)) => {
                assert_eq!(v, vec!["salaperäinen"]);
            }
            other => panic!("expected Inflected(salaperäinen), got {other:?}"),
        }
    }

    #[test]
    fn reported_compounds_and_derived_verbs_are_recognised() {
        for word in [
            "monine",
            "monineen",
            "vedenpitävine",
            "vedenpitävineen",
            "nostatella",
            "nostatelee",
            "jouduttautua",
            "jouduttautui",
            "talonpoikain",
            "mikset",
            "sämpylöityminen",
            "sämpylöitymisen",
            "sämpylöitymisessä",
            "sämpylöitymisprosessimme",
        ] {
            assert!(is_known_word(word).is_some(), "{word}");
        }
        // `jouduttaitui` is a typo of the derived verb's past 3sg.
        assert_eq!(
            suggest("jouduttaitui", 3, 2).first().map(String::as_str),
            Some("jouduttautui"),
        );
        // `minkätähden` is the standard two-word phrase.
        assert_eq!(
            suggest("minkätähden", 3, 2).first().map(String::as_str),
            Some("minkä tähden"),
        );
    }

    #[test]
    fn metafysiikka_is_a_lemma_and_the_double_y_typo_is_suggested() {
        assert!(is_lemma("metafysiikka"));
        assert!(suggest("metafyysiikka", 5, 2).contains(&"metafysiikka".to_string()));
        // `metafyysikko` is the real agent noun; the single-y form is not.
        assert!(suggest("metafysiikko", 5, 2).contains(&"metafyysikko".to_string()));
    }

    #[test]
    fn check_fast_recognises_proper_nouns_case_insensitively() {
        let mut pn: HashSet<String> = HashSet::new();
        pn.insert("helsinki".to_string());
        assert!(matches!(
            check_fast("helsinki", &pn),
            Some(CheckOutcome::Lemma)
        ));
        assert!(matches!(
            check_fast("Helsinki", &pn),
            Some(CheckOutcome::Lemma)
        ));
        assert!(matches!(
            check_fast("HELSINKI", &pn),
            Some(CheckOutcome::Lemma)
        ));
    }

    /// `--suggest-with-state--`. The proper-noun fallback only
    /// fires when plain `suggest` returns empty. With the
    /// case-insensitive candidate walk (the bounded walks in
    /// `find_bases_with` now lowercase the input), `Helsinxi`
    /// already finds `helaini` (a lemma at distance 3) without
    /// needing the fallback. The proper-noun path is reserved
    /// for typos that no lemma is within range of -- where
    /// the user expects a *learned* name, not a random
    /// lemma.
    #[test]
    fn suggest_with_state_falls_back_to_proper_nouns() {
        let mut pn: HashSet<String> = HashSet::new();
        pn.insert("helsinki".to_string());
        // `Helsinxi` finds `helaini` (real lemma) directly,
        // so the proper-noun path is dormant.
        let r = suggest_with_state("Helsinxi", 5, 2, &pn);
        assert!(
            !r.is_empty(),
            "expected some suggestion for 'Helsinxi', got {r:?}"
        );
        // A 3-edit-distance typo no lemma is within range of
        // is what actually exercises the proper-noun
        // fallback.
        let r = suggest_with_state("Heslinki", 5, 2, &pn);
        assert!(
            r.iter().any(|s| s == "Helsinki"),
            "expected 'Helsinki' fallback for 'Heslinki', got {r:?}"
        );
    }

    #[test]
    fn a_learned_proper_noun_is_suggested_capitalised() {
        // A name learned from a mid-sentence `Aloe` is stored lowercased.
        // Asking for the lowercase spelling must return the name's
        // canonical capitalization, not the lowercased lookup key.
        let mut pn: HashSet<String> = HashSet::new();
        pn.insert("aloe".to_string());
        let r = suggest_with_state("aloe", 5, 2, &pn);
        assert!(r.iter().any(|s| s == "Aloe"), "got {r:?}");
        assert!(!r.iter().any(|s| s == "aloe"), "got {r:?}");
    }

    #[test]
    fn suggest_with_state_returns_lexicon_candidates_first() {
        // Even with a proper-noun set populated, "taloo" should still
        // get the standard lexicon suggestion ("talo") from the first
        // pass, not the proper-noun fallback. The proper noun's
        // edit-distance is much larger than `talo`'s.
        let mut pn: HashSet<String> = HashSet::new();
        pn.insert("helsinki".to_string());
        let r = suggest_with_state("taloo", 5, 2, &pn);
        assert!(
            r.iter().any(|s| s == "talo"),
            "expected 'talo' from lexicon, got {r:?}"
        );
    }

    #[test]
    fn suggest_with_state_with_empty_set_matches_plain_suggest() {
        let empty: HashSet<String> = HashSet::new();
        // Same input; the proper-noun fallback is a no-op so the
        // output should be identical to plain `suggest`.
        let a = suggest_with_state("taloo", 5, 2, &empty);
        let b = suggest("taloo", 5, 2);
        assert_eq!(a, b);
    }

    /// A curated correction is licensed by the table, not by the caller's
    /// distance budget, but it must not claim to be an exact match. `0` used
    /// to be stamped on a four-edit repair, which made `edit_distance`
    /// meaningless for every consumer.
    #[test]
    fn a_curated_correction_reports_its_real_distance() {
        let got = suggest_detailed("liektisevä", 5, 0);
        let hit = got
            .iter()
            .find(|s| s.word == "liekehtivä")
            .expect("the curated correction is returned even at distance 0");
        assert_eq!(hit.edit_distance, 4);
        assert!(hit.existing_score > 0.5);
    }

    /// The ordinary pool still obeys the budget: only the curated row may be
    /// farther than `max_distance`.
    #[test]
    fn only_a_curated_correction_exceeds_the_distance_budget() {
        for s in suggest_detailed("liektisevä", 5, 1) {
            if s.word == "liekehtivä" {
                continue;
            }
            assert!(
                s.edit_distance <= 1,
                "{} at {} edits exceeds the budget",
                s.word,
                s.edit_distance
            );
        }
    }

    /// A dropped doubled vowel is a specific typo, not a generic substitution:
    /// `kiväriään` should lead with `kivääriään`, not `kiveriään`.
    #[test]
    fn a_missing_doubled_vowel_wins_the_tie() {
        assert_eq!(
            suggest("kiväriään", 5, 2).first().map(String::as_str),
            Some("kivääriään")
        );
    }

    /// The lemma-only popup path must see the curated clitic answer too.
    #[test]
    fn lemma_popup_uses_the_curated_clitic_answer() {
        assert_eq!(
            suggest_over_lemmas("alottanutkaan", 5, 2),
            vec!["aloittanutkaan"]
        );
    }

    /// The productive feminine `-tar` noun is recognised; the j-spelling
    /// `valtijatar` is not a word, and the lexicalised `valtiatar` is the
    /// correction.
    #[test]
    fn valtijatar_suggests_the_lexicalised_empress() {
        assert!(suggest("valtijatar", 5, 2).contains(&"valtiatar".to_string()));
        assert!(is_known_word("siivoojatar").is_some());
        assert!(is_known_word("valtijatar").is_none());
    }

    /// `salamyhkää` is not a standard form; the curated answer is the adverb
    /// `salamyhkäisesti`, not the adjective's partitive.
    #[test]
    fn salamyhkää_points_at_the_adverb() {
        assert_eq!(
            suggest("salamyhkää", 5, 2).first().map(String::as_str),
            Some("salamyhkäisesti")
        );
    }

    /// The old oracle was built with `--engine`, so an engine bug could write
    /// invented surfaces into the checked-in stem map as if they were real.
    /// `mustaraamattu` used to be recognized through 757 `mudtaraa*` stems
    /// (`mudtaraaakuita`); the map is now rebuilt from the current engine and
    /// filtered against it.
    #[test]
    fn invented_compound_surfaces_are_not_known_words() {
        for word in ["mudtaraaakuita", "hulpmeaaa", "taipae"] {
            assert_eq!(
                is_known_word(word),
                None,
                "{word} came from the stale engine-enriched oracle"
            );
        }
    }
}
