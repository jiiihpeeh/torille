//! Sanalista row parser and blob writer.
//!
//! Split out of `build.rs`; `main` calls `generate` and `emit_catalogue`
//! calls `push_blob_string`.

use std::collections::BTreeMap;

/// Write `s` into the string blob and its `(offset, length)` into `body`,
/// matching `spec::Catalogues::from_bytes`.
pub(crate) fn push_blob_string(strings: &mut Vec<u8>, body: &mut Vec<u8>, s: &str) {
    let offset = strings.len() as u32;
    strings.extend_from_slice(s.as_bytes());
    body.extend_from_slice(&offset.to_le_bytes());
    body.extend_from_slice(&(s.len() as u32).to_le_bytes());
}

/// Pack the per-row records, headword strings, inflections, POS table,
/// homonym side map, and the two sorted orders into one binary blob. See the
/// layout diagram in [`generate`].
pub(crate) fn build_sanalista_blob(rows: &[Row<'_>], order: &[u32]) -> Vec<u8> {
    let n = rows.len();

    // POS is one of ~50 strings ("substantiivi", "adjektiivi, substantiivi",
    // ...). Store the strings once and a u8 code per row: this replaces
    // 8 bytes of offset+length plus ~11 bytes of text per row.
    let mut pos_list: Vec<&str> = rows.iter().map(|r| r.pos).collect();
    pos_list.sort_unstable();
    pos_list.dedup();
    assert!(
        pos_list.len() <= u8::MAX as usize,
        "more than 255 part-of-speech values"
    );
    let pos_id: BTreeMap<&str, u8> = pos_list
        .iter()
        .enumerate()
        .map(|(i, &p)| (p, i as u8))
        .collect();

    // Only ~938 rows carry a homonym. Keep them out of the hot record: a
    // sorted `(row, id)` side map plus a tiny string table.
    let mut hom_list: Vec<&str> = rows
        .iter()
        .map(|r| r.homonymy)
        .filter(|h| !h.is_empty())
        .collect();
    hom_list.sort_unstable();
    hom_list.dedup();
    assert!(
        hom_list.len() <= u8::MAX as usize,
        "more than 255 homonym values"
    );
    let hom_id: BTreeMap<&str, u8> = hom_list
        .iter()
        .enumerate()
        .map(|(i, &h)| (h, i as u8))
        .collect();
    let hom_entries: Vec<(u32, u8)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.homonymy.is_empty())
        .map(|(i, r)| (i as u32, hom_id[r.homonymy]))
        .collect();

    // Per-row record, 8 bytes:
    //   u32 headword offset, u8 headword length, u8 POS code,
    //   u16 inflection start.
    let mut records: Vec<u8> = Vec::with_capacity(n * 8);
    let mut infl_codes: Vec<u16> = Vec::new();
    let mut strings: Vec<u8> = Vec::new();
    for r in rows {
        let h_off = strings.len() as u32;
        strings.extend_from_slice(r.word.as_bytes());
        assert!(
            r.word.len() <= u8::MAX as usize,
            "headword longer than 255 bytes"
        );
        let i_off = infl_codes.len();
        assert!(
            i_off <= u16::MAX as usize,
            "inflection array exceeds the u16 start table"
        );
        for &code in &r.inflections {
            assert!(code <= u16::MAX as u32, "packed inflection exceeds u16");
            infl_codes.push(code as u16);
        }
        records.extend_from_slice(&h_off.to_le_bytes());
        records.push(r.word.len() as u8);
        records.push(pos_id[r.pos]);
        records.extend_from_slice(&(i_off as u16).to_le_bytes());
    }

    // Sort by reversed word for suffix queries.
    let mut tail_order: Vec<u32> = (0..n as u32).collect();
    tail_order.sort_by(|&a, &b| {
        rows[a as usize]
            .word
            .chars()
            .rev()
            .cmp(rows[b as usize].word.chars().rev())
    });

    // POS table: u32 count, (count + 1) u32 offsets, concatenated strings.
    let mut pos_table: Vec<u8> = Vec::new();
    pos_table.extend_from_slice(&(pos_list.len() as u32).to_le_bytes());
    let mut pos_strings: Vec<u8> = Vec::new();
    for p in &pos_list {
        pos_table.extend_from_slice(&(pos_strings.len() as u32).to_le_bytes());
        pos_strings.extend_from_slice(p.as_bytes());
    }
    pos_table.extend_from_slice(&(pos_strings.len() as u32).to_le_bytes());
    pos_table.extend_from_slice(&pos_strings);

    // Homonym table: u32 count, (row u32, id u8) x count, then a POS-shaped
    // string table for the distinct homonym values.
    let mut hom_table: Vec<u8> = Vec::new();
    hom_table.extend_from_slice(&(hom_entries.len() as u32).to_le_bytes());
    for (row, id) in &hom_entries {
        hom_table.extend_from_slice(&row.to_le_bytes());
        hom_table.push(*id);
    }
    hom_table.extend_from_slice(&(hom_list.len() as u32).to_le_bytes());
    let mut hom_strings: Vec<u8> = Vec::new();
    for h in &hom_list {
        hom_table.extend_from_slice(&(hom_strings.len() as u32).to_le_bytes());
        hom_strings.extend_from_slice(h.as_bytes());
    }
    hom_table.extend_from_slice(&(hom_strings.len() as u32).to_le_bytes());
    hom_table.extend_from_slice(&hom_strings);

    // Assemble. Offsets are explicit so the reader never has to infer a
    // section start, and the packed `(row, id)` entries are byte-aligned
    // reads through `read_u32_le`.
    const HEADER: usize = 48;
    const MAGIC: u32 = 0x324E_4153; // "SAN2"
    let strings_off = HEADER + records.len();
    let infl_off = strings_off + strings.len();
    let pos_off = infl_off + infl_codes.len() * 2;
    let hom_off = pos_off + pos_table.len();
    let word_off = hom_off + hom_table.len();
    let tail_off = word_off + 4 * n;

    let mut out: Vec<u8> = Vec::with_capacity(tail_off + 4 * n);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    out.extend_from_slice(&(infl_codes.len() as u32).to_le_bytes());
    out.extend_from_slice(&(strings_off as u32).to_le_bytes());
    out.extend_from_slice(&(infl_off as u32).to_le_bytes());
    out.extend_from_slice(&(pos_off as u32).to_le_bytes());
    out.extend_from_slice(&(hom_off as u32).to_le_bytes());
    out.extend_from_slice(&(hom_entries.len() as u32).to_le_bytes());
    out.extend_from_slice(&(word_off as u32).to_le_bytes());
    out.extend_from_slice(&(tail_off as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&records);
    out.extend_from_slice(&strings);
    for code in &infl_codes {
        out.extend_from_slice(&code.to_le_bytes());
    }
    out.extend_from_slice(&pos_table);
    out.extend_from_slice(&hom_table);
    for &r in order {
        out.extend_from_slice(&r.to_le_bytes());
    }
    for &r in &tail_order {
        out.extend_from_slice(&r.to_le_bytes());
    }
    out
}
/// Parses the rows of `text` and builds the sanalista blob.
///
/// **Blob layout** (`maps/sanalista.bin`):
///
/// ```text
///   0  u32 magic "SAN2"
///   4  u32 row_count
///   8  u32 n_strings        (headword bytes)
///  12  u32 n_infl
///  16  u32 strings_off
///  20  u32 infl_off
///  24  u32 pos_off
///  28  u32 hom_off
///  32  u32 hom_count
///  36  u32 word_order_off
///  40  u32 tail_order_off
///  44  u32 reserved
///  48  records: row_count x 8 B
///         u32 headword offset (into the headword strings)
///         u8  headword length
///         u8  POS code
///         u16 inflection start
///      strings: headwords, concatenated
///      infl:    n_infl x u16 (class<<8 | gradation)
///      pos:     u32 count, (count+1) x u32 offsets, concatenated strings
///      hom:     u32 count, count x (u32 row, u8 id),
///               u32 string_count, (string_count+1) x u32 offsets, strings
///      word_order: row_count x u32, sorted by headword
///      tail_order: row_count x u32, sorted by reversed headword
/// ```
///
/// The inflection count is implicit (the diff to the next row's start).
/// Both the start index (48 k) and the packed class/gradation code (class
/// <= 255, gradation <= 26) fit in `u16`. POS strings and homonyms are
/// stored once in small tables instead of once per row: POS was ~1.15 MB of
/// repeated text, and only 938 rows have a homonym.
///
/// The blob is the *only* lexicon artefact: the old in-source
/// `FIELDS: [(&str, &str, &str); 104743]` and the per-headword phf
/// index were both removed once the sorted `WORD_ORDER` section could
/// serve `by_word` directly.
pub(crate) fn generate(text: &str) -> Vec<u8> {
    let rows = parse_rows(text);
    assert!(!rows.is_empty(), "the sanalista parsed to no rows");

    // Every lookup is served from the blob: the per-row records and
    // strings, the inflections, and `WORD_ORDER` (sorted by headword,
    // which `by_word` binary-searches). There is no separate phf index.
    let order = sorted_word_order(&rows);
    build_sanalista_blob(&rows, &order)
}

/// All row indices sorted by their word, ascending. Used by
/// `build_sanalista_blob` to emit the WORD_ORDER section, which
/// `kotus::Sanalista::by_word` binary-searches.
pub(crate) fn sorted_word_order(rows: &[Row<'_>]) -> Vec<u32> {
    let mut order: Vec<u32> = (0..rows.len() as u32).collect();
    order.sort_by(|&a, &b| rows[a as usize].word.cmp(rows[b as usize].word));
    order
}

/// One sanalista row, as borrowed slices of the source text.
pub(crate) struct Row<'a> {
    word: &'a str,
    homonymy: &'a str,
    pos: &'a str,
    /// Class in the high 16 bits, gradation code in the low 8.
    inflections: Vec<u32>,
}

/// Splits `text` into rows, keeping every field as a slice of `text`.
///
/// This mirrors `kotus::parse_sanalista` exactly -- same skip rules, same
/// inflection grammar -- because the generated tables and that function must
/// agree, or the embedded lexicon would differ from the parsed one.
pub(crate) fn parse_rows(text: &str) -> Vec<Row<'_>> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if n == 0 || line.trim().is_empty() {
            continue; // header row
        }
        let mut f = line.split('\t');
        let Some(word) = f.next() else { continue };
        let word = word.trim();
        if word.is_empty() || crate::rejected::contains(word) {
            continue;
        }
        let homonymy = f.next().unwrap_or("").trim();
        let pos = f.next().unwrap_or("").trim();
        let inflections = parse_inflections(f.next().unwrap_or("").trim());
        // Mirrors `kotus::parse_sanalista`: a class-100 row may name a
        // neighbouring class in `sanalista_overrides`.
        let inflections = if inflections.iter().all(|c| (*c >> 8) == 100) {
            match crate::sanalista_overrides::class_for(word) {
                Some(class) => parse_inflections(class),
                None => inflections,
            }
        } else {
            inflections
        };
        out.push(Row {
            word,
            homonymy,
            pos,
            inflections,
        });
    }
    out
}

/// Mirrors `kotus::parse_inflections`: a bracketed item beside an unbracketed one
/// is an alternant and is dropped, a field that is *only* a bracketed class is
/// read as that class, and only the first gradation letter of an item is kept.
fn parse_inflections(field: &str) -> Vec<u32> {
    let items: Vec<&str> = field
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let unbracketed_absent = !items.is_empty() && items.iter().all(|i| i.starts_with('('));
    items
        .into_iter()
        .filter_map(|item| {
            let item = if unbracketed_absent {
                item.trim_start_matches('(').trim_end_matches(')')
            } else {
                item
            };
            parse_single(item)
        })
        .collect()
}

/// Mirrors `kotus::parse_single`, returning the packed `u32` instead.
fn parse_single(item: &str) -> Option<u32> {
    let item = item.trim();
    // An item in parentheses is an alternant, not an additional class.
    let core = item.split('(').next().unwrap_or("").trim();
    if core.is_empty() {
        return None;
    }
    let digits: String = core.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let class: u32 = digits.parse().ok()?;
    let rest = &core[digits.len()..];
    if !rest.chars().all(|c| c == '*' || c.is_ascii_uppercase()) {
        return None;
    }
    // Only the first gradation letter counts, as in the parser, and it is stored
    // exactly as `Gradation::from_letter` numbers it: `A` = 1, so 0 is free to
    // mean "no gradation". `from_letter` accepts `A`..=`Z` even though only
    // `A`..=`M` have a defined pair, and this must not be stricter than the
    // parser or a letter the parser keeps would be dropped here.
    let gradation = rest
        .split('*')
        .skip(1)
        .find(|g| !g.is_empty())
        .and_then(|g| g.chars().next())
        .filter(char::is_ascii_uppercase)
        .map(|c| c as u32 - 'A' as u32 + 1) // 'A' -> 1
        .unwrap_or(0);
    Some((class << 8) | gradation)
}

