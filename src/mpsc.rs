use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
};

pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}

pub struct ChannelClosedError;

impl<T> Sender<T> {
    /// returns `Ok` is value is sent or `Err(value)` if receiver is dropped
    pub fn send(&mut self, value: T) -> Result<(), T> {
        let mut inner = self.shared.inner.lock().unwrap();
        if !inner.receiver_alive {
            return Err(value);
        }
        inner.queue.push_back(value);
        drop(inner);
        self.shared.can_receive.notify_one();
        Ok(())
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.senders += 1;
        drop(inner);

        Sender {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.senders -= 1;
        let was_last = inner.senders == 0;
        let receiver_alive = inner.receiver_alive;
        drop(inner);

        // notifying receiver to stop blocking if this was the last receiver
        if was_last && receiver_alive {
            self.shared.can_receive.notify_one();
        }
    }
}

pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
    buffer: VecDeque<T>,
}

impl<T> Receiver<T> {
    /// Returns `Some(value)` when value is available (will block if channel is empty) or `None` if channel is closed
    pub fn receive(&mut self) -> Option<T> {
        if let Some(value) = self.buffer.pop_front() {
            return Some(value);
        }

        let mut inner = self.shared.inner.lock().unwrap();
        loop {
            match inner.queue.pop_front() {
                Some(value) => {
                    if !inner.queue.is_empty() {
                        std::mem::swap(&mut inner.queue, &mut self.buffer);
                    }
                    return Some(value);
                }
                None if inner.senders == 0 => return None,
                None => {
                    inner = self.shared.can_receive.wait(inner).unwrap();
                }
            }
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.receiver_alive = false;
        drop(inner);
    }
}

struct Inner<T> {
    queue: VecDeque<T>,
    senders: usize,
    receiver_alive: bool,
}

struct Shared<T> {
    inner: Mutex<Inner<T>>,
    can_receive: Condvar,
}

/// Creates an unbounded mpsc channel
pub fn unbounded_channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Inner {
        queue: VecDeque::new(),
        senders: 1,
        receiver_alive: true,
    };
    let shared = Shared {
        inner: Mutex::new(inner),
        can_receive: Condvar::new(),
    };
    let shared = Arc::new(shared);
    (
        Sender {
            shared: Arc::clone(&shared),
        },
        Receiver {
            shared: Arc::clone(&shared),
            buffer: VecDeque::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let (mut tx, mut rx) = unbounded_channel();
        assert_eq!(tx.send(5), Ok(()));
        assert_eq!(rx.receive(), Some(5));
    }

    #[test]
    fn tx_closed() {
        let (tx, mut rx) = unbounded_channel::<()>();
        drop(tx);
        assert_eq!(rx.receive(), None);
    }

    #[test]
    fn rx_closed() {
        let (mut tx, rx) = unbounded_channel();
        drop(rx);
        assert_eq!(tx.send(5), Err(5));
    }

    #[test]
    fn it_works_in_different_threads() {
        let (tx, mut rx) = unbounded_channel();
        {
            let mut tx1 = tx.clone();
            std::thread::spawn(move || {
                assert_eq!(tx1.send(1), Ok(()));
            });
        }
        {
            let mut tx2 = tx.clone();
            std::thread::spawn(move || {
                assert_eq!(tx2.send(1), Ok(()));
            });
        }
        drop(tx);

        assert_eq!(rx.receive(), Some(1));
        assert_eq!(rx.receive(), Some(1));
        assert_eq!(rx.receive(), None);
    }
}
