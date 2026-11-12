//! Grammar diagnostics panel for the Leptos demo.
//!
//! The panel is deliberately on demand: grammar checking is not part of the
//! keystroke path. A button runs [`torille::syntax_check::check`], lists the
//! diagnostics in the checker's own rank order, and applies a correction by
//! replaying its edits from the end of the text backwards.

use leptos::prelude::*;
use torille::syntax_check::{self, Correction, DiagnosticKind, Severity, SyntaxDiagnostic};

#[component]
pub fn SyntaxPanel(
    /// The textarea's current text.
    text: ReadSignal<String>,
    /// Writer the correction buttons use to replace the text.
    set_text: WriteSignal<String>,
) -> impl IntoView {
    let (diagnostics, set_diagnostics) = signal(Vec::<SyntaxDiagnostic>::new());
    let (checked, set_checked) = signal(false);

    let run_check = move || {
        let current = text.get_untracked();
        set_diagnostics.set(syntax_check::check(&current).diagnostics);
        set_checked.set(true);
    };

    view! {
        <section class="syntax-panel">
            <h2>"Kielioppi"</h2>
            <p class="hint">
                "Tarkista teksti erikseen. Tämä ei ole jokaisen näppäilyksen polulla."
            </p>
            <button type="button" class="syntax-check" on:click=move |_| run_check()>
                "Tarkista kielioppi"
            </button>
            <Show when=move || checked.get()>
                {move || {
                    let items = diagnostics.get();
                    if items.is_empty() {
                        view! { <p class="ok">"Ei huomautuksia."</p> }.into_any()
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
    diagnostic: SyntaxDiagnostic,
    text: ReadSignal<String>,
    set_text: WriteSignal<String>,
    set_diagnostics: WriteSignal<Vec<SyntaxDiagnostic>>,
    set_checked: WriteSignal<bool>,
) -> impl IntoView {
    let kind = diagnostic.kind;
    let severity = diagnostic.severity;
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
                                    .set(syntax_check::check(&updated).diagnostics);
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
        <li class=format!("syntax-diag {}", severity_class(severity))>
            <div class="syntax-head">
                <strong>{kind_label(kind)}</strong>
                <span class="syntax-severity">{severity_label(severity)}</span>
                <code>{kind.name()}</code>
            </div>
            {expected_view}
            {correction_view}
        </li>
    }
}

/// Apply a correction's edits without shifting the later offsets.
fn apply_correction(text: &str, edits: &[torille::syntax_check::Edit]) -> String {
    let mut updated = text.to_string();
    let mut ordered: Vec<&torille::syntax_check::Edit> = edits.iter().collect();
    ordered.sort_by(|a, b| b.start.cmp(&a.start).then_with(|| b.end.cmp(&a.end)));
    for edit in ordered {
        if edit.start > edit.end
            || edit.end > updated.len()
            || !updated.is_char_boundary(edit.start)
            || !updated.is_char_boundary(edit.end)
        {
            continue;
        }
        updated.replace_range(edit.start..edit.end, &edit.replacement);
    }
    updated
}

fn correction_label(correction: &Correction) -> String {
    match correction.edits.first() {
        Some(edit) if edit.replacement.is_empty() => "poista".to_string(),
        Some(edit) => edit.replacement.clone(),
        None => "korjaa".to_string(),
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "virhe",
        Severity::Warning => "varoitus",
        Severity::Suggestion => "ehdotus",
    }
}

fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "syntax-error",
        Severity::Warning => "syntax-warning",
        Severity::Suggestion => "syntax-suggestion",
    }
}

/// Finnish UI labels for the stable diagnostic kinds.
fn kind_label(kind: DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::SubjectPredicateAgreement => "subjektin ja predikaatin kongruenssi",
        DiagnosticKind::AttributeAgreement => "attribuutin kongruenssi",
        DiagnosticKind::VerbRection => "verbien rektio",
        DiagnosticKind::CaseSelection => "sijan valinta",
        DiagnosticKind::ObjectCase => "objektin sija",
        DiagnosticKind::MissingArgument => "puuttuva argumentti",
        DiagnosticKind::ExtraArgument => "ylimääräinen argumentti",
        DiagnosticKind::ClauseStructure => "lauserakenne",
        DiagnosticKind::WordOrder => "sanajärjestys",
        DiagnosticKind::CommaMissing => "puuttuva pilkku",
        DiagnosticKind::CommaExtra => "ylimääräinen pilkku",
        DiagnosticKind::CommaPairUnbalanced => "pilkupari on epätasapainossa",
        DiagnosticKind::QuestionMarkMissing => "puuttuva kysymysmerkki",
        DiagnosticKind::UnbalancedQuote => "epätasapainoinen lainausmerkki",
        DiagnosticKind::ListSeparatorMissing => "puuttuva luettelopilkku",
        DiagnosticKind::ListSeparatorDuplicate => "kaksinkertainen luettelopilkku",
        DiagnosticKind::ListCommaBeforeConjunction => "pilkku ennen konjunktiota",
        DiagnosticKind::ListIntroducerComma => "pilkku kaksoispisteen jälkeen",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_a_single_word_replacement() {
        let check = syntax_check::check("Koira nukkuvat.");
        let correction = &check.diagnostics[0].corrections[0];
        let updated = apply_correction("Koira nukkuvat.", &correction.edits);
        assert_eq!(updated, "Koira nukkuu.");
    }

    #[test]
    fn applies_a_pair_of_insertions_from_the_end() {
        let check = syntax_check::check("Omena esimerkiksi on hyvä.");
        let correction = &check.diagnostics[0].corrections[0];
        let updated = apply_correction("Omena esimerkiksi on hyvä.", &correction.edits);
        assert_eq!(updated, "Omena, esimerkiksi, on hyvä.");
    }

    #[test]
    fn applies_a_deletion() {
        let check = syntax_check::check("Ostin omenoita, appelsiineja,, ja banaaneja.");
        let correction = &check.diagnostics[0].corrections[0];
        let updated = apply_correction(
            "Ostin omenoita, appelsiineja,, ja banaaneja.",
            &correction.edits,
        );
        assert_eq!(updated, "Ostin omenoita, appelsiineja, ja banaaneja.");
    }
}
