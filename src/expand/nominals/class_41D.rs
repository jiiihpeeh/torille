//! Class 41*D (a graded 41 subtype) inflection, written by hand.
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
/// Inflect a class 41*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut base = lemma.to_string();
    base.pop();
    let last = base.chars().last().unwrap();
    let body = &base[..base.len() - base.chars().last().unwrap().len_utf8()];
    let short = format!("{}k{last}", body);
    let long = super::class_41::long_stem(&short);
    let mut forms = super::class_41::a_stem_inflect(lemma, possessives, &long, &short, true, Some(Gradation(4)));
    // `ruis` takes the `-isiin` plural illative (`rukiisiin`); the shared
    // 41 grid's other spelling (`rukiihin`) is not a form of this word.
    if lemma == "ruis" {
        let bad = format!("{short}ih");
        forms.retain(|f| !f.word.starts_with(&bad));
    }
    forms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruis_takes_the_isiin_plural_illative() {
        let w: Vec<String> = inflect("ruis", true).into_iter().map(|f| f.word).collect();
        assert!(w.contains(&"rukiisiin".to_string()));
        assert!(!w.contains(&"rukiihin".to_string()));
    }
}
