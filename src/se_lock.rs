use crate::atomic_wait::wait;
use crate::atomic_wait::wake_all;
use crate::atomic_wait::wake_one;
use std::cell::UnsafeCell;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

// Same as RwLock
// But instead of read-write lock
// I've used shared-exclusive lock
// To optimize more add more states like we did for myoutex
// Maybe 0, 1, 2 and u32::MAX are reserved for our optimizations
pub struct SeLock<T> {
    /// The number of shared locks times two, plus one if there's an exclusive waiting.
    /// [`u32::MAX`] if write locked.
    ///
    /// This means that readers may acquire the lock when
    /// the state is even, but to block when odd.
    state: AtomicU32,
    /// Incremented to wake up writers.
    exclusive_wake_counter: AtomicU32,
    value: UnsafeCell<T>,
}

unsafe impl<T> Sync for SeLock<T> where T: Send + Sync {}

impl<T> SeLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            exclusive_wake_counter: AtomicU32::new(0),
            value: UnsafeCell::new(value),
        }
    }

    pub fn shared(&self) -> ShareGuard<T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s % 2 == 0 {
                assert!(s != u32::MAX - 2, "too many readers");
                match self.state.compare_exchange_weak(
                    s,
                    s + 2,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return ShareGuard { selock: self },
                    Err(e) => s = e,
                }
            }

            if s % 2 == 1 {
                wait(&self.state, s);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }

    pub fn exclusive(&self) -> ExclusiveGuard<T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            // Try to lock if unlocked.
            if s <= 1 {
                match self
                    .state
                    .compare_exchange(s, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
                {
                    Ok(_) => return ExclusiveGuard { selock: self },
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }

            // Block new readers, by making usre the state is odd.
            if s % 2 == 0 {
                match self
                    .state
                    .compare_exchange(s, s + 1, Ordering::Relaxed, Ordering::Relaxed)
                {
                    Ok(_) => {}
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }

            // Wait, if it's still locked
            let w = self.exclusive_wake_counter.load(Ordering::Acquire);
            s = self.state.load(Ordering::Relaxed);
            if s >= 2 {
                wait(&self.exclusive_wake_counter, w);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }
}

pub struct ShareGuard<'a, T> {
    selock: &'a SeLock<T>,
}

pub struct ExclusiveGuard<'a, T> {
    selock: &'a SeLock<T>,
}

impl<T> Deref for ExclusiveGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.selock.value.get() }
    }
}

impl<T> DerefMut for ExclusiveGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.selock.value.get() }
    }
}

impl<T> Deref for ShareGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.selock.value.get() }
    }
}

impl<T> Drop for ShareGuard<'_, T> {
    fn drop(&mut self) {
        // Decrement the state by 2 to remove one shared-lock.
        if self.selock.state.fetch_sub(2, Ordering::Release) == 3 {
            // if we decremented from 3 to 1, that means
            // the SeLock is now unlocked _and_ there is
            // a waiting exclusive, which we wake up.
            self.selock
                .exclusive_wake_counter
                .fetch_add(1, Ordering::Release);
            wake_one(&self.selock.exclusive_wake_counter);
        }
    }
}

impl<T> Drop for ExclusiveGuard<'_, T> {
    fn drop(&mut self) {
        self.selock.state.store(0, Ordering::Release);
        self.selock
            .exclusive_wake_counter
            .fetch_add(1, Ordering::Release);
        wake_one(&self.selock.exclusive_wake_counter);
        wake_all(&self.selock.state);
    }
}
