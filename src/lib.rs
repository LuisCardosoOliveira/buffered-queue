use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

pub struct Producer<T>(Arc<BufferedQueue<T>>);
pub struct Consumer<T>(Arc<BufferedQueue<T>>);

pub struct BufferedQueue<T> {
    data: Mutex<VecDeque<T>>,
    pub capacity: usize,
    pub is_full: Mutex<bool>,
    pub is_full_signal: Condvar,
    pub is_empty: Mutex<bool>,
    pub is_empty_signal: Condvar,
    pub elements_processed: AtomicBool,
}

enum Operation<'a> {
    Push { is_full_flag: MutexGuard<'a, bool> },
    Pop { is_empty_flag: MutexGuard<'a, bool> },
}

impl<T> Producer<T> {
    pub fn push(&self, value: T) {
        let mut queue_is_full = self.0.is_full.lock().unwrap();
        while *queue_is_full {
            queue_is_full = self.0.is_full_signal.wait(queue_is_full).unwrap();
        }
        let mut queue = self.0.data.lock().unwrap();
        queue.push_back(value);
        println!("pushed element");

        self.0.signal_queue_changes(
            queue,
            Operation::Push {
                is_full_flag: queue_is_full,
            },
        );
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        self.0.elements_processed.store(true, Ordering::SeqCst);
    }
}

impl<T> Consumer<T> {
    pub fn pop(&self) -> Option<T> {
        let mut queue_is_empty = self.0.is_empty.lock().unwrap();
        while *queue_is_empty {
            if self.0.elements_processed.load(Ordering::SeqCst) {
                return None;
            }
            queue_is_empty = self.0.is_empty_signal.wait(queue_is_empty).unwrap();
        }

        let mut queue = self.0.data.lock().unwrap();
        let popped_element = queue.pop_front();
        println!("popped element");

        self.0.signal_queue_changes(
            queue,
            Operation::Pop {
                is_empty_flag: queue_is_empty,
            },
        );
        popped_element
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

pub fn buffered_queue<T>(mut capacity: usize) -> (Producer<T>, Consumer<T>) {
    if capacity < 1 {
        eprintln!("capacity cannot be lower than 1, defaulting to 1...");
        capacity = 1
    }

    let buffered_queue = BufferedQueue {
        data: Mutex::new(VecDeque::with_capacity(capacity)),
        capacity,
        is_full: Mutex::new(false),
        is_empty: Mutex::new(true),
        is_full_signal: Condvar::new(),
        is_empty_signal: Condvar::new(),
        elements_processed: AtomicBool::new(false),
    };

    let data = Arc::new(buffered_queue);
    let producer = Producer(data.clone());
    let consumer = Consumer(data);

    (producer, consumer)
}

impl<T> BufferedQueue<T> {
    fn len(&self) -> usize {
        let queue = self.data.lock().unwrap();
        queue.len()
    }

    fn signal_queue_changes(&self, queue: MutexGuard<'_, VecDeque<T>>, operation: Operation) {
        let is_empty = queue.len() == 0;
        let is_full = queue.len() == self.capacity;

        match operation {
            Operation::Push { mut is_full_flag } => {
                let mut is_empty_flag = self.is_empty.lock().unwrap();
                if *is_empty_flag {
                    *is_empty_flag = false;
                    println!("set is_empty to false");
                    self.is_empty_signal.notify_all();
                }

                if is_full {
                    *is_full_flag = true;
                    self.is_full_signal.notify_all();
                    println!("set is_full to true")
                }
            }

            Operation::Pop { mut is_empty_flag } => {
                let mut is_full_flag = self.is_full.lock().unwrap();
                if *is_full_flag {
                    *is_full_flag = false;
                    println!("set is_full to false");
                    self.is_full_signal.notify_all()
                }

                if is_empty {
                    *is_empty_flag = true;
                    println!("set is_empty to true");
                    self.is_empty_signal.notify_all();
                }
            }
        }
    }
}
