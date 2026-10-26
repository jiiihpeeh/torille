//! The suggestion popup.
//!
//! Minimum-viable: shows "ei ehdotuksia" (no suggestions) for
//! every misspelling the popup is asked to render. The real
//! implementation reads the wasm-side `suggest_words_with_state`
//! result that accompanied the analyse call, so the popup
//! doesn't issue a second wasm call -- just like the
//! `lastHits` cache in the SolidJS demo / extension content.
//!
//! Wiring the suggest data through is a follow-up; this file
//! exists so the layout shows up in the demo's first frame.

use leptos::prelude::*;

#[component]
pub fn SuggestionsPopup(
    /// The misspelt word the popup is rendering suggestions
    /// for.
    #[prop(into)] word: Signal<Option<String>>,
    /// Callback the parent uses to close the popup.
    on_close: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <div class="popup">
            {move || match word.get() {
                Some(w) => view! { <span class="word">{w}</span> }.into_any(),
                None => ().into_any(),
            }}
            <button type="button" on:click=move |_| on_close()>
                "sulje"
            </button>
        </div>
    }
}
