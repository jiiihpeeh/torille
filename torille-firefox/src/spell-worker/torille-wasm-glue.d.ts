// Types for the wasm-bindgen glue module (`torille_wasm_bg.js`).
//
// wasm-pack regenerates `torille-wasm/pkg/` on every build and ships
// no `.d.ts` for the glue file itself, so we declare the surface the
// extension uses here. `wasm-loader.ts` imports the namespace and
// hands the whole module to `WebAssembly.instantiate` as the wasm's
// import object; the individual functions are called through the
// `bg` re-export in `spell-core.ts`.
//
// Keep these signatures in sync with `torille-wasm/src/lib.rs`.

declare module '*/torille_wasm_bg.js' {
  export function check(word: string): boolean;
  export function suggest_words(word: string, max: number): string[];
  /** Permitted Finnish line breaks; see docs/hyphenation.md. */
  export function hyphenate(
    word: string,
    left_min: number,
    right_min: number,
    compound_breaks: boolean,
  ): {
    byteOffset: number;
    charOffset: number;
    compound: boolean;
    source: 'pattern' | 'compound' | 'exception';
    penalty: number;
  }[];

  export function create_text_area_state(): number;
  export function destroy_text_area_state(idx: number): void;
  export function clear_state(idx: number): void;
  export function update_state_from_text(idx: number, text: string): void;
  /** First-pass misspells: byte ranges only, no suggestions. */
  export function analyze_misspells(
    idx: number,
    text: string,
  ): { word: string; start: number; end: number }[];
  /** Bounded suggestion pass for one misspelling (the click path). */
  export function suggest_for_misspell(
    idx: number,
    word: string,
    max_n: number,
    max_distance: number,
  ): string[];
  export function analyze_text(
    idx: number,
    text: string,
    max_suggestions: number,
  ): {
    word: string;
    start: number;
    end: number;
    suggestions: string[];
  }[];
  export function suggest_words_with_state(
    idx: number,
    word: string,
    max_n: number,
  ): string[];

  /** Pre-warm the lemma set / catalogue at boot. */
  export function warm_caches(): void;

  /** Attach a manually instantiated instance to the glue. */
  export function __wbg_set_wasm(exports: WebAssembly.Exports): void;
}
