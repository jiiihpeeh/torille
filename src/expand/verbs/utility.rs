//! Shared machinery for the hand-written verb classes.
//!
//! A verb class module works out the handful of stems one Kotus type gives a
//! verb and hands them to [`paradigm`], which applies the endings every Finnish
//! verb shares: the finite person/number/mood endings, the five infinitives,
//! the participles and their nominal declension, and the possessives on
//! everything nominal. So the per-class module is the *stem rules*, and this
//! file is the *paradigm shape*, shared by all of them.
//!
//! The declension here is the twelve ordinary cases plus the instructive and
//! the comitative. The comitative is plural-only and number-invariant, so it is
//! not a cell of the `(case, number)` grid the nominal engine walks; the verb
//! reference did not list it until `tools/oracle/build_verb_lexicon.py` added it from
//! Omorfi, and this engine builds it on the plural oblique stem (the `-ttu`
//! participle's own [`Decl::comitative_stem`] excepted, since its plural is
//! mixed). Neither oracle tags an accusative, which is syncretic with the
//! nominative and genitive: the engine adds it as a second *reading* of those
//! two words, and the agreed reading of the word then drops the case, as it
//! does whenever readings disagree.
//!
//! It mirrors `expand::nominals::utility`: the nominal `Builder` accumulates
//! case cells and their possessives for a noun; [`VerbBuilder`] does the same
//! for a verb, where the "cells" are a mix of finite forms, non-finite forms
//! and the nominal-like declensions of the participles.
//!
//! The declension of a participle is the same noun declension the nominal
//! engine uses -- the genitive stem for the singular oblique, the plural stem
//! for the plural oblique, the possessive suffixes on the case form -- so
//! [`VerbBuilder::decline`] is given the anchors a participle shares with a
//! noun.
//!
//! The possessive rules are not the noun rules, and they are spelled out here
//! from what the verb lexicon confirms rather than assumed:
//!
//! * the genitive and the illative drop their final `n` before a suffix
//!   (`puhumani`, `puhuvaani`), and the nominative takes its suffix on the
//!   genitive stem for `-nut` and `-minen` (`puhuneeni`, `puhumiseni`);
//! * the third person has a long `-nsa`/`-nsä` everywhere except the
//!   translative, and a short doubling `-Vn` only in the local cases and the
//!   plural partitive (`puhuvassaan`, `puhuviaan`) -- the illative already ends
//!   in a long vowel and takes no short form (`*puhuvaaan`);
//! * the translative writes its elided `i` out twice over, taking suffixes on
//!   both the singular and the plural oblique stem: `puhuvaksi` -> `puhuvakseni`
//!   and `puhuvikseni`, `puhuvakseen`, `puhuvaksensa`;
//! * every participle and the `-minen` infinitive take a possessive on the
//!   plural genitive (`puhumieni`, `puhuvieni`), so `possess_gen_pl` is set on
//!   all of them; the reference lists some of those only under `omorfi`, but
//!   none of them is excluded;
//! * the `-ttu` participle takes its essive and illative on the nominative,
//!   not the oblique (`puhuttuna`, `puhuttuun`, against `*puhutuna`), and it
//!   possesses on every case (`puhuttuni`, `puhutussani`, `puhutukseni`), not
//!   the partitive alone -- only the partitive is a `both` row, the rest are
//!   `omorfi`-covered, but the reference has them all.

#![allow(dead_code)]

use crate::harmony::pick_harmony;
use crate::kotus::{Gradation, Grade};
use crate::tags::{
    Case, Confidence, Form, Mood, Number, PartOfSpeech, Person, Polarity, Possessive, Tags, Tense,
    Voice,
};

use super::compound;

/// The personal possessive suffixes, and the owner each spells. The third
/// person is handled apart, since it has two spellings.
pub const PERSONAL: &[(&str, Possessive)] = &[
    ("ni", Possessive::FirstSingular),
    ("si", Possessive::SecondSingular),
    ("mme", Possessive::FirstPlural),
    ("nne", Possessive::SecondPlural),
];

/// The personal possessive suffixes before a suffix that already closes the
/// stem. A participle's own `a` does not block the vowel, so `puhuva` + `ni`
/// is `puhuvani`; but the translative's `ksi` does, and Finnish repairs the
/// clash with an `e`: `puhuvakseni`, not `puhuvaksni`.
pub const PERSONAL_E: &[(&str, Possessive)] = &[
    ("eni", Possessive::FirstSingular),
    ("esi", Possessive::SecondSingular),
    ("emme", Possessive::FirstPlural),
    ("enne", Possessive::SecondPlural),
];

/// The cases a participle or the `-minen` infinitive declines through, in
/// catalogue order. The instructive is plural-only and added by `decline`.
pub const DECLENSION: [Case; 12] = [
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
];

/// Whether a word is a front-harmony word, by its last non-neutral vowel.
///
/// Re-exported from [`crate::harmony::front`]; the canonical home is at the
/// crate root alongside [`crate::gradation`] because vowel harmony is a
/// Finnish primitive used by both the hand-written engine and the heuristic.
pub use crate::harmony::front;

/// The base of the five end-gradating `-ta`/`-tä` types: the lemma less its own
/// `ta`/`tä`.
///
/// Classes 72 to 75 take the stem from it and put the ending's own vowel back on
/// (`vanhe` for `vanheta`, `sala` for `salata`, `selvi` for `selvitä`), and class
/// 76 takes it and puts the whole `ta`/`tä` back on (`tie` for `tietää`,
/// `tai` for `taitaa`) because there the theme is the consonant as well as the
/// vowel. The base is the same word in all five, so it is written once here.
pub fn ta_base(lemma: &str) -> &str {
    lemma
        .strip_suffix("ta")
        .or_else(|| lemma.strip_suffix("tä"))
        .unwrap_or(lemma)
}

/// The strong grade of a base the sanalista writes in the weak one: `pakka` for
/// `paka`, `pake` for `pae`, `huoka` for `huoa`, `jakel` for `jael`.
///
/// Classes 72, 73 and 74 all have a lemma-less stem as their base, all three
/// write that stem in the weak grade in the sanalista (`pakata`, `paeta`,
/// `huoata`, `jaella`), and all three read the present off the strong one, so
/// the operation is the same one three times over. Every code but `D` is a
/// consonant pair the base still carries in its weak form, and
/// [`Gradation::strongen`] finds it and reads it back on its own. `D` — `k`
/// against nothing, as in `reikä` beside `reiän` — is the exception, and the
/// commonest code in all three classes: the base has no `k` left to find, so it
/// has to be written in front of the base's final vowel by hand.
pub fn strong_base_of(base: &str, gradation: Option<Gradation>) -> String {
    match gradation {
        Some(Gradation(4)) => {
            // The site is the last vowel, not the last character: `pae` and
            // `jael` both gradate in front of their final vowel, and only the
            // first of the two ends in one.
            match base.rfind([
                'a', 'ä', 'e', 'i', 'o', 'u', 'ö', 'y', 'A', 'O', 'U', 'Y', 'Ä', 'Ö', 'I', 'E',
            ]) {
                Some(at) => {
                    let mut out = String::with_capacity(base.len() + 1);
                    out.push_str(&base[..at]);
                    out.push('k');
                    out.push_str(&base[at..]);
                    out
                }
                // A base with no vowel at all cannot gradate, and there is
                // nothing to write the `k` in front of.
                None => base.to_string(),
            }
        }
        Some(g) => g.strongen(base),
        None => base.to_string(),
    }
}

/// Which cells of a declension take a possessive suffix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Possess {
    /// No case takes one. No declension currently asks for this -- the negative
    /// participle possesses as freely as the rest in the reference
    /// (`puhumattomani`, `puhumattomiaan`) -- but the reading is spelled out
    /// rather than left to the call site to imply.
    None,
    /// Every case the declension normally allows, which is what every
    /// participle and the `-minen` infinitive take. The `-ttu` participle is
    /// the one worth naming, because the reference licenses its possessives on
    /// every case (`puhuttuni`, `puhutussani`, `puhutukseni`) and not on the
    /// partitive alone, so it is `All` too.
    All,
}

/// How a declension's singular partitive offers the third person: not at all,
/// the long `-nsa`/`-nsä` alone, or both spellings (the long one and the short
/// `-Vn` that doubles the base's last vowel).
///
/// The short spelling is there for the `-nut` and `-ttu` participles and for
/// the `-minen` infinitive (`puhunuttaan`, `puhuttuaan`, `puhumistaan`) but
/// not for the `-va` ones, whose singular partitive already ends in a long
/// vowel (`puhuvaansa`, not `*puhuvaaan`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThirdPossessive {
    None,
    Long,
    LongAndShort,
}

/// How a type writes the consonant its passive adds to the theme.
///
/// A type-1 theme ends in a vowel, so the cluster has to be spelled out and is
/// geminated before a further vowel: `sanot` beside `sanott`, `sallit` beside
/// `sallitt`. Class 62's `-ida` passive has an alternation of its own instead,
/// the `t` deranked to `d` where a vowel follows, which is what puts `voidaan`
/// beside `voitiin`, `voitu` and `voitava` where every other type has `t` and
/// `tt`. Class 66's `-ista` passive keeps its own `t` in the present as every
/// type does and drops the gemination, so it writes one `t` throughout. Class
/// 67's types go further and take no consonant of their own in the present at
/// all: their theme already ends in a consonant, and the present gives that one
/// back (`tullaan` beside `tultiin`, `pannaan` beside `pantiin`). Class 69 needs
/// no giving back: its theme's own `t` *is* the passive's consonant, so the
/// present takes nothing at all (`valitaan` beside `valittiin`).
/// See [`Stems::passive_present`] and [`tu_participle`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PassiveCons {
    /// The usual `t` in the present and `tt` in the past and the moods:
    /// `sanotaan` beside `sanottiin`. The four end-gradating `-ta/-tä` types
    /// write it too, and their one `t` in the present is worth naming because it
    /// looks like a gemination the type has forgotten: `salataan` beside
    /// `salattiin`, `vanhetaan` beside `vanhettiin`, `katketaan` beside
    /// `katkettiin`, `selvitään` beside `selvittiin`. In every type in the
    /// language the present and the past disagree on the number of `t`s; what
    /// differs between types is the *present's* consonant, not the past's.
    Geminated,
    /// `d` in the present, `t` in the past and the moods.
    Deranked,
    /// `t` in the present, the past and the moods, and the `-tu` participle
    /// which is not deranked: `rohkaistaan` beside `rohkaistiin` and
    /// `rohkaistu` with its oblique `rohkaistun`.
    SingleT,
    /// The theme's own final consonant in the present, `t` in the past and the
    /// moods: `tullaan` and `pannaan` beside `tultiin` and `pantiin`, where
    /// every other type adds a `t` the theme does not already carry. The `-tu`
    /// participle is not deranked here either.
    ThemeEnd,
    /// Nothing at all in the present, `t` in the past and the moods: `valitaan`
    /// beside `valittiin` and `valittava`. The theme already ends in the `t`
    /// the passive wants -- it is the type's own, as in [`Self::ThemeEnd`] --
    /// so the present's `-aan` goes straight on it, while everything else
    /// gemmates as usual.
    ThemeOwn,
}

impl PassiveCons {
    /// The consonant the passive adds to the theme in the present, and the
    /// consonant it adds in the past and the moods.
    ///
    /// A type-1 theme ends in a vowel, so a single consonant in front of the
    /// present's `-aan` would be read as part of it, and the cluster is written
    /// out in full: `sanot` beside `sanott`. Class 62's `-ida` passive has the
    /// `t`/`d` alternation of its own, `voidaan` beside `voitiin`, so it writes
    /// one consonant each way and takes the `d` before the `-aan`. Class 66's
    /// writes a single `t` each way too, and a theme that already ends in a
    /// consonant has none of its own to add: the ending gives the theme's own
    /// back. That is why `theme` is a parameter -- the letter is read off it
    /// (`tul` + `l` is `tullaan`, `pan` + `n` is `pannaan`).
    pub fn present(&self, theme: &str) -> String {
        match self {
            Self::Geminated | Self::SingleT => "t".to_string(),
            Self::Deranked => "d".to_string(),
            Self::ThemeEnd => theme
                .chars()
                .next_back()
                .expect("a ThemeEnd passive theme is consonant-final")
                .to_string(),
            Self::ThemeOwn => String::new(),
        }
    }

    /// The consonant the passive adds to the theme in the past and the moods,
    /// which [`PassiveCons::present`] explains. A theme that ends in a
    /// consonant takes the ordinary `t` there: only the present differs.
    pub fn past(&self) -> &'static str {
        match self {
            Self::Geminated => "tt",
            Self::Deranked | Self::SingleT | Self::ThemeEnd | Self::ThemeOwn => "t",
        }
    }

    /// Whether the past passive participle deranks its `t` to a `d` before the
    /// participle's own vowel, which is [`PassiveCons::Deranked`]'s doings in
    /// the oblique too: `voitu` in the nominative, `voidun` and `voidussa` in
    /// the oblique. A geminated type instead drops its own doubled `t` to one
    /// there: `sallittu` beside `sallitun`.
    pub fn deranks_tu(&self) -> bool {
        matches!(self, Self::Deranked)
    }
}

/// A stem a class's own ending-shape hangs off: the stem, and the consonant
/// that its `-e` and `-u` endings open with.
///
/// Type 1 has one shape and class 66 another, and the potential and the past
/// active participle are the two cells where the shape shows: `sano` + `n`
/// gives `sanone` and `sanonut`, `rohkais` + `s` gives `rohkaisse` and
/// `rohkaissut`. The two halves are one fact -- the class's ending brings the
/// consonant with it, and the same consonant fronts both cells -- so they are
/// one value and are set together.
#[derive(Clone, Debug)]
pub struct Alt {
    /// The stem both cells attach to.
    pub stem: String,
    /// What the cell's own ending opens with, before its vowels: `n` in
    /// `sanonut` and `sanone`, `s` in `rohkaissut` and `rohkaisse`.
    pub lead: &'static str,
}

impl Alt {
    /// Type 1's shape: the given stem, with the `n` that fronts the potential's
    /// `-e` and the participle's `-u`.
    pub fn n(stem: &str) -> Self {
        Alt {
            stem: stem.to_string(),
            lead: "n",
        }
    }
}

/// The anchors a declined participle needs: the eight forms the nominal
/// declension is built from, exactly as a noun class is, plus the two oblique
/// stems.
#[derive(Clone, Debug)]
pub struct Decl {
    pub nom_sg: String,
    pub gen_sg: String,
    pub part_sg: String,
    pub ill_sg: String,
    pub nom_pl: String,
    /// The genitive plural, one or more spellings (`puhuneiden`,
    /// `puhuneitten`).
    pub gen_pl: Vec<String>,
    pub part_pl: String,
    /// The illative plural, one or more spellings (`puhuneisiin`,
    /// `puhuneihin`).
    pub ill_pl: Vec<String>,
    /// The singular oblique stem, the genitive without its `n` (`puhunee`).
    pub sg_stem: String,
    /// The plural oblique stem (`puhunei`).
    pub pl_stem: String,
    /// The base the comitative attaches its `ne` to, when it is not
    /// [`Decl::pl_stem`]. The `-ttu` participle is the one: its plural is
    /// mixed -- `puhutuissa` keeps the weak `t` the plural oblique stem has,
    /// but `puhuttujen` and the comitative keep the strong `tt`, so its
    /// comitative is `puhuttu` + `i` + `ne` (`puhuttuine`), not `puhutui` +
    /// `ne`.
    pub comitative_stem: Option<String>,
    /// The essive, when it does not follow the oblique stem. The `-ttu`
    /// participle takes it on the nominative (`puhuttuna`), not the oblique
    /// (`*puhutuna`).
    pub ess_sg: Option<String>,
    pub ess_pl: Option<String>,
    /// The base the *nominative* possessive attaches to, when it is not the
    /// nominative itself: the `-minen` infinitive takes its possessive on the
    /// genitive stem (`puhumiseni`, not `*puhuminenmme`).
    pub nom_base: Option<String>,
    /// The base the *genitive* possessive attaches to, when the genitive's own
    /// stem would not do. The `-ttu` participle reduces its doubled `t` in the
    /// singular genitive (`puhutun`) but keeps it for the possessive
    /// (`puhuttuni`), which is how the reference spells the cell.
    pub gen_sg_poss: Option<String>,
    /// Whether the plural genitive takes possessive suffixes. Every participle
    /// and the `-minen` infinitive do: the present active on `puhuvien` gives
    /// `puhuviemme`, the rest likewise.
    pub possess_gen_pl: bool,
    /// Which cells take possessive suffixes at all.
    pub possess: Possess,
    /// How the singular partitive offers the third person.
    pub part_sg_third: ThirdPossessive,
}

/// Accumulates the forms of one verb.
pub struct VerbBuilder {
    out: Vec<Form>,
    front: bool,
}

impl VerbBuilder {
    pub fn new(lemma: &str) -> Self {
        Self {
            out: Vec::new(),
            front: front(lemma),
        }
    }

    pub fn front(&self) -> bool {
        self.front
    }

    /// The harmonic twin: `back` in the back half, `front_twin` in the front.
    pub fn h(&self, back: &str, front_twin: &str) -> String {
        String::from(if self.front { front_twin } else { back })
    }

    /// Push one form with the given tags, as a plain suggestion.
    pub fn push(&mut self, word: String, tags: Tags) {
        if word.chars().count() >= 3 {
            self.out.push(Form::new(word, tags).with(Confidence::Plain));
        }
    }

    /// Push one form at repaired confidence -- generated, but not offered as a
    /// correction the reference does not confirm.
    pub fn push_rare(&mut self, word: String, tags: Tags) {
        if word.chars().count() >= 3 {
            self.out
                .push(Form::new(word, tags).with(Confidence::Repaired));
        }
    }

    /// Push every case, number and possessive of a declined participle.
    /// `tags` supplies the reading for a (case, number, possessive) cell.
    pub fn decline(&mut self, d: &Decl, tags: impl Fn(Case, Number, Option<Possessive>) -> Tags) {
        let front = self.front;
        for case in DECLENSION {
            // --- singular ---
            let (bare, base, third) = singular_case(case, d, front);
            self.push(bare.clone(), tags(case, Number::Singular, None));
            if allows(d, case) {
                if case == Case::Translative {
                    // The translative has no `-nsa` third person: its two stems
                    // each spell the third person out (`puhuvakseen`).
                    for stem in [format!("{}ks", d.sg_stem), format!("{}ks", d.pl_stem)] {
                        self.possess_translative(stem, |p| tags(case, Number::Singular, Some(p)));
                    }
                } else {
                    self.possess(base, third, |p| tags(case, Number::Singular, Some(p)));
                }
            }
            // --- plural ---
            let (bare_pl, _base_pl, third_pl) = plural_case(case, d, front);
            self.push(bare_pl.clone(), tags(case, Number::Plural, None));
            // The plural genitive and illative can each have a second spelling
            // (`puhuneisiin` beside `puhuneihin`, `puhuneitten` beside
            // `puhuneiden`); which participles have one, and how common it is,
            // depends on the class and the lexeme, and each spelling takes the
            // whole possessive set on its own.
            let variants: &[String] = match case {
                Case::Genitive => &d.gen_pl,
                Case::Illative => &d.ill_pl,
                _ => &[],
            };
            // The plural nominative takes no possessive (`*puhuvatni`); neither
            // does the plural genitive unless the participle is one of the
            // kinds that allow it, nor the translative (handled above). The
            // lexicon does list possessive rows against the plural nominative,
            // but they are the singular forms again, so there is nothing extra
            // to build there.
            let allow = allows(d, case) && !matches!(case, Case::Nominative | Case::Translative);
            for variant in variants.iter().skip(1) {
                self.push(variant.clone(), tags(case, Number::Plural, None));
            }
            if allow {
                let forms = std::iter::once(&bare_pl).chain(variants.iter().skip(1));
                for form in forms {
                    let base = form.strip_suffix('n').unwrap_or(form);
                    self.possess(base.to_string(), third_pl, |p| {
                        tags(case, Number::Plural, Some(p))
                    });
                }
            }
        }
        // The instructive is plural-only: `puhuvin`, `puhunein`.
        self.push(
            format!("{}n", d.pl_stem),
            tags(Case::Instructive, Number::Plural, None),
        );
        // The comitative is plural-only, and the reference confirms it for
        // every participle and the `-minen` infinitive, so it is plain. Its
        // base is the plural oblique stem with `ne` (`puhuvi` + `ne` is
        // `puhuvine`) -- the `-ttu` participle names its own base, since its
        // plural is mixed (`puhuttuine` beside `puhutuissa`) -- and the
        // possessive follows that `e` the way a noun's does (`puhuvineni`,
        // `puhuvineen`).
        let com = format!(
            "{}ne",
            d.comitative_stem
                .clone()
                .unwrap_or_else(|| d.pl_stem.clone())
        );
        // The bare comitative (`puhuvine`) and the `-neen` spelling the
        // nominal engine uses as its basic form (`puhuvineen`) are both real,
        // and both read as no possessive.
        self.push(com.clone(), tags(Case::Comitative, Number::Plural, None));
        self.push(
            format!("{com}en"),
            tags(Case::Comitative, Number::Plural, None),
        );
        for (suf, p) in PERSONAL {
            self.push(
                format!("{com}{suf}"),
                tags(Case::Comitative, Number::Plural, Some(*p)),
            );
        }
        let long = if self.front { "nsä" } else { "nsa" };
        self.push(
            format!("{com}{long}"),
            tags(Case::Comitative, Number::Plural, Some(Possessive::Third)),
        );
        // The accusative is syncretic -- its singular is the genitive, its
        // plural the nominative -- and neither reference tags one, so the two
        // readings are added at repaired confidence as well.
        self.push_rare(
            d.gen_sg.clone(),
            tags(Case::Accusative, Number::Singular, None),
        );
        self.push_rare(
            d.nom_pl.clone(),
            tags(Case::Accusative, Number::Plural, None),
        );
    }

    /// Push the possessive spellings of `base`. `third` says how the third
    /// person is offered: not at all, the long `-nsa`/`-nsä` only, or both
    /// spellings (the long one and the short `-Vn` that doubles the base's last
    /// vowel).
    pub fn possess(
        &mut self,
        base: String,
        third: ThirdPossessive,
        mut make: impl FnMut(Possessive) -> Tags,
    ) {
        for (suf, p) in PERSONAL {
            self.push(format!("{base}{suf}"), make(*p));
        }
        if third != ThirdPossessive::None {
            let long = if self.front { "nsä" } else { "nsa" };
            self.push(format!("{base}{long}"), make(Possessive::Third));
        }
        if third == ThirdPossessive::LongAndShort
            && let Some(last) = base.chars().last()
            && "aeiouyäö".contains(last)
        {
            self.push(format!("{base}{last}n"), make(Possessive::Third));
        }
    }

    /// Push the possessives of a translative. The stem ends in `ks`, and the
    /// suffixes infix an `e` before the vowel that would otherwise close the
    /// stem: `puhuva` + `ks` + `eni` = `puhuvakseni`. The third person has the
    /// short `kseen` and the long `ksensa`.
    fn possess_translative(&mut self, ks: String, mut make: impl FnMut(Possessive) -> Tags) {
        for (suf, p) in PERSONAL_E {
            self.push(format!("{ks}{suf}"), make(*p));
        }
        self.push(format!("{ks}een"), make(Possessive::Third));
        let long = if self.front { "sä" } else { "sa" };
        self.push(format!("{ks}en{long}"), make(Possessive::Third));
    }

    pub fn into_forms(self) -> Vec<Form> {
        self.out
    }
}

/// Whether `case` of this declension takes a possessive suffix. The two cases
/// the plural pass skips -- the nominative and the translative -- are handled
/// there, not here.
fn allows(d: &Decl, case: Case) -> bool {
    let kind_allows = match d.possess {
        Possess::None => false,
        Possess::All => true,
    };
    kind_allows && (case != Case::Genitive || d.possess_gen_pl)
}

/// The singular spelling, the base a possessive attaches to (the genitive
/// drops its `n`; the illative drops it too), and the third person's offer.
fn singular_case(case: Case, d: &Decl, front: bool) -> (String, String, ThirdPossessive) {
    let bare = match case {
        Case::Nominative => d.nom_sg.clone(),
        Case::Genitive => d.gen_sg.clone(),
        Case::Partitive => d.part_sg.clone(),
        Case::Illative => d.ill_sg.clone(),
        Case::Essive => d
            .ess_sg
            .clone()
            .unwrap_or_else(|| format!("{}{}", d.sg_stem, pick_harmony(front, "na", "nä"))),
        Case::Translative => format!("{}ksi", d.sg_stem),
        Case::Inessive => format!("{}{}", d.sg_stem, pick_harmony(front, "ssa", "ssä")),
        Case::Elative => format!("{}{}", d.sg_stem, pick_harmony(front, "sta", "stä")),
        Case::Adessive => format!("{}{}", d.sg_stem, pick_harmony(front, "lla", "llä")),
        Case::Ablative => format!("{}{}", d.sg_stem, pick_harmony(front, "lta", "ltä")),
        Case::Allative => format!("{}lle", d.sg_stem),
        Case::Abessive => format!("{}{}", d.sg_stem, pick_harmony(front, "tta", "ttä")),
        Case::Instructive | Case::Comitative | Case::Prolative | Case::Accusative => String::new(),
    };
    let (base, third) = match case {
        // `puhuvani`, but `-nut` and `-minen` take theirs on the genitive stem
        // (`puhuneeni`, `puhumiseni`).
        Case::Nominative => (
            d.nom_base.clone().unwrap_or_else(|| bare.clone()),
            ThirdPossessive::Long,
        ),
        // `puhuman` -> `puhumani`/`puhuvansa`: the `n` drops before the suffix.
        Case::Genitive => (
            d.gen_sg_poss.clone().unwrap_or_else(|| d.sg_stem.clone()),
            ThirdPossessive::Long,
        ),
        // The singular partitive takes the short third person only where the lexicon
        // has it (`puhunuttaan`, `puhuttuaan`, `puhumistaan`); the `-va`
        // participles take the long one alone (`puhuvaansa`).
        Case::Partitive => (bare.clone(), d.part_sg_third),
        // The illative drops its `n` and already ends in a long vowel, so it
        // takes only the long third person (`puhuvaani`, `puhuvaansa`).
        Case::Illative => (
            bare.trim_end_matches('n').to_string(),
            ThirdPossessive::Long,
        ),
        // The translative is handled apart.
        Case::Translative => (bare.clone(), ThirdPossessive::None),
        // The local cases take both (`puhuvassani`, `puhuvassaan`,
        // `puhuvassansa`).
        _ => (bare.clone(), ThirdPossessive::LongAndShort),
    };
    (bare, base, third)
}

/// The plural spelling, possessive base and third-person offer, as
/// [`singular_case`].
fn plural_case(case: Case, d: &Decl, front: bool) -> (String, String, ThirdPossessive) {
    let bare = match case {
        Case::Nominative => d.nom_pl.clone(),
        Case::Genitive => d.gen_pl[0].clone(),
        Case::Partitive => d.part_pl.clone(),
        Case::Illative => d.ill_pl[0].clone(),
        Case::Essive => d
            .ess_pl
            .clone()
            .unwrap_or_else(|| format!("{}{}", d.pl_stem, pick_harmony(front, "na", "nä"))),
        Case::Translative => format!("{}ksi", d.pl_stem),
        Case::Inessive => format!("{}{}", d.pl_stem, pick_harmony(front, "ssa", "ssä")),
        Case::Elative => format!("{}{}", d.pl_stem, pick_harmony(front, "sta", "stä")),
        Case::Adessive => format!("{}{}", d.pl_stem, pick_harmony(front, "lla", "llä")),
        Case::Ablative => format!("{}{}", d.pl_stem, pick_harmony(front, "lta", "ltä")),
        Case::Allative => format!("{}lle", d.pl_stem),
        Case::Abessive => format!("{}{}", d.pl_stem, pick_harmony(front, "tta", "ttä")),
        Case::Instructive | Case::Comitative | Case::Prolative | Case::Accusative => String::new(),
    };
    let (base, third) = match case {
        Case::Nominative => (bare.clone(), ThirdPossessive::None),
        Case::Genitive => (
            d.gen_pl[0].trim_end_matches('n').to_string(),
            ThirdPossessive::Long,
        ),
        // The plural partitive is a short-vowel stem, so it takes the short
        // third person too (`puhuviaan`).
        Case::Partitive => (bare.clone(), ThirdPossessive::LongAndShort),
        Case::Illative => (
            bare.trim_end_matches('n').to_string(),
            ThirdPossessive::Long,
        ),
        Case::Translative => (bare.clone(), ThirdPossessive::None),
        _ => (bare.clone(), ThirdPossessive::LongAndShort),
    };
    (bare, base, third)
}

// ---------------------------------------------------------------------------
// The type-1 stems
// ---------------------------------------------------------------------------

/// The stems one Kotus verb type gives a verb.
///
/// A type-1 verb differs from its neighbours in just these stems: how many
/// characters the infinitive's ending is (which sets the present stem and the
/// theme vowel), what the past runs on, and what the passive theme becomes.
/// The paradigm that hangs off them is [`paradigm`].
pub struct Stems {
    pub lemma: String,
    /// The strong present theme stem: the infinitive without its ending
    /// (`sano` for `sanoa`, `aakkosta` for `aakkostaa`).
    pub stem: String,
    /// The past stem: the same, less the theme vowel, which the past ending
    /// would otherwise double (`puhu` for `puhua`, but `aakkost` for
    /// `aakkostaa`, giving `aakkostin` beside `aakkostaa`... rather
    /// `aakkostaa`).
    pub past_stem: String,
    /// The grade the past cells take where those cells gradate. For classes 52
    /// and 53 it is the weak grade of the present stem, less its theme vowel
    /// (`haukkoa` -> `haukoin`); class 54's past is a stem of its own and does
    /// not gradate, so there it is `past_stem` again.
    pub past_weak: String,
    /// The passive theme stem. For an `a`/`ä` theme it is the stem with that
    /// vowel replaced by `e`, which is what puts `muistetaan` beside `sanoa`'s
    /// `sanotaan`; for every other theme the stem stands.
    pub p_theme: String,
    /// The consonant gradation the class names (`53*C` is `tt <-> t`), if it
    /// names one. Type 1 gradates in a fixed pattern of cells, so the code is
    /// all a stem needs to know which grade each one takes; see
    /// [`Stems::weak`].
    pub gradation: Option<Gradation>,
    /// The stem the conditional runs on, which is the present stem wherever the
    /// ending `isi` can follow the theme: `muistaisi` beside `aakkostaa`,
    /// `sanoisi` beside `sanoa`. Classes 58 and 59 are the exception, because
    /// there the theme is the stem's own `e` and attaching `isi` to it would give
    /// `laskeisi` and `tunteisi`; their conditional follows the `i`-stem
    /// instead, so `laskisi` and `tuntisi`. The potential and the imperative keep
    /// the present stem either way, which is why `laskenee` and `laskekoon` sit
    /// beside `laskisi`.
    pub cond_stem: String,
    /// The type's *alternant* conditional stem, where the sanalista's type row
    /// puts one in its parentheses: class 74 is `katkeaisi (katkeisi)`, and the
    /// parentheses there are not a spare form but a second spelling of all six
    /// persons, each of which is a `both` row for every member that has one. So
    /// the alternant is a whole second set of the six finite cells and nothing
    /// else -- the passive conditional (`katkettaisiin`), the potential
    /// (`katkenneisiin`) and the negated forms have one spelling each, and
    /// `paradigm` builds those from stems this field does not touch.
    ///
    /// `None`, the default in every constructor, leaves the other types with the
    /// one conditional they have.
    pub cond_alt: Option<String>,
    /// The stem the second infinitive's inessive `ssa` and its instructive `n`
    /// attach to, which is the present stem plus the theme `e` wherever the
    /// ending can follow it: `muiste` for `muisteessa` beside `aakkostaa`,
    /// `sane` for `saneessa` beside `sanoa`. Classes 58 and 59 are the exception,
    /// for the same reason their conditional is, so this is the `i`-stem plus the
    /// inessive's own `e`: `laskie` for `laskiessa` and `laskien`, `tuntie` for
    /// `tuntiessa` and `tuntien`, where the theme would have given `laskeessa`,
    /// `laskeen` and `tunteessa`, `tunteen`. The other cells on this stem are
    /// its possessives.
    pub inf2_stem: String,
    /// The third person singular present's ending, which is the stem's own last
    /// letter wherever the type repeats it: `sallii` beside `sallin`, `sanoo`
    /// beside `sanon`, `laskee` beside `lasken`. Class 62 is the exception --
    /// its infinitive is `-ida`, so the stem's `i` is the type's `i` and there
    /// is nothing left to repeat, giving the bare stem `voi` -- so it sets the
    /// ending empty. See [`Stems::pres3sg_of`].
    pub pres3sg: String,
    /// The stem and leading consonant the potential mood runs on; see [`Alt`].
    /// Type 1 runs it on the strong present stem with an `n`, which with the
    /// mood's own `-e` is the whole of `-nen`.
    pub pot: Alt,
    /// The same, for the past active participle, which takes the same leading
    /// consonant as the potential in every type and so travels with it.
    pub past_part: Alt,
    /// A type's *second* spelling of those two cells, where the sanalista's
    /// type row gives two. Class 76 is the instance, and it spells them apart by
    /// stem rather than by ending: its potential is both `tiennee` (the weak
    /// base with the type's `n`) and `tietänee` (the theme stem with the same
    /// `n`), and its past active participle both `tiennyt` and `tietänyt`, with
    /// `tienneet` and `tietäneet` in the plural and both spellings reaching the
    /// compound phrases (`on tiennut` beside `on tietänyt`, `ei ole tiennyt`
    /// beside `ei ole tietänyt`). So one [`Alt`] carries the alternant of both
    /// cells, which is what the type does — the two are spelled the same way
    /// round — and `paradigm` runs each cell once per stem. A type that spelled
    /// one of the two twice and not the other would need two fields here.
    ///
    /// `None`, the default in every constructor, leaves the other types with the
    /// one potential and one participle they have.
    pub alt: Option<Alt>,
    /// The stem the imperative's `-kaa` and `-koon` cells run on. Type 1 uses
    /// the strong present stem, where the gradation shows (`haukokaa` beside
    /// `hauko`); class 66 uses its past stem instead, weak grade and all.
    pub imp_stem: String,
    /// How the passive writes the consonant it adds to the theme; see
    /// [`PassiveCons`]. Only classes 62 to 67 are not [`PassiveCons::Geminated`].
    pub p_cons: PassiveCons,
    /// The singular oblique stem of the past passive participle, for the class
    /// whose nominative does not say which stem its oblique has.
    ///
    /// Usually it does: type 1's oblique drops the participle's own doubled `t`
    /// (`puhuttu` -> `puhu`) and class 62's deranks that `t` instead (`voitu` ->
    /// `voidu`), so [`tu_participle`] can read the stem off the nominative. Class
    /// 67's cannot be read off at all, because its `-ltu` nominative `tultu` has
    /// `tullu` in the oblique -- an `l` that is the class's own, the same one its
    /// potential and its past active participle carry (`tul` + `l`). So that
    /// class hands the stem over, and `None` leaves every other type to
    /// [`tu_participle`].
    pub p_tu_oblique: Option<String>,
}

impl Stems {
    /// A type whose past runs on the same stem as its present.
    pub fn new(lemma: &str, stem: &str, gradation: Option<Gradation>) -> Self {
        Self::with_past(lemma, stem, stem, gradation)
    }

    /// The same, for a type that shortens the theme in the past. Class 53 is
    /// the one so far: `aakkostaa` inflects `aakkostaa` in the present but
    /// `aakkosti` in the past, so the past runs on `aakkost` and not on
    /// `aakkosta`.
    ///
    /// The passive theme is the weak grade with a stem-final `a`/`ä` read as
    /// the passive's own `e`, which is what puts `aakkostetaan` beside `sanoa`'s
    /// `sanotaan` -- and, where the class gradates, `haukottaa` beside the
    /// strong stem's `haukk`.
    pub fn with_past(lemma: &str, stem: &str, past: &str, gradation: Option<Gradation>) -> Self {
        let weak = Self::weaken(stem, gradation);
        // The past takes the weak grade of the stem, less the theme vowel,
        // wherever it gradates at all: `helpottaa` has `helpotin` beside the
        // strong `helpottivat`.
        let past_weak = if gradation.is_none() {
            past.to_string()
        } else {
            Self::past_stem_of(&weak)
        };
        Self {
            lemma: lemma.to_string(),
            stem: stem.to_string(),
            past_stem: past.to_string(),
            past_weak,
            p_theme: Self::retheme(&weak, 'e'),
            gradation,
            cond_stem: stem.to_string(),
            cond_alt: None,
            inf2_stem: format!("{stem}e"),
            pres3sg: Self::pres3sg_of(stem),
            pot: Alt::n(stem),
            past_part: Alt::n(stem),
            alt: None,
            imp_stem: stem.to_string(),
            p_cons: PassiveCons::Geminated,
            p_tu_oblique: None,
        }
    }

    /// The same, for a type whose past is a stem of its own rather than a grade
    /// of the present one. Class 54 is the one: `huutaa` inflects `huusi` in
    /// every past cell, on a stem the present never shows, and the class's `F`
    /// appears only where the present and the passive do (`huudan`,
    /// `huudetaan`).
    pub fn with_single_past(
        lemma: &str,
        stem: &str,
        past: &str,
        gradation: Option<Gradation>,
    ) -> Self {
        let mut s = Self::with_past(lemma, stem, past, gradation);
        s.past_weak = past.to_string();
        s
    }

    /// The `-ida`/`-idä` type, which classes 62 and 68 are both instances of.
    ///
    /// The stem is the lemma less `da`/`dä` -- `voi` for `voida`, `tupakoi` for
    /// `tupakoida`, `otsikoi` for `otsikoida` -- and because the ending's `i`
    /// is the stem's own last letter the past is the *same word* in all six
    /// cells, `paradigm`'s `absorb_i` dropping the endings' own `i` in place
    /// (`voin`, `voit`, `voi`). Three of the fields are the type's, not a
    /// subclass's:
    ///
    /// * the third person has nothing to repeat, so `pres3sg` is empty rather
    ///   than the stem's last letter as in every other type (`voi`, not
    ///   `voii`);
    /// * the second infinitive runs on the ending's `d` (`voidessa` is `void` +
    ///   `essa`, `tupakoidessa` is `tupakoid` + `essa`), so `inf2_stem` is the
    ///   stem with `de` on it;
    /// * and the passive deranks the `t` it adds to its theme (`voidaan`
    ///   beside `voitiin`, `tupakoidaan` beside `tupakoitiin`).
    ///
    /// The conditional needs nothing: `cond_stem` is the stem itself, and
    /// `absorb_i` swallows the ending's `i` in front of `isi` to give `voisi`
    /// and `tupakoisi` alike. What class 68 adds beside this type is a second
    /// paradigm on the `-tse` stem, which is its own module's business.
    pub fn ida(lemma: &str) -> Self {
        let stem = lemma
            .strip_suffix("da")
            .or_else(|| lemma.strip_suffix("dä"))
            .unwrap_or(lemma);
        let mut s = Self::with_past(lemma, stem, stem, None);
        s.pres3sg = String::new();
        s.inf2_stem = format!("{stem}de");
        s.p_cons = PassiveCons::Deranked;
        s
    }

    /// The `-ita`/`-itä` type, which class 69 is the instance of.
    ///
    /// One base carries every finite cell of the class: `valit`, the lemma less
    /// its `ta`/`tä` with the type's own `t` in front (`vali` + `t`). Three of
    /// the cells take a letter on the front of it -- `se` for the present
    /// (`valitse` in `valitsen`, `valitsee` and `valitsevat`), `si` for the
    /// past (`valitsi` in all six, where the endings' own `i` is absorbed) and
    /// `s` for the conditional, which the ordinary `-isi` ending then goes on
    /// whole (`valitsisi`, `valitsisin`, `valitsisimme`). The past and the
    /// conditional are the same stem reached two ways: `valits` + `i` is
    /// `valitsi` and `valits` + `isi` is `valitsisi`. That is the same base
    /// and the same `s` class 68's `-tse` alternant is built on -- where the
    /// `i` is on the stem and swallowed instead, so its conditional is
    /// `tupakoitsisi` where this type's is `valitsisi` -- which is the whole of
    /// what the two classes have in common beyond their infinitive endings.
    ///
    /// Four of the fields are the type's own:
    ///
    /// * the second infinitive and the imperative's plurals and third person
    ///   run on the base rather than on the present stem -- `valitessa` and
    ///   `valiten` where `valitse` would give `valitsessa` and `valitsen`,
    ///   `valitkaa` and `valitkoon` where it would give `valitsekaa` and
    ///   `valitsekoon` -- so `inf2_stem` is the base with `e` on it and
    ///   `imp_stem` is the base bare;
    /// * the passive's theme is the base too, and it is the base's own `t` that
    ///   serves as the passive's consonant, so the present adds nothing
    ///   (`valitaan`) where every other type adds one and class 67 adds back
    ///   the theme's (`tullaan`), and everything else gemmates as usual
    ///   (`valittiin`, `valittava`);
    /// * the potential and the past active participle are the lemma's own stem
    ///   -- the base less the type's own `t`, `vali` for `valita` -- with the
    ///   `n` doubled in front of the ending, so the two cells are `valin` + `n` +
    ///   `nee` and `valin` + `n` + `ut` (`valinnee`, `valinnut`) on the
    ///   ordinary [`Alt::n`] shape where type 1 writes `sano` + `n` + `ee`;
    /// * and the conditional needs its own stem, the base with `s` on it rather
    ///   than `si` or `se`: `absorb_i` swallows the ending's own `i` in front of
    ///   an `i`-final stem, so a `sisi` stem would give `valitsisisi` for the
    ///   third person where the word is `valitsisi`, and `valits` + `isi` gives
    ///   all six.
    pub fn ita(lemma: &str) -> Self {
        let bare = lemma
            .strip_suffix("ta")
            .or_else(|| lemma.strip_suffix("tä"))
            .unwrap_or(lemma);
        let base = format!("{bare}t");
        let mut s = Self::with_past(lemma, &format!("{base}se"), &format!("{base}si"), None);
        s.p_theme = base.clone();
        s.cond_stem = format!("{base}s");
        s.inf2_stem = format!("{base}e");
        s.imp_stem = base.clone();
        s.pot = Alt::n(&format!("{bare}n"));
        s.past_part = Alt::n(&format!("{bare}n"));
        s.p_cons = PassiveCons::ThemeOwn;
        s
    }

    /// The `-sta`/`-stä` type, which classes 66 and 70 are both instances of.
    ///
    /// The lemma is consonant-final once its `-ta`/`-tä` is off -- `rohkais` for
    /// `rohkaista`, `juos` for `juosta` -- and every cell runs on that stem or on
    /// its strong grade, which the caller names in `strong` because the two
    /// classes reach it differently. Class 66's lemma *is* the weak grade and the
    /// finite present and past read the alternation backwards (`häväis` ->
    /// `häpäis`); class 70's is weak too but its strong grade is the type's own
    /// `k` (`juos` -> `juoks`, `pies` -> `pieks`). `strong` is the strong stem in
    /// both cases, and the present is that with the theme `e` on it (`rohkaise`,
    /// `juokse`), which is the one letter `pres3sg` repeats.
    ///
    /// Five fields are the type's own and all five are the *weak* stem, which is
    /// the lemma's own: the potential's `-s`-lead and the past active participle
    /// (`rohkais` + `s` + `ee`, `juos` + `s` + `ut`), the imperative's plurals
    /// (`rohkaiskaa`, `juoskaa`), the passive theme (`rohkaistaan`,
    /// `juostaan`) and the second infinitive's own `te` (`rohkaistessa`,
    /// `juostessa`). The rest of the paradigm -- the third and fourth
    /// infinitives, the present active participle, the agent and the negative
    /// participle -- takes the *strong* stem, so the two grades split the cells
    /// between them and the imperative alone straddles both: its second person
    /// singular is the present stem (`rohkaise`, `juokse`) and its plurals are
    /// the weak one.
    ///
    /// The passive writes one `t` each way, present and past alike
    /// (`rohkaistaan` beside `rohkaistiin`, `juostaan` beside `juostiin`), and
    /// its `-tu` participle is not deranked: [`PassiveCons::SingleT`].
    pub fn sta(lemma: &str, strong: &str) -> Self {
        let base = lemma
            .strip_suffix("ta")
            .or_else(|| lemma.strip_suffix("tä"))
            .unwrap_or(lemma);
        let mut s = Self::with_past(lemma, &format!("{strong}e"), strong, None);
        s.pot = Alt {
            stem: base.to_string(),
            lead: "s",
        };
        s.past_part = Alt {
            stem: base.to_string(),
            lead: "s",
        };
        s.imp_stem = base.to_string();
        s.p_theme = base.to_string();
        s.cond_stem = strong.to_string();
        s.inf2_stem = format!("{base}te");
        s.p_cons = PassiveCons::SingleT;
        s
    }

    /// The same, for a type whose past keeps the theme vowel as `o`/`ö`
    /// instead of dropping it. Class 56 is the one: `kaivaa` inflects `kaivoi`
    /// and `kaivoin`, where the past stem is the stem's final `a` read as `o`
    /// (`ä` as `ö`). As in class 53 the strong grade gives the third person
    /// (`auttaa` -> `auttoi`) and the weak grade the other persons (`autoin`),
    /// so the class's gradation shows in the past as it does everywhere.
    pub fn with_o_past(lemma: &str, stem: &str, gradation: Option<Gradation>) -> Self {
        let weak = Self::weaken(stem, gradation);
        Self {
            lemma: lemma.to_string(),
            stem: stem.to_string(),
            past_stem: Self::o_past_stem_of(stem),
            past_weak: Self::o_past_stem_of(&weak),
            p_theme: Self::retheme(&weak, 'e'),
            gradation,
            cond_stem: stem.to_string(),
            cond_alt: None,
            inf2_stem: format!("{stem}e"),
            pres3sg: Self::pres3sg_of(stem),
            pot: Alt::n(stem),
            past_part: Alt::n(stem),
            alt: None,
            imp_stem: stem.to_string(),
            p_cons: PassiveCons::Geminated,
            p_tu_oblique: None,
        }
    }

    /// The same, for the type whose theme vowel is the stem's own final `e`.
    /// Class 58 is the one: `laskea` inflects `laskee` in the present and
    /// `laski` in the past, so the past stem is the stem less that `e` (`laske`
    /// -> `lask`, `kylpe` -> `kylp`) rather than less the infinitive's theme.
    /// The consonant is left as it stands.
    ///
    /// Gradation is what makes this more than a renamed `with_past`, and it
    /// shows here and not in class 53. In class 53 the two operations commute --
    /// the gradation site sits before the theme vowel, so weakening the stem and
    /// then dropping the vowel gives the same stem as dropping it and then
    /// weakening. Here they do not, because the vowel dropped *is* the last
    /// consonant's neighbour: `hakea` (`58*D`) drops its `e` first (`hak`) and
    /// gradates after (`ha`), giving `hain` beside the strong `haki`, where the
    /// present's weak grade is `hae` and keeps the `e`. So the past's weak grade
    /// is `weaken` of the past stem, not the theme vowel dropped from the
    /// present's, which is what `past_gradates` selects below.
    pub fn with_e_past(lemma: &str, stem: &str, gradation: Option<Gradation>) -> Self {
        let past = Self::e_past_stem_of(stem);
        Self::e_themed(lemma, stem, &past, gradation, true)
    }

    /// Class 59's (`tuntea`), which is class 58's type with class 54's `s` past:
    /// the stem less its theme `e` and its final consonant turned to `s` (`tunte`
    /// -> `tuns`), and every past cell on that one stem, gradation and all, as
    /// in class 54.
    ///
    /// The `past` is an argument rather than worked out here because class 60
    /// shares the theme but not the rule: its second past stem is `läks`, which
    /// no letter change of `läht` produces. See that module's `s_past_stem`.
    pub fn with_e_s_past(
        lemma: &str,
        stem: &str,
        past: &str,
        gradation: Option<Gradation>,
    ) -> Self {
        Self::e_themed(lemma, stem, past, gradation, false)
    }

    /// The stems of a type whose theme vowel is the stem's own final `e`, which
    /// classes 58 and 59 share and every other type does not.
    ///
    /// What the theme being the stem's last letter costs is the two cells that
    /// cannot take the ending `isi` or `ssa` straight onto the stem, because the
    /// `e` is already there: `laske` + `isi` would be `laskeisi` and `laske` +
    /// `ssa` `laskeessa`, where the words are `laskisi` and `laskiessa`. So both
    /// move onto the verb's `i`-stem, the stem less that `e` (`lask`, `tunt`),
    /// which is where the `i` of `isi` and of `ien` belongs: `tuntisi`,
    /// `tuntien` beside `tunteen`. The `i`-stem is the stem less the `e` and not
    /// the past stem, which in class 59 has also turned its consonant to `s`.
    ///
    /// `past` is the past stem proper and `past_gradates` whether its cells
    /// alternate; see [`Stems::with_e_past`] for why the two orders of dropping
    /// and gradating differ.
    fn e_themed(
        lemma: &str,
        stem: &str,
        past: &str,
        gradation: Option<Gradation>,
        past_gradates: bool,
    ) -> Self {
        let weak = Self::weaken(stem, gradation);
        let i_stem = Self::e_past_stem_of(stem);
        Self {
            lemma: lemma.to_string(),
            stem: stem.to_string(),
            past_stem: past.to_string(),
            past_weak: if past_gradates {
                Self::weaken(past, gradation)
            } else {
                past.to_string()
            },
            p_theme: Self::retheme(&weak, 'e'),
            gradation,
            cond_stem: i_stem.clone(),
            cond_alt: None,
            inf2_stem: format!("{i_stem}ie"),
            pres3sg: Self::pres3sg_of(stem),
            pot: Alt::n(stem),
            past_part: Alt::n(stem),
            alt: None,
            imp_stem: stem.to_string(),
            p_cons: PassiveCons::Geminated,
            p_tu_oblique: None,
        }
    }

    /// The third person singular present's ending for a type that repeats its
    /// stem's last letter: the letter itself, or `a` for an empty stem, which is
    /// what `paradigm` fell back on before the field existed.
    pub fn pres3sg_of(stem: &str) -> String {
        stem.chars()
            .last()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "a".to_string())
    }

    /// The weak grade of a stem: the same stem with `gradation` applied, or
    /// the stem itself where the class names none.
    fn weaken(stem: &str, gradation: Option<Gradation>) -> String {
        match gradation {
            Some(g) => g.apply(stem, Grade::Weak),
            None => stem.to_string(),
        }
    }

    /// The weak grade of the stem, where the class gradates.
    ///
    /// A type-1 verb gradates in a fixed pattern, and the pattern is what makes
    /// the alternation a class fact rather than a lexical one. The third
    /// persons of the present and the past, the whole conditional and
    /// potential, the imperative's third person and its plurals, and every
    /// participle and infinitive take the *strong* stem, which is the one the
    /// infinitive itself shows (`haukkoa` is `haukoo` in the third person but
    /// `haukon` and `haukottaa` everywhere else, `täytyy` beside `täydyn` and
    /// `täydytty`). The remaining cells -- the other persons of the present and
    /// the past, the imperative's second person singular, the whole passive
    /// and the negated present -- take the weak grade, which is the stem with
    /// the class's gradation applied.
    ///
    /// Without a gradation code there is nothing to apply and the two grades
    /// are the same word.
    pub fn weak(&self) -> String {
        Self::weaken(&self.stem, self.gradation)
    }

    /// The passive present base, before `-aan`/`-ään`: `sanot`, `aakkostet`,
    /// `void`.
    pub fn passive_present(&self) -> String {
        format!("{}{}", self.p_theme, self.p_cons.present(&self.p_theme))
    }

    /// The passive past base, before `-iin`: `sanott`, `aakkostett`, `voit`.
    pub fn passive_past(&self) -> String {
        format!("{}{}", self.p_theme, self.p_cons.past())
    }

    /// The passive mood stem, before `-isiin`/`-neen`/`-koon`: `sanotta`,
    /// `aakkostetta`.
    pub fn passive_mood(&self, front: bool) -> String {
        format!("{}{}", self.passive_past(), pick_harmony(front, "a", "ä"))
    }

    /// Replace a stem-final `a`/`ä` by `e`, the way a type-1 passive does.
    pub fn passive_theme_of(stem: &str) -> String {
        Self::retheme(stem, 'e')
    }

    /// Drop a stem-final `a`/`ä`, the way a type-1 past does where the theme is
    /// that vowel (`aakkosta` -> `aakkost`). A stem ending in anything else is
    /// its own past stem.
    pub fn past_stem_of(stem: &str) -> String {
        if stem.ends_with('a') || stem.ends_with('ä') {
            let mut chars: Vec<char> = stem.chars().collect();
            chars.pop();
            return chars.into_iter().collect();
        }
        stem.to_string()
    }

    /// The `o` past stem of the `kaivaa` type: the stem's theme vowel read as
    /// `o`/`ö` rather than dropped (`kaiva` -> `kaivo`, `auta` -> `auto`).
    /// A stem ending in anything else is its own past stem.
    pub fn o_past_stem_of(stem: &str) -> String {
        let to = if stem.ends_with('ä') { 'ö' } else { 'o' };
        Self::retheme(stem, to)
    }

    /// The `laskea` type's past stem: the stem less its theme vowel, which in
    /// this theme is the stem's own final `e` (`laske` -> `lask`, `kylpe` ->
    /// `kylp`). A stem ending in anything else is its own past stem.
    pub fn e_past_stem_of(stem: &str) -> String {
        if stem.ends_with('e') {
            let mut chars: Vec<char> = stem.chars().collect();
            chars.pop();
            return chars.into_iter().collect();
        }
        stem.to_string()
    }

    /// The `s` past stem of the `tuntea` type: class 54's rule with class 58's
    /// vowel, the stem less its own theme `e` and then its final consonant turned
    /// to `s` (`tunte` -> `tuns`). Class 58 drops that `e` but leaves the
    /// consonant standing (`laske` -> `lask`); this type does both, which is
    /// what puts `tunsi` beside `tuntee`.
    pub fn e_s_past_stem_of(stem: &str) -> String {
        let mut base: Vec<char> = Self::e_past_stem_of(stem).chars().collect();
        if let Some(last) = base.last_mut() {
            *last = 's';
        }
        base.into_iter().collect()
    }

    /// The `s` past stem of the `huutaa` type: the stem less its theme vowel,
    /// with that final consonant replaced by `s` (`huuta` -> `huus`, `rakenta`
    /// -> `rakens`, `askelta` -> `askels`). A consonant that already is `s`
    /// stays (`lypsää` -> `lyps`), so the replacement is uniform.
    pub fn s_past_stem_of(stem: &str) -> String {
        let mut base: Vec<char> = Self::past_stem_of(stem).chars().collect();
        if let Some(last) = base.last_mut() {
            *last = 's';
        }
        base.into_iter().collect()
    }

    fn retheme(stem: &str, to: char) -> String {
        let mut chars: Vec<char> = stem.chars().collect();
        if let Some(last) = chars.last_mut()
            && matches!(*last, 'a' | 'ä')
        {
            *last = to;
        }
        chars.into_iter().collect()
    }
}

/// The whole paradigm of an ungradated type-1 verb: the five finite active
/// moods, the finite passive, the negative main-verb words, the five
/// infinitives and the six participles, all declined with their possessives.
pub fn paradigm(s: &Stems) -> Vec<Form> {
    let mut b = VerbBuilder::new(&s.lemma);
    let front = b.front();
    // The two grades of the stem, where the class gradates they differ and
    // where it does not they are the same word; see `Stems::weak` for which
    // cells take which.
    let stem = s.stem.clone();
    let weak = s.weak();
    let past_stem = s.past_stem.clone();
    let past_weak = s.past_weak.clone();
    let cond_stem = s.cond_stem.clone();
    // The third person repeats the stem's own last letter, except in class 62,
    // whose `Stems::pres3sg` is empty.
    let last = s.pres3sg.clone();
    // The past active participle in the nominative, which the negated past
    // quotes whole (`ei haukkonut`) and which the declension below is built
    // from. It is the class's own stem and leading consonant, so class 66's is
    // `rohkaissut` where type 1 has `sanonut`. A type that spells the cell
    // twice has one of these per stem: `tiennyt` beside `tietänyt`.
    let nut_ending = if front { "yt" } else { "ut" };
    let past_parts: Vec<String> = std::iter::once(&s.past_part)
        .chain(s.alt.iter())
        .map(|alt| format!("{}{}{}", alt.stem, alt.lead, nut_ending))
        .collect();
    let a = if front { 'ä' } else { 'a' };
    let o = if front { 'ö' } else { 'o' };
    let e_h = b.h("ssa", "ssä");
    let p_present = s.passive_present();
    let p_past = s.passive_past();
    let p_mood = s.passive_mood(front);

    // --- finite active ---
    // A stem that already ends in `i` does not take a second one, so every
    // ending that opens with `i` loses it: `hilsehtiä` has `hilsehdin`,
    // `hilsehtimme` and `hilsehti` in the past and `hilsehtisin` in the
    // conditional, all on the stem's own `i` rather than a doubled one.
    let absorb_i = |s: &str, e: &str| -> String {
        if s.ends_with('i') {
            e.trim_start_matches('i').to_string()
        } else {
            e.to_string()
        }
    };
    // Present: the weak grade for everyone but the third person, which takes
    // the strong stem and repeats its own final vowel.
    for (stem, e, person, number) in [
        (&weak, "n".to_string(), Person::First, Number::Singular),
        (&weak, "t".to_string(), Person::Second, Number::Singular),
        (&stem, last.clone(), Person::Third, Number::Singular),
        (&weak, "mme".to_string(), Person::First, Number::Plural),
        (&weak, "tte".to_string(), Person::Second, Number::Plural),
        (&stem, b.h("vat", "vät"), Person::Third, Number::Plural),
    ] {
        b.push(
            format!("{stem}{e}"),
            verb_tags(
                Some(Tense::Present),
                Some(Mood::Indicative),
                Some(person),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        );
    }
    // Past: the past stem with `i` and the past endings. The past stem is the
    // present stem less its theme vowel, which is only visible where the theme
    // is `a`/`ä` (`aakkostaa` -> `aakkosti`, not `aakkostai`). As in the
    // present, the third person keeps the strong grade.
    for (past, e, person, number) in [
        (&past_weak, "in", Person::First, Number::Singular),
        (&past_weak, "it", Person::Second, Number::Singular),
        (&past_stem, "i", Person::Third, Number::Singular),
        (&past_weak, "imme", Person::First, Number::Plural),
        (&past_weak, "itte", Person::Second, Number::Plural),
        (
            &past_stem,
            if front { "ivät" } else { "ivat" },
            Person::Third,
            Number::Plural,
        ),
    ] {
        b.push(
            format!("{past}{}", absorb_i(past, e)),
            verb_tags(
                Some(Tense::Past),
                Some(Mood::Indicative),
                Some(person),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        );
    }
    // The conditional's six endings, and the two stems they can go on: the
    // type's own, and the alternant where the sanalista's type row gives one
    // (`katkeaisi` beside `katkeisi`). See `Stems::cond_alt`.
    let cond_cells = [
        ("isin", Person::First, Number::Singular),
        ("isit", Person::Second, Number::Singular),
        ("isi", Person::Third, Number::Singular),
        ("isimme", Person::First, Number::Plural),
        ("isitte", Person::Second, Number::Plural),
        (
            if front { "isivät" } else { "isivat" },
            Person::Third,
            Number::Plural,
        ),
    ];
    let cond_stems = std::iter::once(&cond_stem).chain(s.cond_alt.iter());
    for stem in cond_stems {
        for (e, person, number) in cond_cells {
            b.push(
                format!("{stem}{}", absorb_i(stem, e)),
                verb_tags(
                    Some(Tense::Present),
                    Some(Mood::Conditional),
                    Some(person),
                    Some(number),
                    Some(Voice::Active),
                    None,
                    Polarity::Affirmative,
                ),
            );
        }
    }
    // Potential: `-ne` on the stem, the 3rd person doubling the `e`. The
    // consonant in front of the `e` is the class's own (`Alt::lead`) and the
    // stem is the class's own too, since both differ from type 1: class 66 runs
    // the mood on its past stem and writes the infinitive's `s` in front of the
    // `e`, so `rohkais` + `s` + `ee` is the `rohkaissee` the reference has.
    // A type that spells the mood twice runs the six cells once per stem:
    // `tiennee` beside `tietänee`. See `Stems::alt`.
    let pot_cells = [
        ("en".to_string(), Person::First, Number::Singular),
        ("et".to_string(), Person::Second, Number::Singular),
        ("ee".to_string(), Person::Third, Number::Singular),
        ("emme".to_string(), Person::First, Number::Plural),
        ("ette".to_string(), Person::Second, Number::Plural),
        (b.h("evat", "evät"), Person::Third, Number::Plural),
    ];
    let pot_stems = std::iter::once(&s.pot).chain(s.alt.iter());
    for pot in pot_stems {
        for (e, person, number) in &pot_cells {
            b.push(
                format!("{}{}{}", pot.stem, pot.lead, e),
                verb_tags(
                    Some(Tense::Present),
                    Some(Mood::Potential),
                    Some(*person),
                    Some(*number),
                    Some(Voice::Active),
                    None,
                    Polarity::Affirmative,
                ),
            );
        }
    }
    // Imperative: the bare weak stem (2sg), then `-koon`/`-kaa` by harmony on
    // the class's own stem, which is the strong present stem in type 1, where
    // the gradation shows (`hauko` beside `haukkokaa`), and the past stem in
    // class 66 (`rohkaise` beside `rohkaiskaa`).
    b.push(
        weak.clone(),
        verb_tags(
            Some(Tense::Present),
            Some(Mood::Imperative),
            Some(Person::Second),
            Some(Number::Singular),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    );
    for (e, person, number) in [
        (format!("k{o}{o}n"), Person::Third, Number::Singular),
        (format!("k{a}{a}mme"), Person::First, Number::Plural),
        (format!("k{a}{a}"), Person::Second, Number::Plural),
        (format!("k{o}{o}t"), Person::Third, Number::Plural),
    ] {
        b.push(
            format!("{}{e}", s.imp_stem),
            verb_tags(
                Some(Tense::Present),
                Some(Mood::Imperative),
                Some(person),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        );
    }

    // --- finite passive ---
    //
    // Only the present passive is a person: it is a finite form of the whole
    // verb, and Finnish has it in the third person singular alone (`lasketaan`,
    // `muistetaan`). The past passive `laskettiin` is the impersonal passive of
    // a state, and the conditional, potential and imperative passives are moods
    // of it rather than persons of the present, so all four stay personless --
    // which is also all Kotus records for any passive, `person` included.
    for (word, tense, mood, third) in [
        (
            format!("{p_present}{a}{a}n"),
            Tense::Present,
            Mood::Indicative,
            true,
        ),
        (format!("{p_past}iin"), Tense::Past, Mood::Indicative, false),
        (
            format!("{p_mood}isiin"),
            Tense::Present,
            Mood::Conditional,
            false,
        ),
        (
            format!("{p_mood}neen"),
            Tense::Present,
            Mood::Potential,
            false,
        ),
        (
            format!("{p_mood}k{o}{o}n"),
            Tense::Present,
            Mood::Imperative,
            false,
        ),
    ] {
        b.push(
            word,
            verb_tags(
                Some(tense),
                Some(mood),
                if third { Some(Person::Third) } else { None },
                if third { Some(Number::Singular) } else { None },
                Some(Voice::Passive),
                None,
                Polarity::Affirmative,
            ),
        );
    }

    // --- the negative main-verb words ---
    let conneg = |tense: Tense, mood: Mood, voice: Voice| {
        verb_tags(
            Some(tense),
            Some(mood),
            None,
            None,
            Some(voice),
            None,
            Polarity::Negative,
        )
    };
    // The negative present is the bare weak stem (`en hauko` beside `haukoo`),
    // and the negative past is the `-nut` participle, which is on the strong
    // grade (`ei haukkonut`).
    b.push(
        weak.clone(),
        conneg(Tense::Present, Mood::Indicative, Voice::Active),
    );
    b.push(
        past_parts[0].clone(),
        conneg(Tense::Past, Mood::Indicative, Voice::Active),
    );
    for alt in &past_parts[1..] {
        b.push(
            alt.clone(),
            conneg(Tense::Past, Mood::Indicative, Voice::Active),
        );
    }
    b.push(
        format!("{p_present}{a}"),
        conneg(Tense::Present, Mood::Indicative, Voice::Passive),
    );
    b.push(
        format!("{p_past}{}", if front { "y" } else { "u" }),
        conneg(Tense::Past, Mood::Indicative, Voice::Passive),
    );
    // The conditional, potential and imperative passive connegatives are not
    // words on their own -- they occur only in `ei ... -isi` / `-ne` and
    // `älköön ... -ko` -- so they are generated at repaired confidence and never
    // offered alone.
    b.push_rare(
        format!("{p_mood}isi"),
        conneg(Tense::Present, Mood::Conditional, Voice::Passive),
    );
    b.push_rare(
        format!("{p_mood}ne"),
        conneg(Tense::Present, Mood::Potential, Voice::Passive),
    );
    b.push_rare(
        format!("{p_mood}{}", b.h("ko", "kö")),
        conneg(Tense::Present, Mood::Imperative, Voice::Passive),
    );

    // --- infinitives ---
    b.push(
        s.lemma.clone(),
        verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(Number::Singular),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    );
    // Long first infinitive: the translative `-kse-` with a possessive
    // suffix (`puhuakseni`, `puhuakseen`). It has no bare form; every cell
    // is possessed, and the third person is the short `-kseen` or the long
    // `-ksensa`/`-ksensä`.
    b.possess_translative(format!("{}ks", s.lemma), |poss| {
        let mut t = verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(Number::Singular),
            Some(Voice::Active),
            Some(Case::Translative),
            Polarity::Affirmative,
        );
        t.possessive = Some(poss);
        t
    });
    // 2nd: `inf2_stem`, usually the present stem's own `-e` stem, in the
    // inessive, with possessives, and the passive stem in the inessive bare
    // (`puhuttaessa`); the instructive is bare. Class 58's is the `i`-stem's
    // (`laskiessa`, `laskien`) -- see `Stems::inf2_stem`.
    let e_stem = s.inf2_stem.clone();
    b.push(
        format!("{e_stem}{e_h}"),
        verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(Number::Singular),
            Some(Voice::Active),
            Some(Case::Inessive),
            Polarity::Affirmative,
        ),
    );
    b.possess(
        format!("{e_stem}{e_h}"),
        ThirdPossessive::LongAndShort,
        |poss| {
            let mut t = verb_tags(
                None,
                Some(Mood::Infinitive),
                None,
                Some(Number::Singular),
                Some(Voice::Active),
                Some(Case::Inessive),
                Polarity::Affirmative,
            );
            t.possessive = Some(poss);
            t
        },
    );
    b.push(
        format!("{p_mood}{}", b.h("essa", "essä")),
        verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(Number::Singular),
            Some(Voice::Passive),
            Some(Case::Inessive),
            Polarity::Affirmative,
        ),
    );
    // 2nd infinitive passive instructive: `puhuttaen`. The stem already ends in
    // the theme vowel, so the instructive takes the bare `en`.
    b.push(
        format!("{p_mood}en"),
        verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(Number::Singular),
            Some(Voice::Passive),
            Some(Case::Instructive),
            Polarity::Affirmative,
        ),
    );
    b.push(
        format!("{e_stem}n"),
        verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(Number::Singular),
            Some(Voice::Active),
            Some(Case::Instructive),
            Polarity::Affirmative,
        ),
    );
    // 3rd: the `-ma` stem in six cases, no possessives, plus the passive
    // instructive `puhuttaman`.
    let ma = format!("{stem}m{a}");
    for (word, case) in [
        (format!("{ma}{a}n"), Case::Illative),
        (format!("{ma}{e_h}"), Case::Inessive),
        (
            format!("{ma}{}", if front { "stä" } else { "sta" }),
            Case::Elative,
        ),
        (
            format!("{ma}{}", if front { "llä" } else { "lla" }),
            Case::Adessive,
        ),
        (
            format!("{ma}{}", if front { "ttä" } else { "tta" }),
            Case::Abessive,
        ),
        (format!("{ma}n"), Case::Instructive),
    ] {
        b.push(
            word,
            verb_tags(
                None,
                Some(Mood::Infinitive),
                None,
                Some(Number::Singular),
                Some(Voice::Active),
                Some(case),
                Polarity::Affirmative,
            ),
        );
    }
    b.push(
        format!("{p_mood}m{a}n"),
        verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(Number::Singular),
            Some(Voice::Passive),
            Some(Case::Instructive),
            Polarity::Affirmative,
        ),
    );
    // 4th: the `-minen` nominal, declined with its own possessives.
    b.decline(&minen_infinitive(&stem, front), |case, number, poss| {
        let mut t = verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(number),
            Some(Voice::Active),
            Some(case),
            Polarity::Affirmative,
        );
        t.possessive = poss;
        t
    });
    // 5th: `-maisillä`/`-maisilla`, which harmonises throughout, with possessives
    // and no cases.
    b.possess(
        format!("{stem}m{a}isill{a}"),
        ThirdPossessive::LongAndShort,
        |poss| {
            let mut t = verb_tags(
                None,
                Some(Mood::Infinitive),
                None,
                None,
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            );
            t.possessive = Some(poss);
            t
        },
    );

    // --- participles ---
    // Present active `-va`: plural stem in `i`, no plural genitive possessive.
    b.decline(
        &va_participle(&format!("{stem}v{a}"), &stem, "v", true),
        |case, number, poss| {
            participle_tags(Some(Tense::Present), Voice::Active, case, number, poss)
        },
    );
    // Agent `-ma`: the `-va` shape, and it does take the plural genitive
    // possessive (`puhumiemme`).
    b.decline(
        &va_participle(&format!("{stem}m{a}"), &stem, "m", true),
        |case, number, poss| participle_tags(None, Voice::Active, case, number, poss),
    );
    // Past active `-nut`: singular stem `-nee`, plural `-nei`, plural genitive
    // possessive on both spellings. The whole declension is on the strong grade
    // (`täytynyt`, `täytyneet` beside the weak `täydyn`), and on the class's own
    // stem with the class's own leading consonant, which for class 66 makes the
    // nominative `rohkaissut` where type 1 has `sanonut`; `past_parts` above is
    // those same nominatives, one per stem the type spells the cell on.
    for past_part in &past_parts {
        b.decline(&nut_participle(past_part, front), |case, number, poss| {
            participle_tags(Some(Tense::Past), Voice::Active, case, number, poss)
        });
    }
    // Present passive participle `-ttava`: the `-va` shape on the mood stem.
    b.decline(
        &va_participle(&format!("{p_mood}v{a}"), &p_mood, "v", true),
        |case, number, poss| {
            participle_tags(Some(Tense::Present), Voice::Passive, case, number, poss)
        },
    );
    // Past passive participle `-ttu`: a thin declension whose possessives keep
    // the doubled `t` the bare genitive drops (`puhuttuni`).
    let ppart = format!("{p_past}{}", if front { "y" } else { "u" });
    let p_tu = match &s.p_tu_oblique {
        Some(oblique) => tu_participle_on(&ppart, oblique),
        None => tu_participle(&ppart, s.p_cons.deranks_tu()),
    };
    b.decline(&p_tu, |case, number, poss| {
        participle_tags(Some(Tense::Past), Voice::Passive, case, number, poss)
    });
    // Negative participle `-maton`: declines and possesses like a noun
    // (`puhumattomani`, `puhumattomiaan`).
    let maton = format!("{stem}m{a}t{}n", o);
    b.decline(
        &maton_participle(&stem, a, front, &maton),
        |case, number, poss| participle_tags(None, Voice::Active, case, number, poss),
    );

    // --- compounds ---
    // The participle spellings worked out above are the ones the compound layer
    // needs: `on puhunut` and `on puhuttu` share their second word with the
    // declensions, so they are read back rather than respelled.
    let past_done = nut_participle(&past_parts[0], front).nom_pl;
    let past_done_alt = past_parts
        .get(1)
        .map(|alt| nut_participle(alt, front).nom_pl);
    // The connegatives the negated moods need, from the positive stems above.
    let cond_conneg = format!("{cond_stem}{}", absorb_i(&cond_stem, "isi"));
    // The alternant's connegative, where the type has one: the negative
    // auxiliary takes `en katkeisi` beside its `en katkeaisi`, and the reference
    // has both. See `Stems::cond_alt`.
    let cond_conneg_alt = s
        .cond_alt
        .as_ref()
        .map(|alt| format!("{alt}{}", absorb_i(alt, "isi")));
    let pot_conneg = format!("{}{}e", s.pot.stem, s.pot.lead);
    let pot_conneg_alt = s
        .alt
        .as_ref()
        .map(|alt| format!("{}{}e", alt.stem, alt.lead));
    let imper_conneg = format!("{}{}", s.imp_stem, b.h("ko", "kö"));
    // The reference also lists the potential and imperative connegatives as
    // surfaces of their own (`vetäne`, `vetäkö`). They occur only after the
    // negative auxiliary, so they are repaired like the passive connegatives
    // and never offered as corrections. Class 76's primary potential stem is
    // the one that collides with another word (`tienne` = `tie` + `-nne`), so
    // its theme-stem alternant is the spelling a bare word may use.
    let pot_surface = pot_conneg_alt.as_ref().unwrap_or(&pot_conneg);
    b.push_rare(
        pot_surface.clone(),
        conneg(Tense::Present, Mood::Potential, Voice::Active),
    );
    b.push_rare(
        imper_conneg.clone(),
        conneg(Tense::Present, Mood::Imperative, Voice::Active),
    );
    compound::emit(
        &mut b,
        &compound::Parts {
            stem: weak,
            pass_present: p_present,
            pass_mood: p_mood,
            done_sg: past_parts[0].clone(),
            done_pl: past_done,
            done_sg_alt: past_parts.get(1).cloned(),
            done_pl_alt: past_done_alt,
            pass_done: ppart,
            cond_conneg,
            cond_conneg_alt,
            pot_conneg,
            pot_conneg_alt,
            imper_conneg,
        },
    );

    b.into_forms()
}

/// The tags of a participle read as a noun: the case, the number and the
/// possessive, with the tense and the voice carried by [`verb_tags`] and no mood
/// or person, since a participle takes neither. Every type that declines a
/// participle goes through here, so the classes that write one cell themselves
/// call this rather than spelling the fields out.
pub fn participle_tags(
    tense: Option<Tense>,
    voice: Voice,
    case: Case,
    number: Number,
    poss: Option<Possessive>,
) -> Tags {
    let mut t = verb_tags(
        tense,
        None,
        None,
        Some(number),
        Some(voice),
        Some(case),
        Polarity::Affirmative,
    );
    t.possessive = poss;
    t
}

// ---------------------------------------------------------------------------
// The participle declensions, shared by every type-1 class
// ---------------------------------------------------------------------------

/// The `-va`/`-ma` declension (`puhuva`, `aakkostama`): the nominative is the
/// base and the plural stem replaces the final vowel with `i`. The agent takes
/// possessives on the plural genitive, the present active participle does not.
pub fn va_participle(base: &str, stem: &str, consonant: &str, gen_pl_possess: bool) -> Decl {
    let a = if front(base) { 'ä' } else { 'a' };
    let pl_stem = format!("{stem}{consonant}i");
    Decl {
        nom_sg: base.to_string(),
        gen_sg: format!("{base}n"),
        part_sg: format!("{base}{a}"),
        ill_sg: format!("{base}{a}n"),
        nom_pl: format!("{base}t"),
        gen_pl: vec![format!("{pl_stem}en")],
        part_pl: format!("{pl_stem}{a}"),
        ill_pl: vec![format!("{pl_stem}in")],
        sg_stem: base.to_string(),
        pl_stem,
        comitative_stem: None,
        ess_sg: None,
        ess_pl: None,
        nom_base: None,
        gen_sg_poss: None,
        possess_gen_pl: gen_pl_possess,
        possess: Possess::All,
        part_sg_third: ThirdPossessive::Long,
    }
}

/// The `-nut` declension: the singular stem is `-nee`, the plural `-nei`, and
/// the nominative possessive is on the genitive stem (`puhuneeni`).
pub fn nut_participle(nom: &str, front: bool) -> Decl {
    let a = if front { 'ä' } else { 'a' };
    let core = nom
        .strip_suffix("ut")
        .or_else(|| nom.strip_suffix("yt"))
        .unwrap_or(nom);
    let sg_stem = format!("{core}ee");
    let pl_stem = format!("{core}ei");
    Decl {
        nom_sg: nom.to_string(),
        gen_sg: format!("{sg_stem}n"),
        part_sg: format!("{nom}t{a}"),
        ill_sg: format!("{sg_stem}seen"),
        nom_pl: format!("{core}eet"),
        gen_pl: vec![format!("{pl_stem}den"), format!("{pl_stem}tten")],
        part_pl: format!("{pl_stem}t{a}"),
        ill_pl: vec![format!("{pl_stem}siin"), format!("{pl_stem}hin")],
        sg_stem,
        pl_stem,
        comitative_stem: None,
        ess_sg: None,
        ess_pl: None,
        nom_base: Some(format!("{core}ee")),
        gen_sg_poss: None,
        possess_gen_pl: true,
        possess: Possess::All,
        part_sg_third: ThirdPossessive::LongAndShort,
    }
}

/// The `-ttu` declension (`puhuttu`): a single `t` in the singular oblique
/// (`puhutun`), the doubled `t` in the plural genitive/partitive/illative
/// (`puhuttujen`), and the essive and illative on the nominative rather than the
/// oblique (`puhuttuna`, `puhuttuun`). It possesses on every case, the
/// possessive keeping the doubled `t` the bare genitive drops (`puhuttuni`).
///
/// `deranked` is [`PassiveCons::Deranked`], which writes a single `t` in the
/// nominative and deranks it to `d` in the oblique (`voitu`, then `voidun`,
/// `voidussa`); otherwise the participle's own gemination is written out in
/// full in the nominative and halved in the oblique (`puhuttu`, then
/// `puhutun`).
pub fn tu_participle(nom: &str, deranked: bool) -> Decl {
    // `puhuttu` -> `puhutu`: the doubled `t` before the final vowel is single
    // in the singular oblique. Only the participle's own doubling goes, so
    // look at the last two before the vowel: a stem that already has `tt`
    // (`ammattilaistuttu`) keeps it.
    let mut chars: Vec<char> = nom.chars().collect();
    if deranked {
        // `voitu` -> `voidu`: the one consonant before the final vowel is
        // deranked rather than doubled, which is the type's own alternation
        // showing in the oblique as well (`PassiveCons::deranks_tu`).
        if chars.len() > 1 {
            let before_vowel = chars.len() - 2;
            if chars[before_vowel] == 't' {
                chars[before_vowel] = 'd';
            }
        }
    } else if chars.len() > 2 && chars[chars.len() - 3] == 't' && chars[chars.len() - 2] == 't' {
        chars.remove(chars.len() - 3);
    }
    let base: String = chars.into_iter().collect();
    tu_participle_on(nom, &base)
}

/// The `-ttu` declension on a singular oblique stem the class names itself.
///
/// [`tu_participle`] covers every type whose nominative says which stem its
/// oblique has. Class 67's is not one of them: its `-ltu` nominative `tultu` has
/// `tullu` in the oblique, where the `l` is the class's own -- the same one its
/// potential (`tul` + `l` + `ee`) and its past active participle (`tul` + `l` +
/// `ut`) carry -- and nothing in `tultu` points at it. So that class hands the
/// stem over through [`Stems::p_tu_oblique`], and this is what declines it.
pub fn tu_participle_on(nom: &str, oblique: &str) -> Decl {
    let front = front(nom);
    let a = if front { 'ä' } else { 'a' };
    Decl {
        nom_sg: nom.to_string(),
        gen_sg: format!("{oblique}n"),
        part_sg: format!("{nom}{a}"),
        ill_sg: format!("{nom}{}", if front { "yn" } else { "un" }),
        nom_pl: format!("{oblique}t"),
        gen_pl: vec![format!("{nom}jen")],
        part_pl: format!("{nom}j{a}"),
        ill_pl: vec![format!("{nom}ihin")],
        pl_stem: format!("{oblique}i"),
        comitative_stem: Some(format!("{nom}i")),
        sg_stem: oblique.to_string(),
        ess_sg: Some(format!("{nom}{}", if front { "nä" } else { "na" })),
        ess_pl: Some(format!("{nom}in{a}")),
        nom_base: None,
        // The possessive keeps the doubled `t` the bare genitive drops, as the
        // reference spells it: `puhuttuni`, not `*puhutuni`.
        gen_sg_poss: Some(nom.to_string()),
        possess_gen_pl: true,
        possess: Possess::All,
        part_sg_third: ThirdPossessive::LongAndShort,
    }
}

/// The `-maton` declension: singular stem `-ttoma`, plural `-ttomi`. The
/// `-tt-` block carries the `o`/`ö`, so the plural oblique runs `kysymättömi`
/// and its genitive is `kysymättömien`. It takes possessives throughout, the
/// nominative's on the genitive stem (`kysymättömäni`) rather than on the
/// short `kysymätön`.
pub fn maton_participle(stem: &str, a: char, front: bool, nom: &str) -> Decl {
    let o = if front { 'ö' } else { 'o' };
    let sg_stem = format!("{stem}m{a}tt{o}m{a}");
    let pl_stem = format!("{stem}m{a}tt{o}mi");
    Decl {
        nom_sg: nom.to_string(),
        gen_sg: format!("{sg_stem}n"),
        part_sg: format!("{stem}m{a}t{o}nt{a}"),
        ill_sg: format!("{sg_stem}{a}n"),
        nom_pl: format!("{sg_stem}t"),
        // The `-ien` plural is the ordinary one and leads, so the possessive
        // builds on it (`puhumattomiemme`); `-nten` is the alternant.
        gen_pl: vec![format!("{pl_stem}en"), format!("{stem}m{a}t{o}nten")],
        part_pl: format!("{pl_stem}{a}"),
        ill_pl: vec![format!("{pl_stem}in")],
        ess_sg: None,
        ess_pl: None,
        // The possessive of the short nominative sits on the genitive stem, as
        // on any noun: `puhumattomani`, not `puhumatonni`.
        nom_base: Some(sg_stem.clone()),
        gen_sg_poss: None,
        sg_stem,
        pl_stem,
        comitative_stem: None,
        possess_gen_pl: true,
        possess: Possess::All,
        part_sg_third: ThirdPossessive::Long,
    }
}

/// The `-minen` infinitive as a noun (`puhuminen`): stem `-mise` in the singular
/// and `-misi` in the plural, possessives throughout including the plural
/// genitive (`puhumisteni`). Its partitive singular harmonises like any other,
/// `kysymistä` as against `puhumista`.
pub fn minen_infinitive(stem: &str, front: bool) -> Decl {
    let a = if front { 'ä' } else { 'a' };
    Decl {
        nom_sg: format!("{stem}minen"),
        gen_sg: format!("{stem}misen"),
        part_sg: format!("{stem}mist{a}"),
        ill_sg: format!("{stem}miseen"),
        nom_pl: format!("{stem}miset"),
        gen_pl: vec![format!("{stem}misten"), format!("{stem}misien")],
        part_pl: format!("{stem}misi{a}"),
        ill_pl: vec![format!("{stem}misiin")],
        sg_stem: format!("{stem}mise"),
        pl_stem: format!("{stem}misi"),
        comitative_stem: None,
        ess_sg: None,
        ess_pl: None,
        nom_base: Some(format!("{stem}mise")),
        gen_sg_poss: None,
        possess_gen_pl: true,
        possess: Possess::All,
        part_sg_third: ThirdPossessive::LongAndShort,
    }
}

/// Build the tags of a verb form. The participle and infinitive readings are
/// not fields of [`Tags`]; the mood, tense and voice carry them, as the
/// nominal engine's own participle tags do.
pub fn verb_tags(
    tense: Option<Tense>,
    mood: Option<Mood>,
    person: Option<Person>,
    number: Option<Number>,
    voice: Option<Voice>,
    case: Option<Case>,
    polarity: Polarity,
) -> Tags {
    Tags {
        part_of_speech: Some(PartOfSpeech::Verb),
        case,
        number,
        person,
        tense,
        mood,
        voice,
        possessive: None,
        polarity,
        ..Tags::UNKNOWN
    }
}
