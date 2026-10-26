//! Class 33*D (the puin type) inflection, written by hand.
//!
//! A graded class-33 subtype whose oblique stem inserts a `k` before the final
//! vowel (`puin -> pukime`, `pyyhin -> pyyhkime`). It delegates to the shared
//! class-33 graded engine.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "33*D";
fn estem(lemma: &str) -> String {
    let mut base = lemma.to_string();
    base.pop(); // drop the final n
    let chars: Vec<char> = base.chars().collect();
    let mut li = 0usize;
    for (i, c) in chars.iter().enumerate() {
        if "aeiouyäö".contains(*c) { li = i; }
    }
    let mut out: String = chars[..li].iter().collect();
    out.push('k');
    out.push(chars[li]);
    out.push('m');
    out.push('e');
    out
}
/// Inflect a class 33*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_33::inflect_graded(lemma, possessives, &estem(lemma))
}
