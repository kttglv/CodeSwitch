use crate::{Candidate, Detector, Layout, MAX_WORD_CHARS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub field: u64,
    pub caret: i64,
    pub suffix: String,
    pub layout: Layout,
    pub revision: u64,
}

impl Snapshot {
    /// Keep caret identity between keystrokes without retaining surrounding text.
    pub fn position(&self) -> Self {
        Self {
            field: self.field,
            caret: self.caret,
            suffix: String::new(),
            layout: self.layout,
            revision: self.revision,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Input {
    Text(String),
    Backspace,
    Reset,
    Undo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub expected: String,
    pub replacement: String,
    pub target: Layout,
    pub undo: bool,
}

impl Action {
    pub fn matches_result(&self, before: &Snapshot, after: &Snapshot) -> bool {
        let Some(prefix) = before.suffix.strip_suffix(&self.expected) else {
            return false;
        };
        after.field == before.field
            && after.revision == before.revision
            && after.caret
                == before.caret - self.expected.encode_utf16().count() as i64
                    + self.replacement.encode_utf16().count() as i64
            && after.suffix == format!("{prefix}{}", self.replacement)
            && after.layout == self.target
    }
}

struct Undo {
    snapshot: Snapshot,
    original: String,
    replacement: String,
    previous_layout: Layout,
}

#[derive(Default)]
pub struct Session {
    word: String,
    previous: Option<Snapshot>,
    undo: Option<Undo>,
    // After a reset, don't rewrite a partial word from an earlier context.
    ignore_run: bool,
}

impl Session {
    pub fn reset(&mut self) {
        self.word.clear();
        self.previous = None;
        self.undo = None;
        self.ignore_run = true;
    }

    pub fn accept(
        &mut self,
        input: Input,
        snapshot: &Snapshot,
        detector: &Detector,
    ) -> Option<Action> {
        if matches!(input, Input::Reset) {
            self.reset();
            return None;
        }
        if matches!(input, Input::Undo) {
            let undo = self.undo.take()?;
            self.word.clear();
            self.previous = None;
            self.ignore_run = true;
            return (undo.snapshot.field == snapshot.field
                && undo.snapshot.caret == snapshot.caret
                && undo.snapshot.layout == snapshot.layout
                && snapshot.suffix.ends_with(&undo.replacement))
            .then_some(Action {
                expected: undo.replacement,
                replacement: undo.original,
                target: undo.previous_layout,
                undo: true,
            });
        }
        self.undo = None;
        let change = match &input {
            Input::Text(text) => text.encode_utf16().count() as i64,
            Input::Backspace => -1,
            _ => 0,
        };
        if let Some(previous) = &self.previous {
            if previous.field != snapshot.field
                || previous.layout != snapshot.layout
                || previous.caret + change != snapshot.caret
                || snapshot.revision != previous.revision + 1
            {
                self.reset();
            }
        } else if let Input::Text(text) = &input {
            // A fresh session may start only at the beginning or a verified whitespace.
            let before = snapshot.suffix.strip_suffix(text).unwrap_or("");
            self.ignore_run = !before.is_empty() && !before.ends_with(char::is_whitespace);
        }
        self.previous = Some(snapshot.position());
        match input {
            Input::Backspace => {
                if self.word.pop().is_none() {
                    self.ignore_run = true;
                }
                if !snapshot.suffix.ends_with(&self.word) {
                    self.reset();
                }
                None
            }
            Input::Text(text) => {
                if text.chars().count() != 1 || !snapshot.suffix.ends_with(&text) {
                    self.reset();
                    return None;
                }
                let c = text.chars().next()?;
                if c == ' ' {
                    let action = if !self.ignore_run
                        && snapshot.suffix.ends_with(&format!("{} ", self.word))
                    {
                        detector
                            .analyze(&self.word, snapshot.layout)
                            .map(|candidate| action(candidate, " "))
                    } else {
                        None
                    };
                    self.word.clear();
                    self.ignore_run = false;
                    return action;
                }
                if c.is_whitespace() {
                    self.reset();
                    return None;
                }
                if self.ignore_run {
                    return None;
                }
                self.word.push(c);
                if self.word.chars().count() > MAX_WORD_CHARS || !Detector::is_input_character(c) {
                    self.word.clear();
                    self.ignore_run = true;
                    return None;
                }
                if !snapshot.suffix.ends_with(&self.word) {
                    self.reset();
                    return None;
                }
                // A punctuation key may itself be a Russian letter. The detector
                // considers both interpretations and only returns a unique candidate.
                let maps_to_letter =
                    crate::remap(&c.to_string(), snapshot.layout).is_some_and(|mapped| {
                        mapped
                            .chars()
                            .any(|letter| crate::layout::letter(letter, snapshot.layout.other()))
                    });
                if !c.is_alphabetic() && !maps_to_letter {
                    detector
                        .analyze(&self.word, snapshot.layout)
                        .map(|candidate| action(candidate, ""))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn committed(&mut self, before: &Snapshot, after: &Snapshot, action: &Action) {
        self.word.clear();
        self.previous = Some(after.position());
        self.ignore_run = !action.replacement.ends_with(' ');
        self.undo = (!action.undo).then(|| Undo {
            snapshot: after.position(),
            original: action.expected.clone(),
            replacement: action.replacement.clone(),
            previous_layout: before.layout,
        });
    }
}

fn action(candidate: Candidate, delimiter: &str) -> Action {
    Action {
        expected: format!("{}{delimiter}", candidate.original),
        replacement: format!("{}{delimiter}", candidate.replacement),
        target: candidate.target,
        undo: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_word(
        session: &mut Session,
        detector: &Detector,
        word: &str,
    ) -> (Snapshot, Option<Action>) {
        let mut snapshot = Snapshot {
            field: 1,
            caret: 0,
            suffix: String::new(),
            layout: Layout::English,
            revision: 0,
        };
        let mut action = None;
        for c in word.chars() {
            snapshot.suffix.push(c);
            snapshot.caret += c.len_utf16() as i64;
            snapshot.revision += 1;
            action = session.accept(Input::Text(c.to_string()), &snapshot, detector);
        }
        (snapshot, action)
    }

    #[test]
    fn correction_and_immediate_undo_restore_layout() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (before, action) = type_word(&mut session, &detector, "ghbdtn ");
        let action = action.unwrap();
        assert_eq!(action.replacement, "привет ");
        let after = Snapshot {
            suffix: "привет ".into(),
            layout: Layout::Russian,
            ..before.clone()
        };
        session.committed(&before, &after, &action);
        assert!(session.previous.as_ref().unwrap().suffix.is_empty());
        assert!(session.undo.as_ref().unwrap().snapshot.suffix.is_empty());
        let undo = session.accept(Input::Undo, &after, &detector).unwrap();
        assert_eq!(undo.replacement, "ghbdtn ");
        assert_eq!(undo.target, Layout::English);
    }

    #[test]
    fn focus_changes_navigation_and_submission_cancel() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (mut state, _) = type_word(&mut session, &detector, "ghbdtn");
        state.field = 2;
        state.caret += 1;
        state.revision += 1;
        state.suffix.push(' ');
        assert!(session
            .accept(Input::Text(" ".into()), &state, &detector)
            .is_none());
        for submit in ["ghbdtn\n", "ghbdtn\t"] {
            assert!(type_word(&mut Session::default(), &detector, submit)
                .1
                .is_none());
        }
    }

    #[test]
    fn backspace_updates_buffer_and_revision_gaps_cancel() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (mut state, _) = type_word(&mut session, &detector, "ghbdtnx");
        state.suffix.pop();
        state.caret -= 1;
        state.revision += 1;
        session.accept(Input::Backspace, &state, &detector);
        state.suffix.push(' ');
        state.caret += 1;
        state.revision += 1;
        assert!(session
            .accept(Input::Text(" ".into()), &state, &detector)
            .is_some());
        let mut session = Session::default();
        let (mut state, _) = type_word(&mut session, &detector, "ghbdtn");
        state.suffix.push(' ');
        state.caret += 1;
        state.revision += 2;
        assert!(session
            .accept(Input::Text(" ".into()), &state, &detector)
            .is_none());
    }

    #[test]
    fn result_verification_rejects_stale_or_partial_edits() {
        let before = Snapshot {
            field: 5,
            caret: 12,
            suffix: "text ghbdtn ".into(),
            layout: Layout::English,
            revision: 20,
        };
        let action = Action {
            expected: "ghbdtn ".into(),
            replacement: "привет ".into(),
            target: Layout::Russian,
            undo: false,
        };
        let after = Snapshot {
            suffix: "text привет ".into(),
            layout: Layout::Russian,
            ..before.clone()
        };
        assert!(action.matches_result(&before, &after));
        for bad in [
            Snapshot {
                field: 6,
                ..after.clone()
            },
            Snapshot {
                revision: 21,
                ..after.clone()
            },
            Snapshot {
                caret: 11,
                ..after.clone()
            },
            Snapshot {
                suffix: "oops привет ".into(),
                ..after.clone()
            },
            Snapshot {
                layout: Layout::English,
                ..after.clone()
            },
        ] {
            assert!(!action.matches_result(&before, &bad));
        }
    }

    #[test]
    fn ambiguous_punctuation_waits_for_the_completed_run() {
        let detector = Detector::bundled().unwrap();
        for run in ["k.,", "ghbdtn.", "ghbdtn,"] {
            assert!(type_word(&mut Session::default(), &detector, run)
                .1
                .is_none());
        }
        assert_eq!(
            type_word(&mut Session::default(), &detector, "k.,jdm ")
                .1
                .unwrap()
                .replacement,
            "любовь "
        );
        assert_eq!(
            type_word(&mut Session::default(), &detector, "ghbdtn!")
                .1
                .unwrap()
                .replacement,
            "привет!"
        );
    }
}
