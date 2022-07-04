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

thread_local! {
    /// The per-thread participant for the default garbage collector.
    static DEFAULT_HANDLE: LocalHandle = DEFAULT_COLLECTOR.register();
}

/// Pins the current thread.
#[inline]
pub fn pin<C: CustomCollector>() -> Guard {
    C::with_handle(|handle| handle.pin())
}

/// Returns `true` if the current thread is pinned.
#[inline]
pub fn is_pinned<C: CustomCollector>() -> bool {
    C::with_handle(|handle| handle.is_pinned())
}

/// ccccc
pub trait CustomCollector {
    /// cccc
    fn collector() -> &'static Collector;
    /// hhhh
    fn handle() -> &'static std::thread::LocalKey<LocalHandle>; 

    fn new() -> Box<dyn DynCustomCollector>;

    /// wwww
    fn with_handle<F, R>(mut f: F) -> R
    where
        F: FnMut(&LocalHandle) -> R,
    {
        //dbg!(std::any::type_name::<Self>());
        Self::handle()
            .try_with(|h| f(h))
            .unwrap_or_else(|_| f(&Self::collector().register()))
    }
}

pub trait DynCustomCollector {
    /// cccc
    fn collector(&self) -> &'static Collector;

    /// hhhh
    fn handle(&self) -> &'static std::thread::LocalKey<LocalHandle>;
}

/// aaaa
#[derive(Debug)]
pub struct DefaultCollector;

impl CustomCollector for DefaultCollector {
    fn collector() -> &'static Collector {
        &DEFAULT_COLLECTOR
    }

    fn handle() -> &'static std::thread::LocalKey<LocalHandle> {
        &DEFAULT_HANDLE
    }

    fn new() -> Box<dyn DynCustomCollector> {
        Box::<Self>::new(DefaultCollector)
    }
}

impl DynCustomCollector for DefaultCollector {
    fn collector(&_self) -> &'static Collector {
        &DEFAULT_COLLECTOR
    }

    fn handle(&_self) -> &'static std::thread::LocalKey<LocalHandle> {
        &DEFAULT_HANDLE
    }
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
