//! Argument completeness: must an argument exist?
//!
//! This is a separate, tiny table from [`super::verb_rection`]. Rection answers
//! which case an argument takes; this table answers whether the argument must
//! be present at all. The CSV is `lemma,slot,case,requirement,note` and the
//! build encodes each frame as `slot:case:requirement` in a front-coded map.
//!
//! Only frames verified against Kielitoimiston ohjepankki or a dictionary
//! example belong here. A `forbidden` row means “this verb takes no object”,
//! not “this verb is intransitive in every construction”.

use crate::expand::static_table::StemMap;
use crate::tags::Case;

static ARGUMENTS: StemMap = StemMap::new(include_bytes!(concat!(
    env!("OUT_DIR"),
    "/verb_arguments.bin",
)));

/// Which slot the frame describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Object,
    Recipient,
    /// A case-governed complement (`luottaa johonkin`).
    Argument,
}

impl Slot {
    pub fn name(self) -> &'static str {
        match self {
            Slot::Object => "object",
            Slot::Recipient => "recipient",
            Slot::Argument => "argument",
        }
    }
}

/// Whether the slot must be present or must be absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    Obligatory,
    Forbidden,
}

impl Requirement {
    pub fn name(self) -> &'static str {
        match self {
            Requirement::Obligatory => "obligatory",
            Requirement::Forbidden => "forbidden",
        }
    }
}

/// One argument-completeness frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub slot: Slot,
    /// `None` means the frame accepts any case (`any` in the CSV).
    pub case: Option<Case>,
    pub requirement: Requirement,
}

/// The frames for a verb lemma, in CSV order.
pub fn frames(lemma: &str) -> impl Iterator<Item = Frame> + '_ {
    ARGUMENTS.bases(lemma).filter_map(parse)
}

fn parse(value: &str) -> Option<Frame> {
    let mut parts = value.split(':');
    let slot = match parts.next()? {
        "object" => Slot::Object,
        "recipient" => Slot::Recipient,
        "argument" => Slot::Argument,
        _ => return None,
    };
    let case = match parts.next()? {
        "any" => None,
        name => Some(
            Case::ALL
                .into_iter()
                .find(|case| case.name() == name)?,
        ),
    };
    let requirement = match parts.next()? {
        "obligatory" => Requirement::Obligatory,
        "forbidden" => Requirement::Forbidden,
        _ => return None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(Frame {
        slot,
        case,
        requirement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_obligatory_object_frame_is_listed() {
        let frames: Vec<Frame> = frames("rakastaa").collect();
        assert_eq!(
            frames,
            vec![Frame {
                slot: Slot::Object,
                case: Some(Case::Partitive),
                requirement: Requirement::Obligatory,
            }]
        );
    }

    #[test]
    fn a_case_governed_argument_frame_is_listed() {
        let frames: Vec<Frame> = frames("luottaa").collect();
        assert_eq!(frames[0].slot, Slot::Argument);
        assert_eq!(frames[0].case, Some(Case::Illative));
        assert_eq!(frames[0].requirement, Requirement::Obligatory);
    }

    #[test]
    fn an_any_case_forbidden_frame_has_no_case() {
        let frames: Vec<Frame> = frames("herätä").collect();
        assert_eq!(frames[0].slot, Slot::Object);
        assert_eq!(frames[0].case, None);
        assert_eq!(frames[0].requirement, Requirement::Forbidden);
    }

    #[test]
    fn an_unknown_verb_has_no_frames() {
        assert_eq!(frames("syödä").count(), 0);
    }
}
