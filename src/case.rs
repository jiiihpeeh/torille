//! A declarative model of the nominal cases.
//!
//! The catalogue-driven generator in `expand` and `grammar::noun` produces the
//! forms; this module is the model the spec asks for on top of it: case as a
//! grammatical feature rather than a string, applied after stem selection,
//! gradation and number, and named by a declarative rule instead of by a
//! hard-coded suffix per case.
//!
//! The fifteen productive cases split naturally into the handful that are a
//! bare suffix with a back/front alternation (`-ssa`/`-ssä`, `-sta`/`-stä`,
//! `-lla`/`-llä`, `-lta`/`-ltä`, `-na`/`-nä`, `-tta`/`-ttä`, `-a`/`-ä`), the
//! three invariant ones (`-n`, `-lle`, `-ksi`), and the handful that are too
//! irregular for a suffix and get a rule of their own: the illative, the
//! partitive, the comitative, the instructive and the accusative.
//!
//! [`noun_cases`] holds the rule table the spec's `noun.cases` describes, in
//! the exact shape the generator needs: for every productive case, which stem
//! group the ending attaches to, which grade it takes, and which licence it
//! asks of that stem. The tables of `grammar::noun` turn the rows into forms;
//! this is the table itself. `docs/morfologia.yaml` is the human copy and the
//! tests below check the two stay in step.

use crate::tags::{Case, Number};

/// The grade of a stem, for the gradation that case selection interacts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    Strong,
    Weak,
}

/// The stem group a case ending attaches to.
///
/// This is the same distinction the spec draws: the cases that end in `-lla`,
/// `-lta`, `-lle` and `-ksi` take the genitive stem, the inessive and elative
/// take the inessive stem, the essive takes the stem the class names for it
/// (or the genitive stem, strong), and the plural cases take one of the two
/// plural stems. The plural stems are the callers': the weak grade that the
/// local cases and the instructive read out of the word, and the strong grade
/// the essive and the comitative hang off, which is the class's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemGroup {
    /// The genitive stem: `-lla`, `-lta`, `-lle`, `-ksi`, `-tta`, `-a`.
    Genitive,
    /// The inessive stem: `-ssa`, `-sta`.
    Stems,
    /// The essive stem of the class, where one is named, else the genitive.
    Essive,
    /// The plural stem in its weak grade.
    PluralWeak,
    /// The plural stem in its strong grade.
    PluralStrong,
}

/// What a case ending asks of the stem it hangs off, mirroring
/// `grammar::noun::Licence` per row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Licence {
    /// The ending attaches to the stem as it stands: `-lla`, `-lta`, `-lle`,
    /// `-ksi`, `-tta`.
    Bare,
    /// `-ssa`/`-sta` straight after a vowel-final stem.
    Ssa,
    /// `-essa`/`-estä` after the `s` or `n` of a cut stem.
    Essa,
    /// `-ella`/`-eltä`/`-elle` after the `s` or `n` of a cut stem.
    E,
    /// `-na`/`-nä` on the strong genitive stem of a class that reaches it
    /// without substituting a consonant.
    Essive,
    /// The plural, whose `i` goes between the stem and the ending: the plural
    /// cases, the instructive and the comitative.
    Plural,
    /// No rule licenses the reading; the catalogue tabulates it instead.
    Reading,
}

/// An allomorph a case takes after a stem that ends in `s` or `n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allomorph {
    pub back: &'static str,
    pub front: &'static str,
    pub licence: Licence,
}

/// A case ending, one per (case, number) row of the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseDef {
    pub case: Case,
    pub number: Number,
    pub stem: StemGroup,
    pub grade: Grade,
    pub back: &'static str,
    pub front: &'static str,
    pub licence: Licence,
    /// The `s`/`n` allomorph of the same case, when there is one.
    pub after_s_n: Option<Allomorph>,
}

/// The rule table behind `docs/morfologia.yaml`'s `noun.cases`, one row per
/// productive (case, number) pair the generator owns. The three cases the
/// catalogue gives per lexeme (nominative, genitive, illative) have no row:
/// they are not regular, they are lexical.
pub fn noun_cases() -> &'static [CaseDef] {
    use Case::{
        Abessive, Ablative, Adessive, Allative, Comitative, Elative, Inessive, Instructive,
        Partitive, Translative,
    };
    use Grade::{Strong, Weak};
    use Licence::{Bare, E, Essa, Reading, Ssa};
    use Number::Singular;
    use StemGroup::{Genitive, PluralStrong, PluralWeak, Stems};
    const TWO: Option<Allomorph> = None;
    const C: &[CaseDef] = &[
        // The singular, on the genitive stem.
        CaseDef {
            case: Partitive,
            number: Singular,
            stem: Genitive,
            grade: Strong,
            back: "a",
            front: "ä",
            licence: Reading,
            after_s_n: TWO,
        },
        CaseDef {
            case: Case::Essive,
            number: Singular,
            stem: StemGroup::Essive,
            grade: Strong,
            back: "na",
            front: "nä",
            licence: Licence::Essive,
            after_s_n: TWO,
        },
        CaseDef {
            case: Translative,
            number: Singular,
            stem: Genitive,
            grade: Weak,
            back: "ksi",
            front: "ksi",
            licence: Bare,
            after_s_n: Some(Allomorph {
                back: "eksi",
                front: "eksi",
                licence: E,
            }),
        },
        CaseDef {
            case: Abessive,
            number: Singular,
            stem: Genitive,
            grade: Weak,
            back: "tta",
            front: "ttä",
            licence: Bare,
            after_s_n: Some(Allomorph {
                back: "etta",
                front: "että",
                licence: E,
            }),
        },
        CaseDef {
            case: Inessive,
            number: Singular,
            stem: Stems,
            grade: Weak,
            back: "ssa",
            front: "ssä",
            licence: Ssa,
            after_s_n: Some(Allomorph {
                back: "essa",
                front: "essä",
                licence: Essa,
            }),
        },
        CaseDef {
            case: Elative,
            number: Singular,
            stem: Stems,
            grade: Weak,
            back: "sta",
            front: "stä",
            licence: Ssa,
            after_s_n: Some(Allomorph {
                back: "esta",
                front: "estä",
                licence: Essa,
            }),
        },
        CaseDef {
            case: Adessive,
            number: Singular,
            stem: Genitive,
            grade: Weak,
            back: "lla",
            front: "llä",
            licence: Bare,
            after_s_n: Some(Allomorph {
                back: "ella",
                front: "ellä",
                licence: E,
            }),
        },
        CaseDef {
            case: Ablative,
            number: Singular,
            stem: Genitive,
            grade: Weak,
            back: "lta",
            front: "ltä",
            licence: Bare,
            after_s_n: Some(Allomorph {
                back: "elta",
                front: "eltä",
                licence: E,
            }),
        },
        CaseDef {
            case: Allative,
            number: Singular,
            stem: Genitive,
            grade: Weak,
            back: "lle",
            front: "lle",
            licence: Bare,
            after_s_n: Some(Allomorph {
                back: "elle",
                front: "elle",
                licence: E,
            }),
        },
        // The plural, on the weak or strong plural stem.
        CaseDef {
            case: Inessive,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "issa",
            front: "issä",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Elative,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "ista",
            front: "istä",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Adessive,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "lla",
            front: "llä",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Ablative,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "lta",
            front: "ltä",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Allative,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "lle",
            front: "lle",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Case::Essive,
            number: Number::Plural,
            stem: PluralStrong,
            grade: Strong,
            back: "na",
            front: "nä",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Translative,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "ksi",
            front: "ksi",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Abessive,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "tta",
            front: "ttä",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Comitative,
            number: Number::Plural,
            stem: PluralStrong,
            grade: Strong,
            back: "neen",
            front: "neen",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
        CaseDef {
            case: Instructive,
            number: Number::Plural,
            stem: PluralWeak,
            grade: Weak,
            back: "in",
            front: "in",
            licence: Licence::Plural,
            after_s_n: TWO,
        },
    ];
    C
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Case::*;
    use Grade::{Strong, Weak};
    use Licence::{Bare, Reading, Ssa};
    use Number::Singular;
    use StemGroup::{Genitive, PluralStrong, PluralWeak, Stems};

    fn row(case: Case, number: Number) -> &'static CaseDef {
        noun_cases()
            .iter()
            .find(|r| r.case == case && r.number == number)
            .unwrap_or_else(|| panic!("{case:?} {number:?} has no row"))
    }

    #[test]
    fn the_case_table_covers_every_productive_case_twice() {
        // The parlour of the catalogue is nominative, genitive and illative,
        // which are lexical; the nineteen rows of the table are the uniform
        // nine in the singular and the ten in the plural.
        let singulars = [
            Partitive,
            Essive,
            Translative,
            Abessive,
            Inessive,
            Elative,
            Adessive,
            Ablative,
            Allative,
        ];
        let plurals = [
            Inessive,
            Elative,
            Adessive,
            Ablative,
            Allative,
            Essive,
            Translative,
            Abessive,
            Comitative,
            Instructive,
        ];
        for case in singulars {
            assert!(
                noun_cases()
                    .iter()
                    .any(|r| r.case == case && r.number == Singular),
                "{case:?} is missing a singular row"
            );
        }
        for case in plurals {
            assert!(
                noun_cases()
                    .iter()
                    .any(|r| r.case == case && r.number == Number::Plural),
                "{case:?} is missing a plural row"
            );
        }
    }

    #[test]
    fn the_local_cases_take_the_genitive_stem_in_the_weak_grade() {
        // `-lla`, `-lta`, `-lle`, `-ksi` and `-tta` all hang off the genitive
        // stem and see it weak; the inessive and elative take the inessive stem
        // and see it weak; the partitive is strong on the genitive stem and the
        // essive strong on the essive stem.
        for case in [Adessive, Ablative, Allative, Translative, Abessive] {
            assert_eq!(row(case, Singular).stem, Genitive);
            assert_eq!(row(case, Singular).grade, Weak);
            assert_eq!(row(case, Singular).licence, Bare);
        }
        for case in [Inessive, Elative] {
            assert_eq!(row(case, Singular).stem, Stems);
            assert_eq!(row(case, Singular).grade, Weak);
            assert_eq!(row(case, Singular).licence, Ssa);
            assert!(row(case, Singular).after_s_n.is_some());
        }
        assert_eq!(row(Partitive, Singular).stem, Genitive);
        assert_eq!(row(Partitive, Singular).grade, Strong);
        assert_eq!(row(Partitive, Singular).licence, Reading);
        assert_eq!(row(Essive, Singular).stem, StemGroup::Essive);
        assert_eq!(row(Essive, Singular).grade, Strong);
        assert_eq!(row(Essive, Singular).licence, Licence::Essive);
    }

    #[test]
    fn the_plural_rows_split_weak_from_strong() {
        // The local plurals and the instructive take the weak plural stem; the
        // essive and the comitative keep the strong one.
        for case in [
            Inessive,
            Elative,
            Adessive,
            Ablative,
            Allative,
            Translative,
            Abessive,
            Instructive,
        ] {
            assert_eq!(row(case, Number::Plural).stem, PluralWeak, "{case:?}");
            assert_eq!(row(case, Number::Plural).grade, Weak, "{case:?}");
            assert_eq!(
                row(case, Number::Plural).licence,
                Licence::Plural,
                "{case:?}"
            );
        }
        for case in [Essive, Comitative] {
            assert_eq!(row(case, Number::Plural).stem, PluralStrong, "{case:?}");
            assert_eq!(row(case, Number::Plural).grade, Strong, "{case:?}");
            assert_eq!(
                row(case, Number::Plural).licence,
                Licence::Plural,
                "{case:?}"
            );
        }
    }

    // ── docs/morfologia.yaml drift alarm ────────────────────────────────
    //
    // The YAML is the human-readable copy of this table and is not parsed at
    // runtime. Keep the two in step with this test rather than with a comment:
    // it extracts the `noun.cases` rows and compares every field the table
    // stores. It is deliberately a small hand parser (the block's shape is
    // fixed) so the crate needs no YAML dependency.

    #[derive(Debug, PartialEq, Eq)]
    struct SpecRow {
        case: Case,
        number: Number,
        stem: StemGroup,
        grade: Grade,
        back: String,
        front: String,
        licence: Licence,
        after_s_n: Option<(String, String, Licence)>,
    }

    fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
        let pat = format!("{key}:");
        let start = text.find(&pat)? + pat.len();
        let rest = text[start..].trim_start();
        if rest.starts_with('{') {
            let mut depth = 0usize;
            for (i, ch) in rest.char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(rest[..=i].trim());
                        }
                    }
                    _ => {}
                }
            }
            return None;
        }
        let end = rest.find([',', '\n', '}']).unwrap_or(rest.len());
        let value = rest[..end].split('#').next().unwrap_or("").trim();
        if value.is_empty() { None } else { Some(value) }
    }

    fn ending(value: &str) -> (String, String) {
        if value.starts_with('{') {
            (
                field(value, "back").expect("ending.back").to_string(),
                field(value, "front").expect("ending.front").to_string(),
            )
        } else {
            (value.to_string(), value.to_string())
        }
    }

    fn spec_row(entry: &str) -> SpecRow {
        let case_name = field(entry, "case").expect("case");
        // `instructive` is the plural row in the spec; the other plural rows
        // spell it out (`inessive_plural`, ...).
        let (base, plural) = match case_name.strip_suffix("_plural") {
            Some(base) => (base, true),
            None => (case_name, case_name == "instructive"),
        };
        let case = match base {
            "partitive" => Case::Partitive,
            "essive" => Case::Essive,
            "translative" => Case::Translative,
            "abessive" => Case::Abessive,
            "inessive" => Case::Inessive,
            "elative" => Case::Elative,
            "adessive" => Case::Adessive,
            "ablative" => Case::Ablative,
            "allative" => Case::Allative,
            "comitative" => Case::Comitative,
            "instructive" => Case::Instructive,
            other => panic!("unknown case in morfologia.yaml: {other}"),
        };
        let stem = match field(entry, "stem").expect("stem") {
            "genitive" => StemGroup::Genitive,
            "stems" => StemGroup::Stems,
            "essive" => StemGroup::Essive,
            "plural_weak" => StemGroup::PluralWeak,
            "plural_strong" => StemGroup::PluralStrong,
            other => panic!("unknown stem group in morfologia.yaml: {other}"),
        };
        let grade = match field(entry, "grade").expect("grade") {
            "strong" => Grade::Strong,
            "weak" => Grade::Weak,
            other => panic!("unknown grade in morfologia.yaml: {other}"),
        };
        let licence = |name: &str| match name {
            // The spec calls the lexical partitive "none"; the engine's row
            // says `Reading` (no rule licenses it, the catalogue tabulates it).
            "none" => Licence::Reading,
            "bare" => Licence::Bare,
            "ssa" => Licence::Ssa,
            "essa" => Licence::Essa,
            "e" => Licence::E,
            "essive" => Licence::Essive,
            "plural" => Licence::Plural,
            other => panic!("unknown licence in morfologia.yaml: {other}"),
        };
        let (back, front) = ending(field(entry, "ending").expect("ending"));
        let after_s_n = entry.find("after_s_n:").map(|at| {
            let tail = &entry[at + "after_s_n:".len()..];
            let (b, f) = ending(field(tail, "ending").expect("after_s_n.ending"));
            let l = licence(field(tail, "licence").expect("after_s_n.licence"));
            (b, f, l)
        });
        SpecRow {
            case,
            number: if plural { Number::Plural } else { Number::Singular },
            stem,
            grade,
            back,
            front,
            licence: licence(field(entry, "licence").expect("licence")),
            after_s_n,
        }
    }

    fn parse_spec_cases(yaml: &str) -> Vec<SpecRow> {
        let mut rows = Vec::new();
        let mut in_cases = false;
        let mut current: Option<String> = None;
        for raw in yaml.lines() {
            let indent = raw.len() - raw.trim_start().len();
            if !in_cases {
                if indent == 2 && raw.trim() == "cases:" {
                    in_cases = true;
                }
                continue;
            }
            if indent < 4 && !raw.trim().is_empty() {
                break; // the next top-level key ends the block
            }
            let trimmed = raw.trim_start();
            if indent == 4 && trimmed.starts_with("- ") {
                if let Some(entry) = current.take() {
                    rows.push(spec_row(&entry));
                }
                current = Some(format!("{}\n", &trimmed[2..]));
            } else if let Some(entry) = current.as_mut() {
                entry.push_str(raw);
                entry.push('\n');
            }
        }
        if let Some(entry) = current {
            rows.push(spec_row(&entry));
        }
        rows
    }

    fn render_spec(r: &SpecRow) -> String {
        format!(
            "{:?}/{:?} {:?} {:?} {}/{} {:?} after={:?}",
            r.case, r.number, r.stem, r.grade, r.back, r.front, r.licence,
            r.after_s_n,
        )
    }

    fn render_rust(r: &CaseDef) -> String {
        format!(
            "{:?}/{:?} {:?} {:?} {}/{} {:?} after={:?}",
            r.case,
            r.number,
            r.stem,
            r.grade,
            r.back,
            r.front,
            r.licence,
            r.after_s_n
                .map(|a| (a.back.to_string(), a.front.to_string(), a.licence)),
        )
    }

    #[test]
    fn the_yaml_spec_matches_the_case_table() {
        let spec = parse_spec_cases(include_str!("../docs/morfologia.yaml"));
        assert_eq!(spec.len(), noun_cases().len(), "case row count drifted");
        let mut want: Vec<String> = spec.iter().map(render_spec).collect();
        let mut have: Vec<String> = noun_cases().iter().map(render_rust).collect();
        want.sort();
        have.sort();
        assert_eq!(
            have, want,
            "docs/morfologia.yaml and src/case.rs disagree; the YAML is the \
             human copy, the Rust table is what the engine runs"
        );
    }
}
