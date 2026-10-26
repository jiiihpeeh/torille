// SPDX-License-Identifier: MIT OR Apache-2.0
//
// C++ implementation of the `torille` namespace and the four
// `extern "C"` entry points declared in `include/torille_hunspell.h`.
//
// The cxx-generated header (`src/lib.rs.h`) gives us
// `torille_wrapper::ffi::create`, `check_word`, `suggest_words`, and
// `free_suggestions` as C++ functions that call the Rust side directly.
// We do not expose those names -- the public API is the `torille::Speller`
// class and the four `torille_*` extern-C symbols, both of which are
// thin shells over the FFI.

#include "torille_hunspell.h"

#include <algorithm>
#include <cstring>

// cxx-generated header. Built by build.rs into
// `target/.../cxxbridge/include/wrapper/src/lib.rs.h`; cxx-build adds
// `cxxbridge/include` to the include path, so the qualified path below
// is what `#include "lib.rs.h"` resolves to.
#include "wrapper/src/lib.rs.h"

#include <vector>

namespace torille {

namespace {
// Convert a `rust::Vec<rust::String>` (the type the cxx bridge gives back
// from `suggest_words`) into a `std::vector<std::string>` for the public
// API. `rust::String` has an explicit `operator std::string()` for the
// per-element conversion, so we just walk the slice.
std::vector<std::string> to_std_vec(rust::Vec<rust::String> v) {
    std::vector<std::string> out;
    out.reserve(v.size());
    for (std::size_t i = 0; i < v.size(); ++i) {
        out.emplace_back(static_cast<std::string>(v[i]));
    }
    return out;
}
}  // namespace

Speller::Speller()
    : handle_(create().into_raw()) {}

Speller::~Speller() {
    if (handle_) {
        // `rust::Box::from_raw` reconstructs the box so its destructor
        // runs the Rust `Drop` for `Box<TorilleHandle>`.
        rust::Box<TorilleHandle>::from_raw(
            reinterpret_cast<TorilleHandle*>(handle_)
        );
    }
}

void* Speller::raw_handle() const noexcept { return handle_; }

bool Speller::check_word(const std::string& word) const {
    if (!handle_) {
        return false;
    }
    auto* h = reinterpret_cast<TorilleHandle*>(handle_);
    return ::check_word(*h, word);
}

std::vector<std::string>
Speller::suggest(const std::string& word, std::size_t max_n) const {
    if (!handle_) {
        return {};
    }
    auto* h = reinterpret_cast<TorilleHandle*>(handle_);
    return to_std_vec(::suggest_words(*h, word, max_n));
}

}  // namespace torille

// The C entry points.
extern "C" {

void* torille_create() {
    return create().into_raw();
}

int torille_spell(void* handle, const char* word) {
    if (!handle || !word) {
        return 0;
    }
    auto* h = reinterpret_cast<TorilleHandle*>(handle);
    return ::check_word(*h, std::string(word)) ? 1 : 0;
}

std::size_t torille_suggest(
    void* handle,
    const char* word,
    char** out_suggestions,
    std::size_t max_n
) {
    if (!handle || !word || !out_suggestions || max_n == 0) {
        return 0;
    }
    auto* h = reinterpret_cast<TorilleHandle*>(handle);
    auto suggestions = ::suggest_words(*h, std::string(word), max_n);
    std::size_t n = std::min(suggestions.size(), max_n);
    for (std::size_t i = 0; i < n; ++i) {
        // strdup uses malloc, which `free` can release -- this matches
        // the hunspell contract that the caller can call `free` on each
        // entry, and `torille_free_list` does exactly that.
        out_suggestions[i] = ::strdup(static_cast<std::string>(suggestions[i]).c_str());
    }
    return n;
}

void torille_free_list(void* /*handle*/, char** suggestions, std::size_t n) {
    if (!suggestions) {
        return;
    }
    for (std::size_t i = 0; i < n; ++i) {
        if (suggestions[i]) {
            ::free(suggestions[i]);
            suggestions[i] = nullptr;
        }
    }
}

void torille_destroy(void* handle) {
    if (!handle) {
        return;
    }
    auto* h = reinterpret_cast<TorilleHandle*>(handle);
    rust::Box<TorilleHandle>::from_raw(h);
}

}  // extern "C"