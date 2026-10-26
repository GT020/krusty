//! Live usage samples shared by resource detail views.

#[derive(Clone, Debug)]
pub struct UsageSample {
    pub observed_at: i64,
    pub cpu_used_millicores: Option<u64>,
    pub cpu_capacity_millicores: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_capacity_bytes: Option<u64>,
}

impl UsageSample {
    pub fn cpu_percent(&self) -> Option<f32> {
        let used = self.cpu_used_millicores? as f32;
        let capacity = self.cpu_capacity_millicores? as f32;
        (capacity > 0.0).then(|| (used / capacity * 100.0).clamp(0.0, 100.0))
    }

    pub fn memory_percent(&self) -> Option<f32> {
        let used = self.memory_used_bytes? as f32;
        let capacity = self.memory_capacity_bytes? as f32;
        (capacity > 0.0).then(|| (used / capacity * 100.0).clamp(0.0, 100.0))
    }
}

#[derive(Clone, Debug, Default)]
pub struct UsageHistory {
    samples: Vec<UsageSample>,
    max_samples: usize,
}

impl UsageHistory {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::new(),
            max_samples: max_samples.max(2),
        }
    }

    pub fn push(&mut self, sample: UsageSample) {
        self.samples.push(sample);
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    pub fn samples(&self) -> &[UsageSample] {
        &self.samples
    }

    pub fn latest(&self) -> Option<&UsageSample> {
        self.samples.last()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}
