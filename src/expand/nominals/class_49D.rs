//! Inflection rule for class 49*D, generated from the noun lexicon.
//! Regenerate with:  python3 tools/build/extract_class_rules.py --rust
//! do not edit by hand; the database is the source of truth.
#![allow(unused_imports)]

use crate::tags::{Case, Confidence, Form, Number, Possessive, Tags};

pub const CLASS: &str = "49*D";
pub const MODEL: &str = "säen";

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
    push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "ittä"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "ittämme"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "ittäni"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "ittänne"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "ittäsi"));
    push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneittänsä"), ""));
    push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "että"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "ettämme"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "ettäni"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "ettänne"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "ettäsi"));
    push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneettänsä"), ""));
    push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "iltä"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "iltämme"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "iltäni"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "iltänne"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "iltäsi"));
    push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneiltänsä"), ""));
    push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "eltä"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "eltämme"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "eltäni"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "eltänne"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "eltäsi"));
    push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneeltänsä"), ""));
    push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "illä"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "illämme"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "illäni"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "illänne"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "illäsi"));
    push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneillänsä"), ""));
    push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "ellä"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "ellämme"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "elläni"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "ellänne"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "elläsi"));
    push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneellänsä"), ""));
    push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "ille"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "illemme"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "illeni"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "illenne"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "illesi"));
    push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneilleen"), ""));
    push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "elle"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "ellemme"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "elleni"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "ellenne"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "ellesi"));
    push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneelleen"), ""));
    push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "istä"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "istämme"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "istäni"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "istänne"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "istäsi"));
    push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneistänsä"), ""));
    push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "estä"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "estämme"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "estäni"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "estänne"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "estäsi"));
    push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneestänsä"), ""));
    push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "inä"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "inämme"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "inäni"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "inänne"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "inäsi"));
    push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneinänsä"), ""));
    push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "enä"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "enämme"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "enäni"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "enänne"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "enäsi"));
    push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneenänsä"), ""));
    push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 0, "t"), "en"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, "t"), "emme"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, "t"), "eni"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, "t"), "enne"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, "t"), "esi"));
    push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "t"), "ensä"));
    push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "en"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "emme"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "eni"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "enne"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "esi"));
    push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "kene"), "ensä"));
    push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "keneih"), "in"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "keneih"), "imme"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "keneih"), "ini"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "keneih"), "inne"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "keneih"), "isi"));
    push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneih"), "insä"));
    push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ken"), "een"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ken"), "eemme"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ken"), "eeni"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ken"), "eenne"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kenees"), "eesi"));
    push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "ken"), "eensä"));
    push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "issä"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "issämme"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "issäni"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "issänne"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "issäsi"));
    push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneissänsä"), ""));
    push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "essä"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "essämme"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "essäni"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "essänne"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "essäsi"));
    push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneessänsä"), ""));
    push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "in"));
    push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "et"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "emme"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "eni"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "enne"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "esi"));
    push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "kene"), "ensä"));
    push(Case::Nominative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), ""));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "emme"));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "eni"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "enne"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "esi"));
    push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "kene"), "ensä"));
    push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "keneit"), "ä"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "keneit"), "ämme"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "keneit"), "äni"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "keneit"), "änne"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "keneit"), "äsi"));
    push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneitänsä"), ""));
    push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "tänsä"), ""));
    push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "kene"), "iksi"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "iksemme"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "ikseni"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "iksenne"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "iksesi"));
    push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneikseen"), ""));
    push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "kene"), "eksi"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "kene"), "eksemme"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "kene"), "ekseni"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "kene"), "eksenne"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "kene"), "eksesi"));
    push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "keneekseen"), ""));
    drop(stem);
    drop(double);
    drop(front);
    drop(harmonize);
    drop(push);
    out
}
