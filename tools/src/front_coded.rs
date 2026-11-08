// SPDX-License-Identifier: MIT OR Apache-2.0
//! Front-coded map writer, matching `src/expand/static_table.rs`.
//!
//! The build script has its own copy for the compile-time maps; this one lets
//! `gen_derived` write an engine-computed table in the same compact format.
//! Layout: 40-byte header, restart table, front-coded keys, one `u32` id per
//! key, a deduplicated string pool, and its offset table.

use std::path::Path;

fn write_varint(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Write `entries` (sorted by key, unique keys) as one `MST2` map blob.
pub fn write_map(blob_path: &Path, entries: &[(String, Vec<String>)]) {
    const RESTART: usize = 16;

    let mut keys: Vec<u8> = Vec::new();
    let mut restarts: Vec<u32> = Vec::new();
    let mut prev = String::new();
    for (i, (key, _)) in entries.iter().enumerate() {
        if i % RESTART == 0 {
            restarts.push(keys.len() as u32);
            write_varint(0, &mut keys);
            write_varint(key.len() as u32, &mut keys);
            keys.extend_from_slice(key.as_bytes());
        } else {
            let common = prev
                .bytes()
                .zip(key.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            write_varint(common as u32, &mut keys);
            write_varint((key.len() - common) as u32, &mut keys);
            keys.extend_from_slice(&key.as_bytes()[common..]);
        }
        prev.clear();
        prev.push_str(key);
    }

    let mut pool: Vec<&str> = entries
        .iter()
        .flat_map(|(_, values)| values.iter().map(String::as_str))
        .collect();
    pool.sort_unstable();
    pool.dedup();
    let id_of = |value: &str| pool.binary_search(&value).unwrap() as u32;

    const MULTI: u32 = 0x8000_0000;
    let mut ids: Vec<u32> = Vec::with_capacity(entries.len());
    let mut overflow: Vec<u8> = Vec::new();
    for (_, values) in entries {
        if values.len() == 1 {
            ids.push(id_of(&values[0]));
        } else {
            let offset = overflow.len() as u32;
            debug_assert!(values.len() < u8::MAX as usize);
            overflow.push(values.len() as u8);
            for value in values {
                overflow.extend_from_slice(&id_of(value).to_le_bytes());
            }
            ids.push(MULTI | offset);
        }
    }

    let mut string_offsets: Vec<u8> = Vec::with_capacity((pool.len() + 1) * 4);
    let mut strings: Vec<u8> = Vec::new();
    for value in &pool {
        string_offsets.extend_from_slice(&(strings.len() as u32).to_le_bytes());
        strings.extend_from_slice(value.as_bytes());
    }
    string_offsets.extend_from_slice(&(strings.len() as u32).to_le_bytes());

    const HEADER: usize = 40;
    const MAGIC: u32 = 0x3254_534D; // "MST2"
    let restarts_off = HEADER;
    let keys_off = restarts_off + restarts.len() * 4;
    let ids_off = keys_off + keys.len();
    let overflow_off = ids_off + ids.len() * 4;
    let string_offsets_off = overflow_off + overflow.len();
    let strings_off = string_offsets_off + string_offsets.len();

    let mut blob: Vec<u8> = Vec::with_capacity(strings_off + strings.len());
    blob.extend_from_slice(&MAGIC.to_le_bytes());
    blob.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    blob.extend_from_slice(&(restarts.len() as u32).to_le_bytes());
    blob.extend_from_slice(&(restarts_off as u32).to_le_bytes());
    blob.extend_from_slice(&(keys_off as u32).to_le_bytes());
    blob.extend_from_slice(&(ids_off as u32).to_le_bytes());
    blob.extend_from_slice(&(overflow_off as u32).to_le_bytes());
    blob.extend_from_slice(&(string_offsets_off as u32).to_le_bytes());
    blob.extend_from_slice(&(pool.len() as u32).to_le_bytes());
    blob.extend_from_slice(&(strings_off as u32).to_le_bytes());
    for restart in &restarts {
        blob.extend_from_slice(&restart.to_le_bytes());
    }
    blob.extend_from_slice(&keys);
    for id in &ids {
        blob.extend_from_slice(&id.to_le_bytes());
    }
    blob.extend_from_slice(&overflow);
    blob.extend_from_slice(&string_offsets);
    blob.extend_from_slice(&strings);

    crate::gzip::write(blob_path, &blob);
}
