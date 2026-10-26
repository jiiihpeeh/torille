use crate::kotus::{ClassId, Inflection};
use crate::spec::Paradigm;
use crate::tags::{
    Case, Confidence, Form, Mood, Number, PartOfSpeech, Person, Possessive, Tags, Tense, Voice,
};

use super::Generated;
use super::align::{find_last, split_point, stem_alternation};
use super::compound::Heads;
use super::slot::{Slot, slot_role};
use super::sound::is_vowel_byte;
use crate::is_vowel;

/// The eight cases the catalogue tabulates — the nominative, genitive,
/// partitive and illative in both numbers. These are the lexical cases the
/// oblique never builds, so they are owned by the alignment alone.
const LEXICAL: &[(Case, Number)] = &[
    (Case::Nominative, Number::Singular),
    (Case::Genitive, Number::Singular),
    (Case::Partitive, Number::Singular),
    (Case::Illative, Number::Singular),
    (Case::Nominative, Number::Plural),
    (Case::Genitive, Number::Plural),
    (Case::Partitive, Number::Plural),
    (Case::Illative, Number::Plural),
];

/// Inflects one spelling of the lemma, appending to `out`.
pub(super) fn inflect_from(
    lemma: &str,
    inflection: Inflection,
    out: &mut Vec<Generated>,
    para: &Paradigm,
    model: &str,
    heads: &[Heads],
    pos: PartOfSpeech,
) {
    // Where this spelling's forms begin in `out`. A fixed-stem class is
    // inflected from two spellings (the lemma and the fixed stem) into the
    // same `out`, so a demotion must not reach back into the previous
    // spelling's forms and demote them on this one's evidence.
    let start = out.len();
    // Split points are measured in characters, not bytes: Finnish vowels such
    // as `ä` are two bytes and slicing at a byte offset would panic.
    let mut char_offsets: Vec<usize> = lemma.char_indices().map(|(i, _)| i).collect();
    char_offsets.push(lemma.len()); // end-of-string split point
    let char_len = char_offsets.len() - 1;

    // Which side of the `a`/`ä` split the ending's vowel comes from. The
    // ending is the model's, but a class is one word and Finnish puts words of
    // both harmonies in the same class, so the model's own vowel is the wrong
    // one whenever the lemma is on the other side of the split: class 3 is
    // modelled on the back-harmony `valtio` and holds the front-harmony `eliö`
    // with it. A lemma that harmonises the same way as the model is already
    // carrying the right vowel, which is why the model's own forms, the ones
    // the catalogue tabulates, are left alone.
    let lemma_harmony = crate::grammar::harmony(lemma);
    let read_harmony = lemma_harmony != crate::grammar::harmony(model);

    // A noun class is modelled on a word its own members are cut from, but a
    // verb class is modelled on one particular verb and holds every verb of
    // that shape, so a lemma is nearly always a different word from the model
    // with a stem that has nothing to do with it: `lcp("salata", "hellätä")` is
    // nothing at all. Cutting the lemma where the model's stem ends is then
    // reading one word out of another, and every cut lands where the class never
    // licensed, which is where the plain `hella`, `hellu` and `hello` of a
    // `hellätä` came from. A lemma that is not the model is left to the verb
    // generator, which knows the two forms that pin a stem down.
    let modelled_on_lemma = !crate::grammar::is_verb_class(inflection.class);

    // A class that fixes its stem has nothing to graduate. The flag belongs to
    // the lemma, and a fixed stem is a spelling the class gives rather than a
    // cut of the lemma, so the one says nothing about the other: `lämmin` is
    // `35*H` and the class's stem is `lämpimä`, whose `mp` the lemma's own `H`
    // would otherwise find, giving the `lämmimä` of `lämmimän` and `lämmimät`.
    // Those are not words of any language — the plural of `lämmin` is
    // `lämpimät`, on the same stem as the singular.
    let gradates = inflection
        .gradation
        .filter(|_| crate::grammar::fixed_stem(inflection.class).is_none());

    // A reverse-gradation word carries the weak grade in its nominative
    // (`hammas` is `hamm-` with `mm`, `ahdin` is `ahd-` with `d` and `aallotar`
    // is `aall-` with a single `t`), so every other form is the strong grade:
    // the genitive is `hampaan` and the plural `hampaat`, not `hammaan` and
    // `hammaat`. The weak grade has to sit inside the stem and not at the very
    // start — a word-initial `k` under a kk:k flag is a stray letter, not a
    // gradation — but a single-letter weak grade is a real one, so `ahdin` and
    // `aallotar` are caught with the doubled `hammas`.
    let reverse = inflection.gradation.is_some_and(|g| {
        if crate::grammar::is_verb_class(inflection.class) {
            false
        } else {
            g.weak_site(lemma).is_some_and(|at| at > 0) && g.site(lemma).is_none()
        }
    });

    // The genitive shows the stem alternation the class needs, and applying it
    // is what turns `lapsi` into `lasta` and `kaksi` into `kahden`. Three
    // classes need more than the alignment gives, and the table has them.
    let table_alt = crate::grammar::stem_alternation(inflection.class);
    let genitive = para.slots.get(1).map_or("", |s| s.model.as_str());
    let auto_alt = stem_alternation(model, genitive);
    // Whether the class's own model keeps its lemma as an oblique stem, as
    // class 7's `ovi` does (`oviin` is the whole `ovi`) or cuts it, as class
    // 38's `nainen` does (`naisiin` is `nais`, never the whole word). A class
    // that cuts the model has no member whose whole lemma is a stem, so a
    // member's whole-lemma oblique forms (`aakkonensen` from `aakkonen`)
    // are slid readings.
    let model_is_stem = para
        .slots
        .iter()
        .enumerate()
        .any(|(i, s)| i != 0 && s.variants().any(|v: &str| v.starts_with(model)));

    // The words the catalogue spells out for this lemma, kept as they are
    // pushed. The alignment drops the slid readings against these and against
    // nothing else, because a word the class tabulated is a word nothing else
    // had to be guessed into: what the possessive and case derivations build
    // is a different kind of thing, and two of those landing on one spelling is
    // two readings rather than one word and a mistake.
    let mut spelled_out: Vec<String> = Vec::new();
    // The words the catalogue tabulates for this class, one spelling per slot
    // and alternation. These are the lexical cases (the nominative, genitive,
    // partitive and illative) that the oblique does not own, so the alignment
    // is their only source and a demotion must spare them.
    let catalogue_words: std::collections::HashSet<String> = para
        .slots
        .iter()
        .flat_map(|s| s.variants())
        .map(|v| v.to_string())
        .collect();

    for (i, slot) in para.slots.iter().enumerate() {
        // The tabulated alternations only start at the partitive. `lapsi` gives
        // `lapsen` in the genitive and `lasta` in the partitive, so a rule that
        // ignored the slot would have to break one of them.
        let table_alt = if i >= 2 { table_alt } else { None };
        let role = slot_role(i);
        // What a form of this slot is, grammatically. The catalogue fixes the
        // case and the number by position for a noun, so this is free, and it
        // is not free for a verb: a verb paradigm has the present, the past
        // and the participles in those slots rather than a genitive and a
        // partitive, so the noun order does not describe it. A verb slot is
        // left untagged here and the verb generator, which knows what the slots
        // mean, supplies the reading.
        let tags = match crate::grammar::is_verb_class(inflection.class) {
            true => verb_slot_tags(i),
            false => role
                .nominal()
                .map(|(case, number)| Tags::of_number(case, number))
                .unwrap_or(Tags::UNKNOWN),
        };
        let slot_start = out.len();
        // A class 50/51 compound's forms are not read out of the lemma by
        // aligning it against the class model -- the model is a different word
        // and shares nothing with the compound. They are the parts of the word,
        // each inflected by its own class and joined, so they are pushed once
        // per slot from the split found in the lexicon rather than from the
        // model alignment.
        for head in heads {
            for (head_form, tail_form) in head.forms.get(i).into_iter().flatten() {
                push(
                    out,
                    &format!("{head_form}{tail_form}"),
                    "",
                    tags,
                    Confidence::Plain,
                    pos,
                );
            }
        }
        for variant in slot.variants() {
            let Some(overlap) = split_point(model, variant) else {
                continue;
            };
            // A class whose model word is the lemma itself (`kala` is class 9's
            // model) is inflected from that lemma and nothing else: the stem is
            // the whole word or the shared prefix, and any cut between them is
            // a word the class never licenses. The fine-tuning either side of
            // the prefix belongs to a lemma the class models but does not equal,
            // which is what lets `kaappi` bridge to the `risti` model and
            // `kylmä` to `askel`.
            let direct_model = !crate::grammar::is_verb_class(inflection.class) && lemma == model;
            // The split can sit a little either side of the longest common
            // prefix: on the right when the model grew a stem vowel, on the
            // left when the lemma carries a linking vowel of its own.
            //
            // The right-hand side is only worth trying when the lemma really is
            // the shorter of the two words. Otherwise it slides the start of
            // the ending past the end of the lemma and reads an ending as part
            // of a stem, which is how `talo` came out as a partitive and
            // `talon` as an illative: class 1 is modelled on `valo`, whose
            // illative is `valoon`, and the split at five took the `o` for a
            // stem and left `n` for the ending.
            let mut points = [
                overlap.saturating_sub(2),
                overlap.saturating_sub(1),
                overlap,
                overlap + 1,
            ];
            if variant == model {
                // A variant that is the model word itself has no boundary to
                // move: the model word is the whole lemma for a noun class, and
                // for a verb class it is the first person, whose own `-a` is
                // the infinitive ending and not a case ending. Sliding the
                // split inside it reads that `a` as an ending and the lemma
                // with it, which is where the `hella` of a `hellätä` and the
                // `kello` of a `kellertyä` came from.
                points = [overlap; 4];
            } else if char_len >= model.chars().count() {
                points[3] = overlap;
            } else if overlap >= lemma.len() {
                // The left-hand side is the mirror of that, and it is worth
                // trying only for the same reason: the lemma has to be the word
                // with something extra in it, a linking vowel the model has no
                // room for. When the model's stem is longer than the whole
                // lemma there is nothing to give up, and moving the split left
                // cuts into both at once: the `io` of the `valtio` that models
                // class 3 becomes an ending and the `eliö` is cut to `el` to go
                // with it, which is where the `elo` and the `eloon` of a plain
                // `-io` noun came from.
                points[0] = overlap;
                points[1] = overlap;
            }
            for p in points {
                let Some(ending) = variant.get(p..) else {
                    continue;
                };
                // The ending is the model's, but the vowel in it has to be the
                // lemma's. Class 3 is modelled on the back-harmony `valtio` and
                // holds `eliö` with it, and taking the `a` of `valtiota` for
                // the partitive is what gave the model a noun of its own in
                // `eliöta` and lost `eliötä`.
                let ending = if read_harmony {
                    &crate::grammar::harmonise(ending, lemma_harmony)
                } else {
                    ending
                };
                // An empty ending is the whole lemma again, which says nothing
                // about a case, and a slot is never a case on its own.
                if ending.is_empty() && p > overlap {
                    continue;
                }
                for (nchars, tight) in lemma_splits(overlap, model, char_len) {
                    // The class's own model word is cut at the places the class
                    // spells: the shared prefix (`kal` of `kala` in `kaloja`),
                    // the whole lemma (`kala` in `kalaa`), and once the final
                    // vowel is off for the few readings that drop it (`korke` in
                    // the `korkein` of `korkea`). A cut anywhere else reads a
                    // different word out of it, the `kaiaa` and `kaiain` of a
                    // `kala`, so for the model word itself those are skipped.
                    if direct_model
                        && nchars != model[..overlap].chars().count()
                        && nchars != char_len
                        && nchars != char_len.saturating_sub(1)
                    {
                        continue;
                    }
                    let lsplit = char_offsets[nchars];
                    let stem = &lemma[..lsplit];
                    let tight_fit = p == overlap && tight;
                    // A cut this far into the lemma is not a repair of a
                    // boundary, it is a different word: `dösä` cut to `d` to
                    // take the `ia` of the `koiria` that models its class gives
                    // the `dia` of a wholly different base form, and in the
                    // other direction a word can come out in the other half of
                    // the alphabet, which reads as the harmony breaking. A cut
                    // that keeps half the lemma or less is a guess.
                    let confidence = if tight_fit && modelled_on_lemma {
                        // A gradating stem in a weak-grade slot has its strong
                        // form demoted: `katu` is `kadun` in the genitive, not
                        // the `katun` the ungradated stem gives. The weak form
                        // is produced by the gradation pass below.
                        let gradating_weak_slot = !crate::grammar::is_verb_class(inflection.class)
                            && gradates.is_some()
                            && role.grade().is_weak()
                            && format!("{stem}{ending}") != variant
                            && gradates.is_some_and(|g| g.site(stem).is_some());
                        if gradating_weak_slot {
                            Confidence::Guessed
                        } else if nchars == char_len
                            && role != Slot::Nom
                            && !(reverse && role == Slot::Part)
                            && !model_is_stem
                        {
                            // A whole-lemma cut of a word whose class cuts the
                            // lemma is a slid reading, not a word: `aakkonensen`
                            // from a `aakkonen` whose stem is `aakkose`,
                            // `lämminmin` from `lämmin`. It is kept as a guess but
                            // not offered, for the model word and its members alike.
                            Confidence::Guessed
                        } else if role == Slot::Ill {
                            // The illative singular doubles the stem's final
                            // vowel and adds an `n`, so it is a real word only
                            // when the model's final vowel is the lemma's own.
                            // `aamu` is `aamuun` and not the `aamuon` that the
                            // `valoon` model suggests, because `aamu` ends in
                            // `u` and the model `valo` in `o`; the doubled
                            // vowel is the lemma's.
                            let illative_mismatch =
                                !crate::grammar::is_verb_class(inflection.class)
                                    && lemma.chars().last() != model.chars().last();
                            if illative_mismatch {
                                Confidence::Guessed
                            } else {
                                Confidence::Plain
                            }
                        } else if direct_model
                            && nchars == char_len
                            && format!("{stem}{ending}") != variant
                        {
                            // A whole-lemma cut that does not reproduce the slot's
                            // tabulated form is a slid reading, not a word:
                            // `lämminmin` from `lämmin`, `nainensen` from
                            // `nainen`. It is kept as a guess but not offered.
                            Confidence::Guessed
                        } else {
                            Confidence::Plain
                        }
                    } else if crate::grammar::is_verb_class(inflection.class) {
                        // A verb's own paradigm comes from the verb generator,
                        // not from this alignment. The one thing the slot loop
                        // knows that the generator does not is the exact
                        // tabulated form, so a slot-aligned form is Plain only
                        // when it reproduces the catalogue's own variant for
                        // the slot; anything else is a guess. `tulkoon` is the
                        // tabulated imperative of class 67 and is Plain, while
                        // a `muistaaettiin` that no slot spells out is not.
                        let word = format!("{stem}{ending}");
                        if word == variant {
                            Confidence::Plain
                        } else {
                            Confidence::Guessed
                        }
                    } else if nchars * 2 <= char_len || tight_fit {
                        Confidence::Guessed
                    } else {
                        Confidence::Repaired
                    };
                    // An alternant the catalogue wraps in parentheses is a rare, lexicalised
                    // reading, a word but not the class's own: `kalain` beside
                    // `kalojen`, `korkeain` beside `korkeiden`. It is still
                    // generated, so it is known, but it is not offered as the
                    // class's form.
                    let confidence = if confidence == Confidence::Plain && slot.is_optional(variant)
                    {
                        Confidence::Guessed
                    } else {
                        confidence
                    };
                    // A reverse-gradation word is strong in every form but the
                    // nominative and the partitive: `hammas` is `hampaan` and
                    // `hampaat`, but `hammas` itself and `hammasta` keep the
                    // weak stem. So the stem is strongened here, outside those
                    // two slots, and because the word's reverse gradation is a
                    // settled fact about it, every strong form is offered as
                    // the plain paradigm rather than left a guess.
                    let push_stem = if reverse && role != Slot::Nom && role != Slot::Part {
                        if let Some(g) = inflection.gradation {
                            g.strongen(stem)
                        } else {
                            stem.to_string()
                        }
                    } else {
                        stem.to_string()
                    };
                    let confidence = if reverse && role != Slot::Nom && role != Slot::Part {
                        Confidence::Plain
                    } else {
                        confidence
                    };
                    push(out, &push_stem, ending, tags, confidence, pos);
                    // When the model's illative vowel differs from the lemma's,
                    // the correct form is the stem with its own final vowel
                    // doubled and an `n`, and that is what is offered rather
                    // than the `aamuon` the model suggested.
                    let illative_mismatch = role == Slot::Ill
                        && !crate::grammar::is_verb_class(inflection.class)
                        && lemma.chars().last() != model.chars().last();
                    if illative_mismatch && modelled_on_lemma {
                        // The correct illative is the lemma with its own final vowel doubled:
                        // `aamu` + `u` + `n` = `aamuun`, `eliö` + `ö` + `n` =
                        // `eliöön`. This only holds for a lemma that ends in a
                        // vowel — a consonant-final lemma (`aakkoset`) has no
                        // doubled vowel to build, and its illative comes from
                        // the stem (`aakkoseen`) rather than a doubled
                        // consonant. The model alignment may cut the stem short,
                        // so the lemma itself is used, and the correct form is
                        // generated however the model happened to reach the slot.
                        if let Some(v) = lemma.chars().last()
                            && is_vowel(v)
                        {
                            let correct = format!("{lemma}{v}n");
                            if correct != format!("{stem}{ending}") {
                                push(out, lemma, &format!("{v}n"), tags, Confidence::Plain, pos);
                                add_possessives_for(
                                    out,
                                    &format!("{lemma}{v}n"),
                                    tags,
                                    lemma,
                                    Confidence::Plain,
                                );
                            }
                        }
                    }
                    if tight_fit && let Some(spelled) = out.last() {
                        // Only the plain spellings are the catalogue's
                        // own; a demoted optional variant (now guessed) is
                        // a word but not one the retention may drop.
                        if spelled.plain() {
                            spelled_out.push(spelled.word.clone());
                        }
                    }

                    // A tabulated alternation swaps the last such consonant of
                    // the stem, which is where the case endings go: `laps` ->
                    // `last` gives `lasta`, the partitive, while the genitive
                    // `lapsen` keeps the `p` because it never got here.
                    if let Some((from, to)) = table_alt
                        && let Some(at) = find_last(stem, from)
                    {
                        let mut alt_stem = stem.to_string();
                        alt_stem.replace_range(at..at + from.len(), to);
                        push(
                            out,
                            &alt_stem,
                            ending,
                            tags,
                            alt_confidence(lemma, &alt_stem),
                            pos,
                        );
                    }
                    // An alternation read off the class's own genitive is
                    // positional, and stays positional: `kak` -> `kah` is what
                    // turns the `den` of `kahden` into a real word.
                    if let Some((at, from, to)) = auto_alt
                        && let Some(alt) = stem.get(at..at + from.len_utf8())
                        && alt == from.to_string()
                    {
                        let mut alt_stem = stem.to_string();
                        alt_stem.replace_range(at..at + from.len_utf8(), &to.to_string());
                        push(
                            out,
                            &alt_stem,
                            ending,
                            tags,
                            alt_confidence(lemma, &alt_stem),
                            pos,
                        );
                    }

                    // `-j-`/`-i-` epenthesis after a final stem vowel:
                    // talo + en -> talo-j-en = talojen.
                    if is_vowel_byte(stem.as_bytes().last().copied())
                        && is_vowel_byte(ending.as_bytes().first().copied())
                    {
                        for insert in ['j', 'i'] {
                            push(
                                out,
                                stem,
                                &format!("{insert}{ending}"),
                                tags,
                                Confidence::Repaired,
                                pos,
                            );
                        }
                    }

                    // Consonant gradation, e.g. kaappi -> kaapin. The grade a slot wants
                    // is the slot's own: the genitive is weak (`katu` is
                    // `kadun`) and the illative is strong (`katuun`). The
                    // weak grade in a weak-grade slot is a plain attachment
                    // and is offered; the strong grade in a strong slot is
                    // the stem as it stands, which the alignment above
                    // already made Plain. A class that fixes its stem is left
                    // out, since there the flag is the lemma's and the stem is
                    // the class's own spelling (see `gradates` above).
                    if let Some(grad) = gradates.filter(|_| {
                        !crate::grammar::is_verb_class(inflection.class)
                            && role.grade().is_weak()
                            && tight_fit
                    }) {
                        let alt = grad.apply(stem, crate::kotus::Grade::Weak);
                        if alt != stem {
                            push(out, &alt, ending, tags, Confidence::Plain, pos);
                        }
                    }
                }
            }
        }

        // Possessive forms. Every slot of the catalogue takes one but the
        // plural nominative does not: the Finnish possessive hangs off the
        // nominative or the genitive singular, and the two spellings coincide
        // there, so `talot` has no possessive of its own and `talotani` is not
        // a word. The other seven each take the suffix on their own spelling.
        // Four of them — the genitive and the illative, singular and plural —
        // end in an `n` and drop it before the suffix, so `talon` is `taloni`,
        // `taloon` is `talooni`, `talojen` is `talojeni` and `taloihin` is
        // `taloihini`. The stem is untouched in every one of these; only a case
        // ending goes, which is an ordinary attachment and not the repair of a
        // lemma that `Repaired` stands for.
        //
        // A verb has no possessive paradigm, and its first two slots are the
        // infinitive and the first person present rather than a nominative and
        // a genitive, so attaching possessives to them would read `muistamme`
        // as the possessive of the first person and collide with the real
        // first person plural.
        if !crate::grammar::is_verb_class(inflection.class)
            && matches!(
                role,
                Slot::Nom
                    | Slot::Gen
                    | Slot::Part
                    | Slot::Ill
                    | Slot::PlGen
                    | Slot::PlPart
                    | Slot::PlIll
            )
        {
            let own: Vec<String> = out[slot_start..]
                .iter()
                .filter(|g| g.plain())
                .map(|g| g.word.clone())
                .collect();
            // The possessive attaches to the catalogue's own spellings for the
            // slot, not to every cut that reached them: a slid alignment can
            // land on the same word from a wrong stem (`anana` + `sen` is
            // `ananasen` from the genitive, `anana-` + `sta` is `ananasta`
            // from the partitive), and hanging possessives off those stems is
            // what spreads the wrong stem into `ananasella`-style forms. The
            // first spelling is the tightest fit, which is the one the
            // catalogue's own boundary gives.
            let own: Vec<String> = own.into_iter().take(1).collect();
            for form in &own {
                let front = crate::grammar::harmony(lemma) == crate::grammar::Harmony::Front;
                for poss in Possessive::ALL {
                    let suffix = poss.suffix(front);
                    let possessed = tags.possessed_by(poss);
                    // The partitive takes the suffix as it stands, with nothing
                    // dropped and nothing moved: `lutkutusta` is `lutkutustani`
                    // and `taloa` is `taloani`, both plain attachments like the
                    // nominative's. The third person keeps the two shapes it has
                    // everywhere else, and Finnish spells it both ways — the
                    // bare `-nsa` and a doubling of the last vowel — so `taloa`
                    // is `taloansa` and `taloaan`, and `taloja` is `talojansa`
                    // and `talojaan`. Which one a writer meant is not a question
                    // with an answer, so both are offered.
                    if matches!(role, Slot::Part | Slot::PlPart) {
                        push(out, form, suffix, possessed, Confidence::Plain, pos);
                        if poss == Possessive::Third && form.chars().last().is_some_and(is_vowel) {
                            let v = form.chars().last().unwrap();
                            push(
                                out,
                                form,
                                &format!("{v}n"),
                                possessed,
                                Confidence::Plain,
                                pos,
                            );
                        }
                        continue;
                    }
                    // A noun whose genitive stem is not its lemma attaches the
                    // possessive to that strong genitive stem, not to the
                    // nominative or the weak genitive: `joki` is `jokeni`,
                    // `jokesi`, `jokensa`, and `ovi` is `oveni`, not the
                    // `jokini` and `ovini` that the nominative `joki` and `ovi`
                    // would give. This is the singular's business alone: the
                    // plural genitive has a stem of its own to build on
                    // (`jokien` is `jokieni`, not the `jokeni` the singular
                    // would answer).
                    let strong = match role {
                        Slot::Nom | Slot::Gen | Slot::Ill => {
                            possessive_stem(lemma, inflection.class, form)
                        }
                        _ => None,
                    };
                    if let Some(stem) = &strong {
                        push(out, stem, suffix, possessed, Confidence::Plain, pos);
                    } else if matches!(role, Slot::Gen | Slot::Ill | Slot::PlGen | Slot::PlIll)
                        && form.ends_with('n')
                    {
                        // The genitive and the illative end in `n` and drop it
                        // before the suffix: `talon` is `taloni`, `taloon` is
                        // `talooni`, `talojen` is `talojeni` and `taloihin` is
                        // `taloihini`. The stem is untouched and only the case
                        // ending goes, which is an ordinary attachment and not
                        // the repair of a lemma that `Repaired` stands for.
                        push(
                            out,
                            form.trim_end_matches('n'),
                            suffix,
                            possessed,
                            Confidence::Plain,
                            pos,
                        );
                    } else {
                        // The suffix goes on the word as it stands, so nothing was
                        // changed and the reading is a plain one. `taloni` from
                        // `talo` and `taloillani` from `taloilla` are both of this
                        // kind, and both are words a user could want to be
                        // offered.
                        push(out, form, suffix, possessed, Confidence::Plain, pos);
                    }
                }
            }
        }
    }
    // An alignment that slid is not a second reading of a word that the
    // catalogue already spells out. The split points either side of the common
    // prefix exist to catch a lemma with a linking vowel, and one of them will
    // sometimes land on a word that another slot spells exactly: `tal` with the
    // `on` of the illative is `talon`, which the genitive already has. The
    // shifted form is dropped and the plain one stands, which is also why the
    // reading a verb generator gives `lasken` survives the catalogue slot that
    // reached the same word knowing nothing about it.
    if !spelled_out.is_empty() && !crate::grammar::is_verb_class(inflection.class) {
        out.retain(|f| f.plain() || !spelled_out.contains(&f.word));
    }

    // The plain words the model-word alignment read off the lemma. For a class
    // that fixes its stem, or a reverse-gradation word whose nominative
    // already carries the weak grade (`hammas`), the oblique owns the whole
    // paradigm, so any of these the oblique does not also produce is a slid
    // reading and is demoted after the oblique runs.
    let alignment_plain: std::collections::HashSet<String> =
        if crate::grammar::fixed_stem(inflection.class).is_some() || reverse {
            out[start..]
                .iter()
                .filter(|f| f.plain())
                .map(|f| f.word.clone())
                .collect()
        } else {
            std::collections::HashSet::new()
        };

    // The catalogue stops at eight forms per class, so the remaining noun
    // cases and the rest of the verb paradigm come from `grammar`.
    if crate::grammar::is_verb_class(inflection.class) {
        let imperative3 = para.slots.get(5).and_then(|s| variant_of(para, s, lemma));
        // The imperative plural forms are built from the third person singular
        // imperative, which the catalogue gives: `sanokoon` gives `sanokaa`
        // (2nd person plural), `sanokaamme` (1st person plural) and `sanokoot`
        // (3rd person plural). The `-koon`/`-köön` of the singular turns into
        // the `-kaa`/`-kää` of the second plural, and the `-oot`/`-ööt` of the
        // third plural.
        if let Some(imp3) = &imperative3 {
            let (koon, kaa, koot) = if imp3.ends_with("koon") {
                ("koon", "kaa", "koot")
            } else if imp3.ends_with("köön") {
                ("köön", "kää", "kööt")
            } else {
                ("", "", "")
            };
            let stem = imp3.strip_suffix(koon).unwrap_or(imp3);
            if !stem.is_empty() {
                for (ending, person, number) in [
                    (kaa, Person::Second, Number::Plural),
                    (&format!("{kaa}mme"), Person::First, Number::Plural),
                    (koot, Person::Third, Number::Plural),
                ] {
                    push(
                        out,
                        stem,
                        ending,
                        Tags::finite(Tense::Present, person, number).mood(Mood::Imperative),
                        Confidence::Plain,
                        PartOfSpeech::Verb,
                    );
                }
            }
        }
        let p7 = para.slots.get(7).and_then(|s| variant_of(para, s, lemma));
        let passive = p7
            .as_deref()
            .and_then(|s| crate::grammar::passive_of(lemma, s));
        // The present stem and the past stem are read off the infinitive by
        // the class's type and its model verb's projected past, rather than
        // from the two forms the catalogue tabulates for the model word, so
        // a verb whose own forms the catalogue does not carry is declined
        // the same way.
        for form in crate::grammar::verb_from_infinitive(
            lemma,
            inflection.class,
            inflection.gradation,
            passive,
        ) {
            // A verb class is a verb whatever the sanalista says, so the
            // dictionary's word for it is the one that goes wrong.
            //
            // The two forms these are built from come from the lemma's own
            // stem, read off its infinitive, and not from a cut of the
            // model word. For `hellätä` they are `hellään` and `helläsi`,
            // and the generator expands that stem into the real paradigm.
            // Only the infinitive itself and the slot-aligned readings the
            // class spells out are Plain from the slot loop; everything
            // built from the stem is a reading, and the verb generator's
            // own Plain forms are offered as suggestions.
            out.push(form.of_pos(PartOfSpeech::Verb));
        }
    } else {
        // The plural stems, for the plural of the genitive-stem cases. They
        // are the plural partitive with its theme vowel taken off: `taloja`
        // gives `talo`, `kaloja` gives `kalo` and `jokia` gives `jok`, and
        // those are the stems the plural adessive, ablative and the rest are
        // built on. The plural `i` is hung off them, so a stem that keeps the
        // strong grade feeds the cases that keep it too — `jok` is `jokina`
        // and `jokineen`, `laatikko` is `laatikkoina` and `laatikkoineen` —
        // while the local cases and the instructive gradate it: `jok` is `jo`,
        // so `jokia` without its ending is the `jo` of `joissa`, `joilla` and
        // `join`, and `laatikko` is the `laatiko` of `laatikoissa`,
        // `laatikoilla` and `laatikoin`.
        let mut oblique_forms: Vec<Generated> = Vec::new();
        for (plural_stem, plural_strong) in
            plural_stems(para, lemma, inflection.class, inflection.gradation)
        {
            // A reverse-gradation word keeps the strong grade in its plural too:
            // `hammas` is `hampaat`, `hampain`, `hampaissa`, not the weak `hammai`.
            let (plural_stem, plural_strong) =
                if let Some(g) = inflection.gradation.filter(|_| reverse) {
                    (g.strongen(&plural_stem), g.strongen(&plural_strong))
                } else {
                    (plural_stem, plural_strong)
                };
            for form in crate::grammar::oblique(
                lemma,
                inflection.class,
                inflection.gradation,
                &plural_stem,
                &plural_strong,
                lemma == para.model(),
            ) {
                // Here the sanalista is the only thing that knows, since a noun
                // and an adjective inflect alike and differ in every other word.
                let form = form.of_pos(pos);
                out.push(form.clone());
                oblique_forms.push(form);
            }
        }
        // A noun takes a possessive in every case, not just the nominative and
        // the genitive: `aamulla` is `aamullani` in the adessive and `aamuun`
        // is `aamuuni` in the illative. The oblique cases are built in
        // `grammar`, so their possessives are added here where the noun is
        // being assembled.
        add_possessives(out, &oblique_forms, lemma);
        // For a class that fixes its stem, the oblique produced the word's real
        // paradigm; an alignment reading of the lemma that the oblique does not
        // produce as a sure word is a slid reading (`onnetoia` from `onneton`,
        // `tuhasia` from `tuhat`, `sisimmänä` beside `sisimpänä`), so it is
        // kept as a guess but not offered.
        let oblique_plain: std::collections::HashSet<String> = oblique_forms
            .iter()
            .filter(|o| o.confidence == Confidence::Plain)
            .map(|o| o.word.clone())
            .collect();
        for f in out[start..].iter_mut() {
            // A reverse-gradation word owns its lexical cases (the nominative,
            // genitive, partitive and illative, and their plurals) from the
            // alignment, since the oblique never builds them — `hampaan` and
            // `hampaat` come from the strongened stem, not from a local case.
            // Those are the real paradigm and are kept. The reverse word's
            // slid local readings are still demoted.
            let reverse_lexical = reverse
                && f.readings().iter().any(|r| {
                    r.case
                        .zip(r.number)
                        .is_some_and(|(c, n)| LEXICAL.contains(&(c, n)))
                });
            if f.confidence == Confidence::Plain
                && alignment_plain.contains(&f.word)
                && !catalogue_words.contains(&f.word)
                && !oblique_plain.contains(&f.word)
                && !reverse_lexical
            {
                f.confidence = Confidence::Guessed;
            }
        }
    }
}

/// The possessive forms of every oblique case of a noun.
///
/// Two of the cases are skipped. The instructive and the comitative are not
/// cases a possessor's word can be in: `taloin` is a way of being and
/// `taloineen` is a company, and there is no Finnish in which either belongs to
/// anyone. `taloini` and `taloineeni` are not words, and offering them is worse
/// than offering nothing, because a suggestion a writer can never have meant is
/// a correction that wastes their time.
///
/// For the rest, see [`add_possessives_for`], which builds the suffixes.
fn add_possessives(out: &mut Vec<Generated>, forms: &[Generated], lemma: &str) {
    for form in forms {
        // A possessive is only offered for a form the oblique is sure of. The
        // fallback stem of a class (`nainen`'s bare lemma, which gives
        // `nainena` for the partitive) is a guess, and hanging possessives off
        // it would spread the wrong stem (`nainenaani`) — the catalogue's own
        // spellings carry the possessives.
        if form.confidence != Confidence::Plain {
            continue;
        }
        if matches!(
            form.union().case,
            Some(Case::Instructive | Case::Comitative)
        ) {
            continue;
        }
        add_possessives_for(out, &form.word, form.union(), lemma, form.confidence);
    }
}

/// The possessive forms of one noun case.
///
/// The suffix attaches to the form as it stands, with two exceptions. The
/// genitive and the illative end in an `n` and drop it before the suffix, so
/// `aamun` is `aamuni` and `aamuun` is `aamuuni`. The translative spells its
/// elided `i` out as an `e` instead, which is the one ending a possessive
/// suffix attaches to directly: `taloksi` is `talokseni` and not the
/// `taloksni` the elision would leave.
///
/// The third person is spelled two ways and both are offered: a vowel-final
/// form lengthens its last vowel (`aamulla` is `aamullaan`, and the translative
/// `taloikseen`) and every form takes the bare `-nsa`/`-nsä` (`aamullansa`,
/// `taloiksensa`, and `aamuunsa` where the `n` had to come off first).
fn add_possessives_for(
    out: &mut Vec<Generated>,
    word: &str,
    tags: Tags,
    lemma: &str,
    confidence: Confidence,
) {
    let front = crate::grammar::harmony(lemma) == crate::grammar::Harmony::Front;
    // The translative keeps its ending, and only its ending changes shape: the
    // `ksi` becomes `kse`, which is the same word with the `en` spelled out and
    // The translative keeps its ending, and only its ending changes shape: the
    // elided `i` is written out as an `e`, which is the same word with the `en`
    // in it, and every rule below then applies to the result unchanged. So
    // `taloksi` is `talokse` here and answers `talokseni`, and `taloiksi` is
    // `taloikse` and answers `taloikseni`.
    let word = match word.strip_suffix('i') {
        Some(head) if tags.case == Some(Case::Translative) => &format!("{head}e"),
        _ => word,
    };
    // The genitive and the illative drop their final `n` before any suffix.
    let stem = word.strip_suffix('n').unwrap_or(word);
    for poss in Possessive::ALL {
        let suffix = poss.suffix(front);
        let possessed = tags.possessed_by(poss);
        let pos = tags.part_of_speech.unwrap_or(PartOfSpeech::Unknown);
        if poss == Possessive::Third
            && let Some(v) = word.chars().last()
            && is_vowel(v)
        {
            // A vowel-final form lengthens its last vowel and adds an `n`:
            // `aamulla` -> `aamullaan` and `taloilla` -> `taloillaan`. Which of
            // this and the `-nsa` below a writer meant is not a question with
            // an answer, so both are offered.
            push(out, &format!("{stem}{v}"), "n", possessed, confidence, pos);
        }
        // The possessive inherits the confidence of the case it hangs off: a
        // possessive of a tabulated form is a plain word, and a possessive of
        // an oblique guess is itself a guess, so a spurious `näpilla` does not
        // get a suggestible `näpillani`.
        push(out, stem, suffix, possessed, confidence, pos);
    }
}

/// Where in `lemma` the ending of a catalogue form attaches.
///
/// The catalogue form splits into the model's own stem and the ending, and the
/// lemma has to be cut at the matching place. When the lemma is longer than the
/// model, which is the usual case, the cut moves right by the difference:
/// `kaappi` is cut after `kaap` where `risti` is cut after `ris`. When the
/// lemma is the model itself, or shorter, the cut is simply the model's, which
/// is the case the length difference has to get right: `saada` cuts after `sa`
/// to make `saisi`, three characters from the end, where a split that only ever
/// looks at the end of the word never reaches.
///
/// The two or three characters either side of the cut are tried as well, since
/// the exact place moves when the stem gained or lost a vowel. Each candidate
/// is marked as tight or not, and the tight one is what `plain` means.
///
/// `overlap` is a byte offset into the model; the cut is counted in characters.
fn lemma_splits(overlap: usize, model: &str, char_len: usize) -> Vec<(usize, bool)> {
    let cut = model[..overlap].chars().count();
    let model_len = model.chars().count();
    // The cut, shifted by however much longer the lemma is than the model's
    // stem. The shift is measured against the model word rather than the
    // catalogue form, since it is the model word the overlap is an offset into:
    // a class-3 inessive is measured against `valtio` and not against the seven
    // characters of `valtiossa`, and counting the ending in is what cut `eliö`
    // to `el` to make an `elita` of it. A lemma longer than the model moves the
    // cut right by the difference (`kämmen` is six letters to `sisar`'s five,
    // so its stem is the full `kämmen` and not the `kämme` that matches the
    // model), and a shorter one moves it left.
    let shifted = (cut + char_len).saturating_sub(model_len);
    let mut out: Vec<(usize, bool)> = Vec::with_capacity(8);
    for (n, tight) in [
        (shifted, true),
        (shifted.saturating_sub(1), false),
        (shifted.saturating_sub(2), false),
        (shifted + 1, false),
        (char_len, true),
        (char_len.saturating_sub(1), false),
        (char_len.saturating_sub(2), false),
    ] {
        if n > 0 && n <= char_len && !out.iter().any(|(m, _)| *m == n) {
            out.push((n, tight));
        }
    }
    out
}

/// The spellings of a lemma that a class is inflected from.
pub(super) fn spellings<'a>(lemma: &'a str, fixed: Option<&'static str>) -> Vec<&'a str> {
    // A fixed-stem class is inflected from both spellings: the lemma for the
    // nominative, which the class model already is, and the fixed stem for
    // everything that hangs off the stem.
    match fixed {
        Some(f) => vec![lemma, f],
        None => vec![lemma],
    }
}

/// The grammatical tags of a verb-paradigm slot, by position.
///
/// The noun order does not describe a verb, whose slots are the infinitive, the
/// present, the past, the conditional, the potential, the imperative, the
/// participle and the passive past. These are what the catalogue means by each
/// position, so a form of a verb slot can carry its grammar rather than being
/// left unknown.
fn verb_slot_tags(i: usize) -> Tags {
    match i {
        0 => Tags::UNKNOWN, // the infinitive, which has no person, mood or case
        1 => Tags::finite(Tense::Present, Person::First, Number::Singular),
        2 => Tags::finite(Tense::Past, Person::Third, Number::Singular),
        3 => Tags::finite(Tense::Present, Person::Third, Number::Singular).mood(Mood::Conditional),
        4 => Tags::finite(Tense::Present, Person::Third, Number::Singular).mood(Mood::Potential),
        5 => Tags::finite(Tense::Present, Person::Third, Number::Singular).mood(Mood::Imperative),
        6 => Tags {
            tense: Some(Tense::Present),
            mood: Some(Mood::Instructive),
            ..Tags::UNKNOWN
        },
        7 => Tags {
            tense: Some(Tense::Past),
            mood: Some(Mood::Indicative),
            voice: Some(Voice::Passive),
            case: Some(Case::Illative),
            ..Tags::UNKNOWN
        },
        _ => Tags::UNKNOWN,
    }
}

/// The catalogue form of `slot` as realised for `lemma`, i.e. the model form
/// with the model word swapped for the lemma.
///
/// The split is against the class model, not against the form itself: `huusi`
/// is the past of the model `huutaa`, and the two share the `hu` and differ
/// from there, so the real form for a `huutaa` lemma is `hu` + `usi` again.
/// Splitting the form against itself would say the whole form is model and
/// hand back the lemma unchanged, which is how `huusi` went missing.
/// The plural stem, read out of the catalogue's plural partitive.
///
/// The plural stem is the plural partitive with its marker off: `taloja` gives
/// `talo` and `kaloja` gives `kalo`, `miehiä` gives `mieh` and `ristejä` gives
/// `riste`. The marker is read by [`marker_len`].
pub(super) fn plural_stem_of(pl: &str, para: &Paradigm) -> String {
    let chars: Vec<char> = pl.chars().collect();
    chars[..chars.len().saturating_sub(marker_len(para))]
        .iter()
        .collect()
}

/// The plural stem a class's own stem rule gives a lemma.
///
/// The plural is a stem the class shares rather than a cut of the model word:
/// `nainen` is `naise` in the singular and `nais` in the plural, and the edit
/// between the two is what the class's members make to their own stems, so
/// `aakkonen` — whose class stem is `aakkose` — is `aakkos`, and a
/// `sotilas`, whose class stem is `sotilaa`, is `sotila`. The edit is read
/// once, from the model word's own stem and its plural partitive, and the
/// lemma's stem is what it is applied to.
///
/// Reading the plural stem out of the model word's plural partitive instead
/// and cutting the lemma to the length the two share is what pluralised
/// this class on a shortened stem: `nainen` and `naisia` share only `nai`,
/// and the lemma was cut there.
///
/// `None` when the class makes no edit (`valo` is `valo` in both numbers) or
/// when the edit does not fit the lemma's stem, and the catalogue's own
/// reading is then the only one there is.
fn class_plural_stem(class: ClassId, para: &Paradigm, lemma: &str) -> Option<String> {
    let model = para.model();
    let model_stem = crate::grammar::class_stem(class, model, true)?;
    let model_plural = plural_stem_of(para.slots.get(6)?.model.as_str(), para);
    // The edit the class makes in the plural: the tail of the model's stem
    // that the plural partitive drops or changes, and what it puts there
    // instead. `naise` -> `nais` drops an `e`, `tuhann` -> `tuhans` changes
    // the `n`, and `kalleude` -> `kalleuks` replaces a `de`.
    let common = crate::spec::lcp_len(&model_stem, &model_plural)
        .min(model_stem.len())
        .min(model_plural.len());
    if common == 0 {
        return None;
    }
    let (from, to) = (&model_stem[common..], &model_plural[common..]);
    if from.is_empty() && to.is_empty() {
        return None;
    }
    let stem = crate::grammar::class_stem(class, lemma, lemma == model)?;
    let edited = stem.strip_suffix(from)?;
    Some(format!("{edited}{to}"))
}

/// The weak and strong plural stems of a noun, read out of its class.
///
/// The strong stem is the plural partitive with its marker off, and the weak is
/// that stem under the class's gradation. Two classes do not read their plural
/// that way:
///
/// * A class that fixes its stem is weak in the plural on the fixed stem with
///   its theme vowel off (`sisimmä` is `sisimm`, `vasemma` is `vasemm`), since
///   the plural partitive of those is the strong grade (`sisimpiä` is
///   `sisimp`) that the local plural cases do not take.
/// * The `-a` classes whose plural changes the theme to `o` (`kala` is `kalo`,
///   `omena` is `omeno`, `solakka` is `solako`) are read off the singular stem,
///   because the plural partitive can carry a different reading (`omenia`
///   gives `omeni`).
///
/// The class's own stem rule is a third answer and the one the class's members
/// share, so it is offered beside the catalogue's reading; see
/// [`class_plural_stem`].
pub(super) fn plural_stems(
    para: &Paradigm,
    lemma: &str,
    class: ClassId,
    gradation: Option<crate::kotus::Gradation>,
) -> Vec<(String, String)> {
    let Some(pl) = para.slots.get(6).and_then(|s| variant_of(para, s, lemma)) else {
        return vec![(String::new(), String::new())];
    };
    let strong = plural_stem_of(&pl, para);
    let fixed = crate::grammar::fixed_stem(class);
    // A class that fixes its stem has nothing to graduate, so the stem it
    // names is the grade its endings want and there is no weak to find.
    let gradates = gradation.filter(|_| fixed.is_none());
    let weak_of = |stem: &str| -> String {
        gradates
            .map(|g| g.apply(stem, crate::kotus::Grade::Weak))
            .unwrap_or_else(|| stem.to_string())
    };
    let mut out: Vec<(String, String)> = Vec::new();
    // The `-a` classes whose plural changes the theme to `o`: `kala` is `kalo`,
    // `omena` is `omeno`, `solakka` is `solako`/`solakko`. The plural partitive
    // of these can carry a different reading (`omenia` gives `omeni`), so the
    // plural is read off the singular stem in both grades, and a catalogue stem
    // that differs (`omeni`) is offered too.
    if matches!(class, 9 | 11 | 12 | 13 | 14) && lemma.ends_with('a') {
        let a_to_o = |stem: &str| {
            let cut = stem
                .char_indices()
                .nth(stem.chars().count() - 1)
                .map_or(stem.len(), |(i, _)| i);
            format!("{}o", &stem[..cut])
        };
        let weak_singular = gradates
            .map(|g| g.apply(lemma, crate::kotus::Grade::Weak))
            .unwrap_or_else(|| lemma.to_string());
        let weak = a_to_o(&weak_singular);
        let strong = a_to_o(lemma);
        out.push((weak, strong.clone()));
        // A catalogue plural partitive that spells a different stem than the
        // `-o` one (`omenia` gives `omeni` beside `omeno`) is a second plural.
        let catalogue = plural_stem_of(&pl, para);
        if catalogue != strong && !catalogue.is_empty() {
            out.push((catalogue.clone(), catalogue));
        }
    } else if let Some(fixed) = fixed
        // A class that fixes its stem is weak in the plural on the fixed stem
        // with its theme vowel off; the strong stem stays the plural
        // partitive's.
        && let Some(last) = fixed.chars().next_back()
        && fixed.chars().count() >= 2
        && is_vowel(last)
    {
        let cut = fixed
            .char_indices()
            .nth(fixed.chars().count() - 1)
            .map_or(fixed.len(), |(i, _)| i);
        out.push((fixed[..cut].to_string(), strong));
    } else {
        out.push((weak_of(&strong), strong.clone()));
        // `askel` has a second plural on its longer stem `askele`
        // (`askeleiksi`, `askeleilla`) beside the catalogue's `askel`
        // (`askeliksi`).
        if class == 49 && strong == "askel" {
            out.push(("askele".to_string(), "askele".to_string()));
        }
    }
    // The plural stem the class's own stem rule gives this word, which is the
    // one a member whose word is longer than the model's has as its own. It is
    // the same stem the catalogue reads for the class's model word, so a word
    // the other branches already reach adds nothing here.
    if let Some(own) = class_plural_stem(class, para, lemma) {
        let pair = (weak_of(&own), own);
        if !out.contains(&pair) {
            out.push(pair);
        }
    }
    out
}

/// How many characters the plural partitive marker of a class is.
///
/// The plural stem is the plural partitive with its marker off, and the marker
/// is normally the last two characters, `ja`/`jä`/`ia`/`iä`, which is all a
/// class like the first needs. A class that has a longer marker is read out of
/// the class's own model: class 3 is modelled on `valtio`, whose plural
/// partitive is `valtioita` over the nominative `valtio`, so the marker is the
/// last three characters. Reading two off `eliöitä` left the stem of `eliö` as
/// `eliöi`, and the plural oblique cases were then built on `eliöii` rather
/// than `eliöi`, so none of the `eliöillä`, `eliöissä` and `eliöin` the word
/// has was generated at all.
///
/// A class whose marker works out shorter than that is read the two-character
/// way, because the model cannot show it. Classes 7, 27, 28 and 29 are
/// modelled on `ovi`, `käsi`, `kynsi` and `lapsi`, and the `i` of `ovia`,
/// `käsiä`, `kynsiä` and `lapsia` is the theme vowel of the plural rather
/// than part of the marker: the real stems are `käs` and `kyns`, and the
/// plural inessive of `käsi` and `kynsi` is `käsissä` and `kynsissä`, not
/// `käsiissa` and `kynsiissa`.
fn marker_len(para: &Paradigm) -> usize {
    let Some(plural) = para.slots.get(6) else {
        return 2;
    };
    let pl = &plural.model;
    // A class whose stem drops its theme vowel in the plural takes the longer
    // `-ita`/`-itä` marker: `korkeita` is `korke`+`ita` and `valtioita` is
    // `valtio`+`ita`, not a marker of two on a stem already ending in `i`.
    if pl.ends_with("ita") || pl.ends_with("itä") {
        return 3;
    }
    // The plain markers are `-ja`/`-jä`/`-ia`/`-iä`: `taloja`, `ristejä`,
    // `miehiä`, `vastauksia`.
    if pl.ends_with("ja") || pl.ends_with("jä") || pl.ends_with("ia") || pl.ends_with("iä") {
        return 2;
    }
    // Otherwise the marker is as long as the plural partitive grew past the
    // model word.
    let Some(lemma) = para.slots.first() else {
        return 2;
    };
    match pl.chars().count() - lemma.model.chars().count() {
        2..=3 => pl.chars().count() - lemma.model.chars().count(),
        _ => 2,
    }
}

pub(super) fn variant_of(para: &Paradigm, slot: &crate::spec::Slot, lemma: &str) -> Option<String> {
    let form = &slot.model;
    if crate::grammar::is_verb_class(para.class) {
        // A verb class is modelled on one particular verb, so the lemma is
        // almost always a different word from the model and its stem is not the
        // model's stem at some other position. Cutting the lemma where the
        // model's stem ends is reading one word out of another: `lcp("salata",
        // "salaan")` is `sala`, and overlaying that on `hellätä` gives `hell`
        // for a stem that is `hellä`. The lemma's own stem is its infinitive
        // without the infinitive ending, and the infinitive ending is the
        // model's infinitive without the model's stem. The slot's ending, which
        // is the model's slot form without the model's stem, attaches to the
        // lemma's stem instead, harmonised to it.
        let model_infinitive = para.model();
        let model_stem_bytes = crate::spec::lcp_len(model_infinitive, form)
            .min(model_infinitive.len())
            .min(form.len());
        let model_stem: Vec<char> = model_infinitive[..model_stem_bytes].chars().collect();
        let infinitive_chars: Vec<char> = model_infinitive.chars().collect();
        let infinitive_end = infinitive_chars.len() - model_stem.len();
        if infinitive_end == 0 {
            return None;
        }
        let lemma_chars: Vec<char> = lemma.chars().collect();
        if infinitive_end > lemma_chars.len() {
            return None;
        }
        let lemma_stem: String = lemma_chars[..lemma_chars.len() - infinitive_end]
            .iter()
            .collect();
        let ending = &form[model_stem_bytes..];
        let harmony = crate::grammar::harmony(lemma);
        let ending = crate::grammar::harmonise(ending, harmony);
        Some(format!("{lemma_stem}{ending}"))
    } else {
        // How much of the model's stem the model's own slot form repeats. That
        // is the part of the form the lemma replaces, and for the verb classes
        // above it is read off the infinitive, since their stems are all
        // different lengths. Here it is the model's stem: `valtio` is the model
        // of its class and `valtioita` repeats all of it, so `ita` is the
        // ending and `valo`/`valoja` gives `ja`.
        let model_stem = crate::spec::lcp_len(para.model(), form)
            .min(para.model().len())
            .min(form.len());
        if model_stem == 0 && !form.is_empty() {
            return None;
        }
        let tail = form.get(model_stem..)?;
        // A lemma no longer than the model's stem has no prefix of that length
        // to replace it with, and cutting it to fit would read the model into
        // the middle of the word: class 3 is modelled on `valtio`, whose stem
        // is longer than `eliö`, and `eliöita` came out of overlaying one on
        // the other, which left the plural stem of `eliö` as `eliöo` and built
        // none of the `eliöillä` and `eliöissä` the word has. The lemma is the
        // whole of its own stem there, and the ending goes on the end of it,
        // harmonised to the word the way the verb branch harmonises its own:
        // `ita` on a front word is `itä`, which is how `eliöitä` is spelled.
        //
        // The same holds the other way, for a lemma longer than the model whose
        // stem is the whole model word: `valo` is the whole of class 1's stem,
        // and `pieru` is the whole of its own, cut only where the model's is —
        // which is nowhere. Reading `pieruja` as `pier` + `ja` left the plural
        // stem as `pier` and built `pierilla`, `pierissa` and `pierin` for the
        // `pieruilla`, `pieruissa` and `pieruin` the word has.
        if model_stem >= lemma.len() || model_stem >= para.model().len() {
            let harmony = crate::grammar::harmony(lemma);
            let tail = crate::grammar::harmonise(tail, harmony);
            return Some(format!("{lemma}{tail}"));
        }
        let stem = lemma.get(..model_stem)?;
        Some(format!("{stem}{tail}"))
    }
}

/// Records a generated form.
///
/// No phonotactic filter runs here: the catalogue is authoritative, and a form
/// it lists has to survive whatever a heuristic thinks of it. The filter belongs
/// to the suggestion path, in `FormBuilder`.
/// The half of the alphabet a word is in: 1 for `a`, `o` and `u`, 2 for `ä`
/// and `ö`, and 0 for a word that has nothing to say either way.
fn half_of(word: &str) -> u8 {
    let mut out = 0;
    for c in word.chars() {
        match c {
            'a' | 'o' | 'u' => out |= 1,
            'ä' | 'ö' => out |= 2,
            _ => {}
        }
    }
    out
}

/// The confidence of a stem alternation read off the class.
///
/// Gradation and the genitive change a consonant or a stem vowel inside one
/// word, and the word keeps the harmony it came with: `lapsesta` is still a
/// front word. An alternation that puts the word in the other half of the
/// alphabet has not repaired the lemma, it has reached a different one, and the
/// `emä` of an `emi` or the `heltta` of a `hele` is a base form in its own
/// right. Those are guesses, and a guess is not offered as a correction.
fn alt_confidence(lemma: &str, alt_stem: &str) -> Confidence {
    if crosses_harmony(lemma, alt_stem) {
        Confidence::Guessed
    } else {
        Confidence::Repaired
    }
}

/// Whether a word and an alternative stem fall in different halves of the
/// alphabet, counting a neutral word as agreeing with either.
fn crosses_harmony(lemma: &str, other: &str) -> bool {
    let (a, b) = (half_of(lemma), half_of(other));
    a != 0 && b != 0 && a != b
}

/// The strong genitive stem of a noun, for the possessive, if the class has
/// one that differs from the form the possessive would otherwise attach to.
///
/// A noun whose genitive stem is not its lemma (`joki` is `joke`, `ovi` is
/// `ove`) attaches the possessive to that stem, so the possessive is `jokeni`
/// and `oveni` and not `jokini` and `ovini`. The genitive stem rule is the
/// class's own; applying it to the lemma keeps the strong grade, which is what
/// the genitive-possessive wants even though the plain genitive (`joen`) has
/// already gradated. A class with no such rule, or whose rule reproduces the
/// form, is left alone.
fn possessive_stem(lemma: &str, class: crate::kotus::ClassId, form: &str) -> Option<String> {
    let desc = crate::grammar::noun_class(class)?;
    // The possessive wants the strong genitive stem. For a class that names a
    // separate genitive list that is it; otherwise the class's own stem
    // readings are the genitive stem (`lapsi` is `lapse` on the class's
    // `THEME_E`), since the genitive of these classes is built on the same
    // stem as the rest.
    let rules = desc
        .genitive
        .unwrap_or(desc.stems)
        .iter()
        .filter(|s| !s.is_literal());
    // The strong genitive stem is the class's own first reading: `joki` is
    // `joke`, `ovi` is `ove` and `lapsi` is `lapse`, all on the `drop one and
    // add an e` rule, and not the deeper `joe` that a different member of the
    // class (or the weak grade) would give.
    let mut candidates: Vec<String> = rules
        .map(|s| crate::grammar::apply_stem_of(*s, lemma))
        .filter(|s| !s.is_empty() && s != form)
        .collect();
    candidates.dedup();
    if candidates.is_empty() {
        None
    } else {
        Some(candidates.remove(0))
    }
}

pub(super) fn push(
    out: &mut Vec<Generated>,
    stem: &str,
    ending: &str,
    tags: Tags,
    confidence: Confidence,
    pos: PartOfSpeech,
) {
    let word = format!("{stem}{ending}");
    if !word.is_empty() {
        out.push(Form::new(word, tags).of_pos(pos).with(confidence));
    }
}
