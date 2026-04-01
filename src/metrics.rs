use std::time::Duration;

#[derive(Debug, Clone)]
pub struct StreamMetrics {
    pub total_time: Duration,
    pub token_count: usize,
    pub tokens_per_second: f64,
    pub time_per_token_ms: f64,
    pub time_to_first_token: Duration,
    pub all_token_times: Vec<Duration>,
}

impl StreamMetrics {
    pub fn calculate(
        token_count: usize,
        total_time: Duration,
        all_token_times: Vec<Duration>,
    ) -> Self {
        let tokens_per_second = if total_time.as_secs_f64() > 0.0 {
            token_count as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };

        let time_per_token_ms = if token_count > 0 {
            total_time.as_secs_f64() * 1000.0 / token_count as f64
        } else {
            0.0
        };

        let time_to_first_token = all_token_times.first().copied().unwrap_or(Duration::ZERO);

        Self {
            total_time,
            token_count,
            tokens_per_second,
            time_per_token_ms,
            time_to_first_token,
            all_token_times,
        }
    }
}

impl std::fmt::Display for StreamMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "--- Metrics ---")?;
        writeln!(f, "Total time: {:.3}s", self.total_time.as_secs_f64())?;
        writeln!(f, "Token count: {}", self.token_count)?;
        writeln!(f, "Tokens/second: {:.2}", self.tokens_per_second)?;
        writeln!(f, "Time per token: {:.3}ms", self.time_per_token_ms)?;
        writeln!(
            f,
            "Time to first token: {:.3}s",
            self.time_to_first_token.as_secs_f64()
        )?;

        if !self.all_token_times.is_empty() {
            let mut times: Vec<f64> = self
                .all_token_times
                .iter()
                .map(|d| d.as_secs_f64() * 1000.0)
                .collect();
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());

            writeln!(f, "--- Percentiles (ms) ---")?;
            let p50_idx = times.len() / 2;
            let p90_idx = ((times.len() as f64 * 0.9) as usize).min(times.len() - 1);
            let p99_idx = ((times.len() as f64 * 0.99) as usize).min(times.len() - 1);

            writeln!(f, "  p50: {:.1}", times[p50_idx])?;
            writeln!(f, "  p90: {:.1}", times[p90_idx])?;
            writeln!(f, "  p99: {:.1}", times[p99_idx])?;
        }

        Ok(())
    }
}
