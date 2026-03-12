use crate::types::*;
use bytes::Bytes;
use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    mem::take,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::{Notify, broadcast::Receiver, mpsc},
    time::Instant,
};
use tracing::{debug, instrument, trace};

const BUFFER: usize = 1024;

pub enum TrashData {
    Unlinked(Vec<CacheValue>),
    Flushed((HashMap<String, CacheValue>, BTreeSet<(Instant, String)>)),
}

#[derive(Default, Debug)]
struct State {
    cache: HashMap<String, CacheValue>,
    expiration_queue: BTreeSet<(Instant, String)>,
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
            cache: HashMap::new(),
            expiration_queue: BTreeSet::new(),
        }
    }

    fn insert(&mut self, key: String, value: CacheValue) -> bool {
        if let Some(old) = self.cache.get(&key)
            && let Some(old_expiry) = old.expiry
        {
            self.expiration_queue.remove(&(old_expiry, key.clone()));
        }

        if let Some(expiry) = value.expiry {
            self.expiration_queue.insert((expiry, key.clone()));
        }

        let mut notify = false;
        if let Some((_, first_key)) = self.expiration_queue.first() {
            notify = first_key == &key;
        };

        self.cache.insert(key, value);

        notify
    }

    fn remove(&mut self, item: (Instant, String)) {
        if self.expiration_queue.contains(&item) {
            self.expiration_queue.remove(&item);
            self.cache.remove(&item.1);
        }
    }

    fn delete(&mut self, key: &str) -> bool {
        self.cache
            .remove(key)
            .map(|val| {
                if let Some(expiry) = val.expiry {
                    self.expiration_queue.remove(&(expiry, key.to_string()));
                }
                true
            })
            .unwrap_or_default()
    }

    // In Rust, memory is cleaned up (deallocated) the moment a variable's "owner" is finished with it.
    // This is called a Drop. In unlink: You return the CacheValue. This "moves" the ownership out of the
    // State and into the Db::unlink method. You then move it again into a Vec, and finally move it into the trash_chute.
    fn unlink(&mut self, key: &str) -> Option<CacheValue> {
        self.cache.remove(key).inspect(|val| {
            if let Some(expiry) = val.expiry {
                self.expiration_queue.remove(&(expiry, key.to_string()));
            }
        })
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

        let mut to_remove = None;
        let result = if let Some(cache_value) = guard.cache.get(&key) {
            if cache_value.is_expired() {
                to_remove = cache_value.expiry;
                None
            } else {
                Some(cache_value.value.clone())
            }
        } else {
            None
        };

        if let Some(expiry) = to_remove {
            guard.remove((expiry, key));
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
        if let Some(cache_value) = guard.cache.get(&key)
            && cache_value.is_expired()
        {
            to_remove = cache_value.expiry;
        };

        if let Some(expiry) = to_remove {
            guard.remove((expiry, key.clone()));
        }

        let entry = guard.cache.entry(key.clone());

        match entry {
            Entry::Vacant(e) => {
                let len = values.len();
                e.insert(CacheValue {
                    value: RedisData::List(values),
                    expiry: None,
                });
                Some(len)
            }
            Entry::Occupied(mut e) => {
                let entry_val = e.get_mut();
                match &mut entry_val.value {
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
                state_guard.remove(item);
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
