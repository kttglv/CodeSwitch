use codeswitch_engine::{Detector, Layout, POLICY_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: String,
    layout: Layout,
    expected: Option<String>,
}
#[derive(Default, Serialize)]
struct Counts {
    true_positive: usize,
    false_positive: usize,
    false_negative: usize,
    negative_cases: usize,
    precision: f64,
    recall: f64,
}
impl Counts {
    fn add(&mut self, expected: Option<&str>, actual: Option<&str>) {
        match (expected, actual) {
            (Some(expected), Some(actual)) if expected == actual => self.true_positive += 1,
            (Some(_), Some(_)) => {
                self.false_positive += 1;
                self.false_negative += 1;
            }
            (Some(_), None) => self.false_negative += 1,
            (None, Some(_)) => {
                self.false_positive += 1;
                self.negative_cases += 1;
            }
            (None, None) => self.negative_cases += 1,
        }
    }
    fn finish(&mut self) {
        self.precision =
            self.true_positive as f64 / (self.true_positive + self.false_positive).max(1) as f64;
        self.recall =
            self.true_positive as f64 / (self.true_positive + self.false_negative).max(1) as f64;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Supply an annotated JSONL corpus")?;
    let detector = Detector::bundled()?;
    let reader = BufReader::new(std::fs::File::open(path)?);
    let mut counts = Counts::default();
    let mut by_layout = BTreeMap::from([
        ("english", Counts::default()),
        ("russian", Counts::default()),
    ]);
    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let case: Case = serde_json::from_str(&line)
            .map_err(|_| format!("Invalid case on line {}", line_number + 1))?;
        let actual = detector.analyze(&case.input, case.layout);
        let actual = actual
            .as_ref()
            .map(|candidate| candidate.replacement.as_str());
        counts.add(case.expected.as_deref(), actual);
        by_layout
            .get_mut(match case.layout {
                Layout::English => "english",
                Layout::Russian => "russian",
            })
            .unwrap()
            .add(case.expected.as_deref(), actual);
    }
    counts.finish();
    for counts in by_layout.values_mut() {
        counts.finish();
    }
    let mut report = serde_json::to_value(counts)?;
    report["policy"] = POLICY_VERSION.into();
    report["by_layout"] = serde_json::to_value(by_layout)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
