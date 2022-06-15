use eventstore::operations::{MemberInfo, ServerVersion, VNodeState};
use eventstore_extras::stats::{Drive, Statistics};
use uuid::Uuid;

pub struct Leader {
    instance_id: Uuid,
    epoch_number: i64,
    writer_checkpoint: i64,
}

const GRAPH_TIME_LIMIT: usize = 20;

#[derive(Default)]
pub struct Monitoring {
    increment: usize,
    pub last_epoch_number: Option<i64>,
    pub last_writer_checkpoint: Option<i64>,
    pub writer_checkpoints: Vec<(f64, f64)>,
    pub cpu_load: Vec<(f64, f64)>,
    pub bytes_written: Vec<(f64, f64)>,
    pub leader: Option<Leader>,
    pub out_of_sync_cluster_counter: usize,
    pub truncation_counter: usize,
    pub elections: usize,
    pub no_leader_counter: usize,
    pub free_mem: f64,
    pub unresponsive_nodes: usize,
    pub drive: Option<Drive>,
    pub server_version: ServerVersion,
    pub last_bytes_written: Option<i64>,
}

impl Monitoring {
    pub fn update(&mut self, stats: Statistics, gossip: Vec<MemberInfo>) {
        self.cpu_load.push((self.increment as f64, stats.proc.cpu));
        self.free_mem = stats.sys.free_mem as f64 / 1_073_741_824f64;
        self.unresponsive_nodes = gossip.iter().filter(|m| !m.is_alive).count();
        self.drive = stats.sys.drive;

        if let Some(last_bytes_written) = self.last_bytes_written.as_mut() {
            let diff = stats.proc.disk_io.written_bytes - *last_bytes_written;
            self.bytes_written.push((
                self.increment as f64,
                diff as f64 / (self.increment + 2 - self.increment) as f64,
            ));

            *last_bytes_written = stats.proc.disk_io.written_bytes;
        } else {
            self.bytes_written.push((self.increment as f64, 0f64));
            self.last_bytes_written = Some(stats.proc.disk_io.written_bytes);
        }

        if let Some(leader) = find_leader(&gossip) {
            self.leader = Some(Leader {
                instance_id: leader.instance_id,
                epoch_number: leader.epoch_number,
                writer_checkpoint: leader.writer_checkpoint,
            });

            if let Some(last_epoch) = self.last_epoch_number {
                if last_epoch != leader.epoch_number {
                    self.elections += 1;
                }
            }

            if let Some(last_chk) = self.last_writer_checkpoint {
                let out_of_sync_count = gossip
                    .iter()
                    .filter(|m| m.state == VNodeState::Follower)
                    .filter(|m| m.writer_checkpoint < last_chk)
                    .count();

                if out_of_sync_count > 1 {
                    self.out_of_sync_cluster_counter += 1;
                }

                if last_chk > leader.writer_checkpoint {
                    self.truncation_counter += 1;
                }
            }

            self.last_writer_checkpoint = Some(leader.writer_checkpoint);
            self.last_epoch_number = Some(leader.epoch_number);
        } else {
            self.no_leader_counter += 1;
        }

        self.increment += 2;

        if self.cpu_load.len() >= GRAPH_TIME_LIMIT {
            self.cpu_load.remove(0);
        }

        if self.bytes_written.len() >= GRAPH_TIME_LIMIT {
            self.bytes_written.remove(0);
        }
    }

    pub fn bytes_written_value_bounds(&self) -> [f64; 2] {
        let mut low = f64::MAX;
        let mut high = f64::MIN;

        for (_, value) in self.bytes_written.iter() {
            if *value < low {
                low = *value;
            }

            if *value > high {
                high = *value;
            }
        }

        [low, high]
    }

    pub fn time_bounds(&self) -> [usize; 2] {
        if self.increment <= GRAPH_TIME_LIMIT {
            return [0usize, GRAPH_TIME_LIMIT];
        }

        let low = self.increment - GRAPH_TIME_LIMIT;
        let high = self.increment;

        [low, high]
    }

    pub fn time_period(&self) -> [f64; 2] {
        let bounds = self.time_bounds();

        [bounds[0] as f64, bounds[1] as f64]
    }
}

fn find_leader(members: &Vec<MemberInfo>) -> Option<&MemberInfo> {
    members
        .iter()
        .find(|m| m.state == eventstore::operations::VNodeState::Leader)
}
