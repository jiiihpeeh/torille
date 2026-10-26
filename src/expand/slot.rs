use crate::tags::{Case, Number};

/// The inflection class of a noun/verb template, positionally.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Slot {
    Nom,
    Gen,
    Part,
    Ill,
    PlNom,
    PlGen,
    PlPart,
    PlIll,
    Extra,
}

/// Maps a catalogue slot index to its role. The catalogue always lists the
/// singular nominative, genitive, partitive and illative first, then the same
/// four for the plural, so the order is stable across classes.
pub(super) fn slot_role(i: usize) -> Slot {
    match i {
        0 => Slot::Nom,
        1 => Slot::Gen,
        2 => Slot::Part,
        3 => Slot::Ill,
        4 => Slot::PlNom,
        5 => Slot::PlGen,
        6 => Slot::PlPart,
        7 => Slot::PlIll,
        _ => Slot::Extra,
    }
}

impl Slot {
    /// The position of the slot in the catalogue.
    pub(super) fn index(self) -> usize {
        match self {
            Slot::Nom => 0,
            Slot::Gen => 1,
            Slot::Part => 2,
            Slot::Ill => 3,
            Slot::PlNom => 4,
            Slot::PlGen => 5,
            Slot::PlPart => 6,
            Slot::PlIll => 7,
            Slot::Extra => 8,
        }
    }

    /// The case and number a slot stands for.
    ///
    /// The catalogue says which slot is which by position alone, so this is
    /// where a generated form gets its grammatical tags without anybody having
    /// to label the data. A class with more than eight forms is running past the
    /// end of that order, and a slot there is whatever the class uses it for, so
    /// it carries no tags at all.
    pub(super) fn nominal(self) -> Option<(Case, Number)> {
        crate::kotus::slot_nominal(self.index())
    }

    /// Whether this slot takes the weak grade of a gradating stem.
    ///
    /// Finnish gradation is not a universal rule but a property of each
    /// inflectional form: the genitive and the partitive are weak (`pankki` is
    /// `pankin` and `pankkia`) while the illative and the essive are strong
    /// (`pankkiin` and `pankkina`). A class overrides this where its members
    /// disagree, but for the ordinary noun paradigm the grade follows the case.
    ///
    /// The plural is strong where its stem is the ungradated word and weak
    /// where it is not; the catalogue gives the plural stems exactly. In the
    /// standard paradigm the genitive plural keeps the strong grade (`pankki`
    /// is `pankkien`, `joki` is `jokien`, `umpi` is `umpien`), which is why a
    /// gradating word is not weakened there.
    pub(super) fn grade(self) -> crate::kotus::Grade {
        use crate::kotus::Grade;
        match self {
            Slot::Gen | Slot::PlNom => Grade::Weak,
            Slot::Nom | Slot::Part | Slot::Ill | Slot::PlGen | Slot::PlPart | Slot::PlIll => {
                Grade::Strong
            }
            Slot::Extra => Grade::Strong,
        }
    }
}
