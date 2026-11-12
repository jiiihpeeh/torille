//! Print `syntax::analyze` for each sentence, for auditing role output.
//!
//! ```text
//! cargo run --release -p torille --example syntax_probe
//! cargo run --release -p torille --example syntax_probe -- "Sami söi puuron."
//! SYNTAX_READINGS=1 cargo run --release -p torille --example syntax_probe
//! ```
//!
//! With no arguments it reads `syntax_probe_sentences.txt` beside this file.
//! Set `SYNTAX_READINGS=1` to print every reading behind a token.

use torille::syntax::{self, RoleConfidence};
use torille::tokenise::tokenise;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sentences: Vec<String> = if args.is_empty() {
        include_str!("syntax_probe_sentences.txt")
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .map(|line| line.to_string())
            .collect()
    } else {
        args
    };

    for text in sentences {
        println!("\n=== {text}");
        let tokens = tokenise(&text);
        let analysis = syntax::analyze(&tokens);
        for (i, clause) in analysis.clauses.iter().enumerate() {
            let pred = clause
                .predicate
                .map(|p| tokens[p].text.as_str())
                .unwrap_or("-");
            let subj = clause
                .subject
                .map(|s| tokens[s].text.as_str())
                .unwrap_or("-");
            let obj = clause
                .object
                .map(|o| tokens[o].text.as_str())
                .unwrap_or("-");
            println!(
                "  clause {i}: {:?} pred={pred} subj={subj} obj={obj} neg={} exist={}",
                clause.tokens, clause.negative, clause.existential
            );
        }
        for (i, token) in tokens.iter().enumerate() {
            if !token.is_word() {
                continue;
            }
            let s = &analysis.tokens[i];
            println!(
                "    {:<16} {:<10} {:<8} case={:<14} head={:<4} lemma={}",
                token.text,
                s.role.name(),
                match s.confidence {
                    RoleConfidence::Likely => "likely",
                    RoleConfidence::Weak => "weak",
                },
                s.case.map(|c| c.name()).unwrap_or("-"),
                s.head.map(|h| h.to_string()).unwrap_or_else(|| "-".into()),
                s.lemma.as_deref().unwrap_or("-"),
            );
            if std::env::var_os("SYNTAX_READINGS").is_some() && !s.readings.is_empty() {
                let mut parts = Vec::new();
                for r in &s.readings {
                    let mut part = format!(
                        "{:?}/{:?}/{:?}/{:?}/{:?}",
                        r.part_of_speech, r.case, r.number, r.mood, r.voice
                    );
                    if r.polarity != torille::tags::Polarity::Affirmative {
                        part.push_str(&format!("/{}", r.polarity.name()));
                    }
                    parts.push(part);
                }
                println!("        readings: {}", parts.join(" | "));
            }
        }
    }
}
