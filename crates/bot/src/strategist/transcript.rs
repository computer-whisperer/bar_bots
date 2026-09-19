//! `strategist.jsonl`: everything the strategist saw, said and set, for post-game analysis.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use serde_json::Value;

pub struct Transcript {
    file: Mutex<File>,
}

impl Transcript {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        Ok(Transcript { file: Mutex::new(File::create(path)?) })
    }

    pub fn record(&self, entry: Value) {
        let mut file = self.file.lock().unwrap();
        let _ = writeln!(file, "{entry}");
    }
}
