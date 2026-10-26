//! torille-hunspell: a thin C++ wrapper around `torille::suggest` that
//! exposes a hunspell-shaped C API.
//!
//! ## Why this exists
//!
//! Hunspell is the spell-checker used by Firefox, Chrome, LibreOffice, vim,
//! aspell and many other tools, and it exposes a small C API:
//!
//! ```c
//! Hunhandle* hunspell_create(const char* affpath, const char* dpath);
//! int        hunspell_spell  (Hunhandle*, const char* word);
//! int        hunspell_suggest(Hunhandle*, char*** slst, const char* word);
//! void       hunspell_free_list(Hunhandle*, char** slst, int n);
//! ```
//!
//! Anything that links against hunspell can call those four. Tools that
//! want to swap hunspell out for `torille` shouldn't have to learn a new
//! API, and tools that already link against `torille` shouldn't have to
//! learn hunspell's. This crate sits in the middle: it implements the
//! hunspell C ABI in front of `torille::suggest`, so a C++ project that
//! wants to spell-check Finnish without hunspell can `#include
//! "torille_hunspell.h"` and call `torille_hunspell_spell`.
//!
//! ## How it's wired
//!
//! `cxx::bridge` generates the FFI glue between Rust and C++ from a single
//! declaration. The Rust side here defines the `TorilleHandle` opaque
//! type and the three entry points (`check_word`, `suggest_words`,
//! `free_suggestions`). The C++ side in `src/hunspell_compat.cc` wraps
//! them in a class `torille::Speller` plus the flat C entry points a
//! hunspell consumer expects.

use torille::suggest;

/// The opaque type the C++ side holds a pointer to.
///
/// Empty on the Rust side because `torille::suggest::check` and
/// `torille::suggest::suggest` already manage their own cached lexicon
/// and catalogue. The struct exists so `cxx::bridge` can name a Rust
/// type and pass a `Box<TorilleHandle>` across the FFI.
pub struct TorilleHandle;

/// Construct a handle. Wrapped into `Box` so `cxx` can move it across the
/// FFI as an owned pointer; the C++ side holds the box and drops it via
/// `Box::from_raw` on destruction.
pub fn create() -> Box<TorilleHandle> {
    Box::new(TorilleHandle)
}

/// Hunspell's `hunspell_spell`: 1 if the word is in the dictionary, 0 if
/// not. The C++ side maps it back to a `bool`.
pub fn check_word(_handle: &TorilleHandle, word: &str) -> bool {
    !matches!(suggest::check(word), suggest::CheckOutcome::Unknown)
}

/// Hunspell's `hunspell_suggest`: returns up to `max_n` suggestions
/// ranked by edit distance, with the same shape the C++ side expects
/// (a `Vec<String>` it can index and then call `free_suggestions` on).
pub fn suggest_words(_handle: &TorilleHandle, word: &str, max_n: usize) -> Vec<String> {
    suggest::suggest(word, max_n, /* max_distance = */ 2)
}

/// Drop a previously-allocated suggestion list. Hunspell's
/// `hunspell_free_list` is a no-op because hunspell hands out a malloc'd
/// array. Our Rust side already owns the `Vec` and dropped it when the
/// unique pointer was dropped on the C++ side, so this entry point just
/// exists for symmetry.
pub fn free_suggestions(_handle: &TorilleHandle, _suggestions: &mut Vec<String>) {}

/// The cxx bridge. This is what the C++ side talks to.
///
/// `extern "Rust"` declares Rust functions the C++ side can call;
/// `extern "C++"` would declare the inverse, but we don't need it -- the
/// only thing C++ calls is the Rust entry points above.
#[cxx::bridge]
mod ffi {
    extern "Rust" {
        type TorilleHandle;

        fn create() -> Box<TorilleHandle>;
        fn check_word(handle: &TorilleHandle, word: &str) -> bool;
        fn suggest_words(handle: &TorilleHandle, word: &str, max_n: usize) -> Vec<String>;
        fn free_suggestions(handle: &TorilleHandle, suggestions: &mut Vec<String>);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test through the FFI surface: build a handle, ask it to
    /// spell-check a known word and a made-up word, and confirm the
    /// shape is right. This is what the C++ side does too, just without
    /// the FFI boundary.
    #[test]
    fn handle_spell_and_suggest_round_trip() {
        let handle = create();
        assert!(check_word(&handle, "talo"));
        assert!(check_word(&handle, "taloa"));
        assert!(check_word(&handle, "puhtaan"));
        assert!(!check_word(&handle, "xyzzy"));

        let suggestions = suggest_words(&handle, "taloo", 5);
        assert!(
            suggestions.iter().any(|w| w == "talo"),
            "taloo should suggest talo: {suggestions:?}"
        );

        // `Box<TorilleHandle>` drops here, the way the C++ side's
        // `UniquePtr<TorilleHandle>` drops on destruction.
        drop(handle);
    }
}
