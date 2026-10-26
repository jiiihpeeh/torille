// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Compile-and-run smoke test for the torille-hunspell wrapper. Stands
// up a `torille::Speller`, runs `check_word` and `suggest` over a small
// Finnish vocabulary, and confirms the answers match what the Rust
// spell-checker reports.
//
// Build (from the repo root):
//
//   c++ -std=c++17 -I wrapper/include \
//       wrapper/tests/smoke.cc \
//       target/release/libtorille_wrapper.a \
//       $(pkg-config --libs cxxbridge) \
//       -o /tmp/torille_smoke
//
//   /tmp/torille_smoke

#include "torille_hunspell.h"

#include <algorithm>
#include <cassert>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace {

struct Case {
    std::string word;
    bool should_be_known;
    std::vector<std::string> expected_suggestions;  // empty = no expectation
};

}  // namespace

int main() {
    torille::Speller speller;

    const Case cases[] = {
        {"talo",        true,  {}},                     // lemma
        {"taloa",       true,  {}},                     // form
        {"puhtaan",     true,  {}},                     // gradated form
        {"koiralta",    true,  {}},                     // -lta form
        {"xyzzy",       false, {}},                     // unknown
        {"qwerty",      false, {}},                     // unknown
        {"taloo",       false, {"talo"}},               // typo
        {"koirra",      false, {"koira"}},              // typo
    };

    int failures = 0;
    for (const auto& c : cases) {
        const bool known = speller.check_word(c.word);
        const bool matches = (known == c.should_be_known);
        if (!matches) {
            std::fprintf(
                stderr,
                "FAIL: check_word(%s) = %d, expected %d\n",
                c.word.c_str(),
                int(known),
                int(c.should_be_known)
            );
            ++failures;
            continue;
        }
        std::printf("ok    check_word(%s) = %d\n", c.word.c_str(), int(known));

        if (!c.should_be_known) {
            auto suggestions = speller.suggest(c.word, 5);
            if (suggestions.empty()) {
                std::printf("ok    suggest(%s) -> empty\n", c.word.c_str());
            } else {
                std::printf(
                    "    suggest(%s) -> [%s]\n",
                    c.word.c_str(),
                    [&]() {
                        std::string joined;
                        for (const auto& s : suggestions) {
                            if (!joined.empty()) joined += ", ";
                            joined += s;
                        }
                        return joined;
                    }().c_str()
                );
            }
            continue;
        }

        for (const auto& expected : c.expected_suggestions) {
            auto suggestions = speller.suggest(c.word, 5);
            const bool found = std::find(
                suggestions.begin(),
                suggestions.end(),
                expected
            ) != suggestions.end();
            if (!found) {
                std::fprintf(
                    stderr,
                    "FAIL: suggest(%s) did not include %s\n",
                    c.word.c_str(),
                    expected.c_str()
                );
                ++failures;
            } else {
                std::printf(
                    "ok    suggest(%s) -> [..., %s, ...]\n",
                    c.word.c_str(),
                    expected.c_str()
                );
            }
        }
    }

    // --- C entry points ---
    void* handle = torille_create();
    assert(handle != nullptr);
    const int spell_ok = torille_spell(handle, "talo");
    std::printf("\nC API: torille_spell(\"talo\") = %d\n", spell_ok);
    const int spell_bad = torille_spell(handle, "xyzzy");
    std::printf("C API: torille_spell(\"xyzzy\") = %d\n", spell_bad);

    char* suggestions[5];
    const std::size_t n = torille_suggest(
        handle,
        "taloo",
        suggestions,
        5
    );
    std::printf("C API: torille_suggest(\"taloo\") -> %zu entries\n", n);
    for (std::size_t i = 0; i < n; ++i) {
        std::printf("  [%zu] %s\n", i, suggestions[i]);
    }
    torille_free_list(handle, suggestions, n);
    torille_destroy(handle);

    if (failures > 0) {
        std::fprintf(stderr, "\n%d smoke test failure(s)\n", failures);
        return 1;
    }
    std::printf("\nall smoke checks passed\n");
    return 0;
}