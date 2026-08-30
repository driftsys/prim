//! Running the formatter without letting a panic escape the exit-code
//! contract (AD-0017).
//!
//! FR-5.6 defines three exit codes and `101` is not one of them. prim formats
//! through a rayon pool with no `catch_unwind` anywhere, so one panicking
//! worker took the whole process to `101` and the files beside it produced no
//! output at all — neither the file that panicked nor the ones that would have
//! been fine (#125).
//!
//! The panics that have actually reached prim came from dependencies'
//! `debug_assert!`s, and each was silenced one at a time with a
//! `[profile.dev.package.*] debug-assertions = false` entry (AD-0006). That is
//! per-package maintenance with no general protection, and the next round is
//! already visible in `dprint-plugin-json`'s position assertions. Containing
//! the call holds the contract for every future one instead of one at a time.
//!
//! Containment is not a licence to continue: a panicking file is reported as
//! an error and left byte-for-byte unchanged, which is what FR-6.3 already
//! says to do with a file prim cannot process.

use std::panic::AssertUnwindSafe;

/// A panic that `contained` stopped.
///
/// Carries nothing. The panic's own message has already gone to stderr
/// through the default hook, which is the only place a stack trace is
/// available, and prim's report needs the path — which the caller has and
/// this does not.
#[derive(Debug)]
pub(crate) struct Panicked;

/// Run `op`, converting a panic into an error the caller can report.
///
/// Generic over the operation rather than wrapping `prim_fmt::format`
/// directly, so the containment itself can be tested: no input is known that
/// makes the real formatter panic, and one that did would be a bug to fix
/// rather than a fixture to keep.
///
/// `AssertUnwindSafe` is sound here because nothing observable outlives a
/// failure: `op` returns an owned value, and where it panics the caller
/// discards the file and writes nothing.
pub(crate) fn contained<T>(op: impl FnOnce() -> T) -> Result<T, Panicked> {
    std::panic::catch_unwind(AssertUnwindSafe(op)).map_err(|_| Panicked)
}

/// What prim reports for `path` when the formatter panicked on it.
pub(crate) fn panic_message(path: &std::path::Path) -> String {
    format!(
        "{}: the formatter panicked on this file; left unchanged — please \
         report it at https://github.com/driftsys/prim/issues",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default hook prints the panic to stderr, which is wanted in a real
    /// run and only noise in these tests.
    fn quietly<T>(op: impl FnOnce() -> T) -> Result<T, Panicked> {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = contained(op);
        std::panic::set_hook(hook);
        outcome
    }

    #[test]
    fn a_value_passes_through_untouched() {
        assert_eq!(contained(|| 21 * 2).ok(), Some(42));
    }

    #[test]
    fn an_inner_error_is_not_a_panic() {
        // The formatter's own typed errors must stay distinguishable from a
        // panic: one is a file prim parsed and rejected, the other is a bug.
        let outcome: Result<Result<&str, &str>, Panicked> = contained(|| Err("unparseable"));
        assert!(matches!(outcome, Ok(Err("unparseable"))));
    }

    #[test]
    fn a_panic_becomes_an_error() {
        assert!(quietly(|| panic!("dependency debug_assert")).is_err());
    }

    #[test]
    fn a_panic_does_not_stop_the_next_call() {
        // The property #125 is about: the files beside the panicking one
        // still get processed.
        assert!(quietly(|| panic!("first")).is_err());
        assert_eq!(quietly(|| "second").ok(), Some("second"));
    }
}
