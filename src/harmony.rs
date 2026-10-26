//! Finnish vowel harmony primitives.
//!
//! `front` decides whether a lemma sits in the front (`ä`/`ö`/`y`) or back
//! (`a`/`o`/`u`) half of the alphabet, by the last decisive vowel scanning
//! right to left. `pick_harmony` is the operation when both spellings are
//! already on hand -- it picks the one matching the given front/back flag.
//!
//! The hand-written inflection engines (`expand::nominals::Builder`,
//! `expand::verbs::VerbBuilder`) both call these, so the primitives live
//! at the crate root rather than being carried by either engine. The
//! heuristic engine uses its own separate `crate::grammar::harmony` for
//! ending harmonisation; this module is the hand-written engine's
//! counterpart. A change to how Finnish vowel harmony works -- adding a
//! new neutral vowel, reclassifying `ô` -- lands in one place.

/// Whether a lemma is in the front half of the alphabet, by its last
/// decisive vowel scanning right to left. `e` and `i` are neutral (a word
/// without a decisive vowel is front by default).
///
/// `talo` is back on its `o`, `äly` front on its `y`, and a compound is
/// settled by its head (`hätähousu` back on its `u`).
pub fn front(lemma: &str) -> bool {
    lemma
        .chars()
        .rev()
        .find_map(|c| match c {
            // `ü` is the umlaut form of the same /y/ sound `y` represents;
            // not common in Finnish, but a word carrying it is front by the
            // same reasoning.
            'ä' | 'ö' | 'y' | 'ü' | 'Ä' | 'Ö' | 'Y' | 'Ü' => Some(true),
            // `ô` is the French `o`, back in Finnish treatment (entrecôte).
            // `å` is the Swedish loanword vowel, back in Finnish treatment
            // (`grå`, `små`); `is_vowel` already counts it as Finnish.
            'a' | 'o' | 'u' | 'ô' | 'å' | 'A' | 'O' | 'U' | 'Å' => Some(false),
            _ => None,
        })
        .unwrap_or(true)
}

/// Pick the spelling matching the given front/back flag: `front_twin` in
/// the front half, `back` in the back. The free version of the `.h()` method
/// the [`Builder`](crate::expand::nominals::Builder) and
/// [`VerbBuilder`](crate::expand::verbs::VerbBuilder) types carry, for code
/// that has no builder to hand -- a stem-joining suffix table, the declension
/// helpers that build the plural oblique from the lemma's harmony directly.
pub fn pick_harmony<'a>(front: bool, back: &'a str, front_twin: &'a str) -> &'a str {
    if front { front_twin } else { back }
}