// SPDX-License-Identifier: Apache-2.0
// Copyright 2022 Robyn Speer; 2026 CodeSwitch contributors.
// Modified: Rust port restricted to English/Russian single words; direct Zipf bins.
//! English/Russian single-token cBpack lookup, based on wordfreq by Robyn Speer.
//! See resources/wordfreq for Apache-2.0 code and CC-BY-SA-4.0 data notices.
use crate::Layout;
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize)]
#[serde(untagged)]
enum Entry {
    Header { format: String, version: u32 },
    Words(Vec<String>),
}

pub struct Frequencies {
    en: HashMap<String, f32>,
    ru: HashMap<String, f32>,
}

impl Frequencies {
    pub fn bundled() -> Result<Self, String> {
        Ok(Self {
            en: decode(include_bytes!(
                "../../../resources/wordfreq/large_en.msgpack.gz"
            ))?,
            ru: decode(include_bytes!(
                "../../../resources/wordfreq/large_ru.msgpack.gz"
            ))?,
        })
    }

    pub fn zipf(&self, word: &str, layout: Layout) -> f32 {
        // v1 accepts only ASCII or Russian single words, so the upstream tokenizer's
        // multi-token, numeric, transliteration and CJK rules are deliberately unused.
        let normalized: String = word.nfkc().flat_map(char::to_lowercase).collect();
        let table = match layout {
            Layout::English => &self.en,
            Layout::Russian => &self.ru,
        };
        *table.get(&normalized).unwrap_or(&0.0)
    }
}

fn decode(bytes: &[u8]) -> Result<HashMap<String, f32>, String> {
    let entries: Vec<Entry> = rmp_serde::from_read(GzDecoder::new(bytes))
        .map_err(|e| format!("Invalid bundled frequency data: {e}"))?;
    if !matches!(entries.first(), Some(Entry::Header { format, version: 1 }) if format == "cB") {
        return Err("Unsupported bundled frequency data format".into());
    }
    let mut words = HashMap::new();
    for (bin, entry) in entries.into_iter().skip(1).enumerate() {
        let Entry::Words(tokens) = entry else {
            return Err("Unexpected frequency header".into());
        };
        let zipf = 9.0 - bin as f32 / 100.0;
        for word in tokens {
            words.insert(word, zipf);
        }
    }
    Ok(words)
}
