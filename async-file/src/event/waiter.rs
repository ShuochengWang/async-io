use std::sync::{Arc, Mutex};

use atomic::{Atomic, Ordering};
use intrusive_collections::intrusive_adapter;
use intrusive_collections::linked_list::Iter;
use intrusive_collections::{LinkedList, LinkedListLink};

use crate::event::counter::Counter;
use crate::util::object_id::ObjectId;

/// A waiter.
pub struct Waiter {
    inner: Arc<Waiter_>,
}

struct Waiter_ {
    counter: Counter,
    queue_id: Atomic<ObjectId>,
    link: LinkedListLink,
}

/// A waiter queue.
pub struct WaiterQueue {
    inner: Mutex<WaiterQueue_>,
    queue_id: ObjectId,
}

struct WaiterQueue_ {
    list: LinkedList<LinkedListAdapter>,
}

intrusive_adapter!(LinkedListAdapter =
    Arc<Waiter_>: Waiter_ {
        link: LinkedListLink
    }
);

impl Waiter {
    pub fn new() -> Self {
        let inner = Arc::new(Waiter_::new());
        Self { inner }
    }

    pub async fn wait(&self) {
        self.inner.counter.read().await;
    }
}

impl Waiter_ {
    pub fn new() -> Self {
        let queue_id = Atomic::new(ObjectId::null());
        let counter = Counter::new(0);
        let link = LinkedListLink::new();
        Self {
            counter,
            queue_id,
            link,
        }
    }
}

unsafe impl Sync for Waiter_ {}
unsafe impl Send for Waiter_ {}

impl WaiterQueue {
    pub fn new() -> Self {
        let inner = Mutex::new(WaiterQueue_::new());
        let queue_id = ObjectId::new();
        Self { inner, queue_id }
    }

    pub fn enqueue(&self, waiter: &Waiter) {
        let old_queue_id = waiter.inner.queue_id.swap(self.queue_id, Ordering::Relaxed);
        assert!(old_queue_id == ObjectId::null());

        let mut inner = self.inner.lock().unwrap();
        inner.list.push_back(waiter.inner.clone());
    }

    pub fn dequeue(&self, waiter: &Waiter) {
        let old_queue_id = waiter
            .inner
            .queue_id
            .swap(ObjectId::null(), Ordering::Relaxed);
        assert!(old_queue_id == self.queue_id);

        let mut inner = self.inner.lock().unwrap();
        let mut cursor = unsafe { inner.list.cursor_mut_from_ptr(Arc::as_ptr(&waiter.inner)) };
        let waiter_inner = cursor.remove().unwrap();
        drop(waiter_inner);
    }

    pub fn wake_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .list
            .iter()
            .for_each(|waiter_inner| waiter_inner.counter.write());
    }
}

impl WaiterQueue_ {
    pub fn new() -> Self {
        let list = LinkedList::new(LinkedListAdapter::new());
        Self { list }
    }
}