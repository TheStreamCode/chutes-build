//! Local-only, non-blocking break and late-night suggestions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WellnessPolicy {
    pub break_after_minutes: u64,
    pub late_hour: u32,
    pub late_minute: u32,
}

impl Default for WellnessPolicy {
    fn default() -> Self {
        Self {
            break_after_minutes: 120,
            late_hour: 22,
            late_minute: 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellnessSuggestion {
    TakeBreak,
    ContinueTomorrow,
}

impl WellnessPolicy {
    pub fn evaluate(
        self,
        session_minutes: u64,
        local_hour: u32,
        local_minute: u32,
    ) -> Option<WellnessSuggestion> {
        if local_hour >= 24 {
            return None;
        }
        let now_minutes = local_hour * 60 + local_minute.min(59);
        let late_minutes = self.late_hour.min(23) * 60 + self.late_minute.min(59);
        if session_minutes >= 60 && now_minutes >= late_minutes {
            return Some(WellnessSuggestion::ContinueTomorrow);
        }
        (session_minutes >= self.break_after_minutes).then_some(WellnessSuggestion::TakeBreak)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_night_has_priority() {
        assert_eq!(
            WellnessPolicy::default().evaluate(130, 23, 0),
            Some(WellnessSuggestion::ContinueTomorrow)
        );
    }
}
