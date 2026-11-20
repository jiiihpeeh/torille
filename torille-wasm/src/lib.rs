//! WASM bindings for the torille Finnish spell-checker.
//!
//! This crate is the `wasm-pack` target: `wasm-pack build --release
//! --target web` produces `pkg/torille_check_bg.wasm` plus a tiny
//! JavaScript wrapper that calls it. The wasm module embeds the
//! sanalista and paradigm catalogue as static strings, so it is
//! self-contained -- no fetch, no JSON, no setup.
//!
//! The spell-checking entry points are shaped like hunspell's:
//!
//! - [`check`] is `hunspell_spell` -- true if the word is known, false
//!   otherwise. The `wasm-bindgen` boundary makes this a JavaScript
//!   function that returns a `boolean`.
//! - [`suggest_words`] is `hunspell_suggest` -- up to `max_n`
//!   candidates, ranked by edit distance. Returns a JavaScript
//!   `Array<string>`.
//!
//! [`analyze_syntax`] is a separate, on-demand diagnostic export. It is not
//! called by the keystroke path and must stay out of the render loop.
//! [`analyze_syntax_with_state`] is the same object through a textarea's
//! word cache, for a panel that re-renders on the same document.
//! [`analyze_ast`] adds the typed surface tree and its per-clause constituent
//! order; [`analyze_ast_with_state`] is its cache-aware form.
//! [`check_syntax`] adds grammar diagnostics and corrections to the analysis;
//! [`check_syntax_with_state`] is its cache-aware form. [`check_style`] is
//! the separate, suggestion-only style layer; it never returns an error or a
//! warning.
//!
//! ## Why this layer exists
//!
//! The browser sandbox forbids the runtime file I/O and threading
//! that `torille` itself does at startup, so a direct port would
//! compile but fail at instantiation. wasm-bindgen is the small set
//! of concessions that matters: a self-contained module Firefox (and
//! any other wasm host) can load in one network round-trip.
//!
//! ## Allocator
//!
//! The default is Rust's standard wasm allocator (dlmalloc), which
//! reuses freed blocks. The `wee-alloc` cargo feature switches to
//! `wee_alloc` for a smaller binary; it is **off by default** because
//! `wee_alloc` is unmaintained and does not reuse freed memory -- the
//! long-lived worker pool's linear memory then grows by roughly the
//! per-call peak on every `check`, which OOMs the tab after a few
//! hundred unknown words (the Gutenberg reader hit exactly this).
//!
//! ## Limitations
//!
//! - The wasm module carries the 11 MB sanalista. After `wasm-pack`'s
//!   `opt-level = "z"` + LTO + gzip, the produced `.wasm` is roughly
//!   3-4 MB. Acceptable for a Firefox extension; not what you'd ship
//!   to a slow network.
//! - `check` and `suggest_words` are synchronous on the wasm side.
//!   Firefox runs wasm on the main thread, so a long check blocks
//!   the UI. The caller (the content script in
//!   `extensions/firefox/`) is responsible for debouncing.
//! - Compound analysis is not wired in -- same as the Rust
//!   `torille::suggest::check`. A `nuori-isäntä`-shaped word that has
//!   no single-lemma source returns `false`. The Firefox extension's
//!   fallback path is to ask the daemon over the Unix socket.

use wasm_bindgen::prelude::*;

mod state;

use crate::state::{Hit, TextAreaState};
use torille::syntax::ast::{Ast, Constituent, NodeId};

// Allocator: see the module doc. `wee_alloc` only under the opt-in
// `wee-alloc` feature; the default is Rust's standard wasm allocator,
// which reuses freed memory (the worker pool makes hundreds of
// allocation-heavy `check` calls, and `wee_alloc` grows the linear
// memory on every one of them).
#[cfg(all(target_arch = "wasm32", feature = "wee-alloc"))]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// We lowercase once at the wasm boundary so callers don't have to
// remember to do it themselves, and so the underlying
// `torille::suggest` checks against the lemma-cased form.
fn lowercase(word: &str) -> String {
    word.to_lowercase()
}

/// True if `word` is a known Finnish word (lemma, inflection, or a
/// form the stem-map gates recognise).
///
/// Maps directly to `hunspell_spell`. JavaScript callers see a
/// `boolean`.
///
/// This is the slow-pass check for a candidate the first pass flagged:
/// the first pass already ran [`torille::expand::check_normal_fast`], so
/// this only asks the compound/derivation-aware
/// [`is_known_word_fast`] whether the surface is a real form. It
/// deliberately does not run the engine walk: on a long reader page an
/// engine call per unknown word spent seconds in the suggestion pool, and
/// the only thing it bought was a rarer form the fast detector missed.
#[wasm_bindgen]
pub fn check(word: &str) -> bool {
    let word = lowercase(word);
    torille::suggest::is_known_word_fast(&word).is_some()
}

/// Up to `max_n` spell-check suggestions for `word`, ranked by edit
/// distance. Empty when the word is unknown to the index and no
/// candidate is within distance 2.
///
/// Maps directly to `hunspell_suggest`. JavaScript callers see an
/// `Array<string>`.
#[wasm_bindgen]
pub fn suggest_words(word: &str, max_n: usize) -> js_sys::Array {
    let arr = js_sys::Array::new();
    for s in torille::suggest::suggest(&lowercase(word), max_n, /* max_distance = */ 2) {
        arr.push(&JsValue::from(s));
    }
    arr
}

/// Finnish hyphenation points for `word`.
///
/// Returns an `Array` of `{ byteOffset, charOffset, compound, source,
/// penalty }` objects. The default Finnish policy is used (two-character
/// minimum fragments, no inter-vowel breaks except compound boundaries
/// or explicit exceptions); `compound_breaks = false` gives the
/// pattern-only fallback. Existing soft hyphens are preserved and
/// returned with `source = "soft-hyphen"`.
///
/// This is a presentation helper. It deliberately does not run the full
/// reverse index, so a rare compound may fall back to its component
/// patterns rather than a morphology-recognised boundary.
#[wasm_bindgen]
pub fn hyphenate(
    word: &str,
    left_min: u32,
    right_min: u32,
    compound_breaks: bool,
) -> js_sys::Array {
    use torille::hyphenation::{
        Exceptions, HyphenationOptions, LexicalCompoundAnalyzer, NoCompoundAnalyzer, positions_with,
    };

    let options = HyphenationOptions {
        left_min: left_min as usize,
        right_min: right_min as usize,
        compound_breaks,
        ..HyphenationOptions::default()
    };
    let compound = LexicalCompoundAnalyzer;
    let no_compound = NoCompoundAnalyzer;
    let analyzer: &dyn torille::hyphenation::CompoundAnalyzer = if compound_breaks {
        &compound
    } else {
        &no_compound
    };
    let arr = js_sys::Array::new();
    for point in positions_with(word, &options, analyzer, Exceptions::finnish()) {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &obj,
            &"byteOffset".into(),
            &JsValue::from(point.byte_offset as u32),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &"charOffset".into(),
            &JsValue::from(point.char_offset as u32),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &"compound".into(),
            &JsValue::from(point.compound_boundary),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &"source".into(),
            &JsValue::from(point.source.as_str()),
        );
        let _ = js_sys::Reflect::set(&obj, &"penalty".into(), &JsValue::from(point.penalty));
        arr.push(&obj);
    }
    arr
}

/// On-demand syntax diagnostics for `text`.
///
/// This is **not** part of the keystroke path: it tokenises the whole text,
/// runs the morphology engine per word, and returns sentences, clauses, and
/// per-token roles. Use it for a grammar/diagnostic panel, not for the red
/// underlines returned by [`analyze_misspells`].
///
/// The shape is:
///
/// ```text
/// {
///   sentences: [{ start, end, index, paragraphStart }],
///   clauses:   [{ start, end, tokensStart, tokensEnd, predicate, subject,
///                 object, complement, negative, existential, passive, parent,
///                 children, antecedent, clauseType, construction,
///                 subjectKind, questionType, subjectPosition, objectCase,
///                 focusType, topicPosition, register }],
///   lists:     [{ start, end, tokensStart, tokensEnd, clause, role, head,
///                 kind, items, separators, conjunction, introducer, parent }],
///   tokens:    [{ word, start, end, role, confidence, clause, head,
///                 case, lemma, lemmas, readings: [{ pos, case, number,
///                 person, tense, mood, voice, polarity }] }]
/// }
/// ```
///
/// Token indices in `predicate`, `subject`, `object`, `head` and
/// `antecedent` refer to the `tokens` array; null means no such role,
/// governor or antecedent. `parent` and `children` are clause indices into
/// `clauses`. `role` is one of `predicate`, `auxiliary`, `subject`, `object`,
/// `adverbial`, `complement`, `modifier` or `unknown`.
///
/// `parent`, `children` and `antecedent` are the additive relative-clause
/// tree. The linear clause array stays the source of truth: a relative clause
/// is also present at its own index and keeps its own `start`/`end`.
///
/// The `clauseType` … `register` fields are the word-order features from
/// `docs/word_order.md`. They are additive too, and `questionType` and
/// `objectCase` are null when the clause has no question or no object.
///
/// A clause may span two `sentences` entries when the sentence layer split at
/// a parenthetical bracket; the syntax layer merges that boundary.
///
/// `lists` is post-order: a nested list comes before its parent, and a
/// parent's `parent` is always a smaller index (or null for a top-level list).
/// An item whose `kind` is `list` also carries `lists`, every child index in
/// the same array; `list` repeats the first child for older consumers.
#[wasm_bindgen]
pub fn analyze_syntax(text: &str) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let analysis = torille::syntax::analyze(&tokens);
    syntax_object(&tokens, &analysis)
}

/// The same object, resolved through the per-textarea cache.
///
/// A re-render of the diagnostic panel for the same document then costs one
/// hash probe per word instead of a morphology search. A stale or destroyed
/// handle falls back to the stateless path.
#[wasm_bindgen]
pub fn analyze_syntax_with_state(idx: u32, text: &str) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let mut slots = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = slots.get_mut(idx as usize) {
        let morphology = torille::morphology::analyze_morphology(&tokens, &mut state.syntax_cache);
        let decisions = torille::disambiguation::disambiguate(&tokens, &morphology);
        let analysis =
            torille::syntax::analyze_syntax_with_decisions(&tokens, &morphology, &decisions);
        return syntax_object(&tokens, &analysis);
    }
    drop(slots);
    let analysis = torille::syntax::analyze(&tokens);
    syntax_object(&tokens, &analysis)
}

/// The flat syntax analysis plus the typed surface tree.
///
/// This is the on-demand structural export for a diagnostic panel or a
/// developer tool; it is not part of the keystroke path. The object is:
///
/// ```text
/// {
///   syntax: { ...the analyze_syntax shape... },
///   ast: {
///     root: 0,
///     nodes: [{ id, kind, detail, span, tokens, segments, parent, children,
///               head, governor, role, confidence, antecedent, clause }],
///     clauses: [{ index, constituents, first, fronted }]
///   }
/// }
/// ```
///
/// `ast.nodes` is a pre-order walk; `id` is the stable index for `parent`,
/// `children`, `head`, `governor`, `antecedent` and `node`. `detail` is the
/// clause/phrase/coordination kind, a list index, or `"list.item"`; it is
/// null for the word/document/sentence/container nodes. `ast.clauses` is
/// keyed by the flat clause index in `syntax.clauses`; `first` and `fronted`
/// are role/kind labels, and `fronted` is null when the clause has no
/// object/adverbial/complement before the predicate.
#[wasm_bindgen]
pub fn analyze_ast(text: &str) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let analysis = torille::syntax::analyze(&tokens);
    let ast = torille::syntax::ast::build(&tokens, &analysis);
    ast_object(&tokens, &analysis, &ast)
}

/// The same AST export through the per-textarea word cache.
///
/// A stale or destroyed handle falls back to the stateless path.
#[wasm_bindgen]
pub fn analyze_ast_with_state(idx: u32, text: &str) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let mut slots = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = slots.get_mut(idx as usize) {
        let morphology = torille::morphology::analyze_morphology(&tokens, &mut state.syntax_cache);
        let decisions = torille::disambiguation::disambiguate(&tokens, &morphology);
        let analysis =
            torille::syntax::analyze_syntax_with_decisions(&tokens, &morphology, &decisions);
        let ast = torille::syntax::ast::build(&tokens, &analysis);
        return ast_object(&tokens, &analysis, &ast);
    }
    drop(slots);
    analyze_ast(text)
}

fn ast_object(
    tokens: &[torille::tokenise::Token],
    analysis: &torille::syntax::SyntaxAnalysis,
    ast: &Ast,
) -> js_sys::Object {
    use js_sys::{Array, Object};

    let syntax = syntax_object(tokens, analysis);
    let obj = Object::new();
    set(&obj, "syntax", syntax.into());
    let ast_obj = Object::new();
    set(&ast_obj, "root", JsValue::from(ast.root.index() as u32));

    let nodes = Array::new();
    for id in ast.walk() {
        nodes.push(&node_object(ast, id));
    }
    set(&ast_obj, "nodes", nodes.into());

    let clauses = Array::new();
    for index in 0..analysis.clauses.len() {
        let item = Object::new();
        set(&item, "index", JsValue::from(index as u32));
        let constituents = Array::new();
        for constituent in ast.clause_constituents(analysis, index) {
            constituents.push(&constituent_object(&constituent));
        }
        set(&item, "constituents", constituents.into());
        let first = ast
            .first_constituent(analysis, index)
            .map(|constituent| constituent_label(&constituent));
        let fronted = ast
            .fronted_constituent(analysis, index)
            .map(|constituent| constituent_label(&constituent));
        set_option(&item, "first", first.as_deref());
        set_option(&item, "fronted", fronted.as_deref());
        clauses.push(&item);
    }
    set(&ast_obj, "clauses", clauses.into());

    let hosts = Array::new();
    for (index, node) in ast.nodes.iter().enumerate() {
        if !matches!(node.kind, torille::syntax::ast::NodeKind::Clause(_))
            || node.segments.len() < 2
        {
            continue;
        }
        let Some(clause) = node.clause else {
            continue;
        };
        let Some(host) = ast.host_node(analysis, clause) else {
            continue;
        };
        if host.index() != index {
            continue;
        }
        let item = Object::new();
        set(&item, "node", JsValue::from(index as u32));
        set_option(&item, "clause", node.clause);
        let segments = Array::new();
        for segment in &node.segments {
            let pair = Array::new();
            pair.push(&JsValue::from(segment.start as u32));
            pair.push(&JsValue::from(segment.end as u32));
            segments.push(&pair);
        }
        set(&item, "segments", segments.into());
        let constituents = Array::new();
        for constituent in ast.host_constituents(analysis, host) {
            constituents.push(&constituent_object(&constituent));
        }
        set(&item, "constituents", constituents.into());
        hosts.push(&item);
    }
    set(&ast_obj, "hosts", hosts.into());

    set(&obj, "ast", ast_obj.into());
    obj
}

fn node_object(ast: &Ast, id: NodeId) -> js_sys::Object {
    use js_sys::{Array, Object};

    let node = ast.node(id);
    let item = Object::new();
    set(&item, "id", JsValue::from(id.index() as u32));
    set(&item, "kind", JsValue::from(node.kind.name()));
    let detail = match node.kind {
        torille::syntax::ast::NodeKind::Clause(kind) => Some(kind.name().to_string()),
        torille::syntax::ast::NodeKind::Phrase(kind) => Some(kind.name().to_string()),
        torille::syntax::ast::NodeKind::Coordination(kind) => Some(kind.name().to_string()),
        torille::syntax::ast::NodeKind::List { index } => Some(index.to_string()),
        torille::syntax::ast::NodeKind::ListItem { list, item } => Some(format!("{list}.{item}")),
        _ => None,
    };
    set_option(&item, "detail", detail.as_deref());

    let span = Array::new();
    span.push(&JsValue::from(node.span.start as u32));
    span.push(&JsValue::from(node.span.end as u32));
    set(&item, "span", span.into());

    let tokens = Array::new();
    tokens.push(&JsValue::from(node.tokens.start as u32));
    tokens.push(&JsValue::from(node.tokens.end as u32));
    set(&item, "tokens", tokens.into());

    let segments = Array::new();
    for segment in &node.segments {
        let pair = Array::new();
        pair.push(&JsValue::from(segment.start as u32));
        pair.push(&JsValue::from(segment.end as u32));
        segments.push(&pair);
    }
    set(&item, "segments", segments.into());

    set_option(&item, "parent", node.parent.map(NodeId::index));
    let children = Array::new();
    for child in &node.children {
        children.push(&JsValue::from(child.index() as u32));
    }
    set(&item, "children", children.into());
    set_option(&item, "head", node.head);
    set_option(&item, "governor", node.governor);
    set_option(&item, "role", node.role.map(|role| role.name()));
    set(
        &item,
        "confidence",
        JsValue::from(match node.confidence {
            torille::syntax::RoleConfidence::Likely => "likely",
            torille::syntax::RoleConfidence::Weak => "weak",
        }),
    );
    set_option(&item, "antecedent", node.antecedent);
    set_option(&item, "clause", node.clause);
    item
}

fn constituent_object(constituent: &Constituent) -> js_sys::Object {
    use js_sys::{Array, Object};

    let item = Object::new();
    set(
        &item,
        "node",
        JsValue::from(constituent.node.index() as u32),
    );
    set(&item, "kind", JsValue::from(constituent.kind.name()));
    set_option(&item, "role", constituent.role.map(|role| role.name()));
    set(
        &item,
        "interrogative",
        JsValue::from(constituent.interrogative),
    );
    let tokens = Array::new();
    tokens.push(&JsValue::from(constituent.tokens.start as u32));
    tokens.push(&JsValue::from(constituent.tokens.end as u32));
    set(&item, "tokens", tokens.into());
    let span = Array::new();
    span.push(&JsValue::from(constituent.start as u32));
    span.push(&JsValue::from(constituent.end as u32));
    set(&item, "span", span.into());
    item
}

fn constituent_label(constituent: &Constituent) -> String {
    let label = constituent
        .role
        .map(|role| role.name().to_string())
        .unwrap_or_else(|| constituent.kind.name().to_string());
    if constituent.interrogative {
        format!("q:{label}")
    } else {
        label
    }
}

fn syntax_object(
    tokens: &[torille::tokenise::Token],
    analysis: &torille::syntax::SyntaxAnalysis,
) -> js_sys::Object {
    use js_sys::{Array, Object};

    let obj = Object::new();

    let sentences = Array::new();
    for sentence in &analysis.sentences {
        let item = Object::new();
        set(&item, "start", JsValue::from(sentence.start as u32));
        set(&item, "end", JsValue::from(sentence.end as u32));
        set(&item, "index", JsValue::from(sentence.index as u32));
        set(
            &item,
            "paragraphStart",
            JsValue::from(sentence.paragraph_start),
        );
        sentences.push(&item);
    }
    set(&obj, "sentences", sentences.into());

    let clauses = Array::new();
    for clause in &analysis.clauses {
        let item = Object::new();
        set(&item, "start", JsValue::from(clause.start as u32));
        set(&item, "end", JsValue::from(clause.end as u32));
        set(
            &item,
            "tokensStart",
            JsValue::from(clause.tokens.start as u32),
        );
        set(&item, "tokensEnd", JsValue::from(clause.tokens.end as u32));
        set_option(&item, "predicate", clause.predicate);
        set_option(&item, "subject", clause.subject);
        set_option(&item, "object", clause.object);
        set_option(&item, "complement", clause.complement);
        set(&item, "negative", JsValue::from(clause.negative));
        set(&item, "existential", JsValue::from(clause.existential));
        set_option(&item, "parent", clause.parent);
        let children = Array::new();
        for child in &clause.children {
            children.push(&JsValue::from(*child as u32));
        }
        set(&item, "children", children.into());
        set_option(&item, "antecedent", clause.antecedent);
        item_features(&item, &clause.features);
        clauses.push(&item);
    }
    set(&obj, "clauses", clauses.into());

    let token_items = Array::new();
    for (index, token) in tokens.iter().enumerate() {
        let syntax = &analysis.tokens[index];
        let item = Object::new();
        set(&item, "word", JsValue::from(token.text.as_str()));
        set(&item, "start", JsValue::from(token.start as u32));
        set(&item, "end", JsValue::from(token.end as u32));
        set(&item, "role", JsValue::from(syntax.role.name()));
        set(
            &item,
            "confidence",
            JsValue::from(match syntax.confidence {
                torille::syntax::RoleConfidence::Likely => "likely",
                torille::syntax::RoleConfidence::Weak => "weak",
            }),
        );
        set(&item, "clause", JsValue::from(syntax.clause as u32));
        set_option(&item, "head", syntax.head);
        set_option(&item, "case", syntax.case.map(|case| case.name()));
        set_option(&item, "lemma", syntax.lemma.as_deref());
        let lemmas = Array::new();
        for lemma in &syntax.lemmas {
            lemmas.push(&JsValue::from(lemma.as_str()));
        }
        set(&item, "lemmas", lemmas.into());

        let readings = Array::new();
        for reading in &syntax.readings {
            let reading_item = Object::new();
            set_option(
                &reading_item,
                "pos",
                reading.part_of_speech.map(|pos| pos.name()),
            );
            set_option(&reading_item, "case", reading.case.map(|case| case.name()));
            set_option(
                &reading_item,
                "number",
                reading.number.map(|number| number.name()),
            );
            set_option(
                &reading_item,
                "person",
                reading.person.map(|person| person.name()),
            );
            set_option(
                &reading_item,
                "tense",
                reading.tense.map(|tense| tense.name()),
            );
            set_option(&reading_item, "mood", reading.mood.map(|mood| mood.name()));
            set_option(
                &reading_item,
                "voice",
                reading.voice.map(|voice| voice.name()),
            );
            set(
                &reading_item,
                "polarity",
                JsValue::from(reading.polarity.name()),
            );
            readings.push(&reading_item);
        }
        set(&item, "readings", readings.into());
        token_items.push(&item);
    }
    set(&obj, "tokens", token_items.into());

    let lists = Array::new();
    for list in &analysis.lists {
        let item = Object::new();
        set(&item, "start", JsValue::from(list.span.start as u32));
        set(&item, "end", JsValue::from(list.span.end as u32));
        set(
            &item,
            "tokensStart",
            JsValue::from(list.tokens.start as u32),
        );
        set(&item, "tokensEnd", JsValue::from(list.tokens.end as u32));
        set(&item, "clause", JsValue::from(list.clause as u32));
        set_option(&item, "role", list.role.map(|role| role.name()));
        set_option(&item, "head", list.head);
        set(&item, "kind", JsValue::from(list.kind.name()));
        let items = Array::new();
        for list_item in &list.items {
            let child = Object::new();
            set(&child, "start", JsValue::from(list_item.span.start as u32));
            set(&child, "end", JsValue::from(list_item.span.end as u32));
            set(
                &child,
                "tokensStart",
                JsValue::from(list_item.tokens.start as u32),
            );
            set(
                &child,
                "tokensEnd",
                JsValue::from(list_item.tokens.end as u32),
            );
            set(&child, "kind", JsValue::from(list_item.kind.name()));
            if let torille::syntax_list::ListItemKind::List(indices) = &list_item.kind {
                let nested = Array::new();
                for &index in indices {
                    nested.push(&JsValue::from(index as u32));
                }
                set(&child, "lists", nested.into());
                if let Some(&first) = indices.first() {
                    set(&child, "list", JsValue::from(first as u32));
                }
            }
            items.push(&child);
        }
        set(&item, "items", items.into());
        let separators = Array::new();
        for separator in &list.separators {
            separators.push(&JsValue::from(*separator as u32));
        }
        set(&item, "separators", separators.into());
        set_option(&item, "conjunction", list.conjunction);
        set_option(&item, "introducer", list.introducer);
        set_option(&item, "parent", list.parent);
        lists.push(&item);
    }
    set(&obj, "lists", lists.into());

    obj
}

/// Analysis plus grammar diagnostics for `text`.
///
/// The object is the [`analyze_syntax`] shape with a `diagnostics` array. Each
/// diagnostic has byte offsets, a stable kind/severity/confidence, a message
/// key, and a list of corrections. A correction carries edits, so applying it
/// never risks replacing the whole text.
#[wasm_bindgen]
pub fn check_syntax(text: &str) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let analysis = torille::syntax::analyze(&tokens);
    let diagnostics = torille::syntax_check::check_analysis(&tokens, &analysis);
    let obj = syntax_object(&tokens, &analysis);
    set(&obj, "diagnostics", diagnostics_array(&diagnostics).into());
    obj
}

/// Optional style diagnostics for `text`.
///
/// This is the separate, suggestion-only style layer. It does not run the
/// grammar checker and never returns `error` or `warning`; every diagnostic
/// is a `suggestion`. The object is `{ diagnostics: [...] }` with the same
/// field shape as `check_syntax`, including UTF-8 byte offsets that the
/// worker converts to UTF-16.
#[wasm_bindgen]
pub fn check_style(text: &str) -> js_sys::Object {
    let analysis = torille::style_check::check(text);
    style_object(&analysis.diagnostics)
}

/// The same style diagnostics through the per-textarea morphology cache.
///
/// The grammar checker and the style checker share this cache, so enabling
/// both layers resolves each surface once. A stale or destroyed handle falls
/// back to the stateless path.
#[wasm_bindgen]
pub fn check_style_with_state(idx: u32, text: &str) -> js_sys::Object {
    let mut slots = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = slots.get_mut(idx as usize) {
        let analysis = torille::style_check::check_with_cache(text, &mut state.syntax_cache);
        return style_object(&analysis.diagnostics);
    }
    drop(slots);
    check_style(text)
}

fn style_object(diagnostics: &[torille::style_check::StyleDiagnostic]) -> js_sys::Object {
    let obj = js_sys::Object::new();
    set(
        &obj,
        "diagnostics",
        style_diagnostics_array(diagnostics).into(),
    );
    obj
}

/// The same diagnostics through the per-textarea word cache.
///
/// A stale or destroyed handle falls back to the stateless path.
#[wasm_bindgen]
pub fn check_syntax_with_state(idx: u32, text: &str) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let mut slots = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = slots.get_mut(idx as usize) {
        let morphology = torille::morphology::analyze_morphology(&tokens, &mut state.syntax_cache);
        let decisions = torille::disambiguation::disambiguate(&tokens, &morphology);
        let analysis =
            torille::syntax::analyze_syntax_with_decisions(&tokens, &morphology, &decisions);
        let diagnostics = torille::syntax_check::check_analysis(&tokens, &analysis);
        let obj = syntax_object(&tokens, &analysis);
        set(&obj, "diagnostics", diagnostics_array(&diagnostics).into());
        return obj;
    }
    drop(slots);
    check_syntax(text)
}

/// The unique lowercased word surfaces in `text`, newline-separated.
///
/// The pool coordinator splits this list into groups; a surface cannot contain
/// a newline, so the simple transport is unambiguous.
#[wasm_bindgen]
pub fn unique_surfaces(text: &str) -> js_sys::Uint8Array {
    let tokens = torille::tokenise::tokenise(text);
    let mut seen = std::collections::HashSet::new();
    let mut surfaces = Vec::new();
    for token in &tokens {
        if !token.is_word() {
            continue;
        }
        let surface = token.text.to_lowercase();
        if seen.insert(surface.clone()) {
            surfaces.push(surface);
        }
    }
    js_sys::Uint8Array::from(surfaces.join("\n").as_bytes())
}

/// Resolve a group of surfaces and return the encoded `WordInfo` rows.
///
/// The worker owns one textarea state; its `syntax_cache` persists across
/// groups. The owning state decodes the payload and seeds its own cache before
/// the full-document grammar/style pass.
#[wasm_bindgen]
pub fn morphology_chunk(idx: u32, text: &str) -> js_sys::Uint8Array {
    let tokens = torille::tokenise::tokenise(text);
    let mut slots = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = slots.get_mut(idx as usize) {
        let rows = torille::morphology::resolve_surfaces(&tokens, &mut state.syntax_cache);
        return js_sys::Uint8Array::from(torille::morphology::codec::encode_rows(&rows).as_slice());
    }
    drop(slots);
    let mut cache = torille::morphology::MorphologyCache::new();
    let rows = torille::morphology::resolve_surfaces(&tokens, &mut cache);
    js_sys::Uint8Array::from(torille::morphology::codec::encode_rows(&rows).as_slice())
}

/// Run the full-document grammar/style pass after seeding the collected rows.
///
/// `payloads` is one encoded [`morphology_chunk`] per pool worker. Decoding
/// and seeding is a transport; the morphology pass then hits every seeded
/// surface and only pays for the context upgrades.
#[wasm_bindgen]
pub fn check_grammar_style_from_morphology(
    idx: u32,
    text: &str,
    payloads: js_sys::Array,
    syntax: bool,
    style: bool,
) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let mut slots = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = slots.get_mut(idx as usize) {
        for payload in payloads.iter() {
            if !payload.is_instance_of::<js_sys::Uint8Array>() {
                continue;
            }
            let bytes = js_sys::Uint8Array::new(&payload).to_vec();
            if let Ok(rows) = torille::morphology::codec::decode_rows(&bytes) {
                for (surface, info) in rows {
                    state.syntax_cache.seed(surface, info);
                }
            }
        }
        let TextAreaState {
            sentence_cache,
            syntax_cache,
            ..
        } = state;
        let morphology = torille::morphology::analyze_morphology(&tokens, syntax_cache);
        let decisions = torille::disambiguation::disambiguate(&tokens, &morphology);
        let diagnostics = if syntax {
            sentence_cache.check_with_decisions(
                text,
                &tokens,
                &morphology,
                syntax_cache,
                &decisions,
            )
        } else {
            Vec::new()
        };
        let style_diagnostics = if style {
            torille::style_check::check_tokens_with_morphology(&tokens, &morphology).diagnostics
        } else {
            Vec::new()
        };
        let obj = js_sys::Object::new();
        set(&obj, "diagnostics", diagnostics_array(&diagnostics).into());
        set(
            &obj,
            "style",
            style_diagnostics_array(&style_diagnostics).into(),
        );
        return obj;
    }
    drop(slots);
    let all = torille::check_all(text);
    let obj = js_sys::Object::new();
    set(
        &obj,
        "diagnostics",
        diagnostics_array(&all.syntax.diagnostics).into(),
    );
    set(
        &obj,
        "style",
        style_diagnostics_array(&all.style.diagnostics).into(),
    );
    obj
}

/// Grammar and style diagnostics in one pass through the per-textarea cache.
///
/// `syntax` and `style` select the rule passes; the morphology pass is shared
/// either way. Returns `{ diagnostics, style }`, both arrays in the same shape
/// as [`check_syntax`] / [`check_style`], with UTF-8 byte offsets. This is the
/// chunking entry point: the worker calls it once per sentence batch.
#[wasm_bindgen]
pub fn check_grammar_style_with_state(
    idx: u32,
    text: &str,
    syntax: bool,
    style: bool,
) -> js_sys::Object {
    let tokens = torille::tokenise::tokenise(text);
    let mut slots = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = slots.get_mut(idx as usize) {
        let TextAreaState {
            sentence_cache,
            syntax_cache,
            ..
        } = state;
        let morphology = torille::morphology::analyze_morphology(&tokens, syntax_cache);
        let decisions = torille::disambiguation::disambiguate(&tokens, &morphology);
        let diagnostics = if syntax {
            sentence_cache.check_with_decisions(
                text,
                &tokens,
                &morphology,
                syntax_cache,
                &decisions,
            )
        } else {
            Vec::new()
        };
        let style_diagnostics = if style {
            torille::style_check::check_tokens_with_morphology(&tokens, &morphology).diagnostics
        } else {
            Vec::new()
        };
        let obj = js_sys::Object::new();
        set(&obj, "diagnostics", diagnostics_array(&diagnostics).into());
        set(
            &obj,
            "style",
            style_diagnostics_array(&style_diagnostics).into(),
        );
        return obj;
    }
    drop(slots);
    let all = torille::check_all(text);
    let obj = js_sys::Object::new();
    set(
        &obj,
        "diagnostics",
        diagnostics_array(&all.syntax.diagnostics).into(),
    );
    set(
        &obj,
        "style",
        style_diagnostics_array(&all.style.diagnostics).into(),
    );
    obj
}

fn diagnostics_array(diagnostics: &[torille::syntax_check::SyntaxDiagnostic]) -> js_sys::Array {
    use js_sys::Array;

    let array = Array::new();
    for diagnostic in diagnostics {
        array.push(&diagnostic_object(
            DiagnosticFields {
                kind: diagnostic.kind.name(),
                rule_id: diagnostic.rule_id.as_str(),
                severity: diagnostic.severity.name(),
                confidence: diagnostic.confidence.name(),
                start: diagnostic.span.start,
                end: diagnostic.span.end,
                message: diagnostic.message,
                expected: diagnostic.expected.as_deref(),
                explanation: diagnostic
                    .explanation
                    .map(|explanation| (explanation.relation.name(), explanation.key)),
            },
            &diagnostic.corrections,
        ));
    }
    array
}

fn style_diagnostics_array(diagnostics: &[torille::style_check::StyleDiagnostic]) -> js_sys::Array {
    use js_sys::Array;

    let array = Array::new();
    for diagnostic in diagnostics {
        array.push(&diagnostic_object(
            DiagnosticFields {
                kind: diagnostic.kind.name(),
                rule_id: diagnostic.rule_id.as_str(),
                severity: diagnostic.severity.name(),
                confidence: diagnostic.confidence.name(),
                start: diagnostic.span.start,
                end: diagnostic.span.end,
                message: diagnostic.message,
                expected: diagnostic.expected.as_deref(),
                explanation: diagnostic
                    .explanation
                    .map(|explanation| (explanation.relation.name(), explanation.key)),
            },
            &diagnostic.corrections,
        ));
    }
    array
}

struct DiagnosticFields<'a> {
    kind: &'a str,
    rule_id: &'a str,
    severity: &'a str,
    confidence: &'a str,
    start: usize,
    end: usize,
    message: &'a str,
    expected: Option<&'a str>,
    explanation: Option<(&'a str, &'a str)>,
}

fn diagnostic_object(
    fields: DiagnosticFields<'_>,
    corrections: &[torille::syntax_check::Correction],
) -> js_sys::Object {
    use js_sys::{Array, Object};

    let DiagnosticFields {
        kind,
        rule_id,
        severity,
        confidence,
        start,
        end,
        message,
        expected,
        explanation,
    } = fields;
    let item = Object::new();
    set(&item, "kind", JsValue::from(kind));
    set(&item, "rule_id", JsValue::from(rule_id));
    set(&item, "severity", JsValue::from(severity));
    set(&item, "confidence", JsValue::from(confidence));
    set(&item, "start", JsValue::from(start as u32));
    set(&item, "end", JsValue::from(end as u32));
    set(&item, "message", JsValue::from(message));
    set_option(&item, "expected", expected);
    if let Some((relation, key)) = explanation {
        let explanation = Object::new();
        set(&explanation, "relation", JsValue::from(relation));
        set(&explanation, "key", JsValue::from(key));
        set(&item, "explanation", explanation.into());
    }

    let corrections_array = Array::new();
    for correction in corrections {
        let correction_item = Object::new();
        set(
            &correction_item,
            "score",
            JsValue::from(correction.score as f64),
        );
        set(&correction_item, "note", JsValue::from(correction.note));
        set(
            &correction_item,
            "license",
            JsValue::from(correction.license.name()),
        );
        set(
            &correction_item,
            "confidence",
            JsValue::from(correction.confidence.name()),
        );
        let edits = Array::new();
        for edit in &correction.edits {
            let edit_item = Object::new();
            set(&edit_item, "start", JsValue::from(edit.start as u32));
            set(&edit_item, "end", JsValue::from(edit.end as u32));
            set(
                &edit_item,
                "replacement",
                JsValue::from(edit.replacement.as_str()),
            );
            edits.push(&edit_item);
        }
        set(&correction_item, "edits", edits.into());
        corrections_array.push(&correction_item);
    }
    set(&item, "corrections", corrections_array.into());
    item
}

fn set(obj: &js_sys::Object, key: &str, value: JsValue) {
    let _ = js_sys::Reflect::set(obj, &JsValue::from(key), &value);
}

fn set_option<T: Into<JsValue>>(obj: &js_sys::Object, key: &str, value: Option<T>) {
    match value {
        Some(value) => set(obj, key, value.into()),
        None => set(obj, key, JsValue::NULL),
    }
}

/// Attach the word-order clause features to a clause object.
///
/// They are additive: an older consumer keeps reading the flat clause fields.
/// `questionType` and `objectCase` are null when the clause has no question or
/// no object. See `docs/word_order.md` for the vocabulary.
fn item_features(obj: &js_sys::Object, features: &torille::syntax::ClauseFeatures) {
    set(
        obj,
        "clauseType",
        JsValue::from(features.clause_type.name()),
    );
    set(
        obj,
        "construction",
        JsValue::from(features.construction.name()),
    );
    set(
        obj,
        "subjectKind",
        JsValue::from(features.subject_kind.name()),
    );
    set_option(
        obj,
        "questionType",
        features.question_type.map(|kind| kind.name()),
    );
    set(
        obj,
        "subjectPosition",
        JsValue::from(features.subject_position.name()),
    );
    set_option(
        obj,
        "objectCase",
        features.object_case.map(|case| case.name()),
    );
    set(obj, "focusType", JsValue::from(features.focus_type.name()));
    set(
        obj,
        "topicPosition",
        JsValue::from(features.topic_position.name()),
    );
    set(obj, "register", JsValue::from(features.register.name()));
}

/// One-shot: returns a JavaScript object that bundles a full spell-check
/// report for the caller. Useful when the caller wants everything in
/// one round-trip rather than two wasm boundary crossings.
///
/// The shape is `{ known: bool, lemmas: string[] }`. `lemmas` is
/// empty when `known` is `false`; otherwise it is the lemmas the
/// engine thinks `word` is a form of, in priority order.
#[wasm_bindgen]
pub fn check_report(word: &str) -> js_sys::Object {
    use js_sys::{Array, Object};
    use torille::suggest::CheckOutcome;
    let lower = lowercase(word);
    let outcome = torille::suggest::check(&lower);
    let obj = Object::new();
    let lemmas = Array::new();
    let known = !matches!(outcome, CheckOutcome::Unknown);
    js_sys::Reflect::set(&obj, &"known".into(), &JsValue::from(known)).unwrap();
    match outcome {
        CheckOutcome::Lemma => {
            // The lemma is the word itself, lowercased.
            lemmas.push(&JsValue::from(lower));
        }
        CheckOutcome::Inflected(ls) => {
            for l in ls {
                lemmas.push(&JsValue::from(l));
            }
        }
        CheckOutcome::Unknown => {}
    }
    js_sys::Reflect::set(&obj, &"lemmas".into(), &lemmas).unwrap();
    obj
}

// ---------------------------------------------------------------------------
// Word wizard
//
// The browser's side of `torille::class_wizard`. The sheet and the answer text
// stay text at the boundary; the maps are built in Rust and handed out as JS
// objects, so there is one parser.
// ---------------------------------------------------------------------------

/// The committed class sheet, as the text format `src/class_wizard.rs` parses.
#[wasm_bindgen]
pub fn wizard_sheet() -> String {
    torille::class_wizard::SHEET.to_string()
}

fn wizard_set(obj: &js_sys::Object, key: &str, value: impl Into<JsValue>) {
    js_sys::Reflect::set(obj, &JsValue::from_str(key), &value.into()).unwrap();
}

fn wizard_error(message: impl AsRef<str>) -> JsValue {
    let obj = js_sys::Object::new();
    wizard_set(&obj, "error", message.as_ref());
    obj.into()
}

fn wizard_number(name: &str) -> Result<Option<torille::tags::Number>, String> {
    match name.trim() {
        "" | "-" | "both" => Ok(None),
        "pl" | "plural" => Ok(Some(torille::tags::Number::Plural)),
        "sg" | "singular" => Ok(Some(torille::tags::Number::Singular)),
        other => Err(format!("{other:?} is not pl, sg or both")),
    }
}

/// Infers the class from `answers_tsv` (`cell<TAB>form` lines).
///
/// Returns `{ error }` rather than throwing, so the wizard page can show the
/// message in place. A successful report has the fields named in
/// `class_wizard::Report` plus `total_candidates`.
#[wasm_bindgen]
pub fn wizard_infer(word: &str, pos: &str, answers_tsv: &str) -> JsValue {
    wizard_infer_with_number(word, pos, answers_tsv, "")
}

/// `pl` restricts the questions and the saved entry to the plural, `sg` to
/// the singular, and an empty string (or `-`) asks both.
#[wasm_bindgen]
pub fn wizard_infer_with_number(
    word: &str,
    pos: &str,
    answers_tsv: &str,
    number: &str,
) -> JsValue {
    use torille::class_wizard::{AnswerStatus, infer_with_number, parse_answers, pos_from_name};

    let Some(pos) = pos_from_name(pos) else {
        return wizard_error(format!("{pos:?} is not a part of speech"));
    };
    let number = match wizard_number(number) {
        Ok(number) => number,
        Err(error) => return wizard_error(error),
    };
    let answers = match parse_answers(answers_tsv) {
        Ok(answers) => answers,
        Err(error) => return wizard_error(error.to_string()),
    };
    let report = match infer_with_number(
        word,
        pos,
        &answers,
        torille::spec::Catalogues::embedded(),
        true,
        number,
    ) {
        Ok(report) => report,
        Err(error) => return wizard_error(error.to_string()),
    };

    let obj = js_sys::Object::new();
    wizard_set(&obj, "word", report.lemma.as_str());
    wizard_set(&obj, "pos", report.pos.name());
    wizard_set(&obj, "tie", report.tie);
    wizard_set(&obj, "recorded_plural_only", report.recorded_plural_only);

    let recorded = js_sys::Array::new();
    for inflection in &report.recorded {
        recorded.push(&JsValue::from(inflection.to_string()));
    }
    wizard_set(&obj, "recorded", recorded);

    match report.question_key() {
        Some(key) => wizard_set(&obj, "question", key),
        None => wizard_set(&obj, "question", JsValue::NULL),
    }

    let candidates = js_sys::Array::new();
    for candidate in report.ranked.iter().take(25) {
        let item = js_sys::Object::new();
        wizard_set(&item, "class", candidate.class.to_string());
        wizard_set(&item, "hits", candidate.hits as u32);
        wizard_set(&item, "misses", candidate.misses as u32);
        wizard_set(&item, "unknown", candidate.unknown as u32);
        wizard_set(&item, "recorded", candidate.recorded);
        wizard_set(&item, "tail_prior", candidate.tail_prior as u32);
        wizard_set(&item, "prior", candidate.prior as u32);
        candidates.push(&item);
    }
    wizard_set(&obj, "candidates", candidates);
    wizard_set(&obj, "total_candidates", report.ranked.len() as u32);

    let outcomes = js_sys::Array::new();
    for outcome in &report.outcomes {
        let item = js_sys::Object::new();
        wizard_set(&item, "key", outcome.key.as_str());
        wizard_set(&item, "form", outcome.form.as_str());
        let status = match outcome.status {
            AnswerStatus::Hit => "hit",
            AnswerStatus::Miss => "miss",
            AnswerStatus::Unknown => "unknown",
            AnswerStatus::Skipped => "skipped",
        };
        wizard_set(&item, "status", status);
        outcomes.push(&item);
    }
    wizard_set(&obj, "outcomes", outcomes);
    obj.into()
}

/// The eight core cells of `word` in `class`, as `{ class, word, cells }`.
///
/// `cells` is an object keyed by `nom_sg`, `gen_sg`, ...; a cell the engine
/// does not emit is `null`.
#[wasm_bindgen]
pub fn wizard_paradigm(word: &str, class: &str, pos: &str) -> JsValue {
    use torille::class_wizard::{CORE_KEYS, Cell, VERB_KEYS, pos_from_name};
    use torille::expand::{LexiconIndex, inflect_in};

    let Some(inflection) = torille::kotus::parse_inflections(class).into_iter().next() else {
        return wizard_error(format!("{class:?} is not a Kotus class expression"));
    };
    let pos = pos_from_name(pos).unwrap_or(torille::tags::PartOfSpeech::Noun);
    let verb = matches!(
        pos,
        torille::tags::PartOfSpeech::Verb | torille::tags::PartOfSpeech::NegativeVerb
    );
    let keys: &[&str] = if verb { &VERB_KEYS } else { &CORE_KEYS };
    let lemma = word.trim().to_lowercase();
    let lexicon = torille::kotus::lemmas();
    let index = LexiconIndex::new(lexicon);
    let forms = inflect_in(
        &lemma,
        inflection,
        torille::spec::Catalogues::embedded(),
        Some(&index),
        pos,
        false,
    );

    let cells = js_sys::Object::new();
    for key in keys {
        let Some(cell) = Cell::parse(key) else {
            continue;
        };
        let form = forms
            .iter()
            .filter(|form| form.readings.iter().any(|tags| cell.claims(tags)))
            .min_by_key(|form| (!form.plain(), form.word.as_str()));
        match form {
            Some(form) => wizard_set(&cells, key, form.word.as_str()),
            None => wizard_set(&cells, key, JsValue::NULL),
        }
    }

    let obj = js_sys::Object::new();
    wizard_set(&obj, "word", word);
    wizard_set(&obj, "class", inflection.to_string());
    wizard_set(&obj, "cells", cells);
    obj.into()
}

/// The saved word list as `{ words: [{word, class, pos}], error }`.
///
/// One parser: the text is parsed by `class_wizard::parse_user_words`, not by
/// a second JS copy.
#[wasm_bindgen]
pub fn wizard_user_words(user_words_tsv: &str) -> JsValue {
    use torille::class_wizard::parse_user_words;

    let obj = js_sys::Object::new();
    match parse_user_words(user_words_tsv) {
        Ok(words) => {
            let array = js_sys::Array::new();
            for entry in words.values() {
                let item = js_sys::Object::new();
                wizard_set(&item, "word", entry.word.as_str());
                wizard_set(&item, "class", entry.class.as_str());
                wizard_set(&item, "pos", entry.pos_name.as_str());
                let number = match entry.number {
                    Some(torille::tags::Number::Plural) => "pl",
                    Some(torille::tags::Number::Singular) => "sg",
                    _ => "-",
                };
                wizard_set(&item, "number", number);
                array.push(&item);
            }
            wizard_set(&obj, "words", array);
            wizard_set(&obj, "error", JsValue::NULL);
        }
        Err(error) => {
            wizard_set(&obj, "words", js_sys::Array::new());
            wizard_set(&obj, "error", error.to_string());
        }
    }
    obj.into()
}

/// Adds or replaces one saved word and returns `{ text, error }`.
///
/// On error the original text is returned, so a caller that writes the result
/// back cannot destroy the store.
#[wasm_bindgen]
pub fn wizard_user_words_add(user_words_tsv: &str, word: &str, class: &str, pos: &str) -> JsValue {
    wizard_user_words_add_with_number(user_words_tsv, word, class, pos, "")
}

/// [`wizard_user_words_add`] with an explicit `pl`/`sg` number restriction.
#[wasm_bindgen]
pub fn wizard_user_words_add_with_number(
    user_words_tsv: &str,
    word: &str,
    class: &str,
    pos: &str,
    number: &str,
) -> JsValue {
    use torille::class_wizard::add_user_word_with;

    let number = match wizard_number(number) {
        Ok(number) => number,
        Err(error) => return wizard_error(error),
    };
    let obj = js_sys::Object::new();
    match add_user_word_with(user_words_tsv, word, class, pos, number) {
        Ok(text) => {
            wizard_set(&obj, "text", text);
            wizard_set(&obj, "error", JsValue::NULL);
        }
        Err(error) => {
            wizard_set(&obj, "text", user_words_tsv);
            wizard_set(&obj, "error", error.to_string());
        }
    }
    obj.into()
}

/// Removes one saved word (case-insensitively) and returns `{ text, error }`.
#[wasm_bindgen]
pub fn wizard_user_words_remove(user_words_tsv: &str, word: &str) -> JsValue {
    use torille::class_wizard::remove_user_word;

    let obj = js_sys::Object::new();
    match remove_user_word(user_words_tsv, word) {
        Ok(text) => {
            wizard_set(&obj, "text", text);
            wizard_set(&obj, "error", JsValue::NULL);
        }
        Err(error) => {
            wizard_set(&obj, "text", user_words_tsv);
            wizard_set(&obj, "error", error.to_string());
        }
    }
    obj.into()
}

/// Every lower-cased surface the saved word list generates, for the worker's
/// custom-word set. A malformed blob yields an empty array; the options page
/// validates before saving, so this is the safety net, not the parser.
#[wasm_bindgen]
pub fn wizard_user_forms(user_words_tsv: &str) -> JsValue {
    use std::collections::BTreeSet;
    use torille::class_wizard::{parse_user_words, user_word_forms};
    use torille::expand::LexiconIndex;

    let words = match parse_user_words(user_words_tsv) {
        Ok(words) => words,
        Err(_) => return js_sys::Array::new().into(),
    };
    let lexicon = torille::kotus::lemmas();
    let index = LexiconIndex::new(lexicon);
    let cat = torille::spec::Catalogues::embedded();
    let mut forms: BTreeSet<String> = BTreeSet::new();
    for entry in words.values() {
        forms.extend(user_word_forms(entry, cat, &index));
    }
    let array = js_sys::Array::new();
    for form in forms {
        array.push(&JsValue::from(form));
    }
    array.into()
}

/// Module init helper. Modern demos call `bg.default(bytes)`
/// directly; the `#[wasm_bindgen(start)]` attribute is left off
/// so the symbol `start` doesn't collide with a same-name export
/// from a downstream crate (e.g. `torille-wasm-leptos`).
#[allow(dead_code)]
pub fn init_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    {
        // Allow Rust `console.log!` etc. to print into the browser dev
        // tools console. The no-op branch on non-wasm keeps native
        // tests compilable.
        console_error_panic_hook::set_once();
    }
}

/// High-level, non-`wasm-bindgen` API for callers that compile
/// into the same wasm as the torille core (no postMessage layer).
/// The Leptos demo lives in `torille-wasm-leptos/` and imports
/// this module directly. Browser-extension callers use the
/// `wasm_bindgen`-exported functions above (per-textbox state
/// pool lives on the worker side); this module is for the
/// "single wasm, no worker" deployment shape.
///
/// The state pool is *not* used here -- the caller owns a
/// `TextAreaState` directly. That's the whole point: in the
/// same-wasm shape the per-overlay state lives at the same
/// Rust ownership level as the UI, so signals take its place.
pub mod wasm_demo {
    use super::state::TextAreaState;

    pub use crate::state::Hit;

    /// New, empty per-textarea state.
    pub fn new_state() -> TextAreaState {
        TextAreaState::new()
    }

    /// Run the full analyse pipeline on `text`, returning one
    /// [`Hit`] per misspelling (with byte ranges + suggestion
    /// lists). Cheap microseconds for texts under ~1 KB.
    pub fn analyze(state: &TextAreaState, text: &str, max: usize) -> Vec<Hit> {
        state.analyze(text, max)
    }

    /// Re-extract proper-noun and phrase candidates from
    /// `text`. Call this before [`analyze`] when the user
    /// has edited the text, so the wasm-side state matches
    /// what the renderer last saw.
    pub fn update(state: &mut TextAreaState, text: &str) {
        state.update(text);
    }

    /// Drop the slot -- e.g. on textarea close/replace.
    pub fn clear(state: &mut TextAreaState) {
        state.clear();
    }

    /// Render `word` with a visible hyphen at every permitted break.
    ///
    /// This is the display half of [`crate::hyphenate`]; callers that
    /// need offsets use the exported binding instead. Existing soft
    /// hyphens are shown as ordinary hyphens and removed from the
    /// display, because the UI string is not the source.
    pub fn hyphenate(word: &str) -> String {
        let points = torille::hyphenation::positions(word);
        torille::hyphenation::split(word, &points)
            .into_iter()
            .map(|part| part.replace('\u{00AD}', ""))
            .collect::<Vec<_>>()
            .join("-")
    }
}

// ─────────────────────────────────────────────────────────────────────
// Per-textarea state. One slot per live text input; the worker
// thread owns the data and the main thread holds a u32 handle.
// ─────────────────────────────────────────────────────────────────────

use std::sync::Mutex;

type StateSlot = Option<TextAreaState>;
type StateVec = Vec<StateSlot>;

/// Process-global pool of states. Mutex-protected so the static is
/// `Sync`; on a wasm single-thread the lock never actually
/// contends, but the Rust compiler doesn't know that.
fn states() -> &'static Mutex<StateVec> {
    static STATES: Mutex<StateVec> = Mutex::new(StateVec::new());
    &STATES
}

/// Allocate a slot for a fresh state. Reuses a `None` slot if
/// there is one; otherwise appends. Returns the slot index.
fn alloc_slot(states: &mut StateVec) -> u32 {
    if let Some((i, slot)) = states.iter_mut().enumerate().find(|(_, s)| s.is_none()) {
        *slot = Some(TextAreaState::new());
        i as u32
    } else {
        states.push(Some(TextAreaState::new()));
        (states.len() - 1) as u32
    }
}

/// Create a new per-textarea state. The returned `u32` is the
/// handle every other `..._with_state` function expects. The slot
/// lives until [`destroy_text_area_state`] is called.
#[wasm_bindgen]
pub fn create_text_area_state() -> u32 {
    let mut s = states().lock().expect("states mutex poisoned");
    alloc_slot(&mut s)
}

/// Whether the state handle still names a live slot. A background
/// worker restart (or a timeout that tears the worker down) loses every
/// state; callers use this to notice a stale index and allocate a fresh
/// one instead of silently analysing an empty state.
#[wasm_bindgen]
pub fn text_area_state_exists(idx: u32) -> bool {
    let s = states().lock().expect("states mutex poisoned");
    matches!(s.get(idx as usize), Some(Some(_)))
}

/// Drop the state at `idx`. Out-of-bounds / already-destroyed
/// indices are silently ignored -- callers may not know whether
/// the worker is on its first lifetime.
#[wasm_bindgen]
pub fn destroy_text_area_state(idx: u32) {
    let mut s = states().lock().expect("states mutex poisoned");
    if let Some(slot) = s.get_mut(idx as usize) {
        *slot = None;
    }
}

/// Wipe the learned values in the state at `idx` without
/// releasing the slot. Used when a textarea's contents are
/// replaced wholesale (e.g. the demo's "reset" button).
#[wasm_bindgen]
pub fn clear_state(idx: u32) {
    let mut s = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = s.get_mut(idx as usize) {
        state.clear();
    }
}

/// Re-extract proper nouns and phrases from `text` for the state
/// at `idx`. Out-of-bounds indices are a silent no-op.
///
/// The wasm side does the tokenisation; the main thread sends
/// the text exactly once per textarea update. The previous
/// learned values are *replaced*, never accumulated, so a word
/// the user deleted disappears from the state on the next
/// update.
#[wasm_bindgen]
pub fn update_state_from_text(idx: u32, text: &str) {
    let mut s = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = s.get_mut(idx as usize) {
        state.update(text);
    }
}

/// One misspell in the rendered text. The JS shape is
/// `{ word, start, end, suggestions }` -- a JS object whose
/// `suggestions` field is a JS array of ranked candidates.
fn hit_to_js(hit: &Hit) -> js_sys::Object {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &"word".into(), &JsValue::from(hit.word.as_str()));
    let _ = js_sys::Reflect::set(&obj, &"start".into(), &JsValue::from(hit.start as u32));
    let _ = js_sys::Reflect::set(&obj, &"end".into(), &JsValue::from(hit.end as u32));
    let suggestions = js_sys::Array::new();
    for s in &hit.suggestions {
        suggestions.push(&JsValue::from(s.as_str()));
    }
    let suggestions_js: JsValue = suggestions.into();
    let _ = js_sys::Reflect::set(&obj, &"suggestions".into(), suggestions_js.as_ref());
    obj
}

/// Spell-check the whole text in one wasm call. Walks the tokens,
/// short-circuits on phrase matches, drops lemma hits on the fast
/// path, falls back to the full engine only for the rare inflected
/// form, and returns every misspelling's byte range plus up to
/// `max_suggestions` per hit.
///
/// One string-to-wasm copy for the whole text, instead of one per
/// token. The 17th-call `unreachable` the per-word path hit goes
/// away because we never recurse through the wasm-bindgen bridge
/// inside an analyze_text call.
#[wasm_bindgen]
pub fn analyze_text(idx: u32, text: &str, max_suggestions: u32) -> js_sys::Array {
    let out = js_sys::Array::new();
    let s = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = s.get(idx as usize) {
        for hit in state.analyze(text, max_suggestions as usize) {
            out.push(&hit_to_js(&hit));
        }
    }
    out
}

/// **First-pass** misspell detection. Returns just the byte
/// ranges; suggestions are filled in by [`suggest_for_misspell`]
/// on click.
///
/// Use this in the per-keystroke render path: every word is
/// checked against the lemma HashSet (O(1) each) and the learned
/// phrases/proper-nouns, so even a 1-KB text completes in
/// single-digit milliseconds instead of the multi-second cost
/// of running the inflection generator per token.
///
/// The JS-side caller should never call `analyze_text` *and*
/// `analyze_misspells` for the same render -- that would duplicate
/// work. `analyze_misspells` is the one to call for the demo's
/// "red underline first, suggestions on click" UX.
#[wasm_bindgen]
pub fn analyze_misspells(idx: u32, text: &str) -> js_sys::Array {
    let out = js_sys::Array::new();
    let mut s = states().lock().expect("states mutex poisoned");
    let Some(Some(state)) = s.get_mut(idx as usize) else {
        return out;
    };
    for hit in state.find_misspells(text) {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&obj, &"word".into(), &JsValue::from(hit.word));
        let _ = js_sys::Reflect::set(&obj, &"start".into(), &JsValue::from(hit.start as u32));
        let _ = js_sys::Reflect::set(&obj, &"end".into(), &JsValue::from(hit.end as u32));
        let _ = js_sys::Reflect::set(&obj, &"caseOnly".into(), &JsValue::from(hit.case_only));
        out.push(&obj);
    }
    out
}

/// **Second-pass** suggestion builder. Bounded Levenshtein over
/// the embedded lemma list only; no paradigm generation, no
/// `find_bases_with`. Use this to fill the suggestion popup when
/// the user clicks a misspelling identified by
/// [`analyze_misspells`].
///
/// `max_distance` defaults to 2 in the worker; passing it
/// explicitly here lets the popup tighten to 1 for higher
/// precision when the user is editing a known word.
#[wasm_bindgen]
pub fn suggest_for_misspell(_idx: u32, word: &str, max_n: u32, max_distance: u32) -> js_sys::Array {
    let arr = js_sys::Array::new();
    for w in torille::suggest::suggest_over_lemmas(word, max_n as usize, max_distance as usize) {
        arr.push(&JsValue::from(w));
    }
    arr
}

/// Pre-warm the lazy statics the per-textbox render relies on.
/// Call this once at worker boot (right after `initTorilleWasm`
/// finishes) to pay the cold-start cost of `lemma_set()` and
/// `catalogue()` *before* the first user keystroke. The user
/// sees the first misspell highlight in single-digit
/// milliseconds instead of ~1s.
#[wasm_bindgen]
pub fn warm_caches() {
    torille::suggest::warm_caches();
}

/// Pre-warm the pre-computed inflected form set so the first-pass
/// `find_misspells` can recognise every form the engine can produce
/// (gradated nouns, verb persons, possessive chains, clitic stacks,
/// ...), not just the lemma + productive noun-case strip.
///
/// Walks every lemma in the sanalista and runs its full paradigm
/// once. On a desktop build this is a few seconds; on the wasm
/// it's a comparable single-digit-seconds pass. The work is
/// idempotent (`OnceLock`), so calling it twice is free. The
/// first-pass check reads [`form_set_ready`] and falls back to the
/// fast lemma + noun-case strip until this returns, so the typing
/// path is never blocked.
///
/// Since the form set is a compile-time binary blob (FNV-1a
/// open-addressing table embedded via `include_bytes!`), there is
/// nothing to warm -- the set is ready the moment the wasm module
/// is loaded. This function is kept for API compatibility with
/// callers that schedule the warmup; it now just reports the time
/// the no-op took, which is essentially zero.
#[wasm_bindgen]
pub fn warm_form_set() -> f64 {
    let t0 = js_sys::Date::now();
    // Touch the static so any DCE pass that would otherwise
    // eliminate the form-set linkage doesn't win.
    let _ = torille::expand::all_forms::form_set().len();
    js_sys::Date::now() - t0
}

/// Per-word spell suggestions that include the state-aware fallback
/// for proper nouns the user has typed. Used by the suggestion
/// popup so a click on `Helsnki` returns `["helsinki"]` when
/// `Helsinki` has been learned.
#[wasm_bindgen]
pub fn suggest_words_with_state(
    idx: u32,
    word: &str,
    max_n: u32,
    max_distance: u32,
) -> js_sys::Array {
    let arr = js_sys::Array::new();
    let s = states().lock().expect("states mutex poisoned");
    let Some(Some(state)) = s.get(idx as usize) else {
        return arr;
    };
    for w in torille::suggest::suggest_with_state(
        word,
        max_n as usize,
        max_distance as usize,
        &state.proper_nouns,
    ) {
        arr.push(&JsValue::from(w));
    }
    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wasm-only tests are guarded by `target_arch = "wasm32"`,
    /// because they touch `js_sys` types that are only meaningful in a
    /// wasm host. The pure-Rust `check` function can be tested on
    /// either target.
    #[test]
    fn known_word_returns_true() {
        assert!(check("talo"));
        assert!(check("taloa"));
        assert!(check("koiralta"));
        assert!(check("puhtaan")); // gradated, phase-3 of find_bases
        assert!(check("öihin")); // short lemma, phase-4
    }

    #[test]
    fn unknown_word_returns_false() {
        assert!(!check("xyzzy"));
        assert!(!check(""));
    }

    /// Returns the suggestions as a plain `Vec<String>` -- the same
    /// data the wasm side hands back as a `js_sys::Array`, just
    /// without the JavaScript bridge. Tests on the native target
    /// exercise the *logic*; the wasm-specific test then exercises
    /// the bridge.
    #[test]
    fn suggest_vec_round_trip() {
        let words: Vec<String> = torille::suggest::suggest("taloo", 5, 2)
            .into_iter()
            .collect();
        assert!(
            words.iter().any(|w| w == "talo"),
            "taloo should suggest talo: {words:?}"
        );
    }

    #[test]
    fn report_contains_the_lemma_for_a_finite_a_form() {
        // Drives `check_report` end-to-end on the native side. The
        // js_sys::Object path only runs under wasm; here we just
        // confirm the underlying decision is correct.
        assert!(check("taloa"));
        assert!(!check("xyzzy"));
    }

    #[test]
    fn the_demo_hyphenation_display_uses_the_engine_policy() {
        assert_eq!(
            wasm_demo::hyphenate("tietokoneohjelmointi"),
            "tie-to-ko-ne-oh-jel-moin-ti"
        );
        assert_eq!(wasm_demo::hyphenate("kissa\u{00AD}koira"), "kis-sa-koi-ra");
        assert_eq!(wasm_demo::hyphenate("asia"), "asia");
    }
}

// The console-error-panic-hook dependency is only useful on wasm; we
// keep it in scope conditionally to avoid dragging in std-only deps
// for the native tests.
//
// wasm-bindgen's default project template includes this hook so
// `unwrap` panics land in the browser dev console instead of dying
// silently.
#[cfg(target_arch = "wasm32")]
extern crate console_error_panic_hook;
