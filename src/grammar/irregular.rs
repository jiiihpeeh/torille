//! The classes the catalogue's model word cannot stand in for.
//!
//! A class is headed by one model word, and normally the members take the same
//! endings to stems that can be read off the model and its genitive. These
//! classes are the exception: the model of class 26 is `pieni`, but the class
//! holds `nuori` and `suuri` too, and what happens to the `i` of the nominative
//! differs between them. The model is a different word, so its endings say
//! nothing about the member's.
//!
//! The classes below spell their own cases out instead. Each case is a stem
//! rewrite plus an ending, and the ending is given twice where the member's own
//! harmony decides between two spellings, as `nuorta` and `pientä` do.

use crate::kotus::ClassId;
use crate::tags::{Case, Number};

use super::harmony::{Harmony, harmony};
use crate::tags::{Confidence, Form as Generated, Tags};

/// One hardcoded form of a word the class system cannot derive.
///
/// A word whose sanalista class marks it non-inflecting (99) or as an irregular
/// pronoun (101) has no paradigm for the engine to work from, so its forms are
/// spelled out here instead. Each entry names the case and number the spelling
/// realises, in the same tagging the rest of the engine uses.
pub struct WordForm {
    /// The case this spelling realises.
    pub case: Case,
    /// Whether it is the singular or the plural.
    pub number: Number,
    /// The spelling.
    pub word: &'static str,
}

/// Words the class system cannot derive, spelled out by hand.
///
/// The list is a corner of the language the model words do not reach: the
/// personal pronoun `minä` is suppletive (`minun`, `minua`) and `aprilli` is a
/// regular noun that the sanalista happens to mark indeclinable. Every entry is
/// a real word with real forms, and the table is read before the class engine
/// runs, so a lemma on it is never also derived from a class.
pub const IRREGULAR_WORDS: &[(&str, &[WordForm])] = &[
    // minä minun minua minussa minusta minuun minulla minulta minulle
    // minuna minuksi minutta. The first person pronoun is suppletive: the
    // genitive is `minun`, not a regular `minän`, and the partitive `minua`
    // rather than `minää`. It has no plural of its own — its plural is the
    // separate word `me` — so only the singular is listed.
    (
        "minä",
        &[
            WordForm {
                case: Case::Nominative,
                number: Number::Singular,
                word: "minä",
            },
            WordForm {
                case: Case::Accusative,
                number: Number::Singular,
                word: "minut",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Singular,
                word: "minun",
            },
            WordForm {
                case: Case::Partitive,
                number: Number::Singular,
                word: "minua",
            },
            WordForm {
                case: Case::Inessive,
                number: Number::Singular,
                word: "minussa",
            },
            WordForm {
                case: Case::Elative,
                number: Number::Singular,
                word: "minusta",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Singular,
                word: "minuun",
            },
            WordForm {
                case: Case::Adessive,
                number: Number::Singular,
                word: "minulla",
            },
            WordForm {
                case: Case::Ablative,
                number: Number::Singular,
                word: "minulta",
            },
            WordForm {
                case: Case::Allative,
                number: Number::Singular,
                word: "minulle",
            },
            WordForm {
                case: Case::Essive,
                number: Number::Singular,
                word: "minuna",
            },
            WordForm {
                case: Case::Translative,
                number: Number::Singular,
                word: "minuksi",
            },
            WordForm {
                case: Case::Abessive,
                number: Number::Singular,
                word: "minutta",
            },
        ],
    ),
    // aprilli aprillin aprillia aprillissa ... aprillit aprillien aprilleja
    // aprilleissa ... A perfectly ordinary `-i` noun that the sanalista marks
    // indeclinable, given the full paradigm its spelling actually takes.
    (
        "aprilli",
        &[
            WordForm {
                case: Case::Nominative,
                number: Number::Singular,
                word: "aprilli",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Singular,
                word: "aprillin",
            },
            WordForm {
                case: Case::Partitive,
                number: Number::Singular,
                word: "aprillia",
            },
            WordForm {
                case: Case::Inessive,
                number: Number::Singular,
                word: "aprillissa",
            },
            WordForm {
                case: Case::Elative,
                number: Number::Singular,
                word: "aprillista",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Singular,
                word: "aprilliin",
            },
            WordForm {
                case: Case::Adessive,
                number: Number::Singular,
                word: "aprillilla",
            },
            WordForm {
                case: Case::Ablative,
                number: Number::Singular,
                word: "aprillilta",
            },
            WordForm {
                case: Case::Allative,
                number: Number::Singular,
                word: "aprillille",
            },
            WordForm {
                case: Case::Essive,
                number: Number::Singular,
                word: "aprillina",
            },
            WordForm {
                case: Case::Translative,
                number: Number::Singular,
                word: "aprilliksi",
            },
            WordForm {
                case: Case::Abessive,
                number: Number::Singular,
                word: "aprillitta",
            },
            WordForm {
                case: Case::Nominative,
                number: Number::Plural,
                word: "aprillit",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Plural,
                word: "aprillien",
            },
            WordForm {
                case: Case::Partitive,
                number: Number::Plural,
                word: "aprilleja",
            },
            WordForm {
                case: Case::Inessive,
                number: Number::Plural,
                word: "aprilleissa",
            },
            WordForm {
                case: Case::Elative,
                number: Number::Plural,
                word: "aprilleista",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Plural,
                word: "aprilleihin",
            },
            WordForm {
                case: Case::Adessive,
                number: Number::Plural,
                word: "aprilleilla",
            },
            WordForm {
                case: Case::Ablative,
                number: Number::Plural,
                word: "aprilleilta",
            },
            WordForm {
                case: Case::Allative,
                number: Number::Plural,
                word: "aprilleille",
            },
            WordForm {
                case: Case::Essive,
                number: Number::Plural,
                word: "aprilleina",
            },
            WordForm {
                case: Case::Translative,
                number: Number::Plural,
                word: "aprilleiksi",
            },
            WordForm {
                case: Case::Abessive,
                number: Number::Plural,
                word: "aprilleitta",
            },
            WordForm {
                case: Case::Instructive,
                number: Number::Plural,
                word: "aprillein",
            },
        ],
    ),
    // paras parhaan parasta ... The comparative/superlative of `hyvä` is
    // suppletive inside class 41: the `-as` model would give `paraan`, but
    // the real stem is `parhaa-` (`parhaan`, `parhaat`, `parhaita`). The
    // class engine cannot read that off `vieras`, so the paradigm is spelled
    // out, exactly like the pronouns above.
    (
        "paras",
        &[
            WordForm {
                case: Case::Nominative,
                number: Number::Singular,
                word: "paras",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Singular,
                word: "parhaan",
            },
            WordForm {
                case: Case::Partitive,
                number: Number::Singular,
                word: "parasta",
            },
            WordForm {
                case: Case::Essive,
                number: Number::Singular,
                word: "parhaana",
            },
            WordForm {
                case: Case::Translative,
                number: Number::Singular,
                word: "parhaaksi",
            },
            WordForm {
                case: Case::Inessive,
                number: Number::Singular,
                word: "parhaassa",
            },
            WordForm {
                case: Case::Elative,
                number: Number::Singular,
                word: "parhaasta",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Singular,
                word: "parhaaseen",
            },
            WordForm {
                case: Case::Adessive,
                number: Number::Singular,
                word: "parhaalla",
            },
            WordForm {
                case: Case::Ablative,
                number: Number::Singular,
                word: "parhaalta",
            },
            WordForm {
                case: Case::Allative,
                number: Number::Singular,
                word: "parhaalle",
            },
            WordForm {
                case: Case::Abessive,
                number: Number::Singular,
                word: "parhaatta",
            },
            WordForm {
                case: Case::Nominative,
                number: Number::Plural,
                word: "parhaat",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Plural,
                word: "parhaiden",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Plural,
                word: "parhaitten",
            },
            WordForm {
                case: Case::Partitive,
                number: Number::Plural,
                word: "parhaita",
            },
            WordForm {
                case: Case::Essive,
                number: Number::Plural,
                word: "parhaina",
            },
            WordForm {
                case: Case::Translative,
                number: Number::Plural,
                word: "parhaiksi",
            },
            WordForm {
                case: Case::Inessive,
                number: Number::Plural,
                word: "parhaissa",
            },
            WordForm {
                case: Case::Elative,
                number: Number::Plural,
                word: "parhaista",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Plural,
                word: "parhaisiin",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Plural,
                word: "parhaihin",
            },
            WordForm {
                case: Case::Adessive,
                number: Number::Plural,
                word: "parhailla",
            },
            WordForm {
                case: Case::Ablative,
                number: Number::Plural,
                word: "parhailta",
            },
            WordForm {
                case: Case::Allative,
                number: Number::Plural,
                word: "parhaille",
            },
            WordForm {
                case: Case::Abessive,
                number: Number::Plural,
                word: "parhaitta",
            },
            WordForm {
                case: Case::Instructive,
                number: Number::Plural,
                word: "parhain",
            },
        ],
    ),
    // ien ikenen ientä ... ikenet ikenien ikeniä ikeniin. The gum word builds
    // its obliques on `iken-` and keeps `ien-` only for the nominative and
    // partitive singular. The Wiktionary template `fi-subs-sisar|i|en|ken|ä`
    // gives exactly these forms (and both `ienten` and `ikenien` in the plural
    // genitive); no class-32 ending predicts the `k`.
    (
        "ien",
        &[
            WordForm {
                case: Case::Nominative,
                number: Number::Singular,
                word: "ien",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Singular,
                word: "ikenen",
            },
            WordForm {
                case: Case::Partitive,
                number: Number::Singular,
                word: "ientä",
            },
            WordForm {
                case: Case::Essive,
                number: Number::Singular,
                word: "ikeninä",
            },
            WordForm {
                case: Case::Translative,
                number: Number::Singular,
                word: "ikeneksi",
            },
            WordForm {
                case: Case::Inessive,
                number: Number::Singular,
                word: "ikenessä",
            },
            WordForm {
                case: Case::Elative,
                number: Number::Singular,
                word: "ikenestä",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Singular,
                word: "ikeneen",
            },
            WordForm {
                case: Case::Adessive,
                number: Number::Singular,
                word: "ikenellä",
            },
            WordForm {
                case: Case::Ablative,
                number: Number::Singular,
                word: "ikeneltä",
            },
            WordForm {
                case: Case::Allative,
                number: Number::Singular,
                word: "ikenelle",
            },
            WordForm {
                case: Case::Abessive,
                number: Number::Singular,
                word: "ikenettä",
            },
            WordForm {
                case: Case::Nominative,
                number: Number::Plural,
                word: "ikenet",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Plural,
                word: "ikenien",
            },
            WordForm {
                case: Case::Genitive,
                number: Number::Plural,
                word: "ienten",
            },
            WordForm {
                case: Case::Partitive,
                number: Number::Plural,
                word: "ikeniä",
            },
            WordForm {
                case: Case::Essive,
                number: Number::Plural,
                word: "ikeninä",
            },
            WordForm {
                case: Case::Translative,
                number: Number::Plural,
                word: "ikeniksi",
            },
            WordForm {
                case: Case::Inessive,
                number: Number::Plural,
                word: "ikenissä",
            },
            WordForm {
                case: Case::Elative,
                number: Number::Plural,
                word: "ikenistä",
            },
            WordForm {
                case: Case::Illative,
                number: Number::Plural,
                word: "ikeniin",
            },
            WordForm {
                case: Case::Adessive,
                number: Number::Plural,
                word: "ikenillä",
            },
            WordForm {
                case: Case::Ablative,
                number: Number::Plural,
                word: "ikeniltä",
            },
            WordForm {
                case: Case::Allative,
                number: Number::Plural,
                word: "ikenille",
            },
            WordForm {
                case: Case::Abessive,
                number: Number::Plural,
                word: "ikenittä",
            },
            WordForm {
                case: Case::Instructive,
                number: Number::Plural,
                word: "ikenin",
            },
        ],
    ),
];

/// Whether `word` has a hand-written paradigm in [`IRREGULAR_WORDS`] or is
/// realised by [`word_irregular`].
///
/// The reverse index normally skips class 99, but a hand-written word's forms
/// are real and belong in it (`aprilli` is class 99 in the sanalista and
/// declines in full).
pub fn has_handwritten(word: &str) -> bool {
    IRREGULAR_WORDS.iter().any(|(lemma, _)| *lemma == word)
}

/// The hardcoded forms of `lemma`, where the class system cannot derive them.
pub fn word_irregular(lemma: &str) -> Option<&'static [WordForm]> {
    IRREGULAR_WORDS
        .iter()
        .find(|(word, _)| *word == lemma)
        .map(|(_, forms)| *forms)
}

/// Every hardcoded form of `lemma`, tagged, in no particular order.
///
/// A word on the list is a word whose forms are known outright rather than
/// derived, so every form is a plain one.
pub fn word_cases(lemma: &str) -> Vec<Generated> {
    let Some(forms) = word_irregular(lemma) else {
        return Vec::new();
    };
    forms
        .iter()
        .map(|f| {
            Generated::new(f.word.to_string(), Tags::of_number(f.case, f.number))
                .with(Confidence::Plain)
        })
        .collect()
}

/// One rewrite of a lemma on the way to a stem.
pub struct Step {
    /// How many characters to take off the end first.
    pub drop: u8,
    /// What to swap for what, the last occurrence in what is left.
    pub sub: Option<(&'static str, &'static str)>,
    /// What to put on the end afterwards.
    pub add: Option<&'static str>,
}

impl Step {
    const fn keep() -> Self {
        Self {
            drop: 0,
            sub: None,
            add: None,
        }
    }
    /// Swaps the last `from` for `to`, as `kynsi` to `kynn` does.
    const fn sub(from: &'static str, to: &'static str) -> Self {
        Self {
            drop: 0,
            sub: Some((from, to)),
            add: None,
        }
    }
}

/// One spelling of a case: how the stem comes from the lemma, and the ending
/// that goes on it.
pub struct Form {
    pub stem: &'static [Step],
    /// The ending, as a back-harmony and a front-harmony spelling. Most cases
    /// take the same one either way, and `pientä` against `nuorta` is the pair
    /// where it does not.
    pub ending: (&'static str, &'static str),
}

impl Form {
    /// An ending that does not change with the word's harmony.
    const fn both(ending: &'static str) -> Self {
        Self {
            stem: KEEP,
            ending: (ending, ending),
        }
    }
}

/// A class whose forms are not the model's forms with a different stem.
///
/// These classes are headed by a model word that no other word in the class
/// resembles: `pieni` stands for `nuori` and `suuri`, and `kynsi` for `korsi`
/// and `kansi`. What each case does to a member is regular in itself, but it
/// cannot be read off the model, because the model is a different word and the
/// ending that fits it is not the ending that fits the member.
pub struct Irregular {
    /// The eight cases the catalogue covers, in its order, each with every
    /// spelling that case takes.
    pub cases: &'static [&'static [Form]],
}

const KEEP: &[Step] = &[Step::keep()];
/// The final `i` off, for the `-i` type: `pieni` gives `pien`.
const I_OFF: &[Step] = &[Step::sub("i", "")];
/// `käsi` to `käd`, the `si` that becomes a `d` before the genitive and the
/// nominative plural.
const SI_TO_DE: &[Step] = &[Step::sub("si", "de")];
/// `vesi` to `vet`, the `si` that becomes a `t` before a partitive.
const SI_TO_T: &[Step] = &[Step::sub("si", "t")];
/// `käsi` to `käte`, the `t` in its front reading.
const SI_TO_TE: &[Step] = &[Step::sub("si", "te")];
/// `kynsi` to `kynn`, the `si` that becomes an `n`.
const SI_TO_N: &[Step] = &[Step::sub("si", "n")];
/// `kynsi` to `kyntt`, the `n` that doubles before the partitive.
const SI_TO_TT: &[Step] = &[Step::sub("si", "tt")];
/// `lapsi` to `las`, the `i` and then the `p` that go.
const P_OFF: &[Step] = &[Step {
    drop: 1,
    sub: Some(("p", "")),
    add: None,
}];
/// `lapsi` to `laps`, for the `uksi` group that takes no consonant change.
const SI_OFF: &[Step] = &[Step::sub("si", "")];
/// `veitsi` to `veist`, the `i` and then the `ts` that swap.
const TS_TO_ST: &[Step] = &[Step {
    drop: 1,
    sub: Some(("ts", "st")),
    add: None,
}];
/// `veitsi` to `veis`, for the `veisten` that keeps the `s` and drops the `t`.
const TS_TO_S: &[Step] = &[Step {
    drop: 1,
    sub: Some(("ts", "s")),
    add: None,
}];
/// `isoäiti` to `isoäid`, the `ti` of the head that changes in the genitive.
const TI_TO_D: &[Step] = &[Step::sub("ti", "d")];
/// `kaksi` to `kah`, the `ksi` that goes to an `h`.
const KSI_TO_H: &[Step] = &[Step {
    drop: 3,
    sub: None,
    add: Some("h"),
}];
/// `kaksi` to `kaht`, the same before the long `a` of `kahta`.
const KSI_TO_HT: &[Step] = &[Step {
    drop: 3,
    sub: None,
    add: Some("ht"),
}];
/// `haaksi` to `haas`, the one member of the class that keeps its `s`.
const KSI_TO_S: &[Step] = &[Step::sub("ksi", "s")];

/// The classes that spell their own cases, keyed by class.
///
/// Each row is the class's own catalogue model, so the table can be read off
/// the model and checked against it: running `cases` on the model word of the
/// class has to give that word's row back. `every_model_row_comes_back` is
/// what holds it to that.
const IRREGULAR: &[(ClassId, Irregular)] = &[
    // pieni pienen pientä pieneen pienet pienten pienien pieniä pieniin.
    // The `-i` type drops its `i` for the stem and takes `-ta` in the partitive,
    // and the `i` comes back where the ending itself begins with a vowel.
    (
        26,
        Irregular {
            cases: &[
                &[Form::both("")],
                &[Form {
                    stem: I_OFF,
                    ending: ("en", "en"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("ta", "tä"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("een", "een"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("et", "et"),
                }],
                &[
                    Form {
                        stem: I_OFF,
                        ending: ("ten", "ten"),
                    },
                    Form {
                        stem: I_OFF,
                        ending: ("ien", "ien"),
                    },
                ],
                &[Form {
                    stem: I_OFF,
                    ending: ("ia", "iä"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("iin", "iin"),
                }],
            ],
        },
    ),
    // käsi käden kättä käteen kädet käsien kätten kättiä kättäin. The `si` turns
    // to a `d` before the genitive, an `n` before the rest, and the partitive
    // doubles the `t` off the stem rather than off the `si`: vesi gives vettä
    // and kausi kaustaa-shaped kausta, and uusi keeps the `i` in uusia and
    // uusiin, which is why the nominative spelling is offered alongside the
    // doubled one in the two plural slots.
    (
        27,
        Irregular {
            cases: &[
                &[Form::both("")],
                &[Form {
                    stem: SI_TO_DE,
                    ending: ("n", "n"),
                }],
                &[Form {
                    stem: SI_TO_T,
                    ending: ("ta", "tä"),
                }],
                &[Form {
                    stem: SI_TO_TE,
                    ending: ("en", "en"),
                }],
                &[Form {
                    stem: SI_TO_DE,
                    ending: ("t", "t"),
                }],
                &[
                    Form {
                        stem: KEEP,
                        ending: ("en", "en"),
                    },
                    Form {
                        stem: SI_TO_T,
                        ending: ("ten", "ten"),
                    },
                ],
                &[
                    Form {
                        stem: KEEP,
                        ending: ("a", "ä"),
                    },
                    Form {
                        stem: SI_TO_T,
                        ending: ("tia", "tiä"),
                    },
                ],
                &[
                    Form {
                        stem: KEEP,
                        ending: ("in", "in"),
                    },
                    Form {
                        stem: SI_TO_T,
                        ending: ("täin", "täin"),
                    },
                ],
            ],
        },
    ),
    // kynsi kynnen kynttä kynteen kynnet kynsien kyntten kynsiä kynsiin. The
    // `si` becomes an `n` throughout, and that `n` doubles before the partitive
    // only, which is what separates kynsi from its class-27 cousins vesi and
    // uusi. The nominative spelling stays for the plural slots, as kynsien and
    // kynsiä show, where the `-i` group and the `-n` group meet.
    (
        28,
        Irregular {
            cases: &[
                &[Form::both("")],
                &[Form {
                    stem: SI_TO_N,
                    ending: ("en", "en"),
                }],
                &[Form {
                    stem: SI_TO_TT,
                    ending: ("a", "ä"),
                }],
                &[Form {
                    stem: SI_OFF,
                    ending: ("teen", "teen"),
                }],
                &[Form {
                    stem: SI_TO_N,
                    ending: ("et", "et"),
                }],
                &[
                    Form {
                        stem: KEEP,
                        ending: ("en", "en"),
                    },
                    Form {
                        stem: SI_TO_TT,
                        ending: ("en", "en"),
                    },
                ],
                &[
                    Form {
                        stem: KEEP,
                        ending: ("a", "ä"),
                    },
                    Form {
                        stem: SI_TO_TT,
                        ending: ("a", "ä"),
                    },
                ],
                &[
                    Form {
                        stem: KEEP,
                        ending: ("in", "in"),
                    },
                    Form {
                        stem: SI_OFF,
                        ending: ("teen", "teen"),
                    },
                ],
            ],
        },
    ),
    // lapsi lapsen lasta lapseen lapset lasten lapsien lapsia lapsiin. The `i`
    // goes throughout and the `p` goes in the partitive, which is why lapsi
    // gives lasta but lapsen. The `uksi` group of the class takes no consonant
    // change at all, so the bare `si` is offered beside the `p`-less stem.
    (
        29,
        Irregular {
            cases: &[
                &[Form::both("")],
                &[Form {
                    stem: I_OFF,
                    ending: ("en", "en"),
                }],
                &[
                    Form {
                        stem: P_OFF,
                        ending: ("ta", "tä"),
                    },
                    Form {
                        stem: SI_OFF,
                        ending: ("ta", "tä"),
                    },
                ],
                &[Form {
                    stem: I_OFF,
                    ending: ("een", "een"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("et", "et"),
                }],
                &[
                    Form {
                        stem: P_OFF,
                        ending: ("ten", "ten"),
                    },
                    Form {
                        stem: I_OFF,
                        ending: ("ien", "ien"),
                    },
                ],
                &[Form {
                    stem: I_OFF,
                    ending: ("ia", "iä"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("iin", "iin"),
                }],
            ],
        },
    ),
    // veitsi veitsen veistä veitseen veitset veitsien veisten veitsiä veitsiin.
    // The `i` goes throughout, and the `ts` turns to a `st` before the front
    // vowels of the partitive, which is the same alternation as class 29's `p`
    // dropping. The `veisten` of the plural genitive is the reading that keeps
    // the `s`, so both are offered there.
    (
        30,
        Irregular {
            cases: &[
                &[Form::both("")],
                &[Form {
                    stem: I_OFF,
                    ending: ("en", "en"),
                }],
                &[Form {
                    stem: TS_TO_ST,
                    ending: ("a", "ä"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("een", "een"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("et", "et"),
                }],
                &[
                    Form {
                        stem: I_OFF,
                        ending: ("ien", "ien"),
                    },
                    Form {
                        stem: TS_TO_S,
                        ending: ("ten", "ten"),
                    },
                ],
                &[Form {
                    stem: I_OFF,
                    ending: ("ia", "iä"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("iin", "iin"),
                }],
            ],
        },
    ),
    // kaksi kahden kahta kahteen kahdet kaksien kaksia kaksiin, yksi yhden
    // yhtä yhteen, haaksi haasken haaska. The `ksi` goes to an `h`, and to an
    // `ht` before the long `a` of kahta and the `ee` of kahteen, which is the
    // same place class 27 doubles its `t`. The three plural slots keep the `i`,
    // so kaksien is not kaksnen. haaksi is the one member that keeps its `s`,
    // and it gets its own reading beside the regular one.
    (
        31,
        Irregular {
            cases: &[
                &[Form::both("")],
                &[
                    Form {
                        stem: KSI_TO_H,
                        ending: ("den", "den"),
                    },
                    Form {
                        stem: KSI_TO_S,
                        ending: ("ken", "ken"),
                    },
                ],
                &[
                    Form {
                        stem: KSI_TO_HT,
                        ending: ("a", "ä"),
                    },
                    Form {
                        stem: KSI_TO_S,
                        ending: ("ka", "kä"),
                    },
                ],
                &[Form {
                    stem: KSI_TO_HT,
                    ending: ("een", "een"),
                }],
                &[
                    Form {
                        stem: KSI_TO_H,
                        ending: ("det", "det"),
                    },
                    Form {
                        stem: KSI_TO_S,
                        ending: ("ket", "ket"),
                    },
                ],
                &[Form {
                    stem: KEEP,
                    ending: ("en", "en"),
                }],
                &[Form {
                    stem: KEEP,
                    ending: ("a", "ä"),
                }],
                &[Form {
                    stem: KEEP,
                    ending: ("in", "in"),
                }],
            ],
        },
    ),
    // isoäiti isoäidin isoäitiä isoäitiin isoäidit isoäitien isoäitejä
    // isoäiteihin. The head stays put and the `ti` of it changes in the
    // genitive and the nominative plural only, which is the whole of what
    // class 50 marks. The three remaining plural slots are the `-i` type's,
    // since the head's `i` is the lemma's own final vowel.
    (
        50,
        Irregular {
            cases: &[
                &[Form::both("")],
                &[Form {
                    stem: TI_TO_D,
                    ending: ("in", "in"),
                }],
                &[Form::both("ä")],
                &[Form::both("in")],
                &[Form {
                    stem: TI_TO_D,
                    ending: ("it", "it"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("ien", "ien"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("ejä", "ejä"),
                }],
                &[Form {
                    stem: I_OFF,
                    ending: ("eihin", "eihin"),
                }],
            ],
        },
    ),
];

/// The spellings of `class`'s own cases, where the class has them.
pub fn irregular(class: ClassId) -> Option<&'static Irregular> {
    IRREGULAR
        .iter()
        .find(|(c, _)| *c == class)
        .map(|(_, ir)| ir)
}

/// Realises one spelling of a case for `lemma`, the ending taking the harmony
/// of the stem it lands on: `nuorta` is back and `pientä` is front, and which
/// one a word takes follows from the word, not from the model.
pub fn realise(form: &Form, lemma: &str) -> String {
    let mut stem: String = lemma.to_string();
    for step in form.stem {
        let end = stem
            .char_indices()
            .nth(stem.chars().count().saturating_sub(step.drop as usize))
            .map_or(stem.len(), |(i, _)| i);
        stem.truncate(end);
        if let Some((from, to)) = step.sub
            && let Some(i) = stem.rfind(from)
        {
            stem.replace_range(i..i + from.len(), to);
        }
        if let Some(add) = step.add {
            stem.push_str(add);
        }
    }
    stem.push_str(if harmony(&stem) == Harmony::Front {
        form.ending.1
    } else {
        form.ending.0
    });
    stem
}

/// Every form of `lemma` under the class that spells its own cases, grouped
/// the way the catalogue groups them: one entry per slot, in the catalogue's
/// order, each holding every spelling that slot takes.
///
/// The grouping matters because a compound reads one case off each of its
/// parts, and it has to take the same case from both.
pub fn cases_by_slot(lemma: &str, class: ClassId) -> Vec<Vec<Generated>> {
    let Some(ir) = irregular(class) else {
        return Vec::new();
    };
    ir.cases
        .iter()
        .enumerate()
        .map(|(i, forms)| {
            // A slot is a case by position, the same as everywhere else, and
            // these are the paradigm itself rather than anything worked out
            // from it, so a word that is on the list is a plain one.
            let tags = crate::kotus::slot_nominal(i)
                .map(|(case, number)| Tags::of_number(case, number))
                .unwrap_or(Tags::UNKNOWN);
            forms
                .iter()
                .map(|f| Generated::new(realise(f, lemma), tags).with(Confidence::Plain))
                .collect()
        })
        .collect()
}

/// Every form of `lemma` under the class that spells its own cases.
pub fn cases(lemma: &str, class: ClassId) -> Vec<String> {
    let Some(ir) = irregular(class) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(12);
    for case in ir.cases {
        for form in *case {
            out.push(realise(form, lemma));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::spec::Catalogues;

    fn cat() -> Catalogues {
        Catalogues::embedded().clone()
    }

    /// The forms of `lemma` in the order `cases` returns them, which is the
    /// catalogue's case order: eight slots, with the extra spellings of the
    /// genitive and partitive plurals following the slot they belong to.
    fn slot(forms: &[String], i: usize) -> &str {
        forms.get(i).map(String::as_str).unwrap_or_default()
    }

    /// Every table has to give back the row the catalogue records for its own
    /// model word, which is the only description of the class that is not a
    /// guess. Running the inflector on the model and checking its forms come
    /// back is what keeps a table honest.
    #[test]
    fn every_model_row_comes_back() {
        let cat = cat();
        for (class, ir) in IRREGULAR {
            let para = cat
                .get(*class)
                .unwrap_or_else(|| panic!("class {class} is not in the catalogue"));
            let model = para.model();
            let got = cases(model, *class);
            for slot in &para.slots {
                for want in slot.variants() {
                    assert!(
                        got.iter().any(|g| g == want),
                        "class {class} {model}: missing {want} in {got:?}"
                    );
                }
            }
            assert!(!ir.cases.is_empty());
        }
    }

    /// Every hardcoded word has a nominative singular, and a form that is not
    /// the lemma itself, so the table is never just the word standing for
    /// itself.
    #[test]
    fn every_irregular_word_has_real_forms() {
        for (lemma, _) in IRREGULAR_WORDS {
            let got = word_cases(lemma);
            assert!(!got.is_empty(), "{lemma} has no hardcoded forms");
            assert!(
                got.iter().any(|f| f.word == *lemma),
                "{lemma} is missing its nominative"
            );
            assert!(
                got.iter().any(|f| f.word != *lemma),
                "{lemma} is just itself, not a paradigm"
            );
            for f in &got {
                assert!(f.plain(), "{lemma} {} is not plain", f.word);
            }
        }
    }

    #[test]
    fn the_first_person_pronoun_is_suppletive() {
        let f = word_cases("minä");
        for want in ["minä", "minun", "minua", "minussa", "minuun", "minulla"] {
            assert!(
                f.iter().any(|g| g.word == want),
                "minä is missing {want}: {f:?}"
            );
        }
        // The pronoun has no plural of its own.
        assert!(
            f.iter().all(|g| g
                .readings()
                .iter()
                .all(|t| t.number != Some(Number::Plural))),
            "minä should have no plural: {f:?}"
        );
    }

    #[test]
    fn the_aprilli_paradigm_comes_back() {
        let f = word_cases("aprilli");
        for want in [
            "aprilli",
            "aprillin",
            "aprillia",
            "aprillit",
            "aprillien",
            "aprilleja",
            "aprilleissa",
            "aprilleiksi",
            "aprillein",
        ] {
            assert!(
                f.iter().any(|g| g.word == want),
                "aprilli is missing {want}: {f:?}"
            );
        }
        assert_eq!(f.len(), 25, "aprilli has a full singular and plural");
    }

    #[test]
    fn a_word_not_on_the_list_has_no_hardcoded_forms() {
        assert!(word_cases("talo").is_empty());
        assert!(word_irregular("talo").is_none());
    }

    #[test]
    fn the_i_type_loses_its_i() {
        // pieni: the `i` goes and `-tä` follows, as the class model says.
        let f = cases("pieni", 26);
        assert_eq!(slot(&f, 0), "pieni");
        assert_eq!(slot(&f, 1), "pienen");
        assert_eq!(slot(&f, 2), "pientä");
        assert_eq!(slot(&f, 3), "pieneen");
        assert_eq!(slot(&f, 4), "pienet");
        assert!(f.contains(&"pienien".to_string()));
        assert!(f.contains(&"pienten".to_string()));
        assert_eq!(slot(&f, 7), "pieniä");
        assert_eq!(slot(&f, 8), "pieniin");
    }

    #[test]
    fn a_back_word_of_the_i_type_harmonises_back() {
        // nuori is the class 51 compound head, and the whole reason this table
        // has to work rather than approximate.
        let f = cases("nuori", 26);
        assert!(f.contains(&"nuoren".to_string()));
        assert!(f.contains(&"nuorta".to_string()));
        assert!(f.contains(&"nuorten".to_string()));
        assert!(f.contains(&"nuoria".to_string()));
    }

    #[test]
    fn the_si_type_turns_its_si_to_a_d_or_a_t() {
        // käsi/käden/kättä, and vesi/vettä is the same rule on a back word.
        let f = cases("käsi", 27);
        assert!(f.contains(&"käden".to_string()));
        assert!(f.contains(&"kättä".to_string()));
        assert!(f.contains(&"kätten".to_string()));
        assert!(f.contains(&"kättiä".to_string()));
        let v = cases("vesi", 27);
        assert!(v.contains(&"veden".to_string()));
        assert!(v.contains(&"vettä".to_string()));
    }

    #[test]
    fn the_si_type_keeps_its_i_in_two_classes() {
        // uusi and tosi are the members of class 27 that take `-sia` rather
        // than the doubled `t` of vesi, so both spellings have to be there.
        for w in ["uusi", "tosi", "suuri"] {
            let f = cases(w, 27);
            assert!(
                f.contains(&format!("{w}in")),
                "{w} needs the nominative plural: {f:?}"
            );
        }
    }

    #[test]
    fn the_ksi_type_drops_its_ksi() {
        // kaksi/kahden/kahta, yksi/yhden/yhtä, haaksi/haasken.
        assert!(cases("kaksi", 31).contains(&"kahden".to_string()));
        assert!(cases("kaksi", 31).contains(&"kahta".to_string()));
        assert!(cases("yksi", 31).contains(&"yhden".to_string()));
        assert!(cases("yksi", 31).contains(&"yhtä".to_string()));
        assert!(cases("haaksi", 31).contains(&"haasken".to_string()));
    }

    #[test]
    fn isoaiti_changes_only_in_its_genitive() {
        let f = cases("isoäiti", 50);
        assert!(f.contains(&"isoäidin".to_string()));
        assert!(f.contains(&"isoäidit".to_string()));
        assert!(f.contains(&"isoäitiä".to_string()));
        assert!(f.contains(&"isoäitien".to_string()));
        assert!(f.contains(&"isoäitejä".to_string()));
    }
}
