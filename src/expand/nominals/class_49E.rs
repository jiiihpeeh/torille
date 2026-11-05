//! Inflection rule for class 49*E, generated from the noun lexicon.
//! Regenerate with:  python3 tools/oracle/extract_class_rules.py --rust
//! do not edit by hand; the database is the source of truth.
#![allow(unused_imports)]

use crate::tags::{Case, Confidence, Form, Number, Possessive, Tags};

pub const CLASS: &str = "49*E";
pub const MODEL: &str = "taival";

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
    push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "itta"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ittamme"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ittani"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ittanne"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ittasi"));
    push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleittaan", front(lemma))), ""));
    push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "etta"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ettamme"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ettani"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ettanne"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ettasi"));
    push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleettaan", front(lemma))), ""));
    push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ilta"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iltamme"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iltani"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iltanne"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iltasi"));
    push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleiltaan", front(lemma))), ""));
    push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "elta"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eltamme"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eltani"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eltanne"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eltasi"));
    push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleeltaan", front(lemma))), ""));
    push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illa"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illamme"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illani"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illanne"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illasi"));
    push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleillaan", front(lemma))), ""));
    push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ella"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ellamme"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ellani"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ellanne"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ellasi"));
    push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleellaan", front(lemma))), ""));
    push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ille"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illemme"));
    push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illeni"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illenne"));
    push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "illesi"));
    push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleilleen", front(lemma))), ""));
    push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "elle"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ellemme"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "elleni"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ellenne"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ellesi"));
    push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleelleen", front(lemma))), ""));
    push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ista"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "istamme"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "istani"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "istanne"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "istasi"));
    push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleistaan", front(lemma))), ""));
    push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "esta"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "estamme"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "estani"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "estanne"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "estasi"));
    push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleestaan", front(lemma))), ""));
    push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ina"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "inamme"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "inani"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "inanne"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "inasi"));
    push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleinaan", front(lemma))), ""));
    push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ena"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "enamme"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "enani"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "enanne"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "enasi"));
    push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleenaan", front(lemma))), ""));
    push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("paleid", front(lemma))), "en"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("paleid", front(lemma))), "emme"));
    push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("paleid", front(lemma))), "eni"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("paleid", front(lemma))), "enne"));
    push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("paleid", front(lemma))), "esi"));
    push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleid", front(lemma))), "ensa"));
    push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "en"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "emme"));
    push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eni"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "enne"));
    push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "esi"));
    push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ensa"));
    push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("paleih", front(lemma))), "in"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("paleih", front(lemma))), "imme"));
    push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("paleih", front(lemma))), "ini"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("paleih", front(lemma))), "inne"));
    push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("paleih", front(lemma))), "isi"));
    push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleih", front(lemma))), "insa"));
    push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pal", front(lemma))), "een"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pal", front(lemma))), "eemme"));
    push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pal", front(lemma))), "eeni"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pal", front(lemma))), "eenne"));
    push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("palees", front(lemma))), "eesi"));
    push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("pal", front(lemma))), "eensa"));
    push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "issa"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "issamme"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "issani"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "issanne"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "issasi"));
    push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleissaan", front(lemma))), ""));
    push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "essa"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "essamme"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "essani"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "essanne"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "essasi"));
    push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleessaan", front(lemma))), ""));
    push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "in"));
    push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "et"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "emme"));
    push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eni"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "enne"));
    push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "esi"));
    push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ensa"));
    push(Case::Nominative, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), ""));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "emme"));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eni"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "enne"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "esi"));
    push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ensa"));
    push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("paleit", front(lemma))), "a"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("paleit", front(lemma))), "amme"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("paleit", front(lemma))), "ani"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("paleit", front(lemma))), "anne"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("paleit", front(lemma))), "asi"));
    push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleitaan", front(lemma))), ""));
    push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("taan", front(lemma))), ""));
    push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iksi"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iksemme"));
    push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ikseni"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iksenne"));
    push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "iksesi"));
    push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleikseen", front(lemma))), ""));
    push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eksi"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eksemme"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "ekseni"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eksenne"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, &harmonize("pale", front(lemma))), "eksesi"));
    push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, &harmonize("paleekseen", front(lemma))), ""));
    drop(stem);
    drop(double);
    drop(front);
    drop(harmonize);
    drop(push);
    out
}
