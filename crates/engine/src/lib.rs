mod frequency;
mod layout;
mod session;

pub use frequency::Frequencies;
pub use layout::{remap, Layout};
use serde::{Deserialize, Serialize};
pub use session::{Action, Input, Session, Snapshot};

pub const MAX_WORD_CHARS: usize = 64;
pub const POLICY_VERSION: &str = "conservative-v2";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub original: String,
    pub replacement: String,
    pub target: Layout,
}

pub struct Detector {
    frequencies: Frequencies,
}

impl Detector {
    pub fn bundled() -> Result<Self, String> {
        Ok(Self {
            frequencies: Frequencies::bundled()?,
        })
    }

    /// Check a completed, non-whitespace run. Punctuation may belong to either layout.
    /// A correction must have one unambiguous interpretation and at least 3 letters.
    pub fn analyze(&self, run: &str, layout: Layout) -> Option<Candidate> {
        if run.chars().count() > MAX_WORD_CHARS
            || run.chars().any(|c| c.is_numeric() || c.is_whitespace())
            || run.contains(['@', '_', '\\', '#', '=', '+'])
            || run.contains("://")
        {
            return None;
        }
        let mut matches = Vec::new();
        let boundaries: Vec<usize> = run
            .char_indices()
            .map(|(i, _)| i)
            .chain([run.len()])
            .collect();
        // Consider surrounding punctuation, but never split an internal punctuation key.
        for &start in &boundaries {
            if !run[..start].chars().all(|c| can_strip(c, layout)) {
                break;
            }
            for &end in boundaries.iter().rev() {
                if end < start {
                    continue;
                }
                if !run[end..].chars().all(|c| can_strip(c, layout)) {
                    continue;
                }
                let word = &run[start..end];
                if word.chars().count() < 3 {
                    continue;
                }
                let Some(mapped) = remap(word, layout) else {
                    continue;
                };
                if !mapped.chars().all(|c| layout::letter(c, layout.other())) {
                    continue;
                }
                if !normal_case(&mapped) {
                    continue;
                }
                let source = self.frequencies.zipf(word, layout);
                let alternative = self.frequencies.zipf(&mapped, layout.other());
                if source >= 2.0 || alternative < 3.5 || alternative - source < 2.5 {
                    continue;
                }
                // Don't rewrite a known word just because punctuation can be stripped
                // differently in the other layout (e.g. a valid word inside quotes).
                let current_word = run.trim_matches(|c| can_strip(c, layout));
                if self.frequencies.zipf(current_word, layout) >= 2.0 {
                    continue;
                }
                let replacement = format!("{}{}{}", &run[..start], mapped, &run[end..]);
                if !matches.contains(&replacement) {
                    matches.push(replacement);
                }
            }
        }
        if matches.len() != 1 {
            return None;
        }
        Some(Candidate {
            original: run.to_owned(),
            replacement: matches.pop()?,
            target: layout.other(),
        })
    }

    pub fn is_input_character(c: char) -> bool {
        layout::letter_position(c) || surrounding(c)
    }
}

fn surrounding(c: char) -> bool {
    matches!(
        c,
        '.' | ','
            | '!'
            | '?'
            | ':'
            | ';'
            | '\''
            | '"'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '«'
            | '»'
            | '“'
            | '”'
    )
}

fn can_strip(c: char, layout: Layout) -> bool {
    // Never turn an unknown Russian inflection into a different, common stem
    // by interpreting a letter on a punctuation key as surrounding punctuation.
    surrounding(c)
        && !remap(&c.to_string(), layout).is_some_and(|mapped| {
            mapped
                .chars()
                .any(|letter| layout::letter(letter, layout.other()))
        })
}

fn normal_case(word: &str) -> bool {
    word.chars().all(char::is_lowercase)
        || word.chars().all(char::is_uppercase)
        || (word.chars().next().is_some_and(char::is_uppercase)
            && word.chars().skip(1).all(char::is_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn detector() -> &'static Detector {
        static DETECTOR: OnceLock<Detector> = OnceLock::new();
        DETECTOR.get_or_init(|| Detector::bundled().unwrap())
    }

    #[test]
    fn physical_mapping_round_trips_case_and_yo() {
        for text in [
            "привет",
            "Привет",
            "ПРИВЕТ",
            "ёлка",
            "любовь",
            "жизнь",
            "счастье",
        ] {
            let wrong = remap(text, Layout::Russian).unwrap();
            assert_eq!(remap(&wrong, Layout::English).unwrap(), text);
        }
        assert_eq!(remap("руддщ", Layout::Russian).as_deref(), Some("hello"));
    }

    #[test]
    fn detects_both_directions_and_preserves_case() {
        for (wrong, expected, source) in [
            ("ghbdtn", "привет", Layout::English),
            ("Ghbdtn", "Привет", Layout::English),
            ("GHBDTN", "ПРИВЕТ", Layout::English),
            ("руддщ", "hello", Layout::Russian),
            ("Руддщ", "Hello", Layout::Russian),
            ("РУДДЩ", "HELLO", Layout::Russian),
            ("k.,jdm", "любовь", Layout::English),
            ("ghbdtn!", "привет!", Layout::English),
            ("(ghbdtn)", "(привет)", Layout::English),
        ] {
            assert_eq!(
                detector().analyze(wrong, source).map(|c| c.replacement),
                Some(expected.into()),
                "{wrong}"
            );
        }
    }

    #[test]
    fn avoids_valid_ambiguous_and_structured_text() {
        for word in [
            "hello",
            "world",
            "her",
            "cat",
            "MIT",
            "GitHub",
            "CodeSwitch",
            "https://example.com",
            "foo_bar",
            "abc123",
            "x@y.com",
            "a",
            "gh",
            "GHbdtn",
            "hello/world",
        ] {
            assert!(
                detector().analyze(word, Layout::English).is_none(),
                "{word}"
            );
        }
        for word in ["привет", "мир", "кот", "и", "я", "рук"] {
            assert!(
                detector().analyze(word, Layout::Russian).is_none(),
                "{word}"
            );
        }
    }

    #[test]
    fn frequency_bins_and_normalization_match_upstream() {
        let f = Frequencies::bundled().unwrap();
        assert!((f.zipf("the", Layout::English) - 7.73).abs() < 0.02);
        assert_eq!(
            f.zipf("ПРИВЕТ", Layout::Russian),
            f.zipf("привет", Layout::Russian)
        );
        assert_eq!(f.zipf("notarealwordxyz", Layout::English), 0.0);
    }

    #[test]
    fn ambiguous_edge_keys_never_strip_russian_letters() {
        for (wrong, intended) in [
            ("ufhf;f[", "гаражах"),
            ("wtgm.", "цепью"),
            ("[dfkf", "хвала"),
            ("rfxtcndf[", "качествах"),
            ("rjv,byfwbz[", "комбинациях"),
            ("'ktvtynf[", "элементах"),
            ("djpdhfotybz[", "возвращениях"),
            ("j,obyf[", "общинах"),
            ("fggfhfnf[", "аппаратах"),
            ("Vfh'", "Марэ"),
        ] {
            if let Some(candidate) = detector().analyze(wrong, Layout::English) {
                assert_eq!(candidate.replacement, intended);
            }
        }
        for word in ["ghbdtn.", "ghbdtn,", "'ghbdtn'"] {
            assert!(detector().analyze(word, Layout::English).is_none());
        }
    }
}
