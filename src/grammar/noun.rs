//! The noun cases the catalogue leaves out.
//!
//! The catalogue gives eight forms per noun class. Finnish has fourteen, and
//! the missing ones are far commoner than the eight: the inessive alone appears
//! in almost every sentence. They are built here from the stem rather than
//! looked up, which is what keeps the dictionary to a list of lemmas.
//!
//! The cases are built from a class's stem readings, and a class holds words
//! that disagree, so the set is a superset of the correct forms and holds
//! spellings that are not words: class 7 covers both `ovi` and `joki`, and
//! `ovessa` and `joessa` are what it has to say about the same slot. Every
//! candidate is a word to check against, but only the one the class is certain
//! of may be offered as a correction, so each candidate is judged twice: is the
//! reading one this lemma has (see [`readings_for`]), and does the ending answer
//! the stem it was built on (see [`Licence`]). What survives both is `Plain`,
//! and a slot two certain readings compete for is left to be guessed at.

use crate::case::{StemGroup, noun_cases};
use crate::kotus::{ClassId, Gradation};
use crate::tags::{Case, Confidence, Form, Number, Tags};

use super::harmony::{Harmony, reading};
use super::stem::{S, Stem, apply_stem_of, harmony_stem_of, noun_class, plain};
use crate::is_vowel;

/// Which spelling of the inessive and the elative is meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// `-ssa` and `-stä`, on the stem as it stands: `talossa`, `talosta`.
    Ssa,
    /// `-essa` and `-estä`, for a stem that ends in `s` or `n`: `vastauksessa`.
    Essa,
    /// `-issa` and `-istä`, which is the plural, and whose `i` goes between the
    /// stem and the ending: `taloissa`, `joissa`.
    Issa,
}

/// The remaining noun cases, built from a lemma and its class.
///
/// A class offers one or more stem readings, and Finnish does not use the same
/// one for every case, so the readings are sorted into the two groups the cases
/// actually need. Every candidate stem is tried for every case and the set is a
/// superset of the correct forms, holding some that are not words: a class whose
/// members disagree, as class 7 does with `ovi` and `joki`, cannot say which of
/// them a rule is for. The extra forms are guesses, so they are never offered as
/// suggestions.
pub fn oblique(
    lemma: &str,
    class: ClassId,
    gradation: Option<Gradation>,
    plural_stem: &str,
    plural_strong: &str,
    is_model: bool,
) -> Vec<Form> {
    let mut out = Vec::with_capacity(96);
    // Classes outside the table behave like the unmarked first one, which is
    // what most of the nominal classes not listed here do.
    let desc = noun_class(class).unwrap_or(plain(S));
    // What the lemma's own spelling settles about its harmony, which is the
    // half of the certainty question the letters can answer. A reader that is
    // not the one this says is a reader of some other word.
    let decided = reading(lemma);
    let sure_of_harmony = |h: Harmony| decided == Some(h);
    // Which way a stem harmonises, and how much is actually known about it.
    //
    // A stem the class made with a rule harmonises one way, and the way is
    // settled by the stem before the rule touched it: `lapsi` is `lapsessa`
    // because the `e` the class adds to make `lapse` is a theme vowel and a
    // theme vowel is never stressed. A stem that is the lemma or a literal
    // spelling standing in for the class harmonises both ways, because the
    // spelling is not evidence: `paperi` has an `e` in it and is nonetheless a
    // back-harmony word, and only a corpus can say so. Both are generated, and
    // the one that is not a word is a guess and is never suggested.
    let harmonies = |spec: Stem| -> Vec<Harmony> {
        // A fixed stem of the lemma's own is the lemma's own stem, and the
        // vowel it ends in settles the harmony as it does everywhere else:
        // `lämmin`'s `lämpimä` is `lämpimillä` and not the `lämpimäillä` the
        // other side of the split would give it, which is a singular stem with
        // a plural ending and no `i` between them.
        if matches!(spec, Stem::Fixed(_)) && is_model {
            // The vowel the stem ends in settles the harmony when it is one of
            // the deciding pair (`lämmin`'s `lämpimä` is front). A neutral `e`
            // on the end decides nothing (`tuhat`'s essive stem `tuhante` has
            // its `a` hidden), so both sides are generated.
            let stem = apply_stem_of(spec, lemma);
            return match reading(&stem) {
                Some(Harmony::Back) => vec![Harmony::Back],
                Some(Harmony::Front) => vec![Harmony::Front],
                None => vec![Harmony::Back, Harmony::Front],
            };
        }
        if spec.is_literal() {
            return vec![Harmony::Back, Harmony::Front];
        }
        // A cut stem harmonises by the vowel the class's rule leaves in it. When
        // that vowel does not settle the question — a neutral `e` sits between
        // it and the end, as in the `aske` of `askel` — the spelling is no
        // evidence and both sides are generated, a guess among them, exactly as
        // a literal stem is treated.
        let stem = harmony_stem_of(spec, lemma);
        if reading(&stem) == Some(Harmony::Back) {
            vec![Harmony::Back]
        } else if reading(&stem) == Some(Harmony::Front) {
            vec![Harmony::Front]
        } else {
            vec![Harmony::Back, Harmony::Front]
        }
    };
    let stem_of = |spec: Stem| {
        let stem = apply_stem_of(spec, lemma);
        // A class with no rule at all is one where the lemma is the stem, which
        // is most of them: `talo`, `kala` and `risti` take their endings to the
        // whole word.
        let literal_lemma = desc.genitive.is_none() && desc.stems.iter().all(|s| s.is_literal());
        if stem.is_empty() && !literal_lemma {
            None
        } else {
            Some(stem)
        }
    };
    // The weak stem, which is the genitive stem with gradation applied. The
    // oblique cases that are weak (`-ssa`, `-lla`, `-lta`, `-lle`, `-ksi`)
    // build on it: `pankki` is `pankissa` and `pankilla`, not `pankkissa` and
    // `pankkilla`.
    //
    // A class that fixes its stem has no weak grade to find. The flag belongs
    // to the lemma and the stem is the class's own spelling, so the site is not
    // the flag's site and the two have nothing to do with one another: `lämmin`
    // is `35*H` and the class's stem is `lämpimä`, whose `mp` the flag would
    // otherwise have found, giving the `lämmimä` of `lämmimän` and `lämmimät`.
    // The real weak reading of that class is the spelling itself, `lämpimä` in
    // every case, which is what the grade is here.
    let gradation = gradation.filter(|_| !desc.fixes_stem());
    let weak_stem_of = |stem: &str| -> String {
        match gradation {
            Some(g) if g.site(stem).is_some() => g.apply(stem, crate::kotus::Grade::Weak),
            _ => stem.to_string(),
        }
    };
    // The same, for a stem the class names outright. A fixed stem is a spelling
    // rather than a cut of the lemma even where the lemma's flag does apply to
    // the class as a whole, so the spelling is already the grade the case wants.
    let weak_of = |spec: Stem, stem: &str| -> String {
        match spec {
            Stem::Fixed(_) => stem.to_string(),
            Stem::Rule(_) => weak_stem_of(stem),
        }
    };
    // The catalogue's plural stem, as the caller reads it out of the plural
    // partitive: `kala` reads `kalo` out of `kaloja`, `joki` reads `jo` out of
    // `jokia` on the weak grade, and the plural hangs its `i` off that rather
    // than off the singular stem. Its strong sibling is the same stem before
    // the gradation: `jok` beside `jo`, `laatikko` beside `laatiko`. The weak
    // stem feeds the local cases and the instructive — `joissa`, `joiksi`,
    // `laatikoissa`, `laatikoin` — and the strong one feeds the essive and the
    // comitative, which are strong-grade cases: `jokina`, `jokineen`,
    // `laatikkoina`, `laatikkoineen`.
    //
    // The plural stem is not put through the singular's gradation, because it
    // is not a grade of the singular stem to begin with — it is read out of a
    // plural partitive form, and that form and the singular rule draw the grade
    // differently. The caller is the one that has the two to offer; here the
    // weak one is kept as it stands and asked about, and the form is judged
    // against the stem it hangs off (the weak one for a local case, the strong
    // one for the essive and the comitative).
    let plural_weak = plural_stem.to_string();
    // Whether the class reaches the plural stem, which is what makes a plural
    // form certain. The class's own stem is either the plural stem, or the
    // plural stem with the theme vowel the class's stem ends in: `talo` keeps
    // its `o` and is `talo` in the plural, and the `lämpimä` of class 35 is
    // `lämpim` in the plural, where that `ä` is the theme vowel and the plural
    // drops it — `lämpimät`, `lämpimillä` and `lämpimin` are the word's, and a
    // test that wanted the two stems to be equal would offer none of them.
    //
    // A stem the class cannot reach that way is a stem read out of a word
    // rather than one the class named: `kala` reads `kalo` out of `kaloja`,
    // and the forms on it are guesses however right they look.
    //
    // The stem judged is the one the form in hand hangs off: the weak grade for
    // the local cases and the instructive, the strong grade for the essive and
    // the comitative. `laatikko`'s own stem is `laatikko`, which reaches the
    // strong `laatikko` and not the weak `laatiko`, so its essive is `laatikkoina`.
    let reaches_plural = |stem: &str, plural: &str| -> bool {
        stem == plural
            || (stem.starts_with(plural)
                && stem.chars().count() == plural.chars().count() + 1
                && stem[plural.len()..].chars().all(is_vowel))
            // Class 9 changes its final `a` to an `o` in the plural: `kala` is
            // `kalo`, so `kala` reaches `kaloja`, `kaloissa`, `kaloin` and
            // `kaloineen` and those are the word's own forms, not guesses.
            || (class == 9
                && stem.ends_with('a')
                && plural.ends_with('o')
                && stem
                    .chars()
                    .take(stem.chars().count() - 1)
                    .eq(plural.chars().take(plural.chars().count() - 1)))
            // The genitive stem and the plural stem of one word can diverge while still
            // sharing the word's body: `kynsi` is `kynne` in the genitive and
            // `kyns` in the plural, and `vesi` is `vede` and `ves`. When they
            // share all but the last two letters and the genitive is
            // vowel-final, the plural is the word's own and not a guess.
            || {
                let s: Vec<char> = stem.chars().collect();
                let p: Vec<char> = plural.chars().collect();
                let max = s.len().max(p.len());
                let common = s.iter().zip(p.iter()).take_while(|(a, b)| a == b).count();
                max >= 4
                    && common >= max - 2
                    && common >= 2
                    && s.last().is_some_and(|c| is_vowel(*c))
                    && p[common..].iter().any(|c| !is_vowel(*c))
            }
    };
    let reaches_weak = |stem: &str| reaches_plural(stem, plural_stem);
    let reaches_strong = |stem: &str| reaches_plural(stem, plural_strong);

    // The cases that end in `-lla`, `-lta`, `-lle` and `-na` are built on the
    // genitive stem. `käsi` is `kädellä` and `joki` is `joella`, and neither is
    // `käsellä` or `jokella`, so where a class names a genitive stem these
    // cases use it and the class's own readings are left to the cases that want
    // them.
    let genitive_specs: Vec<Stem> = match desc.genitive {
        Some(readings) => readings
            .iter()
            .copied()
            // A spelling the class names is the stem the endings go on, so a
            // literal one (`sisimmä` for `sisin`) is kept; only the lemma's own
            // keep-rule reading is dropped, since that is where the word starts
            // rather than where the ending goes.
            .filter(|s| !matches!(s, Stem::Rule(r) if r.drop == 0 && r.sub.is_none() && r.add.is_none()))
            .collect(),
        None => desc.stems.to_vec(),
    };
    // A class can offer more than one genitive stem when it holds words that
    // disagree: class 7 holds `ovi` and `joki`, whose genitive stems are `ove`
    // and `joe`. The sanalista's gradation flag is what tells which one a given
    // lemma is. `joki` is `7*D`, where the `k` gradates away, and that is the
    // reading that cuts the deeper of the two (`joe`, dropping two letters);
    // `ovi` is a plain `7` with no gradation, and takes `ove` (`drop` one).
    // When a gradation flag is present, keep only the reading that drops the
    // most; when it is absent, keep the ones that drop the least.
    let genitive_specs = choose_genitive_stems(genitive_specs, gradation);
    // Whether the class has settled on one reading for these cases, and which
    // one it is. A class that offers two has not: whichever of them the endings
    // are put on, the other member's form comes out as well, and the two are
    // caught by `demote_contested` below if the class cannot be read at all. A
    // class that lists the lemma beside its rule has the rule, and the forms
    // built from the fallback entry are guesses however right they look: that
    // is where `lapsi` would offer `lapsilla` beside the `lapsella` the class
    // means.
    let genitive_readings = readings_for(&genitive_specs, lemma, is_model);
    for spec in genitive_specs {
        // A `ks` reading is the reading of the `-us`/`-ys`/`-nas` nouns alone,
        // so it is not a reading of the members that do not end that way. The
        // rule itself says which ending it belongs to (see the `US` table),
        // and a lemma outside it keeps the other readings: an `aidas` in
        // class 39 answers with the lemma as its stem, and `ananasella` on
        // the bare lemma is a form of no word at all.
        if matches!(
            spec,
            Stem::Rule(r) if r.add.is_some_and(|a| a == "kse" || a == "nakse" || a == "näkse")
        ) && !rule_applies(spec, lemma, is_model)
        {
            continue;
        }
        let this_reading = forced_reading(&genitive_readings, spec);
        let Some(mut genitive) = stem_of(spec) else {
            continue;
        };
        // A reverse-gradation word already carries the weak grade in its
        // genitive stem (`hammas` is `hammaa`, `lammas` is `lammaa`), so its
        // oblique is strong throughout: strongen the stem and do not weaken
        // it. `pankki` has the strong `kk` and is unaffected.
        let reverse =
            gradation.is_some_and(|g| g.weak_site(lemma).is_some() && g.site(lemma).is_none());
        if let Some(g) = gradation.filter(|_| reverse) {
            genitive = g.strongen(&genitive);
        }
        // The weak grade of the genitive stem, for the oblique cases that are
        // weak: `pankki` is `pankissa` and `pankilla` on `pank`, not `pankkissa`
        // and `pankkilla` on `pankk`.
        let weak_genitive = if reverse {
            genitive.clone()
        } else {
            weak_of(spec, &genitive)
        };
        // The two stems this group's endings are judged against, and the two
        // facts about how the reading was made: a class that substituted a
        // consonant has a weak stem that the strong-grade cases cannot use, and
        // a class whose inflection gradated the stem has one the weak and the
        // strong differ on.
        let strong = shape(&genitive, spec, plural_stem);
        let weak = shape(&weak_genitive, spec, plural_stem);
        for h in harmonies(spec) {
            let o = h.o();
            // The reading is a candidate either way, since a class that has not
            // settled on one is still the class the word is in, but it is only
            // a correction when the class has settled and the ending answers
            // the stem it was built on.
            let force = this_reading && sure_of_harmony(h);
            // The cases here are the singular rows of the case table, each of
            // which names the ending, the grade and the licence that accepts
            // it. The strong-grade cases (the partitive and the essive) are
            // written on the genitive reading, and the weak ones (the three
            // locals, the translative and the abessive) on its weak grade. The
            // inessive and the elative are the stem loop's own down below, so
            // the table is skipped there, and the accusative is not a case that
            // changes the word at all: `talo` is its own accusative, and which
            // of the two the word is being read as is decided by the position
            // it was found in rather than by the spelling.
            //
            // The partitive is tabulated for every class, so the reading here
            // is a reading and never a correction the generator owns: the
            // catalogue's own `kalaa` is the one offered. The genitive and the
            // illative are not here either, for the same reason: the catalogue
            // gives those exactly, per lemma, and `expand` derives them.
            for row in noun_cases() {
                if row.number != Number::Singular {
                    continue;
                }
                let suffix = if h == Harmony::Back {
                    row.back
                } else {
                    row.front
                };
                // A loanword ending in a consonant (class 22, `parfait`) takes
                // an apostrophe between the stem and the ending, `parfait'ssa`.
                let suffix = if class == 22 {
                    format!("'{suffix}")
                } else {
                    suffix.to_string()
                };
                match row.case {
                    Case::Inessive | Case::Elative => continue,
                    Case::Partitive => {
                        push(
                            &mut out,
                            format!("{genitive}{suffix}"),
                            Case::Partitive,
                            false,
                        );
                        push(&mut out, format!("{genitive}a"), Case::Partitive, false);
                    }
                    Case::Essive => push(
                        &mut out,
                        format!("{genitive}{suffix}"),
                        row.case,
                        force
                            && desc.essive.is_none()
                            && licensed(licence_of(row.licence), &strong, lemma),
                    ),
                    Case::Translative | Case::Abessive => {
                        push(
                            &mut out,
                            format!("{weak_genitive}{suffix}"),
                            row.case,
                            force && licensed(licence_of(row.licence), &weak, lemma),
                        );
                        // A stem that ends in `s` or `n` spells these with an
                        // `e` in front, as the local cases do: `tuhann` is
                        // `tuhanneksi` and `tuhannetta`, not `tuhannksi`.
                        if let Some(allomorph) = row.after_s_n
                            && matches!(genitive.chars().last(), Some('s') | Some('n'))
                            && (rule_applies(spec, lemma, is_model) || !spec_is_ks(spec))
                        {
                            let e_suffix = if h == Harmony::Back {
                                allomorph.back
                            } else {
                                allomorph.front
                            };
                            push(
                                &mut out,
                                format!("{weak_genitive}{e_suffix}"),
                                row.case,
                                force && licensed(licence_of(allomorph.licence), &weak, lemma),
                            );
                        }
                    }
                    Case::Adessive | Case::Ablative | Case::Allative => {
                        push(
                            &mut out,
                            format!("{weak_genitive}{suffix}"),
                            row.case,
                            force && licensed(licence_of(row.licence), &weak, lemma),
                        );
                        // A stem that ends in `s` or `n` spells these endings
                        // with an `e` in front, the same allomorphy as the
                        // `-essa` of the inessive: `nais` is `naisella` and
                        // `tuhann` is `tuhannella`, and neither can take the
                        // plain `ll` after the `s` or `n`. The ablative is the
                        // odd one out among the three: it has a single `l` to
                        // begin with, so it takes a single one after the `e` as
                        // well, and `naiselta` it is. This is the allomorphy of
                        // a stem that ends in `s` or `n`, not of a word whose
                        // stem reading still carries the lemma's final `s`: the
                        // `ks` stem of `ananas` is `ananakse`, and `ananasella`
                        // on the bare `ananas` is a form of no word at all.
                        if let Some(allomorph) = row.after_s_n
                            && matches!(genitive.chars().last(), Some('s') | Some('n'))
                            && (rule_applies(spec, lemma, is_model) || !spec_is_ks(spec))
                        {
                            let e_suffix = if h == Harmony::Back {
                                allomorph.back
                            } else {
                                allomorph.front
                            };
                            push(
                                &mut out,
                                format!("{weak_genitive}{e_suffix}"),
                                row.case,
                                force && licensed(licence_of(allomorph.licence), &weak, lemma),
                            );
                        }
                    }
                    _ => {}
                }
            }
            // The prolative is a candidate and never a correction. Whether a
            // noun has one at all is a fact about the word and not about its
            // class: `maitse` and `meritse` and `ilmateitse` are words and
            // `talotse` is nothing, and the class table cannot tell them apart.
            push(
                &mut out,
                format!("{weak_genitive}tse"),
                Case::Prolative,
                false,
            );
            // The `o`-stem readings, which are how a stem whose own final vowel
            // is an `o` reaches an ending that begins with one. No rule of the
            // class licenses either of them, so they are readings and never
            // corrections.
            if h == Harmony::Back {
                let o_forms = force && licensed(Licence::Reading, &strong, lemma);
                push(
                    &mut out,
                    format!("{genitive}ll{o}"),
                    Case::Adessive,
                    o_forms,
                );
                push(&mut out, format!("{genitive}n{o}"), Case::Essive, o_forms);
            }

            // The plural of these cases hangs an `i` off the plural stem, which
            // is the plural partitive without its theme vowel: `taloja` gives
            // `talo`, so `talo` is `taloilla` in the plural adessive, and
            // `kaloja` gives `kalo`, so `kala` is `kaloilla`. The stem is
            // passed in because it is the one thing the class table cannot say:
            // whether the plural keeps or drops the theme vowel is a property
            // of the individual word. `talo` is `taloilla` and `kala` is
            // `kaloilla`, and both are correct — but only the first is the stem
            // the class named, and the second is a stem read out of the
            // catalogue's plural partitive, so only the first is certain.
            //
            // The two stems are the weak and the strong of the same reading:
            // the local cases are weak and hang off the weak stem (`joki` is
            // `jo` and `joilla`, `laatikko` is `laatiko` and `laatikoilla`),
            // and the essive and the comitative are strong-grade cases and keep
            // the strong stem again (`jok`, `laatikko`).
            let plural = format!("{plural_stem}i");
            let plural_strong_i = format!("{plural_strong}i");
            // The catalogue's plural stem is read out of the word, so only a
            // word the class reaches that way is certain of it: `talo` keeps
            // `talo` and is plain, and `kala` reads `kalo` out of `kaloja` and
            // is not. Judging on the plural stem itself would call both plain,
            // and judging on the class stem would call neither.
            //
            // The reading is judged as the plural stem, since that is what these
            // forms are built from, and the class's own stem is what says
            // whether the class reaches it. A licence asked about the genitive
            // stem instead would compare two stems that are equal for `pankki`,
            // whose plural stem is its genitive stem, and unequal for `lämmin`,
            // whose is that stem without its theme vowel — and would have
            // offered the first's plural and turned the second's away.
            let plural_shape = shape(plural_stem, spec, plural_stem);
            let strong_shape = shape(plural_strong, spec, plural_strong);
            let plural_force =
                force && licensed(Licence::Plural, &plural_shape, lemma) && reaches_weak(&genitive);
            let strong_force = force
                && licensed(Licence::Plural, &strong_shape, lemma)
                && reaches_strong(&genitive);
            // The plural rows of the case table own these endings. The local
            // cases and the translative and the abessive pull the weak stem
            // (`joilla`, `joiksi`, `joitta` on `jo`), and the essive and the
            // comitative keep the strong one: the `joki` that the local cases
            // read as `jo` is `jok` here, so the essive is `jokina` and the
            // comitative `jokineen`, while `ovi` is `ovina` and `valo` is
            // `valoina`. The inessive and the elative plurals are built by
            // the stem loop above, which hangs its `i` off the stem the
            // catalogue read out (`joissa`), and the instructive has bare
            // guesses of its own in the block below, so neither is repeated
            // from the table here.
            for row in noun_cases() {
                if row.number != Number::Plural {
                    continue;
                }
                match row.case {
                    Case::Inessive | Case::Elative | Case::Instructive => continue,
                    _ => {}
                }
                let suffix = if h == Harmony::Back {
                    row.back
                } else {
                    row.front
                };
                let strong = row.stem == StemGroup::PluralStrong;
                let base = if strong { &plural_strong_i } else { &plural };
                let force = if strong { strong_force } else { plural_force };
                push_plural(&mut out, format!("{base}{suffix}"), row.case, force);
            }
        }
    }

    // The essive of a class that names a stem for it, which the genitive stem
    // cannot serve: the essive is a strong-grade case and the genitive stem is
    // the weak one, so `käsi` — `käden` in the genitive, `kätenä` in the
    // essive — has no reading of the genitive stem that makes the second. The
    // class's own stem for the case is `käte`, and that is what the ending
    // goes on here.
    //
    // The genitive stem's own `kädenä` is still generated above, since a class
    // that alternates nothing is spelled from both readings the same way, and
    // it is a guess there: the `substituted` clause of the essive licence is
    // what turns it down, and it is right to, because that clause is what tells
    // the weak reading of a class that alternates a consonant from the strong
    // one. The stem named here is the strong one by name, so the ending is
    // judged on the stem as it stands.
    if let Some(essive_specs) = desc.essive {
        let essive_specs: Vec<Stem> = essive_specs
            .iter()
            .copied()
            // The class names these stems for the essive, so a literal spelling
            // (`sisimpä` for `sisin`) is as much the case's own stem as a cut.
            .filter(|s| is_cut(*s) || matches!(s, Stem::Fixed(_)))
            .collect();
        let essive_readings = readings_for(&essive_specs, lemma, is_model);
        let essive_forced = essive_readings.len() == 1;
        for spec in essive_specs {
            if !rule_applies(spec, lemma, is_model) {
                continue;
            }
            let this_reading = essive_forced && essive_readings[0] == spec;
            let Some(stem) = stem_of(spec) else { continue };
            let judged = shape(&stem, spec, plural_stem);
            for h in harmonies(spec) {
                push(
                    &mut out,
                    format!("{stem}n{}", h.a()),
                    Case::Essive,
                    this_reading && sure_of_harmony(h) && licensed(Licence::Bare, &judged, lemma),
                );
            }
        }
    }

    // The inessive is the one case no single stem reading serves. `käsi` is
    // `kädessä`, on the genitive stem, and `joki` is `joessa`, on the short stem
    // its own class gives. Both classes need the inessive tried on each reading
    // they have, and the one that is not the word is a form of some other word.
    //
    // The readings that are only the lemma or a spelling standing in for the
    // class are left out when the class has a real rule, because those are
    // where the word starts rather than where the ending goes: `käsi` would
    // otherwise give the `kässä` that is not a word. A class that says the
    // lemma is a stem in its own right keeps them, which is what gives class 7
    // the `ovissa` of the plural of `ovi`.
    //
    // A `ks` reading is the reading of the `-us`/`-ys`/`-nas` nouns alone, so
    // a lemma that does not end that way is not answered with it here either:
    // an `aidas` answers with the lemma as its stem, and `ananasessa` on the
    // bare lemma is a form of no word at all.
    let usable = |s: &Stem| {
        if matches!(
            s,
            Stem::Rule(r) if r.add.is_some_and(|a| a == "kse" || a == "nakse" || a == "näkse")
        ) && !rule_applies(*s, lemma, is_model)
        {
            return false;
        }
        !s.is_literal() || desc.lemma_is_stem
    };
    let inessive_specs: Vec<Stem> = {
        let mut specs: Vec<Stem> = desc.stems.iter().copied().filter(usable).collect();
        for spec in desc.genitive.into_iter().flatten().copied().filter(usable) {
            if !specs.contains(&spec) {
                specs.push(spec);
            }
        }
        if specs.is_empty() {
            // The class has no rule anywhere, so the lemma is its stem.
            specs = desc.stems.to_vec();
        }
        specs
    };
    // The gradation flag is what tells two members of one class apart when
    // their short stems differ, exactly as it does for the genitive above:
    // `joki` is `7*D` and takes the deeper cut `jo` (`joessa`), while `ovi` is
    // a plain `7` and keeps both readings, so neither is certain.
    let inessive_specs = choose_genitive_stems(inessive_specs, gradation);
    let inessive_readings = readings_for(&inessive_specs, lemma, is_model);
    for spec in inessive_specs {
        let this_reading = forced_reading(&inessive_readings, spec);
        let Some(mut stem) = stem_of(spec) else {
            continue;
        };
        // A reverse-gradation word keeps the strong grade in the inessive too:
        // `hammas` is `hampaassa`, not `hammaassa`.
        let reverse =
            gradation.is_some_and(|g| g.weak_site(lemma).is_some() && g.site(lemma).is_none());
        if let Some(g) = gradation.filter(|_| reverse) {
            stem = g.strongen(&stem);
        }
        // The inessive and the elative are weak, so the weak stem is used for
        // them: `pankki` is `pankissa` and `pankista`, not `pankkissa` and
        // `pankkista`. The plural takes its own stem from the plural `i`.
        let weak_stem = if reverse {
            stem.clone()
        } else {
            weak_of(spec, &stem)
        };
        let weak = shape(&weak_stem, spec, plural_stem);
        // A stem that already ends in `s` or `n` cannot take `-ssa`, since that
        // gives `naisssa` and `tuhannssa` where Finnish writes `naisessa` and
        // `tuhannessa`. Every other stem takes both readings, because the class
        // cannot tell a stem that was left alone from one that lost a theme
        // vowel: `jo` is `joki` without its `ki` and gives `joessa`, and `joki`
        // itself would give `jokissa`, of which only one is a word. A bare
        // lemma that still carries the final `s` a `ks` rule would have
        // replaced is not such a stem: `ananasessa` on `ananas` is a form of
        // no word at all.
        let s_or_n_allomorph = matches!(weak_stem.chars().last(), Some('s') | Some('n'))
            && (rule_applies(spec, lemma, is_model) || !spec_is_ks(spec));
        for ending in [Ending::Ssa, Ending::Essa, Ending::Issa] {
            if matches!(ending, Ending::Ssa) && s_or_n_allomorph {
                continue;
            }
            for h in harmonies(spec) {
                let a = h.a();
                let s_end = match ending {
                    Ending::Ssa => format!("ss{a}"),
                    Ending::Essa => format!("ess{a}"),
                    Ending::Issa => format!("iss{a}"),
                };
                let t_end = match ending {
                    Ending::Ssa => format!("st{a}"),
                    Ending::Essa => format!("est{a}"),
                    Ending::Issa => format!("ist{a}"),
                };
                // A loanword ending in a consonant (class 22, `parfait`) takes
                // an apostrophe between the singular stem and the ending. The
                // plural keeps its own, which the catalogue stem already carries.
                let (s_end, t_end) = if class == 22 && !matches!(ending, Ending::Issa) {
                    (format!("'{s_end}"), format!("'{t_end}"))
                } else {
                    (s_end, t_end)
                };
                // The singular uses the weak stem. The plural hangs its `i` off the
                // catalogue's plural stem, which is the plural partitive without
                // its ending: `talo` is `talossa` and `taloissa`, and `kala` is
                // `kalassa` and `kaloissa` rather than the `kalaissa` the
                // singular stem alone would give. The singular stem still gives
                // a guess beside it, since a class that drops a theme vowel in
                // the plural is one the class table alone cannot spell. Class 7
                // keeps the lemma through the plural (`ovi` is `ovissa` beside
                // `joissa` on `jo`), so there the two stems coincide. The
                // catalogue's stem is read out of the word, so the plural is
                // certain only where the class reaches it too: `talo` keeps its
                // stem and is plain, and `kala` reads `kalo` out of `kaloja`
                // and is not. The plural reading hangs off the weak stem, the
                // singular likewise.
                let (use_stem, licence, judged, plural_certain) = if matches!(ending, Ending::Issa)
                {
                    (
                        &plural_weak,
                        Licence::Plural,
                        shape(plural_stem, spec, plural_stem),
                        reaches_weak(&stem),
                    )
                } else if matches!(ending, Ending::Ssa) {
                    (&weak_stem, Licence::Ssa, weak, true)
                } else {
                    (&weak_stem, Licence::Essa, weak, true)
                };
                let sure = this_reading
                    && plural_certain
                    && sure_of_harmony(h)
                    && licensed(licence, &judged, lemma);
                let push_one = if matches!(ending, Ending::Issa) {
                    push_plural
                } else {
                    push
                };
                push_one(&mut out, format!("{use_stem}{s_end}"), Case::Inessive, sure);
                push_one(&mut out, format!("{use_stem}{t_end}"), Case::Elative, sure);
                // The guess off the singular stem, for the class that drops a
                // theme vowel in the plural: `kala` gives `kalaissa` beside
                // `kaloissa`. Class 7 is where the two coincide, so no second
                // spelling is emitted there.
                if matches!(ending, Ending::Issa) && plural_stem != weak_stem {
                    push_plural(
                        &mut out,
                        format!("{weak_stem}{s_end}"),
                        Case::Inessive,
                        false,
                    );
                    push_plural(
                        &mut out,
                        format!("{weak_stem}{t_end}"),
                        Case::Elative,
                        false,
                    );
                }
            }
        }
    }

    // The instructive and the comitative are plurals, and their stem is the
    // plural stem: `talo` is `taloin` and `taloineen`, and the `kala` a
    // theme-vowel-dropping class reads as `kalo` is `kaloin` and `kaloineen`
    // where the singular stem alone would give `kalain` and `kalaineen`. The
    // catalogue's stem is read out of the word, so only a word the class
    // reaches that way is certain of it. The bare stem readings beside them
    // are the class's own fallback and are guesses even where they land on
    // the right word.
    //
    // The two endings differ in grade. The instructive is a weak ending: `joki`
    // is `join` and `laatikko` is `laatikoin`, on the same weak stem as the
    // local cases. The comitative is strong, and keeps the strong stem with the
    // essive: `jokineen` and `laatikkoineen`, where the grade the flag took
    // from the local cases is back.
    let instructive_specs = readings_for(desc.stems, lemma, is_model);
    // The same question as everywhere else, asked the same way: the catalogue's
    // plural stem is certain where the class reaches it, and that is the class's
    // own stem rather than a grade of it. `talo` keeps `talo` and is plain, a
    // `kala` reads `kalo` out of `kaloja` and is not, and a `laatikko` reads
    // `laatiko` out of `laatikkoja` and is — the plural of a gradating word
    // keeps the stem the flag's weak grade makes of it, so the local stem is
    // judged on that grade.
    let plural_certain = |stem: &str| reaches_weak(stem);
    let strong_certain = |stem: &str| reaches_strong(stem);
    for spec in desc.stems.iter().copied() {
        // The `ks` stem of the `-us`/`-nas` nouns goes only where it belongs
        // (see the genitive loop above): an `aidas` answers with the lemma as
        // its stem, and `ananasella` on the bare lemma is a form of no word
        // at all.
        if matches!(
            spec,
            Stem::Rule(r) if r.add.is_some_and(|a| a == "kse" || a == "nakse" || a == "näkse")
        ) && !rule_applies(spec, lemma, is_model)
        {
            continue;
        }
        let Some(stem) = stem_of(spec) else { continue };
        // Whether this reading is the class's own and the only one it has: a
        // class that lists the lemma as a fallback beside its rule has the rule
        // for this word, and the fallback is where the word starts and not
        // where these endings go.
        let forced = instructive_specs.len() == 1 && instructive_specs[0] == spec;
        for h in harmonies(spec) {
            let a = h.a();
            let bare = stem.trim_end_matches(is_vowel);
            let judged = shape(&stem, spec, plural_stem);
            // The plural readings hang off the catalogue's stem, which is read
            // out of the word, so they are certain only where the class reaches
            // it too: `talo` keeps `talo` and is plain, and `kala` reads `kalo`
            // out of `kaloja` and is not.
            let plural_shape = shape(&plural_weak, spec, &plural_weak);
            let strong_shape = shape(plural_strong, spec, plural_strong);
            let force = forced && sure_of_harmony(h);
            let plural_force =
                force && plural_certain(&stem) && licensed(Licence::Plural, &plural_shape, lemma);
            let strong_force =
                force && strong_certain(&stem) && licensed(Licence::Plural, &strong_shape, lemma);
            push_plural(
                &mut out,
                format!("{plural_stem}in"),
                Case::Instructive,
                plural_force,
            );
            push(&mut out, format!("{stem}in"), Case::Instructive, false);
            push(&mut out, format!("{stem}ineen"), Case::Comitative, false);
            push(&mut out, format!("{bare}in"), Case::Instructive, false);
            push_plural(
                &mut out,
                format!("{plural_strong}ineen"),
                Case::Comitative,
                strong_force,
            );
            push(
                &mut out,
                format!("{bare}oineen"),
                Case::Comitative,
                force && licensed(Licence::Reading, &judged, lemma),
            );
            push(
                &mut out,
                format!("{stem}n{a}"),
                Case::Essive,
                force && licensed(Licence::Essive, &judged, lemma),
            );
        }
    }

    out.retain(|f| f.word.chars().count() >= 3);
    // A slot is a case and a number, and the certainty of a reading is only
    // half the question: two readings can each be certain and still be about
    // the same slot, which is where the class holds two words that disagree.
    demote_contested(&mut out);
    out.sort_by(|a, b| a.word.cmp(&b.word));
    // A word is reached more than once, from a stem reading and from a harmony
    // that both fit it, and the two readings of it are the same case, so the
    // merge is the same union as everywhere else.
    let mut merged: Vec<Form> = Vec::with_capacity(out.len());
    for form in out {
        match merged.last_mut() {
            Some(last) if last.word == form.word => last.absorb(&form),
            _ => merged.push(form),
        }
    }
    merged
}

/// What an ending is judged on: the stem it would attach to, the plural stem
/// the catalogue gives the lemma, and the one fact about how the reading was
/// built that decides which endings it can take.
#[derive(Debug, Clone, Copy)]
struct Reading<'a> {
    /// The stem the ending attaches to.
    stem: &'a str,
    /// The plural stem the catalogue gives for this lemma.
    plural: &'a str,
    /// Whether the class reached this stem by substituting a consonant, which
    /// is what tells a weak stem from a strong one: `käsi` is `käte` and the
    /// class's weak reading of it is `käde`.
    substituted: bool,
}

impl Reading<'_> {
    /// Whether the stem ends in a vowel, which is the shape the endings that
    /// attach straight on ask for.
    fn vowel_final(&self) -> bool {
        self.stem.chars().last().is_some_and(is_vowel)
    }

    /// Whether the stem is the plural stem the catalogue gives.
    fn is_plural(&self) -> bool {
        !self.plural.is_empty() && self.stem == self.plural
    }

    /// Whether a stem that ends in `s` or `n` is one the ending was cut for.
    ///
    /// An `s` at the end of a stem is either the `s` of a genitive (`naisen`
    /// gives `nais` and `tuhannen` gives `tuhann`) or the last letter of a word
    /// that never had one (`vastaus` is not `vastauks`), and an ending spelled
    /// with an `e` in front of its `ss` or `ll` is for the first kind. The
    /// lemma itself is the second kind, and a stem the class cut is the first.
    fn s_or_n_stem(&self, lemma: &str) -> bool {
        matches!(self.stem.chars().last(), Some('s') | Some('n')) && self.stem != lemma
    }
}

/// The stem a class's own rule gives a lemma, when the rule fits the word.
///
/// The reading is chosen the way the oblique chooses its own: the class's cut
/// of the lemma if the lemma has that shape (`aakkonen` is `aakkose`,
/// `kala` is `kala`), and the lemma itself for a class with no cut to apply.
/// A reading the class names for the class's own model word rather than for
/// the lemma (a fixed spelling such as `sisimmä` for `sisin`) is not a reading
/// of another word, so it is left out.
///
/// The plural is built on this stem and not on a cut of the model word, which
/// is what lets a member of the class pluralise like its class-mates however
/// long its own word is; see `expand::inflect::plural_stems`.
pub fn class_stem(class: ClassId, lemma: &str, is_model: bool) -> Option<String> {
    let desc = noun_class(class)?;
    let live = readings_for(desc.stems, lemma, is_model);
    let spec = live
        .iter()
        .copied()
        .find(|s| !s.is_literal())
        .or_else(|| live.first().copied())?;
    let stem = apply_stem_of(spec, lemma);
    // A class with no rule at all is one where the lemma is the stem, which is
    // most of them: `talo`, `kala` and `risti` take their endings to the whole
    // word. An empty stem out of a rule is no stem at all.
    let literal_lemma = desc.genitive.is_none() && desc.stems.iter().all(|s| s.is_literal());
    if stem.is_empty() && !literal_lemma {
        None
    } else {
        Some(stem)
    }
}

/// The table's licence, told in the terms the stem loop is judged by.
///
/// The declension table and the stem loop each keep their own enumeration of
/// the endings' licences, with the same names: this is the table row's
/// reading, chosen for the `licensed` question.
fn licence_of(table: crate::case::Licence) -> Licence {
    use crate::case::Licence as Table;
    match table {
        Table::Bare => Licence::Bare,
        Table::Ssa => Licence::Ssa,
        Table::Essa => Licence::Essa,
        Table::E => Licence::E,
        Table::Essive => Licence::Essive,
        Table::Plural => Licence::Plural,
        Table::Reading => Licence::Reading,
    }
}

/// What a case ending asks of the stem it hangs off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Licence {
    /// `-na`/`-nä`, the essive, on the genitive stem of a class that reaches it
    /// without substituting a consonant.
    ///
    /// The essive is a strong-grade case, and both stems it is built on are
    /// strong-grade readings — the one the class names for the cases that end
    /// in `-lla` and the one it names for the inessive — so a class that
    /// gradates is no obstacle here. The one stem it is not written on is the
    /// substituted one, which is the weak reading of a class that has both.
    Essive,
    /// `-ssa`/`-stä`, the inessive and the elative, written straight after a
    /// stem that ends in a vowel.
    Ssa,
    /// `-essa`/`-estä`, the same two written after the `s` or `n` of a stem the
    /// class cut to that shape: `nais` gives `naisessa`.
    Essa,
    /// An ending that attaches to the stem as it stands: `-lla`, `-lta`,
    /// `-lle`, `-ksi`, `-tta`, `-tse`, and the `-na` of a stem the class named
    /// for the essive.
    Bare,
    /// The same endings with the `e` of an `s` or `n` stem in front of them.
    E,
    /// The plural, whose `i` goes between the stem and the ending, and the
    /// instructive and comitative that hang off the same stem.
    ///
    /// The plural of a case is built on the plural stem, which is the singular
    /// one only where the word keeps its theme vowel: `talo` is `taloissa` and
    /// `taloin`, and a `kala` whose plural stem is `kalo` is `kaloissa` and
    /// `kaloin` and not the `kalaissa` and `kalain` its singular stem would
    /// give. That stem is read out of the catalogue's own plural partitive, so
    /// it is already the grade the plural cases take: a `laatikko` is
    /// `laatikkoissa` and not the `laatikossa` its singular gradation would
    /// make of the same letters, and the plural of a gradating word is the one
    /// case where the strong stem is the right one.
    ///
    /// What the reading is asked is whether the stem is that one, and not what
    /// it ends in. The plural cases ask for nothing extra, since the `i` in
    /// front of the ending is the plural's own: `lämpim` is `lämpimillä` and
    /// `lämpimissä` as it stands, where a `pankki` — which ends in the `i` of
    /// its plural by accident and so passed a vowel-final test — is
    /// `pankkiilla` by the same rule. A stem that ends in a consonant is as
    /// good here as one that does not, and a test that wanted a vowel was
    /// turning away the plurals of every word whose plural stem is
    /// `lämpim`-shaped.
    Plural,
    /// A reading no rule licenses: the `-o` of an `-llo` or an `-no`, and the
    /// bare-stem spellings of the instructive and the comitative.
    Reading,
}

/// Whether the class licenses this reading of an ending on this stem.
///
/// An ending has a shape it is written in and a stem has a sound it ends in,
/// and an ending that does not answer the stem is a spelling no Finnish word
/// takes: `talo` is `talossa` and `taloessä` is nothing, `nais` is `naisessa`
/// and `naisssa` is nothing, and `maa`, whose partitive is the tabulated
/// `maata`, takes no `-a` on the stem the class names. This is the whole of
/// that arithmetic. Every reading it turns down is still kept as a guess: the
/// union is what the checker accepts, and the choice is what the suggester
/// offers.
///
/// The grade the inflection gradates is not asked about here. It used to be,
/// and the flag it was asked with was the lemma's own, which is a fact about the
/// word rather than about the reading: it turned every ending of every gradating
/// noun down, so `laatikko` — whose class does name the stem the plural cases
/// take, and whose `laatikkoissa` and `laatikkoilla` are words — offered none
/// of them. The stems the readings are built on are already the grade each
/// ending wants, so the question has no answer left to give.
fn licensed(licence: Licence, r: &Reading<'_>, lemma: &str) -> bool {
    match licence {
        Licence::Essive => r.vowel_final() && !r.substituted,
        Licence::Ssa => r.vowel_final(),
        Licence::Essa => r.s_or_n_stem(lemma),
        Licence::Bare => r.vowel_final(),
        Licence::E => r.s_or_n_stem(lemma),
        Licence::Plural => r.is_plural(),
        Licence::Reading => false,
    }
}

///
/// A class with one genitive reading keeps it whatever the gradation says. A
/// class that holds words in different gradation states, as class 7 holds
/// `ovi` and `joki`, lists a stem for each state, and the flag is the only
/// thing that says which lemma is which. The gradated reading is the one that
/// cuts the deepest — the `k` of `joki` gradates away, so its stem drops the
/// extra letter — and a flagged lemma takes that reading while an ungradated
/// one takes the shallower cut.
///
/// The choice is made among the rules alone, and a fixed spelling is kept
/// whichever way it goes. A fixed stem is not one of a pair of gradation states
/// but the class's own model stem: `lämpimä` is the stem of `lämmin` whether or
/// not the word carries a flag, and a flag that dropped it would take the whole
/// paradigm of the class with it — `lämpimässä`, `lämpimellä` and every other
/// form of the class would stop being generated, over a reading that has no
/// competing state to be confused with.
fn choose_genitive_stems(specs: Vec<Stem>, gradation: Option<Gradation>) -> Vec<Stem> {
    if specs.len() <= 1 || gradation.is_none() {
        return specs;
    }
    let rules: Vec<Stem> = specs.iter().copied().filter(|s| is_cut(*s)).collect();
    if rules.len() <= 1 {
        return specs;
    }
    let deep = rules
        .iter()
        .map(|s| match s {
            Stem::Rule(r) => r.drop,
            Stem::Fixed(_) => 0,
        })
        .max()
        .unwrap_or(0);
    let gradated: Vec<Stem> = rules
        .into_iter()
        .filter(|s| matches!(s, Stem::Rule(r) if r.drop == deep))
        .collect();
    if gradated.is_empty() {
        return specs;
    }
    // A class that holds a genitive stem and a bare plural basis of the same
    // depth (`joki` is `joe` and `jo`) has both in the deepest cut. The genitive
    // is the reading that keeps the theme vowel — the `joe` the singular `joessa`
    // is built from — while the bare `jo` is only the plural basis. Prefer the
    // themed reading so the plural basis is not taken for a genitive and offered
    // for the singular oblique.
    let themed: Vec<Stem> = gradated
        .iter()
        .copied()
        .filter(|s| matches!(s, Stem::Rule(r) if r.add.is_some_and(|a| a.chars().next().is_some_and(is_vowel))))
        .collect();
    if !themed.is_empty() { themed } else { gradated }
}

/// Whether a reading cuts the lemma down to a new stem, which is what a class
/// offers one reading per gradation state of.
fn is_cut(spec: Stem) -> bool {
    match spec {
        Stem::Rule(r) => !(r.drop == 0 && r.sub.is_none() && r.add.is_none()),
        Stem::Fixed(_) => false,
    }
}

/// Records one generated case, tagged with what it is and what the class knows
/// about it.
///
/// The cases built here are not in the catalogue, so none of them is a plain
/// attachment of a tabulated ending. What makes one `Plain` is the class having
/// one reading to build it from and the ending being the one that stem
/// licenses; everything else is a guess, accepted as a word and never offered
/// as a correction, since a correction nothing vouches for is a correction to a
/// non-word.
fn push(out: &mut Vec<Form>, word: String, case: Case, sure: bool) {
    push_numbered(out, word, case, Number::Singular, sure)
}

/// The same, for a form that is a plural one.
///
/// The inessive and the elative are the only cases here with two numbers to
/// tell, since the `-ssa` and the `-issa` are the singular and the plural of the
/// same word: `talossa` and `taloissa` are one case and two numbers, and a
/// form that says only the case says the number of every other form in the
/// language.
fn push_plural(out: &mut Vec<Form>, word: String, case: Case, sure: bool) {
    push_numbered(out, word, case, Number::Plural, sure)
}

fn push_numbered(out: &mut Vec<Form>, word: String, case: Case, number: Number, sure: bool) {
    if !word.is_empty() {
        let confidence = if sure {
            Confidence::Plain
        } else {
            Confidence::Guessed
        };
        out.push(Form::new(word, Tags::of_number(case, number)).with(confidence));
    }
}

/// The slots a paradigm has: one per case and number, which is what a form
/// competes for.
const SLOTS: usize = Case::ALL.len() * Number::ALL.len();

/// Takes the `Plain` off every form whose case and number another one claims.
///
/// A reading can be certain on its own and still not be the word: class 7 holds
/// `ovi` and `joki`, whose genitive readings are `ove` and `joe`, and each is
/// right for its own member, so `ovessa` and `joessa` are both what the class
/// has to say about the inessive singular and only one of them is a word of the
/// lemma in hand. The class decides between them only with the gradation flag,
/// and where it does not, the slot has no certain form at all and stays a
/// guess. That is the difference between a class that cannot be read and a
/// class that can: the first is silent, the second is offered.
fn demote_contested(forms: &mut [Form]) {
    let mut claimed = [0u16; SLOTS];
    {
        // One claim per word and slot: the same spelling reached twice, from a
        // stem reading and from a harmony that fits it, is one candidate for
        // the slot and does not compete with itself.
        let mut claims: Vec<(&str, usize)> = Vec::new();
        for form in forms.iter().filter(|f| f.plain()) {
            for at in form.readings().iter().filter_map(slot_of) {
                claims.push((form.word.as_str(), at));
            }
        }
        claims.sort_unstable();
        claims.dedup();
        for (_, at) in claims {
            claimed[at] = claimed[at].saturating_add(1);
        }
    }
    for form in forms.iter_mut() {
        if !form.plain() {
            continue;
        }
        let contested = form
            .readings()
            .iter()
            .filter_map(slot_of)
            .any(|at| claimed[at] > 1);
        if contested {
            form.confidence = Confidence::Guessed;
        }
    }
}

/// The slot a reading names, or `None` for a reading that is not a case of a
/// number.
fn slot_of(tags: &Tags) -> Option<usize> {
    let (case, number) = (tags.case?, tags.number?);
    Some(case as usize * Number::ALL.len() + number as usize)
}

/// The readings of a class this lemma actually has, in the order the class
/// gives them.
///
/// A class lists the shapes its members take, and a shape is a reading of this
/// lemma only when the word has it. The reading that changes nothing is the
/// lemma itself and is always one, a fixed stem is the spelling of the class's
/// own model word and belongs to that word alone, and a rule counts when the
/// tail it cuts is the tail it is for: a rule that adds a consonant is for a
/// word that ends in one, `askel` has the `l` to give up for the `le` of
/// `askele` and a `kylmä` of the same class has not, and a substitution needs
/// the letter it changes, which `nainen` has in `nen` and gives `naise` from.
///
/// A class that lists a rule and the lemma beside it has that one rule: the
/// second entry is where a word with no rule starts and is left out when there
/// is a rule to use. A class with no rule at all is a class of unmarked words,
/// and its one reading is the lemma.
fn readings_for(specs: &[Stem], lemma: &str, is_model: bool) -> Vec<Stem> {
    let live: Vec<Stem> = specs
        .iter()
        .copied()
        .filter(|s| rule_applies(*s, lemma, is_model))
        .collect();
    let rules: Vec<Stem> = live
        .iter()
        .copied()
        .filter(
            |s| !matches!(s, Stem::Rule(r) if r.drop == 0 && r.sub.is_none() && r.add.is_none()),
        )
        .collect();
    if rules.is_empty() { live } else { rules }
}

/// Whether a reading is the one the class means for the oblique, rather than a
/// literal fallback the word starts from.
///
/// A literal reading (a fixed spelling or the bare lemma) is where the word
/// begins, not where the endings go, so it is the real reading only when the
/// class has no cut of its own: `käs` is a fallback beside the `käde` that
/// class 27 inflects from, while `sisimmä` is the only stem `sisin` has.
fn forced_reading(readings: &[Stem], spec: Stem) -> bool {
    let real = readings.iter().any(|s| !s.is_literal());
    readings.contains(&spec) && (!real || !spec.is_literal())
}

/// Whether a stem reading is a `ks` reading of the `-us`/`-ys`/`-nas` nouns
/// (see the `US` table in `stem.rs`).
fn spec_is_ks(spec: Stem) -> bool {
    matches!(
        spec,
        Stem::Rule(r) if r.add.is_some_and(|a| a == "kse" || a == "nakse" || a == "näkse")
    )
}

/// Whether a class's reading is a shape this lemma has.
fn rule_applies(spec: Stem, lemma: &str, is_model: bool) -> bool {
    match spec {
        // A fixed stem is a spelling, and it is the spelling of the class's own
        // model word: `käs` is the stem of the `käsi` that models class 27, and
        // a `kausi` is not inflected from it.
        Stem::Fixed(_) => is_model,
        Stem::Rule(rule) => {
            // The reading that changes nothing is the lemma, which every lemma
            // has.
            if rule.drop == 0 && rule.sub.is_none() && rule.add.is_none() {
                return true;
            }
            let chars: Vec<char> = lemma.chars().collect();
            if rule.drop as usize >= chars.len() {
                return false;
            }
            // A reading written for a lemma of one ending says nothing about a
            // lemma that does not end that way: the `ks` stem of the `vastaus`
            // that models class 39 is not the stem of the `aidas` in it, and a
            // form built as though it were is a form of no word at all.
            if rule.suffix.is_some_and(|tail| !lemma.ends_with(tail)) {
                return false;
            }
            let kept: String = chars[..chars.len() - rule.drop as usize].iter().collect();
            // A substitution the kept portion has no letter for is a no-op, and
            // `apply_stem` lets it stand rather than giving up: the class still
            // drops its tail and adds what it adds. The model `nainen` is
            // `naise` because the last `n` of the `nain` it keeps becomes an
            // `s`, but a member whose own stem has no `n` there to change takes
            // the same cut and the same ending, which is what makes
            // `aakkoset` the stem `aakkose` and pluralise as
            // `aakkosissa` rather than as the whole word. Only a rule whose
            // whole edit is that one substitution is left with nothing to do,
            // and such a word is not the shape the class was written for.
            if rule.drop == 0
                && rule.add.is_none()
                && rule.sub.is_some_and(|(from, _)| !kept.contains(from))
            {
                return false;
            }
            match rule.add {
                // A rule that adds a consonant is for a word that ends in one:
                // what it cuts has to be a consonant, or the stem it hands back
                // is not the word's (`kylmle` of a `kylmä`).
                Some(add) if !add.starts_with(is_vowel) => chars
                    .get(chars.len() - rule.drop as usize)
                    .is_some_and(|c| !is_vowel(*c)),
                _ => true,
            }
        }
    }
}

/// What an ending is judged on, gathered from a stem the class built.
fn shape<'a>(stem: &'a str, spec: Stem, plural: &'a str) -> Reading<'a> {
    Reading {
        stem,
        plural,
        substituted: matches!(spec, Stem::Rule(r) if r.sub.is_some()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(class: ClassId, lemma: &str, want: &str) -> bool {
        let plural = plural_stem_of(class, lemma);
        oblique(lemma, class, None, &plural, &plural, true)
            .iter()
            .any(|f| f.word == want)
    }

    /// The same, with the catalogue's plural stem stated. The harness has no
    /// catalogue to read one from, so a class that drops a theme vowel in the
    /// plural takes its stem here: `kaloja` gives `kalo`.
    fn has_in(class: ClassId, lemma: &str, plural: &str, want: &str) -> bool {
        oblique(lemma, class, None, plural, plural, true)
            .iter()
            .any(|f| f.word == want)
    }

    /// The same, for a form the class is *sure* of, which is what a suggestion
    /// may be drawn from.
    fn has_plain(class: ClassId, lemma: &str, want: &str) -> bool {
        let plural = plural_stem_of(class, lemma);
        oblique(lemma, class, None, &plural, &plural, true)
            .iter()
            .any(|f| f.word == want && f.plain())
    }

    fn has_grad(class: ClassId, gradation: Gradation, lemma: &str, want: &str) -> bool {
        let plural = plural_stem_of(class, lemma);
        oblique(lemma, class, Some(gradation), &plural, &plural, true)
            .iter()
            .any(|f| f.word == want)
    }

    /// The same, for a form the class is sure of and a plural stem the caller
    /// states. The harness has no catalogue to read one from, and the lemma
    /// with its final vowel taken off is a different stem from the lemma, so
    /// the plural forms that hang off the plural stem need it said.
    fn has_plain_in(class: ClassId, lemma: &str, plural: &str, want: &str) -> bool {
        oblique(lemma, class, None, plural, plural, true)
            .iter()
            .any(|f| f.word == want && f.plain())
    }

    /// The same, for a gradated inflection and a certain form.
    fn has_grad_plain(class: ClassId, gradation: Gradation, lemma: &str, want: &str) -> bool {
        let plural = plural_stem_of(class, lemma);
        oblique(lemma, class, Some(gradation), &plural, &plural, true)
            .iter()
            .any(|f| f.word == want && f.plain())
    }

    /// The same, for a gradated inflection, a certain form and the two plural
    /// stems the caller states, which is the one case where they differ: the
    /// harness would spell the plural stem off the singular stem, and a word
    /// whose plural is weak needs the real one.
    fn has_grad_plain_in(
        class: ClassId,
        gradation: Gradation,
        lemma: &str,
        plural: &str,
        plural_strong: &str,
        want: &str,
    ) -> bool {
        oblique(lemma, class, Some(gradation), plural, plural_strong, true)
            .iter()
            .any(|f| f.word == want && f.plain())
    }

    /// The plural stem, for the test harness where there is no catalogue: the
    /// lemma with its final vowel taken off. The production caller reads the
    /// real stem out of the catalogue's plural partitive (`taloja` gives
    /// `talo` and `kaloja` gives `kalo`); the harness has no catalogue, so it
    /// spells the shape the singular stem alone would give, and the tests that
    /// want the real form pass the stem in.
    fn plural_stem_of(class: ClassId, lemma: &str) -> String {
        let _ = class;
        let mut stem = lemma.to_string();
        if stem.chars().last().is_some_and(is_vowel) {
            stem.pop();
        }
        stem
    }

    #[test]
    fn unmarked_classes_attach_directly() {
        assert!(has(1, "talo", "talossa"), "talo -> talossa");
        assert!(has(1, "talo", "taloissa"), "talo -> taloissa");
        assert!(has(9, "kala", "kalassa"), "kala -> kalassa");
        assert!(has(5, "risti", "ristissä"), "risti -> ristissa");
        assert!(has(18, "maa", "maassa"), "maa -> maassa");
        assert!(has(6, "paperi", "paperissa"), "paperi -> paperissa");
    }

    #[test]
    fn theme_vowel_classes() {
        assert!(has(7, "joki", "joessa"), "joki -> joessa");
        assert!(has(7, "joki", "jokella"), "joki -> jokella");
        assert!(has(7, "ovi", "ovessa"), "ovi -> ovessa");
        assert!(has(29, "lapsi", "lapsessa"), "lapsi -> lapsessa");
        assert!(has(24, "uni", "unessa"), "uni -> unessa");
        assert!(has(25, "toimi", "toimessa"), "toimi -> toimessa");
        assert!(has(33, "kytkin", "kytkimessä"), "kytkin -> kytkimessa");
        assert!(has(38, "nainen", "naisessa"), "nainen -> naisessa");
    }

    #[test]
    fn elative_drops_one_s() {
        assert!(has(9, "kala", "kalasta"), "kala -> kalasta");
        assert!(has(7, "joki", "joesta"), "joki -> joesta");
        assert!(has(5, "risti", "rististä"), "risti -> ristista");
    }

    #[test]
    fn fixed_stem_classes() {
        assert!(has(27, "käsi", "kädessä"), "käsi -> kädessä");
        assert!(has(27, "käsi", "kädellä"), "käsi -> kadella");
        assert!(has(35, "lämmin", "lämpimässä"), "lammin -> lampimassa");
        assert!(has(37, "vasen", "vasemmassa"), "vasen -> vasemmassa");
        assert!(has(44, "kevät", "keväällä"), "kevät -> kevalla");
        assert!(has(50, "isoäiti", "isoäidissä"), "isoaiti -> isoaidissa");
    }

    #[test]
    fn added_vowel_classes() {
        assert!(has(41, "vieras", "vieraassa"), "vieras -> vieraassa");
        assert!(has(41, "vieras", "vieraalla"), "vieras -> vieraalla");
        assert!(has(42, "mies", "miehessä"), "mies -> miehessa");
        assert!(has(42, "mies", "miehellä"), "mies -> miehella");
    }

    #[test]
    fn oblique_series_is_complete() {
        let f = oblique("kala", 9, None, "kalo", "kalo", true);
        for want in [
            "kalassa",
            "kalasta",
            "kalalla",
            "kalalta",
            "kalalle",
            "kalana",
            "kalaksi",
            "kalatta",
            "kaloineen",
            "kalin",
        ] {
            assert!(f.iter().any(|x| x.word == want), "missing {want} in {f:?}");
        }
    }

    #[test]
    fn the_local_cases_have_a_plural() {
        // The plural of the adessive, ablative, allative, essive and
        // translative hangs an `i` off the plural stem, which is the plural
        // partitive without its ending: `taloja` gives `talo`, so `talo` is
        // `taloilla`, and `kaloja` gives `kalo`, so `kala` is `kaloilla`.
        for (class, lemma, plural_stem, wants) in [
            (
                1,
                "talo",
                "talo",
                vec!["taloilla", "taloilta", "taloille", "taloina", "taloiksi"],
            ),
            (
                9,
                "kala",
                "kalo",
                vec!["kaloilla", "kaloilta", "kaloille", "kaloina", "kaloiksi"],
            ),
            (
                5,
                "risti",
                "riste",
                vec![
                    "risteillä",
                    "risteiltä",
                    "risteille",
                    "risteinä",
                    "risteiksi",
                ],
            ),
            (
                42,
                "mies",
                "mieh",
                vec!["miehillä", "miehiltä", "miehille", "miehinä", "miehiksi"],
            ),
            (
                27,
                "käsi",
                "käs",
                vec!["käsillä", "käsiltä", "käsille", "käsinä", "käsiksi"],
            ),
        ] {
            let f = oblique(lemma, class, None, plural_stem, plural_stem, true);
            for want in wants {
                assert!(
                    f.iter().any(|x| x.word == want),
                    "{lemma} missing {want} in {f:?}"
                );
            }
        }
    }

    #[test]
    fn the_abessive_follows_the_harmony() {
        // The abessive is `-tta` after a back word and `-ttä` after a front
        // one: `talo` is `talotta` and `käsi` is `kädettä`. The singular and
        // the plural both follow it, so `käsittä` is the front plural and
        // `risteittä` is the `-i` type's own.
        assert!(has(1, "talo", "talotta"), "talo -> talotta");
        assert!(has(27, "käsi", "kädettä"), "käsi -> kädettä");
        let risti = oblique("risti", 5, None, "riste", "riste", true);
        assert!(
            risti.iter().any(|x| x.word == "ristittä"),
            "risti -> ristittä"
        );
        assert!(
            risti.iter().any(|x| x.word == "risteittä"),
            "risti -> risteittä"
        );
    }

    #[test]
    fn a_front_word_takes_no_o_stem_readings() {
        // The `-o` stem readings are how a back `a`-stem reaches `maalla` by
        // way of `maallo`. A front word has no `o` to reach, so `kädenö` and
        // `kädellö` are not words and must not be generated.
        let f = oblique("käsi", 27, None, "käs", "käs", true);
        assert!(f.iter().any(|x| x.word == "kädellä"), "käsi -> kädellä");
        assert!(
            !f.iter().any(|x| x.word == "kädenö" || x.word == "kädellö"),
            "käsi must not take the -o stem readings"
        );
        // A back word still does.
        let m = oblique("maa", 18, None, "ma", "ma", true);
        assert!(m.iter().any(|x| x.word == "maallo"), "maa -> maallo");
    }

    #[test]
    fn the_inessive_and_the_elative_have_a_plural() {
        // The catalogue tabulates no plural inessive, so the plural is the same
        // ending with the plural `i` in it, and it hangs off the catalogue's
        // plural stem: `talo` is `talossa` and `taloissa`, and `joki` is
        // `joessa` in the singular and `joissa` in the plural, since `joet` and
        // `joiden` are on the same `joi`.
        assert!(has(1, "talo", "taloissa"), "talo -> taloissa");
        assert!(has(1, "talo", "taloista"), "talo -> taloista");
        assert!(has(7, "joki", "joessa"), "joki -> joessa");
        assert!(has(7, "joki", "joissa"), "joki -> joissa");
        assert!(has(7, "ovi", "ovessa"), "ovi -> ovessa");
        assert!(has(7, "ovi", "ovissa"), "ovi -> ovissa");
        // A class that drops a theme vowel in the plural reads the stem out of
        // the catalogue: `kala` is `kalassa` and `kaloissa`, and the `kalaissa`
        // the singular stem alone would give is still generated beside it,
        // since a guess is the right thing to keep where the class table alone
        // cannot spell the form, and a guess is never suggested. The harness
        // has no catalogue to read from, so the plural stem is passed in.
        assert!(has_in(9, "kala", "kalo", "kaloissa"), "kala -> kaloissa");
        assert!(has(9, "kala", "kalaissa"), "kala -> kalaissa");
    }

    #[test]
    fn the_cases_ending_in_lla_take_the_genitive_stem() {
        // `joki` is `joessa` on its own short stem and `joella` on the weak
        // one, which is `joe-` rather than `joke-`. `jokella` is not a word of
        // this noun at all.
        assert!(has(7, "joki", "joella"), "joki -> joella");
        assert!(has(7, "joki", "joessa"), "joki -> joessa");
        assert!(has(7, "ovi", "ovella"), "ovi -> ovella");
        assert!(has(7, "ovi", "ovessa"), "ovi -> ovessa");
    }

    #[test]
    fn gradation_names_the_genitive_stem_of_a_disagreement() {
        // Class 7 holds `ovi` and `joki`, whose genitive stems are `ove` and
        // `joe`, so a class on its own cannot say which member a reading is
        // for. The sanalista's gradation flag can: `joki` is `7*D`, where the
        // `k` gradates away and the stem drops the deeper cut, and `ovi` is a
        // plain `7`. With the flag, only that member's stem survives.
        let d = Gradation::from_letter('D').expect("D is a gradation");
        assert!(has_grad(7, d, "joki", "joella"), "joki*D -> joella");
        assert!(!has_grad(7, d, "joki", "ovella"), "joki*D is not ovella");
        // A `joki` that carried the flag still keeps the inessive, which is on
        // the ungradated short stem.
        assert!(has_grad(7, d, "joki", "joessa"), "joki*D -> joessa");
        // `ovi` has no gradation, so it keeps the shallower `ove` stem and not
        // the deeper `joe` one.
        assert!(has(7, "ovi", "ovella"), "ovi -> ovella");
        assert!(!has(7, "ovi", "joella"), "ovi is not joella");
    }

    #[test]
    fn the_minus_ina_classes_lose_the_whole_ending() {
        // The stem of `nainen` is `nais`, not `naise`, and the `n` of the
        // nominative ending is what turns into the `s`.
        assert!(has(38, "nainen", "naisessa"), "nainen -> naisessa");
        assert!(has(38, "nainen", "naisella"), "nainen -> naisella");
        assert!(has(38, "nainen", "naiselta"), "nainen -> naiselta");
    }

    #[test]
    fn the_inessive_takes_the_genitive_stem() {
        // The inessive of `käsi` is `kädessä` and not `kässä`: the cases that
        // end in `-ssa` are built on the genitive stem `käde`, the same one the
        // adessive `kädellä` uses. `kässä` is not a word, and generating it
        // would put a non-word into the suggestion list.
        assert!(has(27, "käsi", "kädessä"), "käsi -> kädessä");
        assert!(has(27, "käsi", "kädestä"), "käsi -> kädestä");
        assert!(!has(27, "käsi", "kässä"), "kässä is not a word");
        assert!(!has(27, "käsi", "kässsä"), "kässsä is not a word either");
        // The other classes that drop a theme vowel are the same case.
        // `lapsi` is `lapsessa`, and it is the back harmony of the `a` in
        // `la-` that says so. The `e` the class adds to make the stem `lapse`
        // is a theme vowel, and a theme vowel is never the stressed one, so
        // reading harmony off it would give the front and `lapsessä`.
        assert!(has(29, "lapsi", "lapsessa"), "lapsi -> lapsessa");
        assert!(has(29, "lapsi", "lapsesta"), "lapsi -> lapsesta");
        assert!(!has(29, "lapsi", "lapsessä"), "lapsessä is not a word");
        // The same in the other direction: `toimi` is `toimessa`, and the
        // added `e` of `toime` would give `toimessä`.
        assert!(has(25, "toimi", "toimessa"), "toimi -> toimessa");
        assert!(!has(25, "toimi", "toimessä"), "toimessä is not a word");
        assert!(has(50, "isoäiti", "isoäidissä"), "isoäiti -> isoäidissä");
    }

    #[test]
    fn a_us_noun_puts_its_oblique_endings_on_the_ks_stem() {
        // `vastaus` is `vastauksen` in the genitive, and the cases the catalogue
        // does not tabulate hang off that `vastaukse`: the `s` the lemma ends in
        // drops out and the `ks` the genitive shows comes in.
        assert!(
            has(39, "vastaus", "vastauksessa"),
            "vastaus -> vastauksessa"
        );
        assert!(
            has(39, "vastaus", "vastauksesta"),
            "vastaus -> vastauksesta"
        );
        assert!(
            has(39, "vastaus", "vastauksella"),
            "vastaus -> vastauksella"
        );
        assert!(
            has(39, "vastaus", "vastaukselle"),
            "vastaus -> vastaukselle"
        );
        // What the lemma would have given is not a word. It is still generated,
        // because the class holds members whose stem is the lemma, but it is
        // never the form this lemma is corrected to.
        for not in ["vastausessa", "vastausesta", "vastausella", "vastausksi"] {
            assert!(!has_plain(39, "vastaus", not), "vastaus is offered {not}");
        }
        // The `ks` stem is the reading of the `-us` and `-nas` nouns alone, so
        // the members of the class that do not end that way are not answered
        // with it.
        assert!(
            !has_plain(39, "aidas", "aidaksella"),
            "aidas is offered aidaksella"
        );
        assert!(
            !has_plain(39, "kymmenes", "kymmeneksella"),
            "kymmenes is offered kymmeneksella"
        );
        // The `-nas` nouns take the same `ks` stem: `ananas` is `ananaksen`,
        // and the untabulated cases hang off `ananakse`.
        assert!(has(39, "ananas", "ananaksessa"), "ananas -> ananaksessa");
        assert!(
            has_plain(39, "ananas", "ananaksella"),
            "ananas is not offered ananaksella"
        );
        for not in ["ananasessa", "ananasella", "ananasnasella"] {
            assert!(!has_plain(39, "ananas", not), "ananas is offered {not}");
        }
        // A stem that really does end in `s` or `n` keeps it and spells the
        // ending with an `e` in front: `tuhat` is `tuhannen`, and so the
        // inessive is `tuhannessa` and the adessive `tuhannella`.
        assert!(has(46, "tuhat", "tuhannessa"), "tuhat -> tuhannessa");
        assert!(has(46, "tuhat", "tuhannella"), "tuhat -> tuhannella");
    }

    #[test]
    fn a_certain_oblique_form_is_offered_as_a_correction() {
        // The cases the catalogue does not tabulate are plain when the class
        // has one reading to build them from and the ending is the one that
        // stem is written with: `talossa` is the inessive of `talo` and nothing
        // else it can be.
        for want in [
            "talossa", "talosta", "talolla", "talolta", "talolle", "talona", "taloksi", "talotta",
        ] {
            assert!(has_plain(1, "talo", want), "talo has no plain {want}");
        }
        // The plural of the same hangs off the plural stem, which the harness
        // has to be told: with `talo` as the plural stem the class named, the
        // local plural cases and the plural attributive are certain.
        for want in [
            "taloissa",
            "taloista",
            "taloilla",
            "taloilta",
            "taloille",
            "taloina",
            "taloiksi",
            "taloitta",
            "taloin",
            "taloineen",
        ] {
            assert!(
                has_plain_in(1, "talo", "talo", want),
                "talo has no plain {want}"
            );
        }
        // The same for a class that cuts its stem: `lapsi` is `lapsessa` and
        // `lapsella`, and for a class the inflection gradates: `laatikko` is
        // `laatikossa` in the weak grade and `laatikkona` in the strong one the
        // essive takes.
        assert!(has_plain(29, "lapsi", "lapsessa"), "lapsi -> lapsessa");
        assert!(has_plain(29, "lapsi", "lapsella"), "lapsi -> lapsella");
        assert!(
            has_grad_plain(
                4,
                Gradation::from_letter('A').unwrap(),
                "laatikko",
                "laatikossa"
            ),
            "laatikko -> laatikossa"
        );
        assert!(
            has_grad_plain(
                4,
                Gradation::from_letter('A').unwrap(),
                "laatikko",
                "laatikkona"
            ),
            "laatikko -> laatikkona"
        );
        // A fixed stem belongs to the class's own model word, and `kevät` is it.
        assert!(has_plain(44, "kevät", "keväässä"), "kevät -> keväässä");
        assert!(has_plain(44, "kevät", "keväällä"), "kevät -> keväällä");
        // A word the spelling settles the harmony of takes the one side of the
        // split: `kylmä` is `kylmässä` and `kylmällä`.
        assert!(has_plain(10, "kylmä", "kylmässä"), "kylmä -> kylmässä");
        assert!(has_plain(10, "kylmä", "kylmällä"), "kylmä -> kylmällä");
    }

    #[test]
    fn a_reading_the_class_cannot_settle_on_stays_a_guess() {
        // The candidates beside the right one are still generated — the union
        // is what the checker accepts — and none of them is offered: the other
        // harmony, the allomorph a vowel-final stem does not take, and the `-o`
        // reading no rule licenses.
        for guess in ["talossä", "taloessa", "talollo", "talono"] {
            assert!(
                !has_plain(1, "talo", guess),
                "talo should not offer {guess}"
            );
            assert!(has(1, "talo", guess), "talo should still generate {guess}");
        }
        // The instructive and the comitative hang off the plural stem, and the
        // plural of `kala` is `kalo`: the `kalain` and `kalaineen` the singular
        // stem alone would give are guesses, and so is the plural inessive
        // built from the singular stem, `kalaissa`. The `kaloineen` beside them
        // is right and is still a guess here, because the harness plural stem
        // is the shape the singular stem alone would give and not the
        // catalogue's `kalo`.
        assert!(!has_plain(9, "kala", "kalain"), "kalain is offered");
        assert!(!has_plain(9, "kala", "kalaineen"), "kalaineen is offered");
        assert!(!has_plain(9, "kala", "kalaissa"), "kalaissa is offered");
        // The catalogue's plural stem beside them is right, and since class 9
        // explicitly licenses the `kala` -> `kalo` change it is the class's
        // own form, offered rather than guessed. The harness has no catalogue,
        // so the stem is passed in: `kaloja` gives `kalo`.
        assert!(has(9, "kala", "kalain"), "kalain is not generated");
        assert!(has(9, "kala", "kalaineen"), "kalaineen is not generated");
        assert!(has(9, "kala", "kalaissa"), "kalaissa is not generated");
        assert!(
            has_in(9, "kala", "kalo", "kaloin"),
            "kaloin is not generated"
        );
        assert!(
            has_plain_in(9, "kala", "kalo", "kaloineen"),
            "kaloineen is not offered"
        );
        // `talo` keeps its stem in the plural, so the harness stem is the
        // catalogue's stem there. Both plurals are generated; neither is
        // offered here, since the stem the class reaches is the shape the
        // singular stem alone would give and not the catalogue's `talo`.
        assert!(has(1, "talo", "taloineen"), "taloineen is not generated");
        assert!(has(1, "talo", "taloin"), "taloin is not generated");
        assert!(!has_plain(1, "talo", "taloineen"), "taloineen is offered");
        // A stem the class cut from the lemma is the stem the endings go on: the
        // `vastaus` of class 39 is offered as `vastauksella`, and the spelling
        // built on the lemma, which belongs to the class's other members, is not
        // offered for this one.
        assert!(
            has_plain(39, "vastaus", "vastauksella"),
            "vastaus is not offered vastauksella"
        );
        assert!(
            has_plain(39, "vastaus", "vastauksessa"),
            "vastaus is not offered vastauksessa"
        );
        assert!(
            !has_plain(39, "vastaus", "vastausella"),
            "vastausella is offered"
        );
        // The prolative is not a case a class can answer for. `maitse` and
        // `meritse` are words and `talotse` is nothing, and which nouns have a
        // prolative is a fact the class table does not hold, so the form is
        // generated as a candidate and never offered as a correction.
        assert!(has(1, "talo", "talotse"), "talotse is not generated");
        assert!(!has_plain(1, "talo", "talotse"), "talotse is offered");
        // The essive is strong and the genitive stem the class gives is weak,
        // so a class that substituted a consonant cannot say its essive: `käsi`
        // is `kätenä` and the `kädenä` here is not that word.
        assert!(!has_plain(27, "käsi", "kädenä"), "kädenä is offered");
        // The plural inessive of `lapsi` hangs off the plural stem `laps`, and
        // the class's own stem is `lapse` — the same stem with the theme vowel
        // off, which is what the plural of this class does. The class reaches
        // that stem, so the real `lapsissa` is offered, and the reading built
        // on the singular stem beside it is the `lapseissa` that is not a word.
        assert!(
            has_plain(29, "lapsi", "lapsissa"),
            "lapsissa is not offered"
        );
        assert!(!has_plain(29, "lapsi", "lapseissa"), "lapseissa is offered");
    }

    /// A class that alternates a consonant names the stem its essive is built
    /// on, because the essive is a strong case and the genitive stem is the weak
    /// one. The two are a letter apart in both directions at once — `käde` gives
    /// `kädenä` and `käte` gives `kätenä` — so there is no reading of the
    /// genitive stem that would have made the second of them.
    #[test]
    fn an_essive_is_built_on_a_stem_of_its_own() {
        assert!(has(27, "käsi", "kätenä"), "käsi -> kätenä");
        assert!(has_plain(27, "käsi", "kätenä"), "kätenä is offered");
        // The weak reading is still generated, since it is a form of some other
        // word and the checker has to accept whatever a class could mean.
        assert!(has(27, "käsi", "kädenä"), "kädenä is not generated");
        // A class that alternates nothing is spelled the same way from both, so
        // the genitive stem is the essive stem and nothing changes for it.
        assert!(has_plain(1, "talo", "talona"), "talona is offered");
        assert!(has_plain(9, "kala", "kalana"), "kalana is offered");
    }

    /// A gradation flag is a fact about the lemma, and it says nothing about a
    /// stem the class named outright.
    ///
    /// `lämmin` is `35*H`, and the flag is on the `mm` of the lemma. The class
    /// gives its stem as a spelling, `lämpimä`, and that spelling carries its
    /// own `mp`, which the lemma's `H` would otherwise have found and turned
    /// into the `mm` of `lämmimä` — a stem no word has, so the word was read as
    /// `lämmimän`, `lämmimässä` and `lämmimät` as well as, or instead of, the
    /// real ones. The plural is `lämpimät` on the same stem as the singular, so
    /// there is nothing in this class for the flag to do.
    ///
    /// Every form below is the one in the sanalista and in Wiktionary, and the
    /// plural stem is the catalogue's own `lämpimä` rather than the harness's,
    /// which spells one off the lemma.
    #[test]
    fn a_fixed_stem_is_not_gradated_with_the_lemma() {
        let h = Gradation::from_letter('H').expect("H is a gradation");
        for want in [
            "lämpimässä",
            "lämpimällä",
            "lämpimältä",
            "lämpimälle",
            "lämpimänä",
            "lämpimäksi",
            "lämpimättä",
        ] {
            assert!(
                has_grad_plain_in(35, h, "lämmin", "lämpim", "lämpim", want),
                "lämmin is not offered {want} with its own flag"
            );
        }
        // The plural is that stem with the theme vowel dropped, which is what
        // the catalogue's own `lämpimiä` says, and the flag does not reach that
        // either.
        for want in [
            "lämpimissä",
            "lämpimistä",
            "lämpimillä",
            "lämpimiltä",
            "lämpimille",
            "lämpiminä",
            "lämpimiksi",
            "lämpimin",
            "lämpimineen",
        ] {
            assert!(
                has_grad_plain_in(35, h, "lämmin", "lämpim", "lämpim", want),
                "the plural of lämmin is not offered {want}"
            );
        }
        // And no form of the word is built on the `lämmimä` the flag would have
        // made of the stem.
        for not in [
            "lämmimän",
            "lämmimässä",
            "lämmimänä",
            "lämmimäksi",
            "lämmimät",
            "lämmimillä",
        ] {
            assert!(
                !has(35, "lämmin", not) && !has_in(35, "lämmin", "lämpim", not),
                "lämmin is given {not}"
            );
        }
    }

    /// The plural of a gradating noun follows the grade of the case, not of the
    /// singular: `laatikko` is `4*A`, the singular is `laatikossa`, and the
    /// plural weakens the stem the same way for the weak local cases and the
    /// instructive — `laatikoissa`, `laatikoilla`, `laatikoin` — while the
    /// essive and the comitative are strong-grade cases and keep the strong
    /// stem: `laatikkoina`, `laatikkoineen`. The two stems are the caller's:
    /// it reads `laatikko` out of `laatikkoja` and graduates it to `laatiko`.
    /// The weak stem is read out of the word, so those forms are guessed
    /// whatever the flag says; the strong stem is the class's own, so the
    /// essive and the comitative are certain.
    #[test]
    fn a_gradating_noun_keeps_its_plural() {
        let a = Gradation::from_letter('A').expect("A is a gradation");
        let have = |grad: bool, want: &str| {
            let g = if grad { Some(a) } else { None };
            oblique("laatikko", 4, g, "laatiko", "laatikko", true)
                .iter()
                .any(|f| f.word == want)
        };
        let plain = |grad: bool, want: &str| {
            let g = if grad { Some(a) } else { None };
            oblique("laatikko", 4, g, "laatiko", "laatikko", true)
                .iter()
                .any(|f| f.word == want && f.plain())
        };
        // The weak plural forms, present with the flag and without it alike.
        for want in [
            "laatikoissa",
            "laatikoista",
            "laatikoilla",
            "laatikoilta",
            "laatikoille",
            "laatikoiksi",
            "laatikoitta",
            "laatikoin",
        ] {
            assert!(
                have(true, want),
                "laatikko does not reach {want} with its own flag"
            );
            assert!(
                have(false, want),
                "laatikko does not reach {want} without a flag either"
            );
        }
        // The strong-grade plurals are the class's own stem and are offered,
        // with the flag and without.
        for want in ["laatikkoina", "laatikkoineen"] {
            assert!(
                plain(true, want),
                "laatikko is not offered {want} with its own flag"
            );
            assert!(
                plain(false, want),
                "laatikko is not offered {want} without a flag either"
            );
        }
        // The singular is weak where the flag says it is, and the plural is one
        // grade apart: `laatikossa` beside `laatikoissa`, and the essive strong
        // again, `laatikkona`.
        for want in [
            "laatikossa",
            "laatikolla",
            "laatikolta",
            "laatikolle",
            "laatikkona",
            "laatikoksi",
        ] {
            assert!(
                plain(true, want),
                "laatikko is not offered {want} with its own flag"
            );
        }
        // The old reading put the ungradated stem behind everything and offered
        // `laatikkoissa` as the plural inessive; it is no word of `laatikko`.
        assert!(
            !have(true, "laatikkoissa"),
            "laatikkoissa is reached as the plural inessive"
        );
    }

    #[test]
    fn a_class_that_holds_two_members_cannot_settle_a_slot() {
        // Class 7 holds `ovi` and `joki`, whose genitive readings are `ove` and
        // `joe`, and each of `ovessa` and `joessa` is the inessive of one of
        // them. Without the flag the class cannot say which lemma it is
        // inflecting, so neither reading is offered.
        assert!(!has_plain(7, "ovi", "ovessa"), "ovessa is offered");
        assert!(!has_plain(7, "joki", "joessa"), "joessa is offered");
        assert!(!has_plain(7, "ovi", "ovella"), "ovella is offered");
        // The flag settles it: `joki` is `7*D`, where the `k` gradates away and
        // the genitive stem is the deeper cut, so `joella` is the adessive and
        // is offered, and `ovella` is a reading of the other member.
        let grad = Gradation::from_letter('D').expect("D is a gradation");
        assert!(
            has_grad_plain(7, grad, "joki", "joella"),
            "joki is not offered joella"
        );
        assert!(
            !has_grad_plain(7, grad, "joki", "ovella"),
            "ovella is offered for joki"
        );
    }
}
