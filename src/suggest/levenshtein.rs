//! Bounded Levenshtein distance and the keyboard-adjacency tie-break.
//!
//! Extracted from `suggest.rs`; the public API is re-exported there so
//! `torille::suggest::{levenshtein_with_ops, EditOp}` keeps working.

/// Bounded Levenshtein distance. Returns `max_distance + 1` (i.e., "too
/// far") as soon as the running minimum exceeds `max_distance`, so the
/// function is `O(min(|a|, |b|) * max_distance)` rather than `O(|a|*|b|)`.
///
/// Hot path inside `suggest`: every misspelled word runs `suggest()` against
/// the entire lexicon, so each call's allocations are visible. The classic
/// implementation allocates a `Vec<char>` per side plus two `Vec<usize>`
/// rows; for a 30-character Finnish word that is four heap allocations
/// per edit-distance computation, repeated for every candidate. We instead
/// pull the chars into stack-allocated small buffers whenever the words
/// fit and only fall to the heap for the long-tail case.
pub fn levenshtein(a: &str, b: &str, max_distance: usize) -> usize {
    if a == b {
        return 0;
    }
    // The cap is `max_distance + 1` -- one beyond the budget. When the caller
    // passes `usize::MAX` we want the unbounded answer, not a wraparound to 0.
    let cap = max_distance.saturating_add(1);

    // Quick length-only filter: any edit must change the length by at most
    // `max_distance`, so a difference larger than that lets us bail without
    // the DP.
    let len_diff = if a.len() >= b.len() {
        a.len() - b.len()
    } else {
        b.len() - a.len()
    };
    // Note: byte-length, not char-length. A `len_diff` based on chars would
    // be tighter for Finnish (where `ä` is 2 bytes); for the bound this is
    // still safe because char-length-distance <= byte-length-distance.
    if len_diff > max_distance {
        return cap;
    }

    // Most Finnish words fit in 64 chars; stack-allocate the buffers.
    // Longer words fall through to the heap path. Three rows are needed
    // for the Damerau transposition predecessor (`dp[i-2][j-2] + 1`).
    const STACK_CHARS: usize = 64;
    let mut a_stack = [0u32; STACK_CHARS];
    let mut b_stack = [0u32; STACK_CHARS];
    let mut prev_prev_stack = [0usize; STACK_CHARS + 1];
    let mut prev_stack = [0usize; STACK_CHARS + 1];
    let mut curr_stack = [0usize; STACK_CHARS + 1];

    if a.len() <= STACK_CHARS && b.len() <= STACK_CHARS {
        let n = decode_into(a, &mut a_stack);
        let m = decode_into(b, &mut b_stack);
        levenshtein_dp(
            &a_stack[..n],
            &b_stack[..m],
            &mut prev_prev_stack[..m + 1],
            &mut prev_stack[..m + 1],
            &mut curr_stack[..m + 1],
            max_distance,
            cap,
        )
    } else {
        let a_heap: Vec<u32> = a.chars().map(|c| c as u32).collect();
        let b_heap: Vec<u32> = b.chars().map(|c| c as u32).collect();
        let m = b_heap.len();
        let mut prev_prev_heap: Vec<usize> = (0..=m).collect();
        let mut prev_heap: Vec<usize> = (0..=m).collect();
        let mut curr_heap: Vec<usize> = vec![0; m + 1];
        levenshtein_dp(
            &a_heap,
            &b_heap,
            prev_prev_heap.as_mut_slice(),
            prev_heap.as_mut_slice(),
            curr_heap.as_mut_slice(),
            max_distance,
            cap,
        )
    }
}

/// The two-row DP, plus a third row for the Damerau transposition
/// predecessor (`dp[i-2][j-2] + 1`). The caller provides all three
/// buffers; this function does not allocate.
///
/// The bail-out is the same one standard Levenshtein uses: once the
/// row's minimum exceeds `max_distance`, no later row can bring the
/// answer back inside. The transposition adds a 4th predecessor that
/// can lower individual cells by 1, but the row-min stays bounded
/// `row_min(i) ∈ [row_min(i-1), row_min(i-1) + 1]`, so the early-exit
/// is still sound -- it may bail one row later than the standard case,
/// never earlier.
#[inline]
fn levenshtein_dp<'a>(
    a: &[u32],
    b: &[u32],
    mut prev_prev_row: &'a mut [usize],
    mut prev_row: &'a mut [usize],
    mut curr_row: &'a mut [usize],
    max_distance: usize,
    cap: usize,
) -> usize {
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m.min(cap);
    }
    if m == 0 {
        return n.min(cap);
    }
    // Initial row: cost of editing the empty a-prefix into each b-prefix
    // is `b.len()` insertions.
    for (j, slot) in prev_row.iter_mut().enumerate() {
        *slot = j;
    }
    // Mirror the initial row into `prev_prev_row` so the transposition at
    // `i = 2` can read `dp[0][j-2] = j - 2`. The value is overwritten
    // before being read again.
    prev_prev_row.copy_from_slice(prev_row);
    for i in 1..=n {
        curr_row[0] = i;
        let mut row_min = curr_row[0];
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let mut v = (prev_row[j] + 1)
                .min(curr_row[j - 1] + 1)
                .min(prev_row[j - 1] + cost);
            // Damerau transposition: if the last two source chars and the
            // last two target chars are a swap, this cell can also be
            // reached in one edit from `dp[i-2][j-2]`. `a[i-1] != a[i-2]`
            // rules out a degenerate case where the "swap" is a no-op.
            if i >= 2
                && j >= 2
                && a[i - 1] == b[j - 2]
                && a[i - 2] == b[j - 1]
                && a[i - 1] != a[i - 2]
            {
                v = v.min(prev_prev_row[j - 2] + 1);
            }
            curr_row[j] = v;
            if v < row_min {
                row_min = v;
            }
        }
        // Bail out: if every cell in this row already exceeds the budget,
        // no further row can bring the answer back inside.
        if row_min > max_distance {
            return cap;
        }
        // Rotate: prev_prev_row <- prev_row, then prev_row <- curr_row.
        // Swapping the slice references is safe and avoids a copy.
        std::mem::swap(&mut prev_prev_row, &mut prev_row);
        std::mem::swap(&mut prev_row, &mut curr_row);
    }
    prev_row[m].min(cap)
}

/// Decode `s` into `dst` (one `u32` per char -- we use `u32` instead of
/// `char` so the buffer is `Copy`, which makes it easier to thread into
/// the heap-allocated path without an extra copy). Returns the number of
/// chars written. `s.len()` must be `<= dst.len()`.
#[inline]
fn decode_into(s: &str, dst: &mut [u32]) -> usize {
    let mut i = 0;
    for ch in s.chars() {
        dst[i] = ch as u32;
        i += 1;
    }
    i
}

/// One edit step in a Levenshtein alignment. Pair with [`levenshtein_with_ops`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOp {
    /// The two chars at this position are equal; not an edit.
    Match,
    /// A substitution: `from` in the source was rewritten to `to`.
    Substitute { from: char, to: char },
    /// A character was inserted in the candidate at this position.
    Insert(char),
    /// A character was deleted from the source at this position.
    Delete(char),
    /// Two adjacent source characters were swapped (not standard Levenshtein
    /// but a common extension; reported only when both chars differ and the
    /// upstream DP table says `dp[i][j] == dp[i-2][j-2] + 1`).
    Transpose { a: char, b: char },
}

/// Levenshtein distance + the full edit script. Use for training data
/// generation where we want the operation *type*, not just the count.
///
/// This allocates a full `(n+1) x (m+1)` matrix, so it is slower than
/// [`levenshtein`]. Reserve it for offline data work.
pub fn levenshtein_with_ops(a: &str, b: &str) -> (usize, Vec<EditOp>) {
    let mut a_buf = [0u32; 256];
    let mut b_buf = [0u32; 256];
    let n = decode_into(a, &mut a_buf);
    let m = decode_into(b, &mut b_buf);
    levenshtein_with_ops_u32(&a_buf[..n], &b_buf[..m])
}

fn levenshtein_with_ops_u32(a: &[u32], b: &[u32]) -> (usize, Vec<EditOp>) {
    let n = a.len();
    let m = b.len();
    if n == 0 {
        let ops = b
            .iter()
            .map(|&c| EditOp::Insert(char::from_u32(c).unwrap_or('\u{fffd}')))
            .collect();
        return (m, ops);
    }
    if m == 0 {
        let ops = a
            .iter()
            .map(|&c| EditOp::Delete(char::from_u32(c).unwrap_or('\u{fffd}')))
            .collect();
        return (n, ops);
    }

    // dp[i][j] = min edits to turn a[..i] into b[..j]. Includes the
    // Damerau transposition predecessor (Optimal String Alignment: no
    // overlapping transpositions), so a swap like "teh" -> "the" is
    // distance 1, not 2.
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        dp[i][0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let mut v = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
            if i >= 2
                && j >= 2
                && a[i - 1] == b[j - 2]
                && a[i - 2] == b[j - 1]
                && a[i - 1] != a[i - 2]
            {
                v = v.min(dp[i - 2][j - 2] + 1);
            }
            dp[i][j] = v;
        }
    }

    // Backtrace, preferring Match > Transpose > Substitute > Delete > Insert
    // when more than one predecessor ties. The tie-breaking is the part of
    // the algorithm that determines the actual edit script; the *distance*
    // is the same regardless.
    let mut ops = Vec::with_capacity(dp[n][m]);
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a[i - 1] == b[j - 1] && dp[i][j] == dp[i - 1][j - 1] {
            ops.push(EditOp::Match);
            i -= 1;
            j -= 1;
        } else if i >= 2
            && j >= 2
            && a[i - 1] == b[j - 2]
            && a[i - 2] == b[j - 1]
            && a[i - 1] != a[i - 2]
            && dp[i][j] == dp[i - 2][j - 2] + 1
        {
            ops.push(EditOp::Transpose {
                a: char::from_u32(a[i - 2]).unwrap_or('\u{fffd}'),
                b: char::from_u32(a[i - 1]).unwrap_or('\u{fffd}'),
            });
            i -= 2;
            j -= 2;
        } else if i > 0 && j > 0 && dp[i][j] == dp[i - 1][j - 1] + 1 {
            ops.push(EditOp::Substitute {
                from: char::from_u32(a[i - 1]).unwrap_or('\u{fffd}'),
                to: char::from_u32(b[j - 1]).unwrap_or('\u{fffd}'),
            });
            i -= 1;
            j -= 1;
        } else if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            ops.push(EditOp::Delete(
                char::from_u32(a[i - 1]).unwrap_or('\u{fffd}'),
            ));
            i -= 1;
        } else {
            ops.push(EditOp::Insert(
                char::from_u32(b[j - 1]).unwrap_or('\u{fffd}'),
            ));
            j -= 1;
        }
    }
    ops.reverse();
    (dp[n][m], ops)
}

/// QWERTY adjacency for the keyboard-neighbour tie-break bonus. The
/// list is the immediate four-way neighbour where the layout is
/// unambiguous; ties (e.g. `o` could call either `i` or `p` its closest
/// neighbour) are resolved to the most common one. The shape is
/// conservative on purpose -- off-by-one neighbours produce distractors
/// that are too random for a spelling tie-break.
///
/// Lower case only. The caller is responsible for normalising the
/// input; [`is_keyboard_adjacent`] lower-cases its arguments before
/// the lookup, so `Ä` / `Ö` / `Å` resolve through the lowercase row.
pub(super) const KEYBOARD_ADJACENT: &[(&str, &[char])] = &[
    ("a", &['s', 'q', 'w', 'z', 'ä']),
    ("b", &['v', 'n', 'g', 'h']),
    ("c", &['x', 'v', 'd', 'f']),
    ("d", &['s', 'f', 'e', 'r', 'x', 'c']),
    ("e", &['w', 'r', 'd', 's']),
    ("f", &['d', 'g', 'r', 't', 'c', 'v']),
    ("g", &['f', 'h', 't', 'y', 'v', 'b']),
    ("h", &['g', 'j', 'y', 'u', 'b', 'n']),
    ("i", &['u', 'o', 'j', 'k']),
    ("j", &['h', 'k', 'u', 'i', 'n', 'm']),
    ("k", &['j', 'l', 'i', 'o', 'm']),
    ("l", &['k', 'o', 'p', 'ö', 'å']),
    ("m", &['n', 'j', 'k']),
    ("n", &['b', 'm', 'h', 'j']),
    ("o", &['i', 'p', 'l', 'k', 'ö']),
    ("p", &['o', 'l', 'å']),
    ("q", &['a', 'w']),
    ("r", &['e', 't', 'd', 'f']),
    ("s", &['a', 'd', 'w', 'e', 'z', 'x']),
    ("t", &['r', 'y', 'f', 'g']),
    ("u", &['y', 'i', 'h', 'j']),
    ("v", &['c', 'b', 'f', 'g']),
    ("w", &['q', 'e', 'a', 's']),
    ("x", &['z', 'c', 's', 'd']),
    ("y", &['t', 'u', 'g', 'h']),
    ("z", &['x', 's']),
    ("å", &['p', 'l', 'ö']),
    ("ä", &['a', 'ö']),
    ("ö", &['o', 'ä', 'p', 'l', 'å']),
];

/// Whether `to` is a QWERTY neighbour of `from`. ASCII-uppercase letters
/// are lower-cased before the lookup. The Finnish special vowels `ä` /
/// `ö` / `å` are looked up directly; a candidate that swaps a Latin
/// letter for one of those is not flagged as adjacent.
pub(super) fn is_keyboard_adjacent(from: char, to: char) -> bool {
    let from = from.to_ascii_lowercase();
    let to = to.to_ascii_lowercase();
    KEYBOARD_ADJACENT
        .iter()
        .find(|(key, _)| key.starts_with(from))
        .is_some_and(|(_, neighbours)| neighbours.contains(&to))
}

/// The keyboard-adjacency tie-break bonus. Returns a small positive
/// score for a distance-1 single-substitution between QWERTY neighbours,
/// and 0 otherwise.
///
/// Single-substitution only: insertions and deletions shift the
/// position-by-position alignment the function uses, so they are
/// deliberately not scored. Multi-substitution typos (distance >= 2
/// with two or more differing positions) are also not scored -- the
/// tie-break is meaningful precisely when two candidates share an edit
/// distance, and a single-substitution typo is the common case where
/// the ranking signal is most useful.
pub(super) fn keyboard_bonus(input: &str, candidate: &str) -> f32 {
    if input.chars().count() != candidate.chars().count() {
        return 0.0;
    }
    let mut diff: Option<(char, char)> = None;
    for (a, b) in input.chars().zip(candidate.chars()) {
        if a != b {
            if diff.is_some() {
                return 0.0;
            }
            diff = Some((a, b));
        }
    }
    if let Some((from, to)) = diff
        && is_keyboard_adjacent(from, to)
    {
        return KEYBOARD_ADJACENT_BONUS;
    }
    0.0
}

/// The tie-break weight for a keyboard-adjacent single-substitution
/// typo. Small enough that a non-adjacent distance-1 candidate at the
/// same edit distance still ranks below a distance-0 exact match (1.0
/// vs 0.5 + 0.05 = 0.55), but large enough to break alphabetical ties
/// between a keyboard typo and a far-apart typo.
pub(super) const KEYBOARD_ADJACENT_BONUS: f32 = 0.05;
