//! The verb forms the catalogue leaves out.
//!
//! A verb is generated from the two forms of the catalogue that pin the stem
//! down: the first person singular present, which carries the theme vowel, and
//! the third person singular past, which does not. Everything else follows from
//! those, because Finnish verbs are regular in their endings to a degree nouns
//! are not.

use crate::kotus::{ClassId, Gradation};

use crate::tags::{
    Case, Confidence, Form, Mood, Number, PartOfSpeech, Person, Polarity, Possessive, Tags, Tense,
    Voice,
};

use super::harmony::{Harmony, harmony};

use crate::is_vowel;

/// The six traditional verb types, mirroring `verb.types` of the spec.
///
/// Each says how the present stem is cut out of the infinitive and where the
/// gradation sits. The engine derives its stems from the two catalogue forms
/// that pin a verb down rather than from the infinitive, so these tables are
/// the declarative source of truth that the `kotus` mapping keys off; the
/// per-type 3rd person endings are the one piece the shape of the stem cannot
/// always say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerbType {
    /// The present stem of the infinitive, as the spec names the derivation.
    pub present: PresentStem,
    /// The gradation site in the stem.
    pub gradation_site: GradationSite,
    /// The endings the 3rd person singular takes, in the order the engine
    /// tries them.
    pub endings_3sg: &'static [Ending3sg],
}

/// The 3rd person singular ending of a verb type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending3sg {
    /// Add one copy of the stem's final theme vowel: `laske` is `laskee`,
    /// `sano` is `sanoo`, `tapa` is `tapaa`.
    DoubleThemeVowel,
    /// Add the literal ending: `tule` with `-ee` is `tulee`.
    Literal(&'static str),
}

/// How a verb type's present stem is read off its infinitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentStem {
    /// The infinitive drops its final `-a/-ä`; the stem is what stays. `puhua`
    /// gives `puhu`.
    DropVowel,
    /// The infinitive drops the named ending; the stem is what stays. `syödä`
    /// drops `-dä` for `syö`, and `tulla` drops `-la` for `tul`.
    Drop(&'static str),
    /// The type-3 `-e-` is added to the stem. `tule`, `juokse`.
    AddE,
    /// The `-ta/-tä` drops and the stem keeps its own vowel. `tapa`, `sala`.
    BaseVowelStem,
    /// The named ending becomes the named stem. `tarvita` gives `tarvitse`.
    Replace(&'static str, &'static str),
}

/// Where the consonant gradation sits in a verb stem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradationSite {
    /// Between the stem and its final vowel, the `t` of `tavata` graduating
    /// before the theme vowel.
    WeakBeforeFinalVowel,
    /// A consonant pair inside the stem.
    ConsonantPair,
    /// The type does not gradate there.
    None,
}

/// The subtype a Kotus verb class carries, for the classes the plain type does
/// not fully describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbSubtype {
    /// `tupakoida` keeps the `-itse`-style stem of the type-5 verbs.
    Oida,
    /// `nähdä` and `tehdä`, whose 3rd person puts a `k` back that no gradation
    /// rule can recover.
    Hda,
    /// `katketa`, which keeps its own `-ea` subtype vowel.
    Katkea,
    /// `selvitä`, which keeps its own `-iä` subtype vowel.
    Selvia,
}

/// A Kotus verb class as the spec's `verb.kotus` maps it: which of the six
/// types it follows, and the exceptions that class carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KotusVerb {
    /// The [`VerbType`] the class follows, `1..=6`.
    pub ty: u8,
    /// The subtype, for the classes that are not a plain instance of the type.
    pub subtype: Option<VerbSubtype>,
    /// Whether the class's 3rd person stem is a table (`nähdä`, `tehdä`).
    pub irregular: bool,
    /// Whether the class is defective, generating only the 3rd person.
    pub defective: bool,
}

/// The six [`VerbType`]s, by their spec number.
pub const VERB_TYPES: [VerbType; 6] = [
    // 1. `puhua`, `ottaa`, `muistaa`, `laskea`, `sanoa`. The infinitive drops
    // its final vowel; the 3sg is the stem with its own last vowel doubled.
    VerbType {
        present: PresentStem::DropVowel,
        gradation_site: GradationSite::WeakBeforeFinalVowel,
        endings_3sg: &[Ending3sg::DoubleThemeVowel],
    },
    // 2. `syödä`, `juoda`, `saada`, `voida`, `käydä`. The `-da/-dä` is not a
    // stem; the 3sg adds nothing (`syö`, `juo`, `saa`).
    VerbType {
        present: PresentStem::Drop("da/dä"),
        gradation_site: GradationSite::None,
        endings_3sg: &[Ending3sg::DoubleThemeVowel],
    },
    // 3. `tulla`, `mennä`, `nousta`, `juosta`. The consonant cluster drops and
    // the class adds `-e-`; the 3sg doubles it (`tulee`) or takes the strong
    // stem's own vowel (`juoksee`).
    VerbType {
        present: PresentStem::AddE,
        gradation_site: GradationSite::ConsonantPair,
        endings_3sg: &[Ending3sg::Literal("ee"), Ending3sg::Literal("e")],
    },
    // 4. `tavata`, `salata`. The `-ta/-tä` drops and the vowel stem stays; the
    // 3sg doubles its vowel (`tapaa`).
    VerbType {
        present: PresentStem::BaseVowelStem,
        gradation_site: GradationSite::WeakBeforeFinalVowel,
        endings_3sg: &[Ending3sg::DoubleThemeVowel],
    },
    // 5. `tarvita`, `valita`. The `-ita/-itä` becomes `-itse`; the 3sg is
    // `tarvitsee`.
    VerbType {
        present: PresentStem::Replace("ita/itä", "itse/itse"),
        gradation_site: GradationSite::WeakBeforeFinalVowel,
        endings_3sg: &[Ending3sg::DoubleThemeVowel],
    },
    // 6. `vanheta`, `paheta`. The `-eta/-etä` becomes `-ene`; the 3sg is
    // `vanhenee`.
    VerbType {
        present: PresentStem::Replace("eta/etä", "ene/ene"),
        gradation_site: GradationSite::WeakBeforeFinalVowel,
        endings_3sg: &[Ending3sg::DoubleThemeVowel],
    },
];

/// The Kotus verb classes and the type each follows, mirroring `verb.kotus`.
///
/// The catalogue's odd members are typed by the shape of their model's
/// infinitive, and the exceptions carry the extra stem they need.
pub const KOTUS_VERBS: &[(ClassId, KotusVerb)] = &[
    (52, kv(1, None, false, false)),                      // sanoa
    (53, kv(1, None, false, false)),                      // muistaa
    (54, kv(1, None, false, false)),                      // huutaa
    (55, kv(1, None, false, false)),                      // soutaa (sousi)
    (56, kv(1, None, false, false)),                      // kaivaa (kaivoi)
    (57, kv(1, None, false, false)),                      // saartaa (saarsi)
    (58, kv(1, None, false, false)),                      // laskea
    (59, kv(1, None, false, false)),                      // tuntea
    (60, kv(1, None, false, false)),                      // lähteä
    (61, kv(1, None, false, false)),                      // sallia
    (62, kv(2, None, false, false)),                      // voida
    (63, kv(2, None, false, false)),                      // saada
    (64, kv(2, None, false, false)),                      // juoda
    (65, kv(2, None, false, false)),                      // käydä
    (66, kv(3, None, false, false)),                      // rohkaista
    (67, kv(3, None, false, false)),                      // tulla
    (68, kv(2, Some(VerbSubtype::Oida), false, false)),   // tupakoida
    (69, kv(5, None, false, false)),                      // valita
    (70, kv(3, None, false, false)),                      // juosta
    (71, kv(2, Some(VerbSubtype::Hda), true, false)),     // nähdä, tehdä
    (72, kv(6, None, false, false)),                      // vanheta
    (73, kv(4, None, false, false)),                      // salata
    (74, kv(4, Some(VerbSubtype::Katkea), false, false)), // katketa
    (75, kv(4, Some(VerbSubtype::Selvia), false, false)), // selvitä
    (76, kv(1, None, false, false)),                      // taitaa
    (77, kv(1, None, false, true)),                       // kumajaa (3rd person only)
    (78, kv(1, None, false, true)),                       // kaikaa (3rd person only)
];

const fn kv(ty: u8, subtype: Option<VerbSubtype>, irregular: bool, defective: bool) -> KotusVerb {
    KotusVerb {
        ty,
        subtype,
        irregular,
        defective,
    }
}

/// The Kotus description of a verb class, from the `verb.kotus` table.
pub fn kotus_verb(class: ClassId) -> Option<&'static KotusVerb> {
    KOTUS_VERBS
        .iter()
        .find(|(c, _)| *c == class)
        .map(|(_, v)| v)
}

/// Whether a Kotus class is a verb class, which is the classes the `verb.kotus`
/// table covers.
pub fn is_verb_class(class: ClassId) -> bool {
    kotus_verb(class).is_some()
}

/// The present stem of a verb, cut out of the infinitive by the class's type.
///
/// The spec's `verb.types` say how the infinitive gives the stem: type 1 drops
/// its final vowel, type 2 its `-da/-dä`, type 3 adds the class `-e-`, and so
/// on. The stem that comes off the infinitive is the strong grade, and the
/// first person present, which is what a paradigm hangs the weak endings off,
/// is its weak grade: `huutaa` gives `huuta` and then, under gradation,
/// `huuda`, the `huudan` of `huudan`/`huutaa`.
pub fn present_stem(infinitive: &str, class: ClassId, gradation: Option<Gradation>) -> String {
    let Some(v) = kotus_verb(class) else {
        return infinitive.to_string();
    };
    // The `-dä` irregulars (`nähdä`, `tehdä`) lose their consonant in the weak
    // present: `näen` is `näe` and `teen` is `tee`, which no type rule reaches.
    // `juosta` puts a `k` back in its present that its infinitive never carries.
    if let Some(weak) = irregular_weak_present(infinitive) {
        return weak.to_string();
    }
    let ty = &VERB_TYPES[v.ty as usize - 1];
    let strong = match ty.present {
        PresentStem::DropVowel => infinitive
            .strip_suffix('a')
            .or_else(|| infinitive.strip_suffix('ä'))
            .unwrap_or(infinitive)
            .to_string(),
        PresentStem::Drop(e) => strip_any(infinitive, e),
        PresentStem::AddE => {
            // The `-sta`/`-stä` verbs keep the `s`: `nousta` is `nouse` and
            // `rohkaista` is `rohkaise`, while the rest drop the last two
            // characters of the cluster (`tulla` is `tul`, `mennä` is `men`,
            // `pierrä` is `pier`) and all add the `e`.
            let base = if infinitive.ends_with("sta") || infinitive.ends_with("stä") {
                strip_any(infinitive, "ta/tä")
            } else {
                let n = infinitive.chars().count();
                infinitive.chars().take(n.saturating_sub(2)).collect()
            };
            format!("{base}e")
        }
        PresentStem::BaseVowelStem => {
            // The `-ta/-tä` drops and the theme vowel goes back on: `salata`
            // gives `sala` and then `salaa`, `katketa` gives `katkea`, `selvitä`
            // gives `selviä`. The theme is the verb's own harmony vowel.
            let base = strip_any(infinitive, "ta/tä");
            format!("{base}{}", harmony(infinitive).a())
        }
        PresentStem::Replace(from, to) => {
            let from = from.split('/').collect::<Vec<_>>();
            let to = to.split('/').collect::<Vec<_>>();
            let mut stem = infinitive.to_string();
            for (f, t) in from.iter().zip(to.iter()) {
                if let Some(i) = stem.rfind(f) {
                    stem.replace_range(i..i + f.len(), t);
                }
            }
            stem
        }
    };
    match gradation {
        Some(g) => g.apply(&strong, crate::kotus::Grade::Weak),
        None => strong,
    }
}

/// The infinitive with the first of the `a/b`-style alternatives stripped off
/// its end, else the infinitive itself.
fn strip_any(word: &str, alternatives: &str) -> String {
    for alt in alternatives.split('/') {
        if let Some(stripped) = word.strip_suffix(alt) {
            return stripped.to_string();
        }
    }
    word.to_string()
}

/// The consonant stem of a type-3 verb, which the present vowel stem cannot
/// make: `tulla` is `tul`, `mennä` is `men`, `nousta` is `nous`, `juosta` is
/// `juos`. It is the infinitive with its consonant ending off, keeping the `s`
/// of the `-sta` verbs. A class that has no such stem gives `None`.
fn consonant_stem(infinitive: &str) -> Option<String> {
    for end in ["stä", "sta", "lä", "la", "nä", "na", "rä", "ra"] {
        if infinitive.ends_with(end) {
            let cut = infinitive.chars().count() - 2;
            return Some(infinitive.chars().take(cut).collect());
        }
    }
    None
}

/// The stem a verb's imperative hangs off. A type-3 verb uses its consonant
/// stem (`tulla` is `tulkoon`, `pierrä` is `pierköön`), and a `-ta/-tä` verb
/// its stem without the final vowel (`salata` is `salatkoon`, `tarvita` is
/// `tarvitkoon`, `vanheta` is `vanhetkoon`). A verb with none of those uses
/// the strong present stem.
fn imperative_stem(lemma: &str) -> Option<String> {
    if let Some((_, imp, _, _)) = hda_stem(lemma) {
        return Some(imp.to_string());
    }
    if let Some(cs) = consonant_stem(lemma) {
        return Some(cs);
    }
    if lemma.ends_with("ta") || lemma.ends_with("tä") {
        let n = lemma.chars().count();
        return Some(lemma.chars().take(n.saturating_sub(1)).collect());
    }
    None
}

/// The Hda irregulars `nähdä` and `tehdä`, whose stems the rules cannot make:
/// the present weak (`näe`), the imperative (`näh`), the past (`nä`), and the
/// past participle (`nähnyt`).
fn hda_stem(lemma: &str) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    match lemma {
        "nähdä" => Some(("näe", "näh", "nä", "nähnyt")),
        "tehdä" => Some(("tee", "teh", "te", "tehnyt")),
        _ => None,
    }
}

/// The model verb and its third person past of every verb class, from the
/// catalogue.
///
/// The imperfect is a fact about the word and not about its type: `muistaa` is
/// `muisti` and `kaivaa` is `kaivoi`, which no stem rule reaches. The model's
/// own past, projected onto a lemma's stem, is what gives any member of the
/// class its own past (see [`past_stem`]).
pub const MODEL_VERBS: &[(ClassId, &str, &str)] = &[
    (52, "sanoa", "sanoi"),
    (53, "muistaa", "muisti"),
    (54, "huutaa", "huusi"),
    (55, "soutaa", "souti"),
    (56, "kaivaa", "kaivoi"),
    (57, "saartaa", "saarsi"),
    (58, "laskea", "laski"),
    (59, "tuntea", "tunsi"),
    (60, "lähteä", "lähti"),
    (61, "sallia", "salli"),
    (62, "voida", "voi"),
    (63, "saada", "sai"),
    (64, "juoda", "joi"),
    (65, "käydä", "kävi"),
    (66, "rohkaista", "rohkaisi"),
    (67, "tulla", "tuli"),
    (68, "tupakoida", "tupakoi"),
    (69, "valita", "valitsi"),
    (70, "juosta", "juoksi"),
    (71, "nähdä", "näki"),
    (72, "vanheta", "vanheni"),
    (73, "salata", "salasi"),
    (74, "katketa", "katkesi"),
    (75, "selvitä", "selvisi"),
    (76, "taitaa", "taisi"),
];

/// The past stem of a verb, projected from its class's model verb.
///
/// The imperfect is not the present stem with `-i`: `muistaa` is `muisti` and
/// `kaivaa` is `kaivoi`, a change no type rule makes. The model verb's own past
/// carries it, and a lemma's past is the model's past with the model's stem
/// replaced by the lemma's stem, harmonised: `salata` gives `salasi`, and
/// `hellätä` of the same class gives `helläsi`. For the model verb itself the
/// projection is the model's past exactly.
///
/// The `gradation` argument is accepted because present and past stems are
/// obtained through the same class-aware API, but it is intentionally not
/// applied here. The model's third person past already carries the class's
/// past-grade alternation; applying present-tense gradation would apply the
/// alternation twice or at the wrong morphological boundary.
pub fn past_stem(infinitive: &str, class: ClassId, _gradation: Option<Gradation>) -> String {
    let Some((_, model_infinitive, model_past)) = MODEL_VERBS.iter().find(|(c, _, _)| *c == class)
    else {
        return String::new();
    };
    // The model's stem is its shared prefix with its own past, and the
    // infinitive ending is the rest of the infinitive; the lemma's stem is the
    // lemma without that ending.
    let stem_len = crate::spec::lcp_len(model_infinitive, model_past)
        .min(model_infinitive.len())
        .min(model_past.len());
    let stem_chars = model_infinitive[..stem_len].chars().count();
    let ending_chars = model_infinitive.chars().count() - stem_chars;
    let lemma_stem: String = infinitive
        .chars()
        .take(infinitive.chars().count().saturating_sub(ending_chars))
        .collect();
    let ending = &model_past[stem_len..];
    let ending = crate::grammar::harmonise(ending, crate::grammar::harmony(infinitive));
    let past = format!("{lemma_stem}{ending}");
    past.strip_suffix('i').unwrap_or(&past).to_string()
}

/// The strong stem of a verb, which is what the present third person is built
/// on, from the weak stem the first person shows.
///
/// Consonant gradation weakens the stem between the first and third person:
/// `tuntea` is `tunnen` in the first person but `tuntee` in the third, and
/// `pitää` is `pidän` but `pitää`. The first person has already gradated, so
/// the third person needs the gradation put back the other way, `nn` to `nt`
/// and `d` to `t`. Where the class names a gradation, the weak-to-strong
/// reversal is applied; where it does not, the stem is the stem.
fn strong_stem(stem: &str, gradation: Option<Gradation>, site: GradationSite) -> String {
    if matches!(site, GradationSite::None) {
        return stem.to_string();
    }
    let Some((strong, weak)) = gradation.and_then(|g| g.pair()) else {
        return stem.to_string();
    };
    if weak.is_empty() || strong == weak {
        return stem.to_string();
    }
    // The gradation pair sits before the final vowel: `tunne` -> `tunte` and
    // `lähe` -> `läht`, so the weak member is replaced with the strong one.
    if let Some(at) = find_pair(stem, weak) {
        return format!("{}{strong}{}", &stem[..at], &stem[at + weak.len()..]);
    }
    stem.to_string()
}

/// The irregular strong stems of the `-dä` verbs, which no gradation rule can
/// recover because the consonant disappears entirely in the weak stem.
///
/// `nähdä` is `näen` in the first person and `näkee` in the third, and `tehdä`
/// is `teen` and `tekee`: the `k` of the third person is not a weakened form
/// of anything in the first, so it has to be a table. These are the only two
/// verbs that behave this way.
fn irregular_strong_stem(lemma: &str) -> Option<&'static str> {
    match lemma {
        "nähdä" => Some("näke"),
        "tehdä" => Some("teke"),
        _ => None,
    }
}

/// The weak present stem of the `-dä` irregulars, mirroring
/// [`irregular_strong_stem`]: `nähdä` is `näe` and `tehdä` is `tee`. `juosta`
/// is here too, since its present `juokse` carries a `k` its infinitive has
/// not.
fn irregular_weak_present(lemma: &str) -> Option<&'static str> {
    match lemma {
        "nähdä" => Some("näe"),
        "tehdä" => Some("tee"),
        "juosta" => Some("juokse"),
        _ => None,
    }
}

/// Byte offset of the last occurrence of a consonant pair inside `word`.
fn find_pair(word: &str, pair: &str) -> Option<usize> {
    word.rfind(pair)
}

/// The third person singular present of a verb, by the type's ending table.
///
/// The type names the ending: type 3 takes a literal `-ee` (`tule` is `tulee`,
/// `juokse` is `juoksee`), and the others double the stem's final theme vowel
/// (`laske` is `laskee`, `sano` is `sanoo`, `tapa` is `tapaa`). Without a type
/// the vowel doubling is the fallback.
fn third_person_singular(
    stem: &str,
    gradation: Option<Gradation>,
    site: GradationSite,
    ty: Option<&VerbType>,
    lemma: &str,
) -> String {
    // A type-3 verb forms its 3rd person on the consonant stem, not the vowel
    // stem: `pierrä` is `pieree` (pier + ee), `tulla` is `tulee` (tul + ee),
    // where the vowel stem `piere`/`tule` would give a doubled vowel on top.
    let consonant = ty
        .filter(|v| v.present == PresentStem::AddE)
        .and_then(|_| consonant_stem(lemma));
    // `juosta` puts a `k` back in its 3rd person that its infinitive and its
    // potential (juossee) never carry: `juos` is `juoksee`.
    let consonant = match lemma {
        "juosta" => Some("juoks".to_string()),
        _ => consonant,
    };
    let base = match &consonant {
        Some(cs) => cs.clone(),
        None => irregular_strong_stem(lemma)
            .map(str::to_string)
            .unwrap_or_else(|| strong_stem(stem, gradation, site)),
    };
    let endings = ty
        .map(|v| v.endings_3sg)
        .unwrap_or(&[Ending3sg::DoubleThemeVowel]);
    endings
        .iter()
        .map(|ending| match ending {
            Ending3sg::Literal(s) => format!("{base}{s}"),
            Ending3sg::DoubleThemeVowel => {
                // A stem that already ends in two vowels is the complete third
                // person: `hellää`, `voi`, `tapaa`. One that ends in a single
                // vowel doubles it: `laske` -> `laskee`, `puhu` -> `puhuu`,
                // `sano` -> `sanoo`, `tunte` -> `tuntee`.
                let mut chars: Vec<char> = base.chars().collect();
                let n = chars.len();
                if let Some(&last) = chars.last() {
                    let already_two = n >= 2 && is_vowel(chars[n - 2]);
                    if is_vowel(last) && !already_two {
                        chars.push(last);
                    }
                }
                chars.into_iter().collect()
            }
        })
        .next()
        .unwrap_or_else(|| base.clone())
}

/// The passive stem, as the catalogue's seventh slot gives it.
///
/// That slot is the passive past illative, so it is the passive stem with one
/// of the `-i` endings on the end, and the only question is which. The past
/// doubles its `t` and the present does not, so `laskettiin` gives `laske` by
/// way of `-ttiin` while `käytiin` gives `käy` by way of `-tiin`.
///
/// The three endings leave the same word three ways, and which of them is the
/// stem is not always answerable by the ending alone: `käytiin` is the stem
/// `käy` with a `t` in the ending, but `nähtiin` is the stem `näht` with none,
/// and the word is the same shape in both. What settles it is that a passive
/// stem always ends in a vowel or a `t`, since the ending has to have something
/// to attach to. `nätiin` therefore cannot be a stem while `näht` can, and
/// `käy` can and `käyt` is the same stem again, one `t` further on.
///
/// Taking the stem from here rather than from the first person matters,
/// because gradation is invisible in the first person: `nähdä` is `näen` in
/// the first person and `näht` here, and `tulla` is `tulen` and `tult`.
pub fn passive_stem(passive_illative: &str) -> Option<&str> {
    passive_illative
        .strip_suffix("ttiin")
        .into_iter()
        .chain(passive_illative.strip_suffix("tiin"))
        .chain(passive_illative.strip_suffix("iin"))
        .find(|s| {
            s.chars()
                .next_back()
                .is_some_and(|c| is_vowel(c) || c == 't')
        })
        .filter(|s| !s.is_empty())
}

/// What one verb's passive amounts to, once the catalogue form and the type
/// between them are taken apart.
///
/// The seventh slot fixes the stem and the past, and the lemma fixes the
/// present, because the two halves answer to different things. The past needs
/// only the slot: `nähtiin` is the stem `näht` and the ending `iin`, while
/// `laskettiin` is the stem `laske` and the ending `ttiin`, and the word says
/// which. The present is the other way about. `nähtään` and `nähdään` have the
/// same stem and the same ending and differ only in that one is a word, and
/// nothing in `näht` or in `nähtiin` says so. The lemma does: `nähdä` is of
/// the `-dä` type, and that type takes the `d` of its own alternation, while
/// `tulla` and `juosta` are not, and take a bare vowel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Passive<'a> {
    /// The passive stem, as the seventh slot has it.
    pub stem: &'a str,
    /// The `t` of the past `-i` endings that the stem does not already carry:
    /// none at all for `nähtiin`, one for `käytiin`, two for `laskettiin`.
    pub past_t: u32,
    /// Whether the verb is of the `-dä`, `-da` or `-ida` types, which take a
    /// `d` in the present passive. Read off the lemma by `passive_of`.
    pub d_type: bool,
    /// Whether the verb is of the `-etä` type, whose passive stem has lost the
    /// `e` of its theme and has to get it back, so that `selvitä` gives
    /// `selvitetään` and not `selvitään`.
    pub e_type: bool,
    /// The harmony of the verb, from the lemma.
    pub harmony: Harmony,
}

/// The passive of one verb, from the catalogue's seventh slot and the lemma.
///
/// The lemma is only consulted for its type. The `-dä` and `-da` types are the
/// ones whose infinitive ends in `dä` or `da`, and the `-ida` type the one
/// ending in `ida`, and those are the verbs whose present passive has the `d`
/// in it: `nähdään`, `saadaan`, `käytetään`, `tupakoidaan`. Nothing else ends
/// that way, `valita` and `salata` among them, and reading the ending as
/// enough to say was what put the `t` in `nähtään` and `voitaan`.
pub fn passive_of<'a>(lemma: &str, passive_illative: &'a str) -> Option<Passive<'a>> {
    let stem = passive_stem(passive_illative)?;
    // The `t`s left between the stem and the `-iin`, which is the whole of the
    // difference between `nähtiin`, `käytiin` and `laskettiin`.
    let past_t = passive_illative
        .strip_prefix(stem)
        .and_then(|rest| rest.strip_suffix("iin"))
        .unwrap_or_default()
        .chars()
        .filter(|c| *c == 't')
        .count() as u32;
    let d_type = lemma.ends_with("dä") || lemma.ends_with("da") || lemma.ends_with("ida");
    let e_type = ["teä", "tea", "etä", "eta", "itä"]
        .iter()
        .any(|e| lemma.ends_with(e));
    Some(Passive {
        stem,
        past_t,
        d_type,
        e_type,
        // The lemma, and not the first person, is where the harmony is safe.
        // The first person of these verbs is often the progressive one, and it
        // has the theme vowel back in the middle of it: `vanheta` is `vanhenen`
        // in the first person, whose last vowel is the `e` of the `nen` and
        // would make the back verb `vanheta` look like a front one and give
        // the `vanhetään` that is not a word. The infinitive ends in the vowel
        // of its own harmony, so it says `vanheta` for what it is.
        harmony: harmony(lemma),
    })
}

/// The forms of a verb, from the two forms the catalogue does give that are
/// enough to find the stem: the first person singular present and the third
/// person singular past.
///
/// The second element of each pair says whether the form is a plain reading of
/// the stem, and so is fit to be offered as a suggestion. The rest are
/// speculative: Finnish has too many one-off verb endings to enumerate them
/// honestly, and the checker can lean on the word list for the ones it misses.
pub fn verb(present1: &str, past3: &str) -> Vec<Form> {
    verb_with_passive(present1, past3, None, None, "")
}

/// The forms of a verb, from its present stem and its past stem.
///
/// The two stems are what the catalogue forms and the infinitive both reduce
/// to: the present stem is the first person without its `-n`, and the past
/// stem is the third person past without its `-i`. [`verb_with_passive`]
/// supplies them from the catalogue's two forms and [`verb_from_infinitive`]
/// from the infinitive and the class, and the body here is shared.
fn verb_core(
    stem: &str,
    past: &str,
    passive: Option<Passive<'_>>,
    gradation: Option<Gradation>,
    lemma: &str,
    kotus: Option<&KotusVerb>,
) -> Vec<Form> {
    let mut out: Vec<Form> = Vec::with_capacity(96);
    let mut add = |s: String, tags: Tags, confidence: Confidence| {
        if s.chars().count() >= 3 {
            out.push(
                Form::new(s, tags)
                    .of_pos(PartOfSpeech::Verb)
                    .with(confidence),
            );
        }
    };
    if stem.is_empty() {
        return out;
    }
    // A verb harmonises on the stem without its theme vowel, so `laske-` is a
    // back verb even though it ends in `e`, and `käydä-` is a front one. Only
    // the one theme vowel comes off: trimming the whole trailing run would
    // leave `saa` as `s`, and a word with no vowel left harmonises to the front
    // by default, which would give `saatään` for the `saadaan` of `saada-`.
    // That stem is also what the past tense and the negative are built on.
    let bare: String = {
        let mut b = stem.to_string();
        b.pop();
        b
    };
    let bare = bare.as_str();
    let h = harmony(bare);
    let a = h.a();
    // The class's type, when it is known, is what drives the 3rd person
    // endings, the gradation site and the `-ida` behaviour below.
    let verb_type = kotus.map(|v| &VERB_TYPES[v.ty as usize - 1]);
    let gradation_site = verb_type
        .map(|v| v.gradation_site)
        // Without a class the strong grade is recovered the common way, before
        // the theme vowel, as the type-1/4/5/6 table names it.
        .unwrap_or(GradationSite::WeakBeforeFinalVowel);
    // The `-ida`/`-idä` verbs (`tupakoida`), whose stem ends in a diphthong,
    // take a `d` in the present passive (`tupakoidaan`), which the catalogue's
    // passive stem supplies; their third person present is the bare `tupakoi`.

    // Present tense. The stem already carries its theme vowel, so the third
    // person singular adds a single vowel: `laske` gives `laskee`. The `o` and
    // `y` are for the stems that have no theme vowel of their own, as `luku`
    // gives `lukua` and `lukuy`.
    //
    // A bare `laske` is the second person imperative, and a bare `tapaa` is both
    // that and the third person present, the two being one word there and two
    // words here. Both readings are kept, since which one a writer meant is
    // not something the form can say. They are kept as two readings rather than
    // united into one set of tags, because they disagree: one is imperative and
    // the other indicative, and a mood that is in dispute is not a mood, so
    // uniting them would lose the mood of both and leave a word that is neither
    // an imperative nor a third person present.
    // A bare stem is the second person imperative: `laske`, `sano`. The third
    // person present is a different word and is built separately, doubled
    // below.
    // The imperative. The bare stem is the second person singular: `laske`,
    // `sano`, `puhu`. (The same spelling is the first person present for a
    // `tapaa`-type verb, and the two readings are kept apart.) The remaining
    // persons hang the imperative endings off the stem by the verb's harmony:
    // `laskekoon`, `laskekaamme`, `laskekaa`, `laskekoot`. A type-3 verb hangs
    // them off its consonant stem instead: `pierrä` is `pierköön`, `tulla` is
    // `tulkoon`.
    let form_stem =
        imperative_stem(lemma).unwrap_or_else(|| strong_stem(stem, gradation, gradation_site));
    // The imperative's harmony is the verb's own, read off the infinitive: a
    // `vanheta` has its `a` hidden behind the `e` of its stem, so reading it
    // off the stem would give a front `vanhetkää` where the word is
    // `vanhetkaa`.
    let imp_h = if lemma.is_empty() { h } else { harmony(lemma) };
    let imp_a = imp_h.a();
    let imp_o = imp_h.o();
    add(
        stem.to_string(),
        Tags::finite(Tense::Present, Person::Second, Number::Singular).mood(Mood::Imperative),
        Confidence::Plain,
    );
    for (ending, person, number) in [
        (format!("k{imp_o}{imp_o}n"), Person::Third, Number::Singular),
        (format!("k{imp_a}{imp_a}mme"), Person::First, Number::Plural),
        (format!("k{imp_a}{imp_a}"), Person::Second, Number::Plural),
        (format!("k{imp_o}{imp_o}t"), Person::Third, Number::Plural),
    ] {
        add(
            format!("{form_stem}{ending}"),
            Tags::finite(Tense::Present, person, number).mood(Mood::Imperative),
            Confidence::Plain,
        );
    }
    add(
        format!("{stem}t"),
        Tags::finite(Tense::Present, Person::Second, Number::Singular),
        Confidence::Plain,
    );
    // The third person singular follows the type's ending table (see
    // [`third_person_singular`]). The `-ida` subtype takes its own `-tsee` as
    // well (`tupakoitsee`).
    let present3 = third_person_singular(stem, gradation, gradation_site, verb_type, lemma);
    add(
        present3,
        Tags::finite(Tense::Present, Person::Third, Number::Singular),
        Confidence::Plain,
    );
    for (e, person, number) in [
        ("mme", Person::First, Number::Plural),
        ("tte", Person::Second, Number::Plural),
        ("n", Person::First, Number::Singular),
    ] {
        add(
            format!("{stem}{e}"),
            Tags::finite(Tense::Present, person, number),
            Confidence::Plain,
        );
    }
    // The third person plural present is the stem with a `-vat` or `-vät`, by
    // the verb's harmony: `sano` -> `sanovat`, `laske` -> `laskevat`,
    // `mene` -> `menevät`. Like the third person singular it is a real
    // paradigm form and is offered as a correction.
    let vat = if (if lemma.is_empty() { h } else { harmony(lemma) }) == Harmony::Back {
        "vat"
    } else {
        "vät"
    };
    let present3pl_stem = irregular_strong_stem(lemma)
        .map(str::to_string)
        .unwrap_or_else(|| strong_stem(stem, gradation, gradation_site));
    add(
        format!("{present3pl_stem}{vat}"),
        Tags::finite(Tense::Present, Person::Third, Number::Plural),
        Confidence::Plain,
    );

    // Past tense. The past stem is the catalogue's own past form less its
    // `-i`, not the present stem less its theme vowel, because the two need
    // not agree: `huuta-` gradates to `huusi`, and only the past form knows
    // that.
    if !past.is_empty() {
        // The Hda irregulars have a short past stem for the 1st/2nd persons
        // (`näin`, `näit`, `näimme`, `näitte`) and keep the strong `näk` for
        // the 3rd (`näki`).
        let hda_past = hda_stem(lemma).map(|s| s.2.to_string());
        // A gradating verb weakens the past stem for the 1st and 2nd persons:
        // `teroitti` is `teroitt-`, and the 2nd person is `teroitit` on the
        // weak `teroit-`, not the strong `teroittit`. The 3rd person keeps the
        // strong stem (`teroitti`, `teroittivat`).
        let weak_past = gradation.map(|g| g.apply(past, crate::kotus::Grade::Weak));
        for (e, person, number) in [
            ("in", Person::First, Number::Singular),
            ("it", Person::Second, Number::Singular),
            ("i", Person::Third, Number::Singular),
            ("imme", Person::First, Number::Plural),
            ("itte", Person::Second, Number::Plural),
        ] {
            let use_past = if person == Person::Third {
                past
            } else {
                hda_past.as_deref().or(weak_past.as_deref()).unwrap_or(past)
            };
            add(
                format!("{use_past}{e}"),
                Tags::finite(Tense::Past, person, number),
                Confidence::Plain,
            );
        }
        // The third person plural past, built on the strong past stem:
        // `teroittivat`, `laskivat`. It is a regular form of the paradigm and
        // is offered as plain, in the verb's own harmony (`laskivat`, not
        // `laskivät`).
        let past_vat = if (if lemma.is_empty() { h } else { harmony(lemma) }) == Harmony::Back {
            "ivat"
        } else {
            "ivät"
        };
        add(
            format!("{past}{past_vat}"),
            Tags::finite(Tense::Past, Person::Third, Number::Plural),
            Confidence::Plain,
        );
    }

    // Conditional mood. It is built with the `-isi` marker on a stem derived
    // from the strong present stem, and the present person endings: `teroittaa`
    // gives `teroittaisi` on `teroitta`, `laskea` gives `laskisi` on `lask`,
    // `saada` gives `saisin` on `sa`, `tuntea` gives `tuntisi` on `tunt` and
    // `tupakoida` gives `tupakoisi` on `tupako`. A trailing `e` or `i` of the
    // present stem drops before `-isi` (`laske` is `lask`, `tupakoi` is
    // `tupako`), as does one of a doubled theme vowel (`tapaa` is `tapa`,
    // `saa` is `sa`), while a single theme vowel is kept (`muistaa` is
    // `muistaisi`, not `muistisi`).
    if !past.is_empty() {
        let mut conditional_stem = irregular_strong_stem(lemma)
            .map(str::to_string)
            .unwrap_or_else(|| strong_stem(stem, gradation, gradation_site));
        if conditional_stem.ends_with('e') || conditional_stem.ends_with('i') {
            conditional_stem.pop();
        } else {
            let mut chars = conditional_stem.chars().rev();
            if let (Some(last), Some(second)) = (chars.next(), chars.next())
                && (last == 'a' || last == 'ä')
                && last == second
            {
                conditional_stem.pop();
            }
        }
        let conditional = format!("{conditional_stem}isi");
        for (e, person, number) in [
            ("n", Person::First, Number::Singular),
            ("t", Person::Second, Number::Singular),
            ("", Person::Third, Number::Singular),
            ("mme", Person::First, Number::Plural),
            ("tte", Person::Second, Number::Plural),
        ] {
            add(
                format!("{conditional}{e}"),
                Tags::finite(Tense::Present, person, number).mood(Mood::Conditional),
                Confidence::Plain,
            );
        }
        let vat = if (if lemma.is_empty() { h } else { harmony(lemma) }) == Harmony::Back {
            "vat"
        } else {
            "vät"
        };
        add(
            format!("{conditional}{vat}"),
            Tags::finite(Tense::Present, Person::Third, Number::Plural).mood(Mood::Conditional),
            Confidence::Plain,
        );
    }

    // Potential mood. A type-3 verb forms it on its consonant stem, where the
    // `n` of the `-nee` ending assimilates into the stem's final consonant:
    // `tulla` is `tullee`, `juosta` is `juossee`, `mennä` is `mennee`. A vowel
    // stem takes the strong grade with `-nee` (`laskea` is `laskenee`, `puhua`
    // is `puhunee`), and a `-dä`/`-da` verb its stem without the `d` (`nähdä`
    // is `nähnee`).
    let potential = if let Some(cs) = consonant_stem(lemma) {
        let last = cs.chars().next_back().expect("consonant stem");
        format!("{cs}{last}ee")
    } else if lemma.ends_with("ta") || lemma.ends_with("tä") {
        // A `-ta/-tä` verb (type 4/5/6) forms the potential on its vowel stem
        // with `-nnee`: `salaa` is `salannee`, `tarvi` is `tarvinnee`, `vanhen`
        // is `vanhennee`. The ending is the same whatever the verb's vowels
        // are: a potential is never `-nyt`, so a front `-tä` verb has `jyllänee`
        // and `jyllännee` where a back one has `sala` and `salannee`. Only the
        // `n` is in question, and only type 6 has already got one of its own
        // (`vanhen` supplies it), which is why that arm takes the bare `nee`.
        let (base, ending) = match verb_type.map(|v| v.present) {
            Some(PresentStem::Replace("eta/etä", "ene/ene")) => {
                // type 6: `vanhene` -> `vanhen`, single `-nee`.
                (stem.chars().take(stem.chars().count() - 1).collect(), "nee")
            }
            Some(PresentStem::Replace(_, _)) => {
                // type 5: `tarvitse` -> `tarvi`.
                (
                    stem.strip_suffix("tse").unwrap_or(stem).to_string(),
                    "nnee",
                )
            }
            _ => {
                // type 4: `salaa` -> `sala`.
                (stem.chars().take(stem.chars().count() - 1).collect(), "nnee")
            }
        };
        format!("{base}{ending}")
    } else if lemma.ends_with("dä") || lemma.ends_with("da") {
        let potential_stem = lemma
            .chars()
            .take(lemma.chars().count() - 2)
            .collect::<String>();
        format!("{potential_stem}nee")
    } else {
        format!("{}nee", strong_stem(stem, gradation, gradation_site))
    };
    // The potential is a full paradigm, not just the 3rd person. The stem is
    // the 3rd person without its final vowel, and the person endings are the
    // present ones: `laskea` is `laskenen`, `laskenet`, `laskenee`,
    // `laskenemme`, `laskenette`, `laskenevat`.
    let pot_a = if lemma.is_empty() {
        a
    } else {
        harmony(lemma).a()
    };
    let pot_vat = if pot_a == 'ä' { "vät" } else { "vat" };
    let pot_stem = potential
        .chars()
        .take(potential.chars().count() - 1)
        .collect::<String>();
    add(
        potential,
        Tags::finite(Tense::Present, Person::Third, Number::Singular).mood(Mood::Potential),
        Confidence::Plain,
    );
    for (suffix, person, number) in [
        ("n", Person::First, Number::Singular),
        ("t", Person::Second, Number::Singular),
        ("mme", Person::First, Number::Plural),
        ("tte", Person::Second, Number::Plural),
    ] {
        add(
            format!("{pot_stem}{suffix}"),
            Tags::finite(Tense::Present, person, number).mood(Mood::Potential),
            Confidence::Plain,
        );
    }
    add(
        format!("{pot_stem}{pot_vat}"),
        Tags::finite(Tense::Present, Person::Third, Number::Plural).mood(Mood::Potential),
        Confidence::Plain,
    );

    // The past participle, which uses its own stem rather than the present one:
    // `tulla` is `tullut` (not `tulenut`), `tavata` is `tavannut` (not
    // `tavaanut`), `laskea` is `laskenut`. A type-3 verb doubles its final
    // consonant on the `-ut`/`-yt` (`tul` is `tullut`, `juos` is `juossut`,
    // `men` is `mennyt`); a `-ta/-tä` verb takes `-nnut` (`tava` is `tavannut`,
    // `tarvi` is `tarvinnut`, `vanhe` is `vanhennut`); the rest take `-nut`.
    let instr = |tense: Tense, number: Number| Tags {
        tense: Some(tense),
        mood: Some(Mood::Instructive),
        number: Some(number),
        ..Tags::UNKNOWN
    };
    // The participle's harmony is the verb's own, read off the infinitive: a
    // `vanheta` has its `a` hidden behind the `e` of its stem, so reading it
    // off the stem would give a front `vanhenyt` where the word is `vanhennut`.
    let part_a = if lemma.is_empty() {
        a
    } else {
        harmony(lemma).a()
    };
    let past_ut = if part_a == 'ä' { "yt" } else { "ut" };
    // `-nnut` is the whole suffix the participle of a `-ta/-tä` verb wears, so
    // fronting it fronts the `u` and nothing else: `levetä` is `levennyt`, the
    // same two `n` as `vanhennut` and not the one that `vanheta`'s hidden `a`
    // would tempt (see the note above).
    let past_nnut = if part_a == 'ä' { "nnyt" } else { "nnut" };
    let past_nut = if part_a == 'ä' { "nyt" } else { "nut" };
    let past_part: String = if let Some((_, _, _, pp)) = hda_stem(lemma) {
        pp.to_string()
    } else if let Some(cs) = consonant_stem(lemma) {
        let last = cs.chars().next_back().expect("consonant stem");
        format!("{cs}{last}{past_ut}")
    } else if lemma.ends_with("ta") || lemma.ends_with("tä") {
        format!("{}{past_nnut}", strip_any(lemma, "ta/tä"))
    } else {
        // A type-1/2 verb forms the participle on the strong present stem:
        // `teroittaa` is `teroittanut` on `teroitta`, not the weak `teroitanut`.
        let strong_present = irregular_strong_stem(lemma)
            .map(str::to_string)
            .unwrap_or_else(|| strong_stem(stem, gradation, gradation_site));
        format!("{strong_present}{past_nut}")
    };
    // The plural of the past participle, which turns the final `-ut`/`-yt`
    // into `-eet`: `laskenut` is `laskeneet`, `tullut` is `tulleet`,
    // `tavannut` is `tavanneet`.
    let past_part_pl = format!(
        "{}eet",
        past_part
            .strip_suffix("yt")
            .or_else(|| past_part.strip_suffix("ut"))
            .unwrap_or(&past_part)
    );
    add(
        past_part.clone(),
        instr(Tense::Past, Number::Singular),
        Confidence::Plain,
    );
    add(
        past_part_pl.clone(),
        instr(Tense::Past, Number::Plural),
        Confidence::Plain,
    );

    // Passive. The stem is the catalogue's own, because gradation puts it out
    // of reach of the first person: `nähdä` is `näen` in the first person but
    // `näht-` here, and the first person alone would give `nä-` and a wrong
    // `näetään`. Without a catalogue stem the best guess is the first-person
    // stem with its theme vowel put back, which is right whenever the verb does
    // not gradate.
    // The guess is only made when there is no catalogue stem, but it is owned
    // out here so that the two arms of the `match` can both hand back a borrow.
    let guess = passive.is_none().then(|| {
        let mut g = stem.to_string();
        if !g
            .chars()
            .next_back()
            .is_some_and(|c| is_vowel(c) && c != 'y')
        {
            g.push(if a == 'ä' { 'e' } else { 'a' });
        }
        g
    });
    let (raw_p_stem, past_t, d_type, e_type, p_h) = match passive {
        Some(p) => (p.stem, p.past_t, p.d_type, p.e_type, p.harmony),
        None => (guess.as_deref().unwrap_or_default(), 1, false, false, h),
    };
    // A gradating verb weakens its passive stem: `teroittaa` is `teroitettaan`
    // on the weak `teroitet`, not the strong `teroittettaan` the model's
    // `muistettiin` projects for it.
    let p_stem = gradation
        .map(|g| g.apply(raw_p_stem, crate::kotus::Grade::Weak))
        .unwrap_or_else(|| raw_p_stem.to_string());
    // The passive harmonises on the verb, not on its own stem. `laske` ends in
    // the theme `e`, which is a front vowel, so reading the stem's own last
    // vowel would make `laskea` a front verb and give `lasketään`; the verb is
    // a back one and gives `lasketaan`.
    let pa = p_h.a();
    let theme = if pa == 'ä' { 'e' } else { 'a' };
    let stem_t = p_stem.ends_with('t');
    let stem_open = p_stem
        .chars()
        .next_back()
        .is_some_and(|c| is_vowel(c) && c != 'y');

    // The present passive, which is the one part the seventh slot says nothing
    // about. What the `t` is depends on the type, and three of the four kinds
    // of answer are decided by the stem alone:
    //
    //   `laske` + `t` + `ään` = `lasketaan`      a stem already in a vowel
    //   `tult` + `` + `aan`   = `tultaan`        a stem already in a `t`
    //   `käy`  + `tet`+ `ään` = `käytetään`      a stem in a `y`
    //   `voi`  + `d` + `aan`  = `voidaan`        the `-dä` and `-da` types
    //
    // The last is the one the stem cannot give. `näht` and `tult` are both
    // stems in a `t`, and they do not agree: `nähdään` is a word and `tultään`
    // is not, because `nähdä` is of the `-dä` type and `tulla` is not. The
    // difference is the `d` of `nähdä` itself, the same alternant the third
    // person past shows in `näki`, and the type is read off the lemma for it.
    // So the `-dä` type also takes the `d` member into the stem, giving
    // `nähd` + `ään`, where the stem alone would have given the `nähtään`
    // that is not a word.
    let mut p_base = p_stem.to_string();
    let p_joiner = if e_type && !p_stem.ends_with('e') {
        // The `-etä` type has no theme vowel left in its passive stem, since the
        // `e` of `selvitä` is the ending of the infinitive and not part of the
        // stem: `selvi` is the stem and the `e` goes back on before the `t`.
        // The same shape as `käy` below, and for the same reason: the stem has
        // no theme vowel of its own, so the `t` goes on, then the theme, then
        // the `t` of the ending. `selvi` is `selvi`, `t` is `selvit`, `e` is
        // `selvite` and the ending is `selvitetään`. The theme is the `e` of
        // the type whichever way the verb harmonises.
        format!("t{}t", 'e')
    } else if d_type {
        if stem_open || p_stem.ends_with('y') {
            // `käy` is the awkward one. Its own `t` goes on first, to make
            // `käyt`, and only then is there a vowel for the theme to sit
            // against, so `käyt` becomes `käyte` and the ending's own `t`
            // follows it: `käy`, `käyte`, `käytetään`. The theme goes between
            // the two `t`s.
            if p_stem.ends_with('y') {
                format!("t{theme}t")
            } else {
                "d".to_string()
            }
        } else {
            // The stem is in a `t`, and the type wants the `d` of the
            // alternation: `näht` becomes `nähd`, and the ending is the bare
            // `-ään` of it, `nähdään`.
            p_base.pop();
            p_base.push('d');
            String::new()
        }
    } else if stem_t {
        String::new()
    } else if stem_open {
        "t".to_string()
    } else {
        // No catalogue stem and a consonant-ending one: put the theme back so
        // the `t` has a vowel to follow.
        p_base = format!("{p_stem}t{theme}");
        "t".to_string()
    };
    // A type-3 verb's passive present uses the doubled-consonant stem, which
    // differs from the past stem `p_base` carries: `mennä` is `mennään` on
    // `menn`, while its past `mentiin` and the mood forms keep `ment`. The
    // present base is the infinitive without its final vowel.
    let type3 = consonant_stem(lemma).is_some();
    let present_base = if type3 {
        lemma
            .chars()
            .take(lemma.chars().count() - 1)
            .collect::<String>()
    } else {
        p_base.clone()
    };
    // The passive present is the stem with the `-n` third person marker on it:
    // `lasketaan`, `käytetään`, `mennään`. It is a finite verb form, not a
    // case, so it carries no case; the shorter `lasketaa`/`lasketan` are not
    // words.
    add(
        format!(
            "{present_base}{}{pa}{pa}n",
            if type3 { "" } else { &p_joiner }
        ),
        Tags {
            tense: Some(Tense::Present),
            number: Some(Number::Singular),
            person: Some(Person::Third),
            voice: Some(Voice::Passive),
            ..Tags::UNKNOWN
        },
        Confidence::Guessed,
    );
    // The negative of the passive present: `ei jäädä`, `ei mennä` (the
    // connegative drops the `-n` and the doubled theme vowel).
    add(
        format!(
            "ei {present_base}{}{pa}",
            if type3 { "" } else { &p_joiner }
        ),
        Tags {
            tense: Some(Tense::Present),
            number: Some(Number::Singular),
            person: Some(Person::Third),
            voice: Some(Voice::Passive),
            polarity: Polarity::Negative,
            ..Tags::UNKNOWN
        },
        Confidence::Plain,
    );

    // The potential, the conditional and the imperative of the passive share a
    // mood stem: the passive stem with the `-ta` marker, single on a `d`-type
    // or consonant stem (`jäädä` is `jäätä`, `tulla` is `tulta`) and doubled on
    // an open one (`laskea` is `lasketta`, `salata` is `salatta`). The endings
    // are `-neen`/`-isiin`/`-koon`, and their negatives are `ei ... -ne`/
    // `-isi` and `älköön ... -ko`.
    let (ta_single, ta_double) = if pa == 'ä' {
        ("tä", "ttä")
    } else {
        ("ta", "tta")
    };
    // The mood stem is the passive stem with the `-ta` marker. A stem that
    // already ends in `t` (`tulla` is `tult`, `nähdä` is `näht`) takes just the
    // theme vowel (`tulta`, `nähtä`); a `d`-type or consonant stem takes the
    // single marker (`jäädä` is `jäätä`); an open one takes the doubled marker
    // (`laskea` is `lasketta`, `salata` is `salatta`).
    let mood_stem = if stem_t {
        format!("{p_stem}{pa}")
    } else if d_type || !stem_open {
        format!("{p_stem}{ta_single}")
    } else {
        format!("{p_stem}{ta_double}")
    };
    let (koon, ko) = if pa == 'ä' {
        ("köön", "kö")
    } else {
        ("koon", "ko")
    };
    let passive_tags = |mood, polarity| Tags {
        tense: Some(Tense::Present),
        number: Some(Number::Singular),
        person: Some(Person::Third),
        mood: Some(mood),
        voice: Some(Voice::Passive),
        polarity,
        ..Tags::UNKNOWN
    };
    for (mood, ending, conneg) in [
        (Mood::Potential, "neen", "ne"),
        (Mood::Conditional, "isiin", "isi"),
    ] {
        add(
            format!("{mood_stem}{ending}"),
            passive_tags(mood, Polarity::Affirmative),
            Confidence::Guessed,
        );
        add(
            format!("ei {mood_stem}{conneg}"),
            passive_tags(mood, Polarity::Negative),
            Confidence::Plain,
        );
    }
    add(
        format!("{mood_stem}{koon}"),
        passive_tags(Mood::Imperative, Polarity::Affirmative),
        Confidence::Guessed,
    );
    add(
        format!("älköön {mood_stem}{ko}"),
        passive_tags(Mood::Imperative, Polarity::Negative),
        Confidence::Plain,
    );

    // The past `-i` forms, whose `t` the seventh slot settles exactly. The slot
    // is an `-iin` ending, and the `t`s in front of it are the stem's own or
    // the ending's: `nähtiin` is `näht` and `iin` with none between,
    // `käytiin` is `käy` and `tiin` with one, `laskettiin` is `laske` and
    // `ttiin` with two. A stem in a `t` never doubles it, since two in a row
    // are not a Finnish ending, so the `t` count can be read off the word.
    // The indefinite passive has no person at all, which is the whole point of
    // it, so these carry a tense and a voice and nothing else but the case.
    let indefinite = |case: Option<Case>| Tags {
        tense: Some(Tense::Past),
        voice: Some(Voice::Passive),
        case,
        ..Tags::UNKNOWN
    };
    let i_t = "t".repeat(past_t as usize);
    for (e, case) in [
        (format!("{i_t}i"), None),
        (format!("{i_t}iin"), Some(Case::Illative)),
    ] {
        add(
            format!("{p_stem}{e}"),
            indefinite(case),
            Confidence::Guessed,
        );
    }

    // The past participle doubles the `-tu` `t` on a stem that carries no `t`
    // of its own, which is why it is `laskettu` and `salattu`, while one
    // already ending in `t` gives the single `t` of `nähty`. The same `t`
    // settles the indefinite `-taa`, so the two go together.
    let part = if stem_t || !stem_open || d_type {
        p_stem.to_string()
    } else {
        format!("{p_stem}t")
    };
    // The active past participle of the passive stem: `laskettu` and, for a
    // stem that had a `y`, `käytty`. A stem that already ends in `t` takes the
    // bare `-u`/`-y` instead, since the ending's `t` would collide: `tult` is
    // `tultu` and `näht` is `nähty`, not `tulttu`/`tultty`.
    let ppart = Tags {
        tense: Some(Tense::Past),
        voice: Some(Voice::Active),
        ..Tags::UNKNOWN
    };
    // The passive past participle, harmony-correct: `teroitettu` for a back
    // verb and `teroitetty` for a front one, not both. A stem that already
    // ends in `t` takes the bare `-u`/`-y` (`tultu`, `nähty`), the rest take
    // `-tu`/`-ty` (`laskettu`, `salattu`).
    let ppart_word = if stem_t {
        if pa == 'ä' {
            format!("{p_stem}y")
        } else {
            format!("{p_stem}u")
        }
    } else if pa == 'ä' {
        format!("{part}ty")
    } else {
        format!("{part}tu")
    };
    // The single-word passive past participle (kept a guess).
    add(ppart_word.clone(), ppart, Confidence::Guessed);
    // The negative of the passive past: `ei teroitettu`.
    add(
        format!("ei {ppart_word}"),
        Tags {
            tense: Some(Tense::Past),
            voice: Some(Voice::Passive),
            polarity: Polarity::Negative,
            ..Tags::UNKNOWN
        },
        Confidence::Plain,
    );
    // The imperative perfect of the passive: `olkoon jääty` and its
    // negation `älköön olko jääty`.
    add(
        format!("olkoon {ppart_word}"),
        Tags {
            tense: Some(Tense::Perfect),
            mood: Some(Mood::Imperative),
            voice: Some(Voice::Passive),
            ..Tags::UNKNOWN
        },
        Confidence::Plain,
    );
    add(
        format!("älköön olko {ppart_word}"),
        Tags {
            tense: Some(Tense::Perfect),
            mood: Some(Mood::Imperative),
            voice: Some(Voice::Passive),
            polarity: Polarity::Negative,
            ..Tags::UNKNOWN
        },
        Confidence::Plain,
    );
    // The compound perfects of the passive: `olla` (and `liene`) plus the
    // passive past participle. The passive takes no person, so each has only
    // the third singular.
    for (aux, neg_aux, tense, mood) in [
        ("on", "ole", Tense::Perfect, Mood::Indicative),
        ("oli", "ollut", Tense::Pluperfect, Mood::Indicative),
        ("lienee", "liene", Tense::Perfect, Mood::Potential),
        ("olisi", "olisi", Tense::Perfect, Mood::Conditional),
    ] {
        let mut pp = |word, pol| {
            add(
                word,
                Tags {
                    tense: Some(tense),
                    mood: Some(mood),
                    voice: Some(Voice::Passive),
                    polarity: pol,
                    ..Tags::UNKNOWN
                },
                Confidence::Plain,
            )
        };
        pp(format!("{aux} {ppart_word}"), Polarity::Affirmative);
        pp(format!("ei {neg_aux} {ppart_word}"), Polarity::Negative);
    }
    if past_t > 0 {
        // The indefinite of the present passive, which takes no person either.
        for e in [format!("{i_t}a"), format!("{i_t}aa")] {
            add(
                format!("{p_stem}{e}"),
                Tags {
                    tense: Some(Tense::Present),
                    voice: Some(Voice::Passive),
                    ..Tags::UNKNOWN
                },
                Confidence::Guessed,
            );
        }
    }

    // The infinitives, built on the verb stem. The first infinitive is the
    // lemma itself. The second is `-essa`/`-essä` in the inessive and `-en` in
    // the instructive (`puhuessa`, `puhuen`). The third is the `ma`/`mä` stem
    // with a case on it (`puhumaan`, `puhumassa`, `puhumasta`, `puhumalla`,
    // `puhumatta`, `puhumaksi`). The fourth is the nominal `-minen`
    // (`puhuminen`).
    let infinitive = |case: Option<Case>| Tags {
        mood: Some(Mood::Infinitive),
        case,
        ..Tags::UNKNOWN
    };
    // The third and fourth infinitives are built on the strong stem, which is
    // the stem the gradation leaves in the strong grade: `näen` is `näke` and
    // `teen` is `teke`, so the ma-stem is `näke`/`teke` and the `-minen` is
    // `näkeminen`/`tekeminen`.
    let strong = irregular_strong_stem(lemma)
        .map(str::to_string)
        .unwrap_or_else(|| strong_stem(stem, gradation, gradation_site));
    // The second infinitive is built on a stem that carries the infinitive's
    // own consonant: `syödä` is `syödessä` on `syöde`, and `nähdä` is
    // `nähdessä` on `nähde`. A `-dä`/`-da` verb keeps its `d` here.
    let inf2 = if lemma.ends_with("dä") || lemma.ends_with("da") {
        let mut s: String = lemma.chars().take(lemma.chars().count() - 2).collect();
        s.push('d');
        s.push('e');
        s
    } else if lemma.ends_with("ta") || lemma.ends_with("tä") {
        // A `-ta`/`-tä` verb (type 4/5/6) builds the second infinitive on the
        // infinitive's own `-ta` stem, not on the strong stem: `vanheta` is
        // `vanhetessa`, `salata` is `salatessa`.
        lemma.chars().take(lemma.chars().count() - 1).collect()
    } else if let Some(cs) = consonant_stem(lemma) {
        // A type-3 verb doubles its final consonant: `tulla` is `tullessa` on
        // `tull`, `mennä` is `mennessä` on `menn`, not the `tuliessa` and
        // `meniessä` of the strong stem's `tule`/`mene`.
        let last = cs.chars().next_back().expect("consonant stem");
        format!("{cs}{last}")
    } else {
        strong.clone()
    };
    // The infinitive harmonises on the infinitive itself: `vanheta` keeps its
    // `a` behind the `e` of `vanhen`, so reading the stem alone would make it
    // a front word and give `vanhenemään` where the word is `vanhenemaan`.
    // When there is no infinitive (the legacy path), the stem's harmony holds.
    let inf_h = if lemma.is_empty() { h } else { harmony(lemma) };
    let inf_a = inf_h.a();
    let ma = if inf_h == Harmony::Front { "mä" } else { "ma" };
    let inf2_ine = if lemma.ends_with("dä") || lemma.ends_with("da") {
        format!("{inf2}ss{inf_a}")
    } else if inf2.ends_with('e') {
        format!("{}iess{inf_a}", &inf2[..inf2.len() - 1])
    } else {
        format!("{inf2}ess{inf_a}")
    };
    add(
        inf2_ine.clone(),
        infinitive(Some(Case::Inessive)),
        Confidence::Plain,
    );
    // The second infinitive inessive takes possessive suffixes: `puhuessa` is
    // `puhuessani`, `puhuessasi`, `puhuessaan`, `puhuessamme`,
    // `puhuessanne`. The third person contracts to `-aan`/`-ään` rather than
    // `-ansa`/`-änsä`.
    let front = a == 'ä';
    for poss in Possessive::ALL {
        let suffix = if poss == Possessive::Third {
            if front { "än" } else { "an" }
        } else {
            poss.suffix(front)
        };
        add(
            format!("{inf2_ine}{suffix}"),
            Tags {
                mood: Some(Mood::Infinitive),
                case: Some(Case::Inessive),
                possessive: Some(poss),
                ..Tags::UNKNOWN
            },
            Confidence::Plain,
        );
    }
    // The instructive is `-en`, but an `-ea`/`-oa` stem inserts an `i`: `puhu`
    // is `puhuen` and `laske` is `laskien`, while a `-dä` verb is `syöden`.
    let inf2_ins = if lemma.ends_with("dä") || lemma.ends_with("da") {
        format!("{inf2}n")
    } else if inf2.ends_with('e') {
        format!("{}ien", &inf2[..inf2.len() - 1])
    } else {
        format!("{inf2}en")
    };
    add(
        inf2_ins,
        infinitive(Some(Case::Instructive)),
        Confidence::Plain,
    );
    // Third infinitive, the ma/mä stem with its six case forms. The stem is the
    // strong stem plus the `ma`/`mä` marker, and the case suffix follows it,
    // harmonised: `puhuma` + `an` is `puhumaan`, `puhuma` + `ssa` is
    // `puhumassa`, and `kysyä` + `än` is `kysymään`.
    let ma_stem = format!("{strong}{ma}");
    for (case_end, case) in [
        (format!("{inf_a}n"), Case::Illative),
        (format!("ss{inf_a}"), Case::Inessive),
        (format!("st{inf_a}"), Case::Elative),
        (format!("ll{inf_a}"), Case::Adessive),
        (format!("tt{inf_a}"), Case::Abessive),
        ("ksi".to_string(), Case::Translative),
    ] {
        add(
            format!("{ma_stem}{case_end}"),
            infinitive(Some(case)),
            Confidence::Plain,
        );
    }
    // Fourth infinitive, the nominal `-minen`.
    add(
        format!("{strong}minen"),
        infinitive(None),
        Confidence::Plain,
    );
    // The fourth infinitive partitive: `jääminen` is `jäämistä`, from the
    // `-minen` stem plus `-stä`.
    add(
        format!("{strong}mist{inf_a}"),
        infinitive(Some(Case::Partitive)),
        Confidence::Plain,
    );
    // The third infinitive instructive: `puhumän` is the ma-stem plus `-n`.
    add(
        format!("{ma_stem}n"),
        infinitive(Some(Case::Instructive)),
        Confidence::Plain,
    );
    // The fifth infinitive, `jäämäisillään`, the ma-stem with `-isillaan`/
    // `-isillään`.
    add(
        format!("{ma_stem}isill{inf_a}{inf_a}n"),
        infinitive(Some(Case::Adessive)),
        Confidence::Guessed,
    );
    // The passive infinitives, on the passive mood stem: the second inessive
    // `jäätäessä` and the third instructive `jäätämän`.
    let passive_inf = |case| Tags {
        mood: Some(Mood::Infinitive),
        voice: Some(Voice::Passive),
        case,
        ..Tags::UNKNOWN
    };
    add(
        format!("{mood_stem}ess{inf_a}"),
        passive_inf(Some(Case::Inessive)),
        Confidence::Guessed,
    );
    add(
        format!("{mood_stem}m{inf_a}n"),
        passive_inf(Some(Case::Instructive)),
        Confidence::Guessed,
    );

    // --- The participle case declensions ---
    // A participle declines like an adjective through the case endings, but
    // each type brings its own stems: `nom` is the nominative, `oblique` the
    // stem the genitive-based singular cases build on, `part_sg`/`illat_sg`
    // the partitive and illative singular, `nom_pl` the nominative plural and
    // `pl_stem` the stem the plural oblique cases build on. The genitive and
    // illative plural endings are the `-en`/`-in` of the `-va` and `-ma` types
    // or the `-den`/`-siin` of the `-nut` type, and `instr_pl`/`comit_pl` are
    // the plural instructive and comitative. These are the rarest forms in the
    // paradigm, so they are all Guessed.
    let pa = part_a;
    let mut decline = |nom: &str,
                       oblique: &str,
                       part_sg: &str,
                       illat_sg: &str,
                       nom_pl: &str,
                       pl_stem: &str,
                       part_pl: &str,
                       gen_pl: &str,
                       illat_pl: &str,
                       instr_pl: &str,
                       comit_pl: &str,
                       tense: Tense,
                       voice: Voice| {
        let with = |case: Case, number: Number| Tags {
            tense: Some(tense),
            mood: Some(Mood::Instructive),
            voice: Some(voice),
            case: Some(case),
            number: Some(number),
            ..Tags::UNKNOWN
        };
        add(
            nom.to_string(),
            with(Case::Nominative, Number::Singular),
            if voice == Voice::Active {
                Confidence::Plain
            } else {
                Confidence::Guessed
            },
        );
        for (e, c) in [
            ("n".to_string(), Case::Genitive),
            (format!("ss{a}"), Case::Inessive),
        ] {
            add(
                format!("{oblique}{e}"),
                with(c, Number::Singular),
                Confidence::Guessed,
            );
        }
        for (e, c) in [
            (format!("st{a}"), Case::Elative),
            (format!("ll{a}"), Case::Adessive),
            (format!("lt{a}"), Case::Ablative),
            ("lle".to_string(), Case::Allative),
            (format!("n{a}"), Case::Essive),
            ("ksi".to_string(), Case::Translative),
            (format!("tt{a}"), Case::Abessive),
        ] {
            add(
                format!("{oblique}{e}"),
                with(c, Number::Singular),
                Confidence::Guessed,
            );
        }
        add(
            part_sg.to_string(),
            with(Case::Partitive, Number::Singular),
            Confidence::Guessed,
        );
        add(
            illat_sg.to_string(),
            with(Case::Illative, Number::Singular),
            Confidence::Guessed,
        );
        add(
            nom_pl.to_string(),
            with(Case::Nominative, Number::Plural),
            Confidence::Guessed,
        );
        for (e, c) in [
            (part_pl.to_string(), Case::Partitive),
            (gen_pl.to_string(), Case::Genitive),
            (format!("ss{a}"), Case::Inessive),
            (format!("st{a}"), Case::Elative),
            (illat_pl.to_string(), Case::Illative),
            (format!("ll{a}"), Case::Adessive),
            (format!("lt{a}"), Case::Ablative),
            ("lle".to_string(), Case::Allative),
            (format!("n{a}"), Case::Essive),
            ("ksi".to_string(), Case::Translative),
            (format!("tt{a}"), Case::Abessive),
        ] {
            add(
                format!("{pl_stem}{e}"),
                with(c, Number::Plural),
                Confidence::Guessed,
            );
        }
        add(
            instr_pl.to_string(),
            with(Case::Instructive, Number::Plural),
            Confidence::Guessed,
        );
        add(
            comit_pl.to_string(),
            with(Case::Comitative, Number::Plural),
            Confidence::Guessed,
        );
    };
    // The present active participle, `-va`/`-vä`: `jäävä`, plural stem
    // `jäävi`.
    let va_nom = format!("{strong}v{pa}");
    decline(
        &va_nom,
        &va_nom,
        &format!("{strong}v{pa}{pa}"),
        &format!("{strong}v{pa}{pa}n"),
        &format!("{va_nom}t"),
        &format!("{strong}vi"),
        &format!("{a}"),
        "en",
        "in",
        &format!("{strong}vein"),
        &format!("{strong}veine"),
        Tense::Present,
        Voice::Active,
    );
    // The passive present participle, on the passive mood stem: `jäätävä`,
    // plural stem `jäätävi`.
    let va_pass_nom = format!("{mood_stem}v{pa}");
    decline(
        &va_pass_nom,
        &va_pass_nom,
        &format!("{mood_stem}v{pa}{pa}"),
        &format!("{mood_stem}v{pa}{pa}n"),
        &format!("{va_pass_nom}t"),
        &format!("{mood_stem}vi"),
        &format!("{a}"),
        "en",
        "in",
        &format!("{mood_stem}vein"),
        &format!("{mood_stem}veine"),
        Tense::Present,
        Voice::Passive,
    );
    // The agent participle, `-ma`/`-mä`: `jäämä`, plural stem `jäämi`.
    let ma_nom = format!("{strong}{ma}");
    decline(
        &ma_nom,
        &ma_nom,
        &format!("{strong}m{pa}{pa}"),
        &format!("{strong}m{pa}{pa}n"),
        &format!("{ma_nom}t"),
        &format!("{strong}mi"),
        &format!("{a}"),
        "en",
        "in",
        &format!("{strong}mein"),
        &format!("{strong}meine"),
        Tense::Present,
        Voice::Active,
    );
    // The past active participle, `-nut`: `jäänyt`, whose genitive-based stem
    // is `jäänee` and plural stem `jäänei`. Its plural genitive and illative
    // are `-den` and `-siin`.
    let past_oblique = past_part_pl.strip_suffix('t').unwrap_or(&past_part_pl);
    let past_pl_stem = format!(
        "{}i",
        past_part_pl.strip_suffix("et").unwrap_or(&past_part_pl)
    );
    decline(
        &past_part,
        past_oblique,
        &format!("{past_part}t{pa}"),
        &format!("{past_oblique}seen"),
        &past_part_pl,
        &past_pl_stem,
        &format!("t{pa}"),
        "den",
        "siin",
        &format!("{past_pl_stem}n"),
        &format!("{past_pl_stem}ne"),
        Tense::Past,
        Voice::Active,
    );
    // The past passive participle, `-tu`: `jääty`. Its declension is thin: a
    // partitive, an illative and an essive in the singular, and a partitive, a
    // genitive, an illative, an essive and a comitative in the plural.
    let ty = ppart_word;
    let ty_tags = |case, number| Tags {
        tense: Some(Tense::Past),
        mood: Some(Mood::Instructive),
        voice: Some(Voice::Passive),
        case: Some(case),
        number: Some(number),
        ..Tags::UNKNOWN
    };
    add(
        format!("{ty}{a}"),
        ty_tags(Case::Partitive, Number::Singular),
        Confidence::Guessed,
    );
    add(
        format!("{ty}yn"),
        ty_tags(Case::Illative, Number::Singular),
        Confidence::Guessed,
    );
    add(
        format!("{ty}n{a}"),
        ty_tags(Case::Essive, Number::Singular),
        Confidence::Guessed,
    );
    add(
        format!("{ty}j{a}"),
        ty_tags(Case::Partitive, Number::Plural),
        Confidence::Guessed,
    );
    add(
        format!("{ty}jen"),
        ty_tags(Case::Genitive, Number::Plural),
        Confidence::Guessed,
    );
    add(
        format!("{ty}ihin"),
        ty_tags(Case::Illative, Number::Plural),
        Confidence::Guessed,
    );
    add(
        format!("{ty}in{a}"),
        ty_tags(Case::Essive, Number::Plural),
        Confidence::Guessed,
    );
    add(
        format!("{ty}ine"),
        ty_tags(Case::Comitative, Number::Plural),
        Confidence::Guessed,
    );

    // The negative forms. Finnish negation is analytic: the negative auxiliary
    // `en`, `et`, `ei`, `emme`, `ette`, `eivät` precedes a connegative form of
    // the main verb. The present connegative is the bare stem (`en puhu`), and
    // the past is the past participle (`en puhunut`, `emme puhuneet`).
    let neg_aux: &[(Person, Number, &str)] = &[
        (Person::First, Number::Singular, "en"),
        (Person::Second, Number::Singular, "et"),
        (Person::Third, Number::Singular, "ei"),
        (Person::First, Number::Plural, "emme"),
        (Person::Second, Number::Plural, "ette"),
        (Person::Third, Number::Plural, "eivät"),
    ];
    for (person, number, aux) in neg_aux {
        let mut neg = |conneg: &str, tense: Tense| {
            add(
                format!("{aux} {conneg}"),
                Tags {
                    tense: Some(tense),
                    person: Some(*person),
                    number: Some(*number),
                    mood: Some(Mood::Indicative),
                    voice: Some(Voice::Active),
                    polarity: Polarity::Negative,
                    ..Tags::UNKNOWN
                },
                Confidence::Plain,
            );
        };
        // Present negative: the bare stem.
        neg(stem, Tense::Present);
        // Past negative: the past participle, singular and plural.
        let part = if *number == Number::Plural {
            &past_part_pl
        } else {
            &past_part
        };
        neg(part, Tense::Past);
    }

    // The perfect and pluperfect, built compositionally from the auxiliary
    // `olla` and the past active participle. The perfect is `olla` in the
    // present plus `puhunut`/`puhuneet`; the pluperfect is `olla` in the past
    // plus the same participle. `olla` is irregular, so its forms are a table.
    let perfect_aux: &[(Person, Number, &str)] = &[
        (Person::First, Number::Singular, "olen"),
        (Person::Second, Number::Singular, "olet"),
        (Person::Third, Number::Singular, "on"),
        (Person::First, Number::Plural, "olemme"),
        (Person::Second, Number::Plural, "olette"),
        (Person::Third, Number::Plural, "ovat"),
    ];
    let pluperfect_aux: &[(Person, Number, &str)] = &[
        (Person::First, Number::Singular, "olin"),
        (Person::Second, Number::Singular, "olit"),
        (Person::Third, Number::Singular, "oli"),
        (Person::First, Number::Plural, "olimme"),
        (Person::Second, Number::Plural, "olitte"),
        (Person::Third, Number::Plural, "olivat"),
    ];
    for (tense, aux_list) in [
        (Tense::Perfect, perfect_aux),
        (Tense::Pluperfect, pluperfect_aux),
    ] {
        for (person, number, aux) in aux_list {
            let part = if *number == Number::Plural {
                &past_part_pl
            } else {
                &past_part
            };
            add(
                format!("{aux} {part}"),
                Tags {
                    tense: Some(tense),
                    person: Some(*person),
                    number: Some(*number),
                    mood: Some(Mood::Indicative),
                    voice: Some(Voice::Active),
                    ..Tags::UNKNOWN
                },
                Confidence::Plain,
            );
        }
    }

    // The compound perfects of the conditional, the potential and the
    // imperative, each `olla` in that mood plus the past active participle.
    // `olla` is irregular in every mood, so the auxiliaries are tables.
    let conditional_aux: &[(Person, Number, &str)] = &[
        (Person::First, Number::Singular, "olisin"),
        (Person::Second, Number::Singular, "olisit"),
        (Person::Third, Number::Singular, "olisi"),
        (Person::First, Number::Plural, "olisimme"),
        (Person::Second, Number::Plural, "olisitte"),
        (Person::Third, Number::Plural, "olisivat"),
    ];
    let potential_aux: &[(Person, Number, &str)] = &[
        (Person::First, Number::Singular, "lienen"),
        (Person::Second, Number::Singular, "lienet"),
        (Person::Third, Number::Singular, "lienee"),
        (Person::First, Number::Plural, "lienemme"),
        (Person::Second, Number::Plural, "lienette"),
        (Person::Third, Number::Plural, "lienevät"),
    ];
    for (mood, aux_list) in [
        (Mood::Conditional, conditional_aux),
        (Mood::Potential, potential_aux),
    ] {
        for (person, number, aux) in aux_list {
            let part = if *number == Number::Plural {
                &past_part_pl
            } else {
                &past_part
            };
            add(
                format!("{aux} {part}"),
                Tags {
                    tense: Some(Tense::Perfect),
                    person: Some(*person),
                    number: Some(*number),
                    mood: Some(mood),
                    voice: Some(Voice::Active),
                    ..Tags::UNKNOWN
                },
                Confidence::Plain,
            );
        }
    }
    // The imperative perfect has only the 3rd persons: `olkoon jäänyt`,
    // `olkoot jääneet`.
    add(
        format!("olkoon {past_part}"),
        Tags {
            tense: Some(Tense::Perfect),
            person: Some(Person::Third),
            number: Some(Number::Singular),
            mood: Some(Mood::Imperative),
            voice: Some(Voice::Active),
            ..Tags::UNKNOWN
        },
        Confidence::Plain,
    );
    add(
        format!("olkoot {past_part_pl}"),
        Tags {
            tense: Some(Tense::Perfect),
            person: Some(Person::Third),
            number: Some(Number::Plural),
            mood: Some(Mood::Imperative),
            voice: Some(Voice::Active),
            ..Tags::UNKNOWN
        },
        Confidence::Plain,
    );

    out.sort_by(|a, b| a.word.cmp(&b.word));
    // The same word can be reached twice, as `laskettu` is by the participle
    // and by the indefinite. The two readings are kept as their union, so
    // nothing that either one knew is lost.
    let mut merged: Vec<Form> = Vec::with_capacity(out.len());
    for form in out {
        match merged.last_mut() {
            Some(last) if last.word == form.word => last.absorb(&form),
            _ => merged.push(form),
        }
    }
    merged
}

/// The forms of a verb, taking the passive stem from the catalogue when it is
/// known. See [`passive_stem`] and [`verb`].
///
/// The two catalogue forms are the first person singular present and the third
/// person singular past, and the two stems they pin down are the present stem
/// and the past stem that [`verb_core`] is built on.
pub fn verb_with_passive(
    present1: &str,
    past3: &str,
    passive: Option<Passive<'_>>,
    gradation: Option<Gradation>,
    lemma: &str,
) -> Vec<Form> {
    let stem = present1.strip_suffix('n').unwrap_or(present1);
    let past = past3.strip_suffix('i').unwrap_or(past3);
    verb_core(stem, past, passive, gradation, lemma, None)
}

/// The forms of a verb, from its infinitive and its class.
///
/// The present stem and the past stem are read off the infinitive by the
/// class's type and the [`IRREGULAR_PAST_STEMS`] table, rather than from the
/// catalogue's two inflected forms, so a verb whose own forms the catalogue
/// does not carry is still declined from the same declarative rules as the
/// model verbs.
pub fn verb_from_infinitive(
    infinitive: &str,
    class: ClassId,
    gradation: Option<Gradation>,
    passive: Option<Passive<'_>>,
) -> Vec<Form> {
    let stem = present_stem(infinitive, class, gradation);
    let past = past_stem(infinitive, class, gradation);
    verb_core(
        &stem,
        &past,
        passive,
        gradation,
        infinitive,
        kotus_verb(class),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vhas(p1: &str, p3: &str, want: &str) -> bool {
        verb(p1, p3).iter().any(|f| f.word == want)
    }

    /// Whether a form of a verb is generated, given the lemma so the infinitive
    /// stems can be read off it.
    fn vhas_lemma(lemma: &str, p1: &str, p3: &str, want: &str) -> bool {
        verb_with_passive(p1, p3, None, None, lemma)
            .iter()
            .any(|f| f.word == want)
    }

    fn vhas_inf(lemma: &str, class: ClassId, passive_illative: &str, want: &str) -> bool {
        verb_from_infinitive(lemma, class, None, passive_of(lemma, passive_illative))
            .iter()
            .any(|f| f.word == want)
    }

    #[test]
    fn the_infinitive_path_agrees_with_the_catalogue_forms() {
        // The present stem and the past stem, read off `laskea`'s infinitive by
        // its type, are the same two the catalogue's `lasken` and `laski` give,
        // so the two paths build the same paradigm.
        let from_catalogue = verb_with_passive("lasken", "laski", None, None, "laskea");
        let from_infinitive = verb_from_infinitive("laskea", 58, None, None);
        let cat: Vec<&str> = from_catalogue.iter().map(|f| f.word.as_str()).collect();
        let inf: Vec<&str> = from_infinitive.iter().map(|f| f.word.as_str()).collect();
        assert_eq!(cat, inf, "the infinitive path diverged from the catalogue");
    }

    #[test]
    fn the_infinitive_path_tables_the_irregular_past() {
        // `huutaa` gradates to `huusi`, and the irregular-past table supplies
        // it to the infinitive path exactly as the catalogue form would.
        let f = verb_from_infinitive("huutaa", 54, Some(Gradation(6)), None);
        assert!(f.iter().any(|x| x.word == "huusi"));
        assert!(f.iter().any(|x| x.word == "huusit"));
        assert!(f.iter().any(|x| x.word == "huusivat"));
    }

    #[test]
    fn the_present_stem_matches_every_catalogue_model_verb() {
        // The present stem of every Kotus verb class, read off its model verb's
        // catalogue first-person form. The infinitive and the class alone have
        // to reach each of these.
        let cases: &[(ClassId, &str, &str)] = &[
            (52, "sanoa", "sano"),
            (53, "muistaa", "muista"),
            (54, "huutaa", "huuda"),
            (55, "soutaa", "souda"),
            (56, "kaivaa", "kaiva"),
            (57, "saartaa", "saarra"),
            (58, "laskea", "laske"),
            (59, "tuntea", "tunne"),
            (60, "lähteä", "lähde"),
            (61, "sallia", "salli"),
            (62, "voida", "voi"),
            (63, "saada", "saa"),
            (64, "juoda", "juo"),
            (65, "käydä", "käy"),
            (66, "rohkaista", "rohkaise"),
            (67, "tulla", "tule"),
            (68, "tupakoida", "tupakoi"),
            (69, "valita", "valitse"),
            (70, "juosta", "juokse"),
            (71, "nähdä", "näe"),
            (72, "vanheta", "vanhene"),
            (73, "salata", "salaa"),
            (74, "katketa", "katkea"),
            (75, "selvitä", "selviä"),
            (76, "taitaa", "taida"),
        ];
        for (class, infinitive, want) in cases {
            let grad = kotus_gradation_of(*class);
            let got = present_stem(infinitive, *class, grad);
            assert_eq!(got, *want, "class {class} ({infinitive})");
        }
    }

    fn kotus_gradation_of(class: ClassId) -> Option<Gradation> {
        match class {
            // The gradated model verbs, by their sanalista `*X` code.
            54 | 55 | 60 | 76 => Some(Gradation(6)), // F: t -> d
            57 => Some(Gradation(11)),               // K: rt -> rr
            59 => Some(Gradation(10)),               // J: nt -> nn
            _ => None,
        }
    }

    #[test]
    fn the_verb_classes_are_the_top_of_the_list() {
        assert!(is_verb_class(52));
        assert!(is_verb_class(78));
        assert!(!is_verb_class(51));
        assert!(!is_verb_class(79));
    }

    #[test]
    fn the_kotus_table_maps_every_verb_class_to_a_valid_type() {
        for (class, v) in KOTUS_VERBS {
            assert!(
                (52..=78).contains(class),
                "class {class} outside the verb range"
            );
            assert!(
                (1..=6).contains(&v.ty),
                "class {class} has invalid type {}",
                v.ty
            );
        }
        assert_eq!(KOTUS_VERBS.len(), 78 - 52 + 1);
    }

    #[test]
    fn the_kotus_table_names_the_exception_classes() {
        // The three classes whose type alone does not describe them.
        assert_eq!(
            kotus_verb(68).map(|v| v.subtype),
            Some(Some(VerbSubtype::Oida))
        );
        assert_eq!(kotus_verb(71).map(|v| v.irregular), Some(true));
        assert_eq!(
            kotus_verb(71).map(|v| v.subtype),
            Some(Some(VerbSubtype::Hda))
        );
        // The defective `-aa` verbs generate only the 3rd person.
        assert_eq!(kotus_verb(77).map(|v| v.defective), Some(true));
        assert_eq!(kotus_verb(78).map(|v| v.defective), Some(true));
        // A type-1 class is a plain instance.
        assert_eq!(kotus_verb(53).map(|v| v.defective), Some(false));
        assert_eq!(kotus_verb(53).map(|v| v.subtype), Some(None));
    }

    #[test]
    fn the_type_table_matches_the_specs_six_types() {
        use PresentStem::*;
        assert_eq!(VERB_TYPES[0].present, DropVowel);
        assert_eq!(VERB_TYPES[1].present, Drop("da/dä"));
        assert_eq!(VERB_TYPES[2].present, AddE);
        assert_eq!(VERB_TYPES[2].gradation_site, GradationSite::ConsonantPair);
        assert_eq!(VERB_TYPES[4].present, Replace("ita/itä", "itse/itse"));
        assert_eq!(VERB_TYPES[5].present, Replace("eta/etä", "ene/ene"));
    }

    #[test]
    fn the_present_stem_is_cut_from_the_infinitive_by_the_type() {
        // Type 1 drops the final vowel: `sanoa` gives `sano`, `muistaa` keeps
        // its doubled vowel and gives `muista`, `laskea` gives `laske`.
        assert_eq!(present_stem("sanoa", 52, None), "sano");
        assert_eq!(present_stem("muistaa", 53, None), "muista");
        assert_eq!(present_stem("laskea", 58, None), "laske");
        // Type 2 drops the `-da/-dä`: `voida` gives `voi`, `saada` gives `saa`.
        assert_eq!(present_stem("voida", 62, None), "voi");
        assert_eq!(present_stem("saada", 63, None), "saa");
        // Type 3 adds the class `-e-`: `tulla` gives `tule`, `valita` is type 5
        // and turns `-ita` into `-itse` (`valitse`), and type 6 turns `-eta`
        // into `-ene` (`vanheta` gives `vanhene`).
        assert_eq!(present_stem("tulla", 67, None), "tule");
        assert_eq!(present_stem("valita", 69, None), "valitse");
        assert_eq!(present_stem("vanheta", 72, None), "vanhene");
        // Type 4 drops the `-ta/-tä` and puts the theme vowel back on: `salata`
        // gives `salaa`, `katketa` gives `katkea`, `selvitä` gives `selviä`.
        assert_eq!(present_stem("salata", 73, None), "salaa");
        assert_eq!(present_stem("katketa", 74, None), "katkea");
        assert_eq!(present_stem("selvitä", 75, None), "selviä");
        // The `-dä` irregulars keep their weak present: `nähdä` is `näe`.
        assert_eq!(present_stem("nähdä", 71, None), "näe");
        assert_eq!(present_stem("tehdä", 71, None), "tee");
    }

    #[test]
    fn the_present_stem_gradates_to_the_first_person_grade() {
        // `huutaa` is `huuta` strong and `huuda` weak, the `huudan` of
        // `huudan`/`huutaa`.
        assert_eq!(present_stem("huutaa", 54, Some(Gradation(6))), "huuda");
        // Without gradation the stem is unchanged.
        assert_eq!(present_stem("huutaa", 54, None), "huuta");
    }

    #[test]
    fn the_past_stem_is_projected_from_the_model_verb() {
        // The model verb's own past is projected onto the lemma's stem: `laskea`
        // gives `lask`, `huutaa` gives `huus`, `kaivaa` gives `kaivo`, and
        // `sanoa` keeps its `o` and gives `sano`.
        assert_eq!(past_stem("laskea", 58, None), "lask");
        assert_eq!(past_stem("tulla", 67, None), "tul");
        assert_eq!(past_stem("sanoa", 52, None), "sano");
        assert_eq!(past_stem("huutaa", 54, Some(Gradation(6))), "huus");
        assert_eq!(past_stem("kaivaa", 56, None), "kaivo");
        assert_eq!(past_stem("nähdä", 71, None), "näk");
    }

    #[test]
    fn a_non_model_verb_gets_its_own_past_from_the_projection() {
        // `hellätä` is class 73, whose model `salata` is `salasi`; the same
        // ending projected onto `hellätä`'s stem is `helläsi`.
        assert_eq!(past_stem("hellätä", 73, None), "helläs");
        // Treated as a class-53 verb, `paistaa` takes the class's `muisti`
        // ending and gives `paisti`; its real class (56) gives `paistoi`.
        assert_eq!(past_stem("paistaa", 53, None), "paist");
    }

    #[test]
    fn present_tense_all_persons() {
        for want in [
            "laske", "lasket", "laskee", "laskemme", "laskette", "laskevat",
        ] {
            assert!(vhas("lasken", "laski", want), "missing {want}");
        }
        // The plural person is on the ending: `-mme` is first person, `-tte`
        // second and `-vat` third, and none takes an extra one. `laskemmette`
        // and `laskete` are not words.
        assert!(!vhas("lasken", "laski", "laskemmette"));
        assert!(!vhas("lasken", "laski", "laskete"));
        assert!(!vhas("sanon", "sanoi", "sanoommette"));
        assert!(!vhas("sanon", "sanoi", "sanoete"));
    }

    #[test]
    fn the_third_person_present_plural_follows_harmony() {
        // The third person plural present is `-vat` or `-vät` by the verb's
        // harmony: `sano` -> `sanovat`, `mene` -> `menevät`.
        assert!(vhas("sanon", "sanoi", "sanovat"));
        assert!(vhas("menen", "meni", "menevät"));
    }

    #[test]
    fn present_of_an_o_stem_verb() {
        for want in ["sano", "sanot", "sanoo", "sanomme", "sanotte"] {
            assert!(vhas("sanon", "sanoi", want), "missing {want}");
        }
    }

    #[test]
    fn the_third_person_present_doubles_only_one_vowel() {
        // The third person singular present is the stem with its final vowel
        // doubled: `laske` -> `laskee`, `sano` -> `sanoo`. A stem that already
        // ends in two vowels is the complete form and is not touched, so
        // `hellää` stays `hellää` and `voi` stays `voi`. Only one vowel is ever
        // added; a second one would give the `hellääu` and `voii` that are not
        // words.
        assert!(vhas("lasken", "laski", "laskee"));
        assert!(vhas("sanon", "sanoi", "sanoo"));
        assert!(vhas("hellään", "helläsi", "hellää"));
        assert!(vhas("voin", "voi", "voi"));
        assert!(!vhas("lasken", "laski", "laskeu"));
        assert!(!vhas("lasken", "laski", "laskey"));
        assert!(!vhas("hellään", "helläsi", "hellääu"));
        assert!(!vhas("hellään", "helläsi", "hellääy"));
        assert!(!vhas("voin", "voi", "voii"));
    }

    fn vhas_grad(inf: &str, p1: &str, p3: &str, grad: u8, want: &str) -> bool {
        verb_with_passive(p1, p3, None, Some(Gradation(grad)), inf)
            .iter()
            .any(|f| f.word == want)
    }

    #[test]
    fn the_potential_keeps_the_strong_stem() {
        // `naukua` gradates (class 52*D, k -> nothing), so the first person is
        // `nau'un`. The potential still needs the strong stem, `naukunee`, and
        // not the weakened `nauunee`.
        assert!(vhas_grad("naukua", "naukun", "naukui", 4, "naukunee"));
        assert!(!vhas_grad("naukua", "naukun", "naukui", 4, "nauunee"));
        // `nähdä` drops its `d` for the potential: `nähnee`, not the
        // `näkenee` that the third person singular's `näke` would give.
        let näh = verb_with_passive("näen", "näki", None, None, "nähdä");
        assert!(näh.iter().any(|f| f.word == "nähnee"));
        assert!(!näh.iter().any(|f| f.word == "näkenee"));
    }

    #[test]
    fn the_potential_assimilates_the_consonant_stem() {
        // A type-3 verb forms the potential on its consonant stem, where the
        // `n` of `-nee` assimilates into the final consonant: `tulla` is
        // `tullee`, `juosta` is `juossee`, `mennä` is `mennee`.
        assert!(vhas_lemma("tulla", "tulen", "tuli", "tullee"));
        assert!(vhas_lemma("juosta", "juoksen", "juoksi", "juossee"));
        assert!(vhas_lemma("mennä", "menen", "meni", "mennee"));
        // A vowel stem keeps the plain `-nee`: `laskea` is `laskenee`.
        assert!(vhas_lemma("laskea", "lasken", "laski", "laskenee"));
    }

    #[test]
    fn the_jäädä_paradigm_matches_verbix() {
        // The complete jäädä paradigm, cross-checked against verbix. The finite
        // moods, their compound perfects, the passive moods, and the
        // infinitives all have to be present.
        for want in [
            // Present and imperfect.
            "jään",
            "jäät",
            "jää",
            "jäämme",
            "jäätte",
            "jäävät",
            "jäin",
            "jäit",
            "jäi",
            "jäimme",
            "jäitte",
            "jäivät",
            // Potential and conditional, all persons.
            "jäänen",
            "jäänet",
            "jäänee",
            "jäänemme",
            "jäänette",
            "jäänevät",
            "jäisin",
            "jäisit",
            "jäisi",
            "jäisimme",
            "jäisitte",
            "jäisivät",
            // Imperative.
            "jää",
            "jääköön",
            "jääkäämme",
            "jääkää",
            "jääkööt",
            // Compound perfects.
            "olen jäänyt",
            "olisi jäänyt",
            "lienen jäänyt",
            "olkoon jäänyt",
            "olkoot jääneet",
            // Passive present, imperfect, potential, conditional, imperative.
            "jäädään",
            "ei jäädä",
            "jäätiin",
            "ei jääty",
            "jäätäneen",
            "ei jäätäne",
            "jäätäisiin",
            "ei jäätäisi",
            "jäätäköön",
            "älköön jäätäkö",
            // Passive compound perfects.
            "on jääty",
            "oli jääty",
            "lienee jääty",
            "olisi jääty",
            "olkoon jääty",
            "älköön olko jääty",
            // Infinitives.
            "jäädessä",
            "jääden",
            "jäätäessä",
            "jäämässä",
            "jäämästä",
            "jäämään",
            "jäämällä",
            "jäämättä",
            "jäämän",
            "jäätämän",
            "jääminen",
            "jäämistä",
            "jäämäisillään",
        ] {
            assert!(
                vhas_inf("jäädä", 63, "jäätiin", want),
                "jäädä should produce {want}"
            );
        }
    }

    #[test]
    fn the_participle_case_declensions_match_verbix() {
        // The participle case forms, cross-checked against verbix: the present
        // active (`jäävä`), the passive present (`jäätävä`), the agent
        // (`jäämä`), the past active (`jäänyt`) and the past passive (`jääty`).
        for want in [
            // Present active `-va`.
            "jäävä",
            "jäävän",
            "jäävää",
            "jäävässä",
            "jäävästä",
            "jäävään",
            "jäävällä",
            "jäävältä",
            "jäävänä",
            "jääväksi",
            "jäävättä",
            "jäävät",
            "jääviä",
            "jäävien",
            "jäävissä",
            "jääviin",
            "jäävein",
            // Passive present `-va`.
            "jäätävä",
            "jäätävää",
            "jäätävän",
            "jäätävässä",
            "jäätävällä",
            "jäätävät",
            "jäätäviä",
            // Agent `-ma`.
            "jäämä",
            "jäämän",
            "jäämää",
            "jäämässä",
            "jäämästä",
            "jäämään",
            "jäämät",
            "jäämiä",
            "jäämien",
            "jäämissä",
            "jäämiin",
            "jäämein",
            // Past active `-nut`.
            "jäänyt",
            "jääneen",
            "jäänyttä",
            "jääneessä",
            "jääneeseen",
            "jääneet",
            "jääneitä",
            "jääneiden",
            "jääneisiin",
            "jäänein",
            // Past passive `-tu`.
            "jääty",
            "jäätyä",
            "jäätyyn",
            "jäätynä",
            "jäätyjä",
            "jäätyjen",
            "jäätyihin",
            "jäätyine",
        ] {
            assert!(
                vhas_inf("jäädä", 63, "jäätiin", want),
                "jäädä should produce participle {want}"
            );
        }
    }

    #[test]
    fn the_consonant_stem_passive_moods_keep_a_single_t() {
        // `tulla`'s passive stem `tult` already ends in `t`, so its mood forms
        // take only the theme vowel: `tultaneen`, not `tulttaneen`.
        assert!(vhas_inf("tulla", 67, "tultiin", "tultaneen"));
        assert!(vhas_inf("tulla", 67, "tultiin", "tultaisiin"));
        assert!(vhas_inf("tulla", 67, "tultiin", "tultakoon"));
        assert!(vhas_inf("tulla", 67, "tultiin", "ei tultane"));
        assert!(vhas_inf("tulla", 67, "tultiin", "älköön tultako"));
    }

    #[test]
    fn the_past_participles_use_their_own_stem_rules() {
        // The past participle is not the present stem with `-nut`; it uses the
        // consonant stem, so `tulla` is `tullut` and `tavata` is `tavannut`,
        // not the `tulenut`/`tavaanut` a mechanical `stem + nut` would give.
        assert!(vhas_lemma("tulla", "tulen", "tuli", "tullut"));
        assert!(vhas_lemma("tavata", "tapaan", "tapasi", "tavannut"));
        assert!(vhas_lemma("tarvita", "tarvitsen", "tarvitsi", "tarvinnut"));
        assert!(vhas_lemma("vanheta", "vanhenen", "vanheni", "vanhennut"));
    }

    #[test]
    fn the_gradating_third_person_uses_the_strong_stem() {
        // The first person has already gradated, so the third person present
        // needs the strong stem, which is the gradation put back the other
        // way: `tuntea` is `tunnen` but `tuntee`, and `pitää` is `pidän` but
        // `pitää`. J is `nt:nn` and F is `t:d`.
        assert!(vhas_grad("tuntea", "tunnen", "tunsi", 10, "tuntee"));
        assert!(vhas_grad("tuntea", "tunnen", "tunsi", 10, "tuntevat"));
        assert!(vhas_grad("pitää", "pidän", "piti", 6, "pitää"));
        assert!(vhas_grad("pitää", "pidän", "piti", 6, "pitävät"));
        assert!(!vhas_grad("tuntea", "tunnen", "tunsi", 10, "tunnee"));
        assert!(!vhas_grad("tuntea", "tunnen", "tunsi", 10, "tunnevat"));
        // The `-dä` type that loses its consonant entirely is tabled: `nähdä`
        // is `näen` but `näkee`.
        assert!(vhas_grad("nähdä", "näen", "näki", 0, "näkee"));
        assert!(vhas_grad("nähdä", "näen", "näki", 0, "näkevät"));
    }

    #[test]
    fn the_infinitives_are_generated() {
        // The second infinitive is `-essa` in the inessive and `-en` in the
        // instructive; the third is the `ma`/`mä` stem with a case on it; the
        // fourth is the nominal `-minen`. All follow the verb's harmony.
        assert!(vhas("puhun", "puhui", "puhuessa"));
        assert!(vhas("puhun", "puhui", "puhuen"));
        assert!(vhas("puhun", "puhui", "puhumaan"));
        assert!(vhas("puhun", "puhui", "puhumassa"));
        assert!(vhas("puhun", "puhui", "puhumatta"));
        assert!(vhas("puhun", "puhui", "puhuminen"));
        // A front verb takes the front forms.
        assert!(vhas("menen", "meni", "menemään"));
        assert!(vhas("menen", "meni", "menemässä"));
        assert!(vhas("menen", "meni", "meneminen"));
        // A gradating verb keeps its consonant in the second infinitive and
        // uses the strong stem in the third and fourth: `syödä` is `syödessä`
        // and `syömään`, and `nähdä` is `nähdessä` and `näkemään`.
        assert!(vhas_lemma("syödä", "syön", "söi", "syödessä"));
        assert!(vhas_lemma("syödä", "syön", "söi", "syöden"));
        assert!(vhas_lemma("syödä", "syön", "söi", "syömään"));
        assert!(vhas_lemma("nähdä", "näen", "näki", "nähdessä"));
        assert!(vhas_lemma("nähdä", "näen", "näki", "näkemään"));
        assert!(vhas_lemma("nähdä", "näen", "näki", "näkeminen"));
    }

    #[test]
    fn past_tense_forms() {
        for want in [
            "laskin", "laskit", "laskimme", "laskitte", "laskivat", "laski",
        ] {
            assert!(vhas("lasken", "laski", want), "missing {want}");
        }
    }

    #[test]
    fn a_gradating_verb_weakens_its_past_for_the_first_and_second_persons() {
        // `teroittaa` gradates `tt` to `t` in the past: the 3rd person keeps
        // the strong `teroitti`, but the 1st and 2nd persons are `teroitin`,
        // `teroitit`, `teroitimme`, `teroititte` on the weak `teroit`, not the
        // strong `teroittin` and `teroittitte`.
        let forms = verb_with_passive(
            "teroitan",
            "teroitti",
            None,
            Some(Gradation::from_letter('C').unwrap()),
            "teroittaa",
        );
        for want in [
            "teroitin",
            "teroitit",
            "teroitimme",
            "teroititte",
            "teroitti",
        ] {
            assert!(
                forms.iter().any(|f| f.word == want),
                "teroittaa is missing the past {want}"
            );
        }
        for wrong in ["teroittin", "teroittit", "teroittitte"] {
            assert!(
                !forms.iter().any(|f| f.word == wrong),
                "teroittaa offers the strong past {wrong}"
            );
        }
    }

    #[test]
    fn the_conditional_keeps_a_type_one_theme_vowel() {
        // `teroittaa` is a type-1 verb whose past stem `teroitt` drops the theme
        // `a`; the conditional restores it: `teroittaisi` on `teroitta`, not the
        // `teroittisi` the past stem alone would give. `laskea` drops its theme
        // `e` (`laskisi`) and `saada` its doubled `a` (`saisin`).
        let forms = verb_with_passive(
            "teroitan",
            "teroitti",
            None,
            Some(Gradation::from_letter('C').unwrap()),
            "teroittaa",
        );
        for want in ["teroittaisin", "teroittaisi", "teroittaisimme"] {
            assert!(
                forms.iter().any(|f| f.word == want),
                "teroittaa is missing the conditional {want}"
            );
        }
        assert!(
            !forms.iter().any(|f| f.word == "teroittisin"),
            "teroittaa offers the past-stem conditional teroittisin"
        );
        let l = verb("lasken", "laski");
        assert!(l.iter().any(|f| f.word == "laskisi"), "laskea conditional");
        let s = verb("saan", "sai");
        assert!(s.iter().any(|f| f.word == "saisin"), "saada conditional");
    }

    #[test]
    fn the_past_participle_uses_the_strong_stem() {
        // `teroittaa` keeps the strong grade in its past participle:
        // `teroittanut`, not the weak `teroitanut`.
        let forms = verb_with_passive(
            "teroitan",
            "teroitti",
            None,
            Some(Gradation::from_letter('C').unwrap()),
            "teroittaa",
        );
        assert!(
            forms.iter().any(|f| f.word == "teroittanut"),
            "teroittaa is missing teroittanut"
        );
        assert!(
            !forms.iter().any(|f| f.word == "teroitanut"),
            "teroittaa offers the weak teroitanut"
        );
    }

    #[test]
    fn the_passive_stem_weakens_a_gradating_verb() {
        // `teroittaa` gradates `tt` to `t`, so its passive is `teroitetaan` and
        // `teroitettiin` on the weak `teroitet`, not the strong `teroittettaan`
        // the model's `muistettiin` projects.
        let forms = verb_from_infinitive(
            "teroittaa",
            53,
            Some(Gradation::from_letter('C').unwrap()),
            passive_of("teroittaa", "teroittettiin"),
        );
        for want in ["teroitetaan", "teroitettiin", "teroitettava"] {
            assert!(
                forms.iter().any(|f| f.word == want),
                "teroittaa is missing the passive {want}"
            );
        }
        for wrong in ["teroittetaan", "teroittettiin", "teroittettava"] {
            assert!(
                !forms.iter().any(|f| f.word == wrong),
                "teroittaa offers the strong passive {wrong}"
            );
        }
    }

    #[test]
    fn the_negative_forms_are_generated() {
        // Finnish negation is analytic: the negative auxiliary precedes a
        // connegative. The present connegative is the bare stem (`en puhu`)
        // and the past is the past participle (`en puhunut`, `emme puhuneet`).
        assert!(vhas("puhun", "puhui", "en puhu"));
        assert!(vhas("puhun", "puhui", "ei puhu"));
        assert!(vhas("puhun", "puhui", "emme puhu"));
        assert!(vhas("puhun", "puhui", "en puhunut"));
        assert!(vhas("puhun", "puhui", "emme puhuneet"));
    }

    #[test]
    fn the_perfect_and_pluperfect_are_compositional() {
        // The perfect is `olla` in the present plus the past participle; the
        // pluperfect is `olla` in the past plus it. `olla` is irregular.
        assert!(vhas("puhun", "puhui", "olen puhunut"));
        assert!(vhas("puhun", "puhui", "on puhunut"));
        assert!(vhas("puhun", "puhui", "ovat puhuneet"));
        assert!(vhas("puhun", "puhui", "olin puhunut"));
        assert!(vhas("puhun", "puhui", "oli puhunut"));
        assert!(vhas("puhun", "puhui", "olivat puhuneet"));
    }

    /// Whether a form of a verb is generated, given the catalogue's passive
    /// past illative and the lemma, as `expand` has them.
    fn vhas_passive(inf: &str, p1: &str, p3: &str, p7: &str, want: &str) -> bool {
        verb_with_passive(p1, p3, passive_of(inf, p7), None, inf)
            .iter()
            .any(|f| f.word == want)
    }

    #[test]
    fn passive_forms() {
        assert!(vhas("lasken", "laski", "lasketaan"), "lasketaan");
        assert!(vhas("sanon", "sanoi", "sanotaan"), "sanotaan");
    }

    #[test]
    fn a_verb_harmonises_without_its_theme_vowel() {
        // `laske-` is a back verb and `käydä-` a front one, so the passive
        // follows the verb and not the stem: `laske` ends in the front `e` of
        // its own theme, and reading that as the harmony would give the
        // `lasketään` of a front verb rather than the `lasketaan` of a back one.
        assert!(vhas("lasken", "laski", "lasketaan"));
        assert!(vhas_passive(
            "käydä",
            "käyn",
            "kävi",
            "käytiin",
            "käytetään"
        ));
    }

    #[test]
    fn the_passive_of_kayda_has_no_d() {
        // The passive follows the third person, and the third person of `käydä`
        // is `käy`, not `käyd`. So the passive stem is `käyt` and the form is
        // `käytetään`; a `d` here would give the `käydetään` that is not a
        // Finnish word at all.
        assert!(vhas_passive(
            "käydä",
            "käyn",
            "kävi",
            "käytiin",
            "käytetään"
        ));
        assert!(!vhas_passive(
            "käydä",
            "käyn",
            "kävi",
            "käytiin",
            "käydetään"
        ));
        // The short stem takes its own `t` and then its theme vowel, and the
        // two `t`s are both in the word.
        assert!(vhas_passive(
            "käydä",
            "käyn",
            "kävi",
            "käytiin",
            "käytetään"
        ));
        assert!(vhas_passive("käydä", "käyn", "kävi", "käytiin", "käytiin"));
    }

    #[test]
    fn the_ida_types_take_a_d_where_the_others_take_a_t() {
        // `tupakoida` is a `-ida` type: the `d` of `tupakoidaan` stands where
        // the `t` of `lasketaan` is. Its third person present is the bare
        // `tupakoi` (not the `tupakoitsee` a mechanical `-tse` would give).
        assert!(vhas_passive(
            "tupakoida",
            "tupakoin",
            "tupakoi",
            "tupakoitiin",
            "tupakoidaan"
        ));
        assert!(vhas("tupakoin", "tupakoi", "tupakoi"));
        assert!(!vhas("tupakoin", "tupakoi", "tupakoitsee"));
        // `valita` is an `-ita` type and keeps its `t`, so it is not affected.
        assert!(vhas_passive(
            "valita",
            "valitsen",
            "valitsi",
            "valittiin",
            "valitaan"
        ));
    }

    #[test]
    fn the_passive_stem_comes_from_the_catalogue_not_the_first_person() {
        // Gradation is already done in the first person, so it cannot show the
        // passive stem: `nähdä` is `näen` in the first person and `näht` in the
        // passive, and `tulla` is `tulen` and `tult`. Without the seventh slot
        // the best guess is the first person with its theme vowel back, which
        // is right for a verb that does not gradate.
        assert_eq!(passive_stem("käytiin"), Some("käy"));
        assert_eq!(passive_stem("laskettiin"), Some("laske"));
        assert_eq!(passive_stem("nähtiin"), Some("näht"));
        assert_eq!(passive_stem("tultiin"), Some("tult"));
        assert_eq!(passive_stem("iin"), None);
        // A stem already ending in `t` cannot take the passive `t` again.
        assert!(vhas_passive("nähdä", "näen", "näki", "nähtiin", "nähdään"));
        // The type-3 passive present doubles the consonant: `tulla` is
        // `tullaan` (verbix), not the `tultaan` the past stem `tult` would
        // give.
        assert!(vhas_passive("tulla", "tulen", "tuli", "tultiin", "tullaan"));
    }
}
