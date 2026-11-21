//! Feature-gated counters for the cold-morphology probe path.
//!
//! The shipping build compiles the no-op version, so the call sites inline
//! away. `torille-tools` enables the `probe-stats` feature and
//! `perf_grammar_style --probe-stats` turns recording on for one cold pass.
//!
//! Counters are thread-local. The measured shape is the single-thread cold
//! pass; the four-worker path records per worker and is not aggregated here.
//! `forms` is the number of forms visited by a build, including the derived
//! layers; `by_class[..].forms` counts the class paradigms only.

#![cfg_attr(not(feature = "probe-stats"), allow(dead_code))]

/// The class bucket for a build with no class (a classless compound).
pub const OTHER_CLASS: usize = 255;

/// Builds, failures and visited forms for one class or caller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassStats {
    pub builds: u64,
    pub failed: u64,
    pub forms: u64,
}

/// Which probe asked for the build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    /// `collect_bases_via_stem_map`.
    NounStem,
    /// `collect_bases_via_verb_stem_map`.
    VerbStem,
    /// `form_readings` (a direct lemma or a reading extraction).
    Readings,
    /// `emits_standalone_form`, the lookup/compound helper on the cheap gate.
    CheapGate,
    /// Everything else (compound heads, mixed-base helpers, ...).
    Other,
}

impl Caller {
    pub const ALL: [Caller; 5] = [
        Caller::NounStem,
        Caller::VerbStem,
        Caller::Readings,
        Caller::CheapGate,
        Caller::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Caller::NounStem => "noun stem",
            Caller::VerbStem => "verb stem",
            Caller::Readings => "readings",
            Caller::CheapGate => "cheap gate",
            Caller::Other => "other",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Caller::NounStem => 0,
            Caller::VerbStem => 1,
            Caller::Readings => 2,
            Caller::CheapGate => 3,
            Caller::Other => 4,
        }
    }
}

/// One cold pass, as recorded by the calling thread.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub builds: u64,
    pub failed: u64,
    pub forms: u64,
    pub by_class: [ClassStats; 256],
    pub by_caller: [ClassStats; 5],
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            builds: 0,
            failed: 0,
            forms: 0,
            by_class: [ClassStats::default(); 256],
            by_caller: [ClassStats::default(); 5],
        }
    }
}

impl Snapshot {
    /// The class buckets with a recorded build or form, sorted by builds.
    pub fn classes(&self) -> Vec<(usize, ClassStats)> {
        let mut rows: Vec<(usize, ClassStats)> = self
            .by_class
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, stats)| stats.builds > 0 || stats.forms > 0)
            .collect();
        rows.sort_by(|a, b| b.1.builds.cmp(&a.1.builds).then(a.0.cmp(&b.0)));
        rows
    }
}

#[cfg(feature = "probe-stats")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{ClassStats, Caller, OTHER_CLASS, Snapshot};
    use crate::kotus::Inflection;

    thread_local! {
        static STATS: RefCell<Snapshot> = RefCell::new(Snapshot::default());
        /// Nesting depth of `emits_form`/`form_readings` builds on this thread.
        /// `note_forms` records only inside a build.
        static DEPTH: Cell<u32> = const { Cell::new(0) };
    }

    static ENABLED: AtomicBool = AtomicBool::new(false);

    /// Start recording on this run. The flag is process-wide so a worker can
    /// read it; the counters themselves stay thread-local.
    pub fn enable() {
        ENABLED.store(true, Ordering::Relaxed);
    }

    pub fn disable() {
        ENABLED.store(false, Ordering::Relaxed);
    }

    pub fn is_enabled() -> bool {
        ENABLED.load(Ordering::Relaxed)
    }

    /// Clear this thread's counters.
    pub fn reset() {
        STATS.with_borrow_mut(|stats| *stats = Snapshot::default());
        DEPTH.set(0);
    }

    pub fn snapshot() -> Snapshot {
        STATS.with_borrow(Clone::clone)
    }

    pub(crate) fn begin_build() {
        if is_enabled() {
            DEPTH.set(DEPTH.get() + 1);
        }
    }

    pub(crate) fn end_build() {
        if is_enabled() {
            DEPTH.set(DEPTH.get().saturating_sub(1));
        }
    }

    /// Count the forms of one class paradigm. Only recorded inside a build.
    pub(crate) fn note_forms(class: u16, forms: usize) {
        if forms == 0 || !is_enabled() || DEPTH.get() == 0 {
            return;
        }
        let index = usize::from(class).min(OTHER_CLASS);
        STATS.with_borrow_mut(|stats| {
            stats.by_class[index].forms += forms as u64;
        });
    }

    /// Count one build and its outcome. `forms` includes derived layers.
    pub(crate) fn note_build(
        inflections: &[Inflection],
        caller: Caller,
        failed: bool,
        forms: usize,
    ) {
        if !is_enabled() {
            return;
        }
        STATS.with_borrow_mut(|stats| {
            stats.builds += 1;
            stats.forms += forms as u64;
            if failed {
                stats.failed += 1;
            }
            let caller_stats = &mut stats.by_caller[caller.index()];
            caller_stats.builds += 1;
            caller_stats.forms += forms as u64;
            if failed {
                caller_stats.failed += 1;
            }
            if inflections.is_empty() {
                add_class(&mut stats.by_class[OTHER_CLASS], failed, forms);
            } else {
                for inflection in inflections {
                    let index = usize::from(inflection.class).min(OTHER_CLASS);
                    add_class(&mut stats.by_class[index], failed, forms);
                }
            }
        });
    }

    fn add_class(stats: &mut ClassStats, failed: bool, forms: usize) {
        stats.builds += 1;
        stats.forms += forms as u64;
        if failed {
            stats.failed += 1;
        }
    }
}

#[cfg(not(feature = "probe-stats"))]
#[allow(dead_code)]
mod imp {
    use super::{Caller, Snapshot};
    use crate::kotus::Inflection;

    pub fn enable() {}
    pub fn disable() {}
    pub fn is_enabled() -> bool {
        false
    }
    pub fn reset() {}
    pub fn snapshot() -> Snapshot {
        Snapshot::default()
    }
    pub(crate) fn begin_build() {}
    pub(crate) fn end_build() {}
    pub(crate) fn note_forms(_class: u16, _forms: usize) {}
    pub(crate) fn note_build(
        _inflections: &[Inflection],
        _caller: Caller,
        _failed: bool,
        _forms: usize,
    ) {
    }
}

pub use imp::*;
