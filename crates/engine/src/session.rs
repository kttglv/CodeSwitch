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
    /// Convert the word before the caret on request, without confidence checks.
    Convert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// Chosen by the detector after a completed word.
    Automatic,
    /// Requested with the convert shortcut.
    Manual,
    /// Restores the previous correction; `automatic` tells whether the detector made it.
    Undo { automatic: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub expected: String,
    pub replacement: String,
    pub target: Layout,
    pub kind: ActionKind,
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
    automatic: bool,
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
                kind: ActionKind::Undo {
                    automatic: undo.automatic,
                },
            });
        }
        if matches!(input, Input::Convert) {
            // Converted text comes from the verified snapshot, not the typing buffer,
            // so the buffer is discarded whether or not the conversion is applied.
            self.reset();
            return convert(&snapshot.suffix, snapshot.layout);
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
        let automatic = action.kind == ActionKind::Automatic;
        self.undo = (!matches!(action.kind, ActionKind::Undo { .. })).then(|| Undo {
            snapshot: after.position(),
            original: action.expected.clone(),
            replacement: action.replacement.clone(),
            previous_layout: before.layout,
            automatic,
        });
    }
}

fn action(candidate: Candidate, delimiter: &str) -> Action {
    Action {
        expected: format!("{}{delimiter}", candidate.original),
        replacement: format!("{}{delimiter}", candidate.replacement),
        target: candidate.target,
        kind: ActionKind::Automatic,
    }
}

/// Physically remap the run before the caret (plus at most one trailing space).
/// Direction follows the run's letters, so text typed in either layout converts.
fn convert(suffix: &str, current: Layout) -> Option<Action> {
    let (body, delimiter) = match suffix.strip_suffix(' ') {
        Some(body) => (body, " "),
        None => (suffix, ""),
    };
    let run = &body[body
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8())..];
    if run.is_empty() || run.chars().count() > MAX_WORD_CHARS {
        return None;
    }
    let source = if run
        .chars()
        .any(|c| crate::layout::letter(c, Layout::Russian))
    {
        Layout::Russian
    } else if run
        .chars()
        .any(|c| crate::layout::letter(c, Layout::English))
    {
        Layout::English
    } else {
        current
    };
    let target = source.other();
    let mut mapped_letter = false;
    let replacement: String = run
        .chars()
        .map(|c| match crate::remap(&c.to_string(), source) {
            Some(mapped) => {
                mapped_letter |= mapped.chars().any(|m| crate::layout::letter(m, target));
                mapped
            }
            None => c.to_string(),
        })
        .collect();
    mapped_letter.then(|| Action {
        expected: format!("{run}{delimiter}"),
        replacement: format!("{replacement}{delimiter}"),
        target,
        kind: ActionKind::Manual,
    })
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

    fn snapshot(suffix: &str, layout: Layout, revision: u64) -> Snapshot {
        Snapshot {
            field: 1,
            caret: suffix.encode_utf16().count() as i64,
            suffix: suffix.into(),
            layout,
            revision,
        }
    }

    #[test]
    fn undo_reports_whether_it_reverts_an_automatic_correction() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (before, action) = type_word(&mut session, &detector, "ghbdtn ");
        let action = action.unwrap();
        assert_eq!(action.kind, ActionKind::Automatic);
        let after = Snapshot {
            suffix: "привет ".into(),
            layout: Layout::Russian,
            ..before.clone()
        };
        session.committed(&before, &after, &action);
        let undo = session.accept(Input::Undo, &after, &detector).unwrap();
        assert_eq!(undo.kind, ActionKind::Undo { automatic: true });

        let mut session = Session::default();
        let before = snapshot("ghb", Layout::English, 4);
        let manual = session.accept(Input::Convert, &before, &detector).unwrap();
        let after = Snapshot {
            suffix: "при".into(),
            layout: Layout::Russian,
            ..before.clone()
        };
        session.committed(&before, &after, &manual);
        let undo = session.accept(Input::Undo, &after, &detector).unwrap();
        assert_eq!(undo.kind, ActionKind::Undo { automatic: false });
        assert_eq!(undo.replacement, "ghb");
    }

    #[test]
    fn convert_rewrites_the_last_word_regardless_of_confidence() {
        let detector = Detector::bundled().unwrap();
        // (text before the caret, current layout, expected, replacement, target)
        for (suffix, layout, expected, replacement, target) in [
            // Too short for automatic correction, but the user asked for it.
            ("ghb", Layout::English, "ghb", "при", Layout::Russian),
            // One trailing space belongs to the word that was just completed.
            (
                "hello ghb ",
                Layout::English,
                "ghb ",
                "при ",
                Layout::Russian,
            ),
            ("ytn", Layout::English, "ytn", "нет", Layout::Russian),
            ("Руддщ", Layout::Russian, "Руддщ", "Hello", Layout::English),
            // Direction follows the characters, not the current layout.
            ("руддщ", Layout::English, "руддщ", "hello", Layout::English),
            // Punctuation keys convert physically; other characters stay.
            (
                "k.,jdm",
                Layout::English,
                "k.,jdm",
                "любовь",
                Layout::Russian,
            ),
            (
                "ghbdtn!",
                Layout::English,
                "ghbdtn!",
                "привет!",
                Layout::Russian,
            ),
        ] {
            let action = Session::default()
                .accept(Input::Convert, &snapshot(suffix, layout, 9), &detector)
                .unwrap_or_else(|| panic!("{suffix}"));
            assert_eq!(action.expected, expected, "{suffix}");
            assert_eq!(action.replacement, replacement, "{suffix}");
            assert_eq!(action.target, target, "{suffix}");
            assert_eq!(action.kind, ActionKind::Manual);
        }
    }

    #[test]
    fn convert_ignores_empty_or_letterless_runs() {
        let detector = Detector::bundled().unwrap();
        for suffix in ["", " ", "hello  ", "123", "?!", "line\n"] {
            assert!(
                Session::default()
                    .accept(
                        Input::Convert,
                        &snapshot(suffix, Layout::English, 2),
                        &detector
                    )
                    .is_none(),
                "{suffix:?}"
            );
        }
        let long = "f".repeat(crate::MAX_WORD_CHARS + 1);
        assert!(Session::default()
            .accept(
                Input::Convert,
                &snapshot(&long, Layout::English, 2),
                &detector
            )
            .is_none());
    }

    #[test]
    fn convert_replaces_a_pending_undo_and_cancels_the_word_buffer() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (state, _) = type_word(&mut session, &detector, "ghbdt");
        let convert = Snapshot {
            revision: state.revision + 1,
            ..state.clone()
        };
        let action = session.accept(Input::Convert, &convert, &detector).unwrap();
        assert_eq!(action.replacement, "приве");
        // Without a commit (e.g. verification failed), typing must not resume the old run.
        let mut next = convert.clone();
        next.suffix.push_str("n ");
        next.caret += 2;
        next.revision += 1;
        assert!(session
            .accept(
                Input::Text("n".into()),
                &Snapshot {
                    suffix: "ghbdtn".into(),
                    caret: next.caret - 1,
                    ..next.clone()
                },
                &detector
            )
            .is_none());
        next.revision += 1;
        assert!(session
            .accept(Input::Text(" ".into()), &next, &detector)
            .is_none());
    }

    #[test]
    fn caret_jump_mid_word_cancels_correction() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (mut state, _) = type_word(&mut session, &detector, "ghbdtn");
        state.caret -= 3; // e.g. a click that the hook did not see
        state.revision += 1;
        state.suffix = "ghb ".into();
        assert!(session
            .accept(Input::Text(" ".into()), &state, &detector)
            .is_none());
    }

    #[test]
    fn layout_change_mid_word_cancels_correction() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (mut state, _) = type_word(&mut session, &detector, "ghbdtn");
        state.layout = Layout::Russian;
        state.suffix.push(' ');
        state.caret += 1;
        state.revision += 1;
        assert!(session
            .accept(Input::Text(" ".into()), &state, &detector)
            .is_none());
    }

    #[test]
    fn typing_after_a_correction_disables_its_undo() {
        let detector = Detector::bundled().unwrap();
        let mut session = Session::default();
        let (before, action) = type_word(&mut session, &detector, "ghbdtn ");
        let action = action.unwrap();
        let mut after = Snapshot {
            suffix: "привет ".into(),
            layout: Layout::Russian,
            ..before.clone()
        };
        session.committed(&before, &after, &action);
        after.suffix.push('и');
        after.caret += 1;
        after.revision += 1;
        session.accept(Input::Text("и".into()), &after, &detector);
        assert!(session.accept(Input::Undo, &after, &detector).is_none());
    }

    #[test]
    fn word_keys_ignore_case_and_surrounding_punctuation() {
        assert_eq!(crate::word_key("(Ghbdtn)! "), "ghbdtn");
        assert_eq!(crate::word_key("«Привет»"), "привет");
        assert_eq!(crate::word_key("k.,jdm"), "k.,jdm");
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
            kind: ActionKind::Automatic,
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
