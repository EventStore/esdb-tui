#[derive(Default)]
pub struct Stats {
    pub free_mem: usize,   // sys-freeMem
    pub load_avg_1m: f64,  // sys-loadavg-1m
    pub load_avg_5m: f64,  // sys-loadavg-5m
    pub load_avg_15m: f64, // sys-loadavg-15m
    pub disk_usage: f64,   // sys-drive-{path}-usage - $num%
}
