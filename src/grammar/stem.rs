//! Stems: the part of a lemma that the case endings attach to.
//!
//! Finnish nouns do not all take their endings to the same part of the word.
//! `talo` takes them to the whole lemma, `lapsi` to `lap`, and `kaksi` to
//! `kak`, so the stem of a class has to be worked out from the class rather
//! than assumed. `NOUN_CLASSES` holds the readings, and this module is the
//! arithmetic that realises them.

use crate::kotus::ClassId;

use crate::is_vowel;

/// An edit that turns a lemma into the stem the oblique cases attach to.
///
/// The three parts apply in order: drop that many trailing characters, replace
/// the last occurrence of one character if asked, then add a character. The
/// substitution exists for the genitive-stem consonants that Finnish alternates,
/// so `käsi` gives `käde` and `isoäiti` gives `isoäid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StemRule {
    pub drop: u8,
    pub sub: Option<(char, char)>,
    pub add: Option<&'static str>,
    /// The ending a lemma has to have for this reading to be its own.
    ///
    /// A class can hold words whose stems are cut in different places, and a
    /// reading written for one of them is a wrong answer about the rest: class
    /// 39 is modelled on `vastaus`, whose `ks` stem an `aidas` in the same class
    /// has no part in.
    pub suffix: Option<&'static str>,
}

impl StemRule {
    pub(crate) const fn keep() -> Self {
        Self {
            drop: 0,
            sub: None,
            add: None,
            suffix: None,
        }
    }

    const fn drop(n: u8) -> Self {
        Self {
            drop: n,
            sub: None,
            add: None,
            suffix: None,
        }
    }

    const fn drop_add(n: u8, c: &'static str) -> Self {
        Self {
            drop: n,
            sub: None,
            add: Some(c),
            suffix: None,
        }
    }

    /// `drop_add` for a class whose members are not all cut in the same place,
    /// where the reading belongs to a lemma ending in `tail` alone.
    const fn suffix_drop_add(tail: &'static str, n: u8, c: &'static str) -> Self {
        Self {
            drop: n,
            sub: None,
            add: Some(c),
            suffix: Some(tail),
        }
    }

    const fn sub_add(from: char, to: char, c: &'static str) -> Self {
        Self {
            drop: 0,
            sub: Some((from, to)),
            add: Some(c),
            suffix: None,
        }
    }

    const fn drop_sub_add(n: u8, from: char, to: char, c: &'static str) -> Self {
        Self {
            drop: n,
            sub: Some((from, to)),
            add: Some(c),
            suffix: None,
        }
    }
}

/// A literal stem, for the classes where the stem is a spelling rather than a
/// rule. The lemma is still used for every other part of the word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stem {
    Rule(StemRule),
    Fixed(&'static str),
}

impl Stem {
    /// Whether this reading is the lemma itself rather than a rule for cutting
    /// it down.
    ///
    /// A literal stem is a spelling that stands in for the class, as `käs` is
    /// for `käsi`, and a rule that changes nothing is the lemma. Both are
    /// fallbacks: they say where the word starts, not where the endings go, so
    /// a class that has a real stem rule does not attach the inessive to these.
    pub fn is_literal(self) -> bool {
        match self {
            Stem::Fixed(_) => true,
            Stem::Rule(r) => r.drop == 0 && r.sub.is_none() && r.add.is_none(),
        }
    }
}

/// The nominal description of a Kotus class.
///
/// `stems` holds every stem reading the class admits, in preference order.
#[derive(Debug, Clone, Copy)]
pub struct NounClass {
    /// The stems the inessive takes.
    pub stems: &'static [Stem],
    /// The stems the cases ending in `-lla`, `-lta`, `-lle` and `-na` take.
    ///
    /// These are not the inessive stems. `käsi` is `kässä` by its own stem and
    /// `kädessä` by this one; `joki` is `joessa` by its own and `joella` by
    /// this. Both classes need the two lists apart, and both may disagree with
    /// themselves: class 7 holds `ovi` and `joki`, whose genitive stems are
    /// `ove` and `joe`, so the list carries both readings and the wrong one is
    /// a form of some other word.
    pub genitive: Option<&'static [Stem]>,
    /// The stems the essive takes, for the class whose essive stem is neither
    /// the lemma nor the genitive stem.
    ///
    /// The essive is a strong-grade case, and a class that alternates a
    /// consonant has two stems to choose between, so the genitive's weak one is
    /// the wrong one to build it on: `käsi` is `käden` in the genitive and
    /// `kätenä` in the essive, and there is no reading of the genitive stem that
    /// makes the second. `None` means the class alternates nothing, and the
    /// genitive stem is the strong stem as well.
    pub essive: Option<&'static [Stem]>,
    /// Whether the reading that changes nothing is a stem the inessive takes
    /// as well, rather than the word where its endings start.
    ///
    /// It usually is not: a stem that was left alone is the lemma, and the
    /// lemma with an inessive on it is a word of its own elsewhere, so the
    /// reading is left out and `käsi` does not give the `kässä` that is not a
    /// word. Class 7 is the class where it is, because there the lemma is also
    /// the stem the plural takes: `ovi` is `ovet`, so the plural inessive is
    /// that same `ovi` with an `-ssa` on it, `ovissa`.
    pub lemma_is_stem: bool,
}

/// Every part of a class's entry.
///
/// The classes below are the combinations of these four that the noun classes
/// actually need, and the last two are the ones the constructors cannot tell
/// apart: class 7 is a class whose lemma is a stem and which also names a stem
/// for the essive, and there is no pair of constructors for that.
const fn class_of(
    stems: &'static [Stem],
    genitive: Option<&'static [Stem]>,
    essive: Option<&'static [Stem]>,
    lemma_is_stem: bool,
) -> NounClass {
    NounClass {
        stems,
        genitive,
        essive,
        lemma_is_stem,
    }
}

pub(crate) const fn plain(stems: &'static [Stem]) -> NounClass {
    class_of(stems, None, None, false)
}

pub(crate) const fn thema(stems: &'static [Stem]) -> NounClass {
    class_of(stems, None, None, false)
}

/// A class that names a genitive stem of its own, which class 27 is: its
/// inessive and adessive are on `käde` where the lemma would give `kässä` and
/// `käsellä`, and `essive` is the stem of the one case that wants a third.
///
/// A class that alternates nothing needs no list of its own, and takes the
/// lemma, which is what [`plain`] and [`thema`] are for.
pub(crate) const fn with_essive(
    stems: &'static [Stem],
    genitive: &'static [Stem],
    essive: &'static [Stem],
) -> NounClass {
    class_of(stems, Some(genitive), Some(essive), false)
}

impl NounClass {
    /// Whether the class gives its stem as a spelling rather than as a cut of
    /// the lemma, which is what makes a gradation flag mean nothing to it.
    ///
    /// A flag belongs to the lemma: `lämmin`'s `H` is a fact about the `mm` of
    /// that word. The class's stem is `lämpimä`, a spelling, and the `mp` in it
    /// is a different consonant pair that the flag never described — graduating
    /// it gives the `lämmimä` of `lämmimän`, which is no form of `lämmin` or of
    /// any other word. The plural of `lämmin` is `lämpimät`, on the same stem as
    /// the singular, so there is nothing in this class for the flag to do.
    pub fn fixes_stem(&self) -> bool {
        self.stems.iter().any(|s| matches!(s, Stem::Fixed(_)))
    }
}

pub(crate) const S: &[Stem] = &[Stem::Rule(StemRule::keep())];
const THEME_E: &[Stem] = &[Stem::Rule(StemRule::drop_add(1, "e")), S[0]];
const THEME_A: &[Stem] = &[Stem::Rule(StemRule::drop_add(1, "a")), S[0]];
const OV_OR_JO: &[Stem] = &[
    Stem::Rule(StemRule::drop_add(1, "e")), // ovi -> ove
    Stem::Rule(StemRule::drop_add(2, "e")), // joki -> joe
    Stem::Rule(StemRule::drop(2)),          // joki -> jo, plural basis
    S[0],
];
const KYTKIN: &[Stem] = &[
    Stem::Rule(StemRule::drop_add(1, "me")),
    Stem::Rule(StemRule::drop(2)),
    S[0],
];
/// The comparative `-mpi` stem, which drops the `-i` and adds the theme `a`:
/// `vanhempi` is `vanhempa`, whose `mp` graduates to `mm` in the weak cases.
const COMPARATIVE: &[Stem] = &[Stem::Rule(StemRule::drop_add(1, "a")), S[0]];
/// A stem that adds an `e` to the lemma: `hame` gives `hamee`, `sisar` gives
/// `sisare`.
const ADD_E: &[Stem] = &[Stem::Rule(StemRule::drop_add(0, "e")), S[0]];
const KANSI: &[Stem] = &[Stem::Fixed("käs"), S[0]];
const LAMMIN: &[Stem] = &[Stem::Fixed("lämpimä"), S[0]];
const SISIN: &[Stem] = &[Stem::Fixed("sisimmä"), S[0]];
const VASEN: &[Stem] = &[Stem::Fixed("vasemma"), S[0]];
/// The strong essive stem of `sisin` and `vasen`, which the weak genitive stem
/// cannot make: `sisin` is `sisimmän` and `sisimpänä`, `vasen` is `vasemman`
/// and `vasempana`.
const SISIN_ESSIVE: &[Stem] = &[Stem::Fixed("sisimpä")];
const VASEN_ESSIVE: &[Stem] = &[Stem::Fixed("vasempa")];
const KEVAT: &[Stem] = &[Stem::Fixed("kevää"), S[0]];
const TUHAT: &[Stem] = &[Stem::Fixed("tuhann"), S[0]];
/// The strong essive stem of `tuhat`, which the weak `tuhann` cannot make:
/// `tuhat` is `tuhannen` and `tuhannessa`, but `tuhantena` in the essive.
const TUHAT_STE: &[Stem] = &[Stem::Fixed("tuhante")];
/// The privative `-ton` stem, fixed because the `-tt-` of `onneton` does not
/// graduate: `onnettoman` keeps its `tt` in every case.
const ONNETON: &[Stem] = &[Stem::Fixed("onnettoma"), S[0]];
const MIES: &[Stem] = &[Stem::Rule(StemRule::sub_add('s', 'h', "e")), S[0]];
const ISOAITI: &[Stem] = &[Stem::Rule(StemRule::drop_sub_add(1, 't', 'd', "i")), S[0]];
const ASKEL: &[Stem] = &[
    Stem::Rule(StemRule::drop_add(1, "lee")),
    Stem::Rule(StemRule::drop_add(1, "le")),
    S[0],
];
/// The `-ut`/`-yt` participle class, whose stem doubles the theme `e`:
/// `kuollut` is `kuollee`, `ollut` is `ollee`.
const PART_UT: &[Stem] = &[Stem::Rule(StemRule::drop_add(2, "ee")), S[0]];
const NAINEN: &[Stem] = &[Stem::Rule(StemRule::drop_sub_add(2, 'n', 's', "e")), S[0]];
// The `-us`/`-ys` class 40, whose stem shows a `d` in the genitive and weak
// cases (`kalleuden`, `kalleudessa`; `ystävyyden`, `ystävyydessä`), a `t` in
// the essive (`kalleutena`, `ystävyytenä`), and an `ks` in the plural
// (`kalleuksia`, already reached from the catalogue).
const KALLEUS: &[Stem] = &[Stem::Rule(StemRule::drop_add(1, "de")), S[0]];
const KALLEUS_GEN: &[Stem] = &[Stem::Rule(StemRule::drop_add(1, "de"))];
const KALLEUS_STE: &[Stem] = &[Stem::Rule(StemRule::drop_add(1, "te"))];
// The ordinal `-s` class 45, whose stem is `kahdeksanne` in the genitive and
// weak cases and `kahdeksante` in the essive.
const KAHDEKSAS: &[Stem] = &[Stem::Rule(StemRule::drop_add(2, "anne")), S[0]];
const KAHDEKSAS_GEN: &[Stem] = &[Stem::Rule(StemRule::drop_add(2, "anne"))];
const KAHDEKSAS_STE: &[Stem] = &[Stem::Rule(StemRule::drop_add(2, "ante"))];
const KYNSI: &[Stem] = &[Stem::Rule(StemRule::drop_sub_add(1, 's', 'n', "e")), S[0]];
// The strong grade of class 7, which is what the genitive of `ovi` and `joki`
// is as well as what the essive is: `oven` and `joen` give `ove` and `joe`, and
// `ovena` and `jokena` are on the same two. The `e` is the class's own long
// grade, and it is the only reading of the lemma that has it, so it cannot be
// read off the `k` that gradates away in `joki`.
const OVEN: &[Stem] = &[Stem::Rule(StemRule::drop_add(1, "e"))];
// The genitive readings of class 7: `ove` and `joe`.
const GENITIVE_OV_OR_JO: &[Stem] = &[OVEN[0], Stem::Rule(StemRule::drop_add(2, "e"))];
const GENITIVE_KANSI: &[Stem] = &[Stem::Rule(StemRule::drop_sub_add(1, 's', 'd', "e"))];
// The numeral `kaksi`, whose stem is `kahde` in the genitive and weak cases
// and `kahte` in the essive, the same d/t alternation as `käsi`.
const KAKSI: &[Stem] = &[Stem::Fixed("kah"), S[0]];
const GENITIVE_KAKSI: &[Stem] = &[Stem::Rule(StemRule::drop_add(3, "hde"))];
const KAKSI_STE: &[Stem] = &[Stem::Rule(StemRule::drop_add(3, "hte"))];
// The essive stem of classes 27 and 28, which the genitive one cannot make:
// `käsi` is `käden` in the genitive and `kätenä` in the essive, and `kynsi` is
// `kynnen` and `kyntenä`, so the `d` the genitive substitutes and the `n` it
// does not are a `t` here. The inessive and the adessive keep the weak grade
// (`kädessä`, `kädellä`, `kynnessä`, `kynnellä`) and only the essive, which is
// a strong-grade case, takes this one. The strong grade is steady across class
// 28 where the genitive grade is not: Wiktionary tabulates `kyntenä`,
// `kantena`, `kortena` and `hirtenä` for its class-28 words, and the `kansina`
// that would have disagreed is the plural of `kansi`, which gets its essive
// from the plural stem and not from this one.
const STE: &[Stem] = &[Stem::Rule(StemRule::drop_sub_add(1, 's', 't', "e"))];
// The `-us` and `-ys` nouns: the `s` the lemma ends in is not in the stem the
// oblique cases attach to. `vastaus` is `vastauksen`, and `vastauksessa` and
// `vastauksella` hang off `vastaukse`. The reading is written for those two
// endings because class 39 holds more than them: an `aidas` and a `kymmenes` are
// in it too, and neither has a `ks` stem, so for them the lemma stays the stem.
// The same holds for the `-nas` nouns of the class: `ananas` is `ananaksen`,
// and its oblique stem is `ananakse`, so `ananaksessa` and `ananaksella` hang
// off that and not off the lemma with its `s` on it. A bare `-as` noun such
// as `aidas` keeps the lemma as its stem.
const US: &[Stem] = &[
    Stem::Rule(StemRule::suffix_drop_add("us", 1, "kse")),
    Stem::Rule(StemRule::suffix_drop_add("ys", 1, "kse")),
    Stem::Rule(StemRule::suffix_drop_add("nas", 3, "nakse")),
    Stem::Rule(StemRule::suffix_drop_add("näs", 3, "näkse")),
    S[0],
];

/// The stem readings of the nominal paradigms, keyed by Kotus class number.
///
/// Each row was read off the class model in the catalogue, and the classes
/// whose members disagree about the stem end carry the disagreeing readings
/// side by side: class 7 has `ove` for `ovi` and `jo` for `joki`, so it lists
/// both `drop_add(1, "e")` and `drop(2)`.
///
/// Classes absent from this table attach their endings straight to the lemma,
/// which is what most of them do.
const NOUN_CLASSES: &[(ClassId, NounClass)] = &[
    (1, plain(S)), // valo     valossa   valolla
    (2, plain(S)), // palvelu  palvelussa palvelulla
    (3, plain(S)), // valtio   valtiossa
    (4, plain(S)), // laatikko laatikossa
    (5, plain(S)), // risti    ristissä
    (6, plain(S)), // paperi   paperissa
    // `ovi` is `ovessa` and `joella`, `joki` is `joessa` and `joella`: the
    // adessive wants the genitive stem and the inessive the short one. The
    // lemma is a stem here as well, since the plural of `ovi` is `ovet` and so
    // `ovissa` hangs off `ovi` itself.
    (
        7,
        class_of(OV_OR_JO, Some(GENITIVE_OV_OR_JO), Some(OVEN), true),
    ), // ovi -> ove, joki -> joke
    (8, plain(S)),            // nalle    nallessa
    (9, plain(S)),            // kala     kalassa  kalalla
    (10, plain(S)),           // koira    koirassa
    (11, plain(S)),           // omena    omenassa
    (12, plain(S)),           // kulkija  kulkija(ssa)
    (13, plain(S)),           // katiska  katiskassa
    (14, plain(S)),           // solakka  solakassa
    (15, plain(S)),           // korkea   korkeassa
    (16, thema(COMPARATIVE)), // vanhempi vanhempa
    (17, plain(S)),           // vapaa    vapaassa
    (18, plain(S)),           // maa      maassa
    (19, plain(S)),           // suo      suossa
    (20, plain(S)),           // filee    fileessä
    (21, plain(S)),           // rose     rosessä
    (23, thema(THEME_E)),     // tiili tiile
    (24, thema(THEME_E)),     // uni une
    (25, thema(THEME_E)),     // toimi toime
    (26, thema(THEME_E)),     // pieni piene
    // `käsi` is `kädessä` and not `kässä`, so the inessive is on the genitive
    // stem here, unlike in class 7. Its essive is on a third stem again, since
    // the essive is a strong case and the genitive stem is the weak one.
    (27, with_essive(KANSI, GENITIVE_KANSI, STE)), //        käsi: kädellä, kätenä
    (28, class_of(KYNSI, None, Some(STE), false)), // kynsi kynne, kyntenä
    (29, thema(THEME_E)),                          // lapsi lapse
    (30, thema(THEME_E)),                          // veitsi veitse
    (31, with_essive(KAKSI, GENITIVE_KAKSI, KAKSI_STE)), // kaksi kahde
    (32, thema(ADD_E)),                            // sisar sisare
    (33, thema(KYTKIN)),                           // kytkin kytkime
    (34, plain(ONNETON)),                          // onneton onnettomassa
    (35, plain(LAMMIN)),                           // lämmin lämpimässä
    (36, with_essive(SISIN, SISIN, SISIN_ESSIVE)), // sisin  sisimmässä, sisimpänä
    (37, with_essive(VASEN, VASEN, VASEN_ESSIVE)), // vasen  vasemmassa, vasempana
    (38, thema(NAINEN)),                           // nainen naise
    // The `-us` and `-ys` nouns of this class take their oblique endings off the
    // stem the genitive shows: `vastaus` is `vastauksen`, and `vastauksessa` and
    // `vastauksella` hang off `vastaukse`, not off the lemma with its `s` on it.
    // The class's other members (`aidas`, `kymmenes`, `vihannes`) keep the lemma
    // as their stem, and it is `US` that says which of the two a lemma is.
    (39, plain(US)), // vastaus  vastauksessa vastauksella
    // Class 40 alternates three ways at once and has none of them written down:
    // `kalleus` is `kalleudessa` by a `ks` stem, `kalleudella` by a `d` one and
    // `kalleutena` by a `t` one. One stem will not do and a partial answer would
    // offer a wrong form as a correction, so the class is left reading off the
    // lemma until all three are described.
    (40, with_essive(KALLEUS, KALLEUS_GEN, KALLEUS_STE)), // kalleus kalleude, kalleutena
    (41, plain(THEME_A)),                                 // vieras vieraa
    (42, plain(MIES)),                                    // mies miehe
    (43, thema(THEME_E)),                                 // ohut ohue
    (44, plain(KEVAT)),                                   // kevät keväällä
    (45, with_essive(KAHDEKSAS, KAHDEKSAS_GEN, KAHDEKSAS_STE)), // kahdeksas kahdeksanne
    (46, with_essive(TUHAT, TUHAT, TUHAT_STE)),           // tuhat tuhannen, tuhantena
    (47, thema(PART_UT)),                                 // kuollut kuolleessa
    (48, thema(ADD_E)),                                   // hame hameessa
    (49, thema(ASKEL)),                                   // askel askele, askelessa
    (50, thema(ISOAITI)),                                 // isoäiti isoäid
];

/// The tail a class substitutes in its stem, where reading it off the
/// model and genitive pair is not enough.
///
/// The substitution is a whole tail, not one letter, because that is how long
/// vowels and consonant gradation interact: `lapsi` gives `lasta` by swapping
/// `ps` for `st` and `veitsi` gives `veistä` by swapping `ts` for `st`. The
/// genitive of the same class, `lapsen`, is what decides it does not happen in
/// the `-n` case, and the caller keeps the table off the singular `-n` slots for
/// that reason. The third class, `käsi`, needs `käd-` in the genitive but gets
/// it from its fixed stem.
pub const STEM_ALTERNATIONS: &[(ClassId, &str, &str)] = &[(29, "ps", "st"), (30, "ts", "st")];

/// The stem alternation of `class`, if the class makes one.
pub fn stem_alternation(class: ClassId) -> Option<(&'static str, &'static str)> {
    STEM_ALTERNATIONS
        .iter()
        .find(|(c, _, _)| *c == class)
        .map(|(_, from, to)| (*from, *to))
}

/// The literal stem of a class, where one is fixed.
///
/// A class whose lemma is just one of two spellings: `käsi` is `27` and
/// the second spelling is `käs`; the class's inflections are built off
/// both. A `None` means the class has no fixed companion.
#[inline]
pub fn fixed_stem(class: ClassId) -> Option<&'static str> {
    noun_class(class)?.stems.iter().find_map(|s| match s {
        Stem::Fixed(f) => Some(*f),
        Stem::Rule(_) => None,
    })
}

/// The nominal class description of `class`, if the table has one.
pub fn noun_class(class: ClassId) -> Option<NounClass> {
    NOUN_CLASSES
        .iter()
        .find(|(c, _)| *c == class)
        .map(|(_, d)| *d)
}

/// Applies a [`StemRule`] to a lemma.
pub fn apply_stem(rule: StemRule, lemma: &str) -> String {
    let end = lemma
        .char_indices()
        .nth(lemma.chars().count().saturating_sub(rule.drop as usize))
        .map_or(lemma.len(), |(i, _)| i);
    let mut s: String = lemma[..end].to_string();
    if let Some((from, to)) = rule.sub {
        // The last occurrence, not the first: the alternation is always in the
        // final syllable, so `nainen` gives `naise` and not `saine`.
        if let Some(i) = s.rfind(from) {
            s.replace_range(i..i + from.len_utf8(), &to.to_string());
        }
    }
    if let Some(c) = rule.add {
        s.push_str(c);
    }
    s
}

/// Applies a [`Stem`] to a lemma.
pub fn apply_stem_of(stem: Stem, lemma: &str) -> String {
    match stem {
        Stem::Rule(r) => apply_stem(r, lemma),
        Stem::Fixed(s) => s.to_string(),
    }
}

/// The stem with a theme vowel the class added taken back off, which is what
/// harmony has to be read from.
///
/// A theme vowel is never a stressed one, so it cannot decide which side of the
/// vowel pair the word belongs to. `lapsi` has the stem `laps` and the class
/// adds the `e` that makes `lapse`; reading harmony off that `e` gives the
/// front and `lapsessä`, where the word is `lapsessa` and the `a` of `la-`
/// decides. The same goes for `toimi`, where the `e` of `toime` would give
/// `toimessä` instead of `toimessa`.
///
/// A vowel the class adds on top of a consonant can be a different matter and
/// decide: `vieras` is `vieraa` in the partitive, and that `a` is stressed,
/// which is why `vieras` harmonises to the back. The two are told apart by how
/// much the class added: an ending is a single vowel, while a stem the class
/// shaped carries a consonant before its theme vowel, as `vastaukse` does.
pub fn harmony_stem_of(stem: Stem, lemma: &str) -> String {
    let applied = apply_stem_of(stem, lemma);
    let Stem::Rule(rule) = stem else {
        return applied;
    };
    let Some(tail) = rule.add else {
        return applied;
    };
    // A single vowel the class added that itself decides the harmony is the
    // reading's own: `vieras` is `vieraa` in the partitive, and that `a`
    // belongs to the word and decides it back. A neutral `e` decides nothing,
    // so it comes off and the word's own vowel is read: `sisar` is `sisare`
    // in the oblique cases, and its `a` is what makes it a back word.
    let decides = tail.chars().count() == 1
        && "aäoö".contains(tail)
        && !lemma.chars().last().is_some_and(is_vowel);
    if decides {
        return applied;
    }
    match applied.strip_suffix(tail) {
        Some(bare) if !bare.is_empty() => bare.to_string(),
        _ => applied,
    }
}

/// Halves a doubled consonant near the end of a stem.
///
/// The gradation sits before the theme vowel, not at the end of the word:
/// `laatikko` gives `laatiko` and `kukka` gives `kuka`.
#[allow(dead_code)]
fn halve_doubled(stem: &str) -> String {
    let chars: Vec<(usize, char)> = stem.char_indices().collect();
    if chars.len() < 3 {
        return stem.to_string();
    }
    for w in 1..=2 {
        let hi = chars.len() - w;
        let lo = hi - 1;
        if chars[lo].1 == chars[hi].1 && is_consonant(chars[hi].1) {
            return stem[..chars[lo].0].to_string() + &stem[chars[hi].0..];
        }
    }
    stem.to_string()
}

/// Whether a character counts as a consonant here, which is anything lettered
/// that is not one of the Finnish vowels.
fn is_consonant(c: char) -> bool {
    c.is_alphabetic()
        && !matches!(
            c,
            'a' | 'e'
                | 'i'
                | 'o'
                | 'u'
                | 'y'
                | 'ä'
                | 'ö'
                | 'å'
                | 'A'
                | 'E'
                | 'I'
                | 'O'
                | 'U'
                | 'Y'
                | 'Ä'
                | 'Ö'
                | 'Å'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stem_rule_drops_swaps_and_adds_in_that_order() {
        // `kynsi` -> `kynne`: the final `i` comes off, then `s` becomes `n`,
        // and only then is the `e` the -een cases want put back. The order
        // matters, because the `e` has to go on after the substitution.
        let r = StemRule::drop_sub_add(1, 's', 'n', "e");
        assert_eq!(apply_stem(r, "kynsi"), "kynne");
        // `kansi` -> `kande`, the same shape with a `d`.
        let r = StemRule::drop_sub_add(1, 's', 'd', "e");
        assert_eq!(apply_stem(r, "kansi"), "kande");
    }

    #[test]
    fn a_substitution_takes_the_last_occurrence() {
        let r = StemRule::drop_sub_add(2, 'n', 's', "e");
        assert_eq!(apply_stem(r, "nainen"), "naise");
    }

    #[test]
    fn a_fixed_stem_ignores_the_lemma() {
        assert_eq!(apply_stem_of(Stem::Fixed("käs"), "käsi"), "käs");
    }

    #[test]
    fn the_fixed_stem_is_found_from_the_class() {
        assert_eq!(fixed_stem(27), Some("käs"));
        assert_eq!(fixed_stem(35), Some("lämpimä"));
        // A class whose stem is a rule has no fixed spelling to offer.
        assert_eq!(fixed_stem(1), None);
    }

    #[test]
    fn stem_alternations_are_tabled_per_class() {
        assert_eq!(stem_alternation(29), Some(("ps", "st")));
        assert_eq!(stem_alternation(30), Some(("ts", "st")));
        assert_eq!(stem_alternation(1), None);
    }

    #[test]
    fn class_7_has_distinct_ovi_and_joki_stems() {
        let class = noun_class(7).unwrap();
        // `ovi` is `ove` and `joki` is `joe` in the weak singular.
        assert_eq!(apply_stem_of(class.stems[0], "ovi"), "ove");
        assert_eq!(apply_stem_of(class.stems[1], "joki"), "joe");
        assert_eq!(apply_stem_of(class.genitive.unwrap()[0], "ovi"), "ove");
        assert_eq!(apply_stem_of(class.genitive.unwrap()[1], "joki"), "joe");
    }

    #[test]
    fn class_40_has_weak_and_strong_singular_stems() {
        let class = noun_class(40).unwrap();
        // `kalleus` is `kalleude` in the genitive and weak cases and
        // `kalleute` in the essive; the same `-ys` ending gives `ystävyyde`
        // and `ystävyyte`.
        assert_eq!(apply_stem_of(class.stems[0], "kalleus"), "kalleude");
        assert_eq!(
            apply_stem_of(class.genitive.unwrap()[0], "kalleus"),
            "kalleude"
        );
        assert_eq!(
            apply_stem_of(class.essive.unwrap()[0], "kalleus"),
            "kalleute"
        );
        assert_eq!(apply_stem_of(class.stems[0], "ystävyys"), "ystävyyde");
        assert_eq!(
            apply_stem_of(class.essive.unwrap()[0], "ystävyys"),
            "ystävyyte"
        );
    }

    #[test]
    fn a_doubled_consonant_halves_before_the_theme_vowel() {
        assert_eq!(halve_doubled("laatikko"), "laatiko");
        assert_eq!(halve_doubled("kukka"), "kuka");
        // Nothing to halve in a stem that has no doubled consonant.
        assert_eq!(halve_doubled("kala"), "kala");
    }
}
