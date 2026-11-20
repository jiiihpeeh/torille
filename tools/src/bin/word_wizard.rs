// SPDX-License-Identifier: MIT OR Apache-2.0
//! `word_wizard` — find a word's Kotus class from a few example forms.
//!
//! Interactive:
//!   word_wizard
//!   word_wizard mökki
//!
//! Batch:
//!   word_wizard mökki --pos substantiivi --form gen_sg=mökin --form par_sg=mökkiä
//!   word_wizard Helsinki --pos erisnimi --form gen_sg=Helsingin
//!   word_wizard mökki --answers answers.tsv --json
//!   word_wizard mökki --answers answers.tsv --emit sanalista
//!
//! The sheet:
//!   word_wizard --write-sheet data/class_examples.tsv
//!   word_wizard --check-sheet
//!
//! The class sheet is generated from the catalogue; `--check-sheet` is the
//! drift gate. The inference asks `expand::inflect_in` what every candidate
//! class would generate, so no ending is ever appended here.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use serde_json::json;
use torille::class_wizard::{
    self, AnswerStatus, Answers, Cell, Report, SHEET, Sheet, check_sheet, infer_with_number,
    parse_answers, parse_sheet, pos_from_name, render_sheet,
};
use torille::expand::{LexiconIndex, inflect_in};
use torille::kotus::{self, Inflection};
use torille::spec::Catalogues;
use torille::tags::{Number, PartOfSpeech};
use torille_tools::catalogue;

fn main() -> ExitCode {
    let mut word = None;
    let mut pos_name = None;
    let mut forms: Vec<String> = Vec::new();
    let mut answers_file = None;
    let mut emit = None;
    let mut write_sheet = None;
    let mut check = false;
    let mut json = false;
    let mut indeclinable = false;
    let mut number: Option<Number> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pos" => pos_name = args.next(),
            "--form" => forms.push(args.next().unwrap_or_default()),
            "--answers" => answers_file = args.next(),
            "--emit" => emit = args.next(),
            "--write-sheet" => write_sheet = Some(args.next().unwrap_or_default()),
            "--check-sheet" => check = true,
            "--json" => json = true,
            "--indeclinable" => indeclinable = true,
            "--plural-only" => number = Some(Number::Plural),
            "--singular-only" => number = Some(Number::Singular),
            "-h" | "--help" => {
                usage();
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("{other}: unknown option (try --help)");
                return ExitCode::from(2);
            }
            other => {
                if word.is_some() {
                    eprintln!("unexpected argument {other:?}");
                    return ExitCode::from(2);
                }
                word = Some(other.to_string());
            }
        }
    }

    let cat = catalogue();

    if check {
        return check_the_sheet(&cat);
    }
    if let Some(path) = write_sheet {
        return write_the_sheet(&cat, &path);
    }

    let pos = match pos_name.as_deref() {
        Some(name) => match pos_from_name(name) {
            Some(pos) => pos,
            None => {
                eprintln!("{name:?}: not a part of speech");
                return ExitCode::from(2);
            }
        },
        None => PartOfSpeech::Noun,
    };

    if matches!(pos, PartOfSpeech::Verb | PartOfSpeech::NegativeVerb) && number.is_some() {
        eprintln!("--plural-only/--singular-only applies to nominals; ignoring");
        number = None;
    }

    let word = match word {
        Some(word) => word,
        None => match read_line("word> ") {
            Some(word) => word,
            None => {
                eprintln!("no word");
                return ExitCode::from(2);
            }
        },
    };
    if word.trim().is_empty() {
        eprintln!("no word");
        return ExitCode::from(2);
    }

    if indeclinable {
        let report = match class_wizard::indeclinable(&word, pos) {
            Ok(report) => report,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
        if json {
            print_json(&report);
        } else {
            print_report(&report, false);
            print_result(&report, &cat, emit.as_deref(), number);
        }
        return ExitCode::SUCCESS;
    }

    let mut answers = Answers::new();
    let had_answers_file = answers_file.is_some();
    if let Some(path) = answers_file {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("{path}: {error}");
                return ExitCode::from(2);
            }
        };
        match parse_answers(&text) {
            Ok(parsed) => answers.extend(parsed),
            Err(error) => {
                eprintln!("{path}: {error}");
                return ExitCode::from(2);
            }
        }
    }
    let had_forms = !forms.is_empty();
    for form in forms {
        let Some((key, value)) = form.split_once('=') else {
            eprintln!("--form {form:?}: expected CELL=FORM");
            return ExitCode::from(2);
        };
        let key = key.trim();
        if Cell::parse(key).is_none() {
            eprintln!("--form {form:?}: unknown cell key {key:?}");
            return ExitCode::from(2);
        }
        answers.insert(key.to_string(), value.trim().to_string());
    }

    let interactive = answers.is_empty() && !had_answers_file && !had_forms;
    if interactive {
        if json {
            eprintln!("--json needs --answers or --form; the interactive flow prints text");
            return ExitCode::from(2);
        }
        interactive_run(&word, pos, &cat, emit.as_deref(), number)
    } else {
        batch_run(&word, pos, &answers, &cat, emit.as_deref(), json, number)
    }
}

fn usage() {
    eprintln!(
        "usage: word_wizard [WORD] [--pos POS] [--form CELL=FORM ...] [--answers FILE]\n\
         \x20                  [--indeclinable] [--plural-only | --singular-only]\n\
         \x20                  [--json] [--emit sanalista|custom]\n\
         \x20      POS: substantiivi | adjektiivi | numeraali | järjestysluku |\n\
         \x20           pronomini | erisnimi | verbi\n\
         \x20      word_wizard --write-sheet PATH\n\
         \x20      word_wizard --check-sheet"
    );
}

fn check_the_sheet(cat: &Catalogues) -> ExitCode {
    match check_sheet(cat) {
        Ok(mismatches) if mismatches.is_empty() => {
            let classes = parse_sheet(SHEET).map(|sheet| sheet.len()).unwrap_or(0);
            println!("sheet ok: {classes} classes");
            ExitCode::SUCCESS
        }
        Ok(mismatches) => {
            for mismatch in &mismatches {
                println!(
                    "{} {}: expected {:?}, got {:?}",
                    mismatch.class, mismatch.key, mismatch.expected, mismatch.actual
                );
            }
            eprintln!("{} stale rows", mismatches.len());
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn write_the_sheet(cat: &Catalogues, path: &str) -> ExitCode {
    if path.is_empty() {
        eprintln!("--write-sheet needs a path");
        return ExitCode::from(2);
    }
    let text = render_sheet(cat);
    if path == "-" {
        print!("{text}");
        return ExitCode::SUCCESS;
    }
    match fs::write(path, text) {
        Ok(()) => {
            println!("wrote {path}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{path}: {error}");
            ExitCode::from(2)
        }
    }
}

fn batch_run(
    word: &str,
    pos: PartOfSpeech,
    answers: &Answers,
    cat: &Catalogues,
    emit: Option<&str>,
    json: bool,
    number: Option<Number>,
) -> ExitCode {
    match infer_with_number(word, pos, answers, cat, true, number) {
        Ok(report) => {
            if json {
                print_json(&report);
            } else {
                print_report(&report, false);
                print_result(&report, cat, emit, number);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn interactive_run(
    word: &str,
    pos: PartOfSpeech,
    cat: &Catalogues,
    emit: Option<&str>,
    number: Option<Number>,
) -> ExitCode {
    let sheet = match parse_sheet(SHEET) {
        Ok(sheet) => sheet,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let mut answers = Answers::new();

    let final_report = 'outer: loop {
        let report = match infer_with_number(word, pos, &answers, cat, true, number) {
            Ok(report) => report,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
        print_report(&report, false);

        let Some(cell) = report.question else {
            break report;
        };
        let key = cell.key().unwrap_or_else(|| "?".to_string());
        let hint = example(&sheet, report.ranked[0].class, &key)
            .map(|example| format!(" (e.g. {example})"))
            .unwrap_or_default();
        let label = format!("{}{hint} ", question_label(&cell));

        loop {
            let Some(answer) = read_line(&label) else {
                break 'outer report;
            };
            match answer.as_str() {
                "" | "skip" => {
                    answers.insert(key.clone(), String::new());
                    break;
                }
                "quit" | "q" => break 'outer report,
                "why" => {
                    print_report(&report, true);
                    continue;
                }
                "examples" => {
                    print_examples(&sheet, &report);
                    continue;
                }
                _ => {
                    answers.insert(key.clone(), answer);
                    break;
                }
            }
        }
    };

    print_result(&final_report, cat, emit, number);
    ExitCode::SUCCESS
}

fn read_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    io::stdout().flush().ok()?;
    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => Some(line.trim().to_string()),
        Err(_) => None,
    }
}

fn question_label(cell: &Cell) -> String {
    if cell.case.is_none() {
        let mood = cell.mood.map(|mood| mood.name()).unwrap_or("?");
        let tense = cell.tense.map(|tense| tense.name()).unwrap_or("");
        let person = cell.person.map(|person| person.name()).unwrap_or("");
        let number = cell.number.map(|number| number.name()).unwrap_or("");
        return format!("{mood} {tense} {person} {number}");
    }
    let case = cell.case.map(|case| case.name()).unwrap_or("?");
    let number = cell.number.map(|number| number.name()).unwrap_or("?");
    format!("{case} {number}")
}

fn example(sheet: &Sheet, class: Inflection, key: &str) -> Option<String> {
    let section = sheet.get(&class.class.to_string())?;
    let model = section.get("model")?;
    let form = section.get(key)?;
    Some(format!("{model} -> {form}"))
}

fn print_report(report: &Report, all: bool) {
    println!("\n{} ({})", report.lemma, report.pos.name());
    if !report.recorded.is_empty() {
        let recorded = report
            .recorded
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("  sanalista: {recorded}");
    }
    println!("  candidates ({}):", report.ranked.len());
    let shown = if all {
        report.ranked.len()
    } else {
        report.ranked.len().min(5)
    };
    for (index, candidate) in report.ranked.iter().take(shown).enumerate() {
        let mark = if index == 0 { "->" } else { "  " };
        let star = if candidate.recorded { "*" } else { " " };
        println!(
            "  {mark}{star} {:<6} hits={} misses={} unknown={} tail={} prior={}",
            candidate.class,
            candidate.hits,
            candidate.misses,
            candidate.unknown,
            candidate.tail_prior,
            candidate.prior
        );
    }
    for outcome in &report.outcomes {
        let status = match outcome.status {
            AnswerStatus::Hit => "hit",
            AnswerStatus::Miss => "MISS",
            AnswerStatus::Unknown => "unknown",
            AnswerStatus::Skipped => "skipped",
        };
        println!("     {:<8} {:<20} {status}", outcome.key, outcome.form);
    }
    if let Some(key) = report.question_key() {
        println!("  next: {key}");
    } else if report.tie {
        println!("  the top classes are indistinguishable for this word");
    }
}

fn print_examples(sheet: &Sheet, report: &Report) {
    let Some(best) = report.ranked.first() else {
        return;
    };
    let mut seen = HashSet::new();
    for candidate in report
        .ranked
        .iter()
        .filter(|candidate| candidate.hits == best.hits)
        .take(3)
    {
        let base = candidate.class.class.to_string();
        if !seen.insert(base.clone()) {
            continue;
        }
        let Some(section) = sheet.get(&base) else {
            continue;
        };
        println!(
            "  class {base}: {}",
            section.get("model").map(String::as_str).unwrap_or("")
        );
        for key in class_wizard::CORE_KEYS {
            if let Some(form) = section.get(key) {
                println!("    {key:<8} {form}");
            }
        }
    }
}

fn capitalize_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn print_result(
    report: &Report,
    cat: &Catalogues,
    emit: Option<&str>,
    number: Option<Number>,
) {
    let Some(best) = report.ranked.first() else {
        println!("no candidate class");
        return;
    };
    println!("\nbest: {}", best.class);
    let capitalize = report
        .lemma
        .chars()
        .next()
        .map(|first| first.is_uppercase())
        .unwrap_or(false);

    if best.class.class == class_wizard::INDECLINABLE_CLASS {
        println!("  taipumaton: vain perusmuoto");
        println!("  perusmuoto {:<8} {}", "", report.lemma);
    } else {
        let index = LexiconIndex::new(kotus::lemmas());
        let forms = inflect_in(
            &report.lemma,
            best.class,
            cat,
            Some(&index),
            report.pos,
            false,
        );
        for key in if matches!(report.pos, PartOfSpeech::Verb | PartOfSpeech::NegativeVerb) {
            &class_wizard::VERB_KEYS[..]
        } else {
            &class_wizard::CORE_KEYS[..]
        } {
            let key = *key;
            let cell = Cell::parse(key).unwrap();
            if !matches!(report.pos, PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
                && number.is_some_and(|want| cell.number != Some(want))
            {
                continue;
            }
            let mut words: Vec<&str> = forms
                .iter()
                .filter(|form| form.readings.iter().any(|tags| cell.claims(tags)))
                .map(|form| form.word.as_str())
                .collect();
            words.sort_unstable();
            words.dedup();
            if !words.is_empty() {
                let shown: Vec<String> = words
                    .iter()
                    .map(|word| {
                        if capitalize {
                            capitalize_first(word)
                        } else {
                            (*word).to_string()
                        }
                    })
                    .collect();
                println!("  {key:<8} {}", shown.join("/"));
            }
        }
    }

    let class = best.class.to_string();
    let pos = report.pos.finnish();
    match emit {
        Some("sanalista") => {
            println!("\n{word}\t\t{pos}\t{class}", word = report.lemma);
            println!(
                "python3 tools/add_sanalista_word.py --after <ANCHOR> {word} {pos} {class}",
                word = report.lemma
            );
        }
        Some("custom") => {
            let flag = match number {
                Some(Number::Singular) => "number=sg",
                Some(Number::Plural) => "number=pl",
                Some(Number::Collective) | None => "-",
            };
            println!("\n{word}\t{class}\t{pos}\t{flag}", word = report.lemma);
        }
        Some(other) => eprintln!("unknown --emit {other:?} (sanalista|custom)"),
        None => {}
    }
}

fn print_json(report: &Report) {
    let candidates: Vec<serde_json::Value> = report
        .ranked
        .iter()
        .take(25)
        .map(|candidate| {
            json!({
                "class": candidate.class.to_string(),
                "hits": candidate.hits,
                "misses": candidate.misses,
                "unknown": candidate.unknown,
                "recorded": candidate.recorded,
                "tail_prior": candidate.tail_prior,
                "prior": candidate.prior,
            })
        })
        .collect();
    let outcomes: Vec<serde_json::Value> = report
        .outcomes
        .iter()
        .map(|outcome| {
            let status = match outcome.status {
                AnswerStatus::Hit => "hit",
                AnswerStatus::Miss => "miss",
                AnswerStatus::Unknown => "unknown",
                AnswerStatus::Skipped => "skipped",
            };
            json!({
                "key": outcome.key,
                "form": outcome.form,
                "status": status,
            })
        })
        .collect();
    let recorded: Vec<String> = report.recorded.iter().map(ToString::to_string).collect();
    println!(
        "{}",
        json!({
            "word": report.lemma,
            "pos": report.pos.name(),
            "recorded": recorded,
            "candidates": candidates,
            "total_candidates": report.ranked.len(),
            "outcomes": outcomes,
            "question": report.question_key(),
            "tie": report.tie,
        })
    );
}
