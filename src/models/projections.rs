use eventstore::ProjectionStatus;
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Projection {
    pub name: String,
    pub events_processed: i64,
    pub rate: f32,
    pub partitions_cached: i32,
    pub reads_in_progress: i32,
    pub writes_in_progress: i32,
    pub write_queue: i32,
    pub write_queue_checkpoint: i32,
    pub checkpoint_status: String,
    pub position: String,
    pub last_checkpoint: String,
    pub result: String,
    pub state: String,
    pub query: String,
    pub buffered_events: i64,
    pub status: String,
    pub mode: String,
    pub progress: f32,
}

#[derive(Clone)]
pub struct Projections {
    clock: Instant,
    inner: BTreeMap<String, Projection>,
    last_time: Option<Duration>,
    previous: HashMap<String, ProjectionStatus>,
}

impl Projections {
    pub fn update(&mut self, updates: Vec<ProjectionStatus>) {
        let now = self.clock.elapsed();
        let last = self.last_time.unwrap_or(now);

        for update in updates {
            let entry = self.inner.entry(update.name.clone()).or_default();
            if let Some(previous) = self.previous.remove(update.name.as_str()) {
                let events_processed =
                    update.events_processed_after_restart - previous.events_processed_after_restart;
                entry.rate = events_processed as f32 / (now.as_secs_f32() - last.as_secs_f32());
            }

            entry.name = update.name.clone();
            entry.events_processed = update.events_processed_after_restart;
            entry.partitions_cached = update.partitions_cached;
            entry.reads_in_progress = update.reads_in_progress;
            entry.writes_in_progress = update.writes_in_progress;
            entry.write_queue = update.write_pending_events_before_checkpoint;
            entry.write_queue_checkpoint = update.write_pending_events_after_checkpoint;
            entry.checkpoint_status = update.checkpoint_status.clone();
            entry.position = update.position.clone();
            entry.last_checkpoint = update.last_checkpoint.clone();
            entry.buffered_events = update.buffered_events;
            entry.status = update.status.clone();
            entry.mode = update.mode.clone();
            entry.progress = update.progress;

            self.previous.insert(update.name.clone(), update);
        }

        self.last_time = Some(now);
    }

    pub fn list(&self) -> impl Iterator<Item = &Projection> {
        self.inner.values()
    }

    pub fn by_idx(&self, idx: usize) -> Option<&Projection> {
        self.list()
            .enumerate()
            .find(|(i, _)| *i == idx)
            .map(|(_, p)| p)
    }

    pub fn by_idx_mut(&mut self, idx: usize) -> Option<&mut Projection> {
        self.inner
            .values_mut()
            .enumerate()
            .find(|(i, _)| *i == idx)
            .map(|(_, p)| p)
    }

    pub fn count(&self) -> usize {
        self.inner.len()
    }
}

impl Default for Projections {
    fn default() -> Self {
        Self {
            clock: Instant::now(),
            inner: Default::default(),
            last_time: None,
            previous: Default::default(),
        }
    }
}
