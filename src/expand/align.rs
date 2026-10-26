use crate::is_vowel;

/// Longest common prefix of the model and a form, capped at the model's length.
pub(super) fn split_point(model: &str, variant: &str) -> Option<usize> {
    let cap = model.len().min(variant.len());
    let n = crate::spec::lcp_len(model, variant).min(cap);
    Some(n)
}

/// The consonant substitution a class makes in its stem, read off the pair of
/// the model word and its genitive.
///
/// `lapsi` and `lasta` share `las` and then differ, `i` against `ta`, so the
/// `p` of the model becomes the `t` of the genitive. `kaksi` and `kahden` share
/// `ka` and differ in a consonant, so the model consonant becomes the genitive
/// one. `laatikko` and `laatikon` are not a substitution at all but a halved
/// consonant, which the doubled character guards against.
pub(super) fn stem_alternation(model: &str, genitive: &str) -> Option<(usize, char, char)> {
    let l = crate::spec::lcp_len(model, genitive)
        .min(model.len())
        .min(genitive.len());
    let model_rest = &model[l..];
    let gen_rest = &genitive[l..];
    let model_c = model_rest.chars().next();
    let gen_c = gen_rest.chars().next();
    let (Some(model_c), Some(gen_c)) = (model_c, gen_c) else {
        return None;
    };
    if model_c == gen_c {
        return None;
    }
    let before = model[..l].chars().last();
    // Two of the same character here is a consonant that halved, which
    // `halve_doubled` deals with rather than a substitution.
    if before == Some(model_c) {
        return None;
    }
    let (at, from) = if !is_vowel(model_c) && !is_vowel(gen_c) {
        (l, model_c)
    } else if is_vowel(model_c) && !is_vowel(gen_c) {
        // The shared prefix reached the model's own final vowel, so the change
        // is in the consonant before it: `vanhemp` and `isoäit` both end here.
        let at = model[..l].char_indices().last().map(|(i, _)| i)?;
        (at, model[..at].chars().last()?)
    } else {
        return None;
    };
    Some((at, from, gen_c))
}

/// The byte index of the last occurrence of `needle` in `haystack`.
pub(super) fn find_last(haystack: &str, needle: &str) -> Option<usize> {
    haystack.rfind(needle)
}
