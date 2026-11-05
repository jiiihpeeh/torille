//! Inflection rule for class 50, generated from the noun lexicon.
//! Regenerate with:  python3 tools/build/extract_class_rules.py --rust
//! do not edit by hand; the database is the source of truth.
#![allow(unused_imports)]

use crate::tags::{Case, Confidence, Form, Number, Possessive, Tags};

pub const CLASS: &str = "50";
pub const MODEL: &str = "isoisä";

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
    if lemma.ends_with("en") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), &harmonize("itta", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "ittä"));
    } else if lemma.ends_with("ti") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), &harmonize("itta", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "itta"));
    } else if lemma.ends_with("ta") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "itta"));
    } else if lemma.ends_with("tö") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "ittä"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "ittä"));
    } else if lemma.ends_with("te") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), &harmonize("itta", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "itta"));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "ittä"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "itta"));
    } else {
        push(Case::Abessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), &harmonize("itta", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "ittämme"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "ittamme"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("ittamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "v"), "ittämme"));
    } else if lemma.ends_with("ti") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("ittamme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "ittamme"));
    } else if lemma.ends_with("ta") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ittamme"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "ittämme"));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "ittämme"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "ittamme"));
    } else {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ittamme", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "ittäni"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "ittani"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("ittani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "v"), "ittäni"));
    } else if lemma.ends_with("ti") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("ittani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "ittani"));
    } else if lemma.ends_with("ta") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ittani"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "ittäni"));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "ittäni"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "ittani"));
    } else {
        push(Case::Abessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ittani", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "ittänne"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "ittanne"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("ittanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "v"), "ittänne"));
    } else if lemma.ends_with("ti") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("ittanne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "ittanne"));
    } else if lemma.ends_with("ta") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ittanne"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "ittänne"));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "ittänne"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "ittanne"));
    } else {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ittanne", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "ittäsi"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "ittasi"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("ittasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "v"), "ittäsi"));
    } else if lemma.ends_with("ti") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("ittasi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "ittasi"));
    } else if lemma.ends_with("ta") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ittasi"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "ittäsi"));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "ittäsi"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "ittasi"));
    } else {
        push(Case::Abessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ittasi", front(lemma))));
    }
    if lemma.ends_with("ite") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teittään"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuittansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sittään"), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vittään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("deittansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keittansa", front(lemma))), ""));
    } else if lemma.ends_with("ta") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nittansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döittään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mittään"), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyittään"), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duittansa", front(lemma))), ""));
    } else {
        push(Case::Abessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("ittaan", front(lemma))), ""));
    }
    if lemma.ends_with("nti") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), &harmonize("tta", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "tta"));
    } else if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "ttä"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "tta"));
    } else if lemma.ends_with("nta") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "tta"));
    } else if lemma.ends_with("lta") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "tta"));
    } else if lemma.ends_with("nto") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "tta"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), &harmonize("tta", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "ttä"));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "tta"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "ttä"));
    } else if lemma.ends_with("tä") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "ttä"));
    } else if lemma.ends_with("te") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), &harmonize("tta", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "tta"));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "ttä"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "tta"));
    } else {
        push(Case::Abessive, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("tta", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ttamme", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ttamme"));
    } else if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "ttämme"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "ttamme"));
    } else if lemma.ends_with("nta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ttamme"));
    } else if lemma.ends_with("lta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ttamme"));
    } else if lemma.ends_with("nto") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ttamme"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ttamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vä"), "ttämme"));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "ttamme"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "ttämme"));
    } else if lemma.ends_with("tä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dä"), "ttämme"));
    } else if lemma.ends_with("te") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ttamme", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "ttämme"));
    } else if lemma.ends_with("as") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ttamme"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "ttamme"));
    } else {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ttamme", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ttani", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ttani"));
    } else if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "ttäni"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "ttani"));
    } else if lemma.ends_with("nta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ttani"));
    } else if lemma.ends_with("lta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ttani"));
    } else if lemma.ends_with("nto") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ttani"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ttani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vä"), "ttäni"));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "ttani"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "ttäni"));
    } else if lemma.ends_with("tä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dä"), "ttäni"));
    } else if lemma.ends_with("te") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ttani", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "ttäni"));
    } else if lemma.ends_with("as") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ttani"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "ttani"));
    } else {
        push(Case::Abessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ttani", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ttanne", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ttanne"));
    } else if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "ttänne"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "ttanne"));
    } else if lemma.ends_with("nta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ttanne"));
    } else if lemma.ends_with("lta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ttanne"));
    } else if lemma.ends_with("nto") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ttanne"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ttanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vä"), "ttänne"));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "ttanne"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "ttänne"));
    } else if lemma.ends_with("tä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dä"), "ttänne"));
    } else if lemma.ends_with("te") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ttanne", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "ttänne"));
    } else if lemma.ends_with("as") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ttanne"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "ttanne"));
    } else {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ttanne", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ttasi", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ttasi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "ttäsi"));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "ttasi"));
    } else if lemma.ends_with("nta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ttasi"));
    } else if lemma.ends_with("lta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ttasi"));
    } else if lemma.ends_with("nto") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ttasi"));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ttasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vä"), "ttäsi"));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "ttasi"));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "ttäsi"));
    } else if lemma.ends_with("tä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dä"), "ttäsi"));
    } else if lemma.ends_with("te") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ttasi", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "ttäsi"));
    } else if lemma.ends_with("as") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ttasi"));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "ttasi"));
    } else {
        push(Case::Abessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ttasi", front(lemma))));
    }
    if lemma.ends_with("rta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("rattansa", front(lemma))), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuttansa", front(lemma))), ""));
    } else if lemma.ends_with("nta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nattansa", front(lemma))), ""));
    } else if lemma.ends_with("lta") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("lattansa", front(lemma))), ""));
    } else if lemma.ends_with("nto") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nottansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "settään"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("dettansa", front(lemma))), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vättään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dittansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keettansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döttään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mettään"), ""));
    } else if lemma.ends_with("tä") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "dättään"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teettansa", front(lemma))), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyttään"), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "dettään"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("attansa", front(lemma))), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duttansa", front(lemma))), ""));
    } else {
        push(Case::Abessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("ttaan", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), &harmonize("ilta", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "iltä"));
    } else if lemma.ends_with("ti") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), &harmonize("ilta", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "ilta"));
    } else if lemma.ends_with("ta") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ilta"));
    } else if lemma.ends_with("tö") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "iltä"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "iltä"));
    } else if lemma.ends_with("te") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), &harmonize("ilta", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "ilta"));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "iltä"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "ilta"));
    } else {
        push(Case::Ablative, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), &harmonize("ilta", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "iltämme"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "iltamme"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("iltamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "v"), "iltämme"));
    } else if lemma.ends_with("ti") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("iltamme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "iltamme"));
    } else if lemma.ends_with("ta") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iltamme"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "iltämme"));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "iltämme"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "iltamme"));
    } else {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("iltamme", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "iltäni"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "iltani"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("iltani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "v"), "iltäni"));
    } else if lemma.ends_with("ti") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("iltani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "iltani"));
    } else if lemma.ends_with("ta") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iltani"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "iltäni"));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "iltäni"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "iltani"));
    } else {
        push(Case::Ablative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("iltani", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "iltänne"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "iltanne"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("iltanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "v"), "iltänne"));
    } else if lemma.ends_with("ti") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("iltanne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "iltanne"));
    } else if lemma.ends_with("ta") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iltanne"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "iltänne"));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "iltänne"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "iltanne"));
    } else {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("iltanne", front(lemma))));
    }
    if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "iltäsi"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "iltasi"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("iltasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "v"), "iltäsi"));
    } else if lemma.ends_with("ti") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("iltasi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "iltasi"));
    } else if lemma.ends_with("ta") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iltasi"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "iltäsi"));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "iltäsi"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "iltasi"));
    } else {
        push(Case::Ablative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("iltasi", front(lemma))));
    }
    if lemma.ends_with("ite") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teiltään"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuiltansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "siltään"), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "viltään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("deiltansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keiltansa", front(lemma))), ""));
    } else if lemma.ends_with("ta") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("niltansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döiltään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "miltään"), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyiltään"), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duiltansa", front(lemma))), ""));
    } else {
        push(Case::Ablative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("iltaan", front(lemma))), ""));
    }
    if lemma.ends_with("nti") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), &harmonize("lta", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "lta"));
    } else if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "ltä"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "lta"));
    } else if lemma.ends_with("nta") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "lta"));
    } else if lemma.ends_with("lta") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "lta"));
    } else if lemma.ends_with("nto") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "lta"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), &harmonize("lta", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "ltä"));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "lta"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "ltä"));
    } else if lemma.ends_with("tä") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "ltä"));
    } else if lemma.ends_with("te") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), &harmonize("lta", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "lta"));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "ltä"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "lta"));
    } else {
        push(Case::Ablative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("lta", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ltamme", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ltamme"));
    } else if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "ltämme"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "ltamme"));
    } else if lemma.ends_with("nta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ltamme"));
    } else if lemma.ends_with("lta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ltamme"));
    } else if lemma.ends_with("nto") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ltamme"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ltamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vä"), "ltämme"));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "ltamme"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "ltämme"));
    } else if lemma.ends_with("tä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dä"), "ltämme"));
    } else if lemma.ends_with("te") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ltamme", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "ltämme"));
    } else if lemma.ends_with("as") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ltamme"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "ltamme"));
    } else {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ltamme", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ltani", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ltani"));
    } else if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "ltäni"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "ltani"));
    } else if lemma.ends_with("nta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ltani"));
    } else if lemma.ends_with("lta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ltani"));
    } else if lemma.ends_with("nto") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ltani"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ltani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vä"), "ltäni"));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "ltani"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "ltäni"));
    } else if lemma.ends_with("tä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dä"), "ltäni"));
    } else if lemma.ends_with("te") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ltani", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "ltäni"));
    } else if lemma.ends_with("as") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ltani"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "ltani"));
    } else {
        push(Case::Ablative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ltani", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ltanne", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ltanne"));
    } else if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "ltänne"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "ltanne"));
    } else if lemma.ends_with("nta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ltanne"));
    } else if lemma.ends_with("lta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ltanne"));
    } else if lemma.ends_with("nto") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ltanne"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ltanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vä"), "ltänne"));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "ltanne"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "ltänne"));
    } else if lemma.ends_with("tä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dä"), "ltänne"));
    } else if lemma.ends_with("te") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ltanne", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "ltänne"));
    } else if lemma.ends_with("as") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ltanne"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "ltanne"));
    } else {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ltanne", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ltasi", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ltasi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "ltäsi"));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "ltasi"));
    } else if lemma.ends_with("nta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ltasi"));
    } else if lemma.ends_with("lta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ltasi"));
    } else if lemma.ends_with("nto") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ltasi"));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ltasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vä"), "ltäsi"));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "ltasi"));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "ltäsi"));
    } else if lemma.ends_with("tä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dä"), "ltäsi"));
    } else if lemma.ends_with("te") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ltasi", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "ltäsi"));
    } else if lemma.ends_with("as") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ltasi"));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "ltasi"));
    } else {
        push(Case::Ablative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ltasi", front(lemma))));
    }
    if lemma.ends_with("rta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("raltansa", front(lemma))), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vultansa", front(lemma))), ""));
    } else if lemma.ends_with("nta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("naltansa", front(lemma))), ""));
    } else if lemma.ends_with("lta") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("laltansa", front(lemma))), ""));
    } else if lemma.ends_with("nto") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("noltansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "seltään"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("deltansa", front(lemma))), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vältään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("diltansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keeltansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döltään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "meltään"), ""));
    } else if lemma.ends_with("tä") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "dältään"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teeltansa", front(lemma))), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyltään"), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "deltään"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("altansa", front(lemma))), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dultansa", front(lemma))), ""));
    } else {
        push(Case::Ablative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("ltaan", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), &harmonize("illa", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "illä"));
    } else if lemma.ends_with("ti") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), &harmonize("illa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "illa"));
    } else if lemma.ends_with("ta") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illa"));
    } else if lemma.ends_with("tö") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "illä"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "illä"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), &harmonize("illa", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "illa"));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "illä"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "illa"));
    } else {
        push(Case::Adessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), &harmonize("illa", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("illamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "v"), "illämme"));
    } else if lemma.ends_with("ti") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("illamme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "illamme"));
    } else if lemma.ends_with("ta") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illamme"));
    } else if lemma.ends_with("tö") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "illämme"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "illämme"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "te"), &harmonize("illamme", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "illamme"));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "illämme"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "illamme"));
    } else {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, ""), &harmonize("illamme", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("illani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "v"), "illäni"));
    } else if lemma.ends_with("ti") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("illani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "illani"));
    } else if lemma.ends_with("ta") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illani"));
    } else if lemma.ends_with("tö") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "illäni"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "illäni"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "te"), &harmonize("illani", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "illani"));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "illäni"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "illani"));
    } else {
        push(Case::Adessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, ""), &harmonize("illani", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("illanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "v"), "illänne"));
    } else if lemma.ends_with("ti") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("illanne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "illanne"));
    } else if lemma.ends_with("ta") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illanne"));
    } else if lemma.ends_with("tö") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "illänne"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "illänne"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "te"), &harmonize("illanne", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "illanne"));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "illänne"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "illanne"));
    } else {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, ""), &harmonize("illanne", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("illasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "v"), "illäsi"));
    } else if lemma.ends_with("ti") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("illasi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "illasi"));
    } else if lemma.ends_with("ta") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illasi"));
    } else if lemma.ends_with("tö") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "illäsi"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "illäsi"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "te"), &harmonize("illasi", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "illasi"));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "illäsi"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "illasi"));
    } else {
        push(Case::Adessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, ""), &harmonize("illasi", front(lemma))));
    }
    if lemma.ends_with("ite") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teillään"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuillansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sillään"), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "villään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("deillansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keillansa", front(lemma))), ""));
    } else if lemma.ends_with("ta") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nillansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döillään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "millään"), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyillään"), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duillansa", front(lemma))), ""));
    } else {
        push(Case::Adessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("illaan", front(lemma))), ""));
    }
    if lemma.ends_with("nti") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), &harmonize("lla", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "lla"));
    } else if lemma.ends_with("ätö") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "llä"));
    } else if lemma.ends_with("uku") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "lla"));
    } else if lemma.ends_with("nta") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "lla"));
    } else if lemma.ends_with("lta") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "lla"));
    } else if lemma.ends_with("nto") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "lla"));
    } else if lemma.ends_with("en") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), &harmonize("lla", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "llä"));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "lla"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "llä"));
    } else if lemma.ends_with("tä") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "llä"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), &harmonize("lla", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "lla"));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "llä"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "lla"));
    } else {
        push(Case::Adessive, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("lla", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("llamme", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "llamme"));
    } else if lemma.ends_with("ätö") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "llämme"));
    } else if lemma.ends_with("uku") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "llamme"));
    } else if lemma.ends_with("nta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "llamme"));
    } else if lemma.ends_with("lta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "llamme"));
    } else if lemma.ends_with("nto") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "llamme"));
    } else if lemma.ends_with("en") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("llamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vä"), "llämme"));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "llamme"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "llämme"));
    } else if lemma.ends_with("tä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dä"), "llämme"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("llamme", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "llämme"));
    } else if lemma.ends_with("as") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "llamme"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "llamme"));
    } else {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("llamme", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("llani", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "llani"));
    } else if lemma.ends_with("ätö") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "lläni"));
    } else if lemma.ends_with("uku") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "llani"));
    } else if lemma.ends_with("nta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "llani"));
    } else if lemma.ends_with("lta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "llani"));
    } else if lemma.ends_with("nto") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "llani"));
    } else if lemma.ends_with("en") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("llani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vä"), "lläni"));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "llani"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "lläni"));
    } else if lemma.ends_with("tä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dä"), "lläni"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("llani", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "lläni"));
    } else if lemma.ends_with("as") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "llani"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "llani"));
    } else {
        push(Case::Adessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("llani", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("llanne", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "llanne"));
    } else if lemma.ends_with("ätö") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "llänne"));
    } else if lemma.ends_with("uku") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "llanne"));
    } else if lemma.ends_with("nta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "llanne"));
    } else if lemma.ends_with("lta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "llanne"));
    } else if lemma.ends_with("nto") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "llanne"));
    } else if lemma.ends_with("en") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("llanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vä"), "llänne"));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "llanne"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "llänne"));
    } else if lemma.ends_with("tä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dä"), "llänne"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("llanne", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "llänne"));
    } else if lemma.ends_with("as") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "llanne"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "llanne"));
    } else {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("llanne", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("llasi", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "llasi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "lläsi"));
    } else if lemma.ends_with("uku") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "llasi"));
    } else if lemma.ends_with("nta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "llasi"));
    } else if lemma.ends_with("lta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "llasi"));
    } else if lemma.ends_with("nto") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "llasi"));
    } else if lemma.ends_with("en") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("llasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vä"), "lläsi"));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "llasi"));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "lläsi"));
    } else if lemma.ends_with("tä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dä"), "lläsi"));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("llasi", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "lläsi"));
    } else if lemma.ends_with("as") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "llasi"));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "llasi"));
    } else {
        push(Case::Adessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("llasi", front(lemma))));
    }
    if lemma.ends_with("rta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("rallansa", front(lemma))), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vullansa", front(lemma))), ""));
    } else if lemma.ends_with("nta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nallansa", front(lemma))), ""));
    } else if lemma.ends_with("lta") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("lallansa", front(lemma))), ""));
    } else if lemma.ends_with("nto") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nollansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sellään"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("dellansa", front(lemma))), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vällään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dillansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keellansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döllään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mellään"), ""));
    } else if lemma.ends_with("tä") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "dällään"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teellansa", front(lemma))), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyllään"), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "dellään"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("allansa", front(lemma))), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dullansa", front(lemma))), ""));
    } else {
        push(Case::Adessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("llaan", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), "ille"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "ille"));
    } else if lemma.ends_with("ti") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), "ille"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "ille"));
    } else if lemma.ends_with("ta") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ille"));
    } else if lemma.ends_with("tö") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "ille"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "ille"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), "ille"));
    } else if lemma.ends_with("ku") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "ille"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "ille"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "ille"));
    } else {
        push(Case::Allative, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), "ille"));
    }
    if lemma.ends_with("en") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), "illemme"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "v"), "illemme"));
    } else if lemma.ends_with("ti") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "de"), "illemme"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "illemme"));
    } else if lemma.ends_with("ta") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illemme"));
    } else if lemma.ends_with("tö") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "illemme"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "illemme"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "te"), "illemme"));
    } else if lemma.ends_with("ku") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "illemme"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "illemme"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "illemme"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, ""), "illemme"));
    }
    if lemma.ends_with("en") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), "illeni"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "v"), "illeni"));
    } else if lemma.ends_with("ti") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "de"), "illeni"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "illeni"));
    } else if lemma.ends_with("ta") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illeni"));
    } else if lemma.ends_with("tö") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "illeni"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "illeni"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "te"), "illeni"));
    } else if lemma.ends_with("ku") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "illeni"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "illeni"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "illeni"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, ""), "illeni"));
    }
    if lemma.ends_with("en") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), "illenne"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "v"), "illenne"));
    } else if lemma.ends_with("ti") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "de"), "illenne"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "illenne"));
    } else if lemma.ends_with("ta") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illenne"));
    } else if lemma.ends_with("tö") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "illenne"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "illenne"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "te"), "illenne"));
    } else if lemma.ends_with("ku") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "illenne"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "illenne"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "illenne"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, ""), "illenne"));
    }
    if lemma.ends_with("en") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), "illesi"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "v"), "illesi"));
    } else if lemma.ends_with("ti") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "de"), "illesi"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "illesi"));
    } else if lemma.ends_with("ta") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "illesi"));
    } else if lemma.ends_with("tö") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "illesi"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "illesi"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "te"), "illesi"));
    } else if lemma.ends_with("ku") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "illesi"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "illesi"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "illesi"));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, ""), "illesi"));
    }
    if lemma.ends_with("ätö") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döillensä"), ""));
    } else if lemma.ends_with("ite") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teillensä"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuillensa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sillensä"), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "villensä"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("deillensa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keillensa", front(lemma))), ""));
    } else if lemma.ends_with("ta") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nillensa", front(lemma))), ""));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "millensä"), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyillensä"), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duillensa", front(lemma))), ""));
    } else {
        push(Case::Allative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "illeen"), ""));
    }
    if lemma.ends_with("nti") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), "lle"));
    } else if lemma.ends_with("rta") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "lle"));
    } else if lemma.ends_with("ätö") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "lle"));
    } else if lemma.ends_with("uku") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "lle"));
    } else if lemma.ends_with("nta") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "lle"));
    } else if lemma.ends_with("lta") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "lle"));
    } else if lemma.ends_with("nto") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "lle"));
    } else if lemma.ends_with("en") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), "lle"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "lle"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "lle"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "lle"));
    } else if lemma.ends_with("tä") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "lle"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), "lle"));
    } else if lemma.ends_with("as") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "lle"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "lle"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "lle"));
    } else {
        push(Case::Allative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), "lle"));
    }
    if lemma.ends_with("nti") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ni"), "llemme"));
    } else if lemma.ends_with("rta") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "llemme"));
    } else if lemma.ends_with("ätö") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "llemme"));
    } else if lemma.ends_with("uku") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "llemme"));
    } else if lemma.ends_with("nta") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "llemme"));
    } else if lemma.ends_with("lta") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "llemme"));
    } else if lemma.ends_with("nto") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "llemme"));
    } else if lemma.ends_with("en") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), "llemme"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vä"), "llemme"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "llemme"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "llemme"));
    } else if lemma.ends_with("tä") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dä"), "llemme"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), "llemme"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "llemme"));
    } else if lemma.ends_with("as") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "llemme"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "llemme"));
    } else {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), "llemme"));
    }
    if lemma.ends_with("nti") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ni"), "lleni"));
    } else if lemma.ends_with("rta") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "lleni"));
    } else if lemma.ends_with("ätö") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "lleni"));
    } else if lemma.ends_with("uku") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "lleni"));
    } else if lemma.ends_with("nta") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "lleni"));
    } else if lemma.ends_with("lta") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "lleni"));
    } else if lemma.ends_with("nto") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "lleni"));
    } else if lemma.ends_with("en") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), "lleni"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vä"), "lleni"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "lleni"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "lleni"));
    } else if lemma.ends_with("tä") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dä"), "lleni"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), "lleni"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "lleni"));
    } else if lemma.ends_with("as") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "lleni"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "lleni"));
    } else {
        push(Case::Allative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), "lleni"));
    }
    if lemma.ends_with("nti") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ni"), "llenne"));
    } else if lemma.ends_with("rta") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "llenne"));
    } else if lemma.ends_with("ätö") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "llenne"));
    } else if lemma.ends_with("uku") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "llenne"));
    } else if lemma.ends_with("nta") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "llenne"));
    } else if lemma.ends_with("lta") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "llenne"));
    } else if lemma.ends_with("nto") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "llenne"));
    } else if lemma.ends_with("en") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), "llenne"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vä"), "llenne"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "llenne"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "llenne"));
    } else if lemma.ends_with("tä") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dä"), "llenne"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), "llenne"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "llenne"));
    } else if lemma.ends_with("as") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "llenne"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "llenne"));
    } else {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), "llenne"));
    }
    if lemma.ends_with("nti") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ni"), "llesi"));
    } else if lemma.ends_with("rta") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "llesi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "llesi"));
    } else if lemma.ends_with("uku") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "llesi"));
    } else if lemma.ends_with("nta") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "llesi"));
    } else if lemma.ends_with("lta") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "llesi"));
    } else if lemma.ends_with("nto") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "llesi"));
    } else if lemma.ends_with("en") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), "llesi"));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vä"), "llesi"));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "llesi"));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "llesi"));
    } else if lemma.ends_with("tä") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dä"), "llesi"));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), "llesi"));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "llesi"));
    } else if lemma.ends_with("as") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "llesi"));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "llesi"));
    } else {
        push(Case::Allative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), "llesi"));
    }
    if lemma.ends_with("nti") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nillensa", front(lemma))), ""));
    } else if lemma.ends_with("rta") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("rallensa", front(lemma))), ""));
    } else if lemma.ends_with("ätö") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döllensä"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vullensa", front(lemma))), ""));
    } else if lemma.ends_with("nta") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nallensa", front(lemma))), ""));
    } else if lemma.ends_with("lta") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("lallensa", front(lemma))), ""));
    } else if lemma.ends_with("nto") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nollensa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sellensä"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("dellensa", front(lemma))), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vällensä"), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keellensa", front(lemma))), ""));
    } else if lemma.ends_with("in") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mellensä"), ""));
    } else if lemma.ends_with("tä") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "dällensä"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teellensa", front(lemma))), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyllensä"), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "dellensä"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("allensa", front(lemma))), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dullensa", front(lemma))), ""));
    } else {
        push(Case::Allative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "lleen"), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), &harmonize("ista", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "istä"));
    } else if lemma.ends_with("ti") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), &harmonize("ista", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "ista"));
    } else if lemma.ends_with("ta") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ista"));
    } else if lemma.ends_with("tö") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "istä"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "istä"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), &harmonize("ista", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "ista"));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "istä"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "ista"));
    } else {
        push(Case::Elative, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), &harmonize("ista", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("istamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "v"), "istämme"));
    } else if lemma.ends_with("ti") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("istamme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "istamme"));
    } else if lemma.ends_with("ta") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "istamme"));
    } else if lemma.ends_with("tö") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "istämme"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "istämme"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "te"), &harmonize("istamme", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "istamme"));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "istämme"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "istamme"));
    } else {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, ""), &harmonize("istamme", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("istani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "v"), "istäni"));
    } else if lemma.ends_with("ti") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("istani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "istani"));
    } else if lemma.ends_with("ta") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "istani"));
    } else if lemma.ends_with("tö") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "istäni"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "istäni"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "te"), &harmonize("istani", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "istani"));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "istäni"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "istani"));
    } else {
        push(Case::Elative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, ""), &harmonize("istani", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("istanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "v"), "istänne"));
    } else if lemma.ends_with("ti") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("istanne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "istanne"));
    } else if lemma.ends_with("ta") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "istanne"));
    } else if lemma.ends_with("tö") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "istänne"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "istänne"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "te"), &harmonize("istanne", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "istanne"));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "istänne"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "istanne"));
    } else {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, ""), &harmonize("istanne", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("istasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "v"), "istäsi"));
    } else if lemma.ends_with("ti") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("istasi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "istasi"));
    } else if lemma.ends_with("ta") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "istasi"));
    } else if lemma.ends_with("tö") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "istäsi"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "istäsi"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "te"), &harmonize("istasi", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "istasi"));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "istäsi"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "istasi"));
    } else {
        push(Case::Elative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, ""), &harmonize("istasi", front(lemma))));
    }
    if lemma.ends_with("ite") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teistään"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuistansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sistään"), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vistään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("deistansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keistansa", front(lemma))), ""));
    } else if lemma.ends_with("ta") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nistansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döistään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mistään"), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyistään"), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duistansa", front(lemma))), ""));
    } else {
        push(Case::Elative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("istaan", front(lemma))), ""));
    }
    if lemma.ends_with("nti") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), &harmonize("sta", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "sta"));
    } else if lemma.ends_with("ätö") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "stä"));
    } else if lemma.ends_with("uku") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "sta"));
    } else if lemma.ends_with("nta") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "sta"));
    } else if lemma.ends_with("lta") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "sta"));
    } else if lemma.ends_with("nto") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "sta"));
    } else if lemma.ends_with("en") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), &harmonize("sta", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "stä"));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "sta"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "stä"));
    } else if lemma.ends_with("tä") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "stä"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), &harmonize("sta", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "sta"));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "stä"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "sta"));
    } else {
        push(Case::Elative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("sta", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("stamme", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "stamme"));
    } else if lemma.ends_with("ätö") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "stämme"));
    } else if lemma.ends_with("uku") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "stamme"));
    } else if lemma.ends_with("nta") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "stamme"));
    } else if lemma.ends_with("lta") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "stamme"));
    } else if lemma.ends_with("nto") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "stamme"));
    } else if lemma.ends_with("en") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("stamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vä"), "stämme"));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "stamme"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "stämme"));
    } else if lemma.ends_with("tä") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dä"), "stämme"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("stamme", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "stämme"));
    } else if lemma.ends_with("as") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "stamme"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "stamme"));
    } else {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("stamme", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("stani", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "stani"));
    } else if lemma.ends_with("ätö") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "stäni"));
    } else if lemma.ends_with("uku") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "stani"));
    } else if lemma.ends_with("nta") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "stani"));
    } else if lemma.ends_with("lta") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "stani"));
    } else if lemma.ends_with("nto") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "stani"));
    } else if lemma.ends_with("en") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("stani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vä"), "stäni"));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "stani"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "stäni"));
    } else if lemma.ends_with("tä") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dä"), "stäni"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("stani", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "stäni"));
    } else if lemma.ends_with("as") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "stani"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "stani"));
    } else {
        push(Case::Elative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("stani", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("stanne", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "stanne"));
    } else if lemma.ends_with("ätö") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "stänne"));
    } else if lemma.ends_with("uku") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "stanne"));
    } else if lemma.ends_with("nta") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "stanne"));
    } else if lemma.ends_with("lta") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "stanne"));
    } else if lemma.ends_with("nto") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "stanne"));
    } else if lemma.ends_with("en") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("stanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vä"), "stänne"));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "stanne"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "stänne"));
    } else if lemma.ends_with("tä") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dä"), "stänne"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("stanne", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "stänne"));
    } else if lemma.ends_with("as") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "stanne"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "stanne"));
    } else {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("stanne", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("stasi", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "stasi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "stäsi"));
    } else if lemma.ends_with("uku") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "stasi"));
    } else if lemma.ends_with("nta") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "stasi"));
    } else if lemma.ends_with("lta") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "stasi"));
    } else if lemma.ends_with("nto") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "stasi"));
    } else if lemma.ends_with("en") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("stasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vä"), "stäsi"));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "stasi"));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "stäsi"));
    } else if lemma.ends_with("tä") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dä"), "stäsi"));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("stasi", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "stäsi"));
    } else if lemma.ends_with("as") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "stasi"));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "stasi"));
    } else {
        push(Case::Elative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("stasi", front(lemma))));
    }
    if lemma.ends_with("rta") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("rastansa", front(lemma))), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vustansa", front(lemma))), ""));
    } else if lemma.ends_with("nta") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nastansa", front(lemma))), ""));
    } else if lemma.ends_with("lta") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("lastansa", front(lemma))), ""));
    } else if lemma.ends_with("nto") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nostansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sestään"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("destansa", front(lemma))), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "västään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("distansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keestansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döstään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mestään"), ""));
    } else if lemma.ends_with("tä") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "dästään"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teestansa", front(lemma))), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nystään"), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "destään"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("astansa", front(lemma))), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dustansa", front(lemma))), ""));
    } else {
        push(Case::Elative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("staan", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), &harmonize("ina", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "ina"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "inä"));
    } else {
        push(Case::Essive, Number::Plural, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("ina", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("inamme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "inamme"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "inämme"));
    } else {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("inamme", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("inani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "inani"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "inäni"));
    } else {
        push(Case::Essive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("inani", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("inanne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "inanne"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "inänne"));
    } else {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("inanne", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("inasi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "inasi"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "inäsi"));
    } else {
        push(Case::Essive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("inasi", front(lemma))));
    }
    if lemma.ends_with("ite") {
        push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teinään"), ""));
    } else if lemma.ends_with("en") {
        push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sinään"), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keinansa", front(lemma))), ""));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "minään"), ""));
    } else {
        push(Case::Essive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("inaan", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), &harmonize("na", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "na"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "nä"));
    } else if lemma.ends_with("te") {
        push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), &harmonize("na", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "na"));
    } else {
        push(Case::Essive, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("na", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("namme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "namme"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "nämme"));
    } else if lemma.ends_with("te") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("namme", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "namme"));
    } else {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("namme", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("nani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "nani"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "näni"));
    } else if lemma.ends_with("te") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("nani", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nani"));
    } else {
        push(Case::Essive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("nani", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("nanne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "nanne"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "nänne"));
    } else if lemma.ends_with("te") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("nanne", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nanne"));
    } else {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("nanne", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("nasi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "nasi"));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "näsi"));
    } else if lemma.ends_with("te") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("nasi", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nasi"));
    } else {
        push(Case::Essive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("nasi", front(lemma))));
    }
    if lemma.ends_with("esi") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "tenään"), ""));
    } else if lemma.ends_with("en") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "senään"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("tenansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keenansa", front(lemma))), ""));
    } else if lemma.ends_with("in") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "menään"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teenansa", front(lemma))), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "tenään"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("anansa", front(lemma))), ""));
    } else {
        push(Case::Essive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("naan", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "si"), "en"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "keitt"), "en"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "mi"), "en"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "teitt"), "en"));
    } else {
        push(Case::Genitive, Number::Plural, None, format!("{}{}", stem(lemma, 0, ""), "en"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "si"), "emme"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "keitt"), "emme"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "mi"), "emme"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "teitt"), "emme"));
    } else {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), "emme"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "si"), "eni"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "keitt"), "eni"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "mi"), "eni"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "teitt"), "eni"));
    } else {
        push(Case::Genitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), "eni"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "si"), "enne"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "keitt"), "enne"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "mi"), "enne"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "teitt"), "enne"));
    } else {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), "enne"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "si"), "esi"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "keitt"), "esi"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "mi"), "esi"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "teitt"), "esi"));
    } else {
        push(Case::Genitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), "esi"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "si"), &harmonize("ensa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "keitt"), "ensa"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mi"), "ensä"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teitt"), &harmonize("ensa", front(lemma))));
    } else {
        push(Case::Genitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, ""), &harmonize("ensa", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), "n"));
    } else if lemma.ends_with("rta") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "n"));
    } else if lemma.ends_with("ätö") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "n"));
    } else if lemma.ends_with("uku") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "n"));
    } else if lemma.ends_with("nta") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "n"));
    } else if lemma.ends_with("lta") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "n"));
    } else if lemma.ends_with("nto") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "n"));
    } else if lemma.ends_with("en") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), "n"));
    } else if lemma.ends_with("pä") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "n"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "n"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "n"));
    } else if lemma.ends_with("tä") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "n"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), "n"));
    } else if lemma.ends_with("as") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "n"));
    } else if lemma.ends_with("ty") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "n"));
    } else if lemma.ends_with("tu") {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "n"));
    } else {
        push(Case::Genitive, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), "n"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), "mme"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "mme"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "mme"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), "mme"));
    } else if lemma.ends_with("as") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "mme"));
    } else {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), "mme"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), "ni"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "ni"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "ni"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), "ni"));
    } else if lemma.ends_with("as") {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ni"));
    } else {
        push(Case::Genitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), "ni"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), "nne"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "nne"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "nne"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), "nne"));
    } else if lemma.ends_with("as") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nne"));
    } else {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), "nne"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), "si"));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "si"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "si"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), "si"));
    } else if lemma.ends_with("as") {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "si"));
    } else {
        push(Case::Genitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), "si"));
    }
    if lemma.ends_with("en") {
        push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "se"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "kee"), "nsa"));
    } else if lemma.ends_with("in") {
        push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "me"), "nsä"));
    } else if lemma.ends_with("te") {
        push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nsa"));
    } else {
        push(Case::Genitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, ""), &harmonize("nsa", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "si"), "in"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "keisi"), "in"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "mi"), "in"));
    } else {
        push(Case::Illative, Number::Plural, None, format!("{}{}", stem(lemma, 0, "ih"), "in"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "si"), "imme"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "keisi"), "imme"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "mi"), "imme"));
    } else {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, "ih"), "imme"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "si"), "ini"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "keisi"), "ini"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "mi"), "ini"));
    } else {
        push(Case::Illative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, "ih"), "ini"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "si"), "inne"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "keisi"), "inne"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "mi"), "inne"));
    } else {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, "ih"), "inne"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "si"), "isi"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "keisi"), "isi"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "mi"), "isi"));
    } else {
        push(Case::Illative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, "ih"), "isi"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "si"), &harmonize("insa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "keisi"), "insa"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mi"), "insä"));
    } else {
        push(Case::Illative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "ih"), &harmonize("insa", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "see"), "n"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "keesee"), "n"));
    } else if lemma.ends_with("ää") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 0, "hä"), "n"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "mee"), "n"));
    } else if lemma.ends_with("uu") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 0, "hu"), "n"));
    } else if lemma.ends_with("te") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "teesee"), "n"));
    } else if lemma.ends_with("as") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("asee", front(lemma))), "n"));
    } else if lemma.ends_with("si") {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "tee"), "n"));
    } else {
        push(Case::Illative, Number::Singular, None, format!("{}{}", stem(lemma, 0, &harmonize("a", front(lemma))), "n"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "see"), "mme"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "keesee"), "mme"));
    } else if lemma.ends_with("ää") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, "hä"), "mme"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "mee"), "mme"));
    } else if lemma.ends_with("uu") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, "hu"), "mme"));
    } else if lemma.ends_with("te") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "teesee"), "mme"));
    } else if lemma.ends_with("as") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("asee", front(lemma))), "mme"));
    } else if lemma.ends_with("si") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "tee"), "mme"));
    } else {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, &harmonize("a", front(lemma))), "mme"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "see"), "ni"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "keesee"), "ni"));
    } else if lemma.ends_with("ää") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, "hä"), "ni"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "mee"), "ni"));
    } else if lemma.ends_with("uu") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, "hu"), "ni"));
    } else if lemma.ends_with("te") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "teesee"), "ni"));
    } else if lemma.ends_with("as") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("asee", front(lemma))), "ni"));
    } else if lemma.ends_with("si") {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "tee"), "ni"));
    } else {
        push(Case::Illative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, &harmonize("a", front(lemma))), "ni"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "see"), "nne"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "keesee"), "nne"));
    } else if lemma.ends_with("ää") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, "hä"), "nne"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "mee"), "nne"));
    } else if lemma.ends_with("uu") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, "hu"), "nne"));
    } else if lemma.ends_with("te") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "teesee"), "nne"));
    } else if lemma.ends_with("as") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("asee", front(lemma))), "nne"));
    } else if lemma.ends_with("si") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "tee"), "nne"));
    } else {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, &harmonize("a", front(lemma))), "nne"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "see"), "si"));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "keesee"), "si"));
    } else if lemma.ends_with("ää") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, "hä"), "si"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "mee"), "si"));
    } else if lemma.ends_with("uu") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, "hu"), "si"));
    } else if lemma.ends_with("te") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "teesee"), "si"));
    } else if lemma.ends_with("as") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("asee", front(lemma))), "si"));
    } else if lemma.ends_with("si") {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "tee"), "si"));
    } else {
        push(Case::Illative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, &harmonize("a", front(lemma))), "si"));
    }
    if lemma.ends_with("en") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "see"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "keesee"), "nsa"));
    } else if lemma.ends_with("ää") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "hä"), "nsä"));
    } else if lemma.ends_with("in") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "mee"), "nsä"));
    } else if lemma.ends_with("uu") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "hu"), "nsa"));
    } else if lemma.ends_with("te") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teesee"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("asee", front(lemma))), "nsa"));
    } else if lemma.ends_with("si") {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "tee"), &harmonize("nsa", front(lemma))));
    } else {
        push(Case::Illative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("a", front(lemma))), "nsa"));
    }
    if lemma.ends_with("en") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), &harmonize("issa", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "issä"));
    } else if lemma.ends_with("ti") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), &harmonize("issa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "issa"));
    } else if lemma.ends_with("ta") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "issa"));
    } else if lemma.ends_with("tö") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "issä"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "issä"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), &harmonize("issa", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "issa"));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "issä"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "issa"));
    } else {
        push(Case::Inessive, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), &harmonize("issa", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("issamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "v"), "issämme"));
    } else if lemma.ends_with("ti") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("issamme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "issamme"));
    } else if lemma.ends_with("ta") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "issamme"));
    } else if lemma.ends_with("tö") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "issämme"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "issämme"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "te"), &harmonize("issamme", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "issamme"));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "issämme"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "issamme"));
    } else {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, ""), &harmonize("issamme", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("issani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "v"), "issäni"));
    } else if lemma.ends_with("ti") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("issani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "issani"));
    } else if lemma.ends_with("ta") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "issani"));
    } else if lemma.ends_with("tö") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "issäni"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "issäni"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "te"), &harmonize("issani", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "issani"));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "issäni"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "issani"));
    } else {
        push(Case::Inessive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, ""), &harmonize("issani", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), &harmonize("issanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "v"), "issänne"));
    } else if lemma.ends_with("ti") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "de"), &harmonize("issanne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "issanne"));
    } else if lemma.ends_with("ta") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "issanne"));
    } else if lemma.ends_with("tö") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "issänne"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "issänne"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "te"), &harmonize("issanne", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "issanne"));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "issänne"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "issanne"));
    } else {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, ""), &harmonize("issanne", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), &harmonize("issasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "v"), "issäsi"));
    } else if lemma.ends_with("ti") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "de"), &harmonize("issasi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "issasi"));
    } else if lemma.ends_with("ta") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "issasi"));
    } else if lemma.ends_with("tö") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "issäsi"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "issäsi"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "te"), &harmonize("issasi", front(lemma))));
    } else if lemma.ends_with("ku") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "issasi"));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "issäsi"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "issasi"));
    } else {
        push(Case::Inessive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, ""), &harmonize("issasi", front(lemma))));
    }
    if lemma.ends_with("ite") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teissään"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuissansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sissään"), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vissään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("deissansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keissansa", front(lemma))), ""));
    } else if lemma.ends_with("ta") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nissansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döissään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "missään"), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyissään"), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duissansa", front(lemma))), ""));
    } else {
        push(Case::Inessive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("issaan", front(lemma))), ""));
    }
    if lemma.ends_with("nti") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ssa", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ssa"));
    } else if lemma.ends_with("ätö") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "ssä"));
    } else if lemma.ends_with("uku") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "ssa"));
    } else if lemma.ends_with("nta") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ssa"));
    } else if lemma.ends_with("lta") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ssa"));
    } else if lemma.ends_with("nto") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ssa"));
    } else if lemma.ends_with("en") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), &harmonize("ssa", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "ssä"));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "ssa"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "ssä"));
    } else if lemma.ends_with("tä") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "ssä"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ssa", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ssa"));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "ssä"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "ssa"));
    } else {
        push(Case::Inessive, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("ssa", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ssamme", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ssamme"));
    } else if lemma.ends_with("ätö") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "ssämme"));
    } else if lemma.ends_with("uku") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "ssamme"));
    } else if lemma.ends_with("nta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ssamme"));
    } else if lemma.ends_with("lta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ssamme"));
    } else if lemma.ends_with("nto") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ssamme"));
    } else if lemma.ends_with("en") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ssamme", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vä"), "ssämme"));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "ssamme"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "ssämme"));
    } else if lemma.ends_with("tä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dä"), "ssämme"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ssamme", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "ssämme"));
    } else if lemma.ends_with("as") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ssamme"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "ssamme"));
    } else {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ssamme", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ssani", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ssani"));
    } else if lemma.ends_with("ätö") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "ssäni"));
    } else if lemma.ends_with("uku") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "ssani"));
    } else if lemma.ends_with("nta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ssani"));
    } else if lemma.ends_with("lta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ssani"));
    } else if lemma.ends_with("nto") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ssani"));
    } else if lemma.ends_with("en") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ssani", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vä"), "ssäni"));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "ssani"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "ssäni"));
    } else if lemma.ends_with("tä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dä"), "ssäni"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ssani", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "ssäni"));
    } else if lemma.ends_with("as") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ssani"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "ssani"));
    } else {
        push(Case::Inessive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ssani", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ssanne", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ssanne"));
    } else if lemma.ends_with("ätö") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "ssänne"));
    } else if lemma.ends_with("uku") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "ssanne"));
    } else if lemma.ends_with("nta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ssanne"));
    } else if lemma.ends_with("lta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ssanne"));
    } else if lemma.ends_with("nto") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ssanne"));
    } else if lemma.ends_with("en") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ssanne", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vä"), "ssänne"));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "ssanne"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "ssänne"));
    } else if lemma.ends_with("tä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dä"), "ssänne"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ssanne", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "ssänne"));
    } else if lemma.ends_with("as") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ssanne"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "ssanne"));
    } else {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("ssanne", front(lemma))));
    }
    if lemma.ends_with("nti") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ni"), &harmonize("ssasi", front(lemma))));
    } else if lemma.ends_with("rta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ssasi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "ssäsi"));
    } else if lemma.ends_with("uku") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "ssasi"));
    } else if lemma.ends_with("nta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ssasi"));
    } else if lemma.ends_with("lta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ssasi"));
    } else if lemma.ends_with("nto") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ssasi"));
    } else if lemma.ends_with("en") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), &harmonize("ssasi", front(lemma))));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vä"), "ssäsi"));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "ssasi"));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "ssäsi"));
    } else if lemma.ends_with("tä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dä"), "ssäsi"));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("ssasi", front(lemma))));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "ssäsi"));
    } else if lemma.ends_with("as") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ssasi"));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "ssasi"));
    } else {
        push(Case::Inessive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ssasi", front(lemma))));
    }
    if lemma.ends_with("rta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("rassansa", front(lemma))), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vussansa", front(lemma))), ""));
    } else if lemma.ends_with("nta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nassansa", front(lemma))), ""));
    } else if lemma.ends_with("lta") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("lassansa", front(lemma))), ""));
    } else if lemma.ends_with("nto") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("nossansa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "sessään"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("dessansa", front(lemma))), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "vässään"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dissansa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keessansa", front(lemma))), ""));
    } else if lemma.ends_with("tö") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "dössään"), ""));
    } else if lemma.ends_with("in") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "messään"), ""));
    } else if lemma.ends_with("tä") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "dässään"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teessansa", front(lemma))), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyssään"), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "dessään"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("assansa", front(lemma))), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("dussansa", front(lemma))), ""));
    } else {
        push(Case::Inessive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("ssaan", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), "in"));
    } else if lemma.ends_with("pä") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "in"));
    } else if lemma.ends_with("ti") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), "in"));
    } else if lemma.ends_with("ke") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "in"));
    } else if lemma.ends_with("ta") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "in"));
    } else if lemma.ends_with("tö") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "in"));
    } else if lemma.ends_with("in") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "in"));
    } else if lemma.ends_with("te") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), "in"));
    } else if lemma.ends_with("ku") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "in"));
    } else if lemma.ends_with("ty") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "in"));
    } else if lemma.ends_with("tu") {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "in"));
    } else {
        push(Case::Instructive, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), "in"));
    }
    if lemma.ends_with("nti") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ni"), "t"));
    } else if lemma.ends_with("rta") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "t"));
    } else if lemma.ends_with("ätö") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "t"));
    } else if lemma.ends_with("uku") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "t"));
    } else if lemma.ends_with("nta") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "t"));
    } else if lemma.ends_with("lta") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "t"));
    } else if lemma.ends_with("nto") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "t"));
    } else if lemma.ends_with("en") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "se"), "t"));
    } else if lemma.ends_with("pä") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vä"), "t"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "kee"), "t"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "me"), "t"));
    } else if lemma.ends_with("tä") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dä"), "t"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "tee"), "t"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "t"));
    } else if lemma.ends_with("ty") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "t"));
    } else if lemma.ends_with("tu") {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "t"));
    } else {
        push(Case::Nominative, Number::Plural, None, format!("{}{}", stem(lemma, 0, ""), "t"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), "mme"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "mme"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "mme"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), "mme"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "mme"));
    } else {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), "mme"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), "ni"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "ni"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "ni"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), "ni"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ni"));
    } else {
        push(Case::Nominative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), "ni"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), "nne"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "nne"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "nne"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), "nne"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nne"));
    } else {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), "nne"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), "si"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "si"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "si"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), "si"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "si"));
    } else {
        push(Case::Nominative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), "si"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "se"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "kee"), "nsa"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "me"), "nsä"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nsa"));
    } else {
        push(Case::Nominative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, ""), &harmonize("nsa", front(lemma))));
    }
    push(Case::Nominative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), ""));
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), "mme"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "mme"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "mme"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), "mme"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "mme"));
    } else {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), "mme"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), "ni"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "ni"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "ni"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), "ni"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ni"));
    } else {
        push(Case::Nominative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), "ni"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), "nne"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "nne"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "nne"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), "nne"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nne"));
    } else {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), "nne"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), "si"));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "si"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "si"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), "si"));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "si"));
    } else {
        push(Case::Nominative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), "si"));
    }
    if lemma.ends_with("en") {
        push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "se"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "kee"), "nsa"));
    } else if lemma.ends_with("in") {
        push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "me"), "nsä"));
    } else if lemma.ends_with("te") {
        push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "tee"), &harmonize("nsa", front(lemma))));
    } else if lemma.ends_with("as") {
        push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "nsa"));
    } else {
        push(Case::Nominative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, ""), &harmonize("nsa", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 3, "si"), &harmonize("a", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "keit"), "a"));
    } else if lemma.ends_with("in") {
        push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "mi"), "ä"));
    } else if lemma.ends_with("te") {
        push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 1, "teit"), &harmonize("a", front(lemma))));
    } else {
        push(Case::Partitive, Number::Plural, None, format!("{}{}", stem(lemma, 0, "j"), &harmonize("a", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "si"), &harmonize("amme", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "keit"), "amme"));
    } else if lemma.ends_with("in") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "mi"), "ämme"));
    } else if lemma.ends_with("te") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "teit"), &harmonize("amme", front(lemma))));
    } else {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, "j"), &harmonize("amme", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "si"), &harmonize("ani", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "keit"), "ani"));
    } else if lemma.ends_with("in") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "mi"), "äni"));
    } else if lemma.ends_with("te") {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "teit"), &harmonize("ani", front(lemma))));
    } else {
        push(Case::Partitive, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, "j"), &harmonize("ani", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "si"), &harmonize("anne", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "keit"), "anne"));
    } else if lemma.ends_with("in") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "mi"), "änne"));
    } else if lemma.ends_with("te") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "teit"), &harmonize("anne", front(lemma))));
    } else {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, "j"), &harmonize("anne", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "si"), &harmonize("asi", front(lemma))));
    } else if lemma.ends_with("ke") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "keit"), "asi"));
    } else if lemma.ends_with("in") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "mi"), "äsi"));
    } else if lemma.ends_with("te") {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "teit"), &harmonize("asi", front(lemma))));
    } else {
        push(Case::Partitive, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, "j"), &harmonize("asi", front(lemma))));
    }
    if lemma.ends_with("en") {
        push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "siään"), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keitansa", front(lemma))), ""));
    } else if lemma.ends_with("in") {
        push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "miään"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teitansa", front(lemma))), ""));
    } else {
        push(Case::Partitive, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("jaan", front(lemma))), ""));
    }
    push(Case::Partitive, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), &harmonize("a", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("amme", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("ani", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), &harmonize("anne", front(lemma))));
    push(Case::Partitive, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), &harmonize("asi", front(lemma))));
    if lemma.ends_with("esi") {
        push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "ttään"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("ttansa", front(lemma))), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "ttään"), ""));
    } else if lemma.ends_with("en") {
        push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "stään"), ""));
    } else {
        push(Case::Partitive, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, &harmonize("ansa", front(lemma))), ""));
    }
    if lemma.ends_with("en") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 3, "s"), "iksi"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "v"), "iksi"));
    } else if lemma.ends_with("ti") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "de"), "iksi"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "ke"), "iksi"));
    } else if lemma.ends_with("ta") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iksi"));
    } else if lemma.ends_with("tö") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "dö"), "iksi"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "m"), "iksi"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 1, "te"), "iksi"));
    } else if lemma.ends_with("ku") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "vu"), "iksi"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "ny"), "iksi"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 2, "du"), "iksi"));
    } else {
        push(Case::Translative, Number::Plural, None, format!("{}{}", stem(lemma, 1, ""), "iksi"));
    }
    if lemma.ends_with("en") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "s"), "iksemme"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "v"), "iksemme"));
    } else if lemma.ends_with("ti") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "de"), "iksemme"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "ke"), "iksemme"));
    } else if lemma.ends_with("ta") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iksemme"));
    } else if lemma.ends_with("tö") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "iksemme"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "m"), "iksemme"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "te"), "iksemme"));
    } else if lemma.ends_with("ku") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "iksemme"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "iksemme"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "iksemme"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, ""), "iksemme"));
    }
    if lemma.ends_with("en") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "s"), "ikseni"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "v"), "ikseni"));
    } else if lemma.ends_with("ti") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "de"), "ikseni"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "ke"), "ikseni"));
    } else if lemma.ends_with("ta") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "ikseni"));
    } else if lemma.ends_with("tö") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "ikseni"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "m"), "ikseni"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "te"), "ikseni"));
    } else if lemma.ends_with("ku") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "ikseni"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "ikseni"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "ikseni"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, ""), "ikseni"));
    }
    if lemma.ends_with("en") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "s"), "iksenne"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "v"), "iksenne"));
    } else if lemma.ends_with("ti") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "de"), "iksenne"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "ke"), "iksenne"));
    } else if lemma.ends_with("ta") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iksenne"));
    } else if lemma.ends_with("tö") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "iksenne"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "m"), "iksenne"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "te"), "iksenne"));
    } else if lemma.ends_with("ku") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "iksenne"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "iksenne"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "iksenne"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, ""), "iksenne"));
    }
    if lemma.ends_with("en") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "s"), "iksesi"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "v"), "iksesi"));
    } else if lemma.ends_with("ti") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "de"), "iksesi"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "ke"), "iksesi"));
    } else if lemma.ends_with("ta") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ro", front(lemma))), "iksesi"));
    } else if lemma.ends_with("tö") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "iksesi"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "m"), "iksesi"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "te"), "iksesi"));
    } else if lemma.ends_with("ku") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "iksesi"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "iksesi"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "iksesi"));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, ""), "iksesi"));
    }
    if lemma.ends_with("ätö") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döiksensä"), ""));
    } else if lemma.ends_with("ite") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "teiksensä"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuiksensa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "siksensä"), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "viksensä"), ""));
    } else if lemma.ends_with("ti") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("deiksensa", front(lemma))), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keiksensa", front(lemma))), ""));
    } else if lemma.ends_with("ta") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("niksensa", front(lemma))), ""));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "miksensä"), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyiksensä"), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duiksensa", front(lemma))), ""));
    } else {
        push(Case::Translative, Number::Plural, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "ikseen"), ""));
    }
    if lemma.ends_with("nti") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ni"), "ksi"));
    } else if lemma.ends_with("rta") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ksi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dö"), "ksi"));
    } else if lemma.ends_with("uku") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vu"), "ksi"));
    } else if lemma.ends_with("nta") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ksi"));
    } else if lemma.ends_with("lta") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ksi"));
    } else if lemma.ends_with("nto") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ksi"));
    } else if lemma.ends_with("en") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 3, "se"), "ksi"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "vä"), "ksi"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "kee"), "ksi"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "me"), "ksi"));
    } else if lemma.ends_with("tä") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "dä"), "ksi"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 1, "tee"), "ksi"));
    } else if lemma.ends_with("as") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ksi"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "ny"), "ksi"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 2, "du"), "ksi"));
    } else {
        push(Case::Translative, Number::Singular, None, format!("{}{}", stem(lemma, 0, ""), "ksi"));
    }
    if lemma.ends_with("nti") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ni"), "ksemme"));
    } else if lemma.ends_with("rta") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ksemme"));
    } else if lemma.ends_with("ätö") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dö"), "ksemme"));
    } else if lemma.ends_with("uku") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vu"), "ksemme"));
    } else if lemma.ends_with("nta") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ksemme"));
    } else if lemma.ends_with("lta") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ksemme"));
    } else if lemma.ends_with("nto") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ksemme"));
    } else if lemma.ends_with("en") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 3, "se"), "ksemme"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "vä"), "ksemme"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "kee"), "ksemme"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "me"), "ksemme"));
    } else if lemma.ends_with("tä") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "dä"), "ksemme"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, "tee"), "ksemme"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "ny"), "ksemme"));
    } else if lemma.ends_with("as") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ksemme"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 2, "du"), "ksemme"));
    } else {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstPlural), format!("{}{}", stem(lemma, 0, ""), "ksemme"));
    }
    if lemma.ends_with("nti") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ni"), "kseni"));
    } else if lemma.ends_with("rta") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "kseni"));
    } else if lemma.ends_with("ätö") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dö"), "kseni"));
    } else if lemma.ends_with("uku") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vu"), "kseni"));
    } else if lemma.ends_with("nta") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "kseni"));
    } else if lemma.ends_with("lta") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "kseni"));
    } else if lemma.ends_with("nto") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "kseni"));
    } else if lemma.ends_with("en") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 3, "se"), "kseni"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "vä"), "kseni"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "kee"), "kseni"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "me"), "kseni"));
    } else if lemma.ends_with("tä") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "dä"), "kseni"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, "tee"), "kseni"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "ny"), "kseni"));
    } else if lemma.ends_with("as") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "kseni"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 2, "du"), "kseni"));
    } else {
        push(Case::Translative, Number::Singular, Some(Possessive::FirstSingular), format!("{}{}", stem(lemma, 0, ""), "kseni"));
    }
    if lemma.ends_with("nti") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ni"), "ksenne"));
    } else if lemma.ends_with("rta") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ksenne"));
    } else if lemma.ends_with("ätö") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dö"), "ksenne"));
    } else if lemma.ends_with("uku") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vu"), "ksenne"));
    } else if lemma.ends_with("nta") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ksenne"));
    } else if lemma.ends_with("lta") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ksenne"));
    } else if lemma.ends_with("nto") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ksenne"));
    } else if lemma.ends_with("en") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 3, "se"), "ksenne"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "vä"), "ksenne"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "kee"), "ksenne"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "me"), "ksenne"));
    } else if lemma.ends_with("tä") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "dä"), "ksenne"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, "tee"), "ksenne"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "ny"), "ksenne"));
    } else if lemma.ends_with("as") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ksenne"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 2, "du"), "ksenne"));
    } else {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondPlural), format!("{}{}", stem(lemma, 0, ""), "ksenne"));
    }
    if lemma.ends_with("nti") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ni"), "ksesi"));
    } else if lemma.ends_with("rta") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("ra", front(lemma))), "ksesi"));
    } else if lemma.ends_with("ätö") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dö"), "ksesi"));
    } else if lemma.ends_with("uku") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vu"), "ksesi"));
    } else if lemma.ends_with("nta") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("na", front(lemma))), "ksesi"));
    } else if lemma.ends_with("lta") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("la", front(lemma))), "ksesi"));
    } else if lemma.ends_with("nto") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, &harmonize("no", front(lemma))), "ksesi"));
    } else if lemma.ends_with("en") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 3, "se"), "ksesi"));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "vä"), "ksesi"));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "kee"), "ksesi"));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "me"), "ksesi"));
    } else if lemma.ends_with("tä") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "dä"), "ksesi"));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, "tee"), "ksesi"));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "ny"), "ksesi"));
    } else if lemma.ends_with("as") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 1, &harmonize("a", front(lemma))), "ksesi"));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 2, "du"), "ksesi"));
    } else {
        push(Case::Translative, Number::Singular, Some(Possessive::SecondSingular), format!("{}{}", stem(lemma, 0, ""), "ksesi"));
    }
    if lemma.ends_with("nti") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("niksensa", front(lemma))), ""));
    } else if lemma.ends_with("rta") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("raksensa", front(lemma))), ""));
    } else if lemma.ends_with("ätö") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "döksensä"), ""));
    } else if lemma.ends_with("uku") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("vuksensa", front(lemma))), ""));
    } else if lemma.ends_with("nta") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("naksensa", front(lemma))), ""));
    } else if lemma.ends_with("lta") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("laksensa", front(lemma))), ""));
    } else if lemma.ends_with("nto") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("noksensa", front(lemma))), ""));
    } else if lemma.ends_with("en") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 3, "seksensä"), ""));
    } else if lemma.ends_with("us") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("deksensa", front(lemma))), ""));
    } else if lemma.ends_with("pä") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "väksensä"), ""));
    } else if lemma.ends_with("ke") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("keeksensa", front(lemma))), ""));
    } else if lemma.ends_with("in") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "meksensä"), ""));
    } else if lemma.ends_with("tä") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "däksensä"), ""));
    } else if lemma.ends_with("te") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("teeksensa", front(lemma))), ""));
    } else if lemma.ends_with("ty") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, "nyksensä"), ""));
    } else if lemma.ends_with("ys") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, "deksensä"), ""));
    } else if lemma.ends_with("as") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 1, &harmonize("aksensa", front(lemma))), ""));
    } else if lemma.ends_with("tu") {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 2, &harmonize("duksensa", front(lemma))), ""));
    } else {
        push(Case::Translative, Number::Singular, Some(Possessive::Third), format!("{}{}", stem(lemma, 0, "kseen"), ""));
    }
    drop(stem);
    drop(double);
    drop(front);
    drop(harmonize);
    drop(push);
    out
}
