// Overlay factory. Picks the textarea/input shadow mirror or the
// contenteditable CSS-Highlight path and returns one small API the scanner
// and the per-textbox toggle use.

import type { DescribeInput, DebugFn } from '../shared';
import type { WorkerClient } from '../worker-client';
import { createContentEditableOverlay } from './contenteditable';
import { createTextareaOverlay } from './textarea';

export interface OverlayOptions {
  el: HTMLElement;
  workerClient: WorkerClient;
  dbg: DebugFn;
  describeInput: DescribeInput;
}

export interface Overlay {
  host: HTMLElement;
  overlay: HTMLElement;
  workerClient: WorkerClient;
  applyTextEdits: (edits: R_SyntaxEdit[]) => boolean;
  initialRender: () => void;
  refresh: () => void;
  setVisible: (visible: boolean) => void;
  setDebug: (on: boolean) => void;
  destroy: () => void;
}

// Build the overlay for one text input, plus the suggestion popup, the
// debounced render, and the popup's open / close / click-outside plumbing.
// The factory returns a small API the boot section wires into the scanner and
// the toggle.
export function createOverlay(opts: OverlayOptions): Overlay {
  if (opts.el.isContentEditable) {
    return createContentEditableOverlay(opts);
  }
  return createTextareaOverlay(opts);
}
