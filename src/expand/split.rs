use super::Generated;

/// Sorts, deduplicates and prunes a finished form list.
///
/// A word reached twice is kept once, with the union of what the two ways of
/// reaching it said about it, and with the better of the two confidences. The
/// order is most trustworthy first, so the first of a pair of same words is the
/// one worth keeping and the union of the tags goes with it.
///
/// The union is what makes a word like `hän` work, where the same form is a
/// nominative and a genitive of the same lemma and an illative of a different
/// one: the case is dropped, since the readings disagree about it, and the
/// number and the person are kept where they agree.
pub(super) fn dedup(out: &mut Vec<Generated>) {
    // Word order first, so that the two readings of a word are neighbours and
    // can be merged. Sorting by confidence first would put them far apart, one
    // in the plain group and one in the repaired, and the merge would miss both.
    // The order for use is the confidence order and comes last, once there is
    // one of everything.
    out.sort_by(|a, b| a.word.cmp(&b.word));
    // An explicit loop rather than `dedup_by`, which hands the closure the two
    // elements in the opposite order to the slice and drops the first of them
    // when the closure says so: everything merged into it goes straight out of
    // the window with it.
    let mut merged: Vec<Generated> = Vec::with_capacity(out.len());
    for form in out.drain(..) {
        match merged.last_mut() {
            Some(last) if last.word == form.word => last.absorb(&form),
            _ => merged.push(form),
        }
    }
    *out = merged;
    out.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then_with(|| a.word.cmp(&b.word))
    });
}
