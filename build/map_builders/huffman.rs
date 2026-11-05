//! Static-Huffman key-set emitter (`HST1`).
//!
//! Split out of `build.rs`; `emit_proper_names` calls the one public entry
//! point.

use std::fs;
use std::path::Path;

use crate::{human_bytes, write_varint};

/// Emit a sorted key list as an `HST1` static-Huffman set.
///
/// The payload is the same front-coded byte stream `emit_front_coded_keys`
/// writes, then entropy-coded. The distribution is strongly skewed (varint
/// prefix/length bytes plus Finnish case endings), so a canonical Huffman
/// code takes `proper_names.bin` from 1.65 MB to about 1.0 MB. Code lengths
/// are limited to 12 bits with package-merge, which lets the reader use a
/// direct 4096-entry lookup table instead of a bit-by-bit tree walk.
///
/// The completed blob is decoded back key-for-key before it is written; a
/// builder/reader mismatch is a build error, not a runtime one.
pub(crate) fn emit_huffman_set(blob_path: &Path, keys: &[String]) {
    const RESTART: usize = 16;
    const MAX_CODE_LEN: usize = 12;
    const HEADER: usize = 40;

    // 1. Front-code the keys (same record shape as `emit_front_coded_keys`).
    let mut stream: Vec<u8> = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    let mut prev = String::new();
    for (i, key) in keys.iter().enumerate() {
        assert!(
            key.len() <= 128,
            "build.rs: key longer than the decoder's buffer: {key:?}"
        );
        if i % RESTART == 0 {
            starts.push(stream.len());
            write_varint(0, &mut stream);
            write_varint(key.len() as u32, &mut stream);
            stream.extend_from_slice(key.as_bytes());
        } else {
            let common = prev
                .bytes()
                .zip(key.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            write_varint(common as u32, &mut stream);
            write_varint((key.len() - common) as u32, &mut stream);
            stream.extend_from_slice(&key.as_bytes()[common..]);
        }
        prev.clear();
        prev.push_str(key);
    }

    // 2. Code lengths and canonical codes.
    let mut freq = [0u64; 256];
    for &byte in &stream {
        freq[byte as usize] += 1;
    }
    let lengths = package_merge_lengths(&freq, MAX_CODE_LEN);

    let mut by_len: Vec<Vec<u8>> = vec![Vec::new(); MAX_CODE_LEN + 1];
    for symbol in 0..256u16 {
        let len = lengths[symbol as usize] as usize;
        if len > 0 {
            by_len[len].push(symbol as u8);
        }
    }
    let mut codes = [0u32; 256];
    let mut code = 0u32;
    for bucket in by_len.iter().skip(1) {
        for &symbol in bucket {
            codes[symbol as usize] = code;
            code += 1;
        }
        code <<= 1;
    }

    // Direct decode table: 12 bits -> (symbol << 4) | code length.
    let mut table = vec![0u16; 1 << MAX_CODE_LEN];
    for symbol in 0..256u16 {
        let len = lengths[symbol as usize] as usize;
        if len == 0 {
            continue;
        }
        let span = 1usize << (MAX_CODE_LEN - len);
        let start = (codes[symbol as usize] as usize) << (MAX_CODE_LEN - len);
        for slot in &mut table[start..start + span] {
            *slot = (symbol << 4) | len as u16;
        }
    }

    // 3. Encode, recording each block's bit offset.
    let mut restarts = Vec::with_capacity(starts.len());
    let mut writer = BitWriter::default();
    let mut bit_count = 0usize;
    let mut next_start = 0;
    for (i, &byte) in stream.iter().enumerate() {
        while next_start < starts.len() && starts[next_start] == i {
            restarts.push(bit_count as u32);
            next_start += 1;
        }
        let len = lengths[byte as usize];
        let code = codes[byte as usize];
        for shift in (0..len).rev() {
            writer.bit((code >> shift) & 1);
        }
        bit_count += len as usize;
    }
    while next_start < starts.len() {
        restarts.push(bit_count as u32);
        next_start += 1;
    }

    // 4. Header, restart table, decode table, bit stream.
    let restarts_off = HEADER;
    let table_off = restarts_off + restarts.len() * 4;
    let bits_off = table_off + table.len() * 2;
    let packed = writer.finish();
    let mut blob: Vec<u8> = Vec::with_capacity(bits_off + packed.len());
    blob.extend_from_slice(&0x3154_5348u32.to_le_bytes()); // "HST1"
    blob.extend_from_slice(&(keys.len() as u32).to_le_bytes());
    blob.extend_from_slice(&(restarts.len() as u32).to_le_bytes());
    blob.extend_from_slice(&(restarts_off as u32).to_le_bytes());
    blob.extend_from_slice(&(table_off as u32).to_le_bytes());
    blob.extend_from_slice(&(bits_off as u32).to_le_bytes());
    blob.extend_from_slice(&(MAX_CODE_LEN as u32).to_le_bytes());
    for _ in 0..3 {
        blob.extend_from_slice(&0u32.to_le_bytes());
    }
    for restart in &restarts {
        blob.extend_from_slice(&restart.to_le_bytes());
    }
    for entry in &table {
        blob.extend_from_slice(&entry.to_le_bytes());
    }
    blob.extend_from_slice(&packed);

    // 5. Prove the reader can recover every key before writing.
    verify_huffman_set(&blob, keys);

    eprintln!(
        "build.rs: {}: {} keys -> {} blob ({} stream bytes)",
        blob_path.file_name().unwrap().to_string_lossy(),
        keys.len(),
        human_bytes(blob.len()),
        human_bytes(stream.len()),
    );
    fs::write(blob_path, &blob).unwrap_or_else(|e| panic!("{}: {e}", blob_path.display()));
}

/// Length-limited Huffman code lengths via package-merge.
///
/// `freq[s]` is the byte frequency; symbols with frequency 0 get length 0.
/// The result is a complete prefix code with `max_len` as the longest code.
fn package_merge_lengths(freq: &[u64; 256], max_len: usize) -> [u8; 256] {
    let mut lengths = [0u8; 256];
    let mut leaves: Vec<(u64, u16)> = (0..256u16)
        .filter(|&symbol| freq[symbol as usize] > 0)
        .map(|symbol| (freq[symbol as usize], symbol))
        .collect();
    leaves.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let n = leaves.len();
    if n == 0 {
        return lengths;
    }
    if n == 1 {
        lengths[leaves[0].1 as usize] = 1;
        return lengths;
    }

    #[derive(Clone, Copy)]
    struct Item {
        weight: u64,
        first: u16,
        mask: [u64; 4],
    }
    fn leaf(weight: u64, symbol: u16) -> Item {
        let mut mask = [0u64; 4];
        mask[symbol as usize / 64] |= 1 << (symbol as usize % 64);
        Item {
            weight,
            first: symbol,
            mask,
        }
    }
    fn merge(a: &Item, b: &Item) -> Item {
        Item {
            weight: a.weight + b.weight,
            first: a.first.min(b.first),
            mask: [
                a.mask[0] | b.mask[0],
                a.mask[1] | b.mask[1],
                a.mask[2] | b.mask[2],
                a.mask[3] | b.mask[3],
            ],
        }
    }

    let leaves: Vec<Item> = leaves
        .iter()
        .map(|&(weight, symbol)| leaf(weight, symbol))
        .collect();
    let mut list = leaves.clone();
    for _ in 1..max_len {
        let mut packages = Vec::with_capacity(list.len() / 2);
        for pair in list.as_chunks::<2>().0 {
            packages.push(merge(&pair[0], &pair[1]));
        }
        let mut merged = Vec::with_capacity(leaves.len() + packages.len());
        merged.extend_from_slice(&leaves);
        merged.extend_from_slice(&packages);
        merged.sort_by(|a, b| a.weight.cmp(&b.weight).then(a.first.cmp(&b.first)));
        list = merged;
    }

    let take = (2 * n - 2).min(list.len());
    for item in &list[..take] {
        for (symbol, count) in lengths.iter_mut().enumerate().take(256) {
            if item.mask[symbol / 64] & (1u64 << (symbol % 64)) != 0 {
                *count += 1;
            }
        }
    }

    let kraft: u64 = (0..256)
        .filter(|&symbol| lengths[symbol] > 0)
        .map(|symbol| 1u64 << (max_len - lengths[symbol] as usize))
        .sum();
    assert_eq!(
        kraft,
        1u64 << max_len,
        "build.rs: package-merge produced an incomplete code"
    );
    lengths
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    filled: u8,
}

impl BitWriter {
    fn bit(&mut self, bit: u32) {
        self.current = (self.current << 1) | (bit & 1) as u8;
        self.filled += 1;
        if self.filled == 8 {
            self.bytes.push(self.current);
            self.current = 0;
            self.filled = 0;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.filled > 0 {
            self.bytes.push(self.current << (8 - self.filled));
        }
        self.bytes
    }
}

/// Decode a completed `HST1` blob and compare every key. This mirrors
/// `expand::huffman_set::HuffmanSet`, deliberately: the build fails if the
/// two ever disagree.
fn verify_huffman_set(blob: &[u8], keys: &[String]) {
    let read_u32 = |offset: usize| {
        u32::from_le_bytes([
            blob[offset],
            blob[offset + 1],
            blob[offset + 2],
            blob[offset + 3],
        ])
    };
    let read_u16 = |offset: usize| u16::from_le_bytes([blob[offset], blob[offset + 1]]);

    assert_eq!(&blob[0..4], &0x3154_5348u32.to_le_bytes(), "HST1 magic");
    let key_count = read_u32(4) as usize;
    let restart_count = read_u32(8) as usize;
    let restarts_off = read_u32(12) as usize;
    let table_off = read_u32(16) as usize;
    let bits_off = read_u32(20) as usize;
    assert_eq!(key_count, keys.len());

    let symbol = |bit: &mut usize| -> u8 {
        let byte = *bit >> 3;
        let shift = *bit & 7;
        let at = bits_off + byte;
        let b0 = *blob.get(at).unwrap_or(&0) as u32;
        let b1 = *blob.get(at + 1).unwrap_or(&0) as u32;
        let b2 = *blob.get(at + 2).unwrap_or(&0) as u32;
        let window = (b0 << 16) | (b1 << 8) | b2;
        let index = ((window >> (24 - MAX_CODE_LEN - shift)) & 0xFFF) as usize;
        let entry = read_u16(table_off + index * 2);
        let len = (entry & 0xF) as usize;
        assert!(len != 0, "HST1: zero-length code");
        *bit += len;
        (entry >> 4) as u8
    };
    let varint = |bit: &mut usize| -> u32 {
        let mut value = 0u32;
        let mut shift = 0;
        loop {
            let byte = symbol(bit);
            value |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    };

    let mut decoded = 0usize;
    for block in 0..restart_count {
        let mut bit = read_u32(restarts_off + block * 4) as usize;
        let mut key: Vec<u8> = Vec::new();
        for _ in 0..16 {
            if decoded >= key_count {
                break;
            }
            let common = varint(&mut bit) as usize;
            let suffix = varint(&mut bit) as usize;
            key.truncate(common);
            for _ in 0..suffix {
                key.push(symbol(&mut bit));
            }
            assert_eq!(
                key,
                keys[decoded].as_bytes(),
                "HST1 round-trip mismatch at key {decoded}"
            );
            decoded += 1;
        }
    }
    assert_eq!(decoded, key_count, "HST1 round-trip stopped short");
}

const MAX_CODE_LEN: usize = 12;

