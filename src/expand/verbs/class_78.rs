//! Class 78 (the `kaikaa` type) inflection, written by hand.
//!
//! The type row is class 77's with one cell fewer, and the cell it loses is the
//! past's:
//!
//! ```text
//! 77 (vaillinaisesti taipuvia verbejä; vain joitakin kolmannen persoonan muotoja)
//! kumajaa
//! kumaji        <- the past, and the cell class 78's row does not have
//! kumajaisi
//!
//! 78 (vaillinaisesti taipuvia verbejä; vain joitakin kolmannen persoonan muotoja)
//! kaikaa
//! kaikaisi
//! ```
//!
//! Same *vaillinaisesti taipuvia* note, same shape, and the two types turn out
//! to differ by exactly that. Read the member list off the reference and the
//! shape is identical form for form: 32 members against class 77's 3, all
//! `-aa`/`-ää`, none gradating, none given a second reading, every one of them
//! with **652** `union` forms where class 77 has **654**. Strip the case, number
//! and possessive out of each reading and class 78 leaves 21 reading families;
//! class 77 leaves those same 21 and two more, the past's third persons:
//!
//! ```text
//! [VOICE=ACT][MOOD=INDV][TENSE=PAST][PERS=SG3]   kumaji
//! [VOICE=ACT][MOOD=INDV][TENSE=PAST][PERS=PL3]   kumajivat
//! ```
//!
//! Those two are written here in class 77's own member, because the reference
//! gives them nowhere else: **not one of class 78's 32 members has a past form
//! at all** -- zero, across the whole class, where class 77's three members
//! (`kumajaa`, `heläjää`, `vipajaa`) each have these same two and nothing
//! else. So this type is class 77's type with its past taken away, and the
//! module is that: one delegation rather than a second copy of four noun
//! declensions and a conditional. `expand::nominals` sets the precedent for a
//! variant class delegating to the type it is a variant of (`class_10J` and its
//! five siblings to `class_10`, `class_18` to `class_17`), and the evidence
//! above is what licenses it here.
//!
//! That the past is *absent* rather than *unknown* is worth being clear about,
//! because it is the one place where reading the reference could easily be read
//! the other way. The `both` coverage of the two types differs sharply -- 28 of
//! class 78's 32 members have no `both` row at all, three have 13 (`hohkaa`,
//! `kapsaa`, `rapsaa`) and `kaikaa`, the type row's own example, has exactly the
//! one that `kumajaa` has for class 77 -- so a thin `both` set is this type's
//! normal state rather than evidence of anything. What settled it was the
//! 652-against-654 count agreeing across all 32 members, plus the zero past
//! forms in every one of them; the `both` set was never the evidence.

#![allow(dead_code)]

use crate::tags::Form;

/// Inflect a class 78 verb, which is a class 77 verb without its past.
pub fn inflect(lemma: &str) -> Vec<Form> {
    super::class_77::inflect_without_past(lemma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::verbs::class_77;
    use crate::expand::verbs::utility::front;

    /// The class's 32 members, as the reference lists them. Kept whole because
    /// the claim this module makes is a claim about the class rather than about
    /// one lemma, and checking a sample of it would not be the claim.
    const MEMBERS: [&str; 32] = [
        "helskää",
        "hohkaa",
        "huiskaa",
        "jyskää",
        "kaikaa",
        "kalkkaa",
        "kapsaa",
        "kilkkaa",
        "kolkkaa",
        "kopsaa",
        "loksaa",
        "läpsää",
        "moikaa",
        "mäikää",
        "mäiskää",
        "naksaa",
        "napsaa",
        "niksaa",
        "paukkaa",
        "puhkaa",
        "pärskää",
        "raikaa",
        "raksaa",
        "rapsaa",
        "roikaa",
        "ruskaa",
        "ryskää",
        "räikää",
        "sykkää",
        "telmää",
        "tuikkaa",
        "ähkää",
    ];

    fn forms(lemma: &str) -> Vec<String> {
        inflect(lemma).into_iter().map(|f| f.word).collect()
    }

    fn has(lemma: &str, word: &str) -> bool {
        forms(lemma).iter().any(|w| w == word)
    }

    /// A member of each harmony, from the two ends of the class's own list.
    /// Eleven of the 32 are front-vowel (`helskää`, `jyskää`, `läpsää`, ...).
    const BACK: &str = "hohkaa";
    const FRONT: &str = "helskää";

    #[test]
    fn the_present_has_its_two_third_persons_and_no_others() {
        for (lemma, third_sg, third_pl, stem) in [
            (BACK, "hohkaa", "hohkavat", "hohka"),
            (FRONT, "helskää", "helskävät", "helskä"),
        ] {
            assert!(has(lemma, third_sg), "missing {third_sg}");
            assert!(has(lemma, third_pl), "missing {third_pl}");
            // The four persons this type does not inflect, which class 77 does
            // not have either -- the class-77 tests say so for `kumajaa`.
            for person in ["n", "t", "mme", "tte"] {
                assert!(
                    !has(lemma, &format!("{stem}{person}")),
                    "invented {stem}{person}"
                );
            }
        }
    }

    #[test]
    fn there_is_no_past_at_all() {
        // Class 77's `kumaji` and `kumajivat` written on these members' stems.
        for (lemma, sg, pl) in [
            (BACK, "hohki", "hohkivat"),
            (FRONT, "helski", "helskivät"),
            // A member whose stem would make the past's two collide with
            // something real, so the absence is the past's and not luck.
            ("jyskää", "jyski", "jyskivät"),
        ] {
            assert!(!has(lemma, sg), "invented {sg}");
            assert!(!has(lemma, pl), "invented {pl}");
        }
        // The present's own third person is still there, which is what makes
        // the past's absence a fact about the type and not a broken stem.
        assert!(has(BACK, "hohkaa"));
        assert!(has(FRONT, "helskää"));
        assert!(has("jyskää", "jyskää"));
    }

    #[test]
    fn the_conditional_has_all_six_persons_and_is_the_only_negative() {
        for (lemma, cond, cond_pl) in [
            (BACK, "hohkaisi", "hohkaisivat"),
            (FRONT, "helskäisi", "helskäisivät"),
        ] {
            for word in [
                cond.to_string(),
                format!("{cond}n"),
                format!("{cond}t"),
                format!("{cond}mme"),
                format!("{cond}tte"),
                cond_pl.to_string(),
            ] {
                assert!(has(lemma, &word), "missing {word}");
            }
            for neg in ["en", "et", "ei", "emme", "ette", "eivät"] {
                assert!(has(lemma, &format!("{neg} {cond}")), "missing {neg} {cond}");
            }
            // The other two moods' negatives, which neither type has.
            let stem = class_77::stem_of(lemma);
            assert!(!has(lemma, &format!("en {stem}ne")), "invented potential");
            assert!(
                !has(lemma, &format!("en {lemma}")),
                "invented negative present"
            );
        }
    }

    #[test]
    fn one_cell_of_each_of_the_nominal_families_and_the_conditional() {
        // The shortest form of each family the type has, in both harmonies. A
        // sample rather than an inventory, because the inventory is class 77's
        // and its tests are exhaustive; what is worth pinning here is that the
        // delegation reaches all of it, harmonies included. `helskää` is a
        // front member, so the front rows carry its own `-mä`/`-tön` where
        // `hohkaa` has `-ma`/`-ton`.
        let families: &[(&str, &[&str])] = &[
            // the A infinitive, which is the lemma's own two cells
            (BACK, &["hohkaa"]),
            (FRONT, &["helskää"]),
            // the MA infinitive, six cases
            (
                BACK,
                &["hohkamaan", "hohkamassa", "hohkamasta", "hohkamatta"],
            ),
            (BACK, &["hohkamalla", "hohkaman"]),
            (FRONT, &["helskämään", "helskämässä", "helskämästä"]),
            (FRONT, &["helskämättä", "helskämällä", "helskämän"]),
            // the MINEN infinitive
            (
                BACK,
                &["hohkaminen", "hohkamista", "hohkamisin", "hohkamisen"],
            ),
            (FRONT, &["helskäminen", "helskämistä", "helskämisiä"]),
            // the MAISILLA infinitive, which is possessives only -- see the
            // test below for why the bare form is not among them
            (BACK, &["hohkamaisillani"]),
            (FRONT, &["helskämäisilläni"]),
            // the VA participle
            (BACK, &["hohkava", "hohkavan", "hohkavia"]),
            (FRONT, &["helskävä", "helskävän", "helskäviä"]),
            // the AGENT participle, whose stem is the A infinitive's
            (BACK, &["hohkama", "hohkaman", "hohkamaa", "hohkamin"]),
            (FRONT, &["helskämä", "helskämän", "helskämää"]),
            // the NEG participle, where the `o` is gone in the front
            (BACK, &["hohkamaton", "hohkamattoman", "hohkamatonta"]),
            (FRONT, &["helskämätön", "helskämätöntä"]),
            // and the conditional, which the type has in all six persons
            (BACK, &["hohkaisi", "hohkaisin", "hohkaisit"]),
            (BACK, &["hohkaisimme", "hohkaisitte", "hohkaisivat"]),
            (
                FRONT,
                &["helskäisi", "helskäisin", "helskäisimme", "helskäisivät"],
            ),
        ];
        for (lemma, family) in families {
            for word in *family {
                assert!(has(lemma, word), "{lemma}: missing {word}");
            }
        }
    }

    #[test]
    fn the_fifth_infinitive_takes_only_possessives() {
        // `VerbBuilder::possess` never pushes the bare form, which is why the
        // reference has `kumajamaisillani` and no `kumajamaisilla`. Class 78
        // inherits that, so it is worth pinning here as well as there.
        assert!(has(BACK, "hohkamaisillani"));
        assert!(!has(BACK, "hohkamaisilla"));
    }

    #[test]
    fn only_the_infinitives_take_a_mood_and_nothing_nominal_takes_a_person() {
        // The participial and infinitive readings are the type's only ones with
        // a case, so that is how this module finds them -- `Tags` has no
        // participle field, and class 77's tests say the same thing about the VA
        // participle's readings. Of those, the infinitives are *meant* to take
        // `Mood::Infinitive`; the three participles are not, which is the half
        // worth asserting here. No case-bearing reading takes a person either
        // way. The present's two third-person cells and the conditional's six
        // are left out: they are meant to carry a person, and the lemma word
        // `hohkaa` carries three of its own.
        let mut participial: Vec<String> = Vec::new();
        for lemma in MEMBERS {
            for form in inflect(lemma) {
                for t in &form.readings {
                    if t.case.is_none() {
                        continue;
                    }
                    assert_eq!(t.person, None, "{lemma}: {} takes a person", form.word);
                    if t.mood.is_none() {
                        participial.push(form.word.clone());
                    } else {
                        assert_eq!(
                            t.mood,
                            Some(crate::tags::Mood::Infinitive),
                            "{lemma}: {} takes a mood that is not the infinitive's",
                            form.word
                        );
                    }
                }
            }
        }
        // The three participles are the readings found with no mood, and they
        // are found for every member -- 32 of each, the VA one of which is
        // visible in the count below rather than named.
        assert_eq!(participial.len() % MEMBERS.len(), 0);
        let per_member = participial.len() / MEMBERS.len();
        assert!(
            per_member > 100,
            "only {per_member} unmooded nominal readings per member"
        );
    }

    #[test]
    fn every_member_is_class_77s_cells_with_the_past_taken_away() {
        // The class-level claim, checked on all 32 members rather than on a
        // sample: each one's cell set is class 77's with the past's two third
        // persons gone and nothing else changed. Comparing *endings* rather
        // than words is what makes two different lemmas comparable at all --
        // `hohkava` and `kumajava` are the same cell on two stems.
        // Class 77 has one member of each harmony (`kumajaa`, `heläjää`), and
        // the harmony has to match: `kumajamassa` and `helskämässä` are the same
        // cell on two different stems only after the vowel is agreed, so a
        // back-vowel member is compared against the back-vowel reference and a
        // front one against the front. Two references, not 32.
        // The two cells the flag removes, in each harmony: the past's two third
        // persons. The plural drops the theme vowel too, so it is `ivat` and
        // `ivät` -- `hohkavat` and `hohkivat` are different cells, which is why
        // the present's plural is in the type and the past's is not.
        let reference = [("kumajaa", ["i", "ivat"]), ("heläjää", ["i", "ivät"])];
        for lemma in MEMBERS {
            let (ref_lemma, past) = reference
                .iter()
                .find(|(r, _)| front(r) == front(lemma))
                .expect("both harmonies have a class 77 member");
            let theirs = endings(ref_lemma, class_77::inflect(ref_lemma));
            let mine = endings(lemma, inflect(lemma));
            let dropped: Vec<&String> = theirs.iter().filter(|e| !mine.contains(e)).collect();
            assert_eq!(
                dropped,
                past.to_vec(),
                "{lemma}: differs from class 77 by more than the past"
            );
            let extra: Vec<&String> = mine.iter().filter(|e| !theirs.contains(e)).collect();
            assert!(extra.is_empty(), "{lemma}: adds {extra:?} of its own");
            // The count follows from the two being equal-but-for-two, but the
            // reference is 654 and the type 652, so say so out loud.
            assert_eq!(mine.len() + 2, theirs.len(), "{lemma}");
        }
    }

    /// A member's cells as endings on its own stem, so that two members can be
    /// compared cell for cell despite spelling different words. The past's two
    /// cells sit on the stem less its theme vowel (`hohk` beside `hohka`), so
    /// that stem is tried too -- which is what lets `i` and `vat` come back as
    /// the only two endings class 77 has and a class-78 member does not.
    fn endings(lemma: &str, forms: Vec<Form>) -> Vec<String> {
        let present = class_77::stem_of(lemma);
        let past = class_77::stem_of(&present);
        forms
            .into_iter()
            .map(|f| {
                // The six negative phrases are one pronoun plus a cell, and the
                // pronoun is the only part that is not the member's, so it
                // comes off first: `en heläjäisi` and `en helskäisi` are the
                // same cell, `en ` + `isi`.
                let bare = NEGATIVES
                    .iter()
                    .find_map(|n| f.word.strip_prefix(&format!("{n} ")))
                    .unwrap_or(&f.word);
                bare.strip_prefix(&present)
                    .or_else(|| bare.strip_prefix(&past))
                    .unwrap_or(bare)
                    .to_string()
            })
            .collect()
    }

    /// The six negative pronouns, which the type's only negative phrases use.
    const NEGATIVES: [&str; 6] = ["en", "et", "ei", "emme", "ette", "eivät"];
}
