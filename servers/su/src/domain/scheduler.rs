
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Sha256, Digest};
use dashmap::DashMap;
use tokio::sync::Mutex;
use base64_url;

use crate::domain::clients::store::{StoreClient};
use crate::domain::core::dal::{ScheduleProvider, Log};
use crate::config::Config;

pub struct SchedulerDeps {
    pub data_store: Arc<StoreClient>,
    pub logger: Arc<dyn Log>,
    pub config: Arc<Config>
}

/*
    information used to build a proper item
    in the schedule aka the proper tags
*/
pub struct ScheduleInfo {
    pub epoch: i32,
    pub nonce: i32,
    pub timestamp: i64,
    pub hash_chain: String,
}

pub type LockedScheduleInfo = Arc<Mutex<ScheduleInfo>>;

/*
    ProcessScheduler provides a Mutex lock per process to 
    ensure there are no conflicts or missing nonces in the sequence
*/
pub struct ProcessScheduler {
    /*
        utilize DashMap to avoid locking up the 
        top level data structure
    */
    locks: Arc<DashMap<String, LockedScheduleInfo>>,
    deps: Arc<SchedulerDeps>
}

impl ProcessScheduler {
    pub fn new(deps: Arc<SchedulerDeps>) -> Self {
        ProcessScheduler {
            locks: Arc::new(DashMap::new()),
            deps
        }
    }

    /*
        acquire the lock while also obtaining
        the info needed epoch, nonce etc.. to 
        build a valid item in the schedule
    */
    pub async fn acquire_lock(&self, id: String, message_id: Option<String>) -> Result<LockedScheduleInfo, String> {
        let locked_schedule_info = {
            self.locks.entry(id.clone()).or_insert_with(|| {
                Arc::new(Mutex::new(ScheduleInfo {
                    epoch: 0,
                    nonce: 0,
                    timestamp: 0,
                    hash_chain: String::new(),
                }))
            }).value().clone() // Clone the Arc here
        };

        // Update the ScheduleInfo in a separate scope to ensure the lock is released
        {
            let mut schedule_info = locked_schedule_info.lock().await;
            let (current_epoch, current_nonce, current_hash_chain, current_timestamp) = match fetch_values(self.deps.clone(), &id, message_id).await {
                Ok(vals) => vals,
                Err(e) => return Err(format!("error acquiring scheduler lock {}", e)),
            };
            schedule_info.epoch = current_epoch;
            schedule_info.nonce = current_nonce;
            schedule_info.hash_chain = current_hash_chain;
            schedule_info.timestamp = current_timestamp; 
        } // The lock is released here

        Ok(locked_schedule_info)
    }
}

fn gen_hash_chain(previous_or_seed: &str, message_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous_or_seed);
    hasher.update(message_id);
    let result = hasher.finalize();
    base64_url::encode(&result)
}

/*
    retrieve the epoch, nonce, hash_chain and timestamp
    increment the values here because this wont be called 
    again until the lock is released.
*/
async fn fetch_values(deps: Arc<SchedulerDeps>, process_id: &String, message_id_in: Option<String>) -> Result<(i32, i32, String, i64), String> {

    let start_time = SystemTime::now();
    let duration = match start_time.duration_since(UNIX_EPOCH) {
        Ok(d) => d,
        Err(e) => return Err(format!("{:?}", e)),
    };
    let millis: i64 = duration.as_secs() as i64 * 1000 + i64::from(duration.subsec_millis());

    match message_id_in {
        Some(message_id) => {
            let latest_message = match deps.data_store.get_latest_message(&message_id) {
                Ok(m) => m,
                Err(e) => return Err(format!("{:?}", e)),
            };
            match latest_message {
                Some(previous_message) => {
                    let epoch = previous_message.epoch;
                    let nonce = previous_message.nonce + 1;
                    let hash_chain = gen_hash_chain(&previous_message.hash_chain, &message_id);
                    Ok((epoch, nonce, hash_chain, millis))
                },
                None => {
                    let hash_chain = gen_hash_chain(&process_id, &message_id);
                    Ok((0, 0, hash_chain, millis))
                }
            }
        },
        // were just using this code to get a timestamp for a process
        None => Ok((0, 0, "wont be used".to_string(), millis))
    }
}

impl ScheduleProvider for ScheduleInfo {
    fn epoch(&self) -> String {
        self.epoch.to_string()
    }

    fn nonce(&self) -> String {
        self.nonce.to_string()
    }

    fn timestamp(&self) -> String {
        self.timestamp.to_string()
    }

    fn hash_chain(&self) -> String {
        self.hash_chain.to_string()
    }
}