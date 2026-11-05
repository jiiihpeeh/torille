#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Fetch Finnish place and personal names from Wikidata.

Wikidata is CC0 and has both the items and their Finnish labels. The query set
is deliberately narrow: names a Finnish spell-checker is likely to meet in
running text, not the whole of Wikidata.

  * places  -- municipalities of Finland (Q856076), plus cities (Q515),
               towns (Q3957), human settlements (Q486972), villages (Q532)
               and rivers (Q4022) whose country (P17) is Finland (Q33);
  * given names  -- P735 of people whose citizenship (P27) is Finland;
  * family names -- P734 of the same people.

The output is `data/proper_names.csv`, one `name<TAB>kind` row per unique
name. Labels are cleaned: only letters (Finnish ä/ö/å included), no spaces or
punctuation, at least 2 characters, first letter upper-case. This excludes
multi-word names (the tokeniser would split them anyway) and odd labels.

Usage:
    python3 tools/build/build_proper_names.py
    python3 tools/build/build_proper_names.py --out /tmp/names.csv --dry-run
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import unicodedata
import urllib.parse
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "data" / "proper_names.csv"

#: QLever's Wikidata index: the same graph as WDQS, but the name joins that
#: time out on `query.wikidata.org` answer in under a second here.
SPARQL = "https://qlever.dev/api/wikidata"
USER_AGENT = "torille-dev/0.1 (Finnish spell-checker data build)"
PREFIXES = """PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
"""

#: Country QIDs for the "usual names" slices (Europe plus Turkey). The top
#: names by Wikidata usage among that country's citizens is a better proxy
#: than a language property (P407 coverage is sparse). The third field is the
#: label language: the local one for most countries, English for the Russian
#: and Ukrainian transliterations (Finnish text does not use Cyrillic for
#: names).
COUNTRY_SLICES = [
    ("Q29", "Spanish", "es"),
    ("Q159", "Russian", "en"),
    ("Q212", "Ukrainian", "en"),
    ("Q36", "Polish", "pl"),
    ("Q183", "German", "de"),
    ("Q34", "Swedish", "sv"),
    ("Q191", "Estonian", "et"),
    ("Q211", "Latvian", "lv"),
    ("Q37", "Lithuanian", "lt"),
    ("Q142", "French", "fr"),
    ("Q45", "Portuguese", "pt"),
    ("Q38", "Italian", "it"),
    ("Q28", "Hungarian", "hu"),
    ("Q214", "Slovak", "sk"),
    ("Q43", "Turkish", "tr"),
    ("Q218", "Romanian", "ro"),
    ("Q35", "Danish", "da"),
    ("Q20", "Norwegian", "nb"),
    ("Q189", "Icelandic", "is"),
    ("Q17", "Japanese", "en"),
    ("Q148", "Chinese", "en"),
    ("Q55", "Dutch", "nl"),
    ("Q213", "Czech", "cs"),
    ("Q41", "Greek", "en"),
    ("Q224", "Croatian", "hr"),
    ("Q403", "Serbian", "en"),
    ("Q219", "Bulgarian", "en"),
    ("Q79", "Arabic (Egypt)", "en"),
    ("Q851", "Arabic (Saudi)", "en"),
    ("Q668", "Hindi", "en"),
    ("Q881", "Vietnamese", "en"),
    ("Q884", "Korean", "en"),
    ("Q801", "Hebrew", "en"),
    ("Q928", "Filipino", "en"),
    ("Q252", "Indonesian", "id"),
]

#: Names that Wikidata's Sámi coverage misses. Common Northern Sámi given
#: names and surnames; the sources are the Sámi name traditions (the
#: Norwegian/Swedish/Finnish Sámi name books), not a single open dataset.
SAMI_NAMES = [
    ("Áilu", "given"),
    ("Áile", "given"),
    ("Ánne", "given"),
    ("Biret", "given"),
    ("Elle", "given"),
    ("Iŋgá", "given"),
    ("Issát", "given"),
    ("Lávra", "given"),
    ("Márjá", "given"),
    ("Máret", "given"),
    ("Niillas", "given"),
    ("Ovllá", "given"),
    ("Piera", "given"),
    ("Risten", "given"),
    ("Sárá", "given"),
    ("Aslak", "given"),
    ("Ailu", "given"),
    ("Aikio", "family"),
    ("Ailu", "family"),
    ("Eira", "family"),
    ("Guttorm", "family"),
    ("Hætta", "family"),
    ("Kemi", "family"),
    ("Kitti", "family"),
    ("Länsman", "family"),
    ("Magga", "family"),
    ("Näkkäläjärvi", "family"),
    ("Pokka", "family"),
    ("Porsanger", "family"),
    ("Sara", "family"),
    ("Somby", "family"),
    ("Utsi", "family"),
    ("Valle", "family"),
    ("Varsi", "family"),
]

#: Common Japanese names in ASCII romanisation. Wikidata's top names are a
#: mix of macron spellings (`Itō`, `Katō`) and older given names; the ASCII
#: variants are added by `fold`, the modern given names here.
JAPANESE_NAMES = [
    ("Haruto", "given"),
    ("Yuto", "given"),
    ("Sota", "given"),
    ("Kaito", "given"),
    ("Riku", "given"),
    ("Hiroto", "given"),
    ("Mei", "given"),
    ("Rin", "given"),
    ("Akari", "given"),
    ("Emi", "given"),
    ("Akira", "given"),
    ("Kenji", "given"),
    ("Hiroshi", "given"),
    ("Takashi", "given"),
    ("Yoko", "given"),
    ("Naomi", "given"),
    ("Hina", "given"),
    ("Sakura", "given"),
]

#: Common Korean given names, usually written hyphenated in Wikidata
#: (`Min-jun`); the unhyphenated forms are what the tokeniser sees.
KOREAN_NAMES = [
    ("Minjun", "given"),
    ("Seoyeon", "given"),
    ("Jiwoo", "given"),
    ("Jihun", "given"),
    ("Jimin", "given"),
    ("Hyun", "given"),
    ("Jiho", "given"),
    ("Minho", "given"),
    ("Yuna", "given"),
    ("Soojin", "given"),
    ("Hyunwoo", "given"),
    ("Donghyun", "given"),
    ("Jiyeon", "given"),
]

#: Common Indonesian names. Wikidata's P735 coverage for Indonesia is thin
#: for the everyday names (`Budi`, `Wati`, `Fitri`).
INDONESIAN_NAMES = [
    ("Budi", "given"),
    ("Wati", "given"),
    ("Fitri", "given"),
    ("Agus", "given"),
    ("Siti", "given"),
    ("Dewi", "given"),
    ("Rina", "given"),
    ("Andi", "given"),
    ("Dwi", "given"),
    ("Eko", "given"),
    ("Putri", "given"),
    ("Ayu", "given"),
    ("Santoso", "family"),
    ("Wijaya", "family"),
    ("Saputra", "family"),
    ("Hidayat", "family"),
    ("Nugroho", "family"),
    ("Setiawan", "family"),
    ("Kusuma", "family"),
    ("Pratama", "family"),
    ("Siregar", "family"),
    ("Lubis", "family"),
    ("Wibowo", "family"),
]

#: One-off names reported by users, kept separate from the sourced slices.
USER_NAMES = [
    ("Rozelyn", "given"),
    ("Lavoisier", "family"),
]

#: Classical Greek/Roman philosophers and scientists. Most are mononyms, and
#: Wikidata does not consistently link them as given or family names, so the
#: sitelink slice misses them.
CLASSICAL_NAMES = [
    ("Socrates", "given"),
    ("Plato", "given"),
    ("Aristotle", "given"),
    ("Pythagoras", "given"),
    ("Heraclitus", "given"),
    ("Democritus", "given"),
    ("Epicurus", "given"),
    ("Zeno", "given"),
    ("Thales", "given"),
    ("Parmenides", "given"),
    ("Plotinus", "given"),
    ("Diogenes", "given"),
    ("Anaximander", "given"),
    ("Anaximenes", "given"),
    ("Empedocles", "given"),
    ("Protagoras", "given"),
    ("Gorgias", "given"),
    ("Xenophanes", "given"),
    ("Sophocles", "given"),
    ("Euripides", "given"),
    ("Aristophanes", "given"),
    ("Herodotus", "given"),
    ("Thucydides", "given"),
    ("Homer", "given"),
    ("Hesiod", "given"),
    ("Archimedes", "given"),
    ("Euclid", "given"),
    ("Ptolemy", "given"),
    ("Seneca", "family"),
    ("Cicero", "family"),
    ("Epictetus", "given"),
    ("Lucretius", "given"),
    ("Aurelius", "family"),
]

#: The Finnish spellings of the classical names. Omorfi knows their case
#: paradigms (`Sokrates` -> `Sokrateen`, `Platon` -> `Platonin`), so the form
#: generator covers the inflected text too.
FINNISH_CLASSICAL_NAMES = [
    ("Sokrates", "given"),
    ("Platon", "given"),
    ("Aristoteles", "given"),
    ("Pythagoras", "given"),
    ("Herakleitos", "given"),
    ("Demokritos", "given"),
    ("Epikuros", "given"),
    ("Zenon", "given"),
    ("Thales", "given"),
    ("Parmenides", "given"),
    ("Plotinos", "given"),
    ("Diogenes", "given"),
    ("Anaksimandros", "given"),
    ("Anaksimenes", "given"),
    ("Empedokles", "given"),
    ("Protagoras", "given"),
    ("Gorgias", "given"),
    ("Ksenofanes", "given"),
    ("Sofokles", "given"),
    ("Euripides", "given"),
    ("Aristofanes", "given"),
    ("Herodotos", "given"),
    ("Thukydides", "given"),
    ("Homeros", "given"),
    ("Hesiodos", "given"),
    ("Arkhimedes", "given"),
    ("Eukleides", "given"),
    ("Ptolemaios", "given"),
    ("Epiktetos", "given"),
]

#: Finnish transliterations of high-profile names whose English spelling
#: differs (`Jeltsin`, not `Yeltsin`; `Hruštšov`, not `Khrushchev`). The
#: Finnish label is preferred in the queries; this is the safety net.
FINNISH_TRANSLITERATIONS = [
    ("Jeltsin", "family"),
    ("Hruštšov", "family"),
    ("Gorbatšov", "family"),
    ("Brežnev", "family"),
    ("Andropov", "family"),
    ("Tšernenko", "family"),
    ("Trotski", "family"),
    ("Navalnyi", "family"),
    ("Lavrov", "family"),
    ("Gromyko", "family"),
    ("Ševardnadze", "family"),
    ("Janukovitš", "family"),
    ("Juštšenko", "family"),
    ("Tymošenko", "family"),
    ("Boris", "given"),
    ("Mihail", "given"),
    ("Nikita", "given"),
    ("Leonid", "given"),
    ("Juri", "given"),
    ("Konstantin", "given"),
    ("Aleksei", "given"),
    ("Lev", "given"),
    ("Josif", "given"),
    ("Friedrich", "given"),
]

#: Finnish exonyms for cities that the top-10-by-population query misses
#: (small cities, or items labelled only in the native language).
CITY_ALIASES = [
    ("Tromssa", "place"),   # Tromsø
    ("Kioto", "place"),     # Kyoto
    ("Viipuri", "place"),   # Vyborg
    ("Varsova", "place"),   # Warszawa
    ("Praha", "place"),     # Praha/Prague
    ("Wien", "place"),      # Vienna
    ("Bryssel", "place"),   # Brussels
    ("Haag", "place"),      # Den Haag
    ("Pietari", "place"),   # St Petersburg
    ("Odessa", "place"),    # Odesa
    ("Kiova", "place"),     # Kyiv
    ("Jerevan", "place"),   # Yerevan
    ("Tbilisi", "place"),   # Tbilisi
]

#: Continents under their Finnish names. The class query supplies most of
#: them; these are the ones with no `P31 wd:Q5107` row.
CONTINENT_ALIASES = [
    ("Eurooppa", "place"),
    ("Aasia", "place"),
    ("Afrikka", "place"),
    ("Amerikka", "place"),
    ("Oseania", "place"),
    ("Antarktis", "place"),
    ("Pohjois-Amerikka", "place"),
    ("Etelä-Amerikka", "place"),
]

#: Components of multi-word state/province names (`New York`, `Nova Scotia`,
#: `Rhode Island`, `British Columbia`). The tokeniser splits those at the
#: space, so the whole name cannot be one key; the parts have to be names too.
PLACE_COMPONENTS = [
    ("New", "place"),
    ("Hampshire", "place"),
    ("Rhode", "place"),
    ("Island", "place"),
    ("Scotia", "place"),
    ("Brunswick", "place"),
    ("Labrador", "place"),
]

#: Finnish historical provinces and the current regions whose names predate
#: or overlap the `maakunta` division (`Satakunta`, `Uusimaa`, `Ahvenanmaa`).
#: The Q193512 query covers the current regions; this keeps the historical
#: names even when Wikidata's class has changed.
FINNISH_REGIONS = [
    ("Ahvenanmaa", "place"),
    ("Varsinais-Suomi", "place"),
    ("Satakunta", "place"),
    ("Kanta-Häme", "place"),
    ("Pirkanmaa", "place"),
    ("Päijät-Häme", "place"),
    ("Kymenlaakso", "place"),
    ("Etelä-Karjala", "place"),
    ("Etelä-Savo", "place"),
    ("Pohjois-Savo", "place"),
    ("Pohjois-Karjala", "place"),
    ("Keski-Suomi", "place"),
    ("Etelä-Pohjanmaa", "place"),
    ("Pohjanmaa", "place"),
    ("Keski-Pohjanmaa", "place"),
    ("Pohjois-Pohjanmaa", "place"),
    ("Kainuu", "place"),
    ("Lappi", "place"),
    ("Uusimaa", "place"),
    ("Itä-Uusimaa", "place"),
    ("Häme", "place"),
    ("Savo", "place"),
    ("Karjala", "place"),
]

#: Notable geographic features whose Finnish label is missing from the
#: class query, or whose class is administrative rather than physical.
GEO_ALIASES = [
    ("Välimeri", "place"),
    ("Itämeri", "place"),
    ("Pohjanmeri", "place"),
    ("Mustameri", "place"),
    ("Punainenmeri", "place"),
    ("Persianlahti", "place"),
    ("Meksikonlahti", "place"),
    ("Atlantti", "place"),
    ("Siperia", "place"),
    ("Gobi", "place"),
    ("Kalahari", "place"),
    ("Atacama", "place"),
    ("Mississippi", "place"),
    ("Jangtse", "place"),
    ("Ganges", "place"),
    ("Tonava", "place"),
    ("Volga", "place"),
    ("Eufrat", "place"),
    ("Tigris", "place"),
    ("Indus", "place"),
    ("Mekong", "place"),
    ("Kongo", "place"),
    ("Zambezi", "place"),
    ("Kilimanjaro", "place"),
    ("Karpaatit", "place"),
    ("Amazonia", "place"),
    ("Sahel", "place"),
    ("Kaukasia", "place"),
    ("Skandinavia", "place"),
    ("Balkan", "place"),
    ("Iberia", "place"),
    ("Anatolia", "place"),
    ("Patagonia", "place"),
    ("Tanganjikajärvi", "place"),
    ("Victoriajärvi", "place"),
    ("Baikal", "place"),
    ("Superiorjärvi", "place"),
    ("Uusi-Guinea", "place"),
    ("Java", "place"),
    ("Kuba", "place"),
    ("Britannia", "place"),
    ("Niagara", "place"),
]

#: Finnish names of historical, de facto and dependent states. The country
#: class query covers most of them; these are the gaps and the hyphenated
#: names (`Etelä-Ossetia`, `Itävalta-Unkari`).
COUNTRY_ALIASES = [
    ("Transnistria", "place"),
    ("Abhasia", "place"),
    ("Etelä-Ossetia", "place"),
    ("Pohjois-Kypros", "place"),
    ("Somalimaa", "place"),
    ("Länsi-Sahara", "place"),
    ("Taiwan", "place"),
    ("Kosovo", "place"),
    ("Palestiina", "place"),
    ("Hongkong", "place"),
    ("Macao", "place"),
    ("Grönlanti", "place"),
    ("Färsaaret", "place"),
    ("Vatikaani", "place"),
    ("Neuvostoliitto", "place"),
    ("Jugoslavia", "place"),
    ("Tšekkoslovakia", "place"),
    ("Preussi", "place"),
    ("Itävalta-Unkari", "place"),
    ("Itä-Saksa", "place"),
    ("Länsi-Saksa", "place"),
    ("Bysantti", "place"),
    ("Persia", "place"),
    ("Babylonia", "place"),
    ("Assyria", "place"),
    ("Sumer", "place"),
    ("Akkad", "place"),
    ("Makedonia", "place"),
    ("Sparta", "place"),
    ("Kartago", "place"),
    ("Troija", "place"),
    ("Mykene", "place"),
]

#: Closed-class words that are never names. Some name items and folds collide
#: with them (`The` from a folded `Thế`, `In`, `Or`, `With`, ...), and the
#: checker must keep flagging lowercase English text.
NOT_NAMES = {
    "the", "this", "that", "these", "those", "and", "or", "but", "if", "in",
    "on", "at", "to", "of", "for", "with", "from", "by", "is", "are", "was",
    "were", "be", "been", "has", "have", "had", "do", "does", "did", "not",
    "than", "then", "there", "when", "where", "who", "what", "which", "how",
    "why",
}

#: Base sets: Finnish places and names, plus foreign places under their
#: Finnish exonyms. The common-name and per-country slices are appended in
#: `main` because their LIMIT is configurable.
QUERIES: list[tuple[str, str]] = [
    (
        """SELECT DISTINCT ?name WHERE {
             ?x wdt:P31 wd:Q856076 .
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
    *[
        (
            f"""SELECT DISTINCT ?name WHERE {{
                  ?x wdt:P31 wd:{qid} ; wdt:P17 wd:Q33 .
                  ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
                }}""",
            "place",
        )
        for qid in ("Q515", "Q3957", "Q486972", "Q532", "Q4022")
    ],
    # Foreign places, under their Finnish exonyms: countries, capitals of
    # sovereign states, and cities over 100k. Labels like `Lontoo`, `Pariisi`
    # and `Berliini` are what Finnish text actually uses.
    (
        """SELECT DISTINCT ?name WHERE {
             VALUES ?class { wd:Q6256 wd:Q3624078 wd:Q3024240 wd:Q21512251
                             wd:Q15239622 wd:Q161243 wd:Q1763527 }
             ?x wdt:P31/wdt:P279* ?class .
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
    (
        """SELECT DISTINCT ?name WHERE {
             ?x wdt:P1376 ?country . ?country wdt:P31 wd:Q6256 .
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
    (
        """SELECT DISTINCT ?name WHERE {
             ?x wdt:P31 wd:Q515 ; wdt:P1082 ?pop .
             FILTER(?pop > 100000)
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
    (
        """SELECT DISTINCT ?name WHERE {
             ?person wdt:P27 wd:Q33 ; wdt:P735 ?x .
             OPTIONAL { ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }
             OPTIONAL { ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }
             OPTIONAL { ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }
             BIND(COALESCE(?fi, ?en, ?mul) AS ?name)
             FILTER(BOUND(?name))
           }""",
        "given",
    ),
    (
        """SELECT DISTINCT ?name WHERE {
             ?person wdt:P27 wd:Q33 ; wdt:P734 ?x .
             OPTIONAL { ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }
             OPTIONAL { ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }
             OPTIONAL { ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }
             BIND(COALESCE(?fi, ?en, ?mul) AS ?name)
             FILTER(BOUND(?name))
           }""",
        "family",
    ),
    # Sámi place names: Finnish municipalities, villages, rivers and lakes
    # carry se/smn/sms labels. Some are the Finnish name repeated, many are
    # the genuine Sámi name (Aanaar, Ohcejohka, Anár, Deatnu).
    (
        """SELECT DISTINCT ?name WHERE {
             VALUES ?class { wd:Q856076 wd:Q532 wd:Q4022 wd:Q23397 }
             ?x wdt:P17 wd:Q33 ; wdt:P31 ?class .
             ?x rdfs:label ?name . FILTER(LANG(?name) IN ("se", "smn", "sms"))
           }""",
        "place",
    ),
    # Sámi personal names: people with the Sámi ethnic group. The long tail
    # of common names is in `SAMI_NAMES`; Wikidata's P172 coverage is small.
    (
        """SELECT DISTINCT ?name WHERE {
             ?person wdt:P172 wd:Q48199 ; wdt:P735 ?x .
             OPTIONAL { ?x rdfs:label ?se . FILTER(LANG(?se) = "se") }
             OPTIONAL { ?x rdfs:label ?smn . FILTER(LANG(?smn) = "smn") }
             OPTIONAL { ?x rdfs:label ?sms . FILTER(LANG(?sms) = "sms") }
             OPTIONAL { ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }
             OPTIONAL { ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }
             OPTIONAL { ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }
             BIND(COALESCE(?se, ?smn, ?sms, ?fi, ?en, ?mul) AS ?name)
             FILTER(BOUND(?name))
           }""",
        "given",
    ),
    (
        """SELECT DISTINCT ?name WHERE {
             ?person wdt:P172 wd:Q48199 ; wdt:P734 ?x .
             OPTIONAL { ?x rdfs:label ?se . FILTER(LANG(?se) = "se") }
             OPTIONAL { ?x rdfs:label ?smn . FILTER(LANG(?smn) = "smn") }
             OPTIONAL { ?x rdfs:label ?sms . FILTER(LANG(?sms) = "sms") }
             OPTIONAL { ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }
             OPTIONAL { ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }
             OPTIONAL { ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }
             BIND(COALESCE(?se, ?smn, ?sms, ?fi, ?en, ?mul) AS ?name)
             FILTER(BOUND(?name))
           }""",
        "family",
    ),
    # Notable people: high sitelink counts cover historical figures and
    # well-known names that are not the most *common* names of a country
    # (Newton, Lavoisier, Marx, Stalin, Hitler, Naomi Klein, ...).
    (
        """SELECT DISTINCT ?name WHERE {
             ?person wdt:P31 wd:Q5 ; wikibase:sitelinks ?sl .
             FILTER(xsd:integer(?sl) > 50)
             ?person wdt:P735 ?x .
             OPTIONAL { ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }
             OPTIONAL { ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }
             OPTIONAL { ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }
             BIND(COALESCE(?fi, ?en, ?mul) AS ?name)
             FILTER(BOUND(?name))
           }""",
        "given",
    ),
    (
        """SELECT DISTINCT ?name WHERE {
             ?person wdt:P31 wd:Q5 ; wikibase:sitelinks ?sl .
             FILTER(xsd:integer(?sl) > 50)
             ?person wdt:P734 ?x .
             OPTIONAL { ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }
             OPTIONAL { ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }
             OPTIONAL { ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }
             BIND(COALESCE(?fi, ?en, ?mul) AS ?name)
             FILTER(BOUND(?name))
           }""",
        "family",
    ),
    # Capitals of current, historical and de facto states. The country filter
    # is the country-like classes; without it, `P1376` also matches every
    # municipality and district seat. Some capitals have no `fi` label, so
    # fall back to en/mul.
    (
        """SELECT DISTINCT ?name WHERE {
             ?city wdt:P1376 ?country .
             VALUES ?class { wd:Q6256 wd:Q3624078 wd:Q3024240 wd:Q21512251
                             wd:Q15239622 wd:Q161243 wd:Q1763527 }
             ?country wdt:P31/wdt:P279* ?class .
             OPTIONAL { ?city rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }
             OPTIONAL { ?city rdfs:label ?en . FILTER(LANG(?en) = "en") }
             OPTIONAL { ?city rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }
             BIND(COALESCE(?fi, ?en, ?mul) AS ?name)
             FILTER(BOUND(?name))
           }""",
        "place",
    ),
    # Continents under their Finnish names. They are not countries, so the
    # country query does not reach them.
    (
        """SELECT DISTINCT ?name WHERE {
             ?x wdt:P31 wd:Q5107 .
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
    # Notable geographic features under their Finnish names (Sahara, Amazon,
    # Andit, Alpit, Niili, Tyynimeri, ...). Direct classes only: the subclass
    # path also reaches arrondissements and districts.
    (
        """SELECT DISTINCT ?name WHERE {
             VALUES ?class { wd:Q8514 wd:Q4022 wd:Q8502 wd:Q46831 wd:Q165
                             wd:Q9430 wd:Q23397 wd:Q23442 wd:Q4421 wd:Q34038
                             wd:Q15003 }
             ?x wdt:P31 ?class .
             ?x wikibase:sitelinks ?sl . FILTER(xsd:integer(?sl) > 50)
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
    # First-level administrative divisions under their Finnish names: US
    # states, Canadian provinces, German Länder and the equivalent elsewhere
    # (Alberta, Missouri, Baijeri, ...).
    (
        """SELECT DISTINCT ?name WHERE {
             ?x wdt:P31/wdt:P279* wd:Q10864048 .
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
    # Finnish regions (`maakunnat`). They are first-level divisions but are
    # not under the generic Q10864048 class, so `Ahvenanmaa`, `Uusimaa` and
    # `Pirkanmaa` were missing from the name table.
    (
        """SELECT DISTINCT ?name WHERE {
             ?x wdt:P31 wd:Q193512 .
             ?x rdfs:label ?name . FILTER(LANG(?name) = "fi")
           }""",
        "place",
    ),
]

#: All cities with a Finnish label, for the per-country top-N slice. Run once
#: and group in Python: a per-country query would be ~200 round trips.
CITY_QUERY = """SELECT ?country ?name ?pop WHERE {
  VALUES ?root { wd:Q515 wd:Q5119 wd:Q1549591 wd:Q3957 wd:Q486972 }
  ?city wdt:P31/wdt:P279* ?root ; wdt:P17 ?country ; wdt:P1082 ?pop .
  ?city rdfs:label ?name . FILTER(LANG(?name) = "fi")
}"""


def query_payload(q: str) -> dict:
    """The parsed SPARQL JSON for one query, retrying transient errors."""
    body = urllib.parse.urlencode({"query": PREFIXES + q}).encode()
    req = urllib.request.Request(
        SPARQL,
        data=body,
        headers={
            "Accept": "application/sparql-results+json",
            "User-Agent": USER_AGENT,
        },
    )
    last: Exception | None = None
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=180) as resp:
                raw = resp.read().decode("utf-8")
            # Wikidata labels occasionally contain a control code;
            # `strict=False` lets the JSON reader accept it instead of losing
            # the whole response.
            return json.loads(raw, strict=False)
        except Exception as e:  # noqa: BLE001 - the endpoint is flaky
            last = e
            time.sleep(2 * (attempt + 1))
    raise last  # type: ignore[misc]


def query(q: str) -> list[str]:
    """The `name` values of one SPARQL query."""
    payload = query_payload(q)
    out = []
    for binding in payload["results"]["bindings"]:
        label = binding.get("name", {}).get("value", "").strip()
        if label:
            out.append(label)
    return out


def query_rows(q: str) -> list[dict]:
    """The raw bindings of one SPARQL query."""
    return query_payload(q)["results"]["bindings"]


def is_latin_name(name: str) -> bool:
    """True when every letter is Latin script (or a modifier letter).

    Wikidata's English label for a name is occasionally still in the native
    script (`Владимир`, `Δαμιανός`); the checker wants the transliteration.
    Modifier letters are allowed because Sámi orthography uses `ʹ`.
    """
    for c in name:
        if not c.isalpha():
            continue
        uname = unicodedata.name(c, "")
        if not (uname.startswith("LATIN") or uname.startswith("MODIFIER LETTER")):
            return False
    return True


def clean(label: str) -> str | None:
    """A usable single-token name, or None.

    Names are stored capitalised. Only Latin letters and hyphens are allowed:
    the checker's Finnish/transliterated name set must not carry Cyrillic,
    Greek, Han, Hangul or Arabic script. Wikidata labels for places can also
    carry a disambiguator (`Foo (kunta)`), a number, or an apostrophe, and
    none of those are a name the tokeniser would present whole.

    Hyphens are **kept**: `Antti-Jussi`, `Anna-Mari` and `Min-jun` are single
    tokens (the tokeniser keeps one internal hyphen), and dropping the hyphen
    would make the stored name miss the token. Apostrophes are dropped
    (`A'isha` -> `Aisha`, `Nuku'alofa` -> `Nukualofa`).
    """
    name = label.strip().replace("'", "").replace("\u2019", "")
    if len(name) < 2 or not name[0].isupper():
        return None
    if name.startswith("-") or name.endswith("-") or "--" in name:
        return None
    if not all(c.isalpha() or c == "-" for c in name):
        return None
    if not is_latin_name(name):
        return None
    return name


#: Characters NFKD does not decompose but which have a conventional ASCII
#: spelling (`ł`, `ø`, `ß`, ...). The Sámi modifier apostrophe is dropped.
FOLD = str.maketrans({
    "ł": "l", "Ł": "L", "ø": "o", "Ø": "O", "đ": "d", "Đ": "D",
    "ð": "d", "Ð": "D", "þ": "th", "Þ": "Th", "ß": "ss",
    "æ": "ae", "Æ": "Ae", "œ": "oe", "Œ": "Oe", "ı": "i",
    "ŋ": "n", "Ŋ": "N", "ʹ": "", "ʼ": "",
})


def fold(name: str) -> str:
    """An ASCII spelling of `name` (`Itō` -> `Ito`, `Müller` -> `Muller`)."""
    decomposed = unicodedata.normalize("NFKD", name)
    stripped = "".join(c for c in decomposed if not unicodedata.combining(c))
    return stripped.translate(FOLD)


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument(
        "--common-names",
        type=int,
        default=1000,
        help="also take the top N given and top N family names by Wikidata "
             "usage (0 disables)",
    )
    ap.add_argument(
        "--european-names",
        type=int,
        default=300,
        help="top N given/family names per European country (Spain, Russia, "
             "Poland, Germany, Sweden, Estonia; 0 disables)",
    )
    ap.add_argument(
        "--cities-per-country",
        type=int,
        default=10,
        help="top N cities per country that have a Finnish label (0 disables)",
    )
    args = ap.parse_args()

    queries = list(QUERIES)
    if args.european_names > 0:
        for qid, lang, local in COUNTRY_SLICES:
            for prop, kind in (("P735", "given"), ("P734", "family")):
                queries.append(
                    (
                        f"""SELECT ?name (COUNT(?person) AS ?n) WHERE {{
                              ?person wdt:P27 wd:{qid} ; wdt:{prop} ?x .
                              OPTIONAL {{ ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }}
                              OPTIONAL {{ ?x rdfs:label ?local . FILTER(LANG(?local) = "{local}") }}
                              OPTIONAL {{ ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }}
                              OPTIONAL {{ ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }}
                              BIND(COALESCE(?fi, ?local, ?en, ?mul) AS ?name)
                              FILTER(BOUND(?name))
                            }} GROUP BY ?x ?name ORDER BY DESC(?n)
                            LIMIT {args.european_names}""",
                        kind,
                    )
                )
                print(f"queued {lang} {kind}", file=sys.stderr)
    if args.common_names > 0:
        # "Common" = used by the most people on Wikidata. This adds the
        # international names Finnish text meets (William, Maria, Smith,
        # ...), not the long tail. Case-sensitive acceptance is what makes
        # the collisions (`Mark`, `Bill`) safe.
        queries += [
            (
                f"""SELECT ?name (COUNT(?person) AS ?n) WHERE {{
                      ?person wdt:P735 ?x .
                      OPTIONAL {{ ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }}
                      OPTIONAL {{ ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }}
                      OPTIONAL {{ ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }}
                      BIND(COALESCE(?fi, ?en, ?mul) AS ?name)
                      FILTER(BOUND(?name))
                    }} GROUP BY ?x ?name ORDER BY DESC(?n)
                    LIMIT {args.common_names}""",
                "given",
            ),
            (
                f"""SELECT ?name (COUNT(?person) AS ?n) WHERE {{
                      ?person wdt:P734 ?x .
                      OPTIONAL {{ ?x rdfs:label ?fi . FILTER(LANG(?fi) = "fi") }}
                      OPTIONAL {{ ?x rdfs:label ?en . FILTER(LANG(?en) = "en") }}
                      OPTIONAL {{ ?x rdfs:label ?mul . FILTER(LANG(?mul) = "mul") }}
                      BIND(COALESCE(?fi, ?en, ?mul) AS ?name)
                      FILTER(BOUND(?name))
                    }} GROUP BY ?x ?name ORDER BY DESC(?n)
                    LIMIT {args.common_names}""",
                "family",
            ),
        ]

    seen: dict[str, str] = {}
    for query_text, kind in queries:
        try:
            labels = query(query_text)
        except Exception as e:  # noqa: BLE001 - report and continue
            print(f"{kind}: query failed: {e}", file=sys.stderr)
            continue
        before = len(seen)
        for label in labels:
            name = clean(label)
            if name is not None:
                # First kind wins; a surname that is also a place is rare
                # and either reading accepts the word.
                seen.setdefault(name, kind)
        print(f"{kind}: {len(labels)} labels, +{len(seen) - before} names",
              file=sys.stderr)

    if args.cities_per_country > 0:
        # All cities with a Finnish label, across all current sovereign
        # states. One query returns ~314 k rows (multiple population/P31
        # values per city); keep the highest population per (country, name)
        # and then the top N per country.
        rows = query_rows(CITY_QUERY)
        best: dict[tuple[str, str], float] = {}
        for binding in rows:
            country = binding["country"]["value"].rsplit("/", 1)[-1]
            name = binding["name"]["value"].strip()
            try:
                pop = float(binding["pop"]["value"])
            except (KeyError, ValueError):
                continue
            key = (country, name)
            if pop > best.get(key, -1.0):
                best[key] = pop
        by_country: dict[str, list[tuple[float, str]]] = {}
        for (country, name), pop in best.items():
            by_country.setdefault(country, []).append((pop, name))
        before = len(seen)
        for items in by_country.values():
            for _pop, name in sorted(items, reverse=True)[
                : args.cities_per_country
            ]:
                cleaned = clean(name)
                if cleaned is not None:
                    seen.setdefault(cleaned, "place")
        print(
            f"cities: {len(best)} Finnish-labelled across "
            f"{len(by_country)} countries, +{len(seen) - before} new",
            file=sys.stderr,
        )

    for name, kind in SAMI_NAMES:
        seen.setdefault(name, kind)

    # ASCII variants: `Itō` -> `Ito`, `Müller` -> `Muller`, `Wiśniewski` ->
    # `Wisniewski`. Users typing a foreign name without diacritics should not
    # be flagged. Case-sensitive acceptance is unchanged.
    for name, kind in list(seen.items()):
        folded = fold(name)
        if (
            folded != name
            and len(folded) >= 2
            and folded[0].isupper()
            and all(c.isalpha() for c in folded)
        ):
            seen.setdefault(folded, kind)

    for name, kind in JAPANESE_NAMES:
        seen.setdefault(name, kind)

    for name, kind in KOREAN_NAMES:
        seen.setdefault(name, kind)

    for name, kind in INDONESIAN_NAMES:
        seen.setdefault(name, kind)

    for name, kind in USER_NAMES:
        seen.setdefault(name, kind)

    for name, kind in CLASSICAL_NAMES:
        seen.setdefault(name, kind)

    for name, kind in FINNISH_CLASSICAL_NAMES:
        seen.setdefault(name, kind)

    for name, kind in FINNISH_TRANSLITERATIONS:
        seen.setdefault(name, kind)

    for name, kind in CITY_ALIASES:
        seen.setdefault(name, kind)

    for name, kind in GEO_ALIASES:
        seen.setdefault(name, kind)

    for name, kind in COUNTRY_ALIASES:
        seen.setdefault(name, kind)

    for name, kind in CONTINENT_ALIASES:
        seen.setdefault(name, kind)

    for name, kind in PLACE_COMPONENTS:
        seen.setdefault(name, kind)

    for name, kind in FINNISH_REGIONS:
        seen.setdefault(name, kind)

    # Function words are never names; drop them after all sources (including
    # folds) have been merged.
    seen = {name: kind for name, kind in seen.items() if name.lower() not in NOT_NAMES}

    rows = sorted(seen.items())
    print(f"{len(rows)} unique names", file=sys.stderr)
    if args.dry_run:
        for name, kind in rows[:20]:
            print(f"{name}\t{kind}")
        return 0

    with open(args.out, "w", encoding="utf-8") as f:
        for name, kind in rows:
            f.write(f"{name}\t{kind}\n")
    print(f"wrote {args.out} ({len(rows)} names)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
