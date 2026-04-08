use crate::types::*;
use ahash::RandomState;
use bytes::Bytes;
use hashbrown::{HashTable, hash_table::Entry};
use std::{
    collections::BTreeSet,
    mem::take,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::{Notify, broadcast::Receiver, mpsc},
    time::Instant,
};
use tracing::{debug, instrument, trace};

const BUFFER: usize = 1024;

type CacheMap = HashTable<(String, CacheValue)>;
type ExpiryQueue = BTreeSet<(Instant, String)>;

pub enum TrashData {
    Unlinked(Vec<CacheValue>),
    Flushed((CacheMap, ExpiryQueue)),
}

#[derive(Default, Debug)]
struct State {
    cache: CacheMap,
    expiration_queue: ExpiryQueue,
    hasher: RandomState,
}

#[derive(Debug)]
pub struct Db {
    state: Mutex<State>,
    alarm: Notify,
    tx: mpsc::Sender<TrashData>,
}

impl State {
    fn new() -> State {
        State {
            cache: HashTable::new(),
            expiration_queue: BTreeSet::new(),
            hasher: RandomState::new(),
        }
    }

    fn insert(&mut self, key: String, value: CacheValue) -> bool {
        let hasher = self.hasher.clone();
        let hash = hasher.hash_one(&key);

        let key_clone = key.clone();
        let expiry = value.expiry;

        match self.cache.find_entry(hash, |val| val.0 == key) {
            Ok(old) => {
                let removed_entry = old.remove();
                let old_expiry = removed_entry.0.1.expiry;
                if let Some(expiry) = old_expiry {
                    self.expiration_queue.remove(&(expiry, removed_entry.0.0));
                };
                removed_entry.1.insert((key, value));
            }
            Err(_) => {
                self.cache
                    .insert_unique(hash, (key, value), move |val| hasher.hash_one(&val.0));
            }
        }

        if let Some(expiry) = expiry {
            self.expiration_queue.insert((expiry, key_clone.clone()));
        }

        let mut notify = false;
        if let Some((_, first_key)) = self.expiration_queue.first() {
            notify = first_key == &key_clone;
        };

        notify
    }

    // Background sweeper for TTL deletion
    fn remove(&mut self, key: &str) {
        let hasher = self.hasher.clone();
        let hash = hasher.hash_one(key);

        if let Ok(old) = self.cache.find_entry(hash, |val| val.0 == key) {
            let removed_entry = old.remove().0;
            if let Some(expiry) = removed_entry.1.expiry {
                self.expiration_queue.remove(&(expiry, removed_entry.0));
            }
        }
    }

    // Public API
    fn delete(&mut self, key: &str) -> bool {
        let hasher = self.hasher.clone();
        let hash = hasher.hash_one(key);

        match self.cache.find_entry(hash, |val| val.0 == key) {
            Ok(old) => {
                let removed_entry = old.remove().0;
                if let Some(expiry) = removed_entry.1.expiry {
                    self.expiration_queue.remove(&(expiry, removed_entry.0));
                }
                true
            }
            Err(_) => false,
        }
    }

    // In Rust, memory is cleaned up (deallocated) the moment a variable's "owner" is finished with it.
    // This is called a Drop. In unlink: You return the CacheValue. This "moves" the ownership out of the
    // State and into the Db::unlink method. You then move it again into a Vec, and finally move it into the trash_chute.
    fn unlink(&mut self, key: &str) -> Option<CacheValue> {
        let hasher = self.hasher.clone();
        let hash = hasher.hash_one(key);

        if let Ok(old) = self.cache.find_entry(hash, |val| val.0 == key) {
            let removed_entry = old.remove().0;
            if let Some(expiry) = removed_entry.1.expiry {
                self.expiration_queue.remove(&(expiry, removed_entry.0));
            }
            Some(removed_entry.1)
        } else {
            None
        }
    }
}

impl Db {
    pub fn new() -> (Db, mpsc::Receiver<TrashData>) {
        let (tx, rx) = mpsc::channel(BUFFER);

        (
            Db {
                state: Mutex::new(State::new()),
                alarm: Notify::new(),
                tx,
            },
            rx,
        )
    }

    #[instrument(level = "trace", skip(self))]
    pub fn get(&self, key: String) -> Option<RedisData> {
        let mut guard = self.state.lock().unwrap();
        let hasher = guard.hasher.clone();
        let hash = hasher.hash_one(&key);

        let mut to_remove = None;
        let result = if let Some(cache_value) = guard.cache.find(hash, |val| val.0 == key) {
            if cache_value.1.is_expired() {
                to_remove = cache_value.1.expiry;
                None
            } else {
                Some(cache_value.1.value.clone())
            }
        } else {
            None
        };

        if let Some(_expiry) = to_remove {
            guard.remove(&key);
        }

        result
    }

    #[instrument(level = "trace", skip(self, cache_value))]
    pub fn set(&self, key: String, cache_value: CacheValue) {
        let mut guard = self.state.lock().expect("DB Mutex poisoned");
        if guard.insert(key, cache_value) {
            self.alarm.notify_one();
        };
    }

    #[instrument(level = "trace", skip(self))]
    pub fn del(&self, keys: Vec<String>) -> usize {
        let mut deleted: usize = 0;
        let mut guard = self.state.lock().expect("Failed to lock DB");
        for key in keys {
            let result = guard.delete(&key);
            if result {
                deleted += 1;
            }
        }
        deleted
    }

    #[instrument(level = "trace", skip(self))]
    pub fn unlink(&self, keys: Vec<String>) -> usize {
        let mut guard = self.state.lock().expect("Failed to lock DB");
        let mut val_to_remove = vec![];
        let mut deleted: usize = 0;

        for key in keys {
            if let Some(val) = guard.unlink(&key) {
                val_to_remove.push(val);
                deleted += 1;
            }
        }

        drop(guard);

        if self
            .tx
            .try_send(TrashData::Unlinked(val_to_remove))
            .is_err()
        {
            debug!("Channel is full, you need to increaste BUFFER size. Falling back to deletion.");
        };
        deleted
    }

    pub fn flush(&self) {
        let mut guard = self.state.lock().expect("Failed to lock DB");

        let old_map = take(&mut guard.cache);
        let old_queue = take(&mut guard.expiration_queue);

        drop(guard);

        if self
            .tx
            .try_send(TrashData::Flushed((old_map, old_queue)))
            .is_err()
        {
            debug!("Channel is full, you need to increaste BUFFER size. Falling back to deletion.");
        };
    }

    #[instrument(level = "trace", skip(self, values))]
    pub fn rpush(&self, key: String, values: Vec<Bytes>) -> Option<usize> {
        let mut to_remove = None;
        let mut guard = self.state.lock().expect("Failed to lock DB");

        let hasher = guard.hasher.clone();
        let hash = hasher.hash_one(&key);

        if let Some(cache_value) = guard.cache.find(hash, |val| val.0 == key) {
            if cache_value.1.is_expired() {
                {
                    to_remove = cache_value.1.expiry;
                }
            }
        }
        if let Some(_expiry) = to_remove {
            guard.remove(&key);
        }

        let entry = guard
            .cache
            .entry(hash, |val| val.0 == key, |val| hasher.hash_one(&val.0));

        match entry {
            Entry::Vacant(e) => {
                let len = values.len();
                e.insert((
                    key,
                    CacheValue {
                        value: RedisData::List(values),
                        expiry: None,
                    },
                ));
                Some(len)
            }
            Entry::Occupied(mut e) => {
                let entry_val = e.get_mut();
                match &mut entry_val.1.value {
                    RedisData::List(list) => {
                        list.extend(values);
                        Some(list.len())
                    }
                    _ => None,
                }
            }
        }
    }
}

pub async fn purge_expired_tasks(db: Arc<Db>, mut shutdown_complete_rx: Receiver<()>) {
    loop {
        let mut scheduled_item = None;
        {
            let guard = db.state.lock().expect("Failed lock DB");
            if let Some(item) = guard.expiration_queue.first() {
                scheduled_item = Some(item.clone());
            };
        }

        if let Some(item) = scheduled_item {
            if item.0 > Instant::now() {
                tokio::select! {
                    _ = tokio::time::sleep_until(item.0) => {},
                    _ = db.alarm.notified() => {
                        continue;
                    },
                    _ = shutdown_complete_rx.recv() => { return },
                }
            }

            {
                let mut state_guard = db.state.lock().expect("Failed to lock DB");
                state_guard.remove(&item.1);
            }
        } else {
            tokio::select! {
                _ = db.alarm.notified() => {
                    continue;
                },
                _ = shutdown_complete_rx.recv() => { return },
            }
        }
    }
}

pub async fn run_trash_janitor(mut rx: mpsc::Receiver<TrashData>) {
    while let Some(i) = rx.recv().await {
        trace!("Janitor is awake now. It will start the cleaning process.");
        drop(i);

        while let Ok(j) = rx.try_recv() {
            drop(j);
        }

        tokio::task::yield_now().await;
    }
}
