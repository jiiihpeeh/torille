//! Parser for the Kotus paradigm catalogue (`Taivutustyypit`).
//!
//! The file is a plain-text, CRLF, Latin-1 document. Under the heading
//! `TAIVUTUSTYYPIT` every class is laid out as a bare number, optionally
//! followed by a bracketed remark, and then one line per paradigm slot:
//!
//! ```text
//! 9
//! kala
//! kalan
//! kalaa
//! kalaan
//! kalat
//! kalojen (kalain)
//! kaloja
//! kalaihin
//! ```
//!
//! A slot line may list several alternants separated by spaces, and optional
//! ones are wrapped in parentheses.

use std::collections::BTreeMap;
use std::path::Path;

use crate::kotus::ClassId;

/// One paradigm slot: the model word plus every alternant the catalogue lists.
#[derive(Clone, Debug, Default)]
pub struct Slot {
    pub model: String,
    pub alternants: Vec<String>,
    /// The alternants the catalogue wrapped in parentheses, the rare and
    /// lexicalised readings that are a word but not the class's own.
    pub optional: Vec<String>,
}

impl Slot {
    /// The model word followed by each alternant, in catalogue order.
    pub fn variants(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.model.as_str()).chain(self.alternants.iter().map(String::as_str))
    }

    /// Whether `variant` is one the catalogue marked optional.
    pub fn is_optional(&self, variant: &str) -> bool {
        self.optional.iter().any(|a| a == variant)
    }

    /// Longest common prefix of the model and an alternant, capped at the
    /// length of the model word itself.
    pub fn overlap(&self, variant: &str) -> usize {
        lcp_len(&self.model, variant)
    }
}

/// The paradigm of a single inflection class.
#[derive(Clone, Debug, Default)]
pub struct Paradigm {
    pub class: ClassId,
    /// Slot 0 is always the nominative, i.e. the model word itself.
    pub slots: Vec<Slot>,
}

impl Paradigm {
    /// The model word the catalogue uses for this class.
    #[inline]
    pub fn model(&self) -> &str {
        self.slots.first().map_or("", |s| s.model.as_str())
    }
}

/// All paradigms, keyed by class id.
#[derive(Clone, Debug, Default)]
pub struct Catalogues {
    paradigms: BTreeMap<ClassId, Paradigm>,
}

impl Catalogues {
    pub fn get(&self, class: ClassId) -> Option<&Paradigm> {
        self.paradigms.get(&class)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ClassId, &Paradigm)> {
        self.paradigms.iter()
    }

    pub fn len(&self) -> usize {
        self.paradigms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paradigms.is_empty()
    }

    /// Parses the catalogue out of the raw document text.
    pub fn parse(text: &str) -> Self {
        let mut paradigms: BTreeMap<ClassId, Paradigm> = BTreeMap::new();
        let mut current: Option<Paradigm> = None;

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if is_section_heading(line) {
                current = None; // end of TAIVUTUSTYYPIT
                continue;
            }
            if let Some(class) = class_header(line) {
                if let Some(done) = current.take() {
                    paradigms.insert(done.class, done);
                }
                current = Some(Paradigm {
                    class,
                    slots: Vec::new(),
                });
                continue;
            }
            let Some(para) = current.as_mut() else {
                continue;
            };
            if !is_form_line(line) {
                continue;
            }
            if let Some(slot) = parse_slot(line) {
                para.slots.push(slot);
            }
        }
        if let Some(done) = current.take() {
            paradigms.insert(done.class, done);
        }
        Self { paradigms }
    }

    /// Reads and parses the catalogue from disk.
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        // The Kotus document is distributed as Latin-1.
        let text = String::from_utf8(bytes.clone())
            .unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
        Ok(Self::parse(&text))
    }

    /// The catalogue as it ships: precomputed into a binary blob (by
    /// `build.rs`, from `data/catalogue.csv`) and embedded with
    /// `include_bytes!`.
    ///
    /// The Kotus taivutustyypit document is neither embedded nor
    /// parsed, at build time or run time; the templates come from the
    /// Omorfi/Voikko oracles via `tools/build_catalogue_csv.py`.
    /// Decoding the blob is a handful of reads and string copies over
    /// ~75 short entries, no line-based parse. Parsed once and cached
    /// for the process.
    pub fn embedded() -> &'static Catalogues {
        static EMBEDDED: std::sync::OnceLock<Catalogues> = std::sync::OnceLock::new();
        EMBEDDED.get_or_init(|| {
            Self::from_bytes(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/precomputed/catalogue.bin",
            )))
        })
    }

    /// Decode the blob `build.rs` writes from `data/catalogue.csv`.
    ///
    /// Panics on a malformed or version-mismatched blob: the only
    /// producer is the build script and the only consumer is
    /// [`Catalogues::embedded`], both checked in together, so a
    /// failure here is a build defect rather than untrusted input.
    pub fn from_bytes(bytes: &[u8]) -> Catalogues {
        assert!(
            bytes.len() >= HEADER_BYTES,
            "catalogue blob is truncated ({} bytes)",
            bytes.len()
        );
        assert_eq!(&bytes[0..4], CATALOGUE_MAGIC, "catalogue blob magic");
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!(version, CATALOGUE_VERSION, "catalogue blob version");
        let body_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let strings_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let body = &bytes[HEADER_BYTES..HEADER_BYTES + body_len];
        let strings = &bytes[HEADER_BYTES + body_len..HEADER_BYTES + body_len + strings_len];

        let mut at = 0usize;
        let class_count = read_u32(body, &mut at) as usize;
        let mut paradigms: BTreeMap<ClassId, Paradigm> = BTreeMap::new();
        for _ in 0..class_count {
            let class = read_u16(body, &mut at);
            let slot_count = read_u32(body, &mut at) as usize;
            let mut slots = Vec::with_capacity(slot_count);
            for _ in 0..slot_count {
                let model = read_blob_string(body, strings, &mut at);
                let alternant_count = read_u32(body, &mut at) as usize;
                let mut alternants = Vec::with_capacity(alternant_count);
                for _ in 0..alternant_count {
                    alternants.push(read_blob_string(body, strings, &mut at));
                }
                let optional_count = read_u32(body, &mut at) as usize;
                let mut optional = Vec::with_capacity(optional_count);
                for _ in 0..optional_count {
                    optional.push(read_blob_string(body, strings, &mut at));
                }
                slots.push(Slot {
                    model,
                    alternants,
                    optional,
                });
            }
            paradigms.insert(class, Paradigm { class, slots });
        }
        Catalogues { paradigms }
    }
}

const CATALOGUE_MAGIC: &[u8; 4] = b"TCAT";
const CATALOGUE_VERSION: u32 = 1;
const HEADER_BYTES: usize = 16;

fn read_u16(buf: &[u8], at: &mut usize) -> u16 {
    let v = u16::from_le_bytes(buf[*at..*at + 2].try_into().unwrap());
    *at += 2;
    v
}

fn read_u32(buf: &[u8], at: &mut usize) -> u32 {
    let v = u32::from_le_bytes(buf[*at..*at + 4].try_into().unwrap());
    *at += 4;
    v
}

fn read_blob_string(body: &[u8], strings: &[u8], at: &mut usize) -> String {
    let offset = read_u32(body, at) as usize;
    let len = read_u32(body, at) as usize;
    std::str::from_utf8(&strings[offset..offset + len])
        .expect("catalogue blob holds UTF-8")
        .to_string()
}

fn is_section_heading(line: &str) -> bool {
    line.starts_with("ASTEVAIHTELUTYYPIT") || line.starts_with("TAIVUTUSTYYPIT")
}

/// Recognises `12`, `77 (vaillinaisesti taipuvia verbejä; ...)` and friends.
fn class_header(line: &str) -> Option<ClassId> {
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = &line[digits.len()..];
    let rest = rest.trim();
    // Either nothing, or a remark in parentheses.
    if !(rest.is_empty() || (rest.starts_with('(') && rest.ends_with(')'))) {
        return None;
    }
    digits.parse().ok()
}

/// A form line holds only letter-ish tokens; prose does not.
fn is_form_line(line: &str) -> bool {
    let mut saw_token = false;
    for tok in tokenize_forms(line) {
        if !saw_token {
            saw_token = true;
        }
        if !is_wordlike(tok) {
            return false;
        }
    }
    saw_token
}

fn tokenize_forms(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’' || c == '-'))
        .filter(|s| !s.is_empty())
}

fn is_wordlike(tok: &str) -> bool {
    tok.len() <= 32
        && !GRAMMAR_HEADINGS.contains(&tok)
        && tok
            .chars()
            .all(|c| c.is_alphabetic() || matches!(c, '\'' | '’' | '-'))
}

/// Headings that appear inside paradigm blocks in the catalogue document. They
/// are labels for following columns (`Yhdysnominit`) or for the conjugation
/// table a noun inflects like (`preesensin`), not forms.
const GRAMMAR_HEADINGS: &[&str] = &[
    "Yhdysnominit",
    "yhdysnominit",
    "preesensin",
    "imperfektin",
    "partisiippi",
];

/// Splits `kalojen (kalain)` into a model plus its alternants, dropping the
/// optional ones from the mandatory set but keeping them in `alternants`.
///
/// The parenthesis has to be read off the raw line, since the space and the
/// brackets are both word delimiters: the scanner carries whether it is inside
/// a parenthetical into the next token.
fn parse_slot(line: &str) -> Option<Slot> {
    let mut mandatory: Vec<String> = Vec::new();
    let mut optional: Vec<String> = Vec::new();
    let mut in_parens = false;
    let mut cur = String::new();
    let flush = |cur: &mut String, bucket: &mut Vec<String>| {
        if !cur.is_empty() {
            bucket.push(std::mem::take(cur));
        }
    };
    for c in line.chars() {
        match c {
            '(' => {
                flush(
                    &mut cur,
                    if in_parens {
                        &mut optional
                    } else {
                        &mut mandatory
                    },
                );
                in_parens = true;
            }
            ')' => {
                flush(&mut cur, &mut optional);
                in_parens = false;
            }
            c if c.is_alphanumeric() || matches!(c, '\'' | '’' | '-') => cur.push(c),
            _ => flush(
                &mut cur,
                if in_parens {
                    &mut optional
                } else {
                    &mut mandatory
                },
            ),
        }
    }
    flush(
        &mut cur,
        if in_parens {
            &mut optional
        } else {
            &mut mandatory
        },
    );
    if mandatory.is_empty() {
        return None;
    }
    Some(Slot {
        model: mandatory.remove(0),
        alternants: mandatory.into_iter().chain(optional.clone()).collect(),
        optional,
    })
}

/// Length of the longest common prefix of `a` and `b`.
pub fn lcp_len(a: &str, b: &str) -> usize {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    ab.iter().zip(bb).take_while(|(x, y)| x == y).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
9
kala
kalan
kalaa
kalaan
kalat
kalojen (kalain)
kaloja
kalaihin

10
koira
koiran
koiraa
kiraan
koirat
koirien (koirain)
koiria
koiriin

78 (vaillinaisesti taipuvia verbejä)
kaikaa
kaikaisi
";

    #[test]
    fn parses_slots_in_order() {
        let cat = Catalogues::parse(SAMPLE);
        assert_eq!(cat.len(), 3);
        let p = cat.get(9).unwrap();
        assert_eq!(p.model(), "kala");
        assert_eq!(p.slots.len(), 8);
        assert_eq!(p.slots[5].model, "kalojen");
        assert_eq!(p.slots[5].alternants, vec!["kalain".to_string()]);
    }

    #[test]
    fn overlap_finds_stem_boundary() {
        let cat = Catalogues::parse(SAMPLE);
        let p = cat.get(10).unwrap();
        // `koirien` shares only `koir` with the model `koira`, so the ending
        // attached at offset 4 is `ien`; koira + ien = koirien.
        assert_eq!(p.slots[0].overlap("koirien"), 4);
        // The slot's own model already *is* the form, so the overlap is total.
        assert_eq!(p.slots[5].overlap("koirien"), 7);
    }

    #[test]
    fn stops_at_next_section() {
        let cat = Catalogues::parse(&format!("{SAMPLE}\nASTEVAIHTELUTYYPIT\n===\n1\nkk : k\n"));
        assert_eq!(cat.len(), 3);
    }

    #[test]
    fn lcp_works() {
        assert_eq!(lcp_len("koira", "koirien"), 4);
        assert_eq!(lcp_len("risti", "ristin"), 5);
        assert_eq!(lcp_len("abc", "xyz"), 0);
    }
}
