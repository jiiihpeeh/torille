# TODO: skip code blocks and BBCode in the browser extensions

Status: **implemented (phases 1–4 + e2e)** · created: 2026-11-10 · updated: 2026-11-10

The extension should underline Finnish prose only. Today it checks every word
in an attached textarea/contenteditable, including code fences, HTML tags,
BBCode markup and code payloads. This plan adds a shared ignore-layer and
applies it to both the textarea (shadow mirror) and contenteditable (CSS
Highlight) paths.

## Ground rules

- [x] No wasm/engine change; the filtering layer lives in the content script.
- [x] Keep the two extension copies in sync (`extensions/chromium` and
      `extensions/firefox`).
- [x] Mask **before** `analyze-text`/`update-state`, not only filter hits after
      the call. Code must not affect sentence context, proper-noun learning or
      suggestion ranking.
- [x] Mask by UTF-16 code units, preserving string length; never by codepoints.
- [x] Keep the post-filter as a defensive check.
- [x] Code blocks are still rendered in the textarea mirror; they are simply
      never marked as misspellings.

## Phase 1 — shared ignore-range parser

- [x] Add `src/content/ignore-ranges.ts` (shared by both extensions):
  - `parseBbcode(text, options) -> Range[]`
  - `parseFencedCode(text) -> Range[]`
  - `parseInlineCode(text) -> Range[]`
  - `parseHtmlCode(text) -> Range[]`
  - `computeIgnoredRanges(text, options) -> Range[]` — merge/sort/coalesce
  - `isIgnored(range, ignored) -> boolean` — binary search/two-pointer
  - `isInsideCodeDom(node, editorRoot) -> boolean`
- [x] Define a small `Range` type (`start`, `end`, JS string indices).
- [x] Add `maskIgnored(text, ignored) -> string`:
  - replace ignored code units with spaces,
  - preserve `\n`/`\r` so sentences/paragraphs survive,
  - preserve length exactly (mask `end - start` code units).
- [x] Unit tests for parser and mask:
  - [x] `[code]xyzzy[/code]` ignored; `[quote]talo[/quote]` prose not ignored
  - [x] `[code=php]`, mixed case, nested quote/code
  - [x] unclosed `[code]` behavior pinned
  - [x] `[url]…[/url]`, `[img]…[/img]`, `[email]…[/email]`, `[media]…[/media]`
  - [x] fenced ```` ``` ```` and `~~~`, language tags, longer closing fence
  - [x] unclosed fence behavior pinned
  - [x] inline backticks; unmatched backticks and apostrophes
  - [x] raw `<code>/<pre>/<kbd>/<samp>/<script>/<style>` in text
  - [x] HTML tag syntax masked; `<a>` link text not masked
  - [x] `ä`, `ö`, emoji, combining marks and `\r\n` keep exact offsets
  - [x] adjacent/overlapping ranges coalesce

## Phase 2 — textarea path

- [x] In `createTextareaOverlay`, after reading `text`:
  - [x] compute `ignored = computeIgnoredRanges(text, uiSettings)`
  - [x] build `analyzed = maskIgnored(text, ignored)`
  - [x] send `analyzed` to `analyzeTextAsync` and `update-state`
  - [x] keep `text` for rendering and `text.slice(hit.start, hit.end)`
- [x] Filter hits whose `[start, end)` intersects an ignored range, as a
      defensive check.
- [x] Keep filtered `lastHits` so the input hit-test and popup cannot open for
      ignored text.
- [x] Existing input `mousedown` hit-test uses the filtered spans.
- [x] BBCode details:
  - [x] mask **all** tag syntax, including attributes (`[color=red]`)
  - [x] code-like payload ignored: `code`, `icode`, `php`, `html`, `sql`,
        `xml`
  - [x] non-prose payload ignored: `url`, `img`, `email`, `media`
  - [x] prose tags (`quote`, `b`, `i`, `u`, `list`, …) keep their content
  - [x] tags are case-insensitive; closing tag matches the opening tag
  - [x] unclosed code ignores to end (documented)
- [x] Markdown:
  - [x] fenced blocks (``` and ~~~) with optional language tag
  - [x] closing fence length >= opening length
  - [x] inline code only when paired on the same line
  - [x] do not treat indented 4-space blocks as code initially
- [x] HTML in plain text:
  - [x] mask tag syntax broadly
  - [x] mask contents of `code`, `pre`, `kbd`, `samp`, `script`, `style`
  - [x] keep anchor text (`<a>…</a>`) checkable

## Phase 3 — contenteditable path

- [x] `renderMisspellHighlights()`:
  - [x] compute ignored ranges from `el.textContent`
  - [x] send `maskIgnored(...)` to the worker
  - [x] skip hits whose start/end node has a `CODE`, `PRE`, `KBD`, `SAMP`,
        `SCRIPT` or `STYLE` ancestor inside the editor
  - [x] skip hits intersecting the text-range mask (raw BBCode/Markdown)
- [x] `checkCurrentWord()`:
  - [x] if the caret’s text node is inside a code ancestor, close and return
  - [x] if the caret offset is inside an ignored range, close and return
- [x] `onEditorClick()`:
  - [x] skip misspell regions inside code ancestors/ignored ranges
- [x] Whole-field code editors:
  - [x] opt-in detection for `.cm-editor`, `.monaco-editor`, `.CodeMirror`,
        `[data-code-editor]`; default off
  - [x] do not place a mirror over a hidden CodeMirror textarea by accident

## Phase 4 — settings

Add to the Solid options page (`src/options/index.tsx`, both extensions):

- [x] `torille-ignore-code-blocks` — default `true`
- [x] `torille-ignore-bbcode` — default `true`
- [x] `torille-ignore-inline-code` — default `true` (or fold into code-blocks)
- [x] `torille-ignore-code-editors` — default `false`

For each:

- [x] add to `Settings`, `DEFAULTS`, `KEYS`, `UI_SETTINGS` in the content script
- [x] add a checkbox under **Tarkastus**
- [x] include in **Palauta oletusasetukset**
- [x] apply live through `storage.onChanged` and `overlay.refresh()`

## Phase 5 — sync and tests

- [x] Keep Chromium and Firefox copies identical:
  - [x] add a check that the two `content.ts` copies match, or
  - [x] generate one shared copy at build time
- [x] E2E textarea fixture (`e2e/code-blocks.html`):
  - [x] prose + fenced code + BBCode + inline code
  - [x] code words not `.misspell`
  - [x] prose misspellings still present
  - [x] clicking code does not open the popup
  - [x] prose immediately before/after code still checked
- [x] E2E contenteditable fixture:
  - [x] `<pre><code>xyzzy</code></pre>` gets no highlight
  - [x] prose siblings still get highlights
  - [x] caret in code shows no popup
- [x] Settings tests:
  - [x] turning `torille-ignore-code-blocks` off restores checking in code
  - [x] reset returns all four toggles to defaults
- [x] Run both extension typechecks and builds:
  - [x] `npm run typecheck`
  - [x] `node build.mjs`

## Edge cases to pin

- [x] unclosed code fence/BBCode
- [x] nested `[code]` inside `[quote]`
- [x] `[code]` inside a Markdown fence (union mask)
- [x] code block at start/end of the text
- [x] inline code adjacent to prose
- [x] escaped-looking BBCode (`\[code]`)
- [x] CRLF and non-BMP characters
- [x] code text that looks like a sentence (capitalised, punctuation)
- [x] native spellcheck remains disabled for the field

## Decisions still open

- [x] Should unclosed BBCode `[code]` ignore to end, or only the tag?
      Recommendation: ignore to end for code; mask only the tag for other
      BBCode.
- [x] Should inline code be a separate setting or part of code blocks?
- [x] Should whole code-editor fields be skipped by default? Recommendation:
      keep opt-in until the field-detection rules are proven.
- [ ] Should the context-menu “check selection” path also skip code? The
      background has no DOM context; decide whether to document the limitation
      or pass a skip flag from the content script.

## Success criteria

- [x] No underline or popup anywhere inside code blocks, Markdown fences,
      inline code, HTML code/pre, BBCode code payloads, or URL/email payloads.
- [x] Prose outside those regions behaves exactly as before.
- [x] Settings save, apply live, reset correctly, and are documented.
- [x] Chromium and Firefox builds match.
- [x] Full extension typecheck/build and e2e fixtures pass.
