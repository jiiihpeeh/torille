//! Ad-hoc derived-form probes that the compound/known-word gates use.

use crate::tags::PartOfSpeech;

use super::{catalogue, is_known_word_with_compound};

pub(super) fn derived_sti_base(word: &str) -> Option<String> {
    let stripped = word.strip_suffix("sti")?;
    if stripped.chars().count() < 3 {
        return None;
    }
    // An `-inen` adjective takes `-sesti`: `tapainen` -> `tapaisesti`, so
    // the host before `-sti` is `...tapaise`. Put the `nen` back before
    // looking for the adjective.
    let host = if let Some(stem) = stripped.strip_suffix("se") {
        format!("{stem}nen")
    } else {
        stripped.to_string()
    };
    let host = host.as_str();
    let Some(base) = is_known_word_with_compound(host, false) else {
        // A compound adjective has no row of its own (`suonenvedontapainen`
        // = `suonenveto` + `tapainen`). Its head shape and a known modifier
        // are the evidence that the `-sti` host is an adjective rather than
        // a noun; avoid re-entering compound detection here, which would
        // make a suffix check walk the whole compound recursively.
        let adjective_compound = ["tapainen", "lainen", "mainen", "inen"]
            .iter()
            .any(|suffix| {
                host.strip_suffix(suffix)
                    .is_some_and(crate::expand::modifier_is_known)
            });
        return adjective_compound.then(|| host.to_string());
    };
    let Some(row) = crate::kotus::sanalista().by_word(&base).next() else {
        let adjective_head = ["tapainen", "lainen", "mainen", "inen"]
            .iter()
            .any(|suffix| host.ends_with(suffix));
        return adjective_head.then_some(base);
    };
    let adjective_or_verb = crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
        .iter()
        .any(|pos| matches!(pos, PartOfSpeech::Adjective | PartOfSpeech::Verb));
    adjective_or_verb.then_some(base)
}

/// A classed suffix row from the sanalista (`‑mielisyys`, `‑peräinen`),
/// stored with its base spelling and paradigm so a compound can be checked
/// against it.
struct BoundSuffix {
    base: &'static str,
    head: String,
    inflections: Vec<crate::kotus::Inflection>,
    pos: PartOfSpeech,
}

/// The bound suffix rows, materialised once. There are a few hundred; the
/// first compound that needs one pays for the scan, later ones reuse it.
fn bound_suffixes() -> &'static [BoundSuffix] {
    static SUFFIXES: std::sync::OnceLock<Vec<BoundSuffix>> = std::sync::OnceLock::new();
    SUFFIXES.get_or_init(|| {
        crate::kotus::lemmas()
            .iter()
            .filter_map(|lemma| {
                let base = lemma.word.strip_prefix('\u{2011}')?;
                if base.is_empty() || lemma.inflections.is_empty() {
                    return None;
                }
                Some(BoundSuffix {
                    base,
                    head: base.chars().take(5).collect(),
                    inflections: lemma.inflections.clone(),
                    pos: PartOfSpeech::parse_all(lemma.part_of_speech)
                        .first()
                        .copied()
                        .unwrap_or(PartOfSpeech::Noun),
                })
            })
            .collect()
    })
}

/// Recognise a compound built on a bound suffix row: `turha` + `mielisyys`
/// gives `turhamielisyyksissäni`, `sala` + `peräinen` gives
/// `salaperäisemmäksi`. The modifier has to resolve and the suffix's own
/// paradigm has to emit the right-hand side; the five-character prefix
/// check only keeps the suffix scan cheap.
pub(super) fn bound_suffix_compound_base(word: &str) -> Option<String> {
    let char_count = word.chars().count();
    if char_count < 8 {
        return None;
    }
    let suffixes = bound_suffixes();
    for split in 2..char_count {
        let byte_split = word.char_indices().nth(split).map(|(index, _)| index)?;
        let (modifier, right) = word.split_at(byte_split);
        if right.chars().count() < 4 || !crate::expand::modifier_is_known(modifier) {
            continue;
        }
        for suffix in suffixes {
            if right.len() < 4 || !right.starts_with(&suffix.head) {
                continue;
            }
            let forms = crate::expand::inflect_all(
                suffix.base,
                &suffix.inflections,
                catalogue(),
                None,
                suffix.pos,
                true,
            );
            if forms.iter().any(|form| form.word == right) {
                return Some(suffix.base.to_string());
            }
        }
    }
    None
}

/// The derived `-us`/`-ys` action noun a surface belongs to, when the verb
/// it comes from is a `-ta`/`-tä` verb (`hoilata` -> `hoilaus`).
///
/// The nominative is reconstructed by peeling the common case endings; the
/// verb source must be in the sanalista and the whole class-39 paradigm must
/// emit the surface. That verification is what keeps ordinary `-us` nouns
/// from being treated as derivations of an unrelated verb.
pub(super) fn derived_action_us_base(word: &str) -> Option<String> {
    use crate::kotus::Inflection;
    let mut candidates = vec![word.to_string()];
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        if let Some(stem) = word.strip_suffix(ending)
            && !stem.is_empty()
        {
            candidates.push(stem.to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    for candidate in candidates {
        let Some(stem) = candidate
            .strip_suffix("us")
            .or_else(|| candidate.strip_suffix("ys"))
        else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        let mut source_is_verb = false;
        for suffix in ["ta", "tä"] {
            let verb = format!("{stem}{suffix}");
            let Some(row) = crate::kotus::sanalista().by_word(&verb).next() else {
                continue;
            };
            if crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                .contains(&PartOfSpeech::Verb)
            {
                source_is_verb = true;
                break;
            }
        }
        if !source_is_verb {
            continue;
        }
        let forms = crate::expand::inflect_all(
            &candidate,
            &[Inflection {
                class: 39,
                gradation: None,
            }],
            catalogue(),
            None,
            PartOfSpeech::Noun,
            true,
        );
        if forms.iter().any(|form| form.word == word) {
            return Some(candidate);
        }
    }
    None
}

/// The `-hko`/`-hkö` adjective a surface belongs to (`kostea` ->
/// `kosteahko`). The derived adjective is not a lemma, so reconstruct its
/// nominative and verify the class-1 paradigm against the surface. The base
/// has to be an adjective, which keeps an unrelated `-hko` noun stem out.
pub(super) fn derived_hko_base(word: &str) -> Option<String> {
    use crate::kotus::Inflection;
    let mut candidates = vec![word.to_string()];
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        if let Some(stem) = word.strip_suffix(ending)
            && !stem.is_empty()
        {
            candidates.push(stem.to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    for candidate in candidates {
        let Some(base) = candidate
            .strip_suffix("hko")
            .or_else(|| candidate.strip_suffix("hkö"))
        else {
            continue;
        };
        if base.is_empty() {
            continue;
        }
        let Some(row) = crate::kotus::sanalista().by_word(base).next() else {
            continue;
        };
        if !crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
            .contains(&PartOfSpeech::Adjective)
        {
            continue;
        }
        let forms = crate::expand::inflect_all(
            &candidate,
            &[Inflection {
                class: 1,
                gradation: None,
            }],
            catalogue(),
            None,
            PartOfSpeech::Adjective,
            true,
        );
        if forms.iter().any(|form| form.word == word) {
            return Some(candidate);
        }
    }
    None
}

/// Candidate superlative nominatives for a degree form: the prefix before
/// the superlative marker (`imma` in the oblique stem, `imp` in the
/// partitive) plus `-in`. `huomatuimpia` -> `huomatuin`,
/// `kauneimman` -> `kaunein`.
fn superlative_candidates(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    for marker in ["imma", "imp"] {
        if let Some(pos) = word.rfind(marker) {
            let prefix = &word[..pos];
            if !prefix.is_empty() {
                out.push(format!("{prefix}in"));
            }
        }
    }
    out
}

/// The positive `-inen` adjective behind a comparative or superlative
/// surface (`salaperäisemmäksi` -> `salaperäinen`, `salaperäisin` ->
/// `salaperäinen`).
///
/// The degree is derived from the positive by the engine, but the fast path
/// sees only the surface. Try every prefix of the surface followed by
/// `nen`; only a prefix that is an adjective lemma is a candidate, and the
/// candidate's whole paradigm has to emit the surface. That bounds this to
/// real `-inen` adjectives and keeps nouns such as `nainen` out.
pub(super) fn derived_inen_degree_base(word: &str) -> Option<String> {
    use crate::tags::PartOfSpeech;

    // Degree suffixes are `-mpi`/`-mm-` (comparative) and `-in`/`-imm-`
    // (superlative); the leading `s` belongs to the degree stem. The guard
    // keeps the prefix walk off ordinary words.
    if !(word.contains("mp") || word.contains("mm") || word.contains("sin") || word.ends_with("in"))
    {
        return None;
    }
    let sanalista = crate::kotus::sanalista();
    for (byte, _) in word.char_indices().skip(1) {
        let prefix = &word[..byte];
        let candidate = format!("{prefix}nen");
        let Some(row) = sanalista.by_word(&candidate).next() else {
            continue;
        };
        if !PartOfSpeech::parse_all(row.part_of_speech()).contains(&PartOfSpeech::Adjective) {
            continue;
        }
        let inflections: Vec<_> = row.inflections().collect();
        let forms = crate::expand::inflect_all(
            &candidate,
            &inflections,
            catalogue(),
            None,
            PartOfSpeech::Adjective,
            false,
        );
        if forms.iter().any(|form| form.word == word) {
            return Some(candidate);
        }
    }
    None
}

/// The superlative degree a surface form belongs to, if the reconstructed
/// nominative is itself a known word and class 36 really emits the surface.
///
/// This covers degrees of participles (`huomattu` -> `huomatuin` ->
/// `huomatuimpia`), which `ADJ_DEGREES` cannot tabulate because the
/// positive is a verb form rather than an adjective lemma.
pub(super) fn derived_superlative_base(word: &str) -> Option<String> {
    use crate::expand::inflect_all;
    use crate::kotus::Inflection;
    for nominative in superlative_candidates(word) {
        if is_known_word_with_compound(&nominative, false).is_none() {
            continue;
        }
        let forms = inflect_all(
            &nominative,
            &[Inflection {
                class: 36,
                gradation: None,
            }],
            catalogue(),
            None,
            PartOfSpeech::Adjective,
            false,
        );
        if forms.iter().any(|f| f.word == word) {
            return Some(nominative);
        }
    }
    None
}
