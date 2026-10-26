//! Class 1*C (the hattu type) -- class 1 with a consonant-geminate gradation.
//!
//! The weak stem reads the *final* consonant cluster once when it is a geminate
//! -- `tt` before the last vowel here, `hattu` -> `hatu` -- by the rule `1*B`
//! uses for `pp` and `kk`. The reading is the same rule, so the two modules say
//! the same thing and differ only in their name.
//!
//! Which cells take which grade is [`super::gradated`], shared by all eleven of
//! these classes.

#![allow(dead_code)]

use crate::tags::Form;

use super::gradated;

pub const CLASS: &str = "1*C";

/// The weak stem: the final consonant cluster is read once when it is a
/// geminate, else the lemma is its own weak stem (makkara's last cluster
/// `r` is not a geminate, so it stays).
fn weak_stem(lemma: &str) -> String {
    let ch: Vec<char> = lemma.chars().collect();
    let n = ch.len();
    if n == 0 {
        return lemma.to_string();
    }
    let is_v = |c: char| "aeiouyäö".contains(c);
    let Some(lv) = (0..n).rev().find(|&i| is_v(ch[i])) else {
        return lemma.to_string();
    };
    // Run of identical consonants immediately before the last vowel.
    let mut start = lv;
    while start > 0 && !is_v(ch[start - 1]) {
        start -= 1;
        if start < lv && ch[start] != ch[lv - 1] {
            start += 1;
            break;
        }
    }
    let run = &ch[start..lv];
    if run.len() >= 2 && run.iter().all(|&c| c == run[0]) {
        let mut out: String = ch[..start].iter().collect();
        out.push(run[0]);
        out.extend(ch[lv..].iter());
        out
    } else {
        lemma.to_string()
    }
}

/// Inflect a class-1*C noun.
///
/// The grid is [`super::gradated`]: every cell of the paradigm, in the order
/// this class emits them. What is left here is the one thing that tells
/// `1*C` from its ten siblings.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    gradated::inflect(lemma, possessives, &weak_stem(lemma))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// The class gradates in exactly the way its code says, which is the whole
    /// of what this module adds to [`super::gradated`]: `aatto` is `aaton` in the
    /// genitive but `aattoni` for the first person, so the possessive sits on the
    /// strong stem while the bare form does not.
    ///
    /// All three spellings are what the kotus reference records for `aatto`, so
    /// this fails if the gradation code is read wrongly as well as if the grid
    /// is.
    #[test]
    fn the_crossover_holds() {
        let w = words("aatto");
        assert!(w.contains(&"aaton".to_string()), "genitive bare is weak");
        assert!(w.contains(&"aattoni".to_string()), "genitive -ni is strong");
    }
}
