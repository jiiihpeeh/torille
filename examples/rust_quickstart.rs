//! A tour of the `torille` crate as a Rust library.
//!
//! The engine is a normal library: the CLI tools, the Python extension and the
//! wasm all call the same functions. This example is a small end-to-end tour.
//!
//! Run it with:
//!
//!     cargo run --example rust_quickstart

use torille::expand::inflect_word;
use torille::syntax_check;
use torille::tags::{Confidence, Form};
use torille::{CheckOutcome, check, hyphenation, suggest};

fn main() {
    println!("check — is the word in the lexicon?");
    for word in ["talo", "talossa", "koirra"] {
        let answer = match check(word) {
            CheckOutcome::Lemma => "lemma".to_string(),
            CheckOutcome::Inflected(bases) => format!("form of {}", bases.join(", ")),
            CheckOutcome::Unknown => "unknown".to_string(),
        };
        println!("  {word:<10} {answer}");
    }

    println!("\nsuggest — likely corrections, best first");
    for word in ["koirra", "kisssa"] {
        println!("  {word:<10} {}", suggest(word, 3, 2).join(", "));
    }

    println!("\ninflect — every form of a known lemma");
    let forms = inflect_word("talo");
    println!("  {} forms; first plain ones:", forms.len());
    for form in forms
        .iter()
        .filter(|form| form.confidence == Confidence::Plain)
        .take(6)
    {
        println!("  {:<12} {}", form.word, reading(form));
    }

    println!("\nhyphenation — permitted line breaks");
    let word = "tietokoneohjelmointi";
    let points = hyphenation::positions(word);
    println!("  {word} -> {}", hyphenation::split(word, &points).join(" - "));

    println!("\nsyntax — grammar diagnostics over a sentence");
    let checked = syntax_check::check("Koira nukkuvat.");
    for diagnostic in &checked.diagnostics {
        println!("  {}: {}", diagnostic.rule_id(), diagnostic.message);
    }
}

/// The parts of the agreed reading that the example prints.
fn reading(form: &Form) -> String {
    let tags = form.union();
    [
        tags.part_of_speech.map(|pos| pos.name().to_string()),
        tags.case.map(|case| case.name().to_string()),
        tags.number.map(|number| number.name().to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
}
