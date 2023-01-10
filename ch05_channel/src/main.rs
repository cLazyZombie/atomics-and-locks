use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{
        AtomicU8,
        Ordering::{Acquire, Relaxed, Release},
    },
    thread,
};

pub struct Channel<T> {
    data: UnsafeCell<MaybeUninit<T>>,
    state: AtomicU8,
}

unsafe impl<T> Sync for Channel<T> {}

const EMPTY: u8 = 0;
const WRITING: u8 = 1;
const READY: u8 = 2;
const READING: u8 = 3;

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            data: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicU8::new(EMPTY),
        }
    }

    pub fn send(&self, message: T) {
        if !self
            .state
            .compare_exchange(EMPTY, WRITING, Relaxed, Relaxed)
            .is_ok()
        {
            panic!("channel is not empty");
        }

        unsafe {
            (*self.data.get()).write(message);
        }
        self.state.store(READY, Release);
    }

    pub fn receive(&self) -> T {
        if !self
            .state
            .compare_exchange(READY, READING, Acquire, Relaxed)
            .is_ok()
        {
            panic!("channel is not ready");
        }

        unsafe { (*self.data.get()).assume_init_read() }
    }

    pub fn is_ready(&self) -> bool {
        self.state.load(Relaxed) == READY
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.state.get_mut() == READY {
            unsafe {
                (*self.data.get()).assume_init_drop();
            }
        }
    }
}

fn main() {
    let channel = Channel::new();
    let t = thread::current();
    thread::scope(|s| {
        s.spawn(|| {
            channel.send("hello, world");
            t.unpark();
        });

        while !channel.is_ready() {
            thread::park();
        }

        assert_eq!(channel.receive(), "hello, world");
    });
}
