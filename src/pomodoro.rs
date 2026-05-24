//! Lightweight pomodoro/focus timer.
//!
//! The timer is intentionally minimal: it tracks a start instant, a duration,
//! and which task it's bound to. The UI shows remaining time in the bottom bar.
//! When the timer elapses it just stays in a "done" state until the user
//! dismisses it; there are no notifications or side effects (yet).

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Pomodoro {
    /// Task this timer is bound to. Surfaced for future per-task summaries; the
    /// current UI only shows the remaining time.
    #[allow(dead_code)]
    pub task_id: i64,
    pub started_at: Instant,
    pub duration: Duration,
}

impl Pomodoro {
    pub fn new(task_id: i64, minutes: u64) -> Self {
        Self {
            task_id,
            started_at: Instant::now(),
            duration: Duration::from_secs(minutes * 60),
        }
    }

    pub fn remaining(&self) -> Duration {
        self.duration.saturating_sub(self.started_at.elapsed())
    }

    pub fn is_done(&self) -> bool {
        self.remaining().is_zero()
    }

    pub fn remaining_str(&self) -> String {
        let r = self.remaining().as_secs();
        let m = r / 60;
        let s = r % 60;
        format!("{m:02}:{s:02}")
    }
}
