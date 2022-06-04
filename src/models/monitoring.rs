use eventstore::operations::MemberInfo;

#[derive(Default)]
pub struct Monitoring {
    increment: usize,
    last_epoch_number: Option<i64>,
    last_writer_checkpoint: Option<i64>,
    pub writer_checkpoints: Vec<(f64, f64)>,
}

impl Monitoring {
    pub fn update(&mut self, gossip: Vec<MemberInfo>) {
        if self.writer_checkpoints.len() > 10 {
            self.writer_checkpoints.remove(0);
        }

        if let Some(leader) = find_leader(&gossip) {
            self.last_writer_checkpoint = Some(leader.writer_checkpoint);
            self.writer_checkpoints
                .push((self.increment as f64, leader.writer_checkpoint as f64));
        } else {
            if let Some(last_writer) = self.last_writer_checkpoint {
                self.writer_checkpoints
                    .push((self.increment as f64, last_writer as f64));
            }
        }

        self.increment += 2;
    }

    pub fn writer_checkpoint_value_bounds(&self) -> [f64; 2] {
        let mut low = f64::MAX;
        let mut high = f64::MIN;

        for (_, value) in self.writer_checkpoints.iter() {
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
        if self.increment <= 20 {
            return [0usize, 20usize];
        }

        let low = self.increment - 20;
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
