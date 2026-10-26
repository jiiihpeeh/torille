//! Parsing of Kotus data files.
//!
//! Two data sets are used:
//!
//! * *Nykysuomen sanalista* — a tab separated lemma list. Each record is
//!   `lemma \t homonymy \t part_of_speech \t inflection_info`, 104 742 records.
//!   `inflection_info` holds inflection class numbers, consonant gradation
//!   letters and the usual `99` (indeclinable) / `101` (irregular pronoun) codes.
//!
//! * *Taivutustyypit* — the paradigm catalogue. Every class number is followed
//!   by a model word and its inflected forms, e.g.
//!
//!   ```text
//!   9
//!   kala
//!   kalan
//!   kalaa
//!   kalaan
//!   kalat
//!   kalojen (kalain)
//!   kaloja
//!   kalaihin
//!   ```

use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

use crate::map_reader::{read_u16, read_u32, section};
use crate::tags::{Case, Number};

/// Inflection class of a lemma.
pub type ClassId = u16;

// `Gradation` and `Grade` live with the morphology primitive they describe,
// not with Kotus data file handling; the canonical home is `crate::gradation`.
// Re-exported here so the long-standing `crate::kotus::Gradation` / `Grade`
// path keeps working for the 100+ call sites in `expand` and the catalogue
// test in `tests/catalogue.rs`.
pub use crate::gradation::{Gradation, Grade};

/// One inflection class claimed by a lemma, plus its gradation code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Inflection {
    pub class: ClassId,
    pub gradation: Option<Gradation>,
}

impl fmt::Display for Inflection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.class)?;
        if let Some(g) = self.gradation {
            write!(f, "*{}", (b'A' + g.0 - 1) as char)?;
        }
        Ok(())
    }
}

/// A single sanalista record.
///
/// The three text fields borrow rather than own, so a lexicon parsed out of a
/// `&str` -- or out of the embedded [`crate::SANALISTA`] -- costs no allocation
/// per field. `build.rs` precomputes the embedded one, so a [`EmbeddedLemma`]
/// is what a hot path should use; this type is the bridge for the engine, which
/// takes `&[Lemma]`.
#[derive(Clone, Debug)]
pub struct Lemma<'a> {
    pub word: &'a str,
    pub homonymy: &'a str,
    pub part_of_speech: &'a str,
    /// Every class the lemma inflects as. Empty for compounds and indeclinables.
    ///
    /// The one owned field: it is a variable-length list, and packing it into
    /// the flat table would mean decoding on every read.
    pub inflections: Vec<Inflection>,
}

/// Whether a class number stands for a model declension rather than marking the
/// word instead of declining it.
///
/// `data/kotus-tyypit.txt:55` says what the three are: a word that is
/// uninflected or only partly inflected carries `99`, and a pronoun carries
/// `101` because "no model declension can be given for it"
/// (*"Pronominien jäljessä ei ole taivutusnumeroa, koska niille ei voida
/// esittää mallitaivutusta"*). `100` is the third of the family and is read the
/// same way.
///
/// This is deliberately not [`Lemma::is_indeclinable`], which asks the other
/// question -- whether *this word* is marked uninflecting -- and which `100` does
/// not answer the same way, so that `carries_a_case` is unchanged by anything
/// said here. What both agree on is that these three numbers are not something a
/// word can be counted as declining.
pub fn carries_a_model(class: ClassId) -> bool {
    class != 99 && class != 100 && class != 101
}

/// Whether the engine supplies a paradigm of its own for `class`, so that a word
/// the sanalista gives no *model* for still has forms worth indexing.
///
/// This asks a different question from [`carries_a_model`] and the two disagree
/// on purpose. `data/kotus-tyypit.txt:55-58` withholds a number from both for the
/// same stated reason -- no model declension can be presented -- but that says
/// different things about the word. `101` marks a pronoun, and a pronoun has no
/// model *precisely because it is irregular*; `class_101.rs` is that irregular
/// paradigm written out by hand, so every form it emits is a pronoun form.
/// `99` marks a word that does not decline, and for those the sanalista offers
/// nothing to inflect from.
///
/// So a `101` word's forms belong in the reverse index even though it carries no
/// model, and before this existed they did not: the index skips every class
/// `carries_a_model` rejects, so `hänen` was a form the engine could produce and
/// the index could not find. Measured over the 626 201 `(lemma, form)` pairs the
/// reference confirms, that was 105 pairs across 13 pronoun lemmas -- all of them
/// forms the engine already emitted correctly.
///
/// A `99` word's forms still do not belong there. `class_99.rs` runs the `-i`
/// loan machinery over every class-99 word outside its list of indeclinables,
/// which is right for the ten such words the reference confirms as loans
/// (`miss`, `seesam`, `aprilli`) and a guess for the other 5 714: indexing them
/// would add about 2 500 forms the reference confirms for none of them. That is
/// a defect of its own, and it needs data the sanalista does not carry rather
/// than a different flag here.
pub fn has_own_paradigm(class: ClassId) -> bool {
    class == 101
}

impl Lemma<'_> {
    /// True when the sanalista marked the word as non-inflecting (`99`) or as
    /// an irregular pronoun (`101`).
    pub fn is_indeclinable(&self) -> bool {
        self.inflections
            .iter()
            .all(|i| i.class == 99 || i.class == 101)
            && !self.inflections.is_empty()
    }

    /// True when the record carried no inflection information at all, which is
    /// how the sanalista flags most compounds and proper nouns.
    pub fn is_compound_like(&self) -> bool {
        self.inflections.is_empty()
    }
}

/// Parses one `inflection_info` field such as `53*F, (54*F)` or `9*B*A*A`.
///
/// Parentheses mean more than one thing in this column, and `data/kotus-tyypit.txt`
/// says what: the `taivutus` attribute of a declension number takes `harvinainen`
/// (rare), `mahdollinen` (possible), `yksikössä` (in the singular) or `monikossa`
/// (in the plural), and the `astevaihtelu` attribute takes `valinnainen` (the
/// gradation may be applied or not). The sanalista writes the first four as
/// brackets.
///
/// **A bracketed item beside an unbracketed one is dropped, and that is
/// deliberate.** `koiras` is `41, (39)`; class 39 is a near neighbour of class 41
/// whose model word is a different word, so running it yields `koirakseen` where
/// the reference confirms `koiraaseen`. Of the 18 fingerprint lemmas in this
/// position, running the bracketed declension gains confirmed forms for three
/// (`humala` gains 13, `smoothie` and `sioux` each gain their partitive) and
/// nothing confirmed for the other fifteen, while `salama` gains `salamia` and
/// `koala` gains `koalia`, which are not words. A declension the sanalista calls
/// rare is a real declension, but a class's model word is not this word, and
/// nothing here tells the two apart.
///
/// **A field that is *only* a bracketed class is a different thing and is read.**
/// It is the sanalista's notation for a word it does not split: `adagio` is `(1)`,
/// `a cappella` is `(9)`, `chic` is `(5)`. There is no unbracketed item to prefer,
/// so the bracket is the only thing standing between the field and the class.
/// Dropping it left 43 rows with no class at all, and `inflect adagio` raised
/// `no class to inflect with`.
///
/// `mon.` is `monikossa`, and `8*(C)` is a `valinnainen` gradation. Neither is
/// read: `kolme` is `8, (mon.7)`, and the plural the reference confirms needs
/// class 7 *with a gradation the sanalista does not give*, so honouring the
/// marker would mean inventing the letter; and `8*(C)` is already covered by
/// taking the plain class, which is 100% of the reference in all five measurable
/// cases where the gradation is not.
pub fn parse_inflections(field: &str) -> Vec<Inflection> {
    let items: Vec<&str> = field
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    // A field with no unbracketed item anywhere has nothing to prefer, so its
    // brackets are notation rather than a qualifier.
    let unbracketed_absent = !items.is_empty() && items.iter().all(|i| i.starts_with('('));
    let mut out: Vec<Inflection> = Vec::new();
    for item in items {
        let item = if unbracketed_absent {
            item.trim_start_matches('(').trim_end_matches(')')
        } else {
            item
        };
        let Some(inf) = parse_single(item) else {
            continue;
        };
        if !out.contains(&inf) {
            out.push(inf);
        }
    }
    out
}

pub(crate) fn parse_single(item: &str) -> Option<Inflection> {
    let core = item.split('(').next()?.trim();
    if core.is_empty() {
        return None;
    }
    // Classes look like `9`, `9*B`, `9*B*A*A`; anything with stray letters
    // (`mon.`) or odd punctuation is dropped.
    let digits: String = core.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let class: ClassId = digits.parse().ok()?;
    let rest = &core[digits.len()..];
    if !rest.chars().all(|c| c == '*' || c.is_ascii_uppercase()) {
        return None;
    }
    let gradations: Vec<Gradation> = rest
        .split('*')
        .skip(1)
        .filter_map(|c| c.chars().next())
        .filter_map(Gradation::from_letter)
        .collect();
    let gradation = gradations.first().copied();
    Some(Inflection { class, gradation })
}

/// Parses the sanalista from its raw text.
/// The case and number a paradigm slot stands for, if it stands for one.
///
/// The Kotus paradigms always list the singular nominative, genitive, partitive
/// and illative first and then the same four for the plural, in that order, for
/// every class that has them. The order is therefore a fact about the data
/// rather than something each class has to state, and it is what lets a
/// generated form be named without anybody labelling the table by hand.
///
/// A class with more than eight forms runs past the end of that list: verbs,
/// which have a dozen or more. Those slots are whatever the class uses them for,
/// so they are named by the code that knows the paradigm, not here, and this
/// returns `None` for them.
pub fn slot_nominal(i: usize) -> Option<(Case, Number)> {
    match i {
        0 => Some((Case::Nominative, Number::Singular)),
        1 => Some((Case::Genitive, Number::Singular)),
        2 => Some((Case::Partitive, Number::Singular)),
        3 => Some((Case::Illative, Number::Singular)),
        4 => Some((Case::Nominative, Number::Plural)),
        5 => Some((Case::Genitive, Number::Plural)),
        6 => Some((Case::Partitive, Number::Plural)),
        7 => Some((Case::Illative, Number::Plural)),
        _ => None,
    }
}

pub fn parse_sanalista(text: &str) -> Vec<Lemma<'_>> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if n == 0 || line.trim().is_empty() {
            continue; // header row
        }
        let mut f = line.split('\t');
        let Some(word) = f.next() else { continue };
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        out.push(Lemma {
            word,
            homonymy: f.next().unwrap_or("").trim(),
            part_of_speech: f.next().unwrap_or("").trim(),
            inflections: parse_inflections(f.next().unwrap_or("").trim()),
        });
    }
    out
}

/// Reads a lexicon file's text, for [`parse_sanalista`] to parse.
///
/// This exists only for callers who want a lexicon other than the embedded one.
/// There is deliberately no `load` function that returns parsed rows: the rows
/// would borrow from the text, which a function cannot return alongside. Keep
/// the `String` alive yourself:
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// let text = torille::kotus::read_sanalista("my-lexicon.txt")?;
/// let lemmas = torille::kotus::parse_sanalista(&text);
/// assert!(!lemmas.is_empty());
/// # Ok(())
/// # }
/// ```
///
/// For the noun lexicon, use [`sanalista`]: it is already in the binary and
/// already parsed, so it needs neither this nor a data file.
pub fn read_sanalista(path: impl AsRef<Path>) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

mod generated {
    //! The lexicon as `build.rs` precomputed it. See the crate's `build.rs`.
    //!
    //! One artefact is committed to `data/precomputed/`: `sanalista.bin`, the
    //! record blob (per-row records, headword strings, inflections, POS table,
    //! homonym side map, `WORD_ORDER`, `TAIL_ORDER`). Embedded here as `BLOB`.
    //!
    //! The blob's section layout (see `build.rs` for the canonical diagram):
    //!
    //! ```text
    //!   [0..48]                 header: magic, row_count, n_strings,
    //!                           n_infl, section offsets, hom_count
    //!   [48..48+8*N]            per-row records:
    //!                             u32 headword offset
    //!                             u8  headword length
    //!                             u8  POS code
    //!                             u16 inflection start
    //!   [..]                    headword strings (packed UTF-8)
    //!   [..]                    inflections array (u16 codes:
    //!                           class<<8 | gradation)
    //!   [..]                    POS table (count, offsets, strings)
    //!   [..]                    homonym side map (row -> id, strings)
    //!   [..]                    WORD_ORDER (u32 array, N, sorted by word)
    //!   [..]                    TAIL_ORDER (u32 array, N)
    //! ```
    pub static BLOB: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/precomputed/sanalista.bin",
    ));
}

// ---------------------------------------------------------------------------
// Blob accessors. The runtime side of the sanalista's compile-time
// tables. Section starts are explicit header fields; per-row fields are
// an 8-byte record (headword offset+length, POS code, inflection start).
// ---------------------------------------------------------------------------

const HEADER_BYTES: usize = 48;
const RECORD_BYTES: usize = 8;

#[inline]
fn blob_header() -> (u32, u32, u32) {
    let b = generated::BLOB;
    (
        read_u32(b, 4),  // row_count
        read_u32(b, 8),  // n_strings
        read_u32(b, 12), // n_infl
    )
}

#[inline]
fn row_record(row: u32) -> usize {
    HEADER_BYTES + RECORD_BYTES * row as usize
}

/// Resolve a POS code through the small POS table
/// (`u32 count`, `(count+1) u32 offsets`, strings).
fn pos_string(code: u8) -> &'static str {
    let b = generated::BLOB;
    let base = section(generated::BLOB, 24);
    let count = read_u32(b, base) as usize;
    let code = code as usize;
    if code >= count {
        return "";
    }
    let offsets = base + 4;
    let start = read_u32(b, offsets + code * 4) as usize;
    let end = read_u32(b, offsets + (code + 1) * 4) as usize;
    let strings = offsets + (count + 1) * 4;
    std::str::from_utf8(&b[strings + start..strings + end]).expect("blob holds UTF-8 POS strings")
}

/// Resolve a row's homonym through the side map: a sorted array of
/// `(row u32, id u8)` entries plus a distinct-string table. Only ~938 rows
/// have a homonym, so the hot record does not carry the field.
fn row_homonym(row: u32) -> &'static str {
    let b = generated::BLOB;
    let base = section(generated::BLOB, 28);
    let count = read_u32(b, base) as usize;
    if count == 0 {
        return "";
    }
    let entries = base + 4;
    let (mut lo, mut hi) = (0usize, count);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if read_u32(b, entries + mid * 5) < row {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo >= count || read_u32(b, entries + lo * 5) != row {
        return "";
    }
    let id = b[entries + lo * 5 + 4] as usize;
    let strings_table = entries + count * 5;
    let n_strings = read_u32(b, strings_table) as usize;
    if id >= n_strings {
        return "";
    }
    let offsets = strings_table + 4;
    let start = read_u32(b, offsets + id * 4) as usize;
    let end = read_u32(b, offsets + (id + 1) * 4) as usize;
    let strings = offsets + (n_strings + 1) * 4;
    std::str::from_utf8(&b[strings + start..strings + end])
        .expect("blob holds UTF-8 homonym strings")
}

/// Read a string field from row `row`: 0 = headword, 1 = homonym,
/// 2 = part of speech. The returned slice borrows from the blob, which is
/// `&'static`, so the lifetime is `'static`.
fn row_field(row: u32, field: u32) -> &'static str {
    let b = generated::BLOB;
    let rec = row_record(row);
    match field {
        0 => {
            let off = read_u32(b, rec) as usize;
            let len = b[rec + 4] as usize;
            if len == 0 {
                ""
            } else {
                let abs = section(generated::BLOB, 16) + off;
                std::str::from_utf8(&b[abs..abs + len]).expect("blob holds UTF-8 headwords")
            }
        }
        1 => row_homonym(row),
        2 => pos_string(b[rec + 5]),
        _ => "",
    }
}

/// Start index in the inflections array for `row`. The count
/// is the diff to the next row's start (or to `n_infl` for
/// the last row).
fn row_infl_start(row: u32) -> u32 {
    read_u16(generated::BLOB, row_record(row) + 6) as u32
}

/// A row of the embedded lexicon, borrowed out of the blob.
///
/// The struct carries only a `u32` row index. The three string
/// fields and the inflection codes are read on demand from the
/// blob via [`row_field`] / [`row_infl_start`]. The previous
/// version held three `&'static str` slices into `SANALISTA`;
/// moving to the blob is allocation-free (no new
/// `Box::leak`s, the strings already live in `.rodata`).
#[derive(Clone, Copy, Debug)]
pub struct EmbeddedLemma {
    row: u32,
}

impl EmbeddedLemma {
    /// The lemma as the sanalista spells it.
    #[inline]
    pub fn word(&self) -> &'static str {
        row_field(self.row, 0)
    }

    /// The homonymy marker, for a lemma listed more than once.
    #[inline]
    pub fn homonymy(&self) -> &'static str {
        row_field(self.row, 1)
    }

    /// The part of speech, in the sanalista's own Finnish wording.
    #[inline]
    pub fn part_of_speech(&self) -> &'static str {
        row_field(self.row, 2)
    }

    /// Every class the lemma inflects as, decoded from the
    /// packed inflections array in the blob.
    pub fn inflections(&self) -> impl Iterator<Item = Inflection> + '_ {
        let row = self.row;
        let (row_count, _, n_infl) = blob_header();
        let infl_start = section(generated::BLOB, 20);
        let from = row_infl_start(row) as usize;
        // The count is the diff to the next row's start; the
        // last row's end is `n_infl`.
        let to = if (row + 1) as usize >= row_count as usize {
            n_infl as usize
        } else {
            row_infl_start(row + 1) as usize
        };
        let mut at = from;
        std::iter::from_fn(move || {
            if at >= to {
                return None;
            }
            let packed = read_u16(generated::BLOB, infl_start + at * 2);
            at += 1;
            let class = packed >> 8;
            let code = (packed & 0xff) as u8;
            Some(Inflection {
                class,
                // `0` is the build script's "no gradation"; `A` is 1.
                gradation: (code != 0).then_some(Gradation(code)),
            })
        })
    }

    /// The row index, for callers that key off position.
    pub fn row(&self) -> u32 {
        self.row
    }
}

/// The embedded sanalista: the lexicon, parsed at build time.
///
/// The string fields borrow from `data/precomputed/sanalista.bin`
/// (`include_bytes!`'d into `.rodata`), and the inflection codes
/// live in the same blob. No data file at run time, no parse on
/// the way there. The expensive part -- splitting 104 743 rows
/// into `Lemma`s, ~31 ms and ~420 000 allocations -- is done
/// once by `build.rs` instead of once per process.
impl Sanalista {
    /// How many rows the lexicon holds. Read from the blob's
    /// header.
    pub fn len(&self) -> usize {
        blob_header().0 as usize
    }

    /// Always false: the file is not empty, and a build that
    /// produced no rows fails in `build.rs` rather than
    /// compiling.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every row, in sanalista order.
    pub fn iter(&self) -> impl Iterator<Item = EmbeddedLemma> + '_ {
        let n = self.len() as u32;
        (0..n).map(|row| EmbeddedLemma { row })
    }

    /// Every row for `word`, in sanalista order.
    ///
    /// `WORD_ORDER` is row indices sorted by headword, so this is a
    /// lower-bound binary search over that array (O(log N) string
    /// comparisons, ~17 for 104 k rows), followed by a walk over the
    /// contiguous run of equal headwords. The walk is bounded by the
    /// word's homonym count (1 for the ~99.6 % of headwords without
    /// homonyms, up to 4 for the 450 that have them). Empty when the
    /// word is not in the lexicon. This replaces the old per-headword
    /// `phf::Map`, which duplicated every word in the binary.
    #[inline]
    pub fn by_word<'w>(&'w self, word: &'w str) -> impl Iterator<Item = EmbeddedLemma> + 'w {
        let order = self.word_order();
        let (mut lo, mut hi) = (0usize, order.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if row_field(order[mid], 0) < word {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        let mut end = start;
        while end < order.len() && row_field(order[end], 0) == word {
            end += 1;
        }
        order[start..end]
            .iter()
            .copied()
            .map(|row| EmbeddedLemma { row })
    }

    /// The first row for `word`.
    pub fn get(&self, word: &str) -> Option<EmbeddedLemma> {
        self.by_word(word).next()
    }

    /// True when `word` is in the lexicon at all.
    pub fn contains(&self, word: &str) -> bool {
        self.get(word).is_some()
    }

    /// Row indices sorted by word. Used by
    /// [`crate::expand::LexiconIndex`] for prefix range
    /// queries -- the prefix is the lower bound in the sorted
    /// array, and the run that follows is the prefix's
    /// matches. Stored in the blob's `WORD_ORDER` section.
    pub fn word_order(&self) -> &'static [u32] {
        // SAFETY / alignment note. The blob's base address can land
        // on a `&[u8]` boundary (alignment 1), even though each
        // section starts on a u32 boundary relative to the start.
        // `slice::from_raw_parts(ptr as *const u32, len)` requires
        // the absolute pointer to be aligned, so we copy the
        // section into a one-shot `Vec` via `ptr::read_unaligned`
        // and cache it. The cache is bounded to one entry per
        // section for the program's lifetime.
        static CACHE: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
        CACHE
            .get_or_init(|| {
                let n = blob_header().0;
                let start = section(generated::BLOB, 36);
                let len = n as usize;
                let b = generated::BLOB;
                let mut out = Vec::with_capacity(len);
                for i in 0..len {
                    let off = start + i * 4;
                    let bytes = [b[off], b[off + 1], b[off + 2], b[off + 3]];
                    out.push(u32::from_le_bytes(bytes));
                }
                out
            })
            .as_slice()
    }

    /// Row indices sorted by reversed word. Same shape as
    /// [`word_order`], used for suffix range queries: every
    /// suffix of a word is a prefix of the word's reversal,
    /// so a suffix query is the same lower-bound-and-scan
    /// against the reversed sort.
    pub fn tail_order(&self) -> &'static [u32] {
        // See `word_order` for the alignment rationale -- the
        // base blob pointer is `&[u8]` (alignment 1), so reading
        // the u32 section via `slice::from_raw_parts` would
        // trip the alignment UB check. One-shot copy on first
        // access, cached for the program's lifetime.
        static CACHE: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
        CACHE
            .get_or_init(|| {
                let n = blob_header().0;
                let start = section(generated::BLOB, 40);
                let len = n as usize;
                let b = generated::BLOB;
                let mut out = Vec::with_capacity(len);
                for i in 0..len {
                    let off = start + i * 4;
                    let bytes = [b[off], b[off + 1], b[off + 2], b[off + 3]];
                    out.push(u32::from_le_bytes(bytes));
                }
                out
            })
            .as_slice()
    }

    /// Every row as a [`Lemma`] borrowing the embedded text.
    ///
    /// This is the bridge to the parts of the engine that take `&[Lemma]`. It
    /// allocates one `Vec<Inflection>` per row -- the only owned field -- and
    /// nothing else, so it is ~15 ms rather than the ~33 ms a re-parse costs,
    /// and it borrows rather than copying every field. A caller that only looks
    /// a word up should use [`by_word`](Self::by_word), which allocates
    /// nothing at all.
    pub fn to_lemmas(&self) -> Vec<Lemma<'static>> {
        self.iter()
            .map(|e| Lemma {
                word: e.word(),
                homonymy: e.homonymy(),
                part_of_speech: e.part_of_speech(),
                inflections: e.inflections().collect(),
            })
            .collect()
    }
}

/// The embedded sanalista.
///
/// The type carries no state -- the tables are `static` -- so it is handed out
/// by reference from [`sanalista`]. That is what lets a lookup iterator borrow
/// the lexicon for `'static` and outlive the call that made it, and it is why
/// [`EmbeddedLemma`] hangs off a reference rather than a value.
pub struct Sanalista;

impl Sanalista {
    /// The embedded lexicon.
    pub const fn new() -> Self {
        Sanalista
    }
}

impl Default for Sanalista {
    fn default() -> Self {
        Sanalista
    }
}

impl std::fmt::Debug for Sanalista {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sanalista({} rows)", self.len())
    }
}

/// The embedded sanalista.
///
/// A `'static` reference, so an iterator over the lexicon can outlive this call:
/// `sanalista().by_word(w)` allocates nothing and borrows nothing the caller has
/// to keep alive.
pub fn sanalista() -> &'static Sanalista {
    static LEXICON: Sanalista = Sanalista;
    &LEXICON
}

/// The embedded sanalista materialised once as a `Lemma` slice.
///
/// Callers that want `&[Lemma]` should use this rather than
/// `parse_sanalista(crate::SANALISTA)`: the parser re-reads the 2.86 MB
/// source text, which forces the binary to carry the lexicon twice (the
/// packed blob *and* the tab-separated source). Building the rows straight
/// from the blob keeps one copy. [`Sanalista::to_lemmas`] is proven
/// field-for-field identical to `parse_sanalista` by
/// `the_embedded_lexicon_is_the_parsed_lexicon`.
pub fn lemmas() -> &'static [Lemma<'static>] {
    static LEMMAS: OnceLock<Vec<Lemma<'static>>> = OnceLock::new();
    LEMMAS.get_or_init(|| sanalista().to_lemmas())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parentheses in the `taivutustiedot` column mean different things, and the
    /// two cases pull in opposite directions.
    ///
    /// A field that is *only* a bracketed class is how the sanalista writes a word
    /// it does not split -- `adagio` is `(1)`, `a cappella` is `(9)`, `chic` is
    /// `(5)` -- and there is no unbracketed item to prefer, so the class is the
    /// field. `inflect adagio` used to raise `no class to inflect with`.
    ///
    /// A bracketed item *beside* an unbracketed one is a declension the sanalista
    /// calls rare or possible, and it stays out. `koiras` is `41, (39)`, and class
    /// 39's model word is a different word: running it yields `koirakseen` where
    /// the reference confirms `koiraaseen`. Adding it would put 2 310 forms into
    /// the fingerprint for three lemmas' worth of confirmed ones.
    #[test]
    fn a_bracket_is_read_only_when_it_is_the_whole_field() {
        let classes = |field: &str| -> Vec<(u16, Option<Gradation>)> {
            parse_inflections(field)
                .iter()
                .map(|i| (i.class, i.gradation))
                .collect()
        };
        // Never cast a letter: `Gradation` counts `A..Z` as `1..=26`.
        let g = |c: char| Gradation::from_letter(c).unwrap();

        // The whole field bracketed: read, and the gradation with it.
        assert_eq!(classes("(1)"), vec![(1, None)], "adagio");
        assert_eq!(classes("(9)"), vec![(9, None)], "a cappella");
        assert_eq!(classes("(5)"), vec![(5, None)], "chic");
        assert_eq!(classes("(54*F)"), vec![(54, Some(g('F')))]);
        assert_eq!(classes("(41*I)"), vec![(41, Some(g('I')))]);

        // Bracketed beside unbracketed: dropped, and the plain one stands.
        assert_eq!(classes("41, (39)"), vec![(41, None)], "koiras");
        assert_eq!(classes("7*E, (5)"), vec![(7, Some(g('E')))], "alpi");
        assert_eq!(classes("9, 12, (10)").len(), 2, "koala keeps 9 and 12");
        assert_eq!(
            classes("39, (41*H)"),
            vec![(39, None)],
            "sammas keeps 39 and not the parenthesised 41*H"
        );

        // `mon.` is `monikossa` -- a plural-only marker. It is not a class, and
        // reading the digits inside it would give `kolme` a second paradigm from
        // a marker that only says *when* it applies.
        assert!(classes("(mon.7)").is_empty(), "mon. is not a class");
        assert!(
            parse_inflections("8, (mon.7)").iter().all(|i| i.class == 8),
            "kolme is class 8; the plural marker is not read"
        );

        // `valinnainen`: the gradation may be applied or not, so the plain class
        // is already the whole of what the sanalista says here.
        assert_eq!(classes("8*(C)"), vec![(8, None)], "ringette");
        assert_eq!(classes("48*(E)"), vec![(48, None)], "häive");
    }

    /// The embedded tables are produced by `build.rs`, which parses the same
    /// file a second time with its own copy of the grammar. If the two ever
    /// disagree the binary would carry a lexicon that is not the sanalista, and
    /// nothing else would notice -- so this compares them field for field over
    /// every row.
    #[test]
    fn the_embedded_lexicon_is_the_parsed_lexicon() {
        let embedded = sanalista().to_lemmas();
        let parsed = parse_sanalista(crate::SANALISTA);
        assert_eq!(
            embedded.len(),
            parsed.len(),
            "the embedded tables and parse_sanalista disagree on the row count"
        );
        for (i, (a, b)) in embedded.iter().zip(parsed.iter()).enumerate() {
            assert_eq!(a.word, b.word, "row {i}: word");
            assert_eq!(a.homonymy, b.homonymy, "row {i} ({}): homonymy", a.word);
            assert_eq!(
                a.part_of_speech, b.part_of_speech,
                "row {i} ({}): part of speech",
                a.word
            );
            assert_eq!(
                a.inflections, b.inflections,
                "row {i} ({}): inflections",
                a.word
            );
        }
    }

    /// Every row must be reachable by its own word, or the hash table has lost
    /// rows the scan would have found.
    #[test]
    fn every_row_is_reachable_by_its_word() {
        let lex = sanalista();
        for entry in lex.iter() {
            let rows: Vec<u32> = lex.by_word(entry.word()).map(|e| e.row()).collect();
            assert!(
                rows.contains(&entry.row()),
                "{} (row {}) is not reachable by hash",
                entry.word(),
                entry.row()
            );
        }
    }

    /// A homonym has several rows, and `by_word` owes the caller all of them.
    #[test]
    fn a_homonym_yields_every_row() {
        let lex = sanalista();
        // `lakka` is listed four times in the sanalista.
        let rows: Vec<String> = lex
            .by_word("lakka")
            .map(|e| format!("{}|{}", e.part_of_speech(), e.homonymy()))
            .collect();
        assert_eq!(rows.len(), 4, "expected four rows for lakka, got {rows:?}");
    }

    /// `get` is the first row for a word, which is what a caller asking "what
    /// class is this lemma" wants.
    #[test]
    fn get_is_the_first_row() {
        let lex = sanalista();
        assert_eq!(
            lex.get("lakka").map(|e| e.row()),
            lex.iter().find(|e| e.word() == "lakka").map(|e| e.row())
        );
        assert!(!lex.contains("zzzzznotaword"));
    }

    /// The inflections have to survive the packing: a class in the high bits and
    /// `A`-as-1 in the low, with 0 meaning "no gradation".
    #[test]
    fn the_inflections_round_trip_through_the_packing() {
        let lex = sanalista();
        let cases = [
            ("talo", "1"),       // no gradation
            ("kana", "9"),       // no gradation, another class
            ("kaupunki", "5*G"), // a gradation letter
            ("kylmä", "10"),
        ];
        for (word, want) in cases {
            let got: Vec<String> = lex
                .get(word)
                .expect("in the lexicon")
                .inflections()
                .map(|i| i.to_string())
                .collect();
            assert_eq!(
                got,
                vec![want.to_string()],
                "{word}: inflections did not round-trip"
            );
        }
    }

    /// Derivative flags live in columns after `Taivutustiedot`. The parser
    /// reads only the four Kotus columns, so a derived row carries its
    /// class normally and the provenance columns are ignored.
    #[test]
    fn derivative_columns_are_ignored_by_the_parser() {
        let text = "Hakusana\tHomonymia\tSanaluokka\tTaivutustiedot\tJohdos\tKanta\tLaatu\n\
                    nostatella\t\tverbi\t67\tFREQ\tnostaa\tvoikko\n\
                    jouduttautua\t\tverbi\t52\tREFL\tjouduttaa\tvoikko\n";
        let lemmas = parse_sanalista(text);
        assert_eq!(lemmas.len(), 2);
        assert_eq!(lemmas[0].word, "nostatella");
        assert_eq!(lemmas[0].part_of_speech, "verbi");
        assert_eq!(
            lemmas[0].inflections,
            vec![Inflection {
                class: 67,
                gradation: None,
            }]
        );
        assert_eq!(lemmas[1].word, "jouduttautua");
        assert_eq!(lemmas[1].inflections[0].class, 52);
    }

    /// A row with no class at all: the sanalista flags compounds and proper nouns
    /// this way, and the packed form must not invent one for them.
    #[test]
    fn a_classless_row_has_no_inflections() {
        let lex = sanalista();
        let all = lex.to_lemmas();
        let classless: Vec<&Lemma<'static>> = all
            .iter()
            .filter(|l| l.inflections.is_empty())
            .take(3)
            .collect();
        assert!(
            !classless.is_empty(),
            "the sanalista has rows with no class"
        );
        for lemma in classless {
            assert!(
                lemma.is_compound_like(),
                "{} has no class, so it is compound-like",
                lemma.word
            );
            // And the same row must be classless when looked up by hash.
            let via_hash = lex.get(lemma.word).expect("reachable by hash");
            assert_eq!(
                via_hash.inflections().count(),
                0,
                "{} gained an inflection through the hash path",
                lemma.word
            );
        }
    }
}
