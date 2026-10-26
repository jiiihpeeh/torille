//! The textarea + mirror overlay.
//!
//! Leptos version of `torille-wasm/demo/src/Editor.tsx`. The
//! SolidJS implementation overlays a `<textarea>` and a
//! coloured underlay inside a shadow DOM; the Leptos version
//! keeps that layout but expresses it in `view!` idioms.
//!
//! The misspellings list comes from the parent's signal. Each
//! keystroke re-runs the analyse pipeline (one Effect, one
//! torille call) and the signal updates atomically.
//!
//! For now this is a flat structure -- no shadow DOM, no font
//! mirroring. Visual fidelity (matching the textarea's box
//! exactly so the underlines align) is a follow-up.

use leptos::prelude::*;

#[component]
pub fn Editor(
    /// The textarea's current text. Read-only here; the
    /// `on:input` handler writes through `set_text`.
    text: ReadSignal<String>,
    /// Writer the input event uses to push the new value
    /// back into the parent's `text` signal.
    set_text: WriteSignal<String>,
    /// The misspellings the wasm currently reports, one entry
    /// per misspelt token. Reads drive the render.
    misspellings: ReadSignal<Vec<String>>,
) -> impl IntoView {
    let (focused, set_focused) = signal::<Option<String>>(None);

    // The minimum-viable render: every misspelling is a
    // `<span class="misspell">` element. Real production
    // would map wasm-returned byte ranges to spans inside
    // an underlay. For now, the focus popup is what the
    // user needs to see that the wasm side is wired up.
    view! {
        <div class="editor">
            <textarea
                class="textarea"
                spellcheck="false"
                placeholder="Type some Finnish prose..."
                prop:value=text
                on:input=move |e| {
                    let v = event_target_value(&e);
                    set_text.set(v);
                }
            />
            <div class="misspell-list">
                {move || {
                    let words: Vec<String> = misspellings.get();
                    if words.is_empty() {
                        view! { <span class="ok">"all spelled"</span> }.into_any()
                    } else {
                        // Take ownership so the closures capture
                        // owned Strings rather than borrowing the
                        // `words` local, which would otherwise be
                        // dropped before the view tree is built.
                        let owned: Vec<String> = words.into_iter().collect();
                        view! {
                            <ul>
                                {owned.into_iter().map(|w| {
                                    let w2 = w.clone();
                                    view! {
                                        <li>
                                            <button
                                                on:click=move |_| {
                                                    set_focused.set(Some(w2.clone()))
                                                }
                                            >{w}</button>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
            </div>
            <Show when=move || focused.get().is_some()>
                <crate::components::popup::SuggestionsPopup
                    word=focused
                    on_close=move || set_focused.set(None)
                />
            </Show>
        </div>
    }
}
