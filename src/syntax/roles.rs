//! Clause role resolution.

use crate::tags::{Case, Mood, PartOfSpeech, Person, Tags};
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
            out[index].role = SyntacticRole::Unknown;
            out[index].confidence = RoleConfidence::Weak;
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
    // `joka päivä` may look like a nominative subject to the agreement rule
    // when the real subject is the dropped 1st/2nd-person pronoun.
    mark_frequency_determiners(&words, tokens, out);

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
    let nominative_after = after
        .iter()
        .any(|&index| nominal_has_case(&out[index], &[Case::Nominative, Case::Partitive]));
    // A nominative before the predicate is the subject of a copula
    // (`Helsingissä asuva mies on opettaja`), not an existential's postverbal
    // subject. Without this gate the locative of a modifying phrase would make
    // every `on` clause existential.
    let nominative_before = before
        .iter()
        .any(|&index| nominal_has_case(&out[index], &[Case::Nominative]));
    clause.existential = is_olla && locative && nominative_after && !nominative_before;

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

    let mut subject = None;
    if !passive && !passive_construction {
        if clause.existential {
            subject = after.iter().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && nominal_has_case(&out[index], &[Case::Nominative, Case::Partitive])
                    && !is_clause_marker(index, tokens, out)
            });
        } else if necessity || necessive {
            subject = genitive_before;
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
        mark_coordinated_subjects(before, index, predicate, tokens, out);
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
        after.iter().copied().find(|&index| {
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
        let complement = complement.or_else(|| {
            after.iter().copied().find(|&index| {
                Some(index) != subject
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
        if out[index].role != SyntacticRole::Unknown || !postposition {
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
    before: &[usize],
    subject: usize,
    predicate: usize,
    tokens: &[Token],
    out: &mut [TokenSyntax],
) {
    for &index in before {
        if index == subject
            || out[index].role == SyntacticRole::Subject
            || !nominal_has_case(&out[index], &[Case::Nominative])
            || is_clause_marker(index, tokens, out)
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
        // (`Kun hän tuli, söin`) must not inherit the previous subject.
        let predicate_is_third_person = clauses[index].predicate.is_some_and(|predicate| {
            !predicate_is_passive(predicate, out)
                && out[predicate].readings.iter().any(|reading| {
                    is_finite_predicate(reading) && reading.person == Some(Person::Third)
                })
        });
        if starts_with_coordinator || predicate_is_non_finite || predicate_is_third_person {
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
