//! Compound recognition: heads/tails, the DP segmenter and the
//! hyphenated-lexeme guard.

use crate::tags::PartOfSpeech;

use super::derived::bound_suffix_compound_base;
use super::stems::{compound_head_from_lemma, is_content_head};
use super::{catalogue, is_known_word_no_compound, is_known_word_with_compound, lexicon};

/// Recognise a compound without running the stem-map, derivation and verb
/// gates that [`is_known_word`] runs first.
///
/// The spelling path only needs a yes/no answer for a word that is already
/// known to be a compound; the full detector spends most of its time on
/// `-inen` degree reconstruction and the noun/verb strip loops. This is the
/// compound part alone, with the same rules:
///
///   * a closed spelling of a lexically hyphenated compound (`linjaauto`) is
///     a misspelling, not a known compound;
///   * the ordinary head/tail split wins over a bound suffix head
///     (`henkiruumiiseenne` names `ruumis`, not `‑ruumiinen`).
pub(crate) fn is_known_compound(word: &str) -> Option<String> {
    is_known_compound_with(word, /* engine_fallback */ true)
}

/// The engine-free variant of [`is_known_compound`].
///
/// First-pass spell checking must stay bounded for unknown words. The
/// engine fallback in `detect_compound_at` is what lets a compound head
/// missing from the old stem snapshot (`kekkereitä`) be recognised, but it
/// also spends seconds probing an arbitrary proper name such as
/// `sortavalan`. The first pass can underline those and let the suggestion
/// pool's full `bg.check` demote them; it must not run the walk itself.
pub(crate) fn is_known_compound_fast(word: &str) -> Option<String> {
    // Short words are cheap to underline and let the full slow check
    // demote them. Skipping the split search here keeps a page of ordinary
    // six/seven-letter misspellings from paying the compound probe. The
    // cheap [`is_known_short_compound`] covers the short compounds that do
    // occur (`esisana`, `autotie`).
    if word.chars().count() < 8 {
        return None;
    }
    is_known_compound_with(word, /* engine_fallback */ false)
}

/// The 6-7 character compounds the full probe leaves to the slow check.
///
/// [`is_known_compound_fast`] starts at eight characters: the general split
/// search is several times more expensive on short words, because most
/// splits have no head and the modifier side recurses into the rest. Short
/// compounds are still ordinary words (`esisana`, `autotie`, `esiosa`), so
/// this checks them with constant-time probes only: a known content-word
/// modifier on the left and a content-word lemma on the right. No nesting,
/// no possessive, no engine.
///
/// Callers must already have established that the whole word is not known;
/// this deliberately skips the "a known form is not a compound" guard,
/// which costs more than the split search at this length.
pub(crate) fn is_known_short_compound(word: &str) -> Option<String> {
    let count = word.chars().count();
    if !(6..8).contains(&count) || word.contains('-') {
        return None;
    }
    let lower = word.to_lowercase();
    let mut splits: Vec<usize> = (1..count).collect();
    splits.reverse();
    for split in splits {
        let byte_split = lower
            .char_indices()
            .nth(split)
            .map(|(i, _)| i)
            .unwrap_or(lower.len());
        let (left, right) = lower.split_at(byte_split);
        // The same head guard as the general probe: a function word or a
        // bare connegative is not a compound head.
        if crate::expand::negation::negation_base(right).is_some()
            || crate::expand::connegative::base(right).is_some()
        {
            continue;
        }
        // A function-word modifier (`ja` + `talo`) is not a compound. Bound
        // prefixes (`esi`) have no sanalista row, so they stay content.
        if !crate::expand::modifier_is_known(left) || !crate::compound::is_content_head(left) {
            continue;
        }
        if crate::kotus::sanalista().by_word(right).next().is_none()
            || !crate::compound::is_content_head(right)
        {
            continue;
        }
        return Some(right.to_string());
    }
    None
}

/// Dynamic-programming compound segmentation.
///
/// Transitions are dictionary modifiers, not arbitrary character splits:
/// `solve(pos)` asks whether the remainder from `pos` can be a compound
/// ending in a valid final form, and memoises the answer. The final-form
/// check is the same non-compound detector as before, so acceptance stays
/// exact; the saving is that it runs at most once per position instead of
/// once per split, and the transition loop only tests prefixes that
/// `modifier_is_known` accepts. This supports compounds of any length
/// without a component cap.
/// A compound head is a content word; the negation auxiliary and the bare
/// connegative are checked on the surface before this. Verbs are allowed
/// because an adverb such as `torkuksiin` names `torkkua`.
fn is_compound_head(base: &str) -> bool {
    crate::compound::is_content_head(base)
}

fn segment_compound_dp(word: &str) -> Option<String> {
    let offsets: Vec<usize> = word
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(word.len()))
        .collect();
    let n = offsets.len() - 1;
    if n < 2 {
        return None;
    }
    let mut memo: Vec<Option<String>> = vec![None; n + 1];
    let mut done = vec![false; n + 1];

    fn solve(
        pos: usize,
        word: &str,
        offsets: &[usize],
        n: usize,
        memo: &mut [Option<String>],
        done: &mut [bool],
    ) -> Option<String> {
        if done[pos] {
            return memo[pos].clone();
        }
        done[pos] = true;
        // The remainder can be the final word form. Position 0 is the whole
        // word: the caller's own gates have already decided that it is not a
        // known non-compound, and a compound needs at least one modifier.
        if pos > 0 {
            let surface = &word[offsets[pos]..];
            // A function word is not a compound head; `-en`/`-an`/`-än` and
            // the bare connegatives otherwise look like a possessive or a
            // verb form of the negation auxiliary.
            if crate::expand::negation::negation_base(surface).is_some()
                || crate::expand::connegative::base(surface).is_some()
            {
                // fall through to the modifier search
            } else if let Some(base) = is_known_word_no_compound(surface)
                && is_compound_head(&base)
            {
                memo[pos] = Some(base.clone());
                return Some(base);
            }
        }
        // Longest modifier first, matching the old split search's preference.
        for end in (pos + 1..n).rev() {
            // A one- or two-character modifier is usually a stem fragment,
            // not a compound member; `ky` + `kynnsä` was the false split in
            // `todistelukykynnsä`. Short real modifiers fall back to the old
            // search, which is still available when the DP misses.
            if end - pos < 3 {
                continue;
            }
            let left = &word[offsets[pos]..offsets[end]];
            if !crate::expand::modifier_is_known(left) {
                continue;
            }
            if let Some(base) = solve(end, word, offsets, n, memo, done) {
                memo[pos] = Some(base.clone());
                return Some(base);
            }
        }
        None
    }

    solve(0, word, &offsets, n, &mut memo, &mut done)
}

/// The DP compound probe used by `check_normal_fast`.
///
/// The caller (gate 4c) has already established that the word is not a
/// known non-compound form, so this skips the expensive "already known"
/// guard that `is_known_compound_with` would otherwise recompute.
pub(crate) fn is_known_compound_dp(word: &str) -> Option<String> {
    if word.contains('-') || word.chars().count() < 6 {
        return None;
    }
    segment_compound_dp(&word.to_lowercase())
}

fn is_known_compound_with(word: &str, engine_fallback: bool) -> Option<String> {
    let lower = word.to_lowercase();
    // Hyphenated input is a spelling decision in its own right, not a closed
    // compound: `kirja-hylly` must still reach `suggest`, which offers
    // `kirjahylly`.
    if lower.contains('-') || lower.chars().count() < 6 {
        return None;
    }
    let part_known = |part: &str| {
        is_known_word_with_compound(part, false).is_some() || crate::expand::modifier_is_known(part)
    };
    // A closed spelling of a lexically hyphenated compound is a
    // misspelling (`linjaauto` for `linja-auto`). The probe has to name a
    // real hyphenated lexeme: the shape rule alone fires on any same-vowel
    // boundary (`päivänpa|ahtamia`) and would reject a perfectly good
    // ordinary compound.
    if hyphenated_lexeme(&lower).is_some()
        || crate::compound::closed_hyphenated_spelling(&lower, part_known).is_some()
    {
        return None;
    }
    // A word that already resolves without compound analysis is an ordinary
    // inflected form, not a compound: `kissaa` is the partitive of `kissa`,
    // and `salaperäisempi` is a degree of `salaperäinen`, even though
    // `kis` + `saa` and `salaperäis` + `empi` also look like splits.
    if is_known_word_with_compound(&lower, false).is_some() {
        return None;
    }
    // A possessive suffix belongs to the whole compound, not to its head:
    // `henkiruumiiseenne` is `henki` + `ruumiiseen` + `-nne`, and
    // `turhamielisyyksissäni` is `turha` + `mielisyyksissä` + `-ni`. Strip it
    // first and retry on the host: otherwise the bare split search names the
    // possessive tail as the head (`aamujuomanne` -> `manne`) instead of a
    // content head of the host.
    // `-en` is the contracted third-person possessive after a case ending
    // (`portille` + `en` = `portilleen`); it belongs here as well as in the
    // non-compound stripper, otherwise `kotiportilleen` never reaches its
    // `koti` + `portille` split.
    for ending in ["nsa", "nsä", "nne", "mme", "ni", "si", "en", "an", "än"] {
        // `-si` is also the tail of the translative `-ksi`; stripping it
        // turns every `-ksi` form into a fake possessive host and sends the
        // compound probe down a slow, pointless path.
        if ending == "si" && lower.ends_with("ksi") {
            continue;
        }
        let Some(host) = lower.strip_suffix(ending) else {
            continue;
        };
        if host.is_empty() {
            continue;
        }
        // `-en` is also an ordinary illative ending (`lankivarteen`), so only
        // strip it as the contracted third-person possessive when the host
        // ends in a case form that possessive actually follows. Otherwise
        // `lankivarteen` loses its illative and resolves as `lanki` +
        // `varte`.
        if matches!(ending, "en" | "an" | "än")
            && ![
                "lle", "lta", "ltä", "lla", "llä", "ssa", "ssä", "sta", "stä", "ksi", "na", "nä",
                "ine",
            ]
            .iter()
            .any(|case| host.ends_with(case))
        {
            continue;
        }
        if let Some(head) = detect_compound_shallow(host, engine_fallback) {
            return Some(head);
        }
        // The illative `-seen` contracts one `n` before `-nne`:
        // `ruumiiseen` + `nne` -> `ruumiiseenne`.
        let illative = format!("{host}n");
        if let Some(head) = detect_compound_shallow(&illative, engine_fallback) {
            return Some(head);
        }
    }
    if let Some(head) = detect_compound_shallow(&lower, engine_fallback) {
        return Some(head);
    }
    if let Some(head) = bound_suffix_compound_base(&lower) {
        return Some(head);
    }
    None
}

/// Internal entry point. `allow_compound` gates the recursive
/// compound-detection step; the public [`is_known_word`] always
/// passes `true`, while the inner recursion from
/// [`detect_compound`] passes `false` so the modifier side of
/// a split is checked against the lemma / stem maps alone and
pub(crate) fn hyphenated_lexeme(word: &str) -> Option<String> {
    if word.contains('-') {
        return None;
    }
    let lower = word.to_lowercase();
    for (i, _) in lower.char_indices().skip(1) {
        let candidate = format!("{}-{}", &lower[..i], &lower[i..]);
        if crate::kotus::sanalista()
            .by_word(&candidate)
            .next()
            .is_some()
        {
            return Some(candidate);
        }
    }
    None
}

/// Recognise a correctly hyphenated compound.
///
/// The shape rules live in [`crate::compound::hyphen_rule`]; this function
/// supplies the lexical side. Every part except the first must resolve (or
/// be a known proper name / derivational stem), and the head must be a
/// content word. A numeric first part may introduce an otherwise unknown
/// suffix (`20-vuotias`), which is the one deliberate escape hatch.
pub(super) fn hyphenated_base(word: &str) -> Option<String> {
    let parts = crate::compound::split_hyphenated(word)?;
    let closed: String = parts.concat().to_lowercase();
    let closed_known = is_known_word_with_compound(&closed, false).is_some();
    let rule = crate::compound::hyphen_rule(word, &parts, closed_known)?;
    let first_is_special = matches!(
        rule,
        crate::compound::HyphenRule::Abbreviation
            | crate::compound::HyphenRule::Number
            | crate::compound::HyphenRule::ProperName
    );

    let head_index = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 && first_is_special {
            continue;
        }
        let lower = part.to_lowercase();
        let resolved = is_known_word_with_compound(&lower, false).is_some()
            || crate::expand::modifier_is_known(&lower)
            || (i == head_index && crate::compound::is_proper_name_part(part));
        if !resolved && !(i == head_index && rule == crate::compound::HyphenRule::Number) {
            return None;
        }
    }

    let head = parts.last()?;
    let head_lower = head.to_lowercase();
    let head_base = is_known_word_with_compound(&head_lower, false)
        .or_else(|| crate::compound::is_proper_name_part(head).then_some(head_lower.clone()));
    match head_base {
        Some(base) => {
            if !crate::compound::is_content_head(&base) {
                return None;
            }
            Some(base)
        }
        None if rule == crate::compound::HyphenRule::Number
            && head.chars().next().is_some_and(char::is_alphabetic) =>
        {
            Some(word.to_lowercase())
        }
        None => None,
    }
}

/// Fast compound detector. Walks every split position and asks
/// [`is_known_word_with_compound`] (with `allow_compound=false`)
/// about each half. The first split where both halves resolve
/// wins; the head's lemma is returned.
///
/// A three-part compound has one side that is itself a compound, so the
/// left half is allowed a bounded number of further splits
/// (`henkivartioväki` = `henkivartio` + `väki` = `henki` + `vartio` +
/// `väki`). The right half stays a single word: it is the head, and its
/// own head is the whole word's head.
///
/// Returns `None` for non-compounds or for compounds whose halves
/// are not both in the fast-path coverage (e.g. a genitive stem
/// the noun stem map doesn't carry, or a possessive suffix the
/// `ENDINGS` table doesn't strip). The slow
/// `crate::expand::find_any_base_with` covers those.
pub(super) fn detect_compound(word: &str) -> Option<String> {
    detect_compound_at(word, 0, true, true)
}

/// [`detect_compound`] without the engine fallback, used by the
/// recognition fast path.
///
/// The `left_is_compound` recursion is kept: Finnish compounds nest, and a
/// three-part word has a modifier that is itself a compound
/// (`salakirjevaihdossa` = `sala` + `kirje` + `vaihto`,
/// `henkivartioväki` = `henki` + `vartio` + `väki`). Without it every such
/// word was reported unknown by the first pass even though both of its
/// parts are ordinary words. The recursion is bounded by
/// [`MAX_COMPOUND_DEPTH`], and with `engine_fallback` false the deep split
/// search never reaches the full engine walk.
fn detect_compound_shallow(word: &str, engine_fallback: bool) -> Option<String> {
    detect_compound_at(word, 0, true, engine_fallback)
}

/// Depth limit for [`detect_compound_at`]. Finnish has no meaningful
/// bound, but every recursion chooses a shorter left half; four is well
/// past the reported `henki + vartio + väki` and keeps the search cheap.
const MAX_COMPOUND_DEPTH: usize = 4;

/// Longest right-hand head worth sending to the engine when the stem maps
/// miss it. The fallback exists for heads the old form snapshot lacks
/// (`kekkereitä`, ten chars); a long word on the right is usually a derived
/// adjective, not a compound head, and the full engine may spend seconds on
/// it (`salaperäisemmäksi` -> `peräisemmäksi`). The bound keeps compound
/// recognition from turning a non-compound into the slow path.
const MAX_COMPOUND_ENGINE_HEAD: usize = 12;

/// Class-6 plural-only heads (`kekkerit` -> `kekkereitä`) are the one
/// engine fallback the bounded fast path still allows. The stem maps were
/// built from an older snapshot, but these `-e-` plural endings identify
/// the shape cheaply, so a proper name such as `haltiuduin` does not send
/// its right half through the engine.
fn class6_plural_head(word: &str) -> bool {
    const ENDINGS: &[&str] = &[
        "eitä", "eissä", "eistä", "eihin", "eillä", "eiltä", "eille", "einä", "eiksi", "eittä",
        "eiden", "eitten", "eja", "eita", "ein", "eineen", "eine",
    ];
    ENDINGS.iter().any(|ending| word.ends_with(ending))
}

fn detect_compound_at(
    word: &str,
    depth: usize,
    recurse_left: bool,
    engine_fallback: bool,
) -> Option<String> {
    // Prefer a split whose modifier is a real word over one that is itself
    // only a compound or a bound modifier. The longest-modifier-first walk
    // otherwise picks `teatterikuole` + `maa` for `teatterikuolemaa` before
    // it ever reaches the correct `teatteri` + `kuolemaa`: `teatterikuole`
    // resolves through `kuolla`'s stem, but the head `maa` is the wrong
    // segmentation. The second pass keeps the old behaviour for words that
    // genuinely need a compound/bound modifier (`salakirjevaihdossa`,
    // `uimapaikalleen`).
    detect_compound_pass(word, depth, recurse_left, engine_fallback, true)
        .or_else(|| detect_compound_pass(word, depth, recurse_left, engine_fallback, false))
}

fn detect_compound_pass(
    word: &str,
    depth: usize,
    recurse_left: bool,
    engine_fallback: bool,
    direct_left_only: bool,
) -> Option<String> {
    if depth >= MAX_COMPOUND_DEPTH {
        return None;
    }
    // Walk splits from longest-modifier to shortest-modifier.
    // Finnish compounds prefer the longest-possible prefix as
    // the modifier, so the first matching split wins.
    //
    // Skip splits where the right half isn't a known word and
    // try a shorter modifier. Longest-modifier-first means most
    // candidate splits bail early; only the first match wins.
    //
    // Iterate over char boundaries, not byte indices -- words
    // like `kylmäkahvi` have multi-byte UTF-8 (ä = 2 bytes)
    // and `split_at(byte_index)` panics on a non-boundary.
    let mut splits: Vec<usize> = (1..word.chars().count()).collect();
    splits.reverse();
    for split in splits {
        let byte_split = word
            .char_indices()
            .nth(split)
            .map(|(i, _)| i)
            .unwrap_or(word.len());
        let (left, right) = word.split_at(byte_split);
        // A function word or a bare connegative is never a compound
        // head: `tehtäne` + `en` is not a compound, and accepting it
        // would make the fast path reach `tehtäneen` (the passive
        // potential) with the wrong base. The head of a Finnish compound
        // is a content word.
        if crate::expand::negation::negation_base(right).is_some()
            || crate::expand::connegative::base(right).is_some()
        {
            continue;
        }
        let mut head_lemma = match is_known_word_with_compound(right, false) {
            Some(base) => base,
            None => {
                // The stem maps are built from an earlier engine snapshot.
                // A class the engine has since fixed (the plural-only class-6
                // `kekkerit` -> `kekkereitä`) can be missing there while the
                // paradigm itself is already right. Ask the engine for the
                // head only after the cheap maps missed, only when the left
                // side is a known word, and only for a head long enough to
                // be a word. The left must be a word, not merely a modifier:
                // a derived prefix such as `salaperäise` would otherwise
                // send `mmäksi` through the engine, which spends seconds on
                // the non-compound reading.
                if depth != 0
                    || right.chars().count() < 6
                    || right.chars().count() > MAX_COMPOUND_ENGINE_HEAD
                    || is_known_word_with_compound(left, false).is_none()
                    || (!engine_fallback && !class6_plural_head(right))
                {
                    continue;
                }
                let found = crate::expand::find_any_base_with(lexicon(), catalogue(), right);
                let Some(lemma) = found.first() else {
                    continue;
                };
                lemma.word.to_string()
            }
        };
        // The form-set fallback names the surface itself (`varteen`). When a
        // stem map can name the lemma behind it (`varsi`, `puoli`), that is
        // the base a caller expects, and it is also what the content-head
        // check below needs to see.
        if head_lemma == right
            && let Some(base) = crate::expand::check_normal::find_base_via_stem_map(
                right,
                PartOfSpeech::Noun,
                false,
            )
        {
            head_lemma = base;
        }
        // `teiden` is a form of both `tee` and `tie`; the first base the
        // stem map returns is `tee`, which is the wrong head for
        // `raitiotie`. Prefer the base that makes the whole word a
        // sanalista compound lemma (`raitio` + `tie` = `raitiotie`). A
        // surface that is itself a function-word lemma (`vallassa` is an
        // adverb as well as a form of `valta`) is only rejected when this
        // content-base disambiguation finds nothing.
        let disambiguated = compound_head_from_lemma(left, right);
        // Check the *named head*, not just the surface: `si` has no lemma
        // row of its own, so `is_content_head("si")` would pass, while the
        // base it resolves to is the pronoun `tämä` (`vetäsi`) or `sä`
        // (`kysäsi`). Those are one-letter-short misspellings of `vetäisi`
        // and `kysäisi`, not compounds. When the whole-word lemma names a
        // content base (`harvainvallassa` -> `valta`), that base is the one
        // to check, not the adverb surface `vallassa`.
        let mut named_head = disambiguated.unwrap_or(head_lemma);
        // A derived nominal head (`päivänpaahtama` = `päivä` + `paahtama`)
        // has no sanalista row, so the lemma probe below cannot see it. The
        // derivation layer knows its POS, which is the question here.
        let derived_nominal = crate::expand::derived::is_nominal(right);
        if !derived_nominal && !is_content_head(&named_head) {
            // A surface can be a function-word lemma and a content
            // inflection at the same time: `varresta` is a postposition,
            // but also the elative of `varsi` (`lankivarresta`). Recover
            // the content base from the noun stem map before rejecting the
            // split.
            let Some(base) = crate::expand::check_normal::find_base_via_stem_map(
                right,
                PartOfSpeech::Noun,
                false,
            )
            .or_else(|| crate::expand::check_normal::find_base_via_verb_stem_map(right, false)) else {
                continue;
            };
            if !is_content_head(&base) {
                continue;
            }
            named_head = base;
        }
        // The left may itself be a compound (`henkivartio` in
        // `henkivartioväki`); try one more level before falling back.
        let left_word = is_known_word_with_compound(left, false);
        // First pass: accept only a modifier that resolves as a word of its
        // own. A compound or a bound modifier is deferred to the second pass.
        if direct_left_only {
            if left_word.is_some() {
                return Some(named_head);
            }
            continue;
        }
        let left_is_compound = recurse_left
            && left_word.is_none()
            && detect_compound_at(left, depth + 1, true, engine_fallback).is_some();
        if left_word.is_some() || left_is_compound || crate::expand::modifier_is_known(left) {
            return Some(named_head);
        }
    }
    None
}
