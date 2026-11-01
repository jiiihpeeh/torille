#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""The curated evidence behind a materialised-oracle disagreement.

The report tool and its evidence gate both need to answer one question: is this
`illegal` row a real engine defect, or is the oracle's expected spelling the
wrong word? The answer is not a class-wide exemption -- a genuine defect in the
same class would disappear -- so every rule names the class, the cell and the
form shape it covers, and carries the source that settles it.

The file is `data/oracle_exceptions.json`; see
`TODO_ILLEGAL_REMOVAL_PLAN.md` for the phases that fill it in.

A rule is a JSON object:

    {
      "id": "34c-nom-possessive-genitive-stem",
      "classes": ["34*C"],
      "cell": {"case": "NOM", "possessive": "*"},
      "predicate": "oblique-stem",
      "source": "https://en.wiktionary.org/wiki/levoton",
      "note": "..."
    }

`classes` and `lemmas` are optional filters; at least one is required except
for the structural `strong-comparative` rule. `cell`
is an optional filter over the finding's cell (`case`, `number`, `possessive`);
`"*"` is a wildcard. An optional `sample` names the per-class sample that first
exercised the rule, so a rule discovered by a wider sweep is not called stale
by the 5-lemma baseline. `applies_to` names the finding statuses the rule
explains: `["illegal"]` (the default) for a wrong engine form, `["missing"]`
for an oracle spelling that is not a word. `predicate` is one of:

* `lemma` -- the class/lemma filter is the whole rule.
* `also-class` -- the lemma also belongs to one of `also_classes`, which is how
  a dual-class word (`omakuva` is 51 and 10) is handled without naming it.
* `oblique-stem` -- the engine form starts with the class's oblique stem
  (`levottoma-` for `levoton`).
* `shorter-than-expected` -- the engine form is a proper subsequence of one of
  the oracle's expected spellings. This is the shape of the plural-only
  `-imet`/`-aat` caveat: `näkimittä` is a subsequence of `näkimkemittä`.
* `fixed-prefix` -- the engine keeps a compound modifier unchanged and the
  oracle inflects it into the middle of the word (`kuiva` + `kakku` against
  `kuiviin` + `kakku`). The head is found by the same sanalista split the
  engine uses.
* `case-less` -- the reading has no case tag, which is how a partitive or
  illative possessive (`mustaherukkaani`) is checked against the nominative
  cell whose correct form (`mustaherukkani`) is also emitted.
* `plural-only-t` -- the lemma is a `-t`-final pluralia tantum in one of the
  rule's classes (`aktiivat`, `rintsikat`, `menkat`); the engine declines the
  dropped-`t` base and the oracle builds every plural cell from the nominative
  plural.
* `strong-comparative` -- the oracle's expected form keeps the comparative's
  strong `-mpa-` grade in a cell that takes the weak `-mm-` (`kuurompalta`,
  `alamaisempalta`). Structural, so it needs no `classes`; it applies to
  `missing` rows and marks them `oracle-only`.

`exclude_lemmas` is an optional list for a rule that would otherwise overlap a
lemma-specific rule; the excluded lemma is left to the more precise rule.

The gate `tools/test_illegal_evidence.py` asserts that every default-report
`illegal` row matches exactly one rule and that every rule still matches at
least one row. A rule that stops matching is stale and must be removed; a row
that matches nothing is a new defect that needs triage.
"""

from __future__ import annotations

import functools
import json
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_PATH = REPO / "data" / "oracle_exceptions.json"

PREDICATES = {
    "lemma", "also-class", "oblique-stem", "shorter-than-expected",
    "shorter", "fixed-prefix", "case-less", "plural-only-t",
    "strong-comparative", "plural-only-iset",
}
CELL_FIELDS = {"case", "number", "possessive"}
CASE_TAGS = {
    "nominative", "accusative", "genitive", "partitive", "essive",
    "translative", "inessive", "elative", "illative", "adessive",
    "ablative", "allative", "abessive", "instructive", "comitative",
    "prolative",
}
#: Cases where a comparative's `-mpi` takes the weak `-mm-` grade. The strong
#: `-mpa-` belongs to the nominative, partitive, illative and essive only;
#: the oracle sometimes keeps it in these open-syllable cases (`kuurompalta`).
WEAK_COMPARATIVE_CASES = {
    "GEN", "INE", "ELA", "ADE", "ABL", "ALL", "TRA", "ABE", "COM", "INS",
}
RULE_FIELDS = {
    "id", "classes", "lemmas", "exclude_lemmas", "cell", "predicate",
    "source", "note", "also_classes", "sample", "applies_to",
}

#: The per-class sample a rule is expected to be exercised by. The baseline
#: report is 5 lemmas per class; a rule discovered by a wider sweep is marked
#: with the sample that first exercised it, so the 5-lemma gate does not call
#: it stale merely because its lemma was not drawn in the baseline.
DEFAULT_SAMPLE = 5


def sample_of(rule: dict) -> int:
    """The per-class sample a rule is expected to be exercised by."""
    return rule.get("sample", DEFAULT_SAMPLE)


class EvidenceError(ValueError):
    """The evidence file or a rule is malformed."""


def load(path: str | Path = DEFAULT_PATH) -> list[dict]:
    """Read and validate the evidence file, returning its rules."""
    path = Path(path)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise EvidenceError(f"{path}: no such evidence file") from None
    except json.JSONDecodeError as exc:
        raise EvidenceError(f"{path}: {exc}") from exc
    if not isinstance(data, dict) or data.get("version") != 1:
        raise EvidenceError(
            f"{path}: expected an object with 'version': 1 and 'rules'")
    rules = data.get("rules")
    if not isinstance(rules, list):
        raise EvidenceError(f"{path}: 'rules' must be a list")
    validate(rules)
    return rules


def validate(rules: list[dict]) -> None:
    """Reject a rule that could hide a defect rather than explain one."""
    seen: set[str] = set()
    for index, rule in enumerate(rules):
        where = f"rule {index}"
        if not isinstance(rule, dict):
            raise EvidenceError(f"{where}: must be an object")
        rule_id = rule.get("id")
        if not isinstance(rule_id, str) or not rule_id.strip():
            raise EvidenceError(f"{where}: missing id")
        if rule_id in seen:
            raise EvidenceError(f"{where}: duplicate id {rule_id!r}")
        seen.add(rule_id)
        where = f"rule {rule_id!r}"
        unknown = set(rule) - RULE_FIELDS
        if unknown:
            raise EvidenceError(
                f"{where}: unknown field(s) {', '.join(sorted(unknown))}")

        predicate = rule.get("predicate")
        if predicate not in PREDICATES:
            raise EvidenceError(f"{where}: unknown predicate {predicate!r}")
        for key in ("source", "note"):
            value = rule.get(key)
            if not isinstance(value, str) or not value.strip():
                raise EvidenceError(f"{where}: missing {key}")
        sample = rule.get("sample", DEFAULT_SAMPLE)
        if (not isinstance(sample, int) or isinstance(sample, bool)
                or sample < 1):
            raise EvidenceError(
                f"{where}: 'sample' must be a positive integer")

        classes = rule.get("classes")
        lemmas = rule.get("lemmas")
        if classes is not None and (
                not isinstance(classes, list) or not classes
                or not all(isinstance(c, str) and c for c in classes)):
            raise EvidenceError(f"{where}: 'classes' must be a non-empty list")
        if lemmas is not None and (
                not isinstance(lemmas, list) or not lemmas
                or not all(isinstance(w, str) and w for w in lemmas)):
            raise EvidenceError(f"{where}: 'lemmas' must be a non-empty list")
        exclude = rule.get("exclude_lemmas")
        if exclude is not None and (
                not isinstance(exclude, list) or not exclude
                or not all(isinstance(w, str) and w for w in exclude)):
            raise EvidenceError(
                f"{where}: 'exclude_lemmas' must be a non-empty list")
        if not classes and not lemmas and predicate != "strong-comparative":
            raise EvidenceError(f"{where}: needs 'classes' or 'lemmas'")

        applies = rule.get("applies_to", ["illegal"])
        if (not isinstance(applies, list) or not applies
                or not all(isinstance(s, str)
                           and s in ("illegal", "missing", "unsupported")
                           for s in applies)):
            raise EvidenceError(
                f"{where}: 'applies_to' must list 'illegal', 'missing' "
                "and/or 'unsupported'")

        cell = rule.get("cell")
        if cell is not None:
            if not isinstance(cell, dict) or not cell:
                raise EvidenceError(f"{where}: 'cell' must be a non-empty object")
            for key, value in cell.items():
                if key not in CELL_FIELDS:
                    raise EvidenceError(
                        f"{where}: unknown cell field {key!r}")
                if not isinstance(value, str) or not value:
                    raise EvidenceError(
                        f"{where}: cell {key!r} must be a non-empty string")

        if predicate == "lemma" and not lemmas:
            raise EvidenceError(
                f"{where}: predicate 'lemma' needs 'lemmas'")
        if predicate == "also-class":
            also = rule.get("also_classes")
            if (not isinstance(also, list) or not also
                    or not all(isinstance(c, str) and c for c in also)):
                raise EvidenceError(
                    f"{where}: predicate 'also-class' needs 'also_classes'")
        if predicate == "oblique-stem":
            if not classes or not cell or cell.get("case") in (None, "*"):
                raise EvidenceError(
                    f"{where}: predicate 'oblique-stem' needs classes and a "
                    "concrete cell case")
        if predicate == "fixed-prefix" and not classes:
            raise EvidenceError(
                f"{where}: predicate 'fixed-prefix' needs classes")
        if predicate == "case-less":
            if not classes or not cell or cell.get("case") in (None, "*"):
                raise EvidenceError(
                    f"{where}: predicate 'case-less' needs classes and a "
                    "concrete cell case")
        if predicate == "plural-only-t" and not classes:
            raise EvidenceError(
                f"{where}: predicate 'plural-only-t' needs classes")
        if predicate == "plural-only-iset" and not classes:
            raise EvidenceError(
                f"{where}: predicate 'plural-only-iset' needs classes")
        if predicate in ("shorter-than-expected", "shorter"):
            if not classes:
                raise EvidenceError(
                    f"{where}: predicate {predicate!r} needs classes")
            if cell is not None and all(value == "*" for value in cell.values()):
                raise EvidenceError(
                    f"{where}: a whole-class cell is too broad")


def _cell_matches(rule_cell: dict, cell) -> bool:
    for key, want in rule_cell.items():
        if want == "*":
            continue
        if getattr(cell, key, "") != want:
            return False
    return True


def _is_subsequence(short: str, long: str) -> bool:
    """Whether every character of `short` occurs in order in `long`."""
    rest = iter(long)
    return all(char in rest for char in short)


def _oblique_stem(lemma: str) -> str:
    """The class-34*C genitive stem, mirroring the Rust engine."""
    return lemma.replace("ton", "ttoma").replace("tön", "ttömä")


@functools.lru_cache(maxsize=None)
def _sanalista_words() -> tuple[set[str], set[str]]:
    """Every sanalista headword, and the nouns among them.

    The compound-prefix rule needs the same word boundary the engine uses, and
    loading the 104k-line CSV once is cheaper than opening the 2.4 GB oracle.
    """
    words: set[str] = set()
    nouns: set[str] = set()
    path = REPO / "data" / "nykysuomensanalista.csv"
    with open(path, encoding="utf-8") as fh:
        next(fh, None)
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if not parts or not parts[0]:
                continue
            words.add(parts[0])
            if len(parts) >= 3 and "substantiivi" in parts[2]:
                nouns.add(parts[0])
    return words, nouns


def _compound_prefix(word: str) -> str:
    """The longest modifier of a compound, or "" when there is no split.

    `kuivakakku` is `kuiva` + `kakku`; `omakuva` is `oma` + `kuva`. The engine
    keeps that modifier in the nominative and inflects the head, which is what
    the `fixed-prefix` rule accepts.
    """
    words, nouns = _sanalista_words()
    for i in range(len(word) - 1, 0, -1):
        if word[:i] in words and word[i:] in nouns:
            return word[:i]
    return ""


def _head_index(form: str, head: str) -> int:
    """Where the compound head starts in `form`, or -1 if it is not there.

    The head may be inflected (`kuva` -> `kuvi` before `-tta`), so try the whole
    head first and then its first three, two and one characters. The position is
    what matters: the engine keeps the modifier unchanged, so the head starts
    earlier than in the oracle's modifier-inflected spelling.
    """
    for size in (len(head), 3, 2, 1):
        if size <= len(head):
            index = form.find(head[:size])
            if index >= 0:
                return index
    return -1


def matches(rule: dict, finding, cls: str, class_of=None) -> bool:
    """Whether one rule explains this finding.

    `finding` is the harness's `Finding` (or any object with the same fields);
    `class_of` is a callable returning the sanalista class string for a lemma,
    used only by the `also-class` predicate.
    """
    if rule.get("classes") is not None and cls not in rule["classes"]:
        return False
    if rule.get("lemmas") is not None and finding.lemma not in rule["lemmas"]:
        return False
    if (rule.get("exclude_lemmas")
            and finding.lemma in rule["exclude_lemmas"]):
        return False
    # A rule says which finding statuses it explains. Most explain an `illegal`
    # row (the engine's form is wrong); a few explain a `missing` row (the
    # oracle's form is wrong). The default keeps the older rules as they were.
    if getattr(finding, "status", "illegal") not in rule.get(
            "applies_to", ["illegal"]):
        return False
    cell = rule.get("cell")
    if cell is not None and not _cell_matches(cell, finding.cell):
        return False

    predicate = rule["predicate"]
    if predicate == "lemma":
        return True
    if predicate == "also-class":
        if class_of is None:
            raise EvidenceError(
                f"rule {rule['id']!r}: 'also-class' needs a class resolver")
        classes = {
            part.strip() for part in class_of(finding.lemma).split(",")
            if part.strip()
        }
        return any(c in classes for c in rule["also_classes"])
    if predicate == "oblique-stem":
        return finding.form.startswith(_oblique_stem(finding.lemma))
    if predicate == "shorter-than-expected":
        expected = [e for e in finding.expected.split("/") if e]
        return any(
            len(finding.form) < len(want)
            and _is_subsequence(finding.form, want)
            for want in expected
        )
    if predicate == "shorter":
        # A case-less nominative reading belongs to the `case-less` rule.
        if (finding.cell.case == "NOM"
                and not any(tag in CASE_TAGS for tag in finding.tags)):
            return False
        expected = [e for e in finding.expected.split("/") if e]
        return any(len(finding.form) < len(e) for e in expected)
    if predicate == "fixed-prefix":
        # A case-less nominative reading belongs to the `case-less` rule; the
        # prefix rule is for the forms whose case is known.
        if (finding.cell.case == "NOM"
                and not any(tag in CASE_TAGS for tag in finding.tags)):
            return False
        prefix = _compound_prefix(finding.lemma)
        if not prefix:
            return False
        head = finding.lemma[len(prefix):]
        if not head or not finding.form.startswith(prefix):
            return False
        engine_at = _head_index(finding.form, head)
        if engine_at < 0:
            return False
        expected = [e for e in finding.expected.split("/") if e]
        if not expected:
            return False
        # A modifier-inflected spelling may not start with the lemma prefix at
        # all (`raskas` -> `raskaiden`, `raitis` -> `raittiiden`); otherwise the
        # head starts later than in the engine's fixed-prefix form (`kuiva` +
        # `kakku` against `kuiviin` + `kakku`).
        if any(not want.startswith(prefix) for want in expected):
            return True
        return all(
            (index := _head_index(want, head)) < 0 or index > engine_at
            for want in expected
        )
    if predicate == "case-less":
        return not any(tag in CASE_TAGS for tag in finding.tags)
    if predicate == "plural-only-t":
        # The class engine declines a plural-only lemma from its dropped-`t`
        # base (`aktiivat` -> `aktiiva`), while the oracle builds the whole
        # plural from the nominative plural (`aktiivatiden`). The lemma ending
        # in `-t` is what marks the shape; the rule's `classes` are the ones
        # where every `-t`-final member is plural-only.
        return finding.lemma.endswith("t")
    if predicate == "strong-comparative":
        # A comparative's `-mpi` takes the weak `-mm-` grade in the
        # open-syllable cases (`kuurommalta`, `alamaisemmalta`), and the
        # oracle sometimes keeps the strong `-mpa-` there (`kuurompalta`,
        # `alamaisempalta`). The strong grade is correct only in the
        # nominative, partitive, illative and essive, which are not in the
        # weak set.
        if finding.cell.case not in WEAK_COMPARATIVE_CASES:
            return False
        return any(
            "mpa" in expected or "mpä" in expected
            for expected in finding.expected.split("/")
        )
    if predicate == "plural-only-iset":
        # A class-38 `-iset` lemma is the nominative plural of an `-inen`
        # singular (`avajaiset` -> `avajainen`), and Wiktionary marks it
        # plural-only. The engine keeps only the plural cells, so a singular
        # cell the oracle still claims is the oracle's, not a recall gap.
        return (
            finding.cell.number == "SG"
            and finding.lemma.endswith("set")
        )
    return False


def match(rules: list[dict], finding, cls: str, class_of=None) -> list[dict]:
    """Every rule that explains `finding`. The gate requires exactly one."""
    return [rule for rule in rules
            if matches(rule, finding, cls, class_of)]
