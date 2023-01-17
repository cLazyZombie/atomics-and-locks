use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicU32,
    sync::atomic::Ordering::{Acquire, Relaxed, Release},
};

pub struct RwLock<T> {
    /// number of readers, or u32::MAX if write-locked.
    state: AtomicU32,
    value: UnsafeCell<T>,
}

unsafe impl<T> Sync for RwLock<T> where T: Send + Sync {}

impl<T> RwLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0), // unlocked.
            value: UnsafeCell::new(value),
        }
    }

    pub fn read(&self) -> ReadGuard<T> {
        let mut s = self.state.load(Relaxed);
        loop {
            if s < u32::MAX {
                match self.state.compare_exchange_weak(s, s + 1, Acquire, Relaxed) {
                    Ok(_) => return ReadGuard { lock: self },
                    Err(e) => s = e,
                }
            }
            if s == u32::MAX {
                atomic_wait::wait(&self.state, u32::MAX);
                s = self.state.load(Relaxed);
            }
        }
    }

    pub fn write(&self) -> WriteGuard<T> {
        while let Err(s) = self.state.compare_exchange(0, u32::MAX, Acquire, Relaxed) {
            atomic_wait::wait(&self.state, s);
        }
        WriteGuard { lock: self }
    }
}

pub struct ReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        if self.lock.state.fetch_sub(1, Release) == 1 {
            atomic_wait::wake_one(&self.lock.state);
        }
    }
}

pub struct WriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.store(0, Release);
        atomic_wait::wake_all(&self.lock.state);
    }
}

impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}
