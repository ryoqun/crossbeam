//! The default garbage collector.
//!
//! For each thread, a participant is lazily initialized on its first use, when the current thread
//! is registered in the default collector.  If initialized, the thread's participant will get
//! destructed on thread exit, which in turn unregisters the thread.

use crate::collector::{Collector, LocalHandle};
use crate::guard::Guard;
use crate::primitive::thread_local;
#[cfg(not(crossbeam_loom))]
use once_cell::sync::Lazy;

/// The global data for the default garbage collector.
#[cfg(not(crossbeam_loom))]
static DEFAULT_COLLECTOR: Lazy<Collector> = Lazy::new(Collector::new);
// FIXME: loom does not currently provide the equivalent of Lazy:
// https://github.com/tokio-rs/loom/issues/263
#[cfg(crossbeam_loom)]
loom::lazy_static! {
    /// The global data for the default garbage collector.
    static ref DEFAULT_COLLECTOR: Collector = Collector::new();
}

/// Returns the default global collector.
pub fn default_collector() -> &'static Collector {
    &DEFAULT_COLLECTOR
}

thread_local! {
    /// The per-thread participant for the default garbage collector.
    static HANDLE: LocalHandle = default_collector().register();
}

/// Pins the current thread.
#[inline]
pub fn pin() -> Guard {
    with_default_handle(|handle| handle.pin())
}

/// Returns `true` if the current thread is pinned.
#[inline]
pub fn is_pinned() -> bool {
    with_default_handle(|handle| handle.is_pinned())
}

#[inline]
fn with_default_handle<F, R>(mut f: F) -> R
where
    F: FnMut(&LocalHandle) -> R,
{
    HANDLE
        .try_with(|h| f(h))
        .unwrap_or_else(|_| f(&DEFAULT_COLLECTOR.register()))
}

#[inline]
fn with_default_handle_traited<C: CustomCollector, F, R>(mut f: F) -> R
where
    F: FnMut(&LocalHandle) -> R,
{
    f(C::local_handle())
}

trait CustomCollector {
    fn collector() -> &'static Collector;

    fn local_handle() -> &'static LocalHandle;
}

struct DefaultCollector;

impl CustomCollector for DefaultCollector {
}

#[cfg(all(test, not(crossbeam_loom)))]
mod tests {
    use crossbeam_utils::thread;

    #[test]
    fn pin_while_exiting() {
        struct Foo;

        impl Drop for Foo {
            fn drop(&mut self) {
                // Pin after `HANDLE` has been dropped. This must not panic.
                super::pin();
            }
        }

        thread_local! {
            static FOO: Foo = Foo;
        }

        thread::scope(|scope| {
            scope.spawn(|_| {
                // Initialize `FOO` and then `HANDLE`.
                FOO.with(|_| ());
                super::pin();
                // At thread exit, `HANDLE` gets dropped first and `FOO` second.
            });
        })
        .unwrap();
    }
}
