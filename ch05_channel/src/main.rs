use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{
        AtomicBool,
        Ordering::{Acquire, Relaxed, Release},
    },
    thread,
};

pub struct Channel<T> {
    data: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            data: UnsafeCell::new(MaybeUninit::uninit()),
            ready: AtomicBool::new(false),
        }
    }

    pub fn split(&mut self) -> (Sender<T>, Receiver<T>) {
        (Sender { channel: self }, Receiver { channel: self })
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            unsafe { (*self.data.get()).assume_init_drop() }
        }
    }
}

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
}

impl<T> Sender<'_, T> {
    pub fn send(self, message: T) {
        unsafe { (*self.channel.data.get()).write(message) };
        self.channel.ready.store(true, Release);
    }
}

pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
}

impl<T> Receiver<'_, T> {
    pub fn receive(self) -> T {
        if !self.channel.ready.swap(false, Acquire) {
            panic!("channel is not ready");
        }

        unsafe { (*self.channel.data.get()).assume_init_read() }
    }

    pub fn is_ready(&self) -> bool {
        self.channel.ready.load(Relaxed)
    }
}

fn main() {
    let mut channel = Channel::<&'static str>::new();
    let (sender, receiver) = channel.split();
    let t = thread::current();
    thread::scope(|s| {
        s.spawn(|| {
            sender.send("hello, world");
            t.unpark();
        });

        while !receiver.is_ready() {
            thread::park();
        }

        assert_eq!(receiver.receive(), "hello, world");
    });
}
