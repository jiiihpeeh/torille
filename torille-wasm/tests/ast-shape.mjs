// The AST wasm export smoke test.
//
// Run from `torille-wasm/` after `wasm-pack build --release`:
//
//   node --experimental-wasm-modules tests/ast-shape.mjs
//
// The check is deliberately shape-level: the Rust engine tests cover the
// tree itself, and this probe catches a missing/misnamed wasm field or a
// broken `{ syntax, ast }` envelope.

import assert from 'node:assert/strict';
import {
  analyze_ast,
  analyze_ast_with_state,
  create_text_area_state,
  destroy_text_area_state,
  update_state_from_text,
} from '../pkg/torille_wasm.js';

const text = 'Kirjan luin eilen.';
const result = analyze_ast(text);

assert.ok(result.syntax, 'the flat syntax object is present');
assert.ok(result.ast, 'the AST object is present');
assert.equal(typeof result.ast.root, 'number');
assert.ok(result.ast.nodes.length > 0, 'the node array is not empty');
for (const node of result.ast.nodes) {
  assert.equal(typeof node.id, 'number');
  assert.equal(typeof node.kind, 'string');
  if (!Array.isArray(node.span) || node.span.length !== 2) {
    assert.fail(`node ${node.id} has no span pair`);
  }
  if (!Array.isArray(node.tokens) || node.tokens.length !== 2) {
    assert.fail(`node ${node.id} has no token pair`);
  }
  if (!Array.isArray(node.segments)) {
    assert.fail(`node ${node.id} has no segments`);
  }
}

const clause = result.ast.clauses[0];
assert.equal(clause.index, 0);
assert.equal(clause.first, 'object');
assert.equal(clause.fronted, 'object');
assert.deepEqual(
  clause.constituents.map((constituent) => constituent.role),
  ['object', 'predicate', 'adverbial'],
);

const state = create_text_area_state();
update_state_from_text(state, text);
const warm = analyze_ast_with_state(state, text);
assert.equal(warm.ast.clauses[0].fronted, 'object');
destroy_text_area_state(state);

console.log('ast probe ok');
