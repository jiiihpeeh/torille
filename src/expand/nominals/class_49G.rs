//! Inflection rule for class 49*G, generated from the noun lexicon.
//! Regenerate with:  python3 tools/build/extract_class_rules.py --rust
//! do not edit by hand; the database is the source of truth.
#![allow(unused_imports)]

use crate::tags::{Case, Confidence, Form, Number, Possessive, Tags};

pub const CLASS: &str = "49*G";
pub const MODEL: &str = "penger";

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
    push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "ittä"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "ittämme"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "ittäni"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "ittänne"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "ittäsi"));
    push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereittänsä"), ""));
    push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "että"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "ettämme"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "ettäni"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "ettänne"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "ettäsi"));
    push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereettänsä"), ""));
    push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "iltä"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "iltämme"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "iltäni"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "iltänne"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "iltäsi"));
    push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereiltänsä"), ""));
    push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "eltä"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "eltämme"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "eltäni"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "eltänne"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "eltäsi"));
    push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereeltänsä"), ""));
    push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "illä"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "illämme"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "illäni"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "illänne"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "illäsi"));
    push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereillänsä"), ""));
    push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "ellä"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "ellämme"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "elläni"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "ellänne"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "elläsi"));
    push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereellänsä"), ""));
    push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "ille"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "illemme"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "illeni"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "illenne"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "illesi"));
    push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereilleen"), ""));
    push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "elle"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "ellemme"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "elleni"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "ellenne"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "ellesi"));
    push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereelleen"), ""));
    push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "istä"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "istämme"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "istäni"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "istänne"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "istäsi"));
    push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereistänsä"), ""));
    push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "estä"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "estämme"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "estäni"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "estänne"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "estäsi"));
    push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereestänsä"), ""));
    push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "inä"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "inämme"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "inäni"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "inänne"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "inäsi"));
    push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereinänsä"), ""));
    push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "enä"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "enämme"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "enäni"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "enänne"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "enäsi"));
    push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereenänsä"), ""));
    push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 0, "t"), "en"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, "t"), "emme"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, "t"), "eni"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, "t"), "enne"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, "t"), "esi"));
    push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "t"), "ensä"));
    push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "en"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "emme"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "eni"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "enne"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "esi"));
    push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kere"), "ensä"));
    push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kereih"), "in"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kereih"), "imme"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kereih"), "ini"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kereih"), "inne"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kereih"), "isi"));
    push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereih"), "insä"));
    push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "ker"), "een"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "ker"), "eemme"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "ker"), "eeni"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "ker"), "eenne"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kerees"), "eesi"));
    push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "ker"), "eensä"));
    push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "issä"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "issämme"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "issäni"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "issänne"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "issäsi"));
    push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereissänsä"), ""));
    push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "essä"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "essämme"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "essäni"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "essänne"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "essäsi"));
    push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereessänsä"), ""));
    push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "in"));
    push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "et"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "emme"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "eni"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "enne"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "esi"));
    push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kere"), "ensä"));
    push(Case::Nominative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), ""));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "emme"));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "eni"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "enne"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "esi"));
    push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kere"), "ensä"));
    push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kereit"), "ä"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kereit"), "ämme"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kereit"), "äni"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kereit"), "änne"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kereit"), "äsi"));
    push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereitänsä"), ""));
    push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "tänsä"), ""));
    push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "kere"), "iksi"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "iksemme"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "ikseni"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "iksenne"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "iksesi"));
    push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereikseen"), ""));
    push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "kere"), "eksi"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "kere"), "eksemme"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "kere"), "ekseni"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "kere"), "eksenne"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "kere"), "eksesi"));
    push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "kereekseen"), ""));
    drop(stem);
    drop(double);
    drop(front);
    drop(harmonize);
    drop(push);
    out
}
