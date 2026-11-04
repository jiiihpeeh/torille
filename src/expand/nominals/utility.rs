//! Shared helpers for the hand-written per-class inflection engines.
//!
//! Every class-1 family file does the same three things around its own stem
//! rules: decide the word's harmony, choose the front/back twin of an ending,
//! and emit a case cell -- the bare form plus the five possessives (and the
//! third person's two spellings). `Builder` is that machinery, so a class file
//! is only its stem rules and the table of which grade feeds which case.

use crate::harmony::pick_harmony;
use crate::tags::{Case, Confidence, Form, Number, PartOfSpeech, Possessive, Tags};

/// The personal possessive suffixes, and the owner each spells.
pub const PERSONAL: &[(&str, Possessive)] = &[
    ("ni", Possessive::FirstSingular),
    ("si", Possessive::SecondSingular),
    ("mme", Possessive::FirstPlural),
    ("nne", Possessive::SecondPlural),
];

/// Whether the lemma front-harmonizes: decided by the *last* non-neutral
/// vowel (`a/ä`, `o/ö`, `u/y`, scanning right to left; `e`/`i` are neutral).
/// `talo` is back on its `o`, `äly` front on its `y`, and a compound is
/// settled by its head (`hätähousu` back on its `u`).
///
/// Re-exported from [`crate::harmony::front`]; the canonical home is at the
/// crate root alongside [`crate::gradation`] because vowel harmony is a
/// Finnish primitive used by both the hand-written engine and the heuristic.
pub use crate::harmony::front;

/// Vowel harmony as a value, for code that builds an ending from the shared
/// suffix table rather than by hand. A word's harmony is a property of the word
/// and not always of its nominative's vowel: the pronoun `minä` is back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Harmony {
    Back,
    Front,
}

impl Harmony {
    /// The ending's spelling on this half of the pair (`ssa`/`ssä`).
    pub fn pick(self, back: &'static str, front: &'static str) -> &'static str {
        match self {
            Harmony::Back => back,
            Harmony::Front => front,
        }
    }
}

/// The regular singular case ending for a noun-like word of the given harmony.
///
/// The nominative, partitive and illative are deliberately absent: they are the
/// suppletive members each class spells outright (`minua`, `minuun`, `sitä`).
/// A class that needs the rest of the paradigm gets it here instead of writing
/// out `na`/`nä`, `ssa`/`ssä`, ... at every call site.
pub fn singular_case_suffix(case: Case, harmony: Harmony) -> Option<&'static str> {
    use Case::*;
    Some(match case {
        Genitive => "n",
        Essive => harmony.pick("na", "nä"),
        Translative => "ksi",
        Inessive => harmony.pick("ssa", "ssä"),
        Elative => harmony.pick("sta", "stä"),
        Adessive => harmony.pick("lla", "llä"),
        Ablative => harmony.pick("lta", "ltä"),
        Allative => "lle",
        Abessive => harmony.pick("tta", "ttä"),
        _ => return None,
    })
}

/// The reading a nominal cell carries: the lemma's part of speech, the case
/// and number of the cell, and the owner when it has one.
///
/// The part of speech is the lemma's rather than the class's, and that is the
/// whole reason this is a function rather than something `push` infers. Class
/// 38 holds `nainen` and `aakkosellinen` alike and class 41*A holds both `talo`
/// and `aallokas`; a form's reading says which one it came from, and only the
/// dictionary knows that. A builder that assumed `Noun` would tag 5 913 rows of
/// adjectives, numerals, pronouns and interjections as nouns.
pub fn nominal_tags(
    pos: PartOfSpeech,
    case: Case,
    number: Number,
    possessive: Option<Possessive>,
) -> Tags {
    let tags = Tags::of_number(case, number).of_pos(pos);
    match possessive {
        Some(p) => tags.possessed_by(p),
        None => tags,
    }
}

/// What a case's possessive suffixes join, where that is not the bare form.
///
/// 946 of the tree's 1 397 cells take them on the bare form (`taloa` gives
/// `taloani`) and are [`Base::Bare`]. The rest say so as a stem and an ending
/// rather than a finished string, because *which* stem is the interesting part:
/// a class with consonant gradation states the strong-genitive rule here, the
/// genitive's bare being weak (`akun`) while its possessives sit on the strong
/// stem (`akkuni`). That is why there is a second variant rather than a list of
/// named cases -- the relation is uniform even when the stems are not, and a
/// variant per special case would be a list of the 435 exceptions rather than a
/// rule for them.
///
/// Nothing here is derived from the ending, deliberately: a rule that recomputed
/// the base would have to be right about all 435 exceptions at once, and the one
/// it guessed would silently produce `*talonani`.
#[derive(Clone, Debug)]
pub enum Base {
    /// The bare form: `taloa` -> `taloani`, `talossa` -> `talossani`.
    ///
    /// This is the common case and the default, and it is worth knowing how
    /// common: of the tree's 1 368 `b.cell` calls, 917 spell the base exactly
    /// as the bare form, 310 put a second ending on the same stem, 88 hang off
    /// a different stem, and 53 hang off the bare stem. So reaching for one of
    /// the other three is the exception and is worth a second look.
    Bare,
    /// A stem with an ending of its own. An empty ending is a bare stem, which is
    /// what the strong-genitive rule wants (`akkuni`) and what the singular
    /// illative's own possessives hang off (`talooni`, off the doubled stem
    /// rather than `*taloonni`).
    On { stem: String, ending: Ending },
}

/// One cell of the grid: which case, on which stem, with which ending.
///
/// The bare form is `stem + ending`. A cell whose stem is not the plain lemma
/// says so -- `doubled_stem` for the singular illative (`talo` is `taloon`
/// there and `talon` everywhere else), a gradating class's weak stem for the
/// cells that take one.
///
/// `double3` is per cell rather than per class on purpose. The doubled
/// third-person spelling (`taloinen` beside `taloinsa`) needs a base ending in a
/// harmonic vowel, and the genitive's base is a bare stem and the illative's
/// ends in a consonant, so a class-wide flag would invent `*taloinen` for a cell
/// whose base cannot carry it. 895 of the tree's 1 397 cells set it and 390 do
/// not, and no two classes agree on where the line falls.
pub struct Cell {
    pub case: Case,
    pub number: Number,
    /// The stem this cell is built on. A class with consonant gradation gives
    /// its strong cells one stem and its weak cells another (`akku`: `akkua` on
    /// the strong, `akussa` on the weak), and a class without gives every cell
    /// the same one.
    pub stem: String,
    /// The ending that joins the stem. Empty where the cell *is* the stem.
    pub ending: Ending,
    /// Whether the third person may also be spelled by doubling the base's
    /// final vowel.
    pub double3: bool,
    pub base: Base,
    /// How much to trust this cell. `Plain` is the default; `Repaired` marks a
    /// whole alternant cell the catalogue wraps in parentheses, which is real
    /// for some words of the class and not for the one at hand, so it is
    /// generated but never offered as a plain correction. A cell inside a
    /// `Builder::new_rare` paradigm stays repaired whatever this says.
    pub confidence: Confidence,
}

/// One thing a class's paradigm says, in the order it says it.
///
/// A `Vec` rather than a fixed twelve-cell array because the order is the
/// class's own and not the canonical one: class 1A emits its strong-grade cells
/// first and its weak-grade ones after, and the alternant plurals are offered
/// between the cells they belong beside (`pianoitten` after `pianoiden`, not at
/// the end). Sorting into `DECLENSION` order would be tidier and would change
/// what the engine emits, which is observable.
pub enum Entry {
    /// A cell and its possessive column.
    Cell(Cell),
    /// Just the possessive column of a case, on a base of its own and with no
    /// bare form of its own. Five modules need this and they all need it the
    /// same way: the translative's bare is `estem` + `-ksi` while its possessives
    /// hang off `estem` + `-kse` (`jouheksi`, `jouhekse` + `ni`), so the `-kse`
    /// column has no cell to belong to and naming it as its own thing is what
    /// stops the two spellings from being confused for one.
    Owned(Column),
    /// An alternant the catalogue wraps in parentheses (`pianoiten`,
    /// `pianoita`): real for some words of the class, generated so a catalogue
    /// model reproduces, and never offered as a plain correction the reference
    /// does not confirm.
    Rare {
        case: Case,
        number: Number,
        stem: String,
        ending: Ending,
    },
    /// One extra plain form the grid does not account for. The instructive is
    /// the only one: `tloin`, which is `stem + in` and belongs to no case's
    /// shape because no other case ends that way.
    Plain {
        case: Case,
        number: Number,
        stem: String,
        ending: Ending,
    },
}

/// A noun class's paradigm, as data.
///
/// The grid is fixed -- twelve cases in two numbers, each with a possessive
/// column -- so a class says only what it changes: which stem each cell is built
/// on, the ending that joins it, and where the possessives hang off. Naming
/// that lets `decline` drive it once instead of 1 397 times, and it makes it
/// impossible to write a `bare` and a `base` that disagree, which 962 of those
/// cells had no way to avoid writing out twice.
pub struct Decl {
    pub entries: Vec<Entry>,
}

impl Decl {
    /// The cells a class emits, in the order it emits them.
    pub fn new(entries: Vec<Entry>) -> Self {
        Self { entries }
    }
}

/// The five possessives of one case, hanging off a base.
///
/// A cell builds one of these and emits it after the bare form. This is the same
/// five forms keyed by the same case and number, so naming them separately is
/// what lets the translative's `-kse` column exist without a `-kse` bare form.
pub struct Column {
    case: Case,
    number: Number,
    base: String,
    double3: bool,
    confidence: Confidence,
}

/// The ordinary cell: `stem + ending`, possessives on the bare form.
///
/// The two exceptions have their own methods on the result, so a class reads as
/// the data it is and the shape of each cell is visible at the call site rather
/// than inferred by whoever wrote [`Builder::decline`].
pub fn cell(
    case: Case,
    number: Number,
    stem: &str,
    ending: impl Into<Ending>,
    double3: bool,
) -> Entry {
    Entry::Cell(Cell {
        case,
        number,
        stem: stem.to_string(),
        ending: ending.into(),
        double3,
        base: Base::Bare,
        confidence: Confidence::Plain,
    })
}

/// A possessive column with no bare form of its own: the translative's `-kse`
/// possessives, in the five modules that have them.
pub fn possessed(case: Case, number: Number, base: impl Into<String>, double3: bool) -> Entry {
    Entry::Owned(Column {
        case,
        number,
        base: base.into(),
        double3,
        confidence: Confidence::Plain,
    })
}

/// An alternant form: generated, but never offered as a plain correction the
/// reference does not confirm. The catalogue wraps these in parentheses
/// (`piano` -> `pianoiden` but also `pianoitten`, and the partitive plural
/// `pianoita`), which is the reference's way of saying a spelling is real for
/// some words of the class and not for the one at hand.
pub fn rare(case: Case, number: Number, stem: &str, ending: [&'static str; 2]) -> Entry {
    Entry::Rare {
        case,
        number,
        stem: stem.to_string(),
        ending: ending.into(),
    }
}

/// One extra plain form the grid does not account for. The instructive is the
/// only one in the tree: `tloin` is `stem + in`, and no case ends that way.
pub fn plain(case: Case, number: Number, stem: &str, ending: [&'static str; 2]) -> Entry {
    Entry::Plain {
        case,
        number,
        stem: stem.to_string(),
        ending: ending.into(),
    }
}

/// One extra plain form spelled outright rather than built on the stem, for the
/// classes whose paradigm is not derivable at all (`class_101`, the sanalista's
/// own spellings for the words it marks as having no model).
pub fn whole(case: Case, number: Number, word: impl Into<String>) -> Entry {
    Entry::Plain {
        case,
        number,
        stem: String::new(),
        ending: Ending::Given(word.into()),
    }
}

impl Entry {
    /// The possessives hang off the cell's own stem rather than the bare form:
    /// the genitive's sit on `talo` (`taloni`, not `*talonani`).
    ///
    /// This is the *rarest* of the four base shapes -- 53 of the 1 368 cells in
    /// the tree -- so a port that reaches for it everywhere is going wrong. Most
    /// cells want nothing here at all, and `haahta` beside `haahtani` is the
    /// mistake it invites: the harmonized local cases take their possessives on
    /// the bare form, ending and all, so they want no suffix and no method.
    pub fn on_stem(self) -> Self {
        self.on_ending(["", ""])
    }

    /// The possessives hang off the cell's own stem with a second ending: the
    /// translative's off `talokse`, the genitive plural's off `taloje`.
    pub fn on_ending(self, ending: impl Into<Ending>) -> Self {
        let stem = match &self {
            Entry::Cell(c) => c.stem.clone(),
            _ => String::new(),
        };
        self.base_on(stem, ending)
    }

    /// The whole cell, possessive column and all, is an alternant: generated so
    /// a catalogue model reproduces, but never offered as a plain correction the
    /// reference does not confirm. This is what the old `cell_rare` did, and it
    /// is a property of the cell rather than a second kind of cell.
    pub fn as_rare(mut self) -> Self {
        if let Entry::Cell(c) = &mut self {
            c.confidence = Confidence::Repaired;
        }
        self
    }

    /// The possessives hang off *another* stem, which is how a gradating class
    /// states the strong-genitive rule: the genitive's bare is weak (`akun`)
    /// while its possessives are strong (`akkuni`). 130-odd cells in the tree
    /// need this, and it is the reason [`Base`] is a relation rather than a
    /// list of named exceptions.
    pub fn base_on(self, stem: String, ending: impl Into<Ending>) -> Self {
        match self {
            Entry::Cell(mut c) => {
                c.base = Base::On {
                    stem,
                    ending: ending.into(),
                };
                Entry::Cell(c)
            }
            other => other,
        }
    }
}

/// The ending that joins a cell's stem.
///
/// `Pair` is the usual case and the only one a class writes by hand: the back
/// spelling and its front twin, so the word's harmony chooses. An ending that is
/// the same either way (`lle`, `ksi`) is written twice, which is what makes it
/// a pair rather than a single `&'static str`.
///
/// `Given` is for the two cells in the tree whose ending is not known until the
/// lemma is in hand: class 17 and class 18 pick the singular illative's vowel
/// doubling from the stem's last letter (`ill_v`), so there is no pair to write
/// down. The string is taken as already harmonized, because a class that
/// computes one has had to decide harmony to compute it.
#[derive(Clone, Debug)]
pub enum Ending {
    Pair([&'static str; 2]),
    Given(String),
}

impl Ending {
    /// The ending on this half of the harmony.
    pub fn pick(&self, front: bool) -> &str {
        match self {
            Ending::Pair([back, front_twin]) => {
                if front {
                    front_twin
                } else {
                    back
                }
            }
            Ending::Given(ending) => ending,
        }
    }
}

impl From<[&'static str; 2]> for Ending {
    fn from(pair: [&'static str; 2]) -> Self {
        Ending::Pair(pair)
    }
}

impl From<&'static str> for Ending {
    fn from(ending: &'static str) -> Self {
        Ending::Given(ending.to_string())
    }
}

impl From<String> for Ending {
    fn from(ending: String) -> Self {
        Ending::Given(ending)
    }
}

/// The stem with its final vowel doubled, for the singular illative, where
/// `talo` is `taloon` and in every other case it is `talon`.
///
/// This is a function and not a flag on [`Cell`] because the three classes that
/// double all do it differently: class 1 doubles the cell's own stem, while
/// class 17 and class 18 bind a second stem beside it. A flag would have
/// expressed the first and forced the other two to fake it.
pub fn doubled_stem(stem: &str) -> String {
    match stem.chars().last() {
        Some(lastv) => format!("{stem}{lastv}"),
        None => stem.to_string(),
    }
}

/// Accumulates the forms of one inflected word.
pub struct Builder {
    out: Vec<Form>,
    front: bool,
    /// Whether the word takes possessive suffixes: only a plain noun does.
    /// An adjective or a participle (`mieto`, `prostituoitu`) does not
    /// participate in the possessive paradigm, so its possessive column is
    /// suppressed.
    possessives: bool,
    /// The confidence every `push`/`cell` form gets. `Plain` for an engine
    /// whose forms the word's own paradigm offers; `Repaired` for a whole
    /// paradigm that is real but that the reference does not confirm as
    /// offered (`buffet'in` beside `buffetin`), so it is never a plain
    /// correction.
    confidence: Confidence,
}

impl Builder {
    /// A builder for `lemma`. Harmony is decided once at the start;
    /// `possessives` says whether the word takes possessive suffixes (only a
    /// noun does; an adjective or participle does not).
    pub fn new(lemma: &str, possessives: bool) -> Self {
        Self {
            out: Vec::new(),
            front: front(lemma),
            possessives,
            confidence: Confidence::Plain,
        }
    }

    /// A builder whose whole paradigm is offered only as repaired guesses --
    /// a real but unconfirmed alternant paradigm.
    pub fn new_rare(lemma: &str, possessives: bool) -> Self {
        Self {
            out: Vec::new(),
            front: front(lemma),
            possessives,
            confidence: Confidence::Repaired,
        }
    }

    /// A builder for `lemma` with an explicit harmony, for a class that knows a
    /// word's real front/back identity better than the orthographic heuristic
    /// (loanword compounds whose neutral head is front-by-convention).
    pub fn new_harmony(lemma: &str, possessives: bool, front: bool) -> Self {
        Self {
            out: Vec::new(),
            front,
            possessives,
            confidence: Confidence::Plain,
        }
    }

    pub fn is_front(&self) -> bool {
        self.front
    }

    /// Whether the word's paradigm includes possessives.
    pub fn has_possessives(&self) -> bool {
        self.possessives
    }

    /// The harmonic twin: `back` in the back half, `front_twin` in the front.
    pub fn h(&self, back: &str, front_twin: &str) -> String {
        String::from(if self.front { front_twin } else { back })
    }

    pub fn push(&mut self, case: Case, num: Number, poss: Option<Possessive>, word: String) {
        let tags = nominal_tags(PartOfSpeech::Noun, case, num, poss);
        self.emit(word, tags);
    }

    /// Push one form with the reading the caller states, at the builder's own
    /// confidence. Every other entry point funnels through here, so a form's
    /// confidence is decided in one place.
    fn emit(&mut self, word: String, tags: Tags) {
        let confidence = self.confidence;
        self.emit_at(word, tags, confidence);
    }

    /// Push one form at a stated confidence, overriding the builder's own. Only
    /// a cell that is itself an alternant does this; everything else takes the
    /// builder's, which is `Repaired` for a class whose whole paradigm is
    /// alternant and `Plain` otherwise.
    fn emit_at(&mut self, word: String, tags: Tags, confidence: Confidence) {
        self.out.push(Form::new(word, tags).with(confidence));
    }

    /// The possessive column of one cell: the bare form, the five owners, and
    /// the third person's two spellings.
    ///
    /// This is the whole point of naming a cell's `base` rather than passing a
    /// `bare` and a `base` string: the base is stated once, as a relation to the
    /// stem, so the two cannot drift apart.
    pub fn decline(&mut self, d: &Decl, tags: impl Fn(Case, Number, Option<Possessive>) -> Tags) {
        let front = self.front;
        for entry in &d.entries {
            match entry {
                Entry::Cell(cell) => self.emit_cell(cell, front, &tags),
                Entry::Owned(column) => {
                    let confidence = self.wiser(column.confidence);
                    let base = column.base.clone();
                    self.emit_column(
                        column.case,
                        column.number,
                        column.double3,
                        &base,
                        confidence,
                        &tags,
                    );
                }
                Entry::Rare {
                    case,
                    number,
                    stem,
                    ending,
                } => {
                    let tags = nominal_tags(PartOfSpeech::Noun, *case, *number, None);
                    let word = format!("{stem}{}", ending.pick(front));
                    self.out
                        .push(Form::new(word, tags).with(Confidence::Repaired));
                }
                Entry::Plain {
                    case,
                    number,
                    stem,
                    ending,
                } => {
                    let tags = nominal_tags(PartOfSpeech::Noun, *case, *number, None);
                    self.emit(format!("{stem}{}", ending.pick(front)), tags);
                }
            }
        }
    }

    /// The bare form of one cell, then its possessives.
    fn emit_cell(
        &mut self,
        cell: &Cell,
        front: bool,
        tags: &impl Fn(Case, Number, Option<Possessive>) -> Tags,
    ) {
        let confidence = self.wiser(cell.confidence);
        let bare = format!("{}{}", cell.stem, cell.ending.pick(front));
        self.emit_at(bare.clone(), tags(cell.case, cell.number, None), confidence);
        if !self.possessives {
            return; // an adjective or participle has no possessive column
        }
        let base = match &cell.base {
            Base::Bare => bare,
            Base::On { stem, ending } => format!("{stem}{}", ending.pick(front)),
        };
        self.emit_column(
            cell.case,
            cell.number,
            cell.double3,
            &base,
            confidence,
            tags,
        );
    }

    /// The more repaired of two confidences wins: a cell marked rare stays rare
    /// even inside a plain builder, and a plain cell is rare inside a rare one.
    fn wiser(&self, confidence: Confidence) -> Confidence {
        match (confidence, self.confidence) {
            (Confidence::Repaired, _) | (_, Confidence::Repaired) => Confidence::Repaired,
            (plain, _) => plain,
        }
    }

    /// The five possessives of one case, on a base already resolved.
    ///
    /// The third person is spelled two ways -- `-nsa`/`-nsä` and the doubling of
    /// the base's last vowel -- the latter only when the column asks for it. The
    /// doubling needs a base ending in a vowel; every column that asks has one,
    /// and `if let` rather than `unwrap` because a panic here would take out the
    /// whole inflection over one odd lemma.
    fn emit_column(
        &mut self,
        case: Case,
        number: Number,
        double3: bool,
        base: &str,
        confidence: Confidence,
        tags: &impl Fn(Case, Number, Option<Possessive>) -> Tags,
    ) {
        if !self.possessives {
            return; // an adjective or participle has no possessive column
        }
        for (suf, p) in PERSONAL {
            self.emit_at(
                format!("{base}{suf}"),
                tags(case, number, Some(*p)),
                confidence,
            );
        }
        let third = tags(case, number, Some(Possessive::Third));
        let nsa = self.h("nsa", "nsä");
        self.emit_at(format!("{base}{nsa}"), third.clone(), confidence);
        if double3 && let Some(lastv) = base.chars().last() {
            self.emit_at(format!("{base}{lastv}n"), third, confidence);
        }
    }

    /// One case cell: the bare form, then the five possessives on `base`.
    ///
    /// The possessives attach to `base` (the genitive, the illative and the
    /// nominative-plural have their own `base` in each class). The third
    /// person is spelled two ways -- `-nsa`/`-nsä` and the doubling of the
    /// owner word's last vowel -- the latter only when `double3` and `base`
    /// ends in a harmonic vowel.
    pub fn cell(&mut self, case: Case, num: Number, bare: String, base: String, double3: bool) {
        self.push(case, num, None, bare);
        if !self.possessives {
            return; // an adjective or participle has no possessive column
        }
        for (suf, p) in PERSONAL {
            self.push(case, num, Some(*p), format!("{base}{suf}"));
        }
        self.push(
            case,
            num,
            Some(Possessive::Third),
            format!("{base}{}", self.h("nsa", "nsä")),
        );
        if double3 {
            let lastv = base.chars().last().unwrap();
            self.push(
                case,
                num,
                Some(Possessive::Third),
                format!("{base}{lastv}n"),
            );
        }
    }

    /// The comitative: a plural-only case whose ending already carries its
    /// owner -- `taloine`, `taloineen`, `taloineni`, `taloinemme`. The
    /// distinct base is `stem + ine` and the owner appends.
    ///
    /// Callers hand us the plural stem, which many classes spell with a
    /// final `i` (`koiri`, `moni`, `vedenpitävi`) while others hand us a
    /// stem without it (`talo`). The comitative marker itself supplies the
    /// `i`, so drop a trailing `i` before appending: otherwise class 10
    /// produced `koiriineen` instead of `koirine`, and class 23 produced
    /// `moniineen` for `monine`.
    ///
    /// The bare form is a case form like any other and is offered whatever
    /// `possessives` says; the owners are possessive suffixes, so they
    /// follow it, the same as every other column here.
    pub fn comitative(&mut self, stem: &str) {
        let base = stem.strip_suffix('i').unwrap_or(stem);
        self.push(Case::Comitative, Number::Plural, None, format!("{base}ine"));
        if !self.possessives {
            return;
        }
        for (suf, p) in [
            ("ineni", Possessive::FirstSingular),
            ("inesi", Possessive::SecondSingular),
            ("inemme", Possessive::FirstPlural),
            ("inenne", Possessive::SecondPlural),
        ] {
            self.push(
                Case::Comitative,
                Number::Plural,
                Some(p),
                format!("{base}{suf}"),
            );
        }
        // The third person has two accepted shapes: the contracted
        // `-ineen` and the full `-inensa`.
        for suffix in ["ineen", "inensa"] {
            self.push(
                Case::Comitative,
                Number::Plural,
                Some(Possessive::Third),
                format!("{base}{suffix}"),
            );
        }
    }

    /// Push just the possessive column of a case cell on `base` (for the cases
    /// that carry a second spelling's possessives, e.g. the `-kseen` and the
    /// evaluative `E`-stem columns).
    pub fn possess(&mut self, case: Case, num: Number, base: &str, double3: bool) {
        if !self.possessives {
            return;
        }
        for (suf, p) in PERSONAL {
            self.push(case, num, Some(*p), format!("{base}{suf}"));
        }
        self.push(
            case,
            num,
            Some(Possessive::Third),
            format!("{base}{}", self.h("nsa", "nsä")),
        );
        if double3 {
            let lastv = base.chars().last().unwrap();
            self.push(
                case,
                num,
                Some(Possessive::Third),
                format!("{base}{lastv}n"),
            );
        }
    }

    /// Push a single rare/alternant form -- the short genitive plural
    /// (`kalain` beside `kalojen`) that the catalogue wraps in parentheses. It
    /// is generated so a catalogue model is reproduced, but at a lower
    /// confidence so it is never `plain` (offered as a correction).
    pub fn push_rare(&mut self, case: Case, num: Number, word: String) {
        let tags = Tags::of_number(case, num).of_pos(PartOfSpeech::Noun);
        self.out
            .push(Form::new(word, tags).with(Confidence::Repaired));
    }

    /// Push a rare/alternant form that belongs to an owner, so the possessive is
    /// part of the reading rather than only of the spelling. See the same method
    /// in `nominals::utility` for why the owning and owning-less versions have
    /// to be separate calls.
    pub fn push_rare_owned(
        &mut self,
        case: Case,
        num: Number,
        poss: Option<Possessive>,
        word: String,
    ) {
        let tags = nominal_tags(PartOfSpeech::Noun, case, num, poss);
        self.out
            .push(Form::new(word, tags).with(Confidence::Repaired));
    }

    /// A whole rare cell: the bare form and its possessed twins, all offered
    /// only as repaired guesses. Use it for an alternant that is real for some
    /// words but not for the one at hand (the `-iden`/`-itten`/`-ita` plurals
    /// are genuine only for the loanwords that confirm them).
    pub fn cell_rare(
        &mut self,
        case: Case,
        num: Number,
        bare: String,
        base: String,
        double3: bool,
    ) {
        self.push_rare(case, num, bare);
        if !self.possessives {
            return;
        }
        for (suf, p) in PERSONAL {
            self.push_rare_owned(case, num, Some(*p), format!("{base}{suf}"));
        }
        self.push_rare_owned(
            case,
            num,
            Some(Possessive::Third),
            format!("{base}{}", self.h("nsa", "nsä")),
        );
        if double3 {
            let lastv = base.chars().last().unwrap();
            self.push_rare_owned(
                case,
                num,
                Some(Possessive::Third),
                format!("{base}{lastv}n"),
            );
        }
    }

    pub fn into_forms(self) -> Vec<Form> {
        self.out
    }
}

/// The `i`-stem of a loanword `-i` noun: the lemma, made to end in `i`
/// (`argon -> argoni`).
pub fn i_stem(lemma: &str) -> String {
    if lemma.ends_with('i') {
        lemma.to_string()
    } else {
        format!("{lemma}i")
    }
}

/// The `e`-stem, the `i`-stem with its final `i` read `e` (`aare`, `argone`).
pub fn e_stem(istem: &str) -> String {
    let mut s = istem.to_string();
    s.pop(); // drop the final `i`
    s.push('e');
    s
}

/// Which of the `-i`-plural alternants (`-iden`/`-itten`/`-ita`) a word really
/// confirms, deciding whether they are offered as plain corrections or as
/// repaired guesses. Class 6 is per-lemma; class 5 is all-or-none.
#[derive(Clone, Copy, PartialEq)]
pub struct IPlural {
    iden: bool,
    itten: bool,
    ita: bool,
}

impl IPlural {
    pub const NONE: Self = Self {
        iden: false,
        itten: false,
        ita: false,
    };
    pub const ALL: Self = Self {
        iden: true,
        itten: true,
        ita: true,
    };

    /// A mask with bit 1 = `-iden`, bit 2 = `-itten`, bit 4 = `-ita`.
    pub fn from_mask(mask: u8) -> Self {
        Self {
            iden: mask & 1 != 0,
            itten: mask & 2 != 0,
            ita: mask & 4 != 0,
        }
    }

    fn iden_plain(self) -> bool {
        self.iden
    }
    fn itten_plain(self) -> bool {
        self.itten
    }
    fn ita_plain(self) -> bool {
        self.ita
    }
}
/// The shared paradigm of the non-gradating loanword `-i` nouns (classes 5/6):
/// the singular is the `i`-stem plus each ending (the illative doubling the
/// `i`), and the plural grows from the `e`-stem with the full alternation --
/// the `i`-stem `-en` genitive plus the `e`-stem `-eiden`/`-eitten` genitive
/// and `-ja`/`-ita` partitive. The bare nominative is the lemma (for a
/// consonant-final word).
pub fn build_i_loans(lemma: &str, possessives: bool, front: bool, ipl: IPlural) -> Vec<Form> {
    build_i_loans_with_nom_base(lemma, possessives, front, ipl, None)
}

/// [`build_i_loans`] with a caller-chosen base for the two nominative
/// possessive columns. Native words that also carry a class-5 dictionary row
/// take `kuoreni`/`vyyhteni` there, while the rest of the class-5 grid stays on
/// the i-stem.
pub fn build_i_loans_with_nom_base(
    lemma: &str,
    possessives: bool,
    front: bool,
    ipl: IPlural,
    nom_base: Option<&str>,
) -> Vec<Form> {
    build_i_loans_impl(lemma, possessives, front, ipl, nom_base, false)
}

/// The consonant-final `-i` loan paradigm (class 22: `nougat`, `gourmet`).
///
/// A vowel-final loan elides its final `i` to `e` before the plural obliques
/// (`paperi` -> `papereissa`). A consonant-final loan keeps the `i`, so the
/// same `i`-stem carries the whole plural (`nougatissa`, `nougatilta`) and the
/// partitive and illative take the bare `-ta`/`-hin` endings instead of the
/// `e`-stem `-ja`/`-ihin` ones.
pub fn build_i_loans_consonant(
    lemma: &str,
    possessives: bool,
    front: bool,
    ipl: IPlural,
) -> Vec<Form> {
    build_i_loans_impl(lemma, possessives, front, ipl, None, true)
}

fn build_i_loans_impl(
    lemma: &str,
    possessives: bool,
    front: bool,
    ipl: IPlural,
    nom_base: Option<&str>,
    consonant_plural: bool,
) -> Vec<Form> {
    let mut b = Builder::new_harmony(lemma, possessives, front);
    let istem = i_stem(lemma);
    let estem = e_stem(&istem);
    let nom_base = nom_base
        .map(str::to_string)
        .unwrap_or_else(|| istem.clone());
    let pstem = if consonant_plural {
        istem.clone()
    } else {
        estem.clone()
    };
    let (sg, pl) = (Number::Singular, Number::Plural);

    // The `-i`-plural alternants on the `e`-stem: genitive `-iden`, genitive
    // `-itten`, partitive `-ita` -- fitness -> fitnesseiden, fitnesseitten,
    // fitnesseita. They come as a set, and a word either takes all three or
    // none, so one flag decides all three (see `IPlural`). They are offered
    // either way: for a word the reference does not confirm them the forms are
    // real for other words of the class but repaired guesses here (`aari` takes
    // only aarien/aareja).
    let iden = cell(Case::Genitive, pl, &estem, ["iden", "iden"], false).on_ending(["ide", "ide"]);
    let iden = if ipl.iden_plain() {
        iden
    } else {
        iden.as_rare()
    };
    let itten =
        cell(Case::Genitive, pl, &estem, ["itten", "itten"], false).on_ending(["itte", "itte"]);
    let itten = if ipl.itten_plain() {
        itten
    } else {
        itten.as_rare()
    };
    let ita = cell(Case::Partitive, pl, &estem, ["ita", "itä"], true);
    let ita = if ipl.ita_plain() { ita } else { ita.as_rare() };

    // A consonant-final loan carries the plural `i` in its stem, so the
    // oblique endings drop the `i` the vowel-final `e`-stem endings begin
    // with: `gourmetilta`, not `gourmetiilta`. The partitive and illative
    // keep their own bare shape (`gourmetita`, `gourmetihin`).
    let part = if consonant_plural {
        ["ta", "tä"]
    } else {
        ["ja", "jä"]
    };
    let ess = if consonant_plural {
        ["na", "nä"]
    } else {
        ["ina", "inä"]
    };
    let tra = if consonant_plural {
        ["ksi", "ksi"]
    } else {
        ["iksi", "iksi"]
    };
    let tra_end = if consonant_plural {
        ["kse", "kse"]
    } else {
        ["ikse", "ikse"]
    };
    let ine = if consonant_plural {
        ["ssa", "ssä"]
    } else {
        ["issa", "issä"]
    };
    let ela = if consonant_plural {
        ["sta", "stä"]
    } else {
        ["ista", "istä"]
    };
    let ill = if consonant_plural {
        ["hin", "hin"]
    } else {
        ["ihin", "ihin"]
    };
    let ill_end = if consonant_plural {
        ["hi", "hi"]
    } else {
        ["ihi", "ihi"]
    };
    let ade = if consonant_plural {
        ["lla", "llä"]
    } else {
        ["illa", "illä"]
    };
    let abl = if consonant_plural {
        ["lta", "ltä"]
    } else {
        ["ilta", "iltä"]
    };
    let all = if consonant_plural {
        ["lle", "lle"]
    } else {
        ["ille", "ille"]
    };
    let abe = if consonant_plural {
        ["tta", "ttä"]
    } else {
        ["itta", "ittä"]
    };
    let ins = if consonant_plural {
        ["n", "n"]
    } else {
        ["in", "in"]
    };

    let mut cells = vec![
        // ---- the singular, on the i-stem ----
        // Bare nominative is the lemma (argon); its possessives sit on the
        // i-stem (argonin).
        cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(nom_base.clone(), ["", ""]),
        cell(Case::Genitive, sg, &istem, ["n", "n"], false).base_on(nom_base.clone(), ["", ""]),
        cell(Case::Partitive, sg, &istem, ["a", "ä"], true),
        cell(Case::Essive, sg, &istem, ["na", "nä"], true),
        cell(Case::Translative, sg, &istem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Inessive, sg, &istem, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, &istem, ["sta", "stä"], true),
        cell(Case::Adessive, sg, &istem, ["lla", "llä"], true),
        cell(Case::Ablative, sg, &istem, ["lta", "ltä"], true),
        cell(Case::Allative, sg, &istem, ["lle", "lle"], true),
        cell(Case::Abessive, sg, &istem, ["tta", "ttä"], true),
        // ---- the plural ----
        cell(Case::Nominative, pl, &istem, ["t", "t"], false).base_on(nom_base.clone(), ["", ""]),
        // Genitive plural keeps the i: aari -> aarien.
        cell(Case::Genitive, pl, &istem, ["en", "en"], false).on_ending(["e", "e"]),
        // Every other oblique plural cell is on the plural stem: the
        // `e`-stem for a vowel-final loan, the `i`-stem for a
        // consonant-final one.
        cell(Case::Partitive, pl, &pstem, part, true),
        iden,
        itten,
        ita,
        cell(Case::Essive, pl, &pstem, ess, true),
        cell(Case::Translative, pl, &pstem, tra, true).on_ending(tra_end),
        cell(Case::Inessive, pl, &pstem, ine, true),
        cell(Case::Elative, pl, &pstem, ela, true),
        cell(Case::Illative, pl, &pstem, ill, false).on_ending(ill_end),
        cell(Case::Adessive, pl, &pstem, ade, true),
        cell(Case::Ablative, pl, &pstem, abl, true),
        cell(Case::Allative, pl, &pstem, all, true),
        cell(Case::Abessive, pl, &pstem, abe, true),
        plain(Case::Instructive, pl, &pstem, ins),
    ];
    // A vowel-final loan doubles the i for its singular illative
    // (`aariin`). A consonant-final loan has a lexical `-han`/`-hen`
    // illative instead (class 22 adds it), so the doubled-i shape is only a
    // repaired guess there (`nougatiin` is not a plain word).
    if !consonant_plural {
        cells.push(cell(Case::Illative, sg, &istem, ["in", "in"], false).on_ending(["i", "i"]));
    } else {
        // The curated class-22 catalogue still carries the i-stem illative
        // (`showiin`) and the e-stem partitive/illative plurals (`showeja`,
        // `showeihin`). They are not the confirmed shape for a consonant-final
        // loan, so they stay repaired, but the catalogue has to be reproduced.
        cells.push(
            cell(Case::Illative, sg, &istem, ["in", "in"], false)
                .on_ending(["i", "i"])
                .as_rare(),
        );
        cells.push(cell(Case::Partitive, pl, &estem, ["ja", "jä"], true).as_rare());
        cells.push(
            cell(Case::Illative, pl, &estem, ["ihin", "ihin"], false)
                .on_ending(["ihi", "ihi"])
                .as_rare(),
        );
    }
    b.decline(&Decl::new(cells), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(&pstem);

    b.into_forms()
}

/// Keeps only the plural cells of a paradigm.
///
/// A plural-only lemma (`aktiivat`, `housut`, `käräjät`) is the nominative
/// plural of a word the sanalista does not list separately, so a class engine
/// is run on the dropped-`t` stem (`aktiiva`, `housu`, `käräjä`) and the
/// singular cells it builds (`aktiiva`, `aktiivan`) are not forms of the word.
/// The nominative plural the stem produces is the lemma itself, so nothing has
/// to be rewritten.
///
/// A form is kept when *any* of its readings is plural. The class engines tag
/// each cell with one number, so this is the cell's own number; the `any` is
/// there for a form that later carries two readings and is plural under one.
pub fn plural_only(forms: Vec<Form>) -> Vec<Form> {
    forms
        .into_iter()
        .filter(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    // The classes this sweep reads. Named here rather than globbed: they are
    // siblings of `utility`, not items of it, so `use super::*` does not reach
    // them, and the live tree spelled them out at file scope for the same
    // reason.
    use crate::expand::nominals::{
        class_11, class_13, class_17, class_18, class_20, class_22, class_24, class_28J, class_45,
    };

    /// A rare cell's possessive column says whose each form is.
    ///
    /// `cell_rare` emits a whole cell at repaired confidence -- the alternants
    /// the catalogue wraps in parentheses, real for some words of a class and
    /// not for the one at hand -- and it used to send those five suffixes through
    /// `push_rare`, which takes no owner. So `ehtooheni` came back tagged as a
    /// bare singular illative: the right word, the right case, and nothing in
    /// the reading to say it was the `-ni` possessive. Across the noun corpus
    /// that was 62 748 forms over 11 363 lemmas, so the test is written as an
    /// invariant over the classes that have rare cells rather than as a check of
    /// one spelling.
    ///
    /// The spelling is what makes it a usable invariant: a form ending in one of
    /// the personal suffixes, or in the third person's `-nsa`, is a possessive,
    /// whatever the reading says about it.
    #[test]
    fn a_rare_possessive_says_whose_it_is() {
        const OWNED: [&str; 6] = ["ni", "si", "mme", "nne", "nsa", "nsä"];
        // One word per class that has a rare cell, so the sweep covers all of
        // them rather than whichever one a fixture happened to pick.
        let cases: [(fn(&str, bool) -> Vec<Form>, &str); 9] = [
            (class_11::inflect, "kynnys"),
            (class_13::inflect, "sähkö"),
            (class_17::inflect, "ehtoo"),
            (class_18::inflect, "aamunkoi"),
            (class_20::inflect, "maa"),
            (class_24::inflect, "tahti"),
            (class_28J::inflect, "kylmä"),
            (class_45::inflect, "koukeri"),
            (class_22::inflect, "kallis"),
        ];

        // Candidates are the repaired forms whose *spelling* is a possessive,
        // tagged or not; misses are the candidates that did not say so.
        let mut candidates = 0usize;
        let mut missed = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for (inflect, lemma) in cases {
            for f in inflect(lemma, true) {
                // The translative is excluded because its `-ksi`/`-iksi` ends
                // in `si` by coincidence: `kallis'ksi` is a translative, not
                // the `-si` possessive, and counting it would make this test
                // fail on a word that is right.
                if !OWNED.iter().any(|s| f.word.ends_with(s))
                    || f.confidence != Confidence::Repaired
                    || f.readings.iter().any(|r| r.case == Some(Case::Translative))
                {
                    continue;
                }
                candidates += 1;
                if !f.readings.iter().any(|r| r.possessive.is_some()) {
                    missed += 1;
                    if examples.len() < 8 {
                        examples.push(format!("{} ({lemma})", f.word));
                    }
                }
            }
        }
        assert!(
            candidates > 50,
            "only {candidates} rare possessives examined, so this asserts nothing"
        );
        assert!(
            missed == 0,
            "{missed} of {candidates} rare possessives lost their owner: {examples:?}"
        );
    }
}
