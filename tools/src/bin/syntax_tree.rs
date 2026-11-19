// SPDX-License-Identifier: MIT OR Apache-2.0
//! `syntax_tree` — print the typed surface tree over `torille::syntax`.
//!
//! Usage:
//!   syntax_tree "Sami söi aamupuuronsa."
//!   syntax_tree --json "Mies, joka tuli, söi."
//!   syntax_tree --check "..."
//!
//! The tree is derived from the flat `SyntaxAnalysis`; this binary never
//! changes the analysis. `--check` runs the tree invariants and exits non-zero
//! on a violation, which is what the phase-0 regression gate uses.

use std::process::ExitCode;

use serde_json::{Value, json};
use torille::syntax::RoleConfidence;
use torille::syntax::ast::{self, Ast, Constituent, Node, NodeId, NodeKind};
use torille::tokenise::{Token, tokenise};

fn main() -> ExitCode {
    let mut as_json = false;
    let mut check = false;
    let mut order = false;
    let mut parts = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => as_json = true,
            "--check" => check = true,
            "--order" => order = true,
            _ => parts.push(arg),
        }
    }
    if parts.is_empty() {
        eprintln!("usage: syntax_tree [--json] [--check] [--order] \"TEXT\"");
        return ExitCode::from(2);
    }

    let text = parts.join(" ");
    let tokens = tokenise(&text);
    let tree = ast::analyze(&tokens);

    if check {
        match ast::validate(&tokens, &tree.analysis, &tree.ast) {
            Ok(()) => {
                if !as_json {
                    println!("ok: {} nodes", tree.ast.nodes.len());
                }
            }
            Err(violations) => {
                for violation in violations {
                    let node = violation
                        .node
                        .map(|id| format!("node {}: ", id.index()))
                        .unwrap_or_default();
                    eprintln!("{}: {node}{}", violation.rule, violation.detail);
                }
                return ExitCode::from(1);
            }
        }
    }

    if as_json && order {
        let clauses: Vec<Value> = (0..tree.analysis.clauses.len())
            .map(|index| order_json(&tree, index))
            .collect();
        let hosts = hosts_json(&tree);
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"clauses": clauses, "hosts": hosts}))
                .expect("the order is serialisable")
        );
    } else if order {
        print_order(&tree);
    } else if as_json {
        let nodes: Vec<Value> = tree
            .ast
            .walk()
            .into_iter()
            .map(|id| node_json(&tree.ast, id))
            .collect();
        let root = tree.ast.root.index();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"root": root, "nodes": nodes}))
                .expect("the tree is serialisable")
        );
    } else if !check {
        dump(&tree.ast, &tokens, &text, tree.ast.root, 0);
    }

    ExitCode::SUCCESS
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

fn print_order(tree: &ast::SyntaxTree) {
    let mut seen_hosts = std::collections::BTreeSet::new();
    for index in 0..tree.analysis.clauses.len() {
        let constituents = tree.clause_constituents(index);
        let sequence: Vec<String> = constituents
            .iter()
            .map(|constituent| {
                format!(
                    "{}[{}..{}]",
                    constituent_label(constituent),
                    constituent.tokens.start,
                    constituent.tokens.end
                )
            })
            .collect();
        let first = tree
            .first_constituent(index)
            .as_ref()
            .map(constituent_label)
            .unwrap_or_else(|| "-".to_string());
        let fronted = tree
            .fronted_constituent(index)
            .as_ref()
            .map(constituent_label)
            .unwrap_or_else(|| "-".to_string());
        println!("clause {index}: {}", sequence.join(" "));
        println!("  first={first} fronted={fronted}");
        if let Some(host) = tree.host_node(index)
            && seen_hosts.insert(host.index())
        {
            let host_sequence: Vec<String> = tree
                .host_constituents(index)
                .iter()
                .map(|constituent| {
                    format!(
                        "{}[{}..{}]",
                        constituent_label(constituent),
                        constituent.tokens.start,
                        constituent.tokens.end
                    )
                })
                .collect();
            println!("  host: {}", host_sequence.join(" "));
        }
    }
}

fn constituent_json(constituent: &Constituent) -> Value {
    json!({
        "node": constituent.node.index(),
        "kind": constituent.kind.name(),
        "role": constituent.role.map(|role| role.name()),
        "tokens": [constituent.tokens.start, constituent.tokens.end],
        "start": constituent.start,
        "end": constituent.end,
        "interrogative": constituent.interrogative,
    })
}

fn order_json(tree: &ast::SyntaxTree, index: usize) -> Value {
    let constituents: Vec<Value> = tree
        .clause_constituents(index)
        .iter()
        .map(constituent_json)
        .collect();
    let first = tree
        .first_constituent(index)
        .as_ref()
        .map(constituent_label);
    let fronted = tree
        .fronted_constituent(index)
        .as_ref()
        .map(constituent_label);
    json!({
        "clause": index,
        "constituents": constituents,
        "first": first,
        "fronted": fronted,
    })
}

fn hosts_json(tree: &ast::SyntaxTree) -> Vec<Value> {
    let mut hosts = Vec::new();
    for (index, node) in tree.ast.nodes.iter().enumerate() {
        if !matches!(node.kind, NodeKind::Clause(_)) || node.segments.len() < 2 {
            continue;
        }
        let Some(clause) = node.clause else {
            continue;
        };
        let constituents: Vec<Value> = tree
            .host_constituents(clause)
            .iter()
            .map(constituent_json)
            .collect();
        let segments: Vec<Value> = node
            .segments
            .iter()
            .map(|segment| json!([segment.start, segment.end]))
            .collect();
        hosts.push(json!({
            "node": index,
            "clause": node.clause,
            "segments": segments,
            "constituents": constituents,
        }));
    }
    hosts
}

fn label(node: &Node) -> String {
    match node.kind {
        NodeKind::Document => "Document".to_string(),
        NodeKind::Sentence => "Sentence".to_string(),
        NodeKind::Clause(kind) => format!("Clause({})", kind.name()),
        NodeKind::Phrase(kind) => format!("Phrase({})", kind.name()),
        NodeKind::List { index } => format!("List({index})"),
        NodeKind::ListItem { list, item } => format!("ListItem({list}.{item})"),
        NodeKind::Coordination(kind) => format!("Coordination({})", kind.name()),
        NodeKind::Parenthetical => "Parenthetical".to_string(),
        NodeKind::Quote => "Quote".to_string(),
        NodeKind::Word => "Word".to_string(),
    }
}

fn dump(ast: &Ast, tokens: &[Token], text: &str, id: NodeId, depth: usize) {
    let node = ast.node(id);
    let mut line = format!(
        "{}{}[{}..{}]",
        "  ".repeat(depth),
        label(node),
        node.span.start,
        node.span.end
    );
    if node.tokens.start < node.tokens.end {
        line.push_str(&format!(
            " tokens={}..{}",
            node.tokens.start, node.tokens.end
        ));
    }
    if node.segments.len() > 1 {
        line.push_str(&format!(" segs={:?}", node.segments));
    }
    if let Some(head) = node.head.and_then(|index| tokens.get(index)) {
        line.push_str(&format!(" head={}", head.text));
    }
    if let Some(governor) = node.governor.and_then(|index| tokens.get(index)) {
        line.push_str(&format!(" gov={}", governor.text));
    }
    if let Some(role) = node.role {
        line.push_str(&format!(" role={}", role.name()));
    }
    if let Some(antecedent) = node.antecedent.and_then(|index| tokens.get(index)) {
        line.push_str(&format!(" antecedent={}", antecedent.text));
    }
    line.push_str(match node.confidence {
        RoleConfidence::Likely => " conf=likely",
        RoleConfidence::Weak => " conf=weak",
    });
    if let Some(clause) = node.clause {
        line.push_str(&format!(" clause={clause}"));
    }
    if node.kind == NodeKind::Word
        && let Some(surface) = text.get(node.span.clone())
    {
        line.push_str(&format!(" {surface:?}"));
    }
    println!("{line}");
    for &child in &node.children {
        dump(ast, tokens, text, child, depth + 1);
    }
}

fn node_json(ast: &Ast, id: NodeId) -> Value {
    let node = ast.node(id);
    let detail = match node.kind {
        NodeKind::Clause(kind) => Some(kind.name().to_string()),
        NodeKind::Phrase(kind) => Some(kind.name().to_string()),
        NodeKind::Coordination(kind) => Some(kind.name().to_string()),
        NodeKind::List { index } => Some(index.to_string()),
        NodeKind::ListItem { list, item } => Some(format!("{list}.{item}")),
        _ => None,
    };
    let confidence = match node.confidence {
        RoleConfidence::Likely => "likely",
        RoleConfidence::Weak => "weak",
    };
    json!({
        "id": id.index(),
        "kind": node.kind.name(),
        "detail": detail,
        "span": [node.span.start, node.span.end],
        "tokens": [node.tokens.start, node.tokens.end],
        "segments": node.segments.iter().map(|range| json!([range.start, range.end])).collect::<Vec<_>>(),
        "parent": node.parent.map(NodeId::index),
        "children": node.children.iter().map(|child| child.index()).collect::<Vec<_>>(),
        "head": node.head,
        "governor": node.governor,
        "role": node.role.map(|role| role.name()),
        "confidence": confidence,
        "antecedent": node.antecedent,
        "clause": node.clause,
    })
}
