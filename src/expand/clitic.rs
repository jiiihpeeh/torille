//! Clitics: post-inflectional particles attached to a complete form.
//!
//! A clitic is not a case or a derivational suffix. It attaches to the end of
//! an already-inflected word and changes its discourse role: `aamullakin` is
//! the adessive `aamulla` with a `-kin` on it, still an adessive. Five are
//! covered here: `-ko`/`-kö`, `-kin`, `-kaan`/`-kään`, `-han`/`-hän` and
//! `-pa`/`-pä`.
//!
//! The a/ä alternation follows the host's vowel harmony, with the exception of
//! `-kin`, which is invariant. A host with only neutral vowels (`e`, `i`) or
//! only `y` counts as front-harmonic, so `vene` takes the front variant.
//!
//! A clitic does not trigger consonant gradation: `pankissa` with `-kin` is
//! `pankissakin`, not a second weakening. Cliticization comes after
//! inflection and possession in the pipeline.

use crate::tags::{Clitic, Form};

/// The five clitics, in the order a form can stack them.
///
/// Finnish permits more than one clitic on a host (`-kin` and `-han` occur
/// together, for instance), and the stack is not a free permutation. This is
/// the order the clitics are appended in; the exact syntactic restrictions are
/// a matter for a caller that is generating rather than analysing.
pub const CLITICS: [Clitic; 5] = [
    Clitic::Ko,
    Clitic::Kaan,
    Clitic::Han,
    Clitic::Pa,
    Clitic::Kin,
];

/// The clitic suffix for `clitic` under `host`'s harmony.
///
/// `-kin` never changes; the rest take their front or back form from the host.
/// The host is already fully inflected, so its harmony is fixed by then.
fn suffix(clitic: Clitic, front: bool) -> &'static str {
    match clitic {
        Clitic::Ko => {
            if front {
                "kö"
            } else {
                "ko"
            }
        }
        Clitic::Kin => "kin",
        Clitic::Kaan => {
            if front {
                "kään"
            } else {
                "kaan"
            }
        }
        Clitic::Han => {
            if front {
                "hän"
            } else {
                "han"
            }
        }
        Clitic::Pa => {
            if front {
                "pä"
            } else {
                "pa"
            }
        }
    }
}

/// Whether the host is front-harmonic, reading `e` and `i` as front and `y`
/// likewise.
///
/// This is the reading the clitics want: a host with only neutral vowels takes
/// the front variant, so `vene` is `venekin` (invariant) but `vene` + `-ko`
/// would be `venekö`.
fn is_front(word: &str) -> bool {
    crate::grammar::harmony(word) == crate::grammar::Harmony::Front
}

/// Attaches `clitic` to a complete inflected `form`, returning a new form.
///
/// The clitic goes on the end of the word and is recorded on the tags, so the
/// result is still recognisably the same case with a clitic on it. The
/// confidence is inherited from the host: a clitic on a plain form is plain.
pub fn cliticize(form: &Form, clitic: Clitic) -> Form {
    let mut word = form.word.clone();
    word.push_str(suffix(clitic, is_front(&form.word)));
    let tags = form.union().clitic(clitic);
    Form::new(word, tags).with(form.confidence)
}

/// Attaches a clitic to every form in `forms`.
pub fn cliticize_all(forms: &[Form], clitic: Clitic) -> Vec<Form> {
    forms.iter().map(|f| cliticize(f, clitic)).collect()
}

/// Attaches every single clitic to a form, one form per clitic.
pub fn all_clitics(form: &Form) -> Vec<Form> {
    CLITICS.iter().map(|c| cliticize(form, *c)).collect()
}

/// The two surface variants of a clitic, `(front, back)`.
///
/// `-kin` is invariant, so its variants are both `"kin"`. The other clitics
/// follow the host's vowel harmony (`-ko`/`-kö`, `-han`/`-hän`, etc.).
fn clitic_variants(c: Clitic) -> (&'static str, &'static str) {
    match c {
        Clitic::Ko => ("kö", "ko"),
        Clitic::Kin => ("kin", "kin"),
        Clitic::Kaan => ("kään", "kaan"),
        Clitic::Han => ("hän", "han"),
        Clitic::Pa => ("pä", "pa"),
    }
}

/// Strip a single clitic from the end of `word`, returning the host.
///
/// Returns `None` if `word` does not end in a recognised clitic suffix.
/// The caller has to verify the host is a real word -- this function
/// only does the surface shape. Trying every variant (front + back, and
/// both possible spellings) is the right call because the clitic's
/// harmony is fixed by the host's vowels, which we don't know without
/// already having recognised the host.
///
/// Clitics in the same form can stack, e.g. `siellähänkin` is `siellä`
/// plus `-hän` plus `-kin`. Stripping one layer at a time is what the
/// spell-check path needs; `strip_all_clitics` peels the full stack.
pub fn strip_clitic(word: &str) -> Option<(String, Clitic)> {
    for c in CLITICS {
        for suf in [clitic_variants(c).0, clitic_variants(c).1] {
            if let Some(host) = word.strip_suffix(suf)
                && !host.is_empty()
            {
                return Some((host.to_string(), c));
            }
        }
    }
    None
}

/// Strip every layer of clitics from the end of `word`.
///
/// Finnish permits `ko/kin/kaan/han/pa` to stack, in that order, so
/// `siellähänkin` peels to `(siellä, [Han, Kin])`. Each step uses
/// [`strip_clitic`], which only checks the surface; the caller is
/// expected to verify the host is a real word.
pub fn strip_all_clitics(word: &str) -> (String, Vec<Clitic>) {
    let mut host = word.to_string();
    let mut stack = Vec::new();
    while let Some((h, c)) = strip_clitic(&host) {
        host = h;
        stack.push(c);
    }
    (host, stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{PartOfSpeech, Tags};

    fn form(word: &str) -> Form {
        Form::new(word.to_string(), Tags::UNKNOWN.of_pos(PartOfSpeech::Noun))
    }

    #[test]
    fn kin_is_invariant() {
        assert_eq!(cliticize(&form("aamu"), Clitic::Kin).word, "aamukin");
        assert_eq!(cliticize(&form("tämä"), Clitic::Kin).word, "tämäkin");
    }

    #[test]
    fn ko_follows_harmony() {
        assert_eq!(cliticize(&form("talo"), Clitic::Ko).word, "taloko");
        assert_eq!(cliticize(&form("tämä"), Clitic::Ko).word, "tämäkö");
    }

    #[test]
    fn kaan_han_pa_follow_harmony() {
        assert_eq!(cliticize(&form("talo"), Clitic::Kaan).word, "talokaan");
        assert_eq!(cliticize(&form("tämä"), Clitic::Kaan).word, "tämäkään");
        assert_eq!(cliticize(&form("talo"), Clitic::Han).word, "talohan");
        assert_eq!(cliticize(&form("tämä"), Clitic::Han).word, "tämähän");
        assert_eq!(cliticize(&form("talo"), Clitic::Pa).word, "talopa");
        assert_eq!(cliticize(&form("tämä"), Clitic::Pa).word, "tämäpä");
    }

    #[test]
    fn a_neutral_host_takes_the_front_variant() {
        // `vene` has only neutral vowels, so it is front-harmonic for a clitic.
        assert_eq!(cliticize(&form("vene"), Clitic::Ko).word, "venekö");
    }
}
