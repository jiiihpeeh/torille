// ─────────────────────────────────────────────────────────────────────
// §6b  Grammar panel
// ─────────────────────────────────────────────────────────────────────

// The grammar check is opt-in through the context menu and runs on
// one input at a time. It is not on the keystroke path.

export function syntaxKindLabel(kind: string): string {
  switch (kind) {
    case 'subject_predicate_agreement': return 'subjektin ja predikaatin kongruenssi';
    case 'attribute_agreement': return 'attribuutin kongruenssi';
    case 'verb_rection': return 'verbien rektio';
    case 'case_selection': return 'sijan valinta';
    case 'object_case': return 'objektin sija';
    case 'missing_argument': return 'puuttuva argumentti';
    case 'extra_argument': return 'ylimääräinen argumentti';
    case 'clause_structure': return 'lauserakenne';
    case 'word_order': return 'sanajärjestys';
    case 'comma_missing': return 'puuttuva pilkku';
    case 'comma_extra': return 'ylimääräinen pilkku';
    case 'comma_pair_unbalanced': return 'pilkupari on epätasapainossa';
    case 'question_mark_missing': return 'puuttuva kysymysmerkki';
    case 'unbalanced_quote': return 'epätasapainoinen lainausmerkki';
    case 'list_separator_missing': return 'puuttuva luettelopilkku';
    case 'list_separator_duplicate': return 'kaksinkertainen luettelopilkku';
    case 'list_comma_before_conjunction': return 'pilkku ennen konjunktiota';
    case 'list_introducer_comma': return 'pilkku kaksoispisteen jälkeen';
    default: return kind;
  }
}

function styleKindLabel(kind: string): string {
  switch (kind) {
    case 'style_repetition': return 'toistuva sana';
    case 'style_long_sentence': return 'pitkä virke';
    case 'style_cliche': return 'kulunut ilmaus';
    case 'style_filler': return 'täytesana';
    case 'style_wordy': return 'monisanainen ilmaus';
    case 'style_colloquial': return 'arkikielinen ilmaus';
    default: return kind;
  }
}

export function syntaxSeverityLabel(severity: string): string {
  if (severity === 'error') return 'virhe';
  if (severity === 'warning') return 'varoitus';
  return 'ehdotus';
}

export function syntaxSeverityColor(severity: string): string {
  if (severity === 'error') return '#c33';
  if (severity === 'warning') return '#d80';
  return '#36c';
}

export function syntaxCorrectionLabel(correction: R_SyntaxCorrection): string {
  const first = correction.edits[0];
  if (!first) return 'korjaa';
  return first.replacement === '' ? 'poista' : first.replacement;
}

export function applySyntaxEdits(text: string, edits: R_SyntaxEdit[]): string {
  const ordered = [...edits].sort(
    (a, b) => (b.start - a.start) || (b.end - a.end));
  let out = text;
  for (const edit of ordered) {
    if (edit.start < 0 || edit.end > out.length || edit.start > edit.end) {
      continue;
    }
    out = out.slice(0, edit.start) + edit.replacement + out.slice(edit.end);
  }
  return out;
}

export function editorText(input: HTMLElement): string {
  if (input instanceof HTMLTextAreaElement
      || input instanceof HTMLInputElement) {
    return input.value;
  }
  return input.textContent ?? '';
}

interface PanelLabels {
  title: string;
  empty: string;
  kindLabel: (kind: string) => string;
}

export function showGrammarPanel(
  input: HTMLElement,
  diagnostics: R_SyntaxDiagnostic[],
  apply: (correction: R_SyntaxCorrection) => boolean,
): void {
  showPanel(input, diagnostics, apply, {
    title: 'Kielioppi',
    empty: 'Ei huomautuksia.',
    kindLabel: syntaxKindLabel,
  });
}

export function showStylePanel(
  input: HTMLElement,
  diagnostics: R_SyntaxDiagnostic[],
  apply: (correction: R_SyntaxCorrection) => boolean,
): void {
  showPanel(input, diagnostics, apply, {
    title: 'Tyyli',
    empty: 'Ei tyylihuomautuksia.',
    kindLabel: styleKindLabel,
  });
}

function showPanel(
  input: HTMLElement,
  diagnostics: R_SyntaxDiagnostic[],
  apply: (correction: R_SyntaxCorrection) => boolean,
  labels: PanelLabels,
): void {
  document.querySelector('.torille-grammar-panel')?.remove();
  const panel = document.createElement('div');
  panel.className = 'torille-grammar-panel';
  panel.style.cssText = [
    'position:fixed',
    'z-index:2147483647',
    'box-sizing:border-box',
    'max-height:60vh',
    'overflow:auto',
    'background:#fff',
    'color:#111',
    'border:1px solid #888',
    'border-radius:6px',
    'box-shadow:0 4px 18px rgba(0,0,0,.25)',
    'padding:10px',
    'font:13px/1.4 system-ui,sans-serif',
  ].join(';');

  const width = Math.max(240, Math.min(440, window.innerWidth - 16));
  const rect = input.getBoundingClientRect();
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
  const below = rect.bottom + 6;
  const top = below + 220 < window.innerHeight
    ? below
    : Math.max(8, rect.top - 226);
  panel.style.width = `${width}px`;
  panel.style.left = `${left}px`;
  panel.style.top = `${top}px`;

  const header = document.createElement('div');
  header.style.cssText =
    'display:flex;justify-content:space-between;align-items:center;gap:8px;margin-bottom:6px;';
  const title = document.createElement('strong');
  title.textContent = labels.title;
  const close = document.createElement('button');
  close.type = 'button';
  close.textContent = 'sulje';
  close.style.cssText = 'font:inherit;padding:0.15em 0.5em;cursor:pointer;';
  close.addEventListener('click', () => panel.remove());
  header.append(title, close);
  panel.append(header);

  if (diagnostics.length === 0) {
    const empty = document.createElement('p');
    empty.textContent = labels.empty;
    empty.style.margin = '0.3em 0';
    panel.append(empty);
  } else {
    const list = document.createElement('ul');
    list.style.cssText = 'list-style:none;margin:0;padding:0;';
    for (const diagnostic of diagnostics) {
      const item = document.createElement('li');
      item.style.cssText =
        'border-left:3px solid ' + syntaxSeverityColor(diagnostic.severity)
        + ';padding:4px 6px;margin:6px 0;background:#fafafa;';
      const head = document.createElement('div');
      const label = document.createElement('strong');
      label.textContent = labels.kindLabel(diagnostic.kind);
      const severity = document.createElement('span');
      severity.textContent = ' ' + syntaxSeverityLabel(diagnostic.severity);
      severity.style.color = '#666';
      head.append(label, severity);
      item.append(head);
      if (diagnostic.expected) {
        const expected = document.createElement('div');
        expected.textContent = 'odotettu: ' + diagnostic.expected;
        expected.style.cssText = 'color:#555;font-size:0.9em;margin-top:2px;';
        item.append(expected);
      }
      if (diagnostic.corrections.length > 0) {
        const actions = document.createElement('div');
        actions.style.cssText = 'margin-top:4px;';
        for (const correction of diagnostic.corrections) {
          const button = document.createElement('button');
          button.type = 'button';
          button.textContent = syntaxCorrectionLabel(correction);
          button.style.cssText =
            'font:inherit;padding:0.15em 0.5em;margin:0 4px 0 0;cursor:pointer;';
          button.addEventListener('click', () => {
            if (apply(correction)) panel.remove();
          });
          actions.append(button);
        }
        item.append(actions);
      }
      list.append(item);
    }
    panel.append(list);
  }
  document.body.append(panel);
}
