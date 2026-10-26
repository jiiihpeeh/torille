//! What a generated word is, grammatically.
//!
//! Every form the crate generates is a word that somebody might reasonably
//! have written, and knowing *which* word it is meant to be is what a checker
//! needs and what a plain set of strings cannot say. `lasta` on its own is a
//! form that may be accepted or rejected; `lasta` as the partitive singular of
//! `lapsi` can be explained, and a correction can be offered for a different
//! case of the same lemma. A noun and a verb take the same tags with different
//! values, and most tags are absent for most words, so every dimension is
//! optional rather than a sentinel.
//!
//! The dimensions are the ones Finnish actually distinguishes. A tag is a
//! claim about one form, never about a word in general: `talossa` is inessive
//! singular, `taloissa` inessive plural, and both are the same word with two
//! readings rather than one reading of each.
//!
//! A word can be more than one form at once, and a checker working on a whole
//! sentence is in the position to say which: `hän` is the nominative of one
//! lemma and the genitive of another, `maa` is two words spelled alike, and
//! which reading is meant follows from what the rest of the sentence says. So
//! the readings are kept as a list on [`Form`] rather than merged into one, and
//! a caller narrows them with [`Form::readings_matching`].
//!
//! The union is still there for the questions that want a single answer, such
//! as what two homonyms agree on, and the difference is what an absent dimension
//! means. In one reading it means the word has no such dimension, as a Finnish
//! verb has no case; across several readings it can also mean they disagree.
//! [`Form::is_ambiguous`] is what tells the two apart.

/// Adds a reading to a list, keeping only the fullest of any that nest.
///
/// A reading nobody else makes is a reading, and a reading somebody already
/// made in fuller is that same reading written down smaller, so the fuller one
/// is kept and the smaller one goes. This is the one place that decides it, and
/// [`Form::add_reading`] and the index both come through here.
pub fn add_reading(readings: &mut Vec<Tags>, tags: Tags) {
    if readings
        .iter()
        .any(|r| r.subsumes(&tags) && !tags.subsumes(r))
    {
        return;
    }
    readings.retain(|r| !tags.subsumes(r));
    readings.push(tags);
}

/// What a set of readings of one word agree on, in a single set of tags.
///
/// A dimension the readings disagree about is `None`, since no value of it is
/// true of all of them, and so is one that none of them claims. This is not the
/// pairwise [`Tags::unite`], which is what two readings of one word are for:
/// there a `None` means that reading says nothing and lets the other one speak
/// for the pair. Folding `unite` over many readings instead would let a
/// disagreement be overwritten by whichever reading came last, and `voi` would
/// come back a verb rather than a word the readings cannot agree is a verb,
/// a noun or an interjection.
pub fn agreed(readings: &[Tags]) -> Tags {
    /// The one value every reading that claims this dimension gives, if any.
    fn agree<T: PartialEq + Copy>(claims: impl Iterator<Item = Option<T>>) -> Option<T> {
        let mut agreed = None;
        for claim in claims.flatten() {
            match agreed {
                None => agreed = Some(claim),
                Some(seen) if seen == claim => {}
                Some(_) => return None,
            }
        }
        agreed
    }
    if readings.is_empty() {
        return Tags::UNKNOWN;
    }
    Tags {
        part_of_speech: agree(readings.iter().map(|r| r.part_of_speech)),
        case: agree(readings.iter().map(|r| r.case)),
        number: agree(readings.iter().map(|r| r.number)),
        person: agree(readings.iter().map(|r| r.person)),
        tense: agree(readings.iter().map(|r| r.tense)),
        mood: agree(readings.iter().map(|r| r.mood)),
        voice: agree(readings.iter().map(|r| r.voice)),
        possessive: agree(readings.iter().map(|r| r.possessive)),
        clitic: agree(readings.iter().map(|r| r.clitic)),
        polarity: agree(readings.iter().map(|r| Some(r.polarity))).unwrap_or_default(),
    }
}

/// A Finnish case.
///
/// Sixteen, which is the usual count with the prolative and the instructive
/// added to the fifteen that are counted in a school grammar. The instructive
/// is not really a case, since `-n` is a plural ending and the case marking is
/// only on the participle, but the checker wants to name the thing it matched
/// and `Instructive` names it honestly.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Case {
    /// `-n` or nothing, the subject of a sentence: `lapsi`.
    Nominative = 0,
    /// The nominative or the genitive, as the object of a transitive verb: on
    /// a total object it is the nominative, `näen pojan`, and on a partitive
    /// one the genitive, `näen pojan`.
    Accusative,
    /// `-n`: `lapsen`.
    Genitive,
    /// `-a` and the consonant gradation that goes with it: `lasta`.
    Partitive,
    /// `-ssa`: `lapsessa`.
    Inessive,
    /// `-sta`, and the `-stä` of the stems that have one: `lapsesta`.
    Elative,
    /// `-n` and the gradation of the illative: `lapseen`.
    Illative,
    /// `-lla`: `lapsella`.
    Adessive,
    /// `-lta`: `lapselta`.
    Ablative,
    /// `-lle`: `lapselle`.
    Allative,
    /// `-na`: `lapsena`.
    Essive,
    /// `-ksi`: `lapseksi`.
    Translative,
    /// `-tta`: `lapsetta`.
    Abessive,
    /// `-ine`: `lapsine`.
    Comitative,
    /// `-tse`, which only a postposition can attach: `lapsena kirjana`.
    Prolative,
    /// `-n` on a participle, used with no other case: `lapsina`.
    Instructive,
}

impl Case {
    /// Every case, in the order they are listed above.
    pub const ALL: [Case; 16] = [
        Case::Nominative,
        Case::Accusative,
        Case::Genitive,
        Case::Partitive,
        Case::Inessive,
        Case::Elative,
        Case::Illative,
        Case::Adessive,
        Case::Ablative,
        Case::Allative,
        Case::Essive,
        Case::Translative,
        Case::Abessive,
        Case::Comitative,
        Case::Prolative,
        Case::Instructive,
    ];

    /// The name used in Finnish grammatical writing, for messages to a user.
    pub fn name(self) -> &'static str {
        match self {
            Case::Nominative => "nominative",
            Case::Accusative => "accusative",
            Case::Genitive => "genitive",
            Case::Partitive => "partitive",
            Case::Inessive => "inessive",
            Case::Elative => "elative",
            Case::Illative => "illative",
            Case::Adessive => "adessive",
            Case::Ablative => "ablative",
            Case::Allative => "allative",
            Case::Essive => "essive",
            Case::Translative => "translative",
            Case::Abessive => "abessive",
            Case::Comitative => "comitative",
            Case::Prolative => "prolative",
            Case::Instructive => "instructive",
        }
    }
}

/// How many of the referent.
///
/// Finnish has three, not two. The collective is the same word as the
/// singular with a different reading, and both are in the lexicon as separate
/// entries, so a form has to be able to say which one it is meant to be.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Number {
    Singular = 0,
    Plural,
    /// The collective reading of a singular form, `kissaa` for `kissa`.
    Collective,
}

impl Number {
    pub const ALL: [Number; 3] = [Number::Singular, Number::Plural, Number::Collective];

    pub fn name(self) -> &'static str {
        match self {
            Number::Singular => "singular",
            Number::Plural => "plural",
            Number::Collective => "collective",
        }
    }
}

/// Who is doing the verbing, or whose it is.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Person {
    First = 0,
    Second,
    Third,
}

impl Person {
    pub const ALL: [Person; 3] = [Person::First, Person::Second, Person::Third];

    pub fn name(self) -> &'static str {
        match self {
            Person::First => "first_person",
            Person::Second => "second_person",
            Person::Third => "third_person",
        }
    }
}

/// When, for a verb.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Tense {
    /// `lasken`, and the whole of the present system.
    Present = 0,
    /// `laskin`.
    Past,
    /// The present perfect, which is a compound and is not generated.
    Perfect,
    /// The pluperfect, likewise.
    Pluperfect,
}

impl Tense {
    pub const ALL: [Tense; 4] = [
        Tense::Present,
        Tense::Past,
        Tense::Perfect,
        Tense::Pluperfect,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tense::Present => "present",
            Tense::Past => "past",
            Tense::Perfect => "perfect",
            Tense::Pluperfect => "pluperfect",
        }
    }
}

/// Which mood, for a verb.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Mood {
    /// The plain one, `lasken`.
    Indicative = 0,
    /// `laskisin`.
    Conditional,
    /// `laskenkin`.
    Potential,
    /// `laske`.
    Imperative,
    /// The instructive, `laskien`, which is a participial mood and is tagged as
    /// both this and a [`Tense`].
    Instructive,
    /// An infinitive: `puhua`, `puhuessa`, `puhumaan`. The infinitive carries
    /// no person, and the second and third infinitives carry a case as well.
    Infinitive,
}

impl Mood {
    pub const ALL: [Mood; 6] = [
        Mood::Indicative,
        Mood::Conditional,
        Mood::Potential,
        Mood::Imperative,
        Mood::Instructive,
        Mood::Infinitive,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mood::Indicative => "indicative",
            Mood::Conditional => "conditional",
            Mood::Potential => "potential",
            Mood::Imperative => "imperative",
            Mood::Instructive => "instructive",
            Mood::Infinitive => "infinitive",
        }
    }
}

/// Which the subject is doing to itself.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[repr(u8)]
pub enum Voice {
    #[default]
    Active = 0,
    /// `lasketaan`.
    Passive,
    /// `laskeutua`, and the other verbs that are neither.
    Middle,
}

impl Voice {
    pub const ALL: [Voice; 3] = [Voice::Active, Voice::Passive, Voice::Middle];

    pub fn name(self) -> &'static str {
        match self {
            Voice::Active => "active",
            Voice::Passive => "passive",
            Voice::Middle => "middle",
        }
    }
}

/// Whether a verb form is affirmative or negative.
///
/// Finnish negation is analytic: the negative auxiliary `en`, `et`, `ei`,
/// `emme`, `ette`, `eivät` precedes a connegative form of the main verb. The
/// polarity is a property of the form, so a checker can tell `puhun` from the
/// `en puhu` construction's `puhu`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[repr(u8)]
pub enum Polarity {
    #[default]
    Affirmative = 0,
    /// The negative auxiliary construction: `en puhu`.
    Negative,
}

impl Polarity {
    pub const ALL: [Polarity; 2] = [Polarity::Affirmative, Polarity::Negative];

    pub fn name(self) -> &'static str {
        match self {
            Polarity::Affirmative => "affirmative",
            Polarity::Negative => "negative",
        }
    }
}

/// The part of speech a form belongs to.
///
/// The set is the one the sanalista itself uses, which is not quite a textbook
/// one: it separates a preposition from a postposition, and it has a category
/// for `kieltoverbi`, the auxiliary of negation, which is neither a verb nor a
/// pronoun and is left out of school lists entirely. A lemma may be of more
/// than one part of speech, as `adjektiivi, substantiivi` says, and the
/// sanalista lists them in order of prominence, so the first is the one this
/// keeps; the whole string is still on
/// [`crate::kotus::Lemma::part_of_speech`] for anyone who needs the rest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[repr(u8)]
pub enum PartOfSpeech {
    /// The sanalista says nothing, which it does for a few hundred entries.
    #[default]
    Unknown = 0,
    /// `substantiivi`
    Noun,
    /// `adjektiivi`
    Adjective,
    /// `pronomini`
    Pronoun,
    /// `numeraali`
    Numeral,
    /// `järjestysluku`, an ordinal such as `toinen`
    Ordinal,
    /// `verbi`
    Verb,
    /// `kieltoverbi`, the verb of negation
    NegativeVerb,
    /// `adverbi`
    Adverb,
    /// `konjunktio`, and the two kinds of it the sanalista distinguishes,
    /// `alistuskonjunktio` and `rinnastuskonjunktio`
    Conjunction,
    /// `interjektio`
    Interjection,
    /// `prepositio`
    Preposition,
    /// `postpositio`
    Postposition,
    /// A code this enum does not know, or a `+` compound of two classes such as
    /// `alistuskonjunktio + kieltoverbi`.
    Other,
}

impl PartOfSpeech {
    /// Every class, in the order of the enum.
    pub const ALL: [PartOfSpeech; 14] = [
        PartOfSpeech::Unknown,
        PartOfSpeech::Noun,
        PartOfSpeech::Adjective,
        PartOfSpeech::Pronoun,
        PartOfSpeech::Numeral,
        PartOfSpeech::Ordinal,
        PartOfSpeech::Verb,
        PartOfSpeech::NegativeVerb,
        PartOfSpeech::Adverb,
        PartOfSpeech::Conjunction,
        PartOfSpeech::Interjection,
        PartOfSpeech::Preposition,
        PartOfSpeech::Postposition,
        PartOfSpeech::Other,
    ];

    /// The sanalista's own name for a class, which is what a message to a user
    /// quoting the dictionary should say.
    pub fn name(self) -> &'static str {
        match self {
            PartOfSpeech::Unknown => "unknown",
            PartOfSpeech::Noun => "noun",
            PartOfSpeech::Adjective => "adjective",
            PartOfSpeech::Pronoun => "pronoun",
            PartOfSpeech::Numeral => "numeral",
            PartOfSpeech::Ordinal => "ordinal",
            PartOfSpeech::Verb => "verb",
            PartOfSpeech::NegativeVerb => "negative_verb",
            PartOfSpeech::Adverb => "adverb",
            PartOfSpeech::Conjunction => "conjunction",
            PartOfSpeech::Interjection => "interjection",
            PartOfSpeech::Preposition => "preposition",
            PartOfSpeech::Postposition => "postposition",
            PartOfSpeech::Other => "other",
        }
    }

    /// The name the sanalista uses, which is Finnish.
    pub fn finnish(self) -> &'static str {
        match self {
            PartOfSpeech::Unknown => "",
            PartOfSpeech::Noun => "substantiivi",
            PartOfSpeech::Adjective => "adjektiivi",
            PartOfSpeech::Pronoun => "pronomini",
            PartOfSpeech::Numeral => "numeraali",
            PartOfSpeech::Ordinal => "järjestysluku",
            PartOfSpeech::Verb => "verbi",
            PartOfSpeech::NegativeVerb => "kieltoverbi",
            PartOfSpeech::Adverb => "adverbi",
            PartOfSpeech::Conjunction => "konjunktio",
            PartOfSpeech::Interjection => "interjektio",
            PartOfSpeech::Preposition => "prepositio",
            PartOfSpeech::Postposition => "postpositio",
            PartOfSpeech::Other => "muu",
        }
    }

    /// Reads one class out of the sanalista's part-of-speech field.
    ///
    /// The field is a list in order of prominence, separated by `, `, and a
    /// couple of entries join two classes with a `+` instead, as
    /// `alistuskonjunktio + kieltoverbi` does. The first class is the one that
    /// says what the word is here; the rest are readings it also has.
    /// Every part of speech the field lists, in the order it lists them.
    ///
    /// A sanalista record can name more than one, and each of them is a reading
    /// of the same word rather than a choice to be made once and for all:
    /// `aamen` is in the data as `adverbi, substantiivi` and inflects as class
    /// 32, so it is an adverb and the noun with that paradigm, and a sentence
    /// says which. Same spelling, same classes, one reading each.
    pub fn parse_all(field: &str) -> Vec<PartOfSpeech> {
        let mut out: Vec<PartOfSpeech> = Vec::new();
        for item in field.split([',', '+']) {
            let pos = PartOfSpeech::of_field(item.trim());
            if !out.contains(&pos) {
                out.push(pos);
            }
        }
        if out.is_empty() {
            out.push(PartOfSpeech::Unknown);
        }
        out
    }

    /// The first part of speech the field lists, for a caller that has to pick
    /// one. [`PartOfSpeech::parse_all`] is the one to reach for otherwise.
    pub fn parse(field: &str) -> PartOfSpeech {
        PartOfSpeech::parse_all(field)
            .into_iter()
            .next()
            .unwrap_or(PartOfSpeech::Unknown)
    }

    /// Whether the field says the word can be a noun, which is what entitles
    /// it to the possessive column.
    ///
    /// [`PartOfSpeech::parse`] collapses a field to its *first* entry, so
    /// `adjektiivi, substantiivi` reads as an adjective and the noun reading is
    /// lost -- and `marto` and `pieni` really are both. This looks for the noun
    /// token anywhere in the field instead.
    pub fn is_substantival(field: &str) -> bool {
        field
            .split([',', '+'])
            .any(|item| matches!(item.trim(), "substantiivi" | "nimisana"))
    }

    /// The reading to inflect a word in, from its sanalista part-of-speech
    /// field.
    ///
    /// A noun inflector produced a noun paradigm, so a word the sanalista says
    /// can be a noun is read as one. Taking the field's first reading instead
    /// was not merely coarse but self-contradictory: `pieni` is
    /// `adjektiivi, substantiivi`, so `inflect_in`'s `of_pos` re-tag stamped the
    /// whole grid `adjective` -- including `pieneemme`, a form carrying the
    /// `-mme` suffix and asserting it is not a noun. The same shape as
    /// `ehtooheni` being tagged a bare singular illative.
    ///
    /// Anything not noun-capable keeps [`PartOfSpeech::parse`]'s first reading,
    /// so `mieto` is still an adjective and takes no possessives.
    pub fn inflecting_reading(field: &str) -> PartOfSpeech {
        if Self::is_substantival(field) {
            Self::Noun
        } else {
            Self::parse(field)
        }
    }

    fn of_field(item: &str) -> PartOfSpeech {
        match item {
            "" => PartOfSpeech::Unknown,
            "substantiivi" | "nimisana" => PartOfSpeech::Noun,
            "adjektiivi" | "laatuadjektiivi" => PartOfSpeech::Adjective,
            "pronomini" => PartOfSpeech::Pronoun,
            "numeraali" => PartOfSpeech::Numeral,
            "järjestysluku" => PartOfSpeech::Ordinal,
            "verbi" | "pääverbi" => PartOfSpeech::Verb,
            "kieltoverbi" => PartOfSpeech::NegativeVerb,
            "adverbi" => PartOfSpeech::Adverb,
            "konjunktio" | "alistuskonjunktio" | "rinnastuskonjunktio" => PartOfSpeech::Conjunction,
            "interjektio" => PartOfSpeech::Interjection,
            "prepositio" => PartOfSpeech::Preposition,
            "postpositio" => PartOfSpeech::Postposition,
            _ => PartOfSpeech::Other,
        }
    }
}

/// A Finnish clitic, a post-inflectional particle attached to the end of a
/// complete form.
///
/// Clitics are not cases or derivational suffixes: they attach to an
/// already-inflected word and change its discourse role rather than its
/// grammar. `aamullakin` is the adessive `aamulla` with a `-kin` on it, still
/// an adessive. Each has a front/back variant except `kin`, which is invariant.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Clitic {
    /// The interrogative `-ko`/`-kö`, which marks its host as the focus of a
    /// question: `sinä` is `sinäkö`.
    Ko,
    /// The additive `-kin`, "also, too, even": `aamu` is `aamukin`.
    Kin,
    /// The negative additive `-kaan`/`-kään`, "either": `minä` is `minäkään`.
    Kaan,
    /// The discourse `-han`/`-hän`, which reminds or softens: `talo` is
    /// `talohan`.
    Han,
    /// The emphatic `-pa`/`-pä`, which prompts or insists: `katso` is `katsopa`.
    Pa,
}

/// A possessive suffix, with whose it is.
///
/// Finnish has five distinct possessive suffixes, but the third person is one
/// form for both singular and plural: `talonsa` is both "his house" and "their
/// house". The third-person suffix also has an `a`/`ä` harmony alternation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Possessive {
    /// `-ni`, mine.
    FirstSingular,
    /// `-si`, yours.
    SecondSingular,
    /// `-nsa`/`-nsä`, his or hers or theirs.
    Third,
    /// `-mme`, ours.
    FirstPlural,
    /// `-nne`, yours.
    SecondPlural,
}

impl Possessive {
    /// Every possessive suffix.
    pub const ALL: [Possessive; 5] = [
        Possessive::FirstSingular,
        Possessive::SecondSingular,
        Possessive::Third,
        Possessive::FirstPlural,
        Possessive::SecondPlural,
    ];

    /// The suffix, with the `a`/`ä` of the third person chosen by harmony.
    pub fn suffix(self, front: bool) -> &'static str {
        match self {
            Possessive::FirstSingular => "ni",
            Possessive::SecondSingular => "si",
            Possessive::Third => {
                if front {
                    "nsä"
                } else {
                    "nsa"
                }
            }
            Possessive::FirstPlural => "mme",
            Possessive::SecondPlural => "nne",
        }
    }
}

/// What a form says about the grammar, with a dimension absent where it does
/// not apply.
///
/// The dimensions are all optional, and a form that is a noun has no person and
/// a form that is a participle may have both a tense and a mood. The type of
/// each is a one-byte discriminant, so a [`Tags`] is seven bytes and a word
/// table of a few million of them costs less than the strings do.
///
/// [`Tags`] is also what makes a word lookup explainable. [`Tags::unite`]
/// combines two readings into the union of what they agree on, and that is
/// what a word shared between two lemmas is indexed under: a case where both
/// lemmas agree, and nothing at all where they do not.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Tags {
    /// What the word is. It is a property of the lemma rather than of the form,
    /// but it belongs in here so that a lookup on a word can filter on it
    /// without a second index, and so that a word of two classes unions the
    /// same way the other dimensions do.
    pub part_of_speech: Option<PartOfSpeech>,
    /// The case, where the form is a nominal one.
    pub case: Option<Case>,
    /// Singular, plural or collective, defaulting to singular for a nominal
    /// form since that is the unmarked reading, and absent for anything that
    /// is not a noun at all.
    pub number: Option<Number>,
    /// The subject of a finite verb, or the person a possessive suffix refers
    /// to when [`Tags::possessive`] is set.
    pub person: Option<Person>,
    pub tense: Option<Tense>,
    pub mood: Option<Mood>,
    pub voice: Option<Voice>,
    /// Whose the thing is, for a suffixed form: `taloni` is first person
    /// singular and `talonsa` third person. The third person covers both
    /// singular and plural, since the suffix does not say which.
    pub possessive: Option<Possessive>,
    /// The clitic attached to the end of the form, if any: `aamullakin` is
    /// `aamulla` with a `kin`. A clitic is a post-inflectional particle, not a
    /// case, so it lives alongside the case rather than replacing it.
    pub clitic: Option<Clitic>,
    /// Whether a finite verb form is affirmative or negative. Defaults to
    /// affirmative; a negative form is the `en puhu` construction.
    pub polarity: Polarity,
}

impl Tags {
    /// The same reading, of another part of speech.
    pub fn of_pos(mut self, pos: PartOfSpeech) -> Self {
        self.part_of_speech = Some(pos);
        self
    }

    /// No tags at all, for a form whose grammar is not known.
    pub const UNKNOWN: Tags = Tags {
        part_of_speech: None,
        case: None,
        number: None,
        person: None,
        tense: None,
        mood: None,
        voice: None,
        possessive: None,
        clitic: None,
        polarity: Polarity::Affirmative,
    };

    /// A nominal form: a case, and singular unless the caller says otherwise.
    pub fn nominal(case: Case) -> Self {
        Tags {
            case: Some(case),
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        }
    }

    /// A nominal form of a stated number.
    pub fn of_number(case: Case, number: Number) -> Self {
        Tags {
            case: Some(case),
            number: Some(number),
            ..Tags::UNKNOWN
        }
    }

    /// A finite verb form.
    pub fn finite(tense: Tense, person: Person, number: Number) -> Self {
        Tags {
            number: Some(number),
            person: Some(person),
            tense: Some(tense),
            mood: Some(Mood::Indicative),
            voice: Some(Voice::Active),
            ..Tags::UNKNOWN
        }
    }

    /// The same form in a stated number.
    pub fn number(mut self, number: Number) -> Self {
        self.number = Some(number);
        self
    }

    /// The same form of a stated person.
    pub fn person(mut self, person: Person) -> Self {
        self.person = Some(person);
        self
    }

    /// The same form in a stated tense.
    pub fn tense(mut self, tense: Tense) -> Self {
        self.tense = Some(tense);
        self
    }

    /// The same form in a stated mood.
    pub fn mood(mut self, mood: Mood) -> Self {
        self.mood = Some(mood);
        self
    }

    /// The same form with no case, for a verb form, which has none.
    pub fn without_case(mut self) -> Self {
        self.case = None;
        self
    }

    /// The same form in the passive.
    pub fn passive(mut self) -> Self {
        self.voice = Some(Voice::Passive);
        self
    }

    /// The same form with a case on it, which is what an illative passive gets:
    /// `lasketaan` is the illative and the passive at once.
    pub fn in_case(mut self, case: Case) -> Self {
        self.case = Some(case);
        self
    }

    /// The same form with a possessive on it.
    pub fn possessed_by(mut self, poss: Possessive) -> Self {
        self.possessive = Some(poss);
        self
    }

    /// The same form with a clitic on it: `aamulla` with a `kin` is
    /// `aamullakin`.
    pub fn clitic(mut self, clitic: Clitic) -> Self {
        self.clitic = Some(clitic);
        self
    }

    /// The union of two readings: a dimension is kept when both agree, and
    /// dropped when they do not.
    ///
    /// This is what a word that is a form of more than one lemma is indexed
    /// under, and it is why a missing [`Tags::case`] does not mean the case is
    /// unknown but that the readings differ about it.
    pub fn unite(self, other: Tags) -> Tags {
        // A dimension that is absent in one reading and present in the other is
        // no disagreement: the reading that knows nothing does not get a vote,
        // and the one that knows is the reading. Only two different values are a
        // disagreement, and that leaves the dimension unknown.
        fn same<T: PartialEq>(a: Option<T>, b: Option<T>) -> Option<T> {
            match (a, b) {
                (Some(x), Some(y)) if x == y => Some(x),
                (Some(x), None) | (None, Some(x)) => Some(x),
                _ => None,
            }
        }
        Tags {
            part_of_speech: same(self.part_of_speech, other.part_of_speech),
            case: same(self.case, other.case),
            number: same(self.number, other.number),
            person: same(self.person, other.person),
            tense: same(self.tense, other.tense),
            mood: same(self.mood, other.mood),
            voice: same(self.voice, other.voice),
            possessive: same(self.possessive, other.possessive),
            clitic: same(self.clitic, other.clitic),
            polarity: same(Some(self.polarity), Some(other.polarity)).unwrap_or_default(),
        }
    }

    /// Whether these tags make every claim `other` makes, with the same value.
    ///
    /// A dimension `other` says nothing about is any value at all here, so a
    /// reading that says less is always subsumed by a reading that says more:
    /// the catalogue slot that says "a verb" is subsumed by the verb reading of
    /// the same word, and two readings that disagree about a dimension are not
    /// related this way at all.
    pub fn subsumes(&self, other: &Tags) -> bool {
        fn sub<T: PartialEq>(mine: Option<T>, theirs: Option<T>) -> bool {
            match theirs {
                Some(t) => mine == Some(t),
                None => true,
            }
        }
        sub(self.part_of_speech, other.part_of_speech)
            && sub(self.case, other.case)
            && sub(self.number, other.number)
            && sub(self.person, other.person)
            && sub(self.tense, other.tense)
            && sub(self.mood, other.mood)
            && sub(self.voice, other.voice)
            && sub(self.possessive, other.possessive)
            && sub(self.clitic, other.clitic)
            && sub(Some(self.polarity), Some(other.polarity))
    }

    /// Whether every dimension is absent, which is what a word is when nothing
    /// is known about it.
    pub fn is_unknown(&self) -> bool {
        *self == Tags::UNKNOWN
    }

    /// Whether the form carries every tag in `want`.
    ///
    /// This is the query a checker wants: a dimension the caller leaves as
    /// `None` in `want` is not required, and one the form has is enough.
    pub fn matches(&self, want: &Tags) -> bool {
        // A `None` on the wanted side is a wildcard, and a `None` on the have
        // side means nothing settled the dimension, which is either that
        // nobody knew or that the readings disagreed. Both of those leave the
        // form open to being what was asked for, so a request for one number
        // finds a word that might be either. The cost is that the answer is a
        // superset: what comes back has to be checked against the sentence
        // before it is called correct.
        fn hit<T: PartialEq>(have: Option<T>, want: Option<T>) -> bool {
            want.is_none() || have.is_none() || have == want
        }
        hit(self.part_of_speech, want.part_of_speech)
            && hit(self.case, want.case)
            && hit(self.number, want.number)
            && hit(self.person, want.person)
            && hit(self.tense, want.tense)
            && hit(self.mood, want.mood)
            && hit(self.voice, want.voice)
            && hit(self.possessive, want.possessive)
    }
}

/// How a form came to be, and so how much to trust it.
///
/// This is not grammar and does not belong in [`Tags`]. It is kept beside it
/// because a suggestion has to be offered only for a form the catalogue
/// actually vouches for, and a boolean cannot say which of three ways a form
/// was built is good enough to suggest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[repr(u8)]
pub enum Confidence {
    /// Guessed from a type rule, with no catalogue form behind it: a form such
    /// as `laskettaa` that the eight slots do not reach, or one built from a
    /// stem the class does not license. Accepted, never suggested.
    #[default]
    Guessed = 0,
    /// Built by repairing the lemma to fit the ending: gradation applied, a
    /// vowel epenthesised, an alternation read off the genitive. These are
    /// real words often enough that the repair is worth making, but the
    /// attachment was not a plain one.
    Repaired,
    /// A plain attachment of a catalogue ending to the lemma, with no repair at
    /// all. These are the forms a correction may be drawn from.
    Plain,
}

/// One generated word, with what it is and how much to trust it.
#[derive(Clone, Debug)]
pub struct Form {
    /// The word as it would be written.
    pub word: String,
    /// What it says about the grammar, once per way of reading it.
    ///
    /// More than one means the word really is ambiguous and something outside
    /// this crate has to say which reading is meant. No two of them are the same,
    /// and none says less than another: a claim that only repeats part of one
    /// already here would be the same reading written down twice, and
    /// [`Form::add_reading`] drops it rather than counting it.
    pub readings: Vec<Tags>,
    /// How it was built.
    pub confidence: Confidence,
}

impl Form {
    /// A form with one reading and no trust attached beyond the default.
    pub fn new(word: String, tags: Tags) -> Self {
        Form {
            word,
            readings: vec![tags],
            confidence: Confidence::Guessed,
        }
    }

    /// The readings, in the order they were found.
    pub fn readings(&self) -> &[Tags] {
        &self.readings
    }

    /// Whether more than one reading of the word is on offer, which is a
    /// question about the word that only a caller with context can answer.
    pub fn is_ambiguous(&self) -> bool {
        self.readings.len() > 1
    }

    /// What the readings say between them, for the caller that wants one answer.
    ///
    /// A dimension that is absent here because the readings disagree about it is
    /// absent for a different reason than one that is absent because no reading
    /// ever claimed it, and the list is what tells those apart.
    pub fn union(&self) -> Tags {
        agreed(&self.readings)
    }

    /// Adds a way of reading the word, unless what is already here says more.
    ///
    /// A reading nobody else makes is a reading, and a reading somebody already
    /// made in fuller is that same reading written down smaller, so the fuller
    /// one is the one that is kept. The catalogue slot that says "a verb" and
    /// the verb generator's reading of `lasken` are one reading between them,
    /// not two, and the person and the tense have to survive that.
    pub fn add_reading(&mut self, tags: Tags) {
        add_reading(&mut self.readings, tags);
    }

    /// Takes in everything another form of the same word had to say.
    pub fn absorb(&mut self, other: &Form) {
        for reading in &other.readings {
            self.add_reading(*reading);
        }
        self.confidence = self.confidence.max(other.confidence);
    }

    /// Whether any reading of the word says what was asked for.
    pub fn matches(&self, want: &Tags) -> bool {
        self.readings.iter().any(|r| r.matches(want))
    }

    /// The readings that fit, for a caller that has the context to choose.
    pub fn readings_matching<'a>(&'a self, want: &'a Tags) -> impl Iterator<Item = &'a Tags> + 'a {
        self.readings.iter().filter(move |r| r.matches(want))
    }

    /// The same form, built another way.
    pub fn with(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }

    /// The same form, of another part of speech.
    ///
    /// This is a claim about the word and not a reading, so it goes on every
    /// reading there is: a class that is a verb class is a verb whatever the
    /// dictionary says, and a form that was read two ways is the same word.
    pub fn of_pos(mut self, pos: PartOfSpeech) -> Self {
        for reading in &mut self.readings {
            reading.part_of_speech = Some(pos);
        }
        self
    }

    /// Whether the form is a plain attachment of a catalogue ending, which is
    /// what the suggestion path filters on.
    pub fn plain(&self) -> bool {
        self.confidence == Confidence::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(case: Case, number: Number) -> Tags {
        Tags::of_number(case, number)
    }

    /// A noun-capable word is read as a noun, whatever its field leads with.
    ///
    /// The sanalista lists a part of speech in order of prominence, and
    /// `parse` takes the first. For a noun inflector that is the wrong end of
    /// the field to ask: `pieni` and `marto` are both `adjektiivi, substantiivi`,
    /// so parsing first says adjective and `inflect_in`'s `of_pos` then stamped
    /// that over the whole grid -- including `pieneemme`, a form carrying the
    /// `-mme` suffix and tagged as not being a noun.
    #[test]
    fn a_word_that_can_be_a_noun_is_read_as_one() {
        for field in ["adjektiivi, substantiivi", "substantiivi, adjektiivi", "nimisana"] {
            assert!(
                PartOfSpeech::is_substantival(field),
                "{field} should be noun-capable"
            );
            assert_eq!(
                PartOfSpeech::inflecting_reading(field),
                PartOfSpeech::Noun,
                "{field} should be inflected as a noun"
            );
        }
        // And a word that cannot is left on its first reading, so the adjective
        // that takes no possessives is still an adjective.
        assert_eq!(
            PartOfSpeech::inflecting_reading("adjektiivi"),
            PartOfSpeech::Adjective
        );
        assert!(!PartOfSpeech::is_substantival("adjektiivi"));
        // The `+` separator joins two classes in a few records
        // (`alistuskonjunktio + kieltoverbi`), so it counts too.
        assert!(PartOfSpeech::is_substantival("adjektiivi + substantiivi"));
    }

    #[test]
    fn a_dimension_survives_only_where_the_readings_agree() {
        let one = tags(Case::Nominative, Number::Singular);
        let two = tags(Case::Nominative, Number::Plural);
        // Both are nominatives, so the case stays, and the one thing they
        // disagree about is the one thing that goes.
        assert_eq!(one.unite(two).case, Some(Case::Nominative));
        assert_eq!(one.unite(two).number, None);
    }

    #[test]
    fn a_dimension_the_readings_disagree_about_goes_but_the_one_they_agree_on_stays() {
        // Nominative of one lemma and partitive of another: the case is
        // contested and drops out, and the number is not in dispute and stays.
        let contested =
            tags(Case::Nominative, Number::Singular).unite(tags(Case::Partitive, Number::Singular));
        assert_eq!(contested.case, None);
        assert_eq!(contested.number, Some(Number::Singular));
        // Which is why the value cannot say whether a `None` is unknown or
        // contested, and the callers that care ask the generators again.
        assert_eq!(Tags::UNKNOWN.unite(Tags::UNKNOWN), Tags::UNKNOWN);
    }

    #[test]
    fn a_wildcard_matches_anything_and_a_value_matches_only_itself() {
        let one = tags(Case::Inessive, Number::Singular);
        assert!(one.matches(&Tags::UNKNOWN));
        assert!(one.matches(&tags(Case::Inessive, Number::Singular)));
        assert!(!one.matches(&tags(Case::Inessive, Number::Plural)));
        // A form whose number the readings disagreed about matches a request
        // for either number, since it might be either.
        let contested = one.unite(tags(Case::Inessive, Number::Plural));
        assert!(contested.matches(&tags(Case::Inessive, Number::Plural)));
    }

    #[test]
    fn the_part_of_speech_comes_from_the_sanalista() {
        assert_eq!(PartOfSpeech::parse("substantiivi"), PartOfSpeech::Noun);
        assert_eq!(PartOfSpeech::parse("verbi"), PartOfSpeech::Verb);
        // A lemma of two classes is the first one, and that is the one the
        // sanalista lists it under.
        assert_eq!(
            PartOfSpeech::parse("adjektiivi, substantiivi"),
            PartOfSpeech::Adjective
        );
        // A `+` compound is read up to the join, and the whole field can be
        // empty.
        assert_eq!(
            PartOfSpeech::parse("alistuskonjunktio + kieltoverbi"),
            PartOfSpeech::Conjunction
        );
        assert_eq!(PartOfSpeech::parse(""), PartOfSpeech::Unknown);
        assert_eq!(PartOfSpeech::parse("jotain muuta"), PartOfSpeech::Other);
    }

    #[test]
    fn every_class_the_sanalista_uses_has_a_name() {
        // A class with no name is a class nothing can say about a word.
        for pos in PartOfSpeech::ALL {
            assert!(!pos.name().is_empty());
        }
        assert_eq!(
            PartOfSpeech::parse(PartOfSpeech::Postposition.finnish()),
            PartOfSpeech::Postposition
        );
    }

    #[test]
    fn a_verb_form_is_a_person_and_a_tense_and_never_a_case() {
        let one = Tags::finite(Tense::Present, Person::First, Number::Singular);
        assert_eq!(one.person, Some(Person::First));
        assert_eq!(one.tense, Some(Tense::Present));
        assert_eq!(one.case, None);
    }

    #[test]
    fn plain_is_one_of_three_and_the_order_is_the_confidence() {
        assert!(Confidence::Plain > Confidence::Repaired);
        assert!(Confidence::Repaired > Confidence::Guessed);
        assert!(!Form::new("x".to_string(), Tags::UNKNOWN).plain());
        assert!(
            Form::new("x".to_string(), Tags::UNKNOWN)
                .with(Confidence::Plain)
                .plain()
        );
    }
}
