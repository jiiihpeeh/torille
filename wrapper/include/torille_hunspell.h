// SPDX-License-Identifier: MIT OR Apache-2.0
//
// C++ wrapper around the torille spell-checker, shaped like the hunspell
// C API so a C++ project that wants to spell-check Finnish without
// pulling in hunspell can `#include "torille_hunspell.h"` and call the
// same four entry points hunspell exposes.
//
// The implementation is in `src/hunspell_compat.cc`; this header is what
// C++ consumers include. The `TorilleHandle` type is a thin opaque pointer
// to a Rust `Box<TorilleHandle>`, owned by the C++ side and dropped on
// destruction.

#pragma once

#include <cstddef>
#include <string>
#include <vector>

namespace torille {

/// A thin wrapper around the torille spell-checker.
///
/// Construction parses the embedded sanalista and paradigm catalogue into
/// a `&'static` reference, then owns a `Box<TorilleHandle>` allocated in
/// Rust. Destruction drops the box. `check_word` and `suggest` are
/// thread-safe because every field the reads touch is shared `&'static`.
class Speller {
public:
    /// Build a spell-checker that reads the embedded sanalista and
    /// paradigm catalogue.
    Speller();

    /// Drop the underlying `Box<TorilleHandle>`. The destructor is the
    /// single source of truth for handle lifetime.
    ~Speller();

    Speller(const Speller&) = delete;
    Speller& operator=(const Speller&) = delete;
    Speller(Speller&&) = delete;
    Speller& operator=(Speller&&) = delete;

    /// True if `word` is a known Finnish word (lemma, inflected form, or
    /// compound; compound analysis is the slow path and is not wired in
    /// here, so a compound that has no single-lemma source is "false").
    bool check_word(const std::string& word) const;

    /// Up to `max_n` spell-check suggestions for `word`, ranked by edit
    /// distance (smaller first). Empty when the word is unknown to the
    /// index and no candidate is within distance 2.
    std::vector<std::string>
    suggest(const std::string& word, std::size_t max_n = 5) const;

    /// The raw `void*` form, for callers that want to hand the handle
    /// to the flat C entry points. The returned pointer is the same
    /// pointer the Rust `Box::into_raw` produced; the caller must
    /// eventually pass it back through `torille_destroy`.
    void* raw_handle() const noexcept;

private:
    /// A `Box<TorilleHandle>` allocated in Rust. Held as a raw pointer
    /// here so this header does not have to drag in the cxx-generated
    /// `rust/cxx.h`; the .cc file owns the box release.
    void* handle_;
};

}  // namespace torille

// --- hunspell-shaped C entry points ----------------------------------------
//
// These four symbols are the entire public surface a C/C++ project has to
// learn to swap hunspell for torille. They take a `void*` handle the
// caller obtained from `torille_create`, exactly as `Hunhandle*` is
// obtained from `Hunspell_create`. `torille_free_list` is a no-op for
// symmetry with `Hunspell_free_list`; the `std::vector` the C++ wrapper
// returns owns its storage, so the C caller has nothing to free when it
// goes through the helper below.

extern "C" {

/// Construct a handle. Returns null on failure (currently impossible;
/// the embedded sanalista always parses).
void* torille_create();

/// True if `word` is a known Finnish word. 1 = yes, 0 = no.
int torille_spell(void* handle, const char* word);

/// Up to `max_n` suggestions, ranked by edit distance. Returns the number
/// of suggestions written into `out_suggestions`. The caller must allocate
/// `out_suggestions` with at least `max_n` slots; on return, slots
/// `[0, return)` are filled with malloc'd C strings the caller is
/// responsible for freeing (use `torille_free_list`, not `free`).
std::size_t torille_suggest(
    void* handle,
    const char* word,
    char** out_suggestions,
    std::size_t max_n
);

/// Free a list previously returned by `torille_suggest`. No-op when
/// called on a list not produced by this library, by design.
void torille_free_list(void* handle, char** suggestions, std::size_t n);

/// Drop a handle. After this call, the `void*` is invalid.
void torille_destroy(void* handle);

}  // extern "C"