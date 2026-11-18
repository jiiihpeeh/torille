//! Clause role resolution.

use crate::tags::{Case, Mood, Number, PartOfSpeech, Person, Tags};
use crate::tokenise::Token;

/// Object cases when the surface carries a possessive suffix.
const POSSESSIVE_OBJECT_CASES: &[Case] = &[
    Case::Partitive,
    Case::Nominative,
    Case::Genitive,
    Case::Accusative,
];

use super::predicates::*;
use super::{Clause, RoleConfidence, SyntacticRole, TokenSyntax};

/// Assign the roles phase 5 can defend from morphology alone.
pub(super) fn classify(readings: &[Tags]) -> (SyntacticRole, RoleConfidence, Option<Case>) {
    if readings.is_empty() {
        return (SyntacticRole::Unknown, RoleConfidence::Weak, None);
    }

    let finite = readings
        .iter()
        .filter(|reading| is_finite_predicate(reading))
        .count();
    if finite > 0 {
        // An infinitive beside a finite reading (`pitää`, `näyttää`) is not
        // an ambiguity: both readings are the same verb. A nominal beside a
        // finite reading (`tuli` the fire, `kirjaa` the imperative) is one,
        // so the role stays weak until context resolves it.
        let competing_nominal = readings
            .iter()
            .any(|reading| is_nominal_pos(reading.part_of_speech));
        let confidence = if competing_nominal {
            RoleConfidence::Weak
        } else {
            RoleConfidence::Likely
        };
        return (SyntacticRole::Predicate, confidence, None);
    }

    // An adposition is an adverbial of its clause, not a noun with the case
    // its homonym happens to carry. `jälkeen` is `postpositio, adverbi`; the
    // noun `jälki` also gives it an illative reading, which must not turn
    // `puolisonsa jälkeen` into a genitive-subject plus illative object.
    if readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Preposition | PartOfSpeech::Postposition)
        )
    }) {
        return (SyntacticRole::Adverbial, RoleConfidence::Likely, None);
    }

    let adverbs = readings
        .iter()
        .filter(|reading| matches!(reading.part_of_speech, Some(PartOfSpeech::Adverb)))
        .count();
    // `sen`/`ne` carry an adverb homonym (`sen verran`) beside the pronoun's
    // genitive/accusative object reading. The adverb reading must not win
    // before the object pass can see `Tarvitsen sen`.
    let pronoun_object = readings.iter().any(|reading| {
        reading.part_of_speech == Some(PartOfSpeech::Pronoun)
            && matches!(reading.case, Some(Case::Genitive | Case::Accusative))
    });
    if adverbs > 0 && !pronoun_object {
        let confidence = if adverbs == readings.len() {
            RoleConfidence::Likely
        } else {
            RoleConfidence::Weak
        };
        return (SyntacticRole::Adverbial, confidence, None);
    }

    let adverbial_cases: Vec<Case> = readings
        .iter()
        .filter_map(|reading| reading.case)
        .filter(|case| is_adverbial_case(*case))
        .collect();
    if let Some(case) = adverbial_cases.first().copied() {
        let confidence = if adverbial_cases.len() == readings.len() {
            RoleConfidence::Likely
        } else {
            RoleConfidence::Weak
        };
        return (SyntacticRole::Adverbial, confidence, Some(case));
    }

    (SyntacticRole::Unknown, RoleConfidence::Weak, None)
}

/// Re-read a demoted predicate from its non-verb readings.
///
/// `siitä` is simultaneously the elative pronoun/adverb and a form of the verb
/// `siittää`. Once the clause has chosen a clearer predicate, the verb reading
/// is no longer available; classify what remains so an adverbial stays an
/// adverbial instead of becoming an `Unknown` that the attribute rule may pick
/// up. A word with no non-verb reading stays `Unknown`.
fn classify_without_verb(readings: &[Tags]) -> (SyntacticRole, RoleConfidence, Option<Case>) {
    let nominal_or_adverbial: Vec<Tags> = readings
        .iter()
        .filter(|reading| !is_verb_reading(reading))
        .cloned()
        .collect();
    if nominal_or_adverbial.len() == readings.len() {
        return (SyntacticRole::Unknown, RoleConfidence::Weak, None);
    }
    classify(&nominal_or_adverbial)
}

/// Assign the initial role with an optional preferred reading.
///
/// The preference is a hint from the disambiguation sidecar, never an
/// assignment. The baseline morphological classification stays authoritative
/// whenever it is [`RoleConfidence::Likely`] or adverbial: the adverb policy
/// deliberately protects pronoun objects (`Tarvitsen sen`). A decision may
/// refine only a weak predicate, re-reading it as a nominal
/// ([`SyntacticRole::Unknown`]) or as an adverbial, and the refined role is
/// always `Weak`. It cannot create a second confident predicate, and a
/// preferred nominal stays `Unknown` so the clause subject/object passes still
/// decide from context. Passing `None` is exactly [`classify`].
pub(super) fn classify_with_preference(
    readings: &[Tags],
    preferred: Option<&Tags>,
) -> (SyntacticRole, RoleConfidence, Option<Case>) {
    let baseline = classify(readings);
    let Some(preferred) = preferred else {
        return baseline;
    };
    if baseline.1 == RoleConfidence::Likely || baseline.0 == SyntacticRole::Adverbial {
        return baseline;
    }
    let Some((role, case)) = preferred_role(preferred) else {
        return baseline;
    };
    if role == baseline.0 {
        return baseline;
    }
    (role, RoleConfidence::Weak, case)
}

/// The role a single preferred reading supports, without clause context.
fn preferred_role(preferred: &Tags) -> Option<(SyntacticRole, Option<Case>)> {
    if is_finite_predicate(preferred) {
        return Some((SyntacticRole::Predicate, None));
    }
    if preferred.part_of_speech == Some(PartOfSpeech::Adverb) {
        return Some((SyntacticRole::Adverbial, None));
    }
    if let Some(case) = preferred.case.filter(|case| is_adverbial_case(*case)) {
        return Some((SyntacticRole::Adverbial, Some(case)));
    }
    if is_nominal_pos(preferred.part_of_speech) {
        return Some((SyntacticRole::Unknown, None));
    }
    None
}

/// The first finite predicate candidate in a clause, preferring an
/// unambiguous one over an ambiguous noun/verb word.
fn choose_predicate(words: &[usize], out: &[TokenSyntax]) -> Option<usize> {
    words
        .iter()
        .copied()
        .filter(|&index| is_predicate_candidate(index, out))
        .find(|&index| out[index].confidence != RoleConfidence::Weak)
        .or_else(|| {
            words
                .iter()
                .copied()
                .find(|&index| is_predicate_candidate(index, out))
        })
        .or_else(|| {
            words
                .iter()
                .copied()
                .find(|&index| is_infinitive(index, out))
        })
        // No main verb at all: keep the auxiliary as the predicate head so
        // the clause still has one (`Eikö hän tullut?` until phase 3 folds
        // the auxiliary and the participle into a predicate phrase).
        .or_else(|| {
            words
                .iter()
                .copied()
                .find(|&index| out[index].role == SyntacticRole::Predicate)
        })
}

/// The person and number of a personal pronoun surface.
fn personal_pronoun(key: &str) -> Option<(Person, Number)> {
    Some(match key {
        "minä" => (Person::First, Number::Singular),
        "sinä" => (Person::Second, Number::Singular),
        "hän" => (Person::Third, Number::Singular),
        "me" => (Person::First, Number::Plural),
        "te" => (Person::Second, Number::Plural),
        "he" => (Person::Third, Number::Plural),
        _ => return None,
    })
}

/// The finite predicate's person and number, including a negative auxiliary.
fn predicate_person_number(
    predicate: usize,
    words: &[usize],
    tokens: &[Token],
    out: &[TokenSyntax],
) -> Option<(Person, Number)> {
    // A connegative main verb is finite through the negative auxiliary before
    // it: `emme istu` is first-person plural even when `istu` also has an
    // imperative homonym with a different person.
    // `words` may be the whole clause or only the run before the predicate;
    // either way, the negative auxiliary we want is in the pre-predicate part.
    let before = words
        .iter()
        .position(|&index| index == predicate)
        .map_or(words, |position| &words[..position]);
    if let Some(auxiliary) = before
        .iter()
        .rev()
        .copied()
        .find(|&index| is_negative_auxiliary(index, out))
    {
        return crate::expand::negation::person_number(&tokens[auxiliary].text);
    }
    if out[predicate]
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
    {
        return crate::expand::negation::person_number(&tokens[predicate].text);
    }
    out[predicate]
        .readings
        .iter()
        .find(|reading| is_finite_predicate(reading))
        .and_then(|reading| reading.person.zip(reading.number))
}

/// Whether the word after `index` is the clause predicate or its auxiliary.
///
/// Used only for the unknown-capitalized-name fallback. `Kenoveeva kehräsi`
/// is a name plus predicate; `Kenoveevan laulua` is a name-form plus a noun,
/// so the latter stays out of the subject slot.
fn next_word_heads_predicate(index: usize, words: &[usize], out: &[TokenSyntax]) -> bool {
    let Some(position) = words.iter().position(|&candidate| candidate == index) else {
        return false;
    };
    words.get(position + 1).is_some_and(|&next| {
        out[next].role == SyntacticRole::Predicate
            || out[next].role == SyntacticRole::Auxiliary
            || is_negative_auxiliary(next, out)
    })
}

/// `Me suuremmat istumme`: the personal pronoun is the subject head and the
/// following nominative phrase is an apposition.
///
/// The ordinary subject search takes the last preverbal nominative, which would
/// make `suuremmat` the subject and leave the 1st-person predicate disagreeing
/// with a 3rd-person plural noun. A pronoun that already matches the predicate
/// is the stronger candidate. Coordination (`Minä ja Liisa`) is not an
/// apposition and stays with the coordinated-subject pass.
fn apposition_subject(
    before: &[usize],
    predicate: usize,
    tokens: &[Token],
    out: &mut [TokenSyntax],
) -> Option<usize> {
    let (person, number) = predicate_person_number(predicate, before, tokens, out)?;
    for (position, &pronoun) in before.iter().enumerate() {
        let Some((pronoun_person, pronoun_number)) = personal_pronoun(&out[pronoun].key) else {
            continue;
        };
        if pronoun_person != person || pronoun_number != number {
            continue;
        }
        let mut appositions = Vec::new();
        for &candidate in &before[position + 1..] {
            if candidate == predicate
                || is_clause_marker(candidate, tokens, out)
                || has_coordinator_between(tokens, pronoun, candidate)
                || out[candidate].role != SyntacticRole::Unknown
                || !nominal_has_case(&out[candidate], &[Case::Nominative])
            {
                break;
            }
            appositions.push(candidate);
        }
        if appositions.is_empty() {
            continue;
        }
        for candidate in appositions {
            out[candidate].role = SyntacticRole::Modifier;
            out[candidate].confidence = RoleConfidence::Likely;
            out[candidate].case = Some(Case::Nominative);
            out[candidate].head = Some(pronoun);
        }
        return Some(pronoun);
    }
    None
}

/// Resolve the core roles in one clause.
pub(super) fn resolve_clause(
    clause: &mut Clause,
    tokens: &[Token],
    out: &mut [TokenSyntax],
    nominal_lemmas: &[Option<String>],
) {
    let words: Vec<usize> = (clause.tokens.start..clause.tokens.end)
        .filter(|&index| tokens[index].is_word())
        .collect();
    let Some(mut predicate) = choose_predicate(&words, out) else {
        return;
    };
    // A compound predicate is an auxiliary plus a past participle: `olen
    // lukenut`, `talo on rakennettu`, `eikö hän tullut`. The main verb is the
    // participle; the auxiliary is linked to it. A present participle is not
    // part of a compound tense (`Minun on mentävä` stays a necessity
    // complement).
    let mut compound_auxiliary: Option<usize> = None;
    if is_negative_auxiliary(predicate, out) || out[predicate].lemma.as_deref() == Some("olla") {
        let position = words
            .iter()
            .position(|&index| index == predicate)
            .unwrap_or(0);
        if let Some(participle) = words[position + 1..]
            .iter()
            .copied()
            .find(|&index| is_past_participle(index, out))
        {
            compound_auxiliary = Some(predicate);
            predicate = participle;
        }
    }
    clause.predicate = Some(predicate);
    if compound_auxiliary.is_some() {
        // A participle is not finite on its own; the clause is correct only
        // because the auxiliary carries the person and tense.
        out[predicate].role = SyntacticRole::Predicate;
        out[predicate].confidence = RoleConfidence::Weak;
    } else if out[predicate].role != SyntacticRole::Predicate {
        out[predicate].role = SyntacticRole::Predicate;
        out[predicate].confidence = RoleConfidence::Likely;
    }
    // One clause has one predicate. A second finite reading that also has a
    // nominal reading (`kirjaa` is the imperative of `kirjata` and the
    // partitive of `kirja`) is re-read as the nominal it can be, so the
    // object/subject pass can use it. Auxiliaries are not demoted; they are
    // linked to the predicate below.
    for &index in &words {
        if index == predicate || Some(index) == compound_auxiliary {
            continue;
        }
        if out[index].role != SyntacticRole::Predicate {
            continue;
        }
        let auxiliary = out[index]
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb));
        let nominal = out[index]
            .readings
            .iter()
            .any(|reading| is_nominal_pos(reading.part_of_speech));
        if !auxiliary && nominal {
            // Re-read the token from its non-verb readings instead of
            // dropping it to a bare `Unknown`. `siitä` is the elative pronoun
            // (and an adverb) beside the verb `siittää`; when the clause has a
            // clearer predicate, the pronoun/adverb reading is the one to
            // keep, so the attribute pass does not mistake an adverbial for
            // an attribute of the next nominal.
            let (role, confidence, case) = classify_without_verb(&out[index].readings);
            out[index].role = role;
            out[index].confidence = confidence;
            if role == SyntacticRole::Adverbial {
                out[index].case = case;
            }
        }
    }
    for &index in &words {
        if index == predicate {
            continue;
        }
        if is_negative_auxiliary(index, out) || Some(index) == compound_auxiliary {
            out[index].role = SyntacticRole::Auxiliary;
            out[index].confidence = RoleConfidence::Likely;
            out[index].head = Some(predicate);
        }
    }
    let predicate_position = words
        .iter()
        .position(|&index| index == predicate)
        .unwrap_or(0);
    let before = &words[..predicate_position];
    let after = &words[predicate_position + 1..];
    // A postposition and its genitive modifier (`Pöydän alla`) must be a
    // locative phrase before the existential test runs.
    mark_adpositions(&words, predicate, out);
    mark_invariant_determiners(&words, out);
    mark_temporal_essives(&words, tokens, out);
    // `joka päivä` may look like a nominative subject to the agreement rule
    // when the real subject is the dropped 1st/2nd-person pronoun.
    mark_frequency_determiners(&words, tokens, out);
    // Before any role is chosen: an attribute that precedes its head must not
    // become the subject or the object.
    mark_prenominal_attributes(&words, predicate, tokens, out);
    mark_direction_extent(&words, out);

    let passive = predicate_is_passive(predicate, out);
    // The negative auxiliary is the only thing that makes a clause negative.
    // A connegative main verb (`syö` in `en syö`) also carries a negative
    // polarity reading, but it does not negate an affirmative imperative
    // (`Syö puuro!`) or an infinitive (`Hänen pitää mennä`).
    let negative = words.iter().any(|&index| {
        out[index]
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
    });
    clause.negative = negative;
    // In an imperative, a nominative after the verb is the object
    // (`Syö puuro!`), never a post-predicate subject.
    let imperative = out[predicate]
        .readings
        .iter()
        .any(|reading| reading.mood == Some(Mood::Imperative) && is_finite_predicate(reading));

    let lemma = out[predicate].lemma.clone().unwrap_or_default();
    let is_olla = lemma == "olla";
    let is_change = matches!(lemma.as_str(), "tulla" | "tulee");
    // The locative may precede the predicate (`Pöydällä on kirja`) or follow
    // it in a question (`Onko sinulla kirjaa?`).
    let locative = before
        .iter()
        .chain(after)
        .any(|&index| is_locative_adverbial(index, tokens, out));
    // A genitive is included because the wrong-case subject of
    // `Pöytällä on kirjan.` must not stop the clause being recognised as
    // existential; the subject rule can then name the case error.
    let nominal_after = after.iter().any(|&index| {
        nominal_has_case(
            &out[index],
            &[Case::Nominative, Case::Partitive, Case::Genitive],
        )
    });
    // A nominative before the predicate is the subject of a copula
    // (`Helsingissä asuva mies on opettaja`), not an existential's postverbal
    // subject. Without this gate the locative of a modifying phrase would make
    // every `on` clause existential.
    let nominative_before = before
        .iter()
        .any(|&index| nominal_has_case(&out[index], &[Case::Nominative]));
    clause.existential = is_olla && locative && nominal_after && !nominative_before;
    // An intransitive verb of existence or appearance takes its indefinite
    // subject after the predicate: `Talossa asuu ihmisiä`, `Puutarhassa kasvaa
    // kukkia`. That postverbal partitive is a subject, not an object. A
    // nominative subject agrees with its verb (`Naapurissa asuvat Liisa ja
    // Pekka`), so only the partitive pattern is an existential here and the
    // agreement rule keeps seeing the nominative one.
    if !clause.existential
        && !is_olla
        && is_existential_verb(&lemma)
        && locative
        && !nominative_before
    {
        let partitive_after = after.iter().any(|&index| {
            out[index].role == SyntacticRole::Unknown
                && nominal_has_case(&out[index], &[Case::Partitive])
                && !is_clause_marker(index, tokens, out)
        });
        clause.existential = partitive_after;
    }

    let genitive_before = before.iter().rev().copied().find(|&index| {
        out[index].role == SyntacticRole::Unknown
            && nominal_has_case(&out[index], &[Case::Genitive])
            && !is_clause_marker(index, tokens, out)
    });
    let necessity = is_olla && genitive_before.is_some() && necessity_complement(after, out);
    // The necessive verbs take the genitive subject the same way `on
    // mentävä` does, but without `olla`: `Minun täytyy mennä`, `Hänen pitää
    // mennä`. A closed set, not a valency guess.
    let necessive = !is_olla
        && matches!(
            lemma.as_str(),
            "täytyä" | "pitää" | "kannattaa" | "sopia" | "kuulua"
        );
    let passive_construction = is_olla && !necessity && passive_participle_after(after, out);
    clause.passive = passive || passive_construction;

    let mut subject = None;
    if !passive && !passive_construction {
        // Inverted order: a finite predicate followed immediately by a
        // nominative personal pronoun has that pronoun as its subject
        // (`sanoi hän`, `kysyi hän sitten`). Whatever stands before the
        // predicate is then not the subject — in a quotation it is a vocative
        // (`..., Heintsi', sanoi hän sille toiselle`), and the quoted
        // predicate must not keep a subject role either.
        if !clause.existential && !necessity && !necessive {
            subject = after.first().copied().filter(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && is_nominative_personal_pronoun(index, out)
            });
        }
        if subject.is_some() {
            // fall through to the common assignment below
        } else if clause.existential {
            subject = after.iter().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && nominal_has_case(&out[index], &[Case::Nominative, Case::Partitive])
                    && !is_clause_marker(index, tokens, out)
            });
        } else if necessity || necessive {
            subject = genitive_before;
        }
        if subject.is_none() {
            subject = apposition_subject(before, predicate, tokens, out);
        }
        if subject.is_none() {
            // A true name (`Sami`) wins over a following capitalized common
            // word (`Ruotsalainen`) so a multiword name stays one subject.
            // The lemma's capitalization is the marker: a common word that
            // happens to be in the name table (`Vanha`) has a lowercase
            // lemma, and an inflected name has no case at all (S2).
            subject = before.iter().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && out[index]
                        .lemma
                        .as_deref()
                        .and_then(|lemma| lemma.chars().next())
                        .is_some_and(|first| first.is_uppercase())
                    && nominal_has_case(&out[index], &[Case::Nominative, Case::Partitive])
                    && !is_clause_marker(index, tokens, out)
            });
        }
        if subject.is_none() {
            // A capitalized word the lexicon does not know at all
            // (`Kenoveeva`, `Sikri`) is still the subject when it stands
            // immediately before the predicate. This is a role-only fallback:
            // it invents no morphology and no lemma, so no correction can be
            // offered for the name. A capitalized word before another nominal
            // (`Kenoveevan laulua`) is a modifier, not a subject.
            subject = before.iter().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && out[index].readings.is_empty()
                    && tokens[index].is_capitalised()
                    && !is_clause_marker(index, tokens, out)
                    && next_word_heads_predicate(index, &words, out)
            });
        }
        if subject.is_none() {
            subject = before.iter().rev().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && nominal_has_case(&out[index], &[Case::Nominative])
                    && !is_clause_marker(index, tokens, out)
            });
        }
    }
    // Finnish allows a subject after the predicate (`jossa roihusi tuli`).
    // Only a nominative can be that subject here; a partitive after the
    // predicate is the object of a negative/existential clause, and a
    // genitive is the total object. This is what keeps `Luen kirjaa` an
    // object while `roihusi tuli` gets its subject.
    // A copula's post-predicate nominative is its complement (`koska oli
    // nälkäinen`), not a subject. The existential case above is the exception
    // and is already handled.
    if subject.is_none() && !passive && !passive_construction && !imperative && !is_olla {
        subject = after.iter().copied().find(|&index| {
            out[index].role == SyntacticRole::Unknown
                && nominal_has_case(&out[index], &[Case::Nominative])
                && !is_clause_marker(index, tokens, out)
        });
    }
    if let Some(index) = subject {
        out[index].role = SyntacticRole::Subject;
        out[index].confidence = nominal_confidence(&out[index]);
        if let Some(noun) = &nominal_lemmas[index] {
            out[index].lemma = Some(noun.clone());
        }
        out[index].case = best_case(
            &out[index],
            &[Case::Nominative, Case::Partitive, Case::Genitive],
        );
        out[index].head = Some(predicate);
        clause.subject = Some(index);
        mark_coordinated_subjects(before, index, predicate, tokens, out, false);
        if index > predicate {
            // A postverbal coordinated subject keeps the same cue:
            // `Naapurissa asuvat Liisa ja Pekka`. Without this the second
            // conjunct would fall through to the object pass and a rection
            // rule could rewrite it. A copular complement is not at risk
            // because this pass only runs when the chosen subject itself
            // follows the predicate.
            mark_coordinated_subjects(after, index, predicate, tokens, out, true);
        }
        mark_numeral_complement(index, &words, out);
    }

    let object_cases: &[Case] = if negative {
        &[Case::Partitive]
    } else {
        &[
            Case::Partitive,
            Case::Genitive,
            Case::Accusative,
            Case::Nominative,
        ]
    };
    // A possessive total object is nominative (`söi aamupuuronsa`), not the
    // genitive the bare head paradigm also offers. Prefer the nominative when
    // the surface carries a possessive.
    let object = if passive || passive_construction {
        before.iter().chain(after).copied().find(|&index| {
            out[index].role == SyntacticRole::Unknown
                && nominal_has_case(&out[index], object_cases)
                && !is_clause_marker(index, tokens, out)
        })
    } else if !is_olla && !is_change {
        // A preverbal object is ordinary Finnish (`Kirjan luin`,
        // `ettei häntä pelottaisi`). Try the post-predicate position first,
        // then the pre-predicate one, skipping the subject and clause
        // markers. A numeral's counted noun is already a modifier by this
        // point, so the fallback cannot take it.
        after
            .iter()
            .copied()
            .chain(before.iter().rev().copied())
            .find(|&index| {
                Some(index) != subject
                    && out[index].role == SyntacticRole::Unknown
                    && nominal_has_case(&out[index], object_cases)
                    && !is_clause_marker(index, tokens, out)
            })
    } else {
        None
    };
    if let Some(index) = object {
        out[index].role = SyntacticRole::Object;
        out[index].confidence = nominal_confidence(&out[index]);
        if let Some(noun) = &nominal_lemmas[index] {
            out[index].lemma = Some(noun.clone());
        }
        let cases = if has_possessive(&out[index]) {
            POSSESSIVE_OBJECT_CASES
        } else {
            object_cases
        };
        out[index].case = best_case(&out[index], cases);
        out[index].head = Some(predicate);
        clause.object = Some(index);
        mark_coordinated_objects(after, index, predicate, cases, tokens, out);
        mark_numeral_complement(index, &words, out);
    }

    // A small productive valency table for verbs whose complement is a case
    // that would otherwise read as an adverbial: `näyttää väsyneeltä`,
    // `maalattiin punaiseksi`.
    if let Some(complement_cases) = predicate_complement_cases(&lemma) {
        let complement = after.iter().copied().find(|&index| {
            matches!(
                out[index].role,
                SyntacticRole::Unknown | SyntacticRole::Adverbial
            ) && nominal_has_case(&out[index], complement_cases)
                && !is_clause_marker(index, tokens, out)
        });
        if let Some(index) = complement {
            out[index].role = SyntacticRole::Complement;
            // The verb's valency, not the word's other readings, licenses
            // this case.
            out[index].confidence = RoleConfidence::Likely;
            out[index].case = best_case(&out[index], complement_cases);
            out[index].head = Some(predicate);
            clause.complement = Some(index);
        }
    }

    if is_olla || is_change {
        let complement_cases: &[Case] = if is_change {
            &[Case::Essive, Case::Translative]
        } else {
            &[
                Case::Nominative,
                Case::Partitive,
                Case::Essive,
                Case::Translative,
            ]
        };
        let complement = after.iter().copied().find(|&index| {
            Some(index) != subject
                && out[index].role == SyntacticRole::Unknown
                && nominal_has_case(&out[index], complement_cases)
                && !is_clause_marker(index, tokens, out)
        });
        // A participle after `olla` is the complement too: `Minun on mentävä`,
        // `Talo on rakennettu`. It has a verb reading and a case, not a noun
        // reading, so the nominal search above cannot see it.
        //
        // Two guards. Only `olla` takes a participle complement directly: a
        // change verb's complement is the translative the nominal search
        // already covers, so `Rinteellä tuli hirvittävä susi` keeps
        // `hirvittävä` as the attribute of `susi` rather than making it the
        // complement of `tuli`. And the token must still be unassigned, so an
        // attribute an earlier pass already placed cannot be re-purposed.
        let complement = complement.or_else(|| {
            if !is_olla {
                return None;
            }
            after.iter().copied().find(|&index| {
                Some(index) != subject
                    && out[index].role == SyntacticRole::Unknown
                    && out[index].readings.iter().any(|reading| {
                        reading.part_of_speech == Some(PartOfSpeech::Verb)
                            && reading.case.is_some()
                            && reading.mood != Some(Mood::Infinitive)
                    })
            })
        });
        if let Some(index) = complement {
            out[index].role = SyntacticRole::Complement;
            out[index].confidence = nominal_confidence(&out[index]);
            if let Some(noun) = &nominal_lemmas[index] {
                out[index].lemma = Some(noun.clone());
            }
            out[index].case = best_case(&out[index], complement_cases);
            out[index].head = Some(predicate);
            clause.complement = Some(index);
        }
    }

    mark_modifiers(&words, predicate, subject, tokens, out);

    for &index in &words {
        if out[index].role == SyntacticRole::Adverbial && out[index].head.is_none() {
            out[index].head = Some(predicate);
        }
    }
}

/// A personal pronoun in the nominative, the shape of an inverted subject.
fn is_nominative_personal_pronoun(index: usize, out: &[TokenSyntax]) -> bool {
    matches!(
        out[index].key.as_str(),
        "minä" | "sinä" | "hän" | "me" | "te" | "he"
    ) && nominal_has_case(&out[index], &[Case::Nominative])
}

/// An intransitive verb of existence or appearance whose postverbal
/// partitive is a subject rather than an object.
///
/// A closed list, not a valency guess: a transitive verb with a partitive
/// object (`Kaupassa ostan ruokaa`) must not turn into an existential.
fn is_existential_verb(lemma: &str) -> bool {
    matches!(
        lemma,
        "asua"
            | "elää"
            | "sijaita"
            | "kasvaa"
            | "kukoistaa"
            | "löytyä"
            | "ilmaantua"
            | "ilmestyä"
            | "seisoa"
            | "istua"
            | "maata"
            | "lojua"
            | "roikkua"
            | "virrata"
            | "juosta"
            | "lentää"
            | "uida"
            | "kaikua"
            | "soida"
            | "tuoksua"
            | "tapahtua"
            | "vallita"
            | "syttyä"
            | "roihuta"
    )
}

/// The `Minun on mentävä` construction: a genitive subject plus a participle.
fn necessity_complement(after: &[usize], out: &[TokenSyntax]) -> bool {
    after.iter().any(|&index| {
        out[index]
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Verb))
    })
}

/// A passive participle after `olla`: `Talo on rakennettu`.
fn passive_participle_after(after: &[usize], out: &[TokenSyntax]) -> bool {
    after.iter().any(|&index| {
        predicate_is_passive(index, out)
            && out[index]
                .readings
                .iter()
                .any(|reading| reading.case.is_some() && reading.mood != Some(Mood::Infinitive))
    })
}

/// Mark an adposition phrase as one locative/adverbial unit.
///
/// `Pöydän alla` is a postposition phrase, not a nominative `alla` subject:
/// `alla` is the adverbial the predicate governs, and `Pöydän` is its
/// genitive modifier. The genitive is caught here before the object pass
/// would offer it to a verb.
///
/// `ilman rahaa` and `ennen minua` are the mirror image: the preposition is
/// the adverbial and the following nominative/partitive nominal is its
/// complement. Without this pass `ilman`'s noun homonym (`ilma`) and the
/// preposition's nominative reading make it look like a subject.
fn mark_adpositions(words: &[usize], predicate: usize, out: &mut [TokenSyntax]) {
    for (position, &index) in words.iter().enumerate() {
        let (preposition, postposition) = adposition_kind(index, out);
        let next_nominal = words.get(position + 1).copied().filter(|&next| {
            out[next].role == SyntacticRole::Unknown && is_nominal(&out[next])
        });
        if preposition {
            if let Some(complement) = next_nominal {
                out[complement].role = SyntacticRole::Complement;
                out[complement].confidence = RoleConfidence::Likely;
                out[complement].case = out[complement]
                    .readings
                    .iter()
                    .find_map(|reading| reading.case);
                out[complement].head = Some(index);
            }
            out[index].role = SyntacticRole::Adverbial;
            out[index].confidence = RoleConfidence::Likely;
            out[index].case = None;
            out[index].head = Some(predicate);
            // A token that is also a postposition (`ennen`, `kautta`) falls
            // through only when it has no following nominal to govern.
            if next_nominal.is_some() {
                continue;
            }
        }
        if !matches!(out[index].role, SyntacticRole::Unknown | SyntacticRole::Adverbial)
            || !postposition
        {
            continue;
        }
        if position > 0 {
            let modifier = words[position - 1];
            if out[modifier].role == SyntacticRole::Unknown
                && nominal_has_case(&out[modifier], &[Case::Genitive])
            {
                out[modifier].role = SyntacticRole::Modifier;
                out[modifier].confidence = RoleConfidence::Likely;
                out[modifier].case = Some(Case::Genitive);
                out[modifier].head = Some(index);
            }
        }
        out[index].role = SyntacticRole::Adverbial;
        out[index].confidence = RoleConfidence::Likely;
        out[index].case = None;
        out[index].head = Some(predicate);
    }
}

/// Invariant determiners that do not agree with their head: `koko`.
///
/// `koko` has an adjective reading, but it is not inflected in `koko päivänä`
/// or `koko talon`. Marking it as the modifier keeps the object/subject pass
/// from taking it as a nominative object; the head carries the case.
const INVARIANT_DETERMINERS: &[&str] = &["koko"];

fn mark_invariant_determiners(words: &[usize], out: &mut [TokenSyntax]) {
    for (position, &index) in words.iter().enumerate() {
        let Some(lemma) = out[index].lemma.as_deref() else {
            continue;
        };
        if out[index].role != SyntacticRole::Unknown
            || !INVARIANT_DETERMINERS.contains(&lemma.to_lowercase().as_str())
        {
            continue;
        }
        let Some(&head) = words.get(position + 1) else {
            continue;
        };
        if !is_nominal(&out[head]) {
            continue;
        }
        out[index].role = SyntacticRole::Modifier;
        out[index].confidence = RoleConfidence::Likely;
        out[index].case = None;
        out[index].head = Some(head);
    }
}

/// Temporal nouns whose essive is an adverbial of time, not an attribute and
/// not a predicate complement: `päivänä`, `iltana`, `vuonna`.
const TEMPORAL_ESSIVE_NOUNS: &[&str] = &[
    "aamu",
    "aika",
    "hetki",
    "ilta",
    "kesä",
    "kevät",
    "kuukausi",
    "päivä",
    "syksy",
    "talvi",
    "viikko",
    "viikonloppu",
    "vuosi",
    "yö",
];

fn mark_temporal_essives(words: &[usize], tokens: &[Token], out: &mut [TokenSyntax]) {
    for &index in words {
        if out[index].role != SyntacticRole::Unknown
            || is_clause_marker(index, tokens, out)
            || !nominal_has_case(&out[index], &[Case::Essive])
        {
            continue;
        }
        let Some(lemma) = out[index].lemma.as_deref() else {
            continue;
        };
        if !TEMPORAL_ESSIVE_NOUNS.contains(&lemma.to_lowercase().as_str()) {
            continue;
        }
        out[index].role = SyntacticRole::Adverbial;
        out[index].confidence = RoleConfidence::Likely;
        out[index].case = Some(Case::Essive);
    }
}

/// Whether a token is lexically a preposition and/or postposition.
///
/// The morphology layer keeps one part of speech per reading, so `ennen`
/// (listed as `adverbi, prepositio, postpositio`) is read only as an adverb.
/// The closed-class row is the authority here, but it is consulted only for a
/// token that already has an adverb/adposition reading.
fn adposition_kind(index: usize, out: &[TokenSyntax]) -> (bool, bool) {
    let Some(syntax) = out.get(index) else {
        return (false, false);
    };
    let candidate = syntax.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Adverb | PartOfSpeech::Preposition | PartOfSpeech::Postposition)
        )
    });
    if !candidate {
        return (false, false);
    }
    let lower = syntax.key.as_str();
    let mut preposition = syntax
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Preposition));
    let mut postposition = syntax
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Postposition));
    for row in crate::kotus::sanalista().by_word(lower) {
        for pos in PartOfSpeech::parse_all(row.part_of_speech()) {
            match pos {
                PartOfSpeech::Preposition => preposition = true,
                PartOfSpeech::Postposition => postposition = true,
                _ => {}
            }
        }
    }
    (preposition, postposition)
}

/// Temporal nouns that form a frequency adverbial with `joka`/`jokainen`.
///
/// The list is deliberately closed. A determiner before a non-temporal noun
/// (`joka mies`) leaves the noun as the subject, so a real number error still
/// fires.
const FREQUENCY_NOUNS: &[&str] = &[
    "aamu",
    "ilta",
    "kesä",
    "kerta",
    "kevät",
    "kuukausi",
    "lauantai",
    "maanantai",
    "minuutti",
    "perjantai",
    "päivä",
    "sekunti",
    "sunnuntai",
    "syksy",
    "talvi",
    "tiistai",
    "torstai",
    "tunti",
    "viikko",
    "viikonloppu",
    "vuosi",
    "yö",
];

/// Mark `joka päivä` / `jokainen päivä` as a frequency adverbial.
///
/// `Joka päivä juon kahvia` has no overt subject. Without this pass `päivä`
/// is read as a 3rd-person subject and the agreement rule offers `juo`. The
/// pass stays out of 3rd-person clauses (`Joka päivä on kaunis`,
/// `Joka mies tietää sen`) so the noun can still be a real subject there.
fn mark_frequency_determiners(words: &[usize], tokens: &[Token], out: &mut [TokenSyntax]) {
    if !words
        .iter()
        .any(|&index| non_third_person(index, tokens, out))
    {
        return;
    }
    for (position, &index) in words.iter().enumerate() {
        let lower = tokens[index].text.to_lowercase();
        if !matches!(lower.as_str(), "joka" | "jokainen") {
            continue;
        }
        let Some(&noun) = words.get(position + 1) else {
            continue;
        };
        if out[noun].role != SyntacticRole::Unknown
            || !nominal_has_case(&out[noun], &[Case::Nominative])
            || comma_between(tokens, index, noun)
        {
            continue;
        }
        let Some(lemma) = out[noun].lemma.as_deref() else {
            continue;
        };
        if !FREQUENCY_NOUNS.contains(&lemma.to_lowercase().as_str()) {
            continue;
        }
        out[index].role = SyntacticRole::Modifier;
        out[index].confidence = RoleConfidence::Likely;
        out[index].head = Some(noun);
        out[noun].role = SyntacticRole::Adverbial;
        out[noun].confidence = RoleConfidence::Likely;
        out[noun].case = Some(Case::Nominative);
    }
}

/// Whether this token's finite reading (or negative auxiliary) is 1st/2nd
/// person, which is when a dropped subject is normal in Finnish.
fn non_third_person(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    if out[index].readings.iter().any(|reading| {
        is_finite_predicate(reading)
            && matches!(reading.person, Some(Person::First | Person::Second))
    }) {
        return true;
    }
    out[index]
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
        && crate::expand::negation::person_number(&tokens[index].text)
            .is_some_and(|(person, _)| matches!(person, Person::First | Person::Second))
}

/// Attributes, genitive modifiers, titles, and appositions.
fn mark_modifiers(
    words: &[usize],
    predicate: usize,
    subject: Option<usize>,
    tokens: &[Token],
    out: &mut [TokenSyntax],
) {
    for &index in words {
        if out[index].role != SyntacticRole::Unknown
            || (!is_nominal(&out[index]) && !is_participle_attribute(index, out))
            || is_clause_marker(index, tokens, out)
        {
            continue;
        }

        // A capitalized word between the subject and the predicate is the rest
        // of a multiword name (`Sami Ruotsalainen`), not an object.
        if let Some(subject) = subject
            && index > subject
            && index < predicate
            && tokens[index].is_capitalised()
        {
            out[index].role = SyntacticRole::Modifier;
            out[index].confidence = RoleConfidence::Weak;
            out[index].head = Some(subject);
            continue;
        }

        let next = words
            .iter()
            .copied()
            .find(|&other| other > index && other != predicate);
        if let Some(next) = next.filter(|&other| other != predicate) {
            let genitive = nominal_has_case(&out[index], &[Case::Genitive]);
            let next_is_nominal = is_nominal(&out[next]);
            if genitive && next_is_nominal {
                out[index].role = SyntacticRole::Modifier;
                out[index].confidence = RoleConfidence::Likely;
                out[index].case = Some(Case::Genitive);
                out[index].head = Some(next);
                continue;
            }
            if Some(next) == subject && index < predicate {
                out[index].role = SyntacticRole::Modifier;
                out[index].confidence = RoleConfidence::Weak;
                out[index].head = Some(next);
            }
        }
    }
}

/// Direction adverbs that take an extent nominal before them.
///
/// `vuorta ylöspäin`, `merta pitkin`: the partitive nominal names the extent
/// the adverb measures, so it is the adverb's complement, not the verb's
/// argument. Without this a rection frame for the verb rewrites it
/// (`tulla` + illative offered `vuorta` -> `vuoreen`).
///
/// The set is closed and limited to the unambiguous `-päin` direction
/// adverbs. `päin` itself is deliberately absent: with a noun or pronoun
/// (`kotiin päin`, `meillä päin`) the two words stay apart and `päin` is not
/// a postposition (see `AGENTS.md`).
const DIRECTION_EXTENT_ADVERBS: &[&str] = &["ylöspäin", "alaspäin", "eteenpäin", "taaksepäin"];

fn mark_direction_extent(words: &[usize], out: &mut [TokenSyntax]) {
    for (position, &index) in words.iter().enumerate() {
        if !DIRECTION_EXTENT_ADVERBS.contains(&out[index].key.as_str()) || position == 0 {
            continue;
        }
        let extent = words[position - 1];
        if out[extent].role == SyntacticRole::Unknown
            && nominal_has_case(&out[extent], &[Case::Partitive])
        {
            out[extent].role = SyntacticRole::Complement;
            out[extent].confidence = RoleConfidence::Likely;
            out[extent].case = Some(Case::Partitive);
            out[extent].head = Some(index);
        }
    }
}

/// Mark a prenominal attribute so the subject and object passes skip it.
///
/// `Rinteellä tuli hirvittävä susi` has `susi` as its subject; without this
/// the subject pass takes `hirvittävä`, because it is the first nominative
/// after the predicate, and every later rule inherits the mistake. The same
/// holds for a demonstrative determiner (`se kesy hirvi`) and for an agreeing
/// participle.
///
/// The cue is deliberately narrow: an adjective, ordinal, participle or
/// demonstrative with an Unknown role, immediately followed (no punctuation
/// in between, and not by the predicate) by a noun or ordinal in the
/// nominative. A nominalised adjective with no head (`Nuoret tulivat`) is not
/// touched, and a cardinal numeral is left to [`mark_numeral_complement`].
fn mark_prenominal_attributes(
    words: &[usize],
    predicate: usize,
    tokens: &[Token],
    out: &mut [TokenSyntax],
) {
    // Adjectives and participles first: the head they name is the noun right
    // after them. `se kesy hirvi` needs both passes: `kesy` attaches to
    // `hirvi`, and then `se` can look past the attribute it now knows about.
    for (position, &index) in words.iter().enumerate() {
        if out[index].role != SyntacticRole::Unknown
            || is_clause_marker(index, tokens, out)
            || !is_attribute_capable(index, out)
            || is_demonstrative(index, out)
        {
            continue;
        }
        let Some(&next) = words.get(position + 1) else {
            continue;
        };
        if next == predicate || !is_noun_head(next, out) {
            continue;
        }
        if !nominal_has_case(&out[index], &[Case::Nominative])
            || !nominal_has_case(&out[next], &[Case::Nominative])
        {
            continue;
        }
        mark_as_modifier(index, next, out);
    }
    // Then the demonstrative determiners, which may have one or more
    // attributes between them and their head (`se kesy hirvi`).
    for (position, &index) in words.iter().enumerate() {
        if out[index].role != SyntacticRole::Unknown
            || is_clause_marker(index, tokens, out)
            || !is_demonstrative(index, out)
            || !nominal_has_case(&out[index], &[Case::Nominative])
        {
            continue;
        }
        let head = words[position + 1..]
            .iter()
            .take_while(|&&word| word != predicate)
            .find(|&&word| {
                out[word].role != SyntacticRole::Modifier && is_noun_head(word, out)
            });
        let Some(head) = head.copied() else {
            continue;
        };
        if !nominal_has_case(&out[head], &[Case::Nominative]) {
            continue;
        }
        mark_as_modifier(index, head, out);
    }
}

fn mark_as_modifier(index: usize, head: usize, out: &mut [TokenSyntax]) {
    out[index].role = SyntacticRole::Modifier;
    out[index].confidence = RoleConfidence::Weak;
    out[index].head = Some(head);
}

/// The demonstratives that can stand as a determiner before a noun.
fn is_demonstrative(index: usize, out: &[TokenSyntax]) -> bool {
    matches!(
        out[index].key.as_str(),
        "se" | "tämä" | "tuo" | "ne" | "nämä" | "nuo" | "sama" | "samat"
    )
}

/// The parts of speech (and the closed determiner set) that can be a
/// prenominal attribute.
fn is_attribute_capable(index: usize, out: &[TokenSyntax]) -> bool {
    is_participle_attribute(index, out)
        || out[index].readings.iter().any(|reading| {
            matches!(
                reading.part_of_speech,
                Some(PartOfSpeech::Adjective | PartOfSpeech::Ordinal)
            )
        })
}

/// A head that a prenominal attribute can modify: a noun or an ordinal.
fn is_noun_head(index: usize, out: &[TokenSyntax]) -> bool {
    out[index].readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Noun | PartOfSpeech::Ordinal)
        )
    })
}

/// Mark the counted noun of a numeral phrase: `kaksi lasta`, `kaksi miestä`.
///
/// The numeral is the head (subject or object); the following partitive is
/// its modifier, not an independent complement. The partitive reading is
/// chosen explicitly because `lasta` also has a lexical nominative reading.
fn mark_numeral_complement(head: usize, words: &[usize], out: &mut [TokenSyntax]) {
    let is_numeral = out[head]
        .readings
        .iter()
        .any(|reading| matches!(reading.part_of_speech, Some(PartOfSpeech::Numeral)));
    if !is_numeral {
        return;
    }
    let Some(&next) = words.iter().find(|&&index| index > head) else {
        return;
    };
    if out[next].role == SyntacticRole::Unknown
        && nominal_has_case(&out[next], &[Case::Partitive])
    {
        out[next].role = SyntacticRole::Modifier;
        out[next].confidence = RoleConfidence::Likely;
        out[next].case = Some(Case::Partitive);
        out[next].head = Some(head);
    }
}

/// Whether a token carries a possessive suffix.
fn has_possessive(syntax: &TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| reading.possessive.is_some())
}

/// Mark `maidon` in `Sami söi puuron ja maidon` as a coordinated object.
fn mark_coordinated_objects(
    after: &[usize],
    object: usize,
    predicate: usize,
    object_cases: &[Case],
    tokens: &[Token],
    out: &mut [TokenSyntax],
) {
    for &index in after {
        if index == object
            || out[index].role != SyntacticRole::Unknown
            || !nominal_has_case(&out[index], object_cases)
            || is_clause_marker(index, tokens, out)
        {
            continue;
        }
        let (left, right) = if index < object {
            (index, object)
        } else {
            (object, index)
        };
        if has_coordinator_between(tokens, left, right) {
            out[index].role = SyntacticRole::Object;
            out[index].confidence = nominal_confidence(&out[index]);
            let cases = if has_possessive(&out[index]) {
                POSSESSIVE_OBJECT_CASES
            } else {
                object_cases
            };
            out[index].case = best_case(&out[index], cases);
            out[index].head = Some(predicate);
        }
    }
}

/// Mark `Liisa` in `Sami ja Liisa söivät` as a coordinated subject.
fn mark_coordinated_subjects(
    words: &[usize],
    subject: usize,
    predicate: usize,
    tokens: &[Token],
    out: &mut [TokenSyntax],
    nouns_only: bool,
) {
    for &index in words {
        if index == subject
            || out[index].role == SyntacticRole::Subject
            || !nominal_has_case(&out[index], &[Case::Nominative])
            || is_clause_marker(index, tokens, out)
        {
            continue;
        }
        // A postverbal coordinated subject is a noun phrase: `Naapurissa
        // asuvat Liisa ja Pekka`. An adjective joined by `ja` is a
        // coordinated complement or attribute, not a second subject —
        // `oli hän yhtä viaton ja herttainen kuin kaunis` must not become a
        // plural subject.
        if nouns_only
            && !out[index].readings.iter().any(|reading| {
                matches!(
                    reading.part_of_speech,
                    Some(PartOfSpeech::Noun | PartOfSpeech::Ordinal | PartOfSpeech::Numeral)
                )
            })
        {
            continue;
        }
        let (left, right) = if index < subject {
            (index, subject)
        } else {
            (subject, index)
        };
        if has_coordinator_between(tokens, left, right) {
            out[index].role = SyntacticRole::Subject;
            out[index].confidence = nominal_confidence(&out[index]);
            out[index].case = Some(Case::Nominative);
            out[index].head = Some(predicate);
        }
    }
}

/// Is there a coordinating conjunction between two token indices?
fn has_coordinator_between(tokens: &[Token], left: usize, right: usize) -> bool {
    tokens
        .iter()
        .skip(left + 1)
        .take(right.saturating_sub(left + 1))
        .filter(|token| token.is_word())
        .any(|token| {
            let lower = token.text.to_lowercase();
            // The second half of `sekä ... että` coordinates the same slots
            // as `ja`; the clause splitter already leaves that `että` alone.
            is_coordinating(&lower) || lower == "että"
        })
}

/// The person and number a clause's subject roles agree on, for deciding
/// whether a following clause may inherit that subject.
fn clause_subject_person_number(
    clause: &Clause,
    out: &[TokenSyntax],
) -> Option<(Person, Number)> {
    let subjects: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&index| out[index].role == SyntacticRole::Subject)
        .collect();
    if subjects.is_empty() {
        return None;
    }
    if subjects.len() > 1 {
        // Coordinated subjects are plural unless a 1st/2nd-person pronoun is
        // involved, in which case the person needs the rule this phase does
        // not have. Unknown means "do not block sharing" at the call site.
        if subjects.iter().any(|&index| {
            matches!(
                personal_pronoun(&out[index].key),
                Some((Person::First | Person::Second, _))
            )
        }) {
            return None;
        }
        return Some((Person::Third, Number::Plural));
    }
    subject_person_number(subjects[0], out)
}

/// The person and number of a single subject token, when they are unambiguous.
fn subject_person_number(index: usize, out: &[TokenSyntax]) -> Option<(Person, Number)> {
    if let Some(pair) = personal_pronoun(&out[index].key) {
        return Some(pair);
    }
    let mut numbers: Vec<Number> = out[index]
        .readings
        .iter()
        .filter_map(|reading| reading.number)
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    let number = match numbers.as_slice() {
        [number] => *number,
        _ => return None,
    };
    let mut persons: Vec<Person> = out[index]
        .readings
        .iter()
        .filter_map(|reading| reading.person)
        .collect();
    persons.sort_unstable();
    persons.dedup();
    let person = match persons.as_slice() {
        [person] => *person,
        [] => Person::Third,
        _ => return None,
    };
    Some((person, number))
}

/// Whether a clause's predicate can agree with the subject of the clause
/// before it, so the subject may be inherited.
///
/// `Sami söi ja joi` shares a 3rd-person singular subject. `Kenoveeva kehräsi
/// ja lauloi` shares a name whose number is unknown. `Eräänä iltana istuivat
/// molemmat ...; Kenoveeva kehräsi` must not copy the plural `molemmat` into a
/// singular predicate just because both are 3rd person.
fn predicate_matches_previous_subject(
    clause: &Clause,
    previous: &Clause,
    tokens: &[Token],
    out: &[TokenSyntax],
) -> bool {
    let Some(predicate) = clause.predicate else {
        return false;
    };
    let words: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    let Some((person, number)) = predicate_person_number(predicate, &words, tokens, out) else {
        return true;
    };
    clause_subject_person_number(previous, out)
        .is_none_or(|(subject_person, subject_number)| {
            subject_person == person && subject_number == number
        })
}

/// Share the subject of a coordinated or non-finite clause with the one before.
///
/// `clause_groups` holds the clause-group index (see [`clause_groups`]); two
/// clauses may share a group across a parenthetical bracket boundary.
pub(super) fn share_clause_roles(
    clauses: &mut [Clause],
    clause_groups: &[usize],
    tokens: &[Token],
    out: &[TokenSyntax],
) {
    for index in 1..clauses.len() {
        if clause_groups[index] != clause_groups[index - 1] {
            continue;
        }
        if clauses[index].subject.is_some() {
            continue;
        }
        // A relative clause introduces its own subject (`poika, joka oli
        // syntynyt ...`). It must not copy the main clause's subject: the
        // relative pronoun is the controller, and inheriting `he` would ask
        // the singular `oli` to agree with a plural subject.
        if is_relative_word(clauses[index].tokens.start, tokens, out) {
            continue;
        }
        let starts_with_coordinator = tokens
            .get(clauses[index].tokens.start)
            .is_some_and(|token| is_coordinating(&token.text.to_lowercase()));
        let predicate_is_non_finite = clauses[index].predicate.is_some_and(|predicate| {
            out[predicate]
                .readings
                .iter()
                .any(|reading| matches!(reading.mood, Some(Mood::Infinitive | Mood::Instructive)))
        });
        // A dropped subject in a subordinate clause is recoverable from
        // agreement: `Sami söi, koska oli nälkäinen`. First/second person
        // (`Kun hän tuli, söin`) must not inherit the previous subject, and a
        // plural subject must not be copied into a singular predicate
        // (`... istuivat molemmat ...; Kenoveeva kehräsi`).
        let predicate_fits = predicate_matches_previous_subject(
            &clauses[index],
            &clauses[index - 1],
            tokens,
            out,
        );
        if starts_with_coordinator || predicate_is_non_finite || predicate_fits {
            clauses[index].subject = clauses[index - 1].subject;
        }
    }
}

/// Local adverbs and postpositions that function as the locative of an
/// existential without carrying a case of their own (`Pöydän alla on kissa`).
const LOCAL_ADVERBS: &[&str] = &[
    "alla",
    "alta",
    "alle",
    "päällä",
    "päältä",
    "päälle",
    "sisällä",
    "sisältä",
    "sisälle",
    "ulkona",
    "ulkoa",
    "ulko",
    "vieressä",
    "vierestä",
    "viereen",
    "luona",
    "luota",
    "luo",
    "kotona",
    "kotoa",
    "kotiin",
    "täällä",
    "täältä",
    "tänne",
    "siellä",
    "sieltä",
    "sinne",
    "tuolla",
    "tuolta",
    "tuonne",
    "ylhäällä",
    "alhaalla",
    "lähellä",
    "kaukana",
    "välissä",
    "keskellä",
    "edessä",
    "takana",
    "rinnalla",
    "mukana",
    "poissa",
    "paikalla",
    "matkalla",
];

/// An adverbial that can be the locative of an existential.
fn is_locative_adverbial(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    out[index].role == SyntacticRole::Adverbial
        && (out[index].case.is_some_and(is_local_case)
            || out[index]
                .readings
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Postposition))
            || LOCAL_ADVERBS.contains(&tokens[index].text.to_lowercase().as_str()))
}

const ABLATIVE_COMPLEMENT: &[Case] = &[Case::Ablative];
const TRANSLATIVE_COMPLEMENT: &[Case] = &[Case::Translative];
/// Verbs of remaining: `Kenoveeva jäi tunnottomana`, `Hän pysyi vaiti`. The
/// essive/translative nominal is the predicate's complement, so a rection
/// frame for the verb's locative sense must not rewrite it.
const ESSIVE_COMPLEMENT: &[Case] = &[Case::Essive, Case::Translative];

/// The complement case of a small, productive verb family.
///
/// The generic case classifier calls these adverbials because the case is
/// local (`väsyneeltä`) or a result state (`punaiseksi`). Their verb governs
/// the case, so a closed table is enough and no suffix rule is involved.
fn predicate_complement_cases(lemma: &str) -> Option<&'static [Case]> {
    match lemma {
        "näyttää" | "tuntua" | "vaikuttaa" | "kuulostaa" | "haisahtaa" | "maistua" => {
            Some(ABLATIVE_COMPLEMENT)
        }
        "maalata" | "värittää" | "kutsua" | "nimittää" | "valita" | "tehdä" => {
            Some(TRANSLATIVE_COMPLEMENT)
        }
        "jäädä" | "pysyä" | "viipyä" | "säilyä" => Some(ESSIVE_COMPLEMENT),
        _ => None,
    }
}

/// Local cases that can license an adverbial.
fn is_local_case(case: Case) -> bool {
    matches!(
        case,
        Case::Inessive
            | Case::Elative
            | Case::Illative
            | Case::Adessive
            | Case::Ablative
            | Case::Allative
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Number, Person, Tense};

    fn finite_verb() -> Tags {
        Tags::finite(Tense::Past, Person::Third, Number::Singular).of_pos(PartOfSpeech::Verb)
    }

    #[test]
    fn a_confident_baseline_role_is_not_overridden() {
        let adverb = Tags::UNKNOWN.of_pos(PartOfSpeech::Adverb);
        let noun = Tags::nominal(Case::Nominative).of_pos(PartOfSpeech::Noun);
        let (role, confidence, _) =
            classify_with_preference(&[adverb, adverb], Some(&noun));
        assert_eq!(role, SyntacticRole::Adverbial);
        assert_eq!(confidence, RoleConfidence::Likely);
    }

    #[test]
    fn a_weak_predicate_can_be_resolved_to_a_nominal() {
        let verb = finite_verb();
        let noun = Tags::nominal(Case::Nominative).of_pos(PartOfSpeech::Noun);
        let (role, confidence, _) = classify_with_preference(&[verb, noun], Some(&noun));
        assert_eq!(role, SyntacticRole::Unknown);
        assert_eq!(confidence, RoleConfidence::Weak);
    }

    #[test]
    fn a_weak_predicate_preference_keeps_the_baseline_role() {
        let verb = finite_verb();
        let noun = Tags::nominal(Case::Nominative).of_pos(PartOfSpeech::Noun);
        let (role, confidence, _) =
            classify_with_preference(&[verb, noun], Some(&verb));
        assert_eq!(role, SyntacticRole::Predicate);
        assert_eq!(confidence, RoleConfidence::Weak);
    }

    #[test]
    fn a_weak_adverbial_keeps_the_baseline_role() {
        let partitive = Tags::nominal(Case::Partitive).of_pos(PartOfSpeech::Noun);
        let inessive = Tags::nominal(Case::Inessive).of_pos(PartOfSpeech::Noun);
        let illative = Tags::nominal(Case::Illative).of_pos(PartOfSpeech::Noun);
        let readings = [partitive, inessive, illative];
        assert_eq!(
            classify_with_preference(&readings, Some(&illative)),
            classify(&readings)
        );
    }

    #[test]
    fn no_preference_is_the_baseline_classifier() {
        let readings = [
            Tags::nominal(Case::Nominative).of_pos(PartOfSpeech::Noun),
            Tags::UNKNOWN.of_pos(PartOfSpeech::Adverb),
        ];
        assert_eq!(
            classify_with_preference(&readings, None),
            classify(&readings)
        );
    }
}
