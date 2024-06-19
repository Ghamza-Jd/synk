use crate::atomic_wait;
use std::cell::UnsafeCell;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

pub struct Myoutex<T> {
    /// - 0: unlocked
    /// - 1: locked, no other threads waiting
    /// - 2: locked, other threads waiting
    state: AtomicU32,
    value: UnsafeCell<T>,
}

unsafe impl<T> Sync for Myoutex<T> where T: Send {}

pub struct MyoutexGuard<'a, T> {
    // pub(crate) to make it accessable for kondvar
    pub(crate) myoutex: &'a Myoutex<T>,
}

impl<T> Deref for MyoutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.myoutex.value.get() }
    }
}

impl<T> DerefMut for MyoutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.myoutex.value.get() }
    }
}

impl<T> Myoutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0), // unlocked state,
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MyoutexGuard<T> {
        if self
            .state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            Self::lock_contended(&self.state);
        }
        MyoutexGuard { myoutex: self }
    }

    fn lock_contended(state: &AtomicU32) {
        let mut spin_count = 0;

        while state.load(Ordering::Relaxed) == 1 && spin_count < 100 {
            spin_count += 1;
            std::hint::spin_loop();
        }

        if state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }

        while state.swap(2, Ordering::Acquire) != 0 {
            atomic_wait::wait(state, 2);
        }
    }
}

impl<T> Drop for MyoutexGuard<'_, T> {
    fn drop(&mut self) {
        if self.myoutex.state.swap(0, Ordering::Release) == 2 {
            atomic_wait::wake_one(&self.myoutex.state);
        }
    }
}
