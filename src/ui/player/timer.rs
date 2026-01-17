use std::time::Instant;

pub struct Timer {
    start_point: Option<Instant>,
    played_us: Option<u64>,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start_point: None,
            played_us: None,
        }
    }

    pub fn start(&mut self) {
        if self.start_point.is_none() {
            self.start_point = Some(Instant::now());
        }
    }

    pub fn stop(&mut self) {
        if let Some(p) = self.start_point.take() {
            self.played_us = Some(p.elapsed().as_micros() as u64 + self.played_us.unwrap_or(0));
        }
    }

    /// pause timer and set time
    pub fn set_time(&mut self, us: u64) {
        self.stop();
        self.played_us = Some(us);
    }

    pub fn set_time_sec(&mut self, sec: f64) {
        self.set_time((sec * 1_000_000.0).round() as u64);
    }

    pub fn current_time_us(&self) -> u64 {
        let played = self.played_us.unwrap_or(0);
        if let Some(p) = self.start_point {
            let elapsed_us = p.elapsed().as_micros().min(u64::MAX as u128) as u64;
            played + elapsed_us
        } else {
            played
        }
    }

    pub fn current_time_sec(&self) -> f64 {
        self.current_time_us() as f64 / 1_000_000.0
    }
}
