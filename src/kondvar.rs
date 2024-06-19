use crate::atomic_wait::wait;
use crate::atomic_wait::wake_all;
use crate::atomic_wait::wake_one;
use crate::myoutex::MyoutexGuard;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

pub struct KondVar {
    counter: AtomicU32,
    num_waiters: AtomicUsize,
}

impl KondVar {
    pub const fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
            num_waiters: AtomicUsize::new(0),
        }
    }

    pub fn notify_one(&self) {
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_one(&self.counter);
        }
    }

    pub fn notify_all(&self) {
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_all(&self.counter);
        }
    }

    pub fn wait<'a, T>(&self, guard: MyoutexGuard<'a, T>) -> MyoutexGuard<'a, T> {
        self.num_waiters.fetch_add(1, Ordering::Relaxed);
        let counter_value = self.counter.load(Ordering::Relaxed);

        // Unlock the mutex by dropping the guard,
        // but remember the mutex so we can lock it again later.
        let myoutex = guard.myoutex;
        drop(guard);

        // Wait, but only if the counter hasn't changed since unlocking.
        wait(&self.counter, counter_value);

        self.num_waiters.fetch_sub(1, Ordering::Relaxed);
        myoutex.lock()
    }
}
