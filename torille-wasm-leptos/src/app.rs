//! The Leptos `App` component.
//!
//! One per-textarea state instance, owned by the App. Two
//! signals (`text` for the input + `misspellings` for the result
//! of `analyze`). One `create_effect` bridges the two: it reads
//! `text`, runs `torille_wasm::wasm_demo::analyze`, and stores
//! the hits back into `misspellings`.
//!
//! The render is immediate on every keystroke (Leptos's
//! `create_effect` is synchronous in CSR mode). The 150 ms
//! debounce the SolidJS demo uses to coalesce IPC round-trips
//! isn't useful here -- `analyze` runs in the same wasm and
//! finishes in microseconds for typical Finnish text.

use leptos::prelude::*;

use crate::components::editor::Editor;
use crate::components::style::StylePanel;
use crate::components::syntax::SyntaxPanel;

/// Default text. Re-used from `torille-wasm/demo` so the demo
/// both ways shows the same misspellings (taloo, koirra,
/// xyzzy), which makes smoke-testing trivial.
const DEFAULT_TEXT: &str = "talo taloo koirra puhtaan öihin auto xyzzy\n\n\
    Kissa istuu talossa. Koira juoksee puistossa. Lintu laulaa puussa.";

#[component]
pub fn App() -> impl IntoView {
    // The per-textarea state lives at this scope. `analyze` is
    // an immutable borrow, so the surrounding type is fine
    // without explicit interior mutability; `update` requires
    // `&mut`, which the effect triggers through a separate
    // signal-driven flow below.
    //
    // We split ownership: `signal_rw` is the writer-wrapped
    // `TextAreaState` for `update`, and `signal_ro` is the reader
    // for `analyze`. Leptos signals satisfy both ends.
    let state_for_update = RwSignal::new(torille_wasm::wasm_demo::new_state());
    let state_for_analyze = state_for_update;

    let (text, set_text) = signal(DEFAULT_TEXT.to_string());
    let (misspellings, set_misspellings) = signal(Vec::new());
    // A small live probe for the hyphenation layer. It is separate from
    // the spell-check text on purpose: hyphenation is a typographic
    // presentation concern and must not change `check`/`suggest`.
    let (hyphen_word, set_hyphen_word) = signal("tietokoneohjelmointi".to_string());

    // Reactive: whenever `text` changes, run the analyse
    // pipeline and write the result into `misspellings`. No
    // debounce -- the wasm call is microseconds for typical
    // inputs.
    Effect::new(move |_| {
        let t = text.get();
        // Update the state inline so the per-textarea proper-noun
        // set mirrors what the renderer just saw.
        state_for_update.update(|s| {
            torille_wasm::wasm_demo::update(s, &t);
        });
        let hits = state_for_analyze.get_untracked().analyze(&t, 8);
        let misspell_words: Vec<String> = hits.iter().map(|h| h.word.clone()).collect();
        set_misspellings.set(misspell_words);
    });

    view! {
        <h1>
            "torille-wasm-leptos demo "
            <small>
                {move || format!("({} misspelling{})",
                    misspellings.get().len(),
                    if misspellings.get().len() == 1 { "" } else { "s" })}
            </small>
        </h1>
        <p>
            "Click a red word to see suggestions; arrow keys + Enter to pick."
        </p>
        <div class="status">
            <span>{move || format!("{} words", count_words(&text.get()))}</span>
            {move || {
                let count = misspellings.get().len();
                if count == 0 {
                    view! { <span class="ok">"all spelled"</span> }.into_any()
                } else {
                    view! { <span class="bad">{format!("{count} misspellings")}</span> }.into_any()
                }
            }}
        </div>
        <Editor
            text=text
            set_text=set_text
            misspellings=misspellings
        />
        <SyntaxPanel text=text set_text=set_text />
        <StylePanel text=text set_text=set_text />
        <section class="hyphenation">
            <h2>"Hyphenation"</h2>
            <label for="hyphen-word">"Word"</label>
            <input
                id="hyphen-word"
                type="text"
                spellcheck="false"
                prop:value=hyphen_word
                on:input=move |e| set_hyphen_word.set(event_target_value(&e))
            />
            <p class="hyphenated">
                {move || torille_wasm::wasm_demo::hyphenate(&hyphen_word.get())}
            </p>
            <p class="hint">
                "Permitted line breaks under the default Finnish policy."
            </p>
        </section>
        <details>
            <summary>"How it works"</summary>
            <pre>{unindent("
                // The wasm is a single bundle: UI and spell-checker
                // compile into the same linear memory.
                //   * `set_text` writes a signal.
                //   * `Effect::new` re-runs on every keystroke.
                //   * `torille_wasm::wasm_demo::analyze` is a
                //     normal Rust call -- no wasm-bindgen
                //     envelope, no postMessage, no worker.
                //   * The hits flow into `misspellings`, which the
                //     editor mirrors on the underlay.
            ")}</pre>
        </details>
    }
}

/// Count word-like runs in `text`. Same heuristic as the
/// SolidJS demo (Unicode letters, hyphens stay inside words).
fn count_words(text: &str) -> usize {
    let mut count = 0;
    let mut in_word = false;
    for ch in text.chars() {
        let is_word = ch.is_alphabetic() || ch == '-';
        match (is_word, &mut in_word) {
            (true, false) => {
                in_word = true;
                count += 1;
            }
            (false, true) => in_word = false,
            _ => {}
        }
    }
    count
}

/// Strip a leading common indent from every non-empty line of
/// `s` so the literal in the docs block stays legible in the
/// editor view.
fn unindent(s: &str) -> String {
    let leading = s
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.chars().take_while(|c| *c == ' ').count())
        .min()
        .unwrap_or(0);
    s.lines()
        .map(|l| if l.len() >= leading { &l[leading..] } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}
