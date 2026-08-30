//! Running the formatter and the linter without letting a panic escape the
//! exit-code contract (AD-0017).
//!
//! FR-5.6 defines three exit codes and `101` is not one of them. prim works
//! through a rayon pool with no `catch_unwind` anywhere, so one panicking
//! worker took the whole process to `101` — and the files beside it produced
//! no output at all, neither the file that panicked nor the ones that would
//! have been fine (#125).
//!
//! The panics that have actually reached prim came from dependencies'
//! `debug_assert!`s, and each was silenced one at a time with a
//! `[profile.dev.package.*] debug-assertions = false` entry (AD-0006). That is
//! per-package maintenance with no general protection, and the next round is
//! already visible in `dprint-plugin-json`'s position assertions.
//!
//! Containment is not a licence to continue: a file prim panicked on is
//! reported as an error and left byte-for-byte unchanged, which is what FR-6.3
//! already says to do with a file prim cannot process.

use std::panic::AssertUnwindSafe;
use std::path::Path;

/// A panic that [`contained`] stopped.
///
/// Carries nothing. The panic's own message has already gone to stderr
/// through the default hook, which is the only place a stack trace is
/// available, and prim's report needs the path — which the caller has.
#[derive(Debug)]
pub(crate) struct Panicked;

/// Run `op` over `path`, converting a panic into an error the caller reports.
///
/// Generic over the operation so one containment covers every third-party
/// entry point prim calls — the formatters and `rumdl`, which is the larger
/// body of the two — and so the containment itself can be tested.
///
/// # Unwind safety
///
/// `AssertUnwindSafe` is not the claim that nothing is left inconsistent. It
/// is the claim that nothing *prim* observes afterwards is: `op` returns an
/// owned value, and where it panics the caller discards the file and writes
/// nothing.
///
/// Inside the dependencies it is not true. `dprint-core`'s `format` increments
/// a thread-local formatting count and decrements it only on the success path,
/// with no drop guard, so an unwind leaves that thread's count stuck above
/// zero and its bump allocator never reset again. The effect is a leak in that
/// worker, not incorrect output and not undefined behaviour — the count can
/// only be left too high, and too high suppresses the reset. It is bounded by
/// the process for `prim fmt`, and by the session for `prim lsp`, which
/// deliberately keeps going. Accepted: a leaked arena in a worker that has
/// already hit a dependency bug is a smaller cost than the exit code.
pub(crate) fn contained<T>(path: &Path, op: impl FnOnce() -> T) -> Result<T, Panicked> {
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        injected_panic(path);
        op()
    }))
    .map_err(|_| Panicked)
}

/// Panic deliberately when `PRIM_PANIC_INJECT` holds a substring of `path`.
///
/// A debug-build-only fault injector, compiled out of every release binary.
/// It exists because the contract this module implements is about faults
/// nobody can otherwise produce on demand: the two inputs known to panic are
/// pinned in `prim-fmt` and only stay quiet because of the profile overrides
/// AD-0006 records, and reaching them from a test would mean building without
/// those. Matching on a substring rather than a flag is what lets a test say
/// "this file panics, its neighbours do not", which is the half of #125 that
/// is about the run continuing.
#[cfg(debug_assertions)]
fn injected_panic(path: &Path) {
    if let Some(marker) = std::env::var_os("PRIM_PANIC_INJECT")
        && !marker.is_empty()
        && path.to_string_lossy().contains(&*marker.to_string_lossy())
    {
        panic!("PRIM_PANIC_INJECT: deliberate panic for {}", path.display());
    }
}

#[cfg(not(debug_assertions))]
fn injected_panic(_path: &Path) {}

/// What prim reports for `subject` when a dependency panicked on it.
pub(crate) fn panic_message(subject: &Path) -> String {
    format!(
        "{}: panicked while processing this file; it is unchanged — please \
         report it at https://github.com/driftsys/prim/issues",
        subject.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `set_hook` is process-wide and the suite runs in parallel, so the two
    /// tests that silence it take turns and put it back.
    static HOOK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn quietly<T>(op: impl FnOnce() -> T) -> Result<T, Panicked> {
        let guard = HOOK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = contained(Path::new("a.md"), op);
        std::panic::set_hook(hook);
        drop(guard);
        outcome
    }

    #[test]
    fn a_value_passes_through_untouched() {
        assert_eq!(contained(Path::new("a.md"), || 21 * 2).ok(), Some(42));
    }

    #[test]
    fn an_inner_error_is_not_a_panic() {
        // The formatter's own typed errors must stay distinguishable from a
        // panic: one is a file prim parsed and rejected, the other is a bug.
        let outcome: Result<Result<&str, &str>, Panicked> =
            contained(Path::new("a.md"), || Err("unparseable"));
        assert!(matches!(outcome, Ok(Err("unparseable"))));
    }

    #[test]
    fn a_panic_becomes_an_error() {
        assert!(quietly(|| panic!("dependency debug_assert")).is_err());
    }

    #[test]
    fn a_panic_does_not_stop_the_next_call() {
        // Note this pins the mechanism only. That the *files* beside a
        // panicking one still get processed is a property of the run, and is
        // pinned end-to-end in `crates/prim-cli/tests/safety.rs` through the
        // injector above.
        assert!(quietly(|| panic!("first")).is_err());
        assert_eq!(quietly(|| "second").ok(), Some("second"));
    }

    #[test]
    fn the_report_names_the_subject() {
        let message = panic_message(Path::new("docs/guide.md"));
        assert!(message.contains("docs/guide.md"), "{message}");
        assert!(message.contains("unchanged"), "{message}");
    }
}
