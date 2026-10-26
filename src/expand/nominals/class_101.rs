//! Class 101 (the pronoun type) inflection, written by hand.
//!
//! # Contract
//!
//! `inflect` returns the pronoun's **paradigm**, not just the subset of forms
//! the reference happens to confirm. Every form the coverage oracle (Omorfi)
//! generates is produced; the spellings the reference confirms are offered as
//! `Plain` (a spell checker may suggest them) and the rest are produced at
//! `Repaired` (generated, but not offered). So `mikä`, `kuka`, `jokin` and
//! `kukaan` come back with their full paradigms, and a cell with several real
//! spellings (`joiden`/`joitten`/`joidenka`/`joittenka`) lists them all.
//!
//! A lemma this class does not know is offered as its bare nominative only.
//! That is a fallback, not a claim that the word is indeclinable -- the class
//! engine is asked for words the dictionary has already placed here.
//!
//! # How
//!
//! Pronouns are suppletive: their case stems are not derivable from the lemma
//! the way a noun's are (`se` is `sen`/`sitä`/`siihen`, `kuka` is
//! `kukaan`/`ketään`/`ke-` in the plural). The class is built in the two layers
//! that fact allows:
//!
//! * the **regular** pronouns (the personal pronouns on `-u`/`-e`, and the
//!   demonstratives `tuo`/`tämä`/`se`) are a stem, a harmony, the two
//!   suppletive members each spells outright (partitive, illative) and a few
//!   per-case overrides. Their regular cases come from
//!   [`singular_case_suffix`], the shared case-ending table, so no ending is
//!   written out here;
//! * the **suppletive** `jo-`/`ku-`/`mi-` clitic families, whose case ending
//!   fuses a clitic (`joka` -> `jonka`, `kuka` -> `ketään`, `jokin` ->
//!   `jotain`), are tabulated outright -- there is no stem+suffix rule that
//!   would not just be the table again.
#![allow(dead_code)]

use crate::tags::{Case, Form, Number};

use super::utility::{
    Builder, Decl, Entry, Harmony, nominal_tags, plain, rare, singular_case_suffix,
};
use crate::tags::PartOfSpeech;

pub const CLASS: &str = "101";

/// A pronoun on a regular oblique stem. The two members the stem does not make
/// (partitive, illative) are given outright; every other case is the stem plus
/// [`singular_case_suffix`], or an override when that is not the reference's
/// spelling.
struct Pron {
    lemma: &'static str,
    stem: &'static str,
    /// The word's harmony, which for pronouns is lexical and not the
    /// nominative's vowel: `minä` is back (`minuna`), `hän` front (`hänenä`).
    harmony: Harmony,
    partitive: &'static str,
    illative: &'static str,
    overrides: &'static [(Case, &'static str)],
}

/// Every singular case, in the order the engine emits them: the nominative,
/// genitive, partitive and illative first, then the rest, as the catalogue
/// orders them.
const SINGULAR_CASES: &[Case] = &[
    Case::Nominative,
    Case::Genitive,
    Case::Partitive,
    Case::Essive,
    Case::Translative,
    Case::Inessive,
    Case::Elative,
    Case::Illative,
    Case::Adessive,
    Case::Ablative,
    Case::Allative,
    Case::Abessive,
    Case::Accusative,
];

const TEMPLATED: &[Pron] = &[
    Pron {
        lemma: "hän",
        stem: "häne",
        harmony: Harmony::Front,
        partitive: "häntä",
        illative: "häneen",
        overrides: &[],
    },
    Pron {
        lemma: "minä",
        stem: "minu",
        harmony: Harmony::Back,
        partitive: "minua",
        illative: "minuun",
        overrides: &[],
    },
    Pron {
        lemma: "sinä",
        stem: "sinu",
        harmony: Harmony::Back,
        partitive: "sinua",
        illative: "sinuun",
        overrides: &[],
    },
    Pron {
        lemma: "tuo",
        stem: "tuo",
        harmony: Harmony::Back,
        partitive: "tuota",
        illative: "tuohon",
        overrides: &[],
    },
    Pron {
        lemma: "tämä",
        stem: "tä",
        harmony: Harmony::Front,
        partitive: "tätä",
        illative: "tähän",
        overrides: &[(Case::Genitive, "tämän")],
    },
    Pron {
        lemma: "se",
        stem: "si",
        harmony: Harmony::Front,
        partitive: "sitä",
        illative: "siihen",
        overrides: &[
            (Case::Genitive, "sen"),
            (Case::Inessive, "siinä"),
            (Case::Elative, "siitä"),
        ],
    },
    // The plural personal pronouns. Their stem is `mei`/`tei`/`hei` and
    // the genitive fuses a `d` (`meidän`), which the regular `-n` would
    // not make.
    Pron {
        lemma: "me",
        stem: "mei",
        harmony: Harmony::Front,
        partitive: "meitä",
        illative: "meihin",
        overrides: &[(Case::Genitive, "meidän")],
    },
    Pron {
        lemma: "te",
        stem: "tei",
        harmony: Harmony::Front,
        partitive: "teitä",
        illative: "teihin",
        overrides: &[(Case::Genitive, "teidän")],
    },
    Pron {
        lemma: "he",
        stem: "hei",
        harmony: Harmony::Front,
        partitive: "heitä",
        illative: "heihin",
        overrides: &[(Case::Genitive, "heidän")],
    },
];

/// A tabulated paradigm: `(case, number, form, confirmed)`. The families whose
/// case ending fuses a clitic (`joka`/`jokin`/`joku`, `kuka`/`kukaan`/`kukin`,
/// `mikä`/`mikään`/`mikin`, and the colloquial short pronouns) are irregular
/// enough to be known outright. A `confirmed` row is offered as `Plain`, the
/// rest as `Repaired`. A cell with several real spellings lists them all.
const IRREGULAR: &[(&str, &[(Case, Number, &str, bool)])] = &[
    (
        "joka",
        &[
            (Case::Nominative, Number::Singular, "joka", true),
            (Case::Nominative, Number::Plural, "jotka", true),
            (Case::Genitive, Number::Singular, "jonka", true),
            (Case::Genitive, Number::Plural, "joiden", true),
            (Case::Genitive, Number::Plural, "joidenka", true),
            (Case::Genitive, Number::Plural, "joitten", true),
            (Case::Genitive, Number::Plural, "joittenka", true),
            (Case::Partitive, Number::Singular, "jota", true),
            (Case::Partitive, Number::Plural, "joita", true),
            (Case::Essive, Number::Singular, "jona", true),
            (Case::Essive, Number::Plural, "joina", true),
            (Case::Translative, Number::Singular, "joksi", true),
            (Case::Translative, Number::Plural, "joiksi", true),
            (Case::Inessive, Number::Singular, "jossa", true),
            (Case::Inessive, Number::Plural, "joissa", true),
            (Case::Elative, Number::Singular, "josta", true),
            (Case::Elative, Number::Plural, "joista", true),
            (Case::Illative, Number::Singular, "johon", true),
            (Case::Illative, Number::Singular, "johonka", true),
            (Case::Illative, Number::Singular, "jonne", true),
            (Case::Illative, Number::Plural, "joihin", true),
            (Case::Adessive, Number::Singular, "jolla", true),
            (Case::Adessive, Number::Plural, "joilla", true),
            (Case::Ablative, Number::Singular, "jolta", true),
            (Case::Ablative, Number::Plural, "joilta", true),
            (Case::Allative, Number::Singular, "jolle", true),
            (Case::Allative, Number::Plural, "joille", true),
            (Case::Abessive, Number::Singular, "jotta", false),
            (Case::Abessive, Number::Plural, "joitta", false),
            (Case::Instructive, Number::Plural, "join", false),
        ],
    ),
    (
        "jokin",
        &[
            (Case::Nominative, Number::Singular, "jokin", true),
            (Case::Nominative, Number::Plural, "jotkin", true),
            (Case::Genitive, Number::Singular, "jonkin", true),
            (Case::Genitive, Number::Plural, "joidenkin", false),
            (Case::Genitive, Number::Plural, "joittenkin", false),
            (Case::Partitive, Number::Singular, "jotain", true),
            (Case::Partitive, Number::Singular, "jotakin", false),
            (Case::Partitive, Number::Plural, "joitain", true),
            (Case::Partitive, Number::Plural, "joitakin", false),
            (Case::Essive, Number::Singular, "jonain", true),
            (Case::Essive, Number::Singular, "jonakin", false),
            (Case::Essive, Number::Plural, "joinain", true),
            (Case::Essive, Number::Plural, "joinakin", false),
            (Case::Translative, Number::Singular, "joksikin", false),
            (Case::Translative, Number::Plural, "joiksikin", false),
            (Case::Inessive, Number::Singular, "jossain", true),
            (Case::Inessive, Number::Singular, "jossakin", false),
            (Case::Inessive, Number::Plural, "joissain", true),
            (Case::Inessive, Number::Plural, "joissakin", false),
            (Case::Elative, Number::Singular, "jostain", true),
            (Case::Elative, Number::Singular, "jostakin", false),
            (Case::Elative, Number::Plural, "joistain", true),
            (Case::Elative, Number::Plural, "joistakin", false),
            (Case::Illative, Number::Singular, "johonkin", false),
            (Case::Illative, Number::Singular, "jonnekin", false),
            (Case::Illative, Number::Plural, "joihinkin", false),
            (Case::Adessive, Number::Singular, "jollain", true),
            (Case::Adessive, Number::Singular, "jollakin", false),
            (Case::Adessive, Number::Plural, "joillain", true),
            (Case::Adessive, Number::Plural, "joillakin", false),
            (Case::Ablative, Number::Singular, "joltain", true),
            (Case::Ablative, Number::Singular, "joltakin", false),
            (Case::Ablative, Number::Plural, "joiltain", true),
            (Case::Ablative, Number::Plural, "joiltakin", false),
            (Case::Allative, Number::Singular, "jollekin", false),
            (Case::Allative, Number::Plural, "joillekin", false),
        ],
    ),
    (
        "joku",
        &[
            (Case::Nominative, Number::Singular, "joku", true),
            (Case::Nominative, Number::Plural, "jotkut", true),
            (Case::Genitive, Number::Singular, "jonkun", true),
            (Case::Genitive, Number::Plural, "joidenkuiden", true),
            (Case::Genitive, Number::Plural, "joittenkuitten", true),
            (Case::Partitive, Number::Singular, "jotakuta", true),
            (Case::Partitive, Number::Plural, "joitakuita", true),
            (Case::Essive, Number::Singular, "jonakuna", true),
            (Case::Essive, Number::Plural, "joinakuina", true),
            (Case::Translative, Number::Singular, "joksikuksi", true),
            (Case::Translative, Number::Plural, "joiksikuiksi", true),
            (Case::Inessive, Number::Singular, "jossakussa", true),
            (Case::Inessive, Number::Plural, "joissakuissa", true),
            (Case::Elative, Number::Singular, "jostakusta", true),
            (Case::Elative, Number::Plural, "joistakuista", true),
            (Case::Illative, Number::Singular, "johonkuhun", true),
            (Case::Illative, Number::Plural, "joihinkuihin", true),
            (Case::Adessive, Number::Singular, "jollakulla", true),
            (Case::Adessive, Number::Plural, "joillakuilla", true),
            (Case::Ablative, Number::Singular, "joltakulta", true),
            (Case::Ablative, Number::Plural, "joiltakuilta", true),
            (Case::Allative, Number::Singular, "jollekulle", true),
            (Case::Allative, Number::Plural, "joillekuille", true),
        ],
    ),
    ("ken", &[(Case::Nominative, Number::Singular, "ken", true)]),
    (
        "kenkään",
        &[(Case::Nominative, Number::Singular, "kenkään", false)],
    ),
    (
        "kuka",
        &[
            (Case::Nominative, Number::Singular, "kuka", true),
            (Case::Nominative, Number::Plural, "ketkä", false),
            (Case::Genitive, Number::Singular, "kenen", false),
            (Case::Genitive, Number::Plural, "keiden", false),
            (Case::Genitive, Number::Plural, "keitten", false),
            (Case::Partitive, Number::Singular, "ketä", false),
            (Case::Partitive, Number::Plural, "keitä", false),
            (Case::Essive, Number::Singular, "kenenä", false),
            (Case::Essive, Number::Singular, "kenä", false),
            (Case::Essive, Number::Plural, "keinä", false),
            (Case::Translative, Number::Singular, "keneksi", false),
            (Case::Translative, Number::Plural, "keiksi", false),
            (Case::Inessive, Number::Singular, "kenessä", false),
            (Case::Inessive, Number::Singular, "kessä", false),
            (Case::Inessive, Number::Plural, "keissä", false),
            (Case::Elative, Number::Singular, "kenestä", false),
            (Case::Elative, Number::Singular, "kestä", false),
            (Case::Elative, Number::Plural, "keistä", false),
            (Case::Illative, Number::Singular, "kehen", false),
            (Case::Illative, Number::Singular, "keneen", false),
            (Case::Illative, Number::Plural, "keihin", false),
            (Case::Adessive, Number::Singular, "kellä", false),
            (Case::Adessive, Number::Singular, "kenellä", false),
            (Case::Adessive, Number::Plural, "keillä", false),
            (Case::Ablative, Number::Singular, "keltä", false),
            (Case::Ablative, Number::Singular, "keneltä", false),
            (Case::Ablative, Number::Plural, "keiltä", false),
            (Case::Allative, Number::Singular, "kelle", false),
            (Case::Allative, Number::Singular, "kenelle", false),
            (Case::Allative, Number::Plural, "keille", false),
            (Case::Abessive, Number::Singular, "kenettä", false),
            (Case::Abessive, Number::Plural, "keittä", false),
            (Case::Instructive, Number::Plural, "kein", false),
        ],
    ),
    (
        "kukaan",
        &[
            (Case::Nominative, Number::Singular, "kukaan", true),
            (Case::Nominative, Number::Plural, "ketkään", true),
            (Case::Genitive, Number::Singular, "kenenkään", false),
            (Case::Genitive, Number::Plural, "keidenkään", false),
            (Case::Genitive, Number::Plural, "keittenkään", false),
            (Case::Partitive, Number::Singular, "ketäkään", false),
            (Case::Partitive, Number::Singular, "ketään", true),
            (Case::Partitive, Number::Plural, "keitäkään", false),
            (Case::Partitive, Number::Plural, "keitään", true),
            (Case::Essive, Number::Singular, "kenäkään", false),
            (Case::Essive, Number::Singular, "kenään", false),
            (Case::Essive, Number::Plural, "keinäkään", false),
            (Case::Essive, Number::Plural, "keinään", false),
            (Case::Translative, Number::Singular, "keneksikään", false),
            (Case::Illative, Number::Singular, "kehenkään", false),
            (Case::Illative, Number::Singular, "keneenkään", false),
            (Case::Illative, Number::Plural, "keihinkään", false),
            (Case::Adessive, Number::Singular, "kelläkään", false),
            (Case::Adessive, Number::Singular, "kellään", true),
            (Case::Adessive, Number::Singular, "kenelläkään", false),
            (Case::Adessive, Number::Plural, "keilläkään", false),
            (Case::Adessive, Number::Plural, "keillään", false),
            (Case::Ablative, Number::Singular, "keltäkään", false),
            (Case::Ablative, Number::Singular, "keltään", true),
            (Case::Ablative, Number::Singular, "keneltäkään", false),
            (Case::Ablative, Number::Plural, "keiltään", false),
            (Case::Allative, Number::Singular, "kellekään", false),
            (Case::Allative, Number::Singular, "kenellekään", false),
            (Case::Allative, Number::Plural, "keillekään", false),
        ],
    ),
    (
        "kukin",
        &[
            (Case::Nominative, Number::Singular, "kukin", true),
            (Case::Genitive, Number::Singular, "kunkin", true),
            (Case::Genitive, Number::Plural, "kuidenkin", false),
            (Case::Partitive, Number::Singular, "kutakin", false),
            (Case::Partitive, Number::Plural, "kuitakin", false),
            (Case::Essive, Number::Singular, "kunakin", false),
            (Case::Essive, Number::Plural, "kuinakin", false),
            (Case::Translative, Number::Singular, "kuksikin", false),
            (Case::Translative, Number::Plural, "kuiksikin", false),
            (Case::Inessive, Number::Singular, "kussakin", false),
            (Case::Inessive, Number::Plural, "kuissakin", false),
            (Case::Elative, Number::Singular, "kustakin", false),
            (Case::Elative, Number::Plural, "kuistakin", false),
            (Case::Illative, Number::Singular, "kuhunkin", false),
            (Case::Illative, Number::Plural, "kuihinkin", false),
            (Case::Adessive, Number::Singular, "kullakin", false),
            (Case::Adessive, Number::Plural, "kuillakin", false),
            (Case::Ablative, Number::Singular, "kultakin", false),
            (Case::Ablative, Number::Plural, "kuiltakin", false),
            (Case::Allative, Number::Singular, "kullekin", false),
            (Case::Allative, Number::Plural, "kuillekin", false),
        ],
    ),
    (
        "mikin",
        &[
            (Case::Nominative, Number::Singular, "mikin", true),
            (Case::Nominative, Number::Plural, "mitkin", false),
        ],
    ),
    (
        "mikä",
        &[
            (Case::Nominative, Number::Singular, "mikä", true),
            (Case::Genitive, Number::Singular, "minkä", true),
            (Case::Accusative, Number::Singular, "minkä", true),
            (Case::Partitive, Number::Singular, "mitä", true),
            (Case::Essive, Number::Singular, "minä", true),
            (Case::Translative, Number::Singular, "miksi", true),
            (Case::Inessive, Number::Singular, "missä", true),
            (Case::Elative, Number::Singular, "mistä", true),
            (Case::Illative, Number::Singular, "mihin", true),
            (Case::Adessive, Number::Singular, "millä", true),
            (Case::Ablative, Number::Singular, "miltä", true),
            (Case::Allative, Number::Singular, "mille", true),
            (Case::Abessive, Number::Singular, "minutta", true),
            (Case::Nominative, Number::Plural, "mitkä", true),
            (Case::Genitive, Number::Plural, "minkä", true),
            (Case::Accusative, Number::Plural, "mitkä", true),
            (Case::Partitive, Number::Plural, "mitä", true),
            (Case::Essive, Number::Plural, "minä", true),
            (Case::Translative, Number::Plural, "miksi", true),
            (Case::Inessive, Number::Plural, "missä", true),
            (Case::Elative, Number::Plural, "mistä", true),
            (Case::Illative, Number::Plural, "mihin", true),
            (Case::Adessive, Number::Plural, "millä", true),
            (Case::Ablative, Number::Plural, "miltä", true),
            (Case::Allative, Number::Plural, "mille", true),
            (Case::Abessive, Number::Plural, "minutta", true),
        ],
    ),
    (
        "mikään",
        &[
            (Case::Nominative, Number::Singular, "mikään", true),
            (Case::Nominative, Number::Plural, "mitkään", false),
            (Case::Genitive, Number::Singular, "minkään", true),
            (Case::Genitive, Number::Plural, "mittenkään", false),
        ],
    ),
    (
        "muuan",
        &[
            (Case::Nominative, Number::Singular, "muuan", true),
            (Case::Partitive, Number::Singular, "muuatta", true),
        ],
    ),
    (
        "mä",
        &[
            (Case::Nominative, Number::Singular, "mä", false),
            (Case::Genitive, Number::Singular, "mun", false),
            (Case::Partitive, Number::Singular, "mua", false),
            (Case::Essive, Number::Singular, "muna", false),
            (Case::Translative, Number::Singular, "muksi", false),
            (Case::Inessive, Number::Singular, "mussa", false),
            (Case::Elative, Number::Singular, "must", false),
            (Case::Elative, Number::Singular, "musta", false),
            (Case::Illative, Number::Singular, "muhun", false),
            (Case::Adessive, Number::Singular, "mul", false),
            (Case::Adessive, Number::Singular, "mull", false),
            (Case::Adessive, Number::Singular, "mulla", false),
            (Case::Ablative, Number::Singular, "mult", false),
            (Case::Ablative, Number::Singular, "multa", false),
            (Case::Allative, Number::Singular, "mulle", false),
            (Case::Abessive, Number::Singular, "mutta", false),
        ],
    ),
    (
        "sä",
        &[
            (Case::Nominative, Number::Singular, "sä", false),
            (Case::Genitive, Number::Singular, "sun", false),
            (Case::Partitive, Number::Singular, "sua", false),
            (Case::Essive, Number::Singular, "suna", false),
            (Case::Translative, Number::Singular, "suksi", false),
            (Case::Inessive, Number::Singular, "sussa", false),
            (Case::Elative, Number::Singular, "sust", false),
            (Case::Elative, Number::Singular, "susta", false),
            (Case::Illative, Number::Singular, "suhun", false),
            (Case::Adessive, Number::Singular, "sul", false),
            (Case::Adessive, Number::Singular, "sull", false),
            (Case::Adessive, Number::Singular, "sulla", false),
            (Case::Ablative, Number::Singular, "sult", false),
            (Case::Ablative, Number::Singular, "sulta", false),
            (Case::Allative, Number::Singular, "sulle", false),
            (Case::Abessive, Number::Singular, "sutta", false),
        ],
    ),
    ("tää", &[(Case::Nominative, Number::Singular, "tää", false)]),
    (
        "toi",
        &[
            (Case::Nominative, Number::Singular, "toi", false),
            (Case::Partitive, Number::Singular, "tota", false),
            (Case::Essive, Number::Singular, "tona", false),
            (Case::Translative, Number::Singular, "toksi", false),
            (Case::Inessive, Number::Singular, "tossa", false),
            (Case::Elative, Number::Singular, "tosta", false),
            (Case::Illative, Number::Singular, "tohon", false),
            (Case::Adessive, Number::Singular, "tolla", false),
            (Case::Ablative, Number::Singular, "tolta", false),
            (Case::Allative, Number::Singular, "tolle", false),
            (Case::Abessive, Number::Singular, "totta", false),
        ],
    ),
    // The demonstratives' plural members are suppletive lemmas of their
    // own (`se`/`ne`, `tämä`/`nämä`, `tuo`/`nuo`), so each is tabulated.
    (
        "ne",
        &[
            (Case::Nominative, Number::Plural, "ne", true),
            (Case::Genitive, Number::Plural, "niiden", true),
            (Case::Genitive, Number::Plural, "niitten", false),
            (Case::Partitive, Number::Plural, "niitä", true),
            (Case::Illative, Number::Plural, "niihin", true),
            (Case::Inessive, Number::Plural, "niissä", true),
            (Case::Elative, Number::Plural, "niistä", true),
            (Case::Adessive, Number::Plural, "niillä", true),
            (Case::Ablative, Number::Plural, "niiltä", true),
            (Case::Allative, Number::Plural, "niille", true),
            (Case::Essive, Number::Plural, "niinä", true),
            (Case::Translative, Number::Plural, "niiksi", true),
            (Case::Abessive, Number::Plural, "niittä", false),
            (Case::Instructive, Number::Plural, "niin", true),
        ],
    ),
    (
        "nämä",
        &[
            (Case::Nominative, Number::Plural, "nämä", true),
            (Case::Genitive, Number::Plural, "näiden", true),
            (Case::Genitive, Number::Plural, "näitten", true),
            (Case::Partitive, Number::Plural, "näitä", true),
            (Case::Illative, Number::Plural, "näihin", true),
            (Case::Inessive, Number::Plural, "näissä", true),
            (Case::Elative, Number::Plural, "näistä", true),
            (Case::Adessive, Number::Plural, "näillä", true),
            (Case::Ablative, Number::Plural, "näiltä", true),
            (Case::Allative, Number::Plural, "näille", true),
            (Case::Essive, Number::Plural, "näinä", true),
            (Case::Translative, Number::Plural, "näiksi", true),
            (Case::Abessive, Number::Plural, "näittä", false),
            (Case::Instructive, Number::Plural, "näin", false),
        ],
    ),
    (
        "nuo",
        &[
            (Case::Nominative, Number::Plural, "nuo", true),
            (Case::Genitive, Number::Plural, "noiden", true),
            (Case::Genitive, Number::Plural, "noitten", true),
            (Case::Partitive, Number::Plural, "noita", true),
            (Case::Illative, Number::Plural, "noihin", true),
            (Case::Inessive, Number::Plural, "noissa", true),
            (Case::Elative, Number::Plural, "noista", true),
            (Case::Adessive, Number::Plural, "noilla", true),
            (Case::Ablative, Number::Plural, "noilta", true),
            (Case::Allative, Number::Plural, "noille", true),
            (Case::Essive, Number::Plural, "noina", true),
            (Case::Translative, Number::Plural, "noiksi", true),
            (Case::Abessive, Number::Plural, "noitta", false),
            (Case::Instructive, Number::Plural, "noin", false),
        ],
    ),
];

/// One singular case of a templated pronoun, as a `Decl` entry. The
/// nominative, partitive and illative are the members the stem does not make,
/// so the pronoun spells them outright; every other case is the stem plus the
/// shared suffix table's ending, or an override. The abessive is real but
/// unconfirmed for these pronouns, so it is generated rather than offered.
fn pronoun_entry(p: &Pron, case: Case) -> Entry {
    match case {
        Case::Nominative => plain(case, Number::Singular, p.lemma, ["", ""]),
        Case::Partitive => plain(case, Number::Singular, p.partitive, ["", ""]),
        Case::Illative => plain(case, Number::Singular, p.illative, ["", ""]),
        // The accusative is not the stem plus an ending: `hän` is `hänet`,
        // and the plural pronouns take `-dät` (`meidät`, `heidät`), while
        // `se` and `tuo` use the genitive shape (`sen`, `tuon`).
        Case::Accusative => {
            let word = match p.lemma {
                "minä" => "minut",
                "sinä" => "sinut",
                "hän" => "hänet",
                "me" => "meidät",
                "te" => "teidät",
                "he" => "heidät",
                "tuo" => "tuon",
                "tämä" => "tämän",
                "se" => "sen",
                _ => return plain(case, Number::Singular, p.lemma, ["", ""]),
            };
            plain(case, Number::Singular, word, ["", ""])
        }
        _ => {
            let suffix = singular_case_suffix(case, p.harmony)
                .expect("a named singular case has a regular ending");
            let word = p
                .overrides
                .iter()
                .find(|(c, _)| *c == case)
                .map_or_else(|| format!("{}{suffix}", p.stem), |(_, w)| (*w).to_string());
            if case == Case::Abessive {
                rare(case, Number::Singular, &word, ["", ""])
            } else {
                plain(case, Number::Singular, &word, ["", ""])
            }
        }
    }
}

/// A templated pronoun's singular paradigm.
fn inflect_regular(p: &Pron) -> Vec<Entry> {
    SINGULAR_CASES
        .iter()
        .map(|case| pronoun_entry(p, *case))
        .collect()
}

/// A tabulated pronoun's paradigm: each confirmed form offered, each
/// unconfirmed one generated but not offered.
fn inflect_irregular(forms: &[(Case, Number, &str, bool)]) -> Vec<Entry> {
    forms
        .iter()
        .map(|(case, num, word, is_plain)| {
            if *is_plain {
                plain(*case, *num, word, ["", ""])
            } else {
                rare(*case, *num, word, ["", ""])
            }
        })
        .collect()
}

/// Inflect a class-101 pronoun.
///
/// The lemma is looked up by a linear scan. The two tables hold about twenty
/// entries between them, so the scan is nothing next to building the forms; a
/// `match` would dispatch statically but tie the arms to the tables' positions,
/// which is not worth it unless this ever shows up in a profile.
pub fn inflect(lemma: &str) -> Vec<Form> {
    let entries = if let Some(p) = TEMPLATED.iter().find(|p| p.lemma == lemma) {
        inflect_regular(p)
    } else if let Some((_, forms)) = IRREGULAR.iter().find(|(w, _)| *w == lemma) {
        inflect_irregular(forms)
    } else {
        // A lemma the class does not know -- only reachable if the caller asks
        // for a word this class was never built for -- is offered as its bare
        // nominative.
        vec![plain(Case::Nominative, Number::Singular, lemma, ["", ""])]
    };
    let mut b = Builder::new(lemma, false);
    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma).into_iter().map(|f| f.word).collect()
    }

    fn plain(lemma: &str) -> Vec<String> {
        inflect(lemma)
            .into_iter()
            .filter(|f| f.plain())
            .map(|f| f.word)
            .collect()
    }

    #[test]
    fn personal_pronouns_use_back_harmony() {
        let w = words("minä");
        assert!(w.contains(&"minun".to_string()));
        assert!(w.contains(&"minuun".to_string()));
        assert!(w.contains(&"minuna".to_string()));
        assert!(w.contains(&"minussa".to_string()));
        assert!(w.contains(&"minusta".to_string()));
        assert!(w.contains(&"minulla".to_string()));
        assert!(w.contains(&"minulta".to_string()));
        assert!(w.contains(&"minulle".to_string()));
        assert!(!w.contains(&"minunä".to_string()));
    }

    #[test]
    fn se_uses_irregular_genitive_and_local_cases() {
        let w = words("se");
        for want in [
            "sen", "sitä", "sinä", "siksi", "siinä", "siitä", "siihen", "sillä", "siltä", "sille",
        ] {
            assert!(w.contains(&want.to_string()), "se should have {want}");
        }
        assert!(
            !w.contains(&"sin".to_string()),
            "no templated genitive for se"
        );
        assert!(!w.contains(&"sissä".to_string()));
        assert!(!w.contains(&"sistä".to_string()));
    }

    #[test]
    fn tämä_overrides_only_genitive() {
        let w = words("tämä");
        assert!(w.contains(&"tämän".to_string()));
        assert!(w.contains(&"tänä".to_string()));
        assert!(w.contains(&"tässä".to_string()));
        assert!(w.contains(&"tästä".to_string()));
        assert!(w.contains(&"tähän".to_string()));
        assert!(!w.contains(&"tän".to_string()));
    }

    #[test]
    fn clitic_family_is_complete() {
        // `jokin` needs its genitive plural, which the reference does not
        // confirm but the paradigm does have.
        let w = words("jokin");
        assert!(w.contains(&"joidenkin".to_string()));
        assert!(w.contains(&"joittenkin".to_string()));
        assert!(w.contains(&"johonkin".to_string()));

        // The confirmed spellings are offered, the rest are not.
        let p = plain("jokin");
        assert!(p.contains(&"jotain".to_string()));
        assert!(!p.contains(&"jotakin".to_string()));
    }

    #[test]
    fn colloquial_short_pronoun_has_a_paradigm() {
        // Not lemma-only: the oracle generates a paradigm for `mä`.
        let w = words("mä");
        assert!(w.contains(&"mun".to_string()));
        assert!(w.contains(&"mua".to_string()));
        assert!(w.contains(&"mulla".to_string()));
    }

    #[test]
    fn unknown_lemma_falls_back_to_nominative_only() {
        // The fallback is nominative-only and is not a claim of indeclinability.
        let w = inflect("eitietokone")
            .into_iter()
            .map(|f| f.word)
            .collect::<Vec<_>>();
        assert_eq!(w, vec!["eitietokone".to_string()]);
    }
}
