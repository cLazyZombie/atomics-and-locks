use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering::Relaxed};

use atomic_wait::{wait, wake_all, wake_one};

use crate::mutex::MutexGuard;

pub struct CondVar {
    counter: AtomicU32,
    num_waiters: AtomicUsize,
}

impl CondVar {
    pub const fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
            num_waiters: AtomicUsize::new(0),
        }
    }

    pub fn notify_one(&self) {
        if self.num_waiters.load(Relaxed) > 0 {
            self.counter.fetch_add(1, Relaxed);
            wake_one(&self.counter);
        }
    }

    pub fn notify_all(&self) {
        if self.num_waiters.load(Relaxed) > 0 {
            self.counter.fetch_add(1, Relaxed);
            wake_all(&self.counter);
        }
    }

    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        self.num_waiters.fetch_add(1, Relaxed);
        let counter_value = self.counter.load(Relaxed);
        let mutex = guard.mutex;
        drop(guard);

        wait(&self.counter, counter_value);

        self.num_waiters.fetch_sub(1, Relaxed);

        mutex.lock()
    }
}

#[cfg(test)]
mod tests {
    use crate::mutex::Mutex;

    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn test_condvar() {
        let mutex = Mutex::new(0);
        let condvar = CondVar::new();

        let mut wakeups = 0;

        thread::scope(|s| {
            s.spawn(|| {
                thread::sleep(Duration::from_secs(1));
                *mutex.lock() = 123;
                condvar.notify_one();
            });

            let mut m = mutex.lock();
            while *m < 100 {
                m = condvar.wait(m);
                wakeups += 1;
            }

            assert_eq!(*m, 123);
        });

        assert!(wakeups < 10);
    }
}
