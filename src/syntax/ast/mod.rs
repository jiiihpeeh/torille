//! A typed surface tree over the flat syntax analysis.
//!
//! [`crate::syntax::SyntaxAnalysis`] answers "what is this word doing?" with a
//! token array, a clause array and a list arena. This module groups those
//! facts into a constituency tree: sentences contain clauses, clauses contain
//! phrases and leaves, and lists own their items.
//!
//! The tree is **derived**, not authoritative. [`build`] never searches the
//! lexicon, never re-runs the role resolver and never invents a role. Every
//! node copies the role and confidence the flat model already chose. When the
//! flat model says [`SyntacticRole::Unknown`], the tree keeps an unattached
//! [`NodeKind::Word`] leaf rather than guessing a phrase.
//!
//! The design, the construction rules and the phase plan are in
//! `../../misc_md/PLAN_SYNTAX_AST.md`.
//!
//! Phase 0 covers the skeleton: one [`NodeKind::Document`], one
//! [`NodeKind::Sentence`] per sentence span that contains a word, one
//! [`NodeKind::Clause`] per flat clause, and one [`NodeKind::Word`] leaf per
//! word token. Phase 1 adds head-driven [`NodeKind::Phrase`] nodes inside each
//! clause. List nodes, host segments and coordination are later phases.

use std::ops::Range;

use crate::syntax::{
    Clause, QuestionType, RoleConfidence, SyntacticRole, SyntaxAnalysis, TokenSyntax,
};
use crate::syntax_list::{List, ListItem, ListKind};
use crate::tags::Clitic;
use crate::tokenise::Token;

mod attach;
mod clauses;
mod lists;
mod phrases;
mod seed;
mod validate;

pub use validate::validate;

/// A stable handle for one node in [`Ast::nodes`].
///
/// The arena is append-only, so a `NodeId` stays valid for the lifetime of the
/// tree. Ids are creation order, not source order; use [`Ast::walk`] for a
/// source-order traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u32);

impl NodeId {
    /// The index into [`Ast::nodes`].
    pub fn index(self) -> usize {
        self.0 as usize
    }

    fn new(index: usize) -> Self {
        Self(u32::try_from(index).expect("syntax tree has more than u32::MAX nodes"))
    }
}

/// The structural kind of a clause.
///
/// These are structural subtypes only. The clause's sentence type,
/// construction, question type and information-structure features stay on
/// [`Clause::features`] and are read through [`Node::clause`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClauseKind {
    Main,
    Relative,
    Subordinate,
    Infinitive,
    /// Reserved for the optional coordination phase.
    Coordination,
}

impl ClauseKind {
    pub fn name(self) -> &'static str {
        match self {
            ClauseKind::Main => "main",
            ClauseKind::Relative => "relative",
            ClauseKind::Subordinate => "subordinate",
            ClauseKind::Infinitive => "infinitive",
            ClauseKind::Coordination => "coordination",
        }
    }
}

/// The lexical kind of a phrase.
///
/// Phase 0 does not build phrases yet; the enum is public from the start so
/// later phases are additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhraseKind {
    Noun,
    Adjective,
    Pronoun,
    Numeral,
    Adverb,
    Postposition,
    Preposition,
    Verb,
    Infinitive,
    Participle,
}

impl PhraseKind {
    pub fn name(self) -> &'static str {
        match self {
            PhraseKind::Noun => "noun",
            PhraseKind::Adjective => "adjective",
            PhraseKind::Pronoun => "pronoun",
            PhraseKind::Numeral => "numeral",
            PhraseKind::Adverb => "adverb",
            PhraseKind::Postposition => "postposition",
            PhraseKind::Preposition => "preposition",
            PhraseKind::Verb => "verb",
            PhraseKind::Infinitive => "infinitive",
            PhraseKind::Participle => "participle",
        }
    }
}

/// The category of the conjuncts in a coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoordKind {
    Nominal,
    Verbal,
    Clausal,
    Mixed,
}

impl CoordKind {
    pub fn name(self) -> &'static str {
        match self {
            CoordKind::Nominal => "nominal",
            CoordKind::Verbal => "verbal",
            CoordKind::Clausal => "clausal",
            CoordKind::Mixed => "mixed",
        }
    }
}

/// What a node represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Document,
    Sentence,
    Clause(ClauseKind),
    Phrase(PhraseKind),
    List { index: usize },
    ListItem { list: usize, item: usize },
    Coordination(CoordKind),
    Parenthetical,
    Quote,
    Word,
}

impl NodeKind {
    /// A stable name for diagnostics, JSON and the debug dump.
    pub fn name(self) -> &'static str {
        match self {
            NodeKind::Document => "document",
            NodeKind::Sentence => "sentence",
            NodeKind::Clause(_) => "clause",
            NodeKind::Phrase(_) => "phrase",
            NodeKind::List { .. } => "list",
            NodeKind::ListItem { .. } => "list_item",
            NodeKind::Coordination(_) => "coordination",
            NodeKind::Parenthetical => "parenthetical",
            NodeKind::Quote => "quote",
            NodeKind::Word => "word",
        }
    }
}

/// One node in the surface tree.
///
/// `head` and `governor` are deliberately separate. `head` is the lexical
/// head of a phrase or the predicate of a clause; `governor` is the flat
/// [`TokenSyntax::head`] edge. In `Talo on kaunis`, `kaunis` is headed by
/// itself and governed by `on`; only a `Modifier` or `Auxiliary` edge becomes
/// a parent/child link inside a phrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    /// Byte envelope for diagnostics and UI highlighting.
    pub span: Range<usize>,
    /// Token-index envelope. Separators are not leaves.
    pub tokens: Range<usize>,
    /// Token ranges this node owns directly. Usually one; a host clause split
    /// by a relative clause will have two or more in a later phase.
    pub segments: Vec<Range<usize>>,
    pub parent: Option<NodeId>,
    /// Source order. Never empty for a node that owns words.
    pub children: Vec<NodeId>,
    /// The token this node is about: the phrase head, the clause predicate,
    /// the list head, or the conjunction of a coordination. `None` for the
    /// document, a sentence and a list item.
    pub head: Option<usize>,
    /// The flat [`TokenSyntax::head`] of `head`: the governor edge. It may
    /// repeat the parent link for an attached modifier; it is the only copy of
    /// a clause-level edge the tree keeps.
    pub governor: Option<usize>,
    /// The role the flat model chose for `head`, copied, never re-derived.
    pub role: Option<SyntacticRole>,
    pub confidence: RoleConfidence,
    /// For a relative clause, the nominal the relative pronoun modifies.
    /// `None` on every other node and when the flat model left it unlinked.
    pub antecedent: Option<usize>,
    /// Index into [`SyntaxAnalysis::clauses`] for the node's head token. A
    /// multi-segment host uses the flat clause of its predicate.
    pub clause: Option<usize>,
}

impl Node {
    fn framed(kind: NodeKind, span: Range<usize>, tokens: Range<usize>) -> Self {
        let segments = if tokens.start < tokens.end {
            vec![tokens.clone()]
        } else {
            Vec::new()
        };
        Self {
            kind,
            span,
            tokens,
            segments,
            parent: None,
            children: Vec::new(),
            head: None,
            governor: None,
            role: None,
            confidence: RoleConfidence::Weak,
            antecedent: None,
            clause: None,
        }
    }

    fn document(tokens: Range<usize>, span_end: usize) -> Self {
        let mut node = Self::framed(NodeKind::Document, 0..span_end, tokens);
        node.confidence = RoleConfidence::Likely;
        node
    }

    fn sentence(span: Range<usize>, tokens: Range<usize>) -> Self {
        let mut node = Self::framed(NodeKind::Sentence, span, tokens);
        node.confidence = RoleConfidence::Likely;
        node
    }

    fn clause(clause: &Clause, index: usize, kind: ClauseKind) -> Self {
        let mut node = Self::framed(
            NodeKind::Clause(kind),
            clause.start..clause.end,
            clause.tokens.clone(),
        );
        node.head = clause.predicate;
        node.confidence = if clause.predicate.is_some() {
            RoleConfidence::Likely
        } else {
            RoleConfidence::Weak
        };
        node.antecedent = clause.antecedent;
        node.clause = Some(index);
        node
    }

    fn host_clause(
        kind: ClauseKind,
        first: &Clause,
        continuation: &Clause,
        continuation_index: usize,
    ) -> Self {
        let tokens = first.tokens.start..continuation.tokens.end;
        let mut node = Self::framed(
            NodeKind::Clause(kind),
            first.start..continuation.end,
            tokens,
        );
        node.segments = vec![first.tokens.clone(), continuation.tokens.clone()];
        node.head = continuation.predicate;
        node.confidence = if continuation.predicate.is_some() {
            RoleConfidence::Likely
        } else {
            RoleConfidence::Weak
        };
        node.clause = Some(continuation_index);
        node
    }

    fn word(token: &Token, index: usize, syntax: &TokenSyntax) -> Self {
        let mut node = Self::framed(NodeKind::Word, token.start..token.end, index..index + 1);
        node.head = Some(index);
        node.governor = syntax.head;
        node.role = Some(syntax.role);
        node.confidence = syntax.confidence;
        node.clause = Some(syntax.clause);
        node
    }

    fn phrase(
        kind: PhraseKind,
        tokens: Range<usize>,
        span: Range<usize>,
        head: usize,
        syntax: &TokenSyntax,
        clause: usize,
    ) -> Self {
        let mut node = Self::framed(NodeKind::Phrase(kind), span, tokens);
        node.head = Some(head);
        node.governor = syntax.head;
        node.role = Some(syntax.role);
        node.confidence = syntax.confidence;
        node.clause = Some(clause);
        node
    }

    fn list(list: &List, index: usize) -> Self {
        let mut node = Self::framed(
            NodeKind::List { index },
            list.span.clone(),
            list.tokens.clone(),
        );
        node.head = list.head;
        node.role = list.role;
        node.confidence = if list.kind == ListKind::Mixed {
            RoleConfidence::Weak
        } else {
            RoleConfidence::Likely
        };
        node.clause = Some(list.clause);
        node
    }

    fn list_item(list: &List, item: &ListItem, list_index: usize, item_index: usize) -> Self {
        let mut node = Self::framed(
            NodeKind::ListItem {
                list: list_index,
                item: item_index,
            },
            item.span.clone(),
            item.tokens.clone(),
        );
        node.confidence = if list.kind == ListKind::Mixed {
            RoleConfidence::Weak
        } else {
            RoleConfidence::Likely
        };
        node.clause = Some(list.clause);
        node
    }

    fn parenthetical(span: Range<usize>, tokens: Range<usize>, clause: Option<usize>) -> Self {
        Self::container(NodeKind::Parenthetical, span, tokens, clause)
    }

    fn quote(span: Range<usize>, tokens: Range<usize>, clause: Option<usize>) -> Self {
        Self::container(NodeKind::Quote, span, tokens, clause)
    }

    fn container(
        kind: NodeKind,
        span: Range<usize>,
        tokens: Range<usize>,
        clause: Option<usize>,
    ) -> Self {
        let mut node = Self::framed(kind, span, tokens);
        node.confidence = RoleConfidence::Likely;
        node.clause = clause;
        node
    }
}

/// The whole tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ast {
    /// Append-only arena. Ids are creation order, not source order.
    pub nodes: Vec<Node>,
    /// The `Document` node. [`build`] creates it first, so it is id 0.
    pub root: NodeId,
}

impl Ast {
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.index()]
    }

    pub fn span(&self, id: NodeId) -> &Range<usize> {
        &self.node(id).span
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.node(id).children
    }

    /// A pre-order walk: parent before children, siblings in source order.
    pub fn walk(&self) -> Vec<NodeId> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            out.push(id);
            stack.extend(self.node(id).children.iter().rev().copied());
        }
        out
    }

    /// The direct constituents of one flat clause, in source order.
    ///
    /// A host clause is found by the segment whose token range is the flat
    /// clause's range, so `Mies` and `söi` in `Mies, joka tuli, söi` are seen
    /// as constituents of separate flat clauses even though they share a host.
    pub fn clause_constituents(
        &self,
        analysis: &SyntaxAnalysis,
        clause_index: usize,
    ) -> Vec<Constituent> {
        let Some(clause) = analysis.clauses.get(clause_index) else {
            return Vec::new();
        };
        let Some(node_id) = self.clause_node(analysis, clause_index) else {
            return Vec::new();
        };
        self.node(node_id)
            .children
            .iter()
            .filter_map(|&child| {
                let node = self.node(child);
                if node.tokens.start < clause.tokens.start || node.tokens.end > clause.tokens.end {
                    return None;
                }
                Some(self.constituent_of(analysis, Some(clause_index), child))
            })
            .collect()
    }

    /// The direct children of a multi-segment host, in source order.
    ///
    /// The relative clause appears as one `Constituent` with `role = None`;
    /// the host’s own segments keep their flat-clause roles.
    pub fn host_constituents(&self, analysis: &SyntaxAnalysis, host: NodeId) -> Vec<Constituent> {
        let node = self.node(host);
        if !matches!(node.kind, NodeKind::Clause(_)) || node.segments.len() < 2 {
            return Vec::new();
        }
        node.children
            .iter()
            .map(|&child| {
                let clause = self.node(child).clause.or(node.clause);
                self.constituent_of(analysis, clause, child)
            })
            .collect()
    }

    /// The clause node that owns a flat clause, whether it is a normal node or
    /// a host segment.
    fn clause_node(&self, analysis: &SyntaxAnalysis, clause_index: usize) -> Option<NodeId> {
        let clause = analysis.clauses.get(clause_index)?;
        self.nodes.iter().enumerate().find_map(|(index, node)| {
            if !matches!(node.kind, NodeKind::Clause(_)) {
                return None;
            }
            node.segments
                .contains(&clause.tokens)
                .then_some(NodeId::new(index))
        })
    }

    /// The multi-segment host that owns a flat clause, if there is one.
    pub fn host_node(&self, analysis: &SyntaxAnalysis, clause_index: usize) -> Option<NodeId> {
        let host = self.clause_node(analysis, clause_index)?;
        (self.node(host).segments.len() > 1).then_some(host)
    }

    fn constituent_of(
        &self,
        analysis: &SyntaxAnalysis,
        clause_index: Option<usize>,
        child: NodeId,
    ) -> Constituent {
        let node = self.node(child);
        Constituent {
            node: child,
            kind: node.kind,
            role: node.role,
            tokens: node.tokens.clone(),
            start: node.span.start,
            end: node.span.end,
            interrogative: clause_index
                .is_some_and(|clause| self.is_interrogative(analysis, clause, node)),
        }
    }

    fn is_interrogative(
        &self,
        analysis: &SyntaxAnalysis,
        clause_index: usize,
        node: &Node,
    ) -> bool {
        let Some(clause) = analysis.clauses.get(clause_index) else {
            return false;
        };
        let Some(head) = node.head else {
            return false;
        };
        let Some(syntax) = analysis.tokens.get(head) else {
            return false;
        };
        match clause.features.question_type {
            Some(QuestionType::Wh | QuestionType::Echo) => {
                crate::syntax::INTERROGATIVE_SURFACES.contains(&syntax.key.as_str())
            }
            Some(QuestionType::YesNo | QuestionType::Alternative) => {
                syntax
                    .readings
                    .iter()
                    .any(|reading| reading.clitic == Some(Clitic::Ko))
                    || syntax.key.ends_with("ko")
                    || syntax.key.ends_with("kö")
            }
            _ => false,
        }
    }

    /// The first direct constituent of one flat clause.
    pub fn first_constituent(
        &self,
        analysis: &SyntaxAnalysis,
        clause_index: usize,
    ) -> Option<Constituent> {
        self.clause_constituents(analysis, clause_index)
            .into_iter()
            .next()
    }

    /// The first object, adverbial or complement that precedes the predicate.
    ///
    /// A subject before the predicate is the neutral order, not fronting, and
    /// an unknown/modifier word is not asserted to be a moved constituent.
    pub fn fronted_constituent(
        &self,
        analysis: &SyntaxAnalysis,
        clause_index: usize,
    ) -> Option<Constituent> {
        let clause = analysis.clauses.get(clause_index)?;
        let predicate = clause.predicate?;
        let constituents = self.clause_constituents(analysis, clause_index);
        let predicate_start = constituents
            .iter()
            .find(|constituent| constituent.tokens.contains(&predicate))
            .map(|constituent| constituent.start)?;
        constituents.into_iter().find(|constituent| {
            constituent.start < predicate_start
                && matches!(
                    constituent.role,
                    Some(
                        SyntacticRole::Object
                            | SyntacticRole::Adverbial
                            | SyntacticRole::Complement
                    )
                )
        })
    }
}

/// One top-level constituent of a clause in the surface tree.
///
/// The word-order layer needs constituents, not token positions: `Kirjan
/// luin eilen` has an object constituent before the predicate, while
/// `Sami söi aamupuuronsa` has a subject there. A `Constituent` is a direct
/// child of the clause node (a phrase, a list, a container or a bare word).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constituent {
    pub node: NodeId,
    pub kind: NodeKind,
    pub role: Option<SyntacticRole>,
    pub tokens: Range<usize>,
    pub start: usize,
    pub end: usize,
    /// True when this constituent carries the clause's question focus: a wh
    /// word in a `Wh`/`Echo` question, or a `-ko`/`-kö` host in a yes/no or
    /// alternative question.
    pub interrogative: bool,
}

/// The analysis plus its surface tree, from one morphology pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTree {
    pub analysis: SyntaxAnalysis,
    pub ast: Ast,
}

impl SyntaxTree {
    /// The direct constituents of one flat clause, in source order.
    pub fn clause_constituents(&self, clause: usize) -> Vec<Constituent> {
        self.ast.clause_constituents(&self.analysis, clause)
    }

    /// The first direct constituent of one flat clause.
    pub fn first_constituent(&self, clause: usize) -> Option<Constituent> {
        self.ast.first_constituent(&self.analysis, clause)
    }

    /// The first object/adverbial/complement before the predicate, if any.
    pub fn fronted_constituent(&self, clause: usize) -> Option<Constituent> {
        self.ast.fronted_constituent(&self.analysis, clause)
    }

    /// The direct children of the multi-segment host that owns a flat clause.
    pub fn host_constituents(&self, clause: usize) -> Vec<Constituent> {
        let Some(host) = self.ast.host_node(&self.analysis, clause) else {
            return Vec::new();
        };
        self.ast.host_constituents(&self.analysis, host)
    }

    /// The multi-segment host node that owns a flat clause, if there is one.
    pub fn host_node(&self, clause: usize) -> Option<NodeId> {
        self.ast.host_node(&self.analysis, clause)
    }
}

/// Build the surface tree from an existing analysis.
///
/// This is pure: it never searches the lexicon and never re-runs the role
/// resolver. In debug builds the result is validated before it is returned.
pub fn build(tokens: &[Token], analysis: &SyntaxAnalysis) -> Ast {
    let mut builder = Builder::new();
    let seed = seed::build(tokens, analysis, &mut builder);
    let root = seed.root;
    let clause_nodes = clauses::group(tokens, analysis, &mut builder, &seed);
    phrases::attach(tokens, analysis, &mut builder, &seed, &clause_nodes);
    lists::attach(tokens, analysis, &mut builder, &clause_nodes);
    attach::parentheticals(tokens, analysis, &mut builder, &seed);
    attach::quotes(tokens, analysis, &mut builder, &seed);
    let ast = builder.finish(root);

    #[cfg(debug_assertions)]
    if let Err(violations) = validate(tokens, analysis, &ast) {
        panic!("syntax AST invariant failed: {violations:#?}");
    }

    ast
}

/// Analyse a token stream and build its surface tree.
///
/// The morphology cache path is the same one [`crate::syntax::analyze`] uses;
/// this function pays for the analysis and the tree, nothing else.
pub fn analyze(tokens: &[Token]) -> SyntaxTree {
    let analysis = crate::syntax::analyze(tokens);
    let ast = build(tokens, &analysis);
    SyntaxTree { analysis, ast }
}

/// A validation failure. One entry per broken invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub node: Option<NodeId>,
    pub rule: &'static str,
    pub detail: String,
}

pub(super) struct Builder {
    nodes: Vec<Node>,
}

impl Builder {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn push(&mut self, node: Node) -> NodeId {
        let id = NodeId::new(self.nodes.len());
        self.nodes.push(node);
        id
    }

    fn attach(&mut self, parent: NodeId, child: NodeId) {
        debug_assert_ne!(parent, child, "a node cannot be its own parent");
        let parent_index = parent.index();
        let child_index = child.index();
        debug_assert!(parent_index < self.nodes.len());
        debug_assert!(child_index < self.nodes.len());
        self.nodes[child_index].parent = Some(parent);
        self.nodes[parent_index].children.push(child);
    }

    fn reparent(&mut self, parent: NodeId, child: NodeId) {
        let child_index = child.index();
        let Some(old) = self.nodes[child_index].parent else {
            self.attach(parent, child);
            return;
        };
        if old == parent {
            return;
        }
        self.nodes[old.index()].children.retain(|&id| id != child);
        self.nodes[child_index].parent = None;
        self.attach(parent, child);
    }

    fn sort_children(&mut self) {
        for index in 0..self.nodes.len() {
            let mut children = std::mem::take(&mut self.nodes[index].children);
            children.sort_by_key(|&id| {
                let node = &self.nodes[id.index()];
                (node.span.start, node.span.end)
            });
            self.nodes[index].children = children;
        }
    }

    fn finish(self, root: NodeId) -> Ast {
        Ast {
            nodes: self.nodes,
            root,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn analyzed(text: &str) -> (Vec<Token>, SyntaxTree) {
        let tokens = tokenise(text);
        let tree = analyze(&tokens);
        (tokens, tree)
    }

    fn clauses(tree: &SyntaxTree) -> Vec<&Node> {
        tree.ast
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::Clause(_)))
            .collect()
    }

    fn phrase_for<'a>(tree: &'a SyntaxTree, tokens: &[Token], text: &str) -> &'a Node {
        let index = tokens
            .iter()
            .position(|token| token.text == text)
            .unwrap_or_else(|| panic!("no token {text:?}"));
        tree.ast
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::Phrase(_)) && node.head == Some(index))
            .unwrap_or_else(|| panic!("no phrase headed by {text:?}"))
    }

    #[test]
    fn an_empty_text_has_a_document_root() {
        let (tokens, tree) = analyzed("");
        assert_eq!(tree.ast.nodes.len(), 1);
        assert_eq!(tree.ast.node(tree.ast.root).kind, NodeKind::Document);
        validate(&tokens, &tree.analysis, &tree.ast).expect("empty tree is valid");
    }

    #[test]
    fn a_sentence_has_one_leaf_per_word() {
        let (tokens, tree) = analyzed("Sami söi aamupuuronsa.");
        let words = tokens.iter().filter(|token| token.is_word()).count();
        let leaves = tree
            .ast
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Word)
            .count();
        assert_eq!(leaves, words);
        assert_eq!(clauses(&tree).len(), 1);
        validate(&tokens, &tree.analysis, &tree.ast).expect("ordinary tree is valid");
    }

    #[test]
    fn every_flat_clause_has_exactly_one_node() {
        let (tokens, tree) = analyzed("Sami söi puuron ja joi maidon.");
        assert_eq!(clauses(&tree).len(), tree.analysis.clauses.len());
        for (index, _) in tree.analysis.clauses.iter().enumerate() {
            let found = clauses(&tree)
                .iter()
                .filter(|node| node.clause == Some(index))
                .count();
            assert_eq!(found, 1, "clause {index}");
        }
        validate(&tokens, &tree.analysis, &tree.ast).expect("coordinated tree is valid");
    }

    #[test]
    fn a_relative_host_has_two_segments_and_a_relative_child() {
        let (tokens, tree) = analyzed("Mies, joka tuli, söi.");
        let (host_id, host) = tree
            .ast
            .nodes
            .iter()
            .enumerate()
            .find(|(_, node)| {
                matches!(node.kind, NodeKind::Clause(ClauseKind::Main)) && node.segments.len() == 2
            })
            .map(|(index, node)| (NodeId::new(index), node))
            .expect("a host clause");
        assert_eq!(host.segments, vec![0..1, 6..7]);
        assert_eq!(host.clause, Some(2));
        assert_eq!(host.head, Some(6));
        let relative = clauses(&tree)
            .into_iter()
            .find(|node| node.kind == NodeKind::Clause(ClauseKind::Relative))
            .expect("a relative clause");
        assert_eq!(relative.parent, Some(host_id));
        assert!(
            host.children
                .iter()
                .any(|&child| tree.ast.node(child).kind == NodeKind::Clause(ClauseKind::Relative))
        );
        assert_eq!(relative.antecedent, tree.analysis.clauses[1].antecedent);
        validate(&tokens, &tree.analysis, &tree.ast).expect("host tree is valid");
    }

    #[test]
    fn a_relative_without_a_continuation_stays_flat() {
        let (tokens, tree) = analyzed("Mies, joka tuli.");
        assert!(tree.ast.nodes.iter().all(|node| node.segments.len() <= 1));
        validate(&tokens, &tree.analysis, &tree.ast).expect("flat relative tree is valid");
    }

    #[test]
    fn a_coordinated_continuation_is_not_merged() {
        let (tokens, tree) = analyzed("Mies, joka tuli, söi ja joi.");
        assert!(tree.ast.nodes.iter().all(|node| node.segments.len() <= 1));
        validate(&tokens, &tree.analysis, &tree.ast).expect("coordinated tree is valid");
    }

    #[test]
    fn a_word_leaf_copies_the_flat_role_and_governor() {
        let (tokens, tree) = analyzed("Talo on kaunis.");
        let kaunis = tokens
            .iter()
            .position(|token| token.text == "kaunis")
            .expect("kaunis");
        let leaf = tree
            .ast
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Word && node.tokens.start == kaunis)
            .expect("kaunis leaf");
        assert_eq!(leaf.role, Some(SyntacticRole::Complement));
        assert_eq!(leaf.governor, Some(2));
        assert_eq!(leaf.head, Some(kaunis));
        validate(&tokens, &tree.analysis, &tree.ast).expect("copular tree is valid");
    }

    #[test]
    fn a_nominal_list_owns_its_items_and_conjunction() {
        let (tokens, tree) = analyzed("Ostin omenan, banaanin ja kaksi päärynää.");
        let list = tree
            .ast
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::List { .. }))
            .expect("a list node");
        assert_eq!(list.role, Some(SyntacticRole::Object));
        let items = list
            .children
            .iter()
            .filter(|child| matches!(tree.ast.node(**child).kind, NodeKind::ListItem { .. }))
            .count();
        assert_eq!(items, 3);
        assert!(
            list.children.iter().any(|child| {
                let node = tree.ast.node(*child);
                node.kind == NodeKind::Word && node.tokens.start == 6
            }),
            "the conjunction is a list child"
        );
        validate(&tokens, &tree.analysis, &tree.ast).expect("list tree is valid");
    }

    #[test]
    fn a_colon_list_is_attached_to_its_clause() {
        let (tokens, tree) = analyzed("Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.");
        let list = tree
            .ast
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::List { .. }))
            .expect("a list node");
        assert_eq!(list.clause, Some(1));
        assert_eq!(
            list.children
                .iter()
                .filter(|child| matches!(tree.ast.node(**child).kind, NodeKind::ListItem { .. }))
                .count(),
            3
        );
        validate(&tokens, &tree.analysis, &tree.ast).expect("colon list tree is valid");
    }

    #[test]
    fn a_mixed_or_nested_list_stays_flat() {
        for text in [
            "Punainen, sininen ja vihreä talo on kaunis.",
            "Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.",
        ] {
            let tokens = tokenise(text);
            let tree = analyze(&tokens);
            assert!(
                tree.ast
                    .nodes
                    .iter()
                    .all(|node| !matches!(node.kind, NodeKind::List { .. })),
                "{text}"
            );
            validate(&tokens, &tree.analysis, &tree.ast).expect("flat list tree is valid");
        }
    }

    #[test]
    fn a_parenthetical_is_a_container() {
        let (tokens, tree) =
            analyzed("Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.");
        let parenthetical = tree
            .ast
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Parenthetical)
            .expect("a parenthetical node");
        assert_eq!(parenthetical.tokens, 6..9);
        assert_eq!(parenthetical.children.len(), 2);
        validate(&tokens, &tree.analysis, &tree.ast).expect("parenthetical tree is valid");
    }

    #[test]
    fn a_quote_is_a_container() {
        let (tokens, tree) = analyzed("Hän sanoi: \"Minä tulen.\"");
        let quote = tree
            .ast
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Quote)
            .expect("a quote node");
        assert_eq!(quote.tokens, 4..7);
        assert_eq!(quote.children.len(), 2);
        validate(&tokens, &tree.analysis, &tree.ast).expect("quote tree is valid");
    }

    #[test]
    fn a_bracket_parenthetical_stays_in_one_sentence() {
        let (_, tree) = analyzed("Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.");
        let sentences = tree
            .ast
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Sentence)
            .count();
        assert_eq!(sentences, 1);
    }

    #[test]
    fn a_fronted_object_is_a_constituent() {
        let (_, tree) = analyzed("Kirjan luin eilen.");
        let first = tree.first_constituent(0).expect("a first constituent");
        assert_eq!(first.role, Some(SyntacticRole::Object));
        assert_eq!(first.start, 0);
        let fronted = tree.fronted_constituent(0).expect("a fronted object");
        assert_eq!(fronted.role, Some(SyntacticRole::Object));
        assert_eq!(fronted.start, 0);
    }

    #[test]
    fn an_svo_clause_has_no_fronted_constituent() {
        let (_, tree) = analyzed("Sami söi aamupuuronsa.");
        assert_eq!(
            tree.first_constituent(0)
                .and_then(|constituent| constituent.role),
            Some(SyntacticRole::Subject)
        );
        assert!(tree.fronted_constituent(0).is_none());
    }

    #[test]
    fn an_existential_fronts_its_adverbial() {
        let (_, tree) = analyzed("Pöydällä on kirja.");
        assert_eq!(
            tree.fronted_constituent(0)
                .and_then(|constituent| constituent.role),
            Some(SyntacticRole::Adverbial)
        );
    }

    #[test]
    fn a_wh_question_marks_its_initial_constituent() {
        let (_, tree) = analyzed("Missä hän asuu?");
        let first = tree.first_constituent(0).expect("a first constituent");
        assert!(first.interrogative);
        assert!(tree.fronted_constituent(0).is_none());
    }

    #[test]
    fn a_yes_no_question_marks_the_clitic_host() {
        let (_, tree) = analyzed("Tuleeko hän?");
        let first = tree.first_constituent(0).expect("a first constituent");
        assert!(first.interrogative);
    }

    #[test]
    fn a_host_exposes_one_constituent_order() {
        let (_, tree) = analyzed("Mies, joka tuli, söi.");
        let host = tree.host_constituents(0);
        assert_eq!(host.len(), 3);
        assert_eq!(host[0].kind, NodeKind::Word);
        assert_eq!(host[1].kind, NodeKind::Clause(ClauseKind::Relative));
        assert_eq!(host[2].kind, NodeKind::Phrase(PhraseKind::Verb));
    }

    #[test]
    fn a_list_candidate_does_not_split_an_existing_phrase() {
        let (tokens, tree) = analyzed(
            "Oli ollut ihan hiljaista, sen kun Anni vain kertoi, koko ajan kynttilään katsoen, tasaisella äänellä.",
        );
        let ollut = tokens
            .iter()
            .position(|token| token.text == "ollut")
            .expect("ollut");
        let phrase = phrase_for(&tree, &tokens, "ollut");
        assert_eq!(phrase.tokens, 0..3);
        assert_eq!(phrase.children.len(), 2);
        assert!(phrase.tokens.contains(&ollut));
        validate(&tokens, &tree.analysis, &tree.ast).expect("phrase/list tree is valid");
    }

    #[test]
    fn a_predicate_less_np_gets_a_shape_phrase() {
        let (tokens, tree) = analyzed("Punainen auto.");
        let phrase = phrase_for(&tree, &tokens, "auto");
        assert_eq!(phrase.kind, NodeKind::Phrase(PhraseKind::Noun));
        assert_eq!(phrase.role, Some(SyntacticRole::Unknown));
        assert_eq!(phrase.confidence, RoleConfidence::Weak);
        assert_eq!(phrase.children.len(), 2);
        validate(&tokens, &tree.analysis, &tree.ast).expect("fragment tree is valid");
    }

    #[test]
    fn walk_visits_every_node() {
        let (_, tree) = analyzed("Pöydällä on kirja.");
        let walked = tree.ast.walk();
        assert_eq!(walked.len(), tree.ast.nodes.len());
        assert_eq!(walked.first(), Some(&tree.ast.root));
    }

    #[test]
    fn a_simple_clause_has_subject_predicate_object_phrases() {
        let (tokens, tree) = analyzed("Sami söi aamupuuronsa.");
        let kinds: Vec<PhraseKind> = tree
            .ast
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::Phrase(kind) => Some(kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![PhraseKind::Noun, PhraseKind::Verb, PhraseKind::Noun]
        );
        assert_eq!(
            phrase_for(&tree, &tokens, "Sami").role,
            Some(SyntacticRole::Subject)
        );
        assert_eq!(
            phrase_for(&tree, &tokens, "söi").role,
            Some(SyntacticRole::Predicate)
        );
        assert_eq!(
            phrase_for(&tree, &tokens, "aamupuuronsa").role,
            Some(SyntacticRole::Object)
        );
        validate(&tokens, &tree.analysis, &tree.ast).expect("phrase tree is valid");
    }

    #[test]
    fn a_modifier_chain_stays_in_one_phrase() {
        let (tokens, tree) = analyzed("Matin vanha auto on punainen.");
        let phrase = phrase_for(&tree, &tokens, "auto");
        assert_eq!(phrase.kind, NodeKind::Phrase(PhraseKind::Noun));
        assert_eq!(phrase.children.len(), 3);
        let surfaces: Vec<&str> = phrase
            .children
            .iter()
            .map(|child| tokens[tree.ast.node(*child).tokens.start].text.as_str())
            .collect();
        assert_eq!(surfaces, vec!["Matin", "vanha", "auto"]);
        validate(&tokens, &tree.analysis, &tree.ast).expect("modifier tree is valid");
    }

    #[test]
    fn a_numeral_phrase_is_typed() {
        let (tokens, tree) = analyzed("Kaksi miestä tuli.");
        let phrase = phrase_for(&tree, &tokens, "Kaksi");
        assert_eq!(phrase.kind, NodeKind::Phrase(PhraseKind::Numeral));
        assert_eq!(phrase.children.len(), 2);
        validate(&tokens, &tree.analysis, &tree.ast).expect("numeral tree is valid");
    }

    #[test]
    fn an_adposition_phrase_owns_its_complement() {
        let postposition_tokens = tokenise("Pöydän alla on kissa.");
        let postposition_tree = analyze(&postposition_tokens);
        let postposition = phrase_for(&postposition_tree, &postposition_tokens, "alla");
        assert_eq!(
            postposition.kind,
            NodeKind::Phrase(PhraseKind::Postposition)
        );
        assert_eq!(postposition.children.len(), 2);

        let preposition_tokens = tokenise("Ilman rahaa en tule.");
        let preposition_tree = analyze(&preposition_tokens);
        let preposition = phrase_for(&preposition_tree, &preposition_tokens, "Ilman");
        assert_eq!(preposition.kind, NodeKind::Phrase(PhraseKind::Preposition));
        assert_eq!(preposition.children.len(), 2);
    }

    #[test]
    fn a_fronted_object_is_its_own_phrase() {
        let (tokens, tree) = analyzed("Kirjan luin eilen.");
        let object = phrase_for(&tree, &tokens, "Kirjan");
        assert_eq!(object.role, Some(SyntacticRole::Object));
        assert_eq!(object.kind, NodeKind::Phrase(PhraseKind::Noun));
        let verb = phrase_for(&tree, &tokens, "luin");
        assert_eq!(verb.role, Some(SyntacticRole::Predicate));
        validate(&tokens, &tree.analysis, &tree.ast).expect("fronting tree is valid");
    }

    #[test]
    fn validate_rejects_a_detached_node() {
        let (tokens, mut tree) = analyzed("Sami söi aamupuuronsa.");
        let root = tree.ast.root;
        tree.ast.node_mut(root).children.clear();
        assert!(validate(&tokens, &tree.analysis, &tree.ast).is_err());
    }
}
