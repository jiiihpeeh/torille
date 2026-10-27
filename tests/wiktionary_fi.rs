//! Declensions checked against the Finnish Wiktionary tables.
//!
//! The Kotus catalogue is the engine's primary source, but the hyphenated
//! class-51 compound `nuori-isäntä` is the case the catalogue model could not
//! stand in for: the engine needs to split it on the hyphen and inflect both
//! parts. The table below is the one fi.wiktionary.org gives for the lemma.

use torille::expand::inflect;
use torille::kotus::Inflection;
use torille::spec::Catalogues;
use torille::tags::PartOfSpeech;

/// The Finnish Wiktionary declension of `nuori-isäntä`.
///
/// Both parts inflect, and the hyphen is kept in every cell.
#[test]
fn nuori_isanta_matches_the_wiktionary_table() {
    let cat = Catalogues::embedded();
    let forms: Vec<String> = inflect(
        "nuori-isäntä",
        Inflection {
            class: 51,
            gradation: None,
        },
        cat,
        PartOfSpeech::Noun,
        false,
    )
    .into_iter()
    .map(|form| form.word)
    .collect();

    for want in [
        // Singular.
        "nuori-isäntä",
        "nuoren-isännän",
        "nuorta-isäntää",
        "nuoreen-isäntään",
        "nuoresta-isännästä",
        // Plural.
        "nuoret-isännät",
        "nuorten-isäntien",
        "nuoria-isäntiä",
        "nuoriin-isäntiin",
        "nuorista-isännistä",
    ] {
        assert!(
            forms.iter().any(|form| form == want),
            "fi.wiktionary.org lists {want}; torille did not produce it"
        );
    }

    // The whole-word catalogue alignment used to invent `nuori-idenkuun`.
    for wrong in ["nuori-idenkuun", "nuori-isäntään"] {
        assert!(
            !forms.iter().any(|form| form == wrong),
            "invented {wrong} instead of using the split"
        );
    }
}
