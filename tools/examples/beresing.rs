// SPDX-License-Identifier: MIT OR Apache-2.0
//! Print the suggestion lists for a handful of clitic and negative forms at
//! several distance bounds. A small manual probe for `suggest`; run with
//! `cargo run -p torille-tools --example beresing`.

use torille::suggest::{is_known_word, suggest};

fn main() {
    let cases = [
        "näetkös",
        "katsos",
        "hyppääs",
        "mennääns",
        "joos",
        "näetkö",
        "katsokaa",
        "hyppää",
        "mennään",
    ];
    for w in cases {
        let known = is_known_word(w);
        let sugg1 = suggest(w, 8, 1);
        let sugg2 = suggest(w, 8, 2);
        let sugg3 = suggest(w, 8, 3);
        println!("{w}: known={known:?}");
        println!("  suggest dist=1: {sugg1:?}");
        println!("  suggest dist=2: {sugg2:?}");
        println!("  suggest dist=3: {sugg3:?}");
    }
}
