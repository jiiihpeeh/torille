//! The current verified surface, for the target-aware verb builder.
//!
//! A stem-map verify knows the exact surface. The verb builders can skip a
//! cell before formatting when the cell's `stem + ending` cannot equal that
//! surface. The target is thread-local because the builders are deep inside
//! the paradigm walk and do not otherwise take a context.
//!
//! The target is `None` for every ordinary build. Only
//! [`crate::expand::emits_target_form`] sets it, and it restores the previous
//! value on the way out, including on a panic.

use std::cell::RefCell;

thread_local! {
    static TARGET: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The target of the build on this thread, if any.
pub(crate) fn current() -> Option<String> {
    TARGET.with_borrow(Clone::clone)
}

/// Run `build` with `target` active, restoring the previous target after.
pub(crate) fn with_target<R>(target: Option<&str>, build: impl FnOnce() -> R) -> R {
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TARGET.with_borrow_mut(|slot| *slot = self.0.take());
        }
    }
    let previous = TARGET
        .with_borrow_mut(|slot| std::mem::replace(&mut *slot, target.map(str::to_string)));
    let _restore = Restore(previous);
    build()
}
