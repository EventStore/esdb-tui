use eventstore::{PersistentSubscriptionInfo, RevisionOrPosition};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct PersistentSubscription {
    pub stream_name: String,
    pub group_name: String,
    pub total_items_processed: i64,
    pub connection_count: i64,
    pub last_known_event_position: Option<RevisionOrPosition>,
    pub last_checkpointed_event_position: Option<RevisionOrPosition>,
    pub in_flight_messages: i64,
    pub status: String,
    pub behind_by_message: f32,
    pub behind_by_time: f32,
}

pub struct PersistentSubscriptions {
    pub inner: BTreeMap<String, PersistentSubscription>,
    pub clock: Instant,
    pub last_time: Option<Duration>,
}

impl PersistentSubscriptions {
    pub fn update(&mut self, ps: Vec<PersistentSubscriptionInfo<RevisionOrPosition>>) {
        let now = self.clock.elapsed();
        let prev = self.last_time.unwrap_or(now);

        for p in ps {}
    }
}

impl Default for PersistentSubscriptions {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            clock: Instant::now(),
            last_time: None,
        }
    }
}
