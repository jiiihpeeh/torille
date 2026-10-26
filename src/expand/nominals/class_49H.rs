//! Inflection rule for class 49*H, generated from the noun lexicon.
//! Regenerate with:  python3 tools/extract_class_rules.py --rust
//! do not edit by hand; the database is the source of truth.
#![allow(unused_imports)]

use crate::tags::{Case, Confidence, Form, Number, Possessive, Tags};

pub const CLASS: &str = "49*H";
pub const MODEL: &str = "ommel";

/// Every form of `lemma`, one `Form` per confirmed cell.
///
/// `possessives` is the caller's decision, taken from the part of
/// speech: a noun takes possessive suffixes, an adjective or a
/// participle does not. The cells below are read off the database, which
/// records both, so the flag is what keeps an adjective from being
/// offered a possessive column it cannot have.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut out = Vec::new();
    // `stem` cuts the class's stem out of the lemma: `(drop, add)`
    // makes `lemma[:len-drop] + add`. A non-graded class is `(0, "")`,
    // so `stem` is the lemma itself; class 27 is `käsi -> käde`.
    let stem = |lemma: &str, drop: usize, add: &str| -> String {
        let chars: Vec<char> = lemma.chars().collect();
        let keep = chars.len().saturating_sub(drop);
        let mut s: String = chars[..keep].iter().collect();
        s.push_str(add);
        s
    };
    // `double` appends the lemma's last vowel: `äly -> älyy`, the
    // illative (and a few others) doubling their stem's final vowel.
    let double = |lemma: &str| -> String {
        let mut s = lemma.to_string();
        if let Some(v) = lemma.chars().last() {
            s.push(v);
        }
        s
    };
    // Harmony: a cell of a class holds both the back (`a`/`o`) and the
    // front (`ä`/`ö`) spelling of its ending; the lemma's own vowels
    // pick the one. A word with a back vowel is back, one with only
    // `e`/`i` or a front vowel is front, matching the engine's reading.
    let front = |lemma: &str| -> bool {
        // Finnish harmony is set by the *last* non-neutral vowel --
        // `a/ä`, `o/ö`, `u/y` -- scanning right to left; `e`/`i` are
        // neutral and pass. This is the real rule, and it settles the
        // compound on its last constituent by itself: `hätähousu` is
        // back on its `u` though its modifier is front.
        for c in lemma.chars().rev() {
            match c {
                'ä'|'ö'|'y'|'Ä'|'Ö'|'Y' => return true,
                'a'|'o'|'u'|'A'|'O'|'U' => return false,
                _ => {}
            }
        }
        true   // only neutral vowels; read front, as convention does
    };
    // `harmonize` fixes a spelling to the front or back half: `ssani`
    // in the back half is `ssäni` in the front. A and O have front
    // twins (Ä, Ö); a spelling already on the right half is unchanged.
    let harmonize = |s: &str, front: bool| -> String {
        if !front { return s.to_string(); }
        s.chars().map(|c| match c { 'a' => 'ä', 'o' => 'ö',
                                         'A' => 'Ä', 'O' => 'Ö', _ => c }).collect()
    };
    let mut push = |case: Case, num: Number, poss: Option<Possessive>, word: String| {
        // Every confirmed cell is pushed through here, so this is the one
        // place the flag can be honoured: a bare form is always offered,
        // and an owned one only when the word can take the suffix.
        if poss.is_some() && !possessives {
            return;
        }
        let tags = match poss {
            Some(p) => Tags::of_number(case, num).possessed_by(p),
            None => Tags::of_number(case, num),
        };
        out.push(Form::new(word, tags).with(Confidence::Plain));
    };
    push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("itta", front(lemma))));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ittamme", front(lemma))));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ittani", front(lemma))));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ittanne", front(lemma))));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ittasi", front(lemma))));
    push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleittaan", front(lemma))), ""));
    push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("etta", front(lemma))));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ettamme", front(lemma))));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ettani", front(lemma))));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ettanne", front(lemma))));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ettasi", front(lemma))));
    push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleettaan", front(lemma))), ""));
    push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ilta", front(lemma))));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("iltamme", front(lemma))));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("iltani", front(lemma))));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("iltanne", front(lemma))));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("iltasi", front(lemma))));
    push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleiltaan", front(lemma))), ""));
    push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("elta", front(lemma))));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("eltamme", front(lemma))));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("eltani", front(lemma))));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("eltanne", front(lemma))));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("eltasi", front(lemma))));
    push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleeltaan", front(lemma))), ""));
    push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("illa", front(lemma))));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("illamme", front(lemma))));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("illani", front(lemma))));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("illanne", front(lemma))));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("illasi", front(lemma))));
    push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleillaan", front(lemma))), ""));
    push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ella", front(lemma))));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ellamme", front(lemma))));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ellani", front(lemma))));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ellanne", front(lemma))));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ellasi", front(lemma))));
    push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleellaan", front(lemma))), ""));
    push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), "ille"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), "illemme"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), "illeni"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), "illenne"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), "illesi"));
    push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "peleilleen"), ""));
    push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), "elle"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), "ellemme"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), "elleni"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), "ellenne"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), "ellesi"));
    push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "peleelleen"), ""));
    push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ista", front(lemma))));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("istamme", front(lemma))));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("istani", front(lemma))));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("istanne", front(lemma))));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("istasi", front(lemma))));
    push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleistaan", front(lemma))), ""));
    push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("esta", front(lemma))));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("estamme", front(lemma))));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("estani", front(lemma))));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("estanne", front(lemma))));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("estasi", front(lemma))));
    push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleestaan", front(lemma))), ""));
    push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ina", front(lemma))));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("inamme", front(lemma))));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("inani", front(lemma))));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("inanne", front(lemma))));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("inasi", front(lemma))));
    push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleinaan", front(lemma))), ""));
    push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ena", front(lemma))));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("enamme", front(lemma))));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("enani", front(lemma))));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("enanne", front(lemma))));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("enasi", front(lemma))));
    push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleenaan", front(lemma))), ""));
    push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 0, "t"), "en"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, "t"), "emme"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, "t"), "eni"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, "t"), "enne"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, "t"), "esi"));
    push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "t"), &harmonize("ensa", front(lemma))));
    push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), "en"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), "emme"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), "eni"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), "enne"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), "esi"));
    push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ensa", front(lemma))));
    push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "peleih"), "in"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "peleih"), "imme"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "peleih"), "ini"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "peleih"), "inne"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "peleih"), "isi"));
    push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "peleih"), &harmonize("insa", front(lemma))));
    push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pel"), "een"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pel"), "eemme"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pel"), "eeni"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pel"), "eenne"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pelees"), "eesi"));
    push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "pel"), &harmonize("eensa", front(lemma))));
    push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("issa", front(lemma))));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("issamme", front(lemma))));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("issani", front(lemma))));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("issanne", front(lemma))));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("issasi", front(lemma))));
    push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleissaan", front(lemma))), ""));
    push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), &harmonize("essa", front(lemma))));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("essamme", front(lemma))));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("essani", front(lemma))));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("essanne", front(lemma))));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("essasi", front(lemma))));
    push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleessaan", front(lemma))), ""));
    push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), "in"));
    push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), "et"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), "emme"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), "eni"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), "enne"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), "esi"));
    push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ensa", front(lemma))));
    push(Case::Nominative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), ""));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), "emme"));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), "eni"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), "enne"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), "esi"));
    push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "pele"), &harmonize("ensa", front(lemma))));
    push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "peleit"), &harmonize("a", front(lemma))));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "peleit"), &harmonize("amme", front(lemma))));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "peleit"), &harmonize("ani", front(lemma))));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "peleit"), &harmonize("anne", front(lemma))));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "peleit"), &harmonize("asi", front(lemma))));
    push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("peleitaan", front(lemma))), ""));
    push(Case::Partitive, Number::Singular, None, format!("{}{}", stem(lemma, 4, ""), &harmonize("mmelta", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, ""), &harmonize("mmeltamme", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, ""), &harmonize("mmeltani", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, ""), &harmonize("mmeltanne", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, ""), &harmonize("mmeltasi", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("taan", front(lemma))), ""));
    push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "pele"), "iksi"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), "iksemme"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), "ikseni"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), "iksenne"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), "iksesi"));
    push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "peleikseen"), ""));
    push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "pele"), "eksi"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "pele"), "eksemme"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "pele"), "ekseni"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "pele"), "eksenne"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "pele"), "eksesi"));
    push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "peleekseen"), ""));
    drop(stem);
    drop(double);
    drop(front);
    drop(harmonize);
    drop(push);
    out
}
