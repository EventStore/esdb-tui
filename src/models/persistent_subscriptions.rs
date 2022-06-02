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
    pub behind_by_messages: i64,
    pub behind_by_time: f64,
    pub average_items_per_second: f64,
}

pub struct PersistentSubscriptions {
    pub inner: BTreeMap<String, PersistentSubscription>,
}

impl PersistentSubscriptions {
    pub fn update(&mut self, ps: Vec<PersistentSubscriptionInfo<RevisionOrPosition>>) {
        for p in ps {
            let key = format!("{}/{}", p.event_source, p.group_name);
            let entry = self.inner.entry(key.clone()).or_default();

            compute_behind_metrics(entry, &p);
            entry.stream_name = p.event_source;
            entry.group_name = p.group_name;
            entry.total_items_processed = p.stats.total_items as i64;
            entry.connection_count = p.connections.len() as i64;
            entry.in_flight_messages = p.stats.total_in_flight_messages as i64;
            entry.status = p.status;
            entry.average_items_per_second = p.stats.average_per_second;
        }
    }

    pub fn list(&self) -> impl Iterator<Item = (&String, &PersistentSubscription)> {
        self.inner.iter()
    }
}

fn compute_behind_metrics(
    current: &mut PersistentSubscription,
    info: &PersistentSubscriptionInfo<RevisionOrPosition>,
) {
    if info.event_source == "$all" {
        current.behind_by_time = -1f64;
        current.last_known_event_position = info
            .stats
            .last_known_position
            .map(RevisionOrPosition::Position);

        current.last_checkpointed_event_position = info
            .stats
            .last_checkpointed_position
            .map(RevisionOrPosition::Position);

        current.behind_by_messages =
            if info.stats.last_known_position == info.stats.last_checkpointed_position {
                0i64
            } else {
                -1i64
            };
    } else {
        current.last_known_event_position = info
            .stats
            .last_known_event_revision
            .map(RevisionOrPosition::Revision);

        current.last_checkpointed_event_position = info
            .stats
            .last_checkpointed_event_revision
            .map(RevisionOrPosition::Revision);

        let last_checkpointed_rev = info
            .stats
            .last_checkpointed_event_revision
            .unwrap_or_default();

        let last_known_event_rev = info.stats.last_known_event_revision.unwrap_or_default();

        current.behind_by_messages =
            (last_known_event_rev as i64 - last_checkpointed_rev as i64) + 1;

        current.behind_by_time =
            ((current.behind_by_messages as f64 / info.stats.average_per_second) * 100f64).round()
                / 100f64;

        if !current.behind_by_time.is_finite() {
            current.behind_by_time = 0f64;
        }
    }
}

impl Default for PersistentSubscriptions {
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}
