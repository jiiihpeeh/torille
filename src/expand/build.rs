use crate::kotus::Lemma;
use crate::spec::Catalogues;
use crate::tags::{Case, Number, PartOfSpeech, Tags};
use crate::wordset::WordSet;

use super::inflect_in;
use super::is_phonotactic;
use super::lexicon_index::LexiconIndex;

/// Accumulates the full form set of a word list.
#[derive(Debug, Default)]
pub struct FormBuilder {
    /// Everything that is accepted as a word.
    pub known: WordSet,
    /// Which of `known`'s words may be offered as a spelling correction, one
    /// bit per position in `known`.
    ///
    /// This used to be a second [`WordSet`]: 446 MB and a hash insert per form,
    /// to answer a question the position already answers. A word is
    /// suggestible only if it is known, and [`Self::accept`] hands back the
    /// position, so a bit is enough.
    suggestible: Vec<u64>,
    /// How many bits in [`Self::suggestible`] are set.
    suggestible_len: usize,
    /// Lemmas only, for head detection in compound analysis.
    pub lemmas: WordSet,
    /// The readings of every word in `known`, in the order the words went in.
    ///
    /// A word has as many readings as there are ways of being that word, and
    /// keeping them all is the point: `voi` is a noun of class 18, an
    /// interjection, and the first person present of `voida`, and which of
    /// those a sentence means is not a thing this crate can decide.
    ///
    /// This is a plain vector rather than a second word set, because
    /// `WordSet::position` hands out the insertion index for free and a second
    /// hash of every word in the language would double the cost of the one
    /// structure that has to fit in memory. The length of the vector is the
    /// length of `known` at all times: one entry goes in for every word that
    /// was not already there.
    pub analyses: Vec<Vec<Tags>>,
    /// The first base form of every word, in the order the words went in, as an
    /// index into `base_words` and the set of cases the word was reached in.
    ///
    /// This is the way back, and it is flat because a word has one base form
    /// almost always and a `Vec` per word would cost more in allocations than
    /// the pairs themselves: the language has millions of forms. The rest of
    /// the index answers "what can this word be", which is the question a
    /// sentence asks; this answers "what is this an inflected form of", which
    /// is the question a checker and a lemmatiser ask, and neither can be
    /// answered from the other without searching the whole language again.
    ///
    /// The length is the length of `known` at all times, for the same reason
    /// `analyses` is: one entry goes in for every word that was not already
    /// there.
    pub first_base: Vec<(u32, u16)>,
    /// The base forms of a word beyond its first, as the word's position, the
    /// base and the set of cases.
    ///
    /// A word is reached once for each lemma that makes it, and a word made by
    /// two lemmas has two bases. The first one to reach it takes the slot that
    /// keeps the array flat, and the others come here, because inserting into
    /// the middle of the first array would mean moving everything after it.
    ///
    /// This is a list rather than a second array indexed by word because a
    /// lookup here is a search through it, and a word asked about in a
    /// sentence is asked about once. A caller doing this per keystroke wants
    /// [`FormBuilder::build`] to hand over something sorted, and that is a
    /// change worth making when someone needs it.
    pub more_bases: Vec<(u32, u32, u16)>,
    /// `(word position, base id)` -> index into [`Self::more_bases`].
    ///
    /// The list is small (a couple of hundred thousand entries), but the build
    /// calls `add_base` once per word and a scan of the list is what made
    /// `merge` quadratic: 27 billion comparisons across the 96 shares. A hash
    /// probe keeps the merge linear and the list in its insertion order.
    more_index: std::collections::HashMap<(u32, u32), u32>,
    /// The base forms themselves, in the order they were first reached.
    ///
    /// A word is named here once however many forms of it the index holds, so
    /// the ids in `base_pairs` are indices into this rather than a copy of the
    /// string per form. `base_ids` is what turns a base word into one of those
    /// ids, and it is a hash rather than a search because a form is accepted
    /// for every lemma it belongs to and a search would be quadratic in the
    /// size of the language.
    pub base_words: Vec<String>,
    base_ids: std::collections::HashMap<String, u32>,
    /// The classes and parts of speech each base inflects as, so that a base
    /// can be mutated into the forms it makes and not only found behind them.
    ///
    /// One entry per base rather than per form, which is the whole point: the
    /// catalogue spells out eight forms of a class and the other six are made
    /// when they are wanted, so a base and its paradigm is the generator's work
    /// and not something the index has to have stored.
    pub base_classes: Vec<Vec<crate::kotus::Inflection>>,
    pub base_poses: Vec<Vec<PartOfSpeech>>,
    /// The class prior, so a word's bases can be tried commonest-first.
    ///
    /// `bases_of` reports the order the index reached the bases, which is the
    /// sanalista's order: `parin` is `par` (class 5) before `pari` (class 1).
    /// For a compound head the commoner reading is the right one
    /// (`nuorenparin` is `nuori` + `pari`), so the prior is kept here rather
    /// than rebuilt per lookup.
    prior: ClassPrior,
}

/// How many lemmas of a lexicon each class accounts for.
///
/// This is the prior that puts a word's base forms in the order a caller should
/// try them. Which words a word could be is a fact the index holds and is
/// reported in the order the words were reached, which is the sanalista's order
/// and nothing more than that. Which of them to try *first* is a guess, and a
/// guess has to be declared rather than left to whichever order the words
/// happened to go in. The size of a class is the one piece of evidence the
/// sanalista gives away for free: `talo` is class 1 and class 1 has thousands of
/// lemmas, `kaiku` is class 47 and class 47 has four, so a word that could be
/// either is likelier to be the first.
///
/// Nothing here is a word list and nothing is a hand-placed order. The counts
/// come from the lexicon the caller passes in, so a caller with a different
/// lexicon gets a different prior and the reasoning behind each still stands.
#[derive(Debug, Default, Clone)]
pub struct ClassPrior {
    /// (class, how many lemmas), by class number so a class is found by a
    /// search rather than a hash. There are a couple of hundred classes and a
    /// lookup per base of one word, so the simpler structure is the faster one.
    sizes: Vec<(crate::kotus::ClassId, usize)>,
}

impl ClassPrior {
    /// Counts the classes `lexicon` gives.
    ///
    /// A class that marks a word rather than declining it is not counted.
    /// [`crate::kotus::carries_a_model`] says why, and it matters more here than
    /// the size of any class: `99` is what 5 832 sanalista lemmas carry to say
    /// they are uninflected or only partly inflected, so a count that took `99`
    /// for a declension would make a pronoun look like the best explanation of
    /// any word it appears in.
    pub fn of(lexicon: &[Lemma]) -> Self {
        let mut counts: std::collections::HashMap<crate::kotus::ClassId, usize> =
            std::collections::HashMap::new();
        for lemma in lexicon {
            for inflection in &lemma.inflections {
                if crate::kotus::carries_a_model(inflection.class) {
                    *counts.entry(inflection.class).or_default() += 1;
                }
            }
        }
        let mut sizes: Vec<(crate::kotus::ClassId, usize)> = counts.into_iter().collect();
        sizes.sort_unstable();
        Self { sizes }
    }

    /// How many lemmas are in `class`.
    ///
    /// A class the lexicon never uses is zero rather than absent, so an unknown
    /// class cannot make its word look likelier than a known one by being
    /// missing from the comparison.
    pub fn size_of(&self, class: crate::kotus::ClassId) -> usize {
        self.sizes
            .binary_search_by_key(&class, |probe| probe.0)
            .map(|at| self.sizes[at].1)
            .unwrap_or(0)
    }

    /// The size of the commonest class among `inflections`, which is how a base
    /// is scored.
    ///
    /// A word the sanalista gives several classes to is scored by its commonest
    /// one. The question is which of a word's base forms is likelier, and the
    /// commonest class is the best evidence there is about the word as a whole:
    /// a lemma listed as `9, 50` is a class-9 word that happens to be a
    /// compound, not a class-50 word.
    ///
    /// A base with no class to score on is zero, and a base whose only classes
    /// are the markers `99`, `100` and `101` is zero too, since those say the
    /// sanalista has no model for the word rather than saying the word is a rare
    /// one. That is not a punishment: the sanalista splits no class across a
    /// compound it does not break up and gives a pronoun no class to give, so
    /// for those words there is no evidence to score on and the caller's own
    /// order is what is left.
    pub fn score(&self, inflections: &[crate::kotus::Inflection]) -> usize {
        inflections
            .iter()
            .filter(|i| crate::kotus::carries_a_model(i.class))
            .map(|i| self.size_of(i.class))
            .max()
            .unwrap_or(0)
    }
}

impl FormBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one lemma and, when it inflects, all of its forms.
    ///
    /// `lexicon` is the whole lemma list, which compound heads are looked up
    /// in. Pass an empty slice when a single lemma is all there is: the word
    /// itself is still added, only its compounds are missed.
    pub fn add(&mut self, lemma: &Lemma, cat: &Catalogues, lexicon: &LexiconIndex<'_>) {
        // What the lemma inflects as, kept so that the walk from a word back to
        // its base can be walked forwards again from there.
        match self.base_ids.get(lemma.word).copied() {
            Some(id) => {
                let id = id as usize;
                if self.base_classes[id].is_empty() {
                    self.base_classes[id] = lemma.inflections.clone();
                    self.base_poses[id] = PartOfSpeech::parse_all(lemma.part_of_speech);
                }
            }
            None => {
                self.base_classes.push(lemma.inflections.clone());
                self.base_poses
                    .push(PartOfSpeech::parse_all(lemma.part_of_speech));
            }
        }
        // The word goes into `known` through `accept` and nowhere else, since
        // `accept` is what keeps `analyses` the same length as `known` and an
        // insert here without a reading to go with it would leave the two out
        // of step.
        self.lemmas.insert(lemma.word);
        // A record can name more than one part of speech, and each of them is
        // generated in full: `aamen` is in the data as an adverb and a noun of
        // class 32, so it gets an adverb's worth of forms and a noun's worth
        // and the two are readings of the same word rather than one or the
        // other. The classes are the same for both, since the sanalista lists
        // them once, so the part of speech is the only thing that varies.
        for pos in PartOfSpeech::parse_all(lemma.part_of_speech) {
            // The word on the list is a form like any other. For a noun or an
            // adjective it is the nominative singular, which is what the bare
            // form of a word is. For a verb it is the infinitive, which has no
            // case at all, so it is recorded with the part of speech and
            // nothing else rather than being called a nominative it cannot be.
            let citation = match lemma.inflections.first() {
                Some(first) if crate::grammar::is_verb_class(first.class) => {
                    Tags::UNKNOWN.of_pos(pos)
                }
                // An indeclinable word is not a nominative singular, it is a
                // word with no case at all, and the only thing to say about it
                // is what it is.
                _ if lemma.is_indeclinable() => Tags::UNKNOWN.of_pos(pos),
                _ => Tags::of_number(Case::Nominative, Number::Singular).of_pos(pos),
            };
            let p = self.accept(lemma.word, lemma.word, &[citation]);
            self.mark_suggestible(p);
            for inflection in &lemma.inflections {
                // A class the sanalista gives no model for is skipped, because
                // there is nothing to inflect from -- unless the engine has a
                // paradigm of its own for it, which is the pronoun case: the
                // sanalista withholds a number from a pronoun because it is
                // irregular, and `class_101.rs` is that irregular paradigm.
                if !crate::kotus::carries_a_model(inflection.class)
                    && !crate::kotus::has_own_paradigm(inflection.class)
                    && !(inflection.class == 99
                        && (crate::expand::nominals::class_99::has_real_paradigm(lemma.word)
                            || crate::grammar::has_handwritten(lemma.word)))
                {
                    continue;
                }
                for form in inflect_in(
                    lemma.word,
                    *inflection,
                    cat,
                    Some(lexicon),
                    pos,
                    lemma.part_of_speech.contains("substantiivi"),
                ) {
                    let plain = form.plain() && is_phonotactic(&form.word);
                    let p = self.accept(&form.word, lemma.word, form.readings());
                    if plain {
                        self.mark_suggestible(p);
                    }
                }
            }
        }
    }

    /// Accepts a word and takes in the readings it was just reached with.
    ///
    /// A word is usually reached more than once, from its own lemma, from a
    /// class that happens to produce it, and from a homonym spelled the same
    /// way, and the readings are all kept rather than the last one winning.
    /// `voi` is reached three times over: as the noun of class 18, as the
    /// interjection that shares the spelling, and as the first person present
    /// of `voida`.
    fn accept(&mut self, word: &str, base: &str, readings: &[Tags]) -> usize {
        let next = self.base_words.len() as u32;
        let base_id = *self.base_ids.entry(base.to_string()).or_insert_with(|| {
            self.base_words.push(base.to_string());
            next
        });
        let cases: u16 = readings.iter().fold(0, |mask, r| mask | case_bit(r.case));
        let (is_new, p) = self.known.insert_at(word);
        if is_new {
            let mut slot: Vec<Tags> = Vec::with_capacity(readings.len());
            for reading in readings {
                crate::tags::add_reading(&mut slot, *reading);
            }
            self.analyses.push(slot);
            self.first_base.push((base_id, cases));
        } else {
            for reading in readings {
                crate::tags::add_reading(&mut self.analyses[p], *reading);
            }
            self.add_base(p, base_id, cases);
        }
        p
    }

    /// Marks the word at `position` as one a suggestion may offer.
    fn mark_suggestible(&mut self, position: usize) {
        let word = position / 64;
        if word >= self.suggestible.len() {
            self.suggestible.resize(word + 1, 0);
        }
        let bit = 1u64 << (position % 64);
        if self.suggestible[word] & bit == 0 {
            self.suggestible[word] |= bit;
            self.suggestible_len += 1;
        }
    }

    /// Whether the word at `position` is marked suggestible.
    fn suggestible_bit(&self, position: usize) -> bool {
        self.suggestible
            .get(position / 64)
            .is_some_and(|w| w & (1u64 << (position % 64)) != 0)
    }

    /// Whether `word` may be offered as a spelling correction.
    pub fn is_suggestible(&self, word: &str) -> bool {
        self.known
            .position(word)
            .is_some_and(|p| self.suggestible_bit(p))
    }

    /// How many of the known words may be offered as corrections.
    pub fn suggestible_len(&self) -> usize {
        self.suggestible_len
    }

    /// The base forms a word could be an inflected form of.
    ///
    /// This is the whole answer for a word and nothing more: `talojen` is a
    /// form of `talo`, and so is `taloja` if the word happens to be both, and
    /// which of the senses a sentence means is not decided here. A word that is
    /// not in the index has no base forms, and a base form is its own base.
    pub fn bases_of(&self, word: &str) -> Vec<&str> {
        self.bases_with_classes(word)
            .into_iter()
            .map(|(base, _)| base)
            .collect()
    }

    /// The one base to name for `word`, or `None` when it is not a known form.
    ///
    /// Two keys, in order: the commonest class ([`ClassPrior`]), then how much
    /// of the surface the base covers. The first key is the general one -- a
    /// word that could be class 1 or class 47 is likelier the class-1 word.
    /// The second breaks ties the class cannot: `parin` is both `pari` (class
    /// 5) and the golf term `par` (class 5), and `pari` + `n` covers more of
    /// the surface than `par` + `in`, which is the reading a compound head
    /// wants (`nuorenparin` is `nuori` + `pari`).
    pub fn best_base(&self, word: &str) -> Option<&str> {
        let mut best: Option<(&str, usize, usize)> = None;
        for (base, classes) in self.bases_with_classes(word) {
            let score = self.prior.score(classes);
            let prefix = common_prefix(word, base);
            let better = match best {
                None => true,
                Some((_, best_score, best_prefix)) => {
                    score > best_score || (score == best_score && prefix > best_prefix)
                }
            };
            if better {
                best = Some((base, score, prefix));
            }
        }
        best.map(|(base, _, _)| base)
    }

    /// The base forms a word is an inflected form of in the case given, in the
    /// order the index reached them.
    ///
    /// The case is what makes the answer an answer rather than a list: `voi` is
    /// a form of `voi` and of `voida`, but only the noun is in the nominative,
    /// since a verb has no case to be in. A `None` case asks for the words that
    /// have no case at all, which is a verb form, an indeclinable word and a
    /// bare verb stem among others.
    ///
    /// A base the word is a form of in two cases is named once, since the
    /// question is which words it could be and not how many ways.
    pub fn bases_in(&self, word: &str, case: Option<Case>) -> Vec<&str> {
        self.bases_in_with_classes(word, case)
            .into_iter()
            .map(|(base, _)| base)
            .collect()
    }

    /// The base forms of a word could be an inflected form of, and the classes
    /// each was reached under, in the order the index reached them.
    ///
    /// This is the half of [`FormBuilder::bases_of`] that a ranking needs and
    /// that a caller showing its working wants anyway: `talo` is class 1 and
    /// `kaiku` class 47, and that is not visible from the name.
    pub fn bases_with_classes(&self, word: &str) -> Vec<(&str, &[crate::kotus::Inflection])> {
        let Some(p) = self.known.position(word) else {
            return Vec::new();
        };
        let mut out: Vec<(&str, &[crate::kotus::Inflection])> = Vec::new();
        for id in self.base_ids_of(p) {
            let id = id as usize;
            let base = self.base_words[id].as_str();
            if out.iter().any(|(b, _)| *b == base) {
                continue;
            }
            out.push((base, &self.base_classes[id]));
        }
        out
    }

    /// The base forms a word is an inflected form of in the case given, with the
    /// classes each was reached under.
    ///
    /// The case is what makes the answer an answer rather than a list: `voi` is
    /// a form of `voi` and of `voida`, but only the noun is in the nominative,
    /// since a verb has no case to be in. A `None` case asks for the words that
    /// have no case at all, which is a verb form, an indeclinable word and a
    /// bare verb stem among others.
    ///
    /// A base the word is a form of in two cases is named once, since the
    /// question is which words it could be and not how many ways.
    pub fn bases_in_with_classes(
        &self,
        word: &str,
        case: Option<Case>,
    ) -> Vec<(&str, &[crate::kotus::Inflection])> {
        let Some(p) = self.known.position(word) else {
            return Vec::new();
        };
        let mut ids: Vec<u32> = Vec::new();
        let (first_id, first_cases) = self.first_base[p];
        if first_id != NO_BASE && case_mask_matches(first_cases, case) {
            ids.push(first_id);
        }
        for &(at, id, cases) in &self.more_bases {
            if at == p as u32 && case_mask_matches(cases, case) && !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.into_iter()
            .map(|id| {
                let id = id as usize;
                (self.base_words[id].as_str(), &self.base_classes[id][..])
            })
            .collect()
    }

    /// The base forms of a word could be an inflected form of, likelier first.
    ///
    /// [`FormBuilder::bases_of`] answers *which* words a word could be, in the
    /// order the index reached them. This answers which of them to try first,
    /// which is a different question and needs something the index does not
    /// have, so the caller supplies it as `prior` -- there is no default, since
    /// a default here would be a guess dressed as a fact.
    ///
    /// The sort is stable, so bases the prior cannot separate keep the order
    /// they were reached in. That is what happens to every base the sanalista
    /// gives no class to, and it is the right outcome rather than a fallback:
    /// the sanalista's order is then the only order there is.
    pub fn ranked_bases_of(&self, word: &str, prior: &ClassPrior) -> Vec<&str> {
        rank(self.bases_with_classes(word), prior)
    }

    /// The base forms of a word in the case given, likelier first.
    ///
    /// The two questions a caller has -- which words, and which first -- are
    /// both asked here rather than by a caller sorting one answer itself, so
    /// that the ranking is the same ranking whichever question it arrived with.
    pub fn ranked_bases_in(&self, word: &str, case: Option<Case>, prior: &ClassPrior) -> Vec<&str> {
        rank(self.bases_in_with_classes(word, case), prior)
    }

    /// The ids of every base form of a word, in the order they were reached.
    fn base_ids_of(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        let first = self.first_base[p].0;
        let word = p as u32;
        std::iter::once(first).chain(
            self.more_bases
                .iter()
                .filter(move |(at, _, _)| *at == word)
                .map(|(_, id, _)| *id),
        )
    }

    /// The readings of a word, if it is a word at all.
    ///
    /// This is the answer for a caller working on a sentence, since which
    /// reading is meant is decided by the rest of the sentence and not here.
    /// More than one is not an error, and an empty answer means the word is not
    /// in the index.
    pub fn readings_of(&self, word: &str) -> Option<&[Tags]> {
        self.known
            .position(word)
            .map(|p| self.analyses[p].as_slice())
    }

    /// What the readings of a word say between them.
    ///
    /// A dimension that comes out `None` is either one nothing ever claimed, as
    /// a verb has no case, or one the readings disagree about, and the list from
    /// [`FormBuilder::readings_of`] is what tells those apart.
    pub fn tags_of(&self, word: &str) -> Option<Tags> {
        self.readings_of(word).map(crate::tags::agreed)
    }

    /// The words whose readings say they are the given case, number and part of
    /// speech.
    ///
    /// A `None` in a filter means "any", so an empty one takes everything the
    /// index knows about. This walks the whole vector, which is fine for a
    /// query asked once and a bad idea per keystroke: the intended use is to
    /// narrow the word set to one reading before a suggestion search.
    pub fn words_matching(
        &self,
        case: Option<Case>,
        number: Option<Number>,
        pos: Option<PartOfSpeech>,
    ) -> impl Iterator<Item = &str> + '_ {
        let want = Tags {
            case,
            number,
            part_of_speech: pos,
            ..Tags::UNKNOWN
        };
        (0..self.analyses.len()).filter_map(move |p| {
            // Any reading will do: a word is in the answer if one of the ways
            // of being it fits, and the caller narrows it down afterwards.
            self.analyses[p]
                .iter()
                .any(|r| r.matches(&want))
                .then(|| self.known.word_at(p))
                .flatten()
        })
    }

    /// Every form a base makes, with the readings of each.
    ///
    /// This is the base being mutated rather than looked up: the catalogue
    /// spells out eight forms of a class and the remaining cases and the whole
    /// verb paradigm are generated here, so the paradigm of a base is a
    /// function of the base and the catalogue and is not held in the index.
    /// The forms come back with the confidence they were reached at, and a
    /// guessed one is a form a class might make rather than one it does.
    pub fn inflected(&self, base: &str, cat: &Catalogues) -> Vec<crate::tags::Form> {
        let Some(id) = self.base_ids.get(base) else {
            return Vec::new();
        };
        let id = *id as usize;
        let mut out = Vec::new();
        for inflection in &self.base_classes[id] {
            for pos in &self.base_poses[id] {
                out.extend(super::inflect(base, *inflection, cat, *pos, true));
            }
        }
        super::split::dedup(&mut out);
        out
    }

    /// The words a base makes that fit the tags asked for.
    ///
    /// This is the mutator, and it is asked in the whole of the tags rather
    /// than in a case alone, because that is what a person or a tense or a
    /// number is: `laskemme` is asked for by a first person plural present and
    /// not by a case at all, and a caller who has worked out that much from the
    /// sentence should not have to throw it away to use this.
    ///
    /// A `None` in `want` asks for anything, and a dimension no reading of a
    /// form settles is left open, so the answer is a superset of the forms that
    /// are certainly what was asked for. A form with several readings comes
    /// back if any one of them fits, and comes back once.
    pub fn forms_of(&self, base: &str, want: &Tags, cat: &Catalogues) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for form in self.inflected(base, cat) {
            if form.matches(want) && !out.contains(&form.word) {
                out.push(form.word.clone());
            }
        }
        out
    }

    /// The base forms a word could be, out of the ones whose readings fit.
    ///
    /// The way back out of [`FormBuilder::bases_in`], and the same superset
    /// rule: a word asked about with a person in it is a word at least one
    /// reading of which is that person, and the bases of its other readings
    /// come with it. Asking for a case rather than for the whole of the tags
    /// is [`FormBuilder::bases_in`], which is exact about the case, and this
    /// is for a caller that has more than a case to go on.
    pub fn bases_matching(&self, word: &str, want: &Tags) -> Vec<&str> {
        let Some(readings) = self.readings_of(word) else {
            return Vec::new();
        };
        if !readings.iter().any(|r| r.matches(want)) {
            return Vec::new();
        }
        self.bases_of(word)
    }

    /// The words a base makes in the case given.
    ///
    /// Unlike [`FormBuilder::forms_of`] this is exact about the case: a form
    /// comes back only if one of its readings is in that case, and a case of
    /// `None` asks for the forms that have no case at all, which is a verb form
    /// and an indeclinable word rather than a noun that failed to say so. A
    /// form is named once however many of its readings are in the case, since
    /// the question is which words a base can be put into a case as and not how
    /// many ways the form reads.
    pub fn forms_in(&self, base: &str, case: Option<Case>, cat: &Catalogues) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for form in self.inflected(base, cat) {
            let readings = form.readings();
            let fits = if case.is_some() {
                readings.iter().any(|r| r.case == case)
            } else {
                readings.iter().all(|r| r.case.is_none())
            };
            if fits && !out.contains(&form.word) {
                out.push(form.word.clone());
            }
        }
        out
    }

    /// Folds another builder's words into this one.
    ///
    /// Merging is what makes the threaded build below possible: every thread
    /// ends up with a whole index of its share of the language, and the shares
    /// are put together here. The words keep the readings of both halves, and
    /// the base forms of both, which is the point of holding readings at all: a
    /// word one thread reached as a noun and another as a verb is one word with
    /// two readings rather than whichever half was merged last.
    fn merge(&mut self, other: FormBuilder) {
        // Where each of the other's words has landed here. Inserting a word
        // that is already known does not move it, so the positions stay valid
        // as the loop goes on, and a word that is new has to have its place in
        // the two arrays beside `known` made for it before anything is said
        // about its readings.
        let mut where_from: Vec<u32> = Vec::with_capacity(other.known.len());
        let mut fresh: Vec<bool> = Vec::with_capacity(other.known.len());
        for word in other.known.iter() {
            let (is_new, position) = self.known.insert_at(word);
            where_from.push(position as u32);
            fresh.push(is_new);
            if is_new {
                self.analyses.push(Vec::new());
                // The base of a word nobody has named yet is filled in below,
                // once the other half's base for it is known.
                self.first_base.push((NO_BASE, 0));
            }
        }
        for (op, &p) in where_from.iter().enumerate() {
            if other.suggestible_bit(op) {
                self.mark_suggestible(p as usize);
            }
        }
        for word in other.lemmas.iter() {
            self.lemmas.insert(word);
        }
        // A word this half has not seen takes the other's reading list whole:
        // the `Vec` moves instead of being copied and then freed, which is what
        // made the merge spend as long releasing the shares as merging them.
        for (op, readings) in other.analyses.into_iter().enumerate() {
            let p = where_from[op] as usize;
            if fresh[op] {
                self.analyses[p] = readings;
            } else {
                for reading in readings {
                    crate::tags::add_reading(&mut self.analyses[p], reading);
                }
            }
        }
        // A base is named once here however many words reach it, and the names
        // from the other half go into the same table so that the ids of both
        // halves mean the same word.
        let mut base_of: Vec<u32> = Vec::with_capacity(other.base_words.len());
        for base in &other.base_words {
            base_of.push(self.base_id_of(base));
        }
        for (op, (id, cases)) in other.first_base.iter().enumerate() {
            let p = where_from[op] as usize;
            let id = base_of[*id as usize];
            if self.first_base[p].0 == NO_BASE {
                self.first_base[p] = (id, *cases);
            } else {
                self.add_base(p, id, *cases);
            }
        }
        for (at, id, cases) in other.more_bases {
            let p = where_from[at as usize] as usize;
            self.add_base(p, base_of[id as usize], cases);
        }
        // Each half numbered its bases in its own order, so `other`'s id `id`
        // is `self`'s id `base_of[id]`. Reading `self` at `id` would put one
        // half's classes on the other's words -- silently, since both tables
        // stay the same length as `base_words` -- and a base with another
        // base's classes is a base ranked by somebody else's class.
        for (id, classes) in other.base_classes.iter().enumerate() {
            let mine = base_of[id] as usize;
            if self.base_classes[mine].is_empty() {
                self.base_classes[mine] = classes.clone();
                self.base_poses[mine] = other.base_poses[id].clone();
            }
        }
    }

    /// Makes room for `words` more known words without rehashing or
    /// reallocating the tables that run beside `known`.
    ///
    /// The merge knows its final size (the sum of the shares), so it can pay
    /// for one table instead of the doubling chain. A rehash re-probes every
    /// word already in the set, and at 17M words that chain is seconds.
    pub fn reserve_words(&mut self, words: usize) {
        self.known.reserve(words);
        self.analyses.reserve(words);
        self.first_base.reserve(words);
        self.more_bases.reserve(words / 64);
        self.more_index.reserve(words / 64);
        self.suggestible.reserve(words.div_ceil(64));
    }

    /// Records that a word is also a form of a base, in the cases given.
    fn add_base(&mut self, p: usize, id: u32, cases: u16) {
        if self.first_base[p].0 == id {
            self.first_base[p].1 |= cases;
            return;
        }
        let key = (p as u32, id);
        match self.more_index.get(&key) {
            Some(&at) => self.more_bases[at as usize].2 |= cases,
            None => {
                let at = self.more_bases.len() as u32;
                self.more_bases.push((key.0, key.1, cases));
                self.more_index.insert(key, at);
            }
        }
    }

    /// The id of a base form, naming it if this is the first time it has been seen.
    fn base_id_of(&mut self, base: &str) -> u32 {
        let next = self.base_words.len() as u32;
        *self.base_ids.entry(base.to_string()).or_insert_with(|| {
            self.base_words.push(base.to_string());
            self.base_classes.push(Vec::new());
            self.base_poses.push(Vec::new());
            next
        })
    }

    /// Reports the number of words a lemma list yields, for logging.
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.known.len(), self.suggestible_len, self.lemmas.len())
    }
}

/// The base of a word that has not been named yet, which is a moment in a merge
/// and never a state the index is left in.
const NO_BASE: u32 = u32::MAX;

/// How many bytes `word` and `base` share from the start.
fn common_prefix(word: &str, base: &str) -> usize {
    word.bytes()
        .zip(base.bytes())
        .take_while(|(a, b)| a == b)
        .count()
}

/// Puts the commonest class first, leaving everything the prior cannot separate
/// in the order it arrived.
///
/// The sort is stable and the key is reversed, so `talo` (class 1, thousands of
/// lemmas) comes before `kaiku` (class 47, a handful) while two bases the prior
/// scores the same keep the index's order, which is the sanalista's. Both halves
/// of that are the point: the ranking is a stated guess and it is the *only*
/// guess, so a caller can turn it off and get the sanalista's answer back.
fn rank<'a>(
    bases: Vec<(&'a str, &'a [crate::kotus::Inflection])>,
    prior: &ClassPrior,
) -> Vec<&'a str> {
    let mut scored: Vec<(usize, &str)> = bases
        .into_iter()
        .map(|(base, classes)| (prior.score(classes), base))
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, base)| base).collect()
}

/// The bit a case occupies in the set of cases a form is in.
///
/// A word with no case is in no case at all, which is the empty set, so that
/// asking about a case and asking about no case are different questions and
/// neither of them gets the other's answer.
fn case_bit(case: Option<Case>) -> u16 {
    match case {
        Some(case) => 1 << (case as u8),
        None => 0,
    }
}

/// Whether a form that is in the cases of `mask` answers a query for `case`.
///
/// A `Some` case is a membership question: the form must be in that case. The
/// empty mask is not a wildcard, though, because a form with no case is in no
/// case at all, so a `None` query matches exactly the forms whose mask is
/// empty. `voi` is a noun in the nominative and a verb in no case: asking for
/// no case must give the verb and not the noun.
fn case_mask_matches(mask: u16, case: Option<Case>) -> bool {
    match case {
        Some(case) => mask & (1 << (case as u8)) == (1 << (case as u8)),
        None => mask == 0,
    }
}

/// Builds the full form set from parsed sanalista records, on every core.
///
/// The lemmas are cut into one share per thread, each share is built into a
/// builder of its own, and the shares are merged. The cut is by lemma and not
/// by form because a lemma is what a thread needs whole: a form is generated
/// from the lemma it belongs to, and the lexicon every thread is handed is the
/// whole list so that a compound head is found wherever it is.
///
/// One core does the whole language in a few seconds, which is long enough to
/// be felt every time a checker starts, and this is the difference between
/// that and a moment. The shares are uneven because the lemmas are not of a
/// length, so a thread that draws long words is the one that takes longest;
/// the work is split into more shares than there are threads and taken in
/// order, which keeps the idle time down without a work queue to write.
pub fn build(lemmas: &[Lemma], cat: &Catalogues) -> FormBuilder {
    // One index of the lexicon, shared by every thread. Compound splitting
    // needs it on every share, and building one per thread would cost 12 passes
    // over the sanalista to avoid 12 scans inside them -- which is the wrong
    // way round: the whole point of the index is that the scans are expensive.
    let lexicon_index = LexiconIndex::new(lemmas);
    let lexicon_index = &lexicon_index;
    // The same worker count the reverse walk uses: `TORILLE_SINGLE_THREAD`
    // and `set_single_thread(true)` give the build the wasm's shape, so
    // `perf_index --single-thread` measures what the extension pays.
    let threads = super::find_bases::worker_count().min(lemmas.len()).max(1);
    if threads == 1 {
        let mut b = FormBuilder::new();
        for lemma in lemmas {
            b.add(lemma, cat, lexicon_index);
        }
        b.prior = ClassPrior::of(lemmas);
        return b;
    }
    // A share is a multiple of a thousand lemmas or so, which is small enough
    // that a dozen threads are not all waiting on the longest compounds and
    // large enough that the merge is not done hundreds of times.
    let share = (lemmas.len() / (threads * 8)).max(256);
    let shares: Vec<&[Lemma]> = lemmas.chunks(share).collect();
    let built: Vec<FormBuilder> = std::thread::scope(|scope| {
        let handles: Vec<_> = shares
            .iter()
            .map(|chunk| {
                scope.spawn(move || {
                    let mut b = FormBuilder::new();
                    for lemma in chunk.iter() {
                        b.add(lemma, cat, lexicon_index);
                    }
                    b
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut b = FormBuilder::new();
    b.reserve_words(built.iter().map(|s| s.known.len()).sum());
    for share in built {
        b.merge(share);
    }
    b.prior = ClassPrior::of(lemmas);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotus::{Inflection, Lemma};

    fn cat() -> Catalogues {
        Catalogues::embedded().clone()
    }

    /// A lexicon of `(word, classes)` pairs, which is all of a `Lemma` the
    /// reverse index reads: the word it reached a form from and the classes it
    /// reached it under.
    fn lexicon(rows: &[(&'static str, &[u16])]) -> Vec<Lemma<'static>> {
        rows.iter()
            .map(|(word, classes)| Lemma {
                word,
                homonymy: "",
                part_of_speech: "substantiivi",
                inflections: classes
                    .iter()
                    .map(|class| Inflection {
                        class: *class,
                        gradation: None,
                    })
                    .collect(),
            })
            .collect()
    }

    /// The whole sanalista, which is what a prior meant for the language rather
    /// than for a test is counted over.
    fn whole_lexicon() -> Vec<Lemma<'static>> {
        crate::kotus::sanalista().to_lemmas()
    }

    /// A word generated from a lemma is a form of that lemma. This is the whole
    /// point of the index, and the direction everything else here reports in.
    #[test]
    fn a_generated_form_finds_its_lemma() {
        // The classes are the sanalista's, which is the only thing that says them.
        let lex = lexicon(&[("talo", &[1]), ("kala", &[9]), ("lapsi", &[29])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        for (form, base) in [
            ("talojen", "talo"),
            ("talossa", "talo"),
            ("kalassa", "kala"),
            ("kalasta", "kala"),
            ("kaloissa", "kala"),
            ("lasten", "lapsi"),
            ("lapsia", "lapsi"),
        ] {
            assert_eq!(b.bases_of(form), vec![base], "{form}");
        }
    }

    /// A word the index has never heard of has no base forms rather than a wrong
    /// one, and a lemma is a form of itself, which is what lets a caller ask
    /// about any word without first knowing whether it is a word.
    #[test]
    fn an_unknown_word_has_no_base_forms() {
        let lex = lexicon(&[("talo", &[1])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        assert_eq!(b.bases_of("talo"), vec!["talo"], "a lemma is its own base");
        assert_eq!(b.bases_of("taloja"), vec!["talo"]);
        assert!(b.bases_of("kirjoittamaton").is_empty(), "a non-word");
    }

    /// The prior is a count over the lexicon and not a hand-placed order:
    /// `talo` is class 1, which the sanalista gives thousands of lemmas, and
    /// `kaiku` is class 47, which it gives a handful of. A word that could be
    /// either is likelier to be the first.
    #[test]
    fn the_prior_counts_the_classes_the_sanalista_uses() {
        let prior = ClassPrior::of(&whole_lexicon());
        assert!(
            prior.size_of(1) > 1_000,
            "class 1 is the commonest class there is"
        );
        assert!(
            prior.size_of(1) > prior.size_of(47),
            "class 47 is a small class"
        );
        assert_eq!(prior.size_of(9_999), 0, "a class nobody uses scores zero");
    }

    /// A repaired form is accepted but never offered as a correction.
    ///
    /// `väki` (class 7*D) has the genitive plural `väkien` beside the
    /// over-complete `väkein`; the first is plain, the second repaired. The
    /// index marks only the plain one suggestible.
    #[test]
    fn a_repaired_form_is_accepted_but_not_suggestible() {
        let cat = cat();
        let lemma = Lemma {
            word: "väki",
            homonymy: "",
            part_of_speech: "substantiivi",
            inflections: crate::kotus::parse_inflections("7*D"),
        };
        let b = build(&[lemma], &cat);
        assert!(b.known.contains("väkien"), "the plain form is a word");
        assert!(b.known.contains("väkein"), "the repaired form is a word");
        assert!(
            b.is_suggestible("väkien"),
            "the plain form is offered as a correction"
        );
        assert!(
            !b.is_suggestible("väkein"),
            "a repaired form is never offered as a correction"
        );
    }

    /// Naming one base uses the prior, then how much of the surface the base
    /// covers.
    ///
    /// `par` and `pari` are both class 5, so the prior cannot separate them.
    /// `parin` is `pari` + `n` before it is `par` + `in`, and the longer cover
    /// is the reading a compound head wants (`nuorenparin` is `nuori` +
    /// `pari`).
    #[test]
    fn the_best_base_prefers_the_reading_that_covers_the_surface() {
        let lex = lexicon(&[("par", &[5]), ("pari", &[5])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        assert_eq!(
            b.bases_of("parin"),
            vec!["par", "pari"],
            "the order reached"
        );
        assert_eq!(b.best_base("parin"), Some("pari"), "the longer cover");
    }

    /// A word with two base forms is a form of both, and the two orders answer
    /// two different questions: which words, and which first. Ranking must not
    /// lose a base to gain an order.
    #[test]
    fn ranking_reorders_the_bases_without_losing_any() {
        let lex = lexicon(&[("talo", &[1]), ("kaiku", &[47])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        // A word both lemmas reach, reached by the rarer class first so that the
        // two orders cannot come out the same by accident.
        let word = "kalastaja";
        b.accept(word, "kaiku", &[Tags::UNKNOWN]);
        b.accept(word, "talo", &[Tags::UNKNOWN]);
        assert_eq!(b.bases_of(word), vec!["kaiku", "talo"], "the order reached");
        let prior = ClassPrior::of(&whole_lexicon());
        assert_eq!(
            b.ranked_bases_of(word, &prior),
            vec!["talo", "kaiku"],
            "the commoner class first"
        );
    }

    /// Bases the prior scores level keep the order the index reached them in,
    /// which is the sanalista's. A prior counted over a lexicon that has neither
    /// class scores both zero, which is the case that has to hold.
    #[test]
    fn a_level_score_keeps_the_sanalista_order() {
        let lex = lexicon(&[("talo", &[1]), ("kaiku", &[47])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        let word = "kalastaja";
        b.accept(word, "kaiku", &[Tags::UNKNOWN]);
        b.accept(word, "talo", &[Tags::UNKNOWN]);
        let flat = ClassPrior::of(&[]);
        assert_eq!(b.ranked_bases_of(word, &flat), vec!["kaiku", "talo"]);
    }

    /// A word with several classes is scored by its commonest one, because a
    /// lemma the sanalista lists as `9, 50` is a class-9 word that happens to be
    /// a compound rather than a class-50 word.
    #[test]
    fn a_word_with_several_classes_is_scored_by_its_commonest() {
        let prior = ClassPrior::of(&whole_lexicon());
        let nine = vec![Inflection {
            class: 9,
            gradation: None,
        }];
        let fifty = vec![Inflection {
            class: 50,
            gradation: None,
        }];
        let both = vec![
            Inflection {
                class: 50,
                gradation: None,
            },
            Inflection {
                class: 9,
                gradation: None,
            },
        ];
        assert_eq!(prior.score(&both), prior.score(&nine));
        assert!(prior.score(&nine) > prior.score(&fifty));
        assert_eq!(prior.score(&[]), 0, "a word with no class scores zero");
    }

    /// The index is built on every core and the halves are merged, and a merge
    /// has to line the halves' base tables up with each other: each half
    /// numbered its own bases in its own order, so a base's classes have to
    /// follow the base across rather than keep the number it had.
    ///
    /// Both halves are built here by hand rather than through [`build`], because
    /// a test of a merge should not depend on how many cores the machine has.
    /// The second half is the smaller one on purpose: that is the shape in which
    /// the two id spaces disagree, since one base numbered 0 arrives here as
    /// number 2.
    #[test]
    fn a_merge_keeps_each_base_with_its_own_classes() {
        let cat = cat();
        let first = lexicon(&[("talo", &[1]), ("kala", &[9])]);
        let second = lexicon(&[("lapsi", &[29])]);
        let mut left = FormBuilder::new();
        for lemma in &first {
            left.add(lemma, &cat, &LexiconIndex::new(&first));
        }
        let mut right = FormBuilder::new();
        for lemma in &second {
            right.add(lemma, &cat, &LexiconIndex::new(&second));
        }
        let mut whole = FormBuilder::new();
        whole.merge(left);
        whole.merge(right);
        let classes_of = |word: &str| {
            whole
                .bases_with_classes(word)
                .into_iter()
                .map(|(base, classes)| (base.to_string(), classes.to_vec()))
                .collect::<Vec<_>>()
        };
        for (form, base, class) in [
            ("talojen", "talo", 1),
            ("kalassa", "kala", 9),
            ("lapsessa", "lapsi", 29),
        ] {
            assert_eq!(
                classes_of(form),
                vec![(
                    base.to_string(),
                    vec![Inflection {
                        class,
                        gradation: None
                    }]
                )],
                "{form}"
            );
        }
        assert_eq!(
            whole.bases_of("lapsessa"),
            vec!["lapsi"],
            "a base keeps its own spelling across a merge"
        );
    }

    /// `99`, `100` and `101` say the sanalista has no model for the word rather
    /// than saying the word is a rare one, and there are more of them than there
    /// are lemmas in any real class. A count that took one for a declension
    /// would rank a pronoun above the noun it is a form of.
    #[test]
    fn a_class_that_marks_a_word_is_not_a_class_to_score_on() {
        let prior = ClassPrior::of(&whole_lexicon());
        for marker in [99, 100, 101] {
            assert_eq!(prior.size_of(marker), 0, "class {marker} marks a word");
        }
        let pronoun = vec![Inflection {
            class: 99,
            gradation: None,
        }];
        let noun = vec![Inflection {
            class: 1,
            gradation: None,
        }];
        assert_eq!(prior.score(&pronoun), 0, "no model, nothing to score");
        assert!(prior.score(&noun) > 0, "a noun has a model and a class");
    }

    /// The case is what makes the answer an answer rather than a list: a word that
    /// is a noun form and a verb form at once is only the noun in a case, since a
    /// verb has no case to be in.
    #[test]
    fn a_case_filter_keeps_the_readings_that_are_in_that_case() {
        let lex = lexicon(&[("talo", &[1]), ("kaiku", &[47])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        let word = "kalastaja";
        b.accept(
            word,
            "talo",
            &[Tags::of_number(Case::Genitive, Number::Plural)],
        );
        b.accept(word, "kaiku", &[Tags::UNKNOWN]);
        assert_eq!(b.bases_in(word, Some(Case::Genitive)), vec!["talo"]);
        assert_eq!(
            b.bases_in(word, Some(Case::Allative)),
            Vec::<&str>::new(),
            "the noun is not in the allative"
        );
        assert_eq!(
            b.bases_in(word, None),
            vec!["kaiku"],
            "a word with no case is in no case, which is a different question"
        );
    }

    /// The sanalista withholds a number from a pronoun because it is irregular,
    /// which is the one reason a model cannot be stated for it, and
    /// `class_101.rs` is that irregular paradigm written out by hand. So a
    /// pronoun's forms are indexed even though `101` carries no model: `hänen` is
    /// a form the engine produces, and an index that cannot find it is wrong in
    /// the way that is easiest to miss -- the caller is told the word is not in
    /// the index at all.
    #[test]
    fn a_pronouns_forms_are_indexed_though_it_carries_no_model() {
        assert!(!crate::kotus::carries_a_model(101));
        let lex = lexicon(&[("hän", &[101]), ("minä", &[101])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        for (form, base) in [
            ("hänen", "hän"),
            ("hänessä", "hän"),
            ("häntä", "hän"),
            ("minun", "minä"),
            ("minussa", "minä"),
        ] {
            assert_eq!(b.bases_of(form), vec![base], "{form}");
        }
    }

    /// The other side of the same line. `99` is withheld because the word does
    /// not decline, and `class_99.rs` runs the `-i` loan machinery over every
    /// class-99 word outside its own list of indeclinables -- right for `miss`
    /// and `seesam`, which the reference confirms as loans, and a guess for the
    /// other 5 714 of them. Those guessed forms stay out of the index, or it
    /// would answer `ainutien` with a lemma rather than with nothing.
    #[test]
    fn a_class_99_words_guessed_forms_stay_out_of_the_index() {
        assert!(!crate::kotus::carries_a_model(99));
        assert!(!crate::kotus::has_own_paradigm(99));
        let lex = lexicon(&[("ainut", &[99])]);
        let cat = cat();
        let mut b = FormBuilder::new();
        for lemma in &lex {
            b.add(lemma, &cat, &LexiconIndex::new(&lex));
        }
        assert_eq!(
            b.bases_of("ainut"),
            vec!["ainut"],
            "a lemma is its own base"
        );
        for guess in ["ainutien", "ainuteiden", "ainutilla"] {
            assert!(
                b.bases_of(guess).is_empty(),
                "{guess} is a guess, and the index says so"
            );
        }
    }
}
