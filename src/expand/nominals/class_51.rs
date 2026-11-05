//! Inflection rule for class 51, generated from the noun lexicon.
//! Regenerate with:  python3 tools/build/extract_class_rules.py --rust
//! do not edit by hand; the database is the source of truth.
#![allow(unused_imports)]

use crate::tags::{Case, Confidence, Form, Number, Possessive, Tags};

pub const CLASS: &str = "51";
pub const MODEL: &str = "uusikuu";

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
    push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 5, &harmonize("oittamer", front(lemma))), "ittä"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, &harmonize("oittamer", front(lemma))), "ittämme"));
    push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, &harmonize("oittamer", front(lemma))), "ittäni"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, &harmonize("oittamer", front(lemma))), "ittänne"));
    push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, &harmonize("oittamer", front(lemma))), "ittäsi"));
    push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "oittamerittänsä"), ""));
    push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 4, &harmonize("ttamere", front(lemma))), "ttä"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, &harmonize("ttamere", front(lemma))), "ttämme"));
    push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, &harmonize("ttamere", front(lemma))), "ttäni"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, &harmonize("ttamere", front(lemma))), "ttänne"));
    push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, &harmonize("ttamere", front(lemma))), "ttäsi"));
    push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "ttamerettänsä"), ""));
    push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 5, &harmonize("oiltamer", front(lemma))), "iltä"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, &harmonize("oiltamer", front(lemma))), "iltämme"));
    push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, &harmonize("oiltamer", front(lemma))), "iltäni"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, &harmonize("oiltamer", front(lemma))), "iltänne"));
    push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, &harmonize("oiltamer", front(lemma))), "iltäsi"));
    push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "oiltameriltänsä"), ""));
    push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 4, &harmonize("ltamere", front(lemma))), "ltä"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, &harmonize("ltamere", front(lemma))), "ltämme"));
    push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, &harmonize("ltamere", front(lemma))), "ltäni"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, &harmonize("ltamere", front(lemma))), "ltänne"));
    push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, &harmonize("ltamere", front(lemma))), "ltäsi"));
    push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "ltamereltänsä"), ""));
    push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 5, &harmonize("oillamer", front(lemma))), "illä"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, &harmonize("oillamer", front(lemma))), "illämme"));
    push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, &harmonize("oillamer", front(lemma))), "illäni"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, &harmonize("oillamer", front(lemma))), "illänne"));
    push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, &harmonize("oillamer", front(lemma))), "illäsi"));
    push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "oillamerillänsä"), ""));
    push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 4, &harmonize("llamere", front(lemma))), "llä"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, &harmonize("llamere", front(lemma))), "llämme"));
    push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, &harmonize("llamere", front(lemma))), "lläni"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, &harmonize("llamere", front(lemma))), "llänne"));
    push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, &harmonize("llamere", front(lemma))), "lläsi"));
    push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "llamerellänsä"), ""));
    if lemma.ends_with("ri") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 5, "illemer"), "ille"));
    } else {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 6, &harmonize("illesarjo", front(lemma))), "ille"));
    }
    if lemma.ends_with("ri") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, "illemer"), "illemme"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 6, &harmonize("illesarjo", front(lemma))), "illemme"));
    }
    if lemma.ends_with("ri") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, "illemer"), "illeni"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 6, &harmonize("illesarjo", front(lemma))), "illeni"));
    }
    if lemma.ends_with("ri") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, "illemer"), "illenne"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 6, &harmonize("illesarjo", front(lemma))), "illenne"));
    }
    if lemma.ends_with("ri") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, "illemer"), "illesi"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 6, &harmonize("illesarjo", front(lemma))), "illesi"));
    }
    if lemma.ends_with("ri") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "illemerilleen"), ""));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 6, &harmonize("illesarjoilleen", front(lemma))), ""));
    }
    push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 4, "llemere"), "lle"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, "llemere"), "llemme"));
    push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, "llemere"), "lleni"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, "llemere"), "llenne"));
    push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, "llemere"), "llesi"));
    push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "llemerelleen"), ""));
    push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 5, &harmonize("oissameriss", front(lemma))), "ä"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, &harmonize("oissameriss", front(lemma))), "ämme"));
    push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, &harmonize("oissameriss", front(lemma))), "äni"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, &harmonize("oissameriss", front(lemma))), "änne"));
    push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, &harmonize("oissameriss", front(lemma))), "äsi"));
    push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "oissamerissänsä"), ""));
    push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 4, &harmonize("stamere", front(lemma))), "stä"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, &harmonize("stamere", front(lemma))), "stämme"));
    push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, &harmonize("stamere", front(lemma))), "stäni"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, &harmonize("stamere", front(lemma))), "stänne"));
    push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, &harmonize("stamere", front(lemma))), "stäsi"));
    push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "stamerestänsä"), ""));
    push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 5, &harmonize("oinamer", front(lemma))), "inä"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, &harmonize("oinamer", front(lemma))), "inämme"));
    push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, &harmonize("oinamer", front(lemma))), "inäni"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, &harmonize("oinamer", front(lemma))), "inänne"));
    push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, &harmonize("oinamer", front(lemma))), "inäsi"));
    push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "oinamerinänsä"), ""));
    push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 4, &harmonize("namere", front(lemma))), "nä"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, &harmonize("namere", front(lemma))), "nämme"));
    push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, &harmonize("namere", front(lemma))), "näni"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, &harmonize("namere", front(lemma))), "nänne"));
    push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, &harmonize("namere", front(lemma))), "näsi"));
    push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "namerenänsä"), ""));
    if lemma.ends_with("ri") {
        push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 5, "ienmert"), "en"));
    } else {
        push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 6, &harmonize("idensarjoj", front(lemma))), "en"));
    }
    push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 4, "nmere"), "n"));
    push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 5, "iinmeri"), "in"));
    push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 10, &harmonize("seennoutajaa", front(lemma))), "n"));
    push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 5, &harmonize("oissamer", front(lemma))), "issä"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, &harmonize("oissamer", front(lemma))), "issämme"));
    push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, &harmonize("oissamer", front(lemma))), "issäni"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, &harmonize("oissamer", front(lemma))), "issänne"));
    push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, &harmonize("oissamer", front(lemma))), "issäsi"));
    push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "oissamerissänsä"), ""));
    push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 4, &harmonize("ssamere", front(lemma))), "ssä"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, &harmonize("ssamere", front(lemma))), "ssämme"));
    push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, &harmonize("ssamere", front(lemma))), "ssäni"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, &harmonize("ssamere", front(lemma))), "ssänne"));
    push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, &harmonize("ssamere", front(lemma))), "ssäsi"));
    push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "ssameressänsä"), ""));
    push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 4, "tmere"), "t"));
    push(Case::Nominative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), ""));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), "mme"));
    push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), "ni"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), "nne"));
    push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), "si"));
    push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, ""), "nsa"));
    push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 5, &harmonize("ojameri", front(lemma))), "ä"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, &harmonize("ojameri", front(lemma))), "ämme"));
    push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, &harmonize("ojameri", front(lemma))), "äni"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, &harmonize("ojameri", front(lemma))), "änne"));
    push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, &harmonize("ojameri", front(lemma))), "äsi"));
    push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 5, "ojameriänsä"), ""));
    push(Case::Partitive, Number::Singular, None, format!("{}{}", stem(lemma, 4, &harmonize("amert", front(lemma))), "a"));
    push(Case::Partitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, &harmonize("amert", front(lemma))), "amme"));
    push(Case::Partitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, &harmonize("amert", front(lemma))), "ani"));
    push(Case::Partitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, &harmonize("amert", front(lemma))), "anne"));
    push(Case::Partitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, &harmonize("amert", front(lemma))), "asi"));
    push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, &harmonize("amertansa", front(lemma))), ""));
    if lemma.ends_with("ri") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 5, "iksimer"), "iksi"));
    } else {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 6, &harmonize("iksisarjo", front(lemma))), "iksi"));
    }
    if lemma.ends_with("ri") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 5, "iksimer"), "iksemme"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 6, &harmonize("iksisarjo", front(lemma))), "iksemme"));
    }
    if lemma.ends_with("ri") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 5, "iksimer"), "ikseni"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 6, &harmonize("iksisarjo", front(lemma))), "ikseni"));
    }
    if lemma.ends_with("ri") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 5, "iksimer"), "iksenne"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 6, &harmonize("iksisarjo", front(lemma))), "iksenne"));
    }
    if lemma.ends_with("ri") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 5, "iksimer"), "iksesi"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 6, &harmonize("iksisarjo", front(lemma))), "iksesi"));
    }
    push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 6, &harmonize("iksisarjoiksensa", front(lemma))), ""));
    push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 4, "ksimere"), "ksi"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 4, "ksimere"), "ksemme"));
    push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 4, "ksimere"), "kseni"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 4, "ksimere"), "ksenne"));
    push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 4, "ksimere"), "ksesi"));
    push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 4, "ksimerekseen"), ""));
    drop(stem);
    drop(double);
    drop(front);
    drop(harmonize);
    drop(push);
    out
}
