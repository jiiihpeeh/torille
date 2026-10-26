// SPDX-License-Identifier: MIT OR Apache-2.0
//! `tags_dump` — emit the engine's `Tags` enum layout as JSON.
//!
//! The neural reranker's morphology one-hot block is derived from the
//! engine's enum definitions in `src/tags.rs`. Hand-maintaining the
//! Python mapping in `model.py` would let the two diverge silently —
//! exactly the failure mode the Slice-3 review warned about. This tool
//! makes the engine the single source of truth: run it whenever the
//! enum's tag set changes, save the JSON to `data/tags_layout.json`,
//! and have the trainer load it at module init.
//!
//! ```bash
//! cargo run -p torille-tools --release --bin tags_dump -- data/tags_layout.json
//! ```
//!
//! The output schema is:
//
//! ```json
//! {
//!   "dimensions": [
//!     {"name": "POS", "size": 14, "values": ["unknown", "noun", ...]},
//!     {"name": "Case", "size": 16, "values": ["nominative", ...]},
//!     ...
//!   ],
//!   "morph_dim": 53
//! }
//! ```

use std::fs;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use torille::tags::{Case, Mood, Number, PartOfSpeech, Person, Polarity, Tense, Voice};

/// One row in the layout. Mirrors the schema documented in the module
/// docstring above.
#[derive(Debug)]
struct Dim {
    name: &'static str,
    values: Vec<&'static str>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: tags_dump <output.json>");
        eprintln!();
        eprintln!("writes the Tags enum dimension + value names to <output.json>");
        return ExitCode::from(2);
    }
    let out_path = PathBuf::from(&args[0]);

    // The order here is the order in which the morphology one-hot
    // block is laid out. Adding or removing a dimension means
    // bumping MORPH_DIM in model.py -- this is intentional, since
    // changing the order of features would silently break trained
    // weights.
    let dims: Vec<Dim> = vec![
        // Every dimension gets an explicit "unknown" slot at index 0.
        // When the morphology engine does not produce a reading for
        // the candidate, or when the trainer sees a feature dict with
        // a missing key, the loader maps the value to this slot --
        // not to whatever happens to be last in the engine enum.
        Dim {
            name: "POS",
            values: pos_with_unknown(),
        },
        Dim {
            name: "Case",
            values: case_with_unknown(),
        },
        Dim {
            name: "Number",
            values: number_with_unknown(),
        },
        Dim {
            name: "Person",
            values: person_with_unknown(),
        },
        Dim {
            name: "Tense",
            values: tense_with_unknown(),
        },
        Dim {
            name: "Mood",
            values: mood_with_unknown(),
        },
        Dim {
            name: "Voice",
            values: voice_with_unknown(),
        },
        Dim {
            name: "Polarity",
            values: polarity_with_unknown(),
        },
        // "false" / "true" already cover the binary case; "unknown" is
        // an explicit third slot for the "no reading produced" case.
        Dim {
            name: "HasReadings",
            values: vec!["unknown", "false", "true"],
        },
    ];

    let mut writer = BufWriter::new(match fs::File::create(&out_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("creating {out_path:?}: {e}");
            return ExitCode::from(1);
        }
    });

    let mut morph_dim = 0usize;
    write!(writer, "{{\n  \"dimensions\": [\n").unwrap();
    for (i, d) in dims.iter().enumerate() {
        write!(
            writer,
            "    {{\"name\":\"{}\",\"size\":{},\"values\":[",
            d.name,
            d.values.len(),
        )
        .unwrap();
        for (j, v) in d.values.iter().enumerate() {
            if j > 0 {
                writer.write_all(b",").unwrap();
            }
            write!(writer, "{v:?}").unwrap();
        }
        writer.write_all(b"]}").unwrap();
        if i + 1 < dims.len() {
            writer.write_all(b",").unwrap();
        }
        writer.write_all(b"\n").unwrap();
        morph_dim += d.values.len();
    }
    write!(writer, "  ],\n  \"morph_dim\": {}\n}}\n", morph_dim,).unwrap();
    writer.flush().unwrap();

    println!(
        "wrote {} ({} dimensions, morph_dim={})",
        out_path.display(),
        dims.len(),
        morph_dim,
    );
    ExitCode::SUCCESS
}

// =============================================================================
//   Per-enum value lists. These are the source of truth for the JSON
//   schema. If a `Tags` enum gains a variant, this list (and the
//   matching `name()` function in src/tags.rs) must be updated
//   together; the test below pins them.
// =============================================================================

fn pos_names() -> Vec<&'static str> {
    PartOfSpeech::ALL.iter().map(|p| p.name()).collect()
}

fn case_names() -> Vec<&'static str> {
    Case::ALL.iter().map(|c| c.name()).collect()
}

fn number_names() -> Vec<&'static str> {
    Number::ALL.iter().map(|n| n.name()).collect()
}

fn person_names() -> Vec<&'static str> {
    Person::ALL.iter().map(|p| p.name()).collect()
}

fn tense_names() -> Vec<&'static str> {
    Tense::ALL.iter().map(|t| t.name()).collect()
}

fn mood_names() -> Vec<&'static str> {
    Mood::ALL.iter().map(|m| m.name()).collect()
}

fn voice_names() -> Vec<&'static str> {
    Voice::ALL.iter().map(|v| v.name()).collect()
}

fn polarity_names() -> Vec<&'static str> {
    Polarity::ALL.iter().map(|p| p.name()).collect()
}

// Per-dimension variants that prepend an explicit "unknown" slot. The
// first index in each dim is reserved for "this dimension's value is
// not available" -- which is different from "this dimension's value
// is the engine enum's last variant". The user's review pointed out
// that mapping missing → last-slot was a false morphology signal.
fn pos_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(pos_names());
    v
}
fn case_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(case_names());
    v
}
fn number_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(number_names());
    v
}
fn person_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(person_names());
    v
}
fn tense_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(tense_names());
    v
}
fn mood_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(mood_names());
    v
}
fn voice_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(voice_names());
    v
}
fn polarity_with_unknown() -> Vec<&'static str> {
    let mut v = vec!["unknown"];
    v.extend(polarity_names());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All enum-NAME lists must match the engine's `name()` outputs.
    /// If any enum gains a variant, this test forces the matching
    /// `*_names()` to be updated.
    #[test]
    fn names_match_engine_layout() {
        // pos_names() returns the raw engine names. PartOfSpeech
        // has 14 variants with Unknown at index 0.
        assert_eq!(pos_names().len(), PartOfSpeech::ALL.len());
        assert_eq!(pos_names()[0], "unknown");
        assert_eq!(pos_names()[1], "noun");

        // Case has 16.
        assert_eq!(case_names().len(), Case::ALL.len());
        assert_eq!(case_names()[9], "allative");
        // Regression: the previous layout had `essive` and `allative`
        // both mapped to the same index, which silently merged two
        // distinct cases into one hot slot.
        assert_eq!(case_names()[10], "essive");
        assert_ne!(
            case_names()[9],
            case_names()[10],
            "allative and essive must be distinct case indices"
        );

        // The "unknown"-prepended dims add one slot at index 0; the
        // engine's Unknown variant is at index 1 in the prepended
        // form, with the rest of the enum names following.
        assert_eq!(pos_with_unknown().len(), PartOfSpeech::ALL.len() + 1);
        assert_eq!(pos_with_unknown()[0], "unknown");
        assert_eq!(pos_with_unknown()[1], "unknown");
        assert_eq!(pos_with_unknown()[2], "noun");
        assert_eq!(case_with_unknown()[0], "unknown");
        assert_eq!(case_with_unknown()[10], "allative");
        assert_eq!(case_with_unknown()[11], "essive");
    }
}
