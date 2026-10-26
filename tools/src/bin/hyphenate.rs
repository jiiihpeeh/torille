//! `hyphenate` — show the permitted Finnish line breaks in words.
//!
//! Default output inserts a hyphen at every permitted break. `--json`
//! prints the same points with their byte/character offsets, source and
//! ranking penalty, which is what the corpus checks consume.

use std::process::ExitCode;

use torille::hyphenation::{
    Exceptions, HyphenationOptions, LexicalCompoundAnalyzer, NoCompoundAnalyzer, VowelBreaks,
    positions_with, split,
};

fn usage() -> ExitCode {
    eprintln!(
        "usage: hyphenate [--left N] [--right N] [--no-compounds]\n\
         \x20                 [--vowel-breaks compound-only|never|always] [--json] WORD..."
    );
    ExitCode::from(2)
}

#[derive(Clone, Copy, Debug)]
struct Arguments<'a> {
    options: HyphenationOptions,
    json: bool,
    words: &'a [String],
}

fn parse_args(args: &[String]) -> Result<Arguments<'_>, ()> {
    let mut options = HyphenationOptions::default();
    let mut json = false;
    let mut at = 0usize;
    while at < args.len() {
        match args[at].as_str() {
            "--help" | "-h" => return Err(()),
            "--left" => {
                at += 1;
                options.left_min = args.get(at).and_then(|s| s.parse().ok()).ok_or(())?;
            }
            "--right" => {
                at += 1;
                options.right_min = args.get(at).and_then(|s| s.parse().ok()).ok_or(())?;
            }
            "--no-compounds" => options.compound_breaks = false,
            "--vowel-breaks" => {
                at += 1;
                options.vowel_breaks = match args.get(at).map(String::as_str) {
                    Some("compound-only" | "compound") => VowelBreaks::CompoundOnly,
                    Some("never") => VowelBreaks::Never,
                    Some("always") => VowelBreaks::Always,
                    _ => return Err(()),
                };
            }
            "--json" => json = true,
            "--" => {
                at += 1;
                break;
            }
            flag if flag.starts_with('-') && flag != "-" => return Err(()),
            _ => break,
        }
        at += 1;
    }
    if at >= args.len() {
        return Err(());
    }
    Ok(Arguments {
        options,
        json,
        words: &args[at..],
    })
}

fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn print_json(word: &str, points: &[torille::hyphenation::HyphenationPoint]) {
    let points: Vec<String> = points
        .iter()
        .map(|point| {
            format!(
                "{{\"byte_offset\":{},\"char_offset\":{},\"compound\":{},\"source\":\"{}\",\"penalty\":{}}}",
                point.byte_offset,
                point.char_offset,
                point.compound_boundary,
                point.source.as_str(),
                point.penalty,
            )
        })
        .collect();
    println!(
        "{{\"word\":{},\"points\":[{}]}}",
        json_escape(word),
        points.join(",")
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Ok(arguments) = parse_args(&args) else {
        return usage();
    };

    let exceptions = Exceptions::finnish();
    let compound = LexicalCompoundAnalyzer;
    let no_compound = NoCompoundAnalyzer;
    let analyzer: &dyn torille::hyphenation::CompoundAnalyzer = if arguments.options.compound_breaks
    {
        &compound
    } else {
        &no_compound
    };

    for word in arguments.words {
        let points = positions_with(word, &arguments.options, analyzer, exceptions);
        if arguments.json {
            print_json(word, &points);
        } else {
            println!("{}", split(word, &points).join("-"));
            if std::env::var_os("TORILLE_HYPHEN_DEBUG").is_some() {
                for point in &points {
                    eprintln!(
                        "  {}..{} {:?} compound={} penalty={}",
                        point.char_offset,
                        point.byte_offset,
                        point.source,
                        point.compound_boundary,
                        point.penalty
                    );
                }
            }
        }
    }
    ExitCode::SUCCESS
}
