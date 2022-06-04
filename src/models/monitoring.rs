#[derive(Default)]
pub struct Monitoring {
    increment: usize,
    pub epoch_numbers: Vec<(f64, f64)>,
}

impl Monitoring {
    pub fn update(&mut self) {
        if self.epoch_numbers.len() > 10 {
            self.epoch_numbers.remove(0);
        }

        let value = rand::random::<i64>() % 20;

        self.epoch_numbers
            .push((self.increment as f64, value as f64));

        self.increment += 2;
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
