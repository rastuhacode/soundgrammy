//! OS interruption state is independent of user playback intent.
//! Ending an interruption only removes a gate; it never issues a Play command.
#[allow(dead_code)] // Constructed by mobile adapters.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Begin(u64),
    End(u64, bool),
    PermanentLoss,
}

#[derive(Default)]
pub struct Policy {
    interruption: Option<u64>,
    watermark: u64,
}
impl Policy {
    pub fn blocked(&self) -> bool {
        self.interruption.is_some()
    }
    /// Returns true when the OS requires clearing playback intent.
    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Begin(id) if id > self.watermark => {
                self.watermark = id;
                self.interruption = Some(id);
            }
            Event::End(id, resume) if self.interruption == Some(id) => {
                self.interruption = None;
                return !resume;
            }
            Event::PermanentLoss => {
                self.interruption = None;
                return true;
            }
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_and_duplicate_callbacks_cannot_end_a_new_interruption() {
        let mut p = Policy::default();
        p.apply(Event::Begin(1));
        p.apply(Event::Begin(2));
        p.apply(Event::End(1, true));
        assert!(p.blocked());
        assert!(!p.apply(Event::End(2, true)));
        assert!(!p.blocked());
        p.apply(Event::Begin(2));
        assert!(!p.blocked());
    }
    #[test]
    fn resume_never_overrides_user_pause_and_permanent_loss_cancels_resume() {
        let mut p = Policy::default();
        p.apply(Event::Begin(1));
        let desired_after_user_pause = false;
        p.apply(Event::End(1, true));
        assert!(!(desired_after_user_pause && !p.blocked()));
        p.apply(Event::Begin(2));
        assert!(p.apply(Event::PermanentLoss));
        assert!(!p.apply(Event::End(2, true)));
        p.apply(Event::Begin(3));
        assert!(p.apply(Event::End(3, false)));
    }
}
