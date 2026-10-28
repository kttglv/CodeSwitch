use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    English,
    Russian,
}

impl Layout {
    pub fn other(self) -> Self {
        match self {
            Self::English => Self::Russian,
            Self::Russian => Self::English,
        }
    }
}

// Physical ANSI/ISO letter positions, including Russian letters on punctuation keys.
// The shifted rows are separate: Russian punctuation is not English punctuation.
const EN: &str = "`qwertyuiop[]asdfghjkl;'zxcvbnm,./";
const RU: &str = "ёйцукенгшщзхъфывапролджэячсмитьбю.";
const EN_SHIFT: &str = "~QWERTYUIOP{}ASDFGHJKL:\"ZXCVBNM<>?";
const RU_SHIFT: &str = "ЁЙЦУКЕНГШЩЗХЪФЫВАПРОЛДЖЭЯЧСМИТЬБЮ,";

pub fn remap(text: &str, from: Layout) -> Option<String> {
    let (source, destination, shifted_source, shifted_destination) = match from {
        Layout::English => (EN, RU, EN_SHIFT, RU_SHIFT),
        Layout::Russian => (RU, EN, RU_SHIFT, EN_SHIFT),
    };
    text.chars()
        .map(|c| {
            source
                .chars()
                .position(|key| key == c)
                .and_then(|i| destination.chars().nth(i))
                .or_else(|| {
                    shifted_source
                        .chars()
                        .position(|key| key == c)
                        .and_then(|i| shifted_destination.chars().nth(i))
                })
        })
        .collect()
}

pub fn letter(c: char, layout: Layout) -> bool {
    match layout {
        Layout::English => c.is_ascii_alphabetic(),
        Layout::Russian => matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё'),
    }
}

pub fn letter_position(c: char) -> bool {
    EN.chars().chain(EN_SHIFT.chars()).any(|key| key == c)
        || RU.chars().chain(RU_SHIFT.chars()).any(|key| key == c)
}
