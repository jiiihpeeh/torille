//! Optional style diagnostics panel for the Leptos demo.
//!
//! This is the separate, suggestion-only style layer. It never shows a
//! grammar error, and it is deliberately a different button from
//! `Tarkista kielioppi`: the user asks for style advice explicitly.

use leptos::prelude::*;
use torille::style_check::{self, StyleDiagnostic, StyleKind};
use torille::syntax_check::Severity;

use super::syntax::apply_correction;

#[component]
pub fn StylePanel(
    /// The textarea's current text.
    text: ReadSignal<String>,
    /// Writer the correction buttons use to replace the text.
    set_text: WriteSignal<String>,
) -> impl IntoView {
    let (diagnostics, set_diagnostics) = signal(Vec::<StyleDiagnostic>::new());
    let (checked, set_checked) = signal(false);

    let run_check = move || {
        let current = text.get_untracked();
        set_diagnostics.set(style_check::check(&current).diagnostics);
        set_checked.set(true);
    };

    view! {
        <section class="syntax-panel">
            <h2>"Tyyli"</h2>
            <p class="hint">
                "Valinnainen tyylitarkistus. Ehdotukset eivät ole virheitä."
            </p>
            <button type="button" class="syntax-check" on:click=move |_| run_check()>
                "Tarkista tyyli"
            </button>
            <Show when=move || checked.get()>
                {move || {
                    let items = diagnostics.get();
                    if items.is_empty() {
                        view! { <p class="ok">"Ei tyylihuomautuksia."</p> }.into_any()
                    } else {
                        view! {
                            <ul class="syntax-list">
                                {items.into_iter().map(|diagnostic| {
                                    diagnostic_row(
                                        diagnostic,
                                        text,
                                        set_text,
                                        set_diagnostics,
                                        set_checked,
                                    )
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
            </Show>
        </section>
    }
}

fn diagnostic_row(
    diagnostic: StyleDiagnostic,
    text: ReadSignal<String>,
    set_text: WriteSignal<String>,
    set_diagnostics: WriteSignal<Vec<StyleDiagnostic>>,
    set_checked: WriteSignal<bool>,
) -> impl IntoView {
    let kind = diagnostic.kind;
    let expected = diagnostic.expected.clone();
    let corrections = diagnostic.corrections.clone();

    let expected_view = move || match expected.clone() {
        Some(value) => view! { <p class="syntax-expected">"odotettu: " {value}</p> }.into_any(),
        None => ().into_any(),
    };

    let correction_view = if corrections.is_empty() {
        ().into_any()
    } else {
        view! {
            <div class="syntax-corrections">
                {corrections.into_iter().map(|correction| {
                    let label = correction_label(&correction);
                    view! {
                        <button
                            type="button"
                            on:click=move |_| {
                                let current = text.get_untracked();
                                let updated = apply_correction(&current, &correction.edits);
                                set_text.set(updated.clone());
                                set_diagnostics
                                    .set(style_check::check(&updated).diagnostics);
                                set_checked.set(true);
                            }
                        >{label}</button>
                    }
                }).collect_view()}
            </div>
        }
        .into_any()
    };

    view! {
        <li class="syntax-diag syntax-suggestion">
            <div class="syntax-head">
                <strong>{kind_label(kind)}</strong>
                <span class="syntax-severity">{severity_label()}</span>
                <code>{kind.name()}</code>
            </div>
            {expected_view}
            {correction_view}
        </li>
    }
}

fn correction_label(correction: &torille::syntax_check::Correction) -> String {
    match correction.edits.first() {
        Some(edit) if edit.replacement.is_empty() => "poista".to_string(),
        Some(edit) => edit.replacement.clone(),
        None => "korjaa".to_string(),
    }
}

fn severity_label() -> &'static str {
    // Style findings are always suggestions; the function keeps the label in
    // one place if the UI later distinguishes a hint.
    match Severity::Suggestion {
        Severity::Error => "virhe",
        Severity::Warning => "varoitus",
        Severity::Suggestion => "ehdotus",
    }
}

/// Finnish UI labels for the stable style kinds.
fn kind_label(kind: StyleKind) -> &'static str {
    match kind {
        StyleKind::Repetition => "toistuva sana",
        StyleKind::LongSentence => "pitkä virke",
        StyleKind::Cliche => "kulunut ilmaus",
        StyleKind::Filler => "täytesana",
        StyleKind::Wordy => "monisanainen ilmaus",
        StyleKind::Colloquial => "arkikielinen ilmaus",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_labels_are_finnish() {
        assert_eq!(kind_label(StyleKind::Repetition), "toistuva sana");
        assert_eq!(kind_label(StyleKind::LongSentence), "pitkä virke");
        assert_eq!(kind_label(StyleKind::Cliche), "kulunut ilmaus");
        assert_eq!(kind_label(StyleKind::Filler), "täytesana");
        assert_eq!(kind_label(StyleKind::Wordy), "monisanainen ilmaus");
        assert_eq!(kind_label(StyleKind::Colloquial), "arkikielinen ilmaus");
    }

    #[test]
    fn a_style_correction_applies_from_the_end() {
        let check = style_check::check("Tällä hetkellä tilanne on rauhallinen.");
        let correction = &check.diagnostics[0].corrections[0];
        let updated = apply_correction(
            "Tällä hetkellä tilanne on rauhallinen.",
            &correction.edits,
        );
        assert_eq!(updated, "Nyt tilanne on rauhallinen.");
    }
}
