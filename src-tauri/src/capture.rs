//! Clipboard transition bookkeeping, tested without touching the system clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureKey {
    Text(String),
    Image(String),
}

#[derive(Default)]
pub struct CaptureState {
    armed: bool,
    sequence: Option<u64>,
    previous: Option<CaptureKey>,
}
impl CaptureState {
    pub fn should_read(&mut self, sequence: Option<u64>) -> bool {
        if let Some(sequence) = sequence {
            if !self.armed {
                // Opening/resuming never imports whatever was copied before consent.
                self.armed = true;
                self.sequence = Some(sequence);
                return false;
            }
            if self.sequence == Some(sequence) {
                return false;
            }
        }
        true
    }
    pub fn needs_capture(&self, key: &Option<CaptureKey>) -> bool {
        self.armed && key.is_some() && self.previous != *key
    }
    pub fn accept(&mut self, sequence: Option<u64>, key: Option<CaptureKey>) {
        self.armed = true;
        self.sequence = sequence;
        self.previous = key;
    }
    pub fn skip_invalid(&mut self, sequence: Option<u64>) {
        // Don't decode/encode an oversized image every poll. Retry only after a new copy.
        self.sequence = sequence;
        self.previous = None;
    }
    pub fn copied(&mut self, key: CaptureKey) {
        self.previous = Some(key);
        // Re-read once: another program may replace our copy before the sequence is read.
        self.sequence = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequence_baseline_retry_wrap_and_copy_suppression() {
        let mut state = CaptureState::default();
        assert!(!state.should_read(Some(14)));
        assert!(!state.should_read(Some(14)));
        let image = Some(CaptureKey::Image("test-image".into()));
        assert!(state.should_read(Some(15)));
        assert!(
            state.should_read(Some(15)),
            "failed reads must remain retryable"
        );
        assert!(state.needs_capture(&image));
        state.accept(Some(15), image.clone());
        assert!(!state.should_read(Some(15)));
        assert!(
            state.should_read(Some(0)),
            "sequence wrap is still a change"
        );
        assert!(!state.needs_capture(&image));
        state.accept(Some(0), None);
        assert!(
            state.needs_capture(&image),
            "image after clearing is a new copy"
        );
        state.copied(image.unwrap());
        assert!(state.should_read(Some(3)));
        assert!(!state.needs_capture(&Some(CaptureKey::Image("test-image".into()))));
        assert!(state.needs_capture(&Some(CaptureKey::Text("test-image".into()))));
        state.skip_invalid(Some(3));
        assert!(!state.should_read(Some(3)));
        assert!(state.should_read(Some(4)));
    }
    #[test]
    fn fallback_reads_initial_baseline_but_only_captures_later_changes() {
        let mut state = CaptureState::default();
        let text = Some(CaptureKey::Text("original fixture".into()));
        assert!(state.should_read(None));
        assert!(!state.needs_capture(&text));
        state.accept(None, text.clone());
        assert!(state.should_read(None));
        assert!(!state.needs_capture(&text));
        assert!(state.needs_capture(&Some(CaptureKey::Image("fixture".into()))));
        state = CaptureState::default();
        assert!(
            !state.should_read(Some(5)),
            "resume also establishes a fresh baseline"
        );
    }
}
