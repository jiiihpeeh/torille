//! Class 33*D (the puin type) inflection, written by hand.
//!
//! A graded class-33 subtype whose oblique stem inserts a `k` before the final
//! vowel (`puin -> pukime`, `pyyhin -> pyyhkime`). It delegates to the shared
//! class-33 graded engine.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "33*D";
fn estem(lemma: &str) -> Option<String> {
    let mut base = lemma.to_string();
    base.pop(); // drop the final n
    let chars: Vec<char> = base.chars().collect();
    // The stem grows from the last vowel (`puin -> pukime`). A surface with
    // no vowel before the final `n` is not a member of the class.
    let li = chars.iter().rposition(|c| "aeiouyäö".contains(*c))?;
    let mut out: String = chars[..li].iter().collect();
    out.push('k');
    out.push(chars[li]);
    out.push('m');
    out.push('e');
    Some(out)
}
/// Inflect a class 33*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let Some(estem) = estem(lemma) else {
        return Vec::new();
    };
    super::class_33::inflect_graded(lemma, possessives, &estem)
}
