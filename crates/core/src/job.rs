use crate::models::Job;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct JobQueue {
    inner: Arc<Mutex<VecDeque<Job>>>,
}

impl JobQueue {
    // Provide Default for ergonomic construction in tests and APIs
    pub fn new() -> Self {
        JobQueue { inner: Arc::new(Mutex::new(VecDeque::new())) }
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    pub fn enqueue(&self, job: Job) {
        let mut q = self.inner.lock().unwrap();
        q.push_back(job);
    }

    pub fn dequeue(&self) -> Option<Job> {
        let mut q = self.inner.lock().unwrap();
        q.pop_front()
    }

    pub fn len(&self) -> usize {
        let q = self.inner.lock().unwrap();
        q.len()
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        JobQueue::new()
    }
}

