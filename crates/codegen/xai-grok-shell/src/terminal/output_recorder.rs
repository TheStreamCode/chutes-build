//! Reconstructs a client-side terminal log from cumulative `terminal/output`
//! snapshots so truncation hints and output monitors work for remote clients.

use std::path::PathBuf;

pub(crate) struct OutputRecorder {
    path: PathBuf,
    last: String,
    overlap_window: usize,
    realign_warned: bool,
    file: Option<tokio::fs::File>,
    overlap_s: Vec<u16>,
    overlap_pi: Vec<u32>,
}

impl OutputRecorder {
    pub(crate) fn new(path: PathBuf, output_byte_limit: usize) -> Self {
        Self {
            path,
            last: String::new(),
            overlap_window: output_byte_limit,
            realign_warned: false,
            file: None,
            overlap_s: Vec::new(),
            overlap_pi: Vec::new(),
        }
    }

    pub(crate) async fn initialize(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Err(error) = tokio::fs::File::create(&self.path).await {
            tracing::debug!(path = %self.path.display(), %error, "output recorder: failed to create log file");
        }
    }

    /// Append only the suffix added by the current cumulative snapshot. When a
    /// remote buffer rolls, realign on its largest overlap with the prior one.
    pub(crate) async fn append(&mut self, current: &str) -> std::io::Result<()> {
        if current.is_empty() || current == self.last {
            return Ok(());
        }
        let new_suffix = match current.strip_prefix(self.last.as_str()) {
            Some(suffix) => suffix,
            None => {
                let overlap = largest_overlap(
                    &self.last,
                    current,
                    self.overlap_window,
                    &mut self.overlap_s,
                    &mut self.overlap_pi,
                );
                if overlap == 0 && !self.last.is_empty() && !self.realign_warned {
                    self.realign_warned = true;
                    tracing::warn!(
                        path = %self.path.display(),
                        "output recorder: snapshots had no overlap; output may contain duplication"
                    );
                }
                &current[overlap..]
            }
        };
        if !new_suffix.is_empty() {
            use tokio::io::AsyncWriteExt as _;
            if self.file.is_none() {
                self.file = Some(
                    tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&self.path)
                        .await?,
                );
            }
            let file = self.file.as_mut().expect("file initialized above");
            if let Err(error) = file.write_all(new_suffix.as_bytes()).await {
                self.file = None;
                return Err(error);
            }
            if let Err(error) = file.flush().await {
                self.file = None;
                return Err(error);
            }
        }
        self.last.clear();
        self.last.push_str(current);
        Ok(())
    }
}

fn largest_overlap(
    last: &str,
    current: &str,
    window: usize,
    storage: &mut Vec<u16>,
    prefix: &mut Vec<u32>,
) -> usize {
    let current_bytes = current.as_bytes();
    let last_bytes = last.as_bytes();
    if current_bytes.is_empty() || last_bytes.is_empty() {
        return 0;
    }
    let tail = &last_bytes[last_bytes.len().saturating_sub(window)..];
    storage.clear();
    storage.extend(current_bytes.iter().map(|byte| u16::from(*byte)));
    // A byte value cannot equal this sentinel, so matches cannot accidentally
    // bridge the pattern/snapshot boundary in the prefix-function table.
    storage.push(256);
    storage.extend(tail.iter().map(|byte| u16::from(*byte)));
    prefix.clear();
    prefix.resize(storage.len(), 0);
    let mut matched = 0u32;
    for index in 1..storage.len() {
        while matched > 0 && storage[index] != storage[matched as usize] {
            matched = prefix[(matched - 1) as usize];
        }
        if storage[index] == storage[matched as usize] {
            matched += 1;
        }
        prefix[index] = matched;
    }
    let cap = current_bytes.len().min(tail.len());
    let mut overlap = prefix[storage.len() - 1] as usize;
    while overlap > cap {
        overlap = prefix[overlap - 1] as usize;
    }
    while overlap > 0 && !current.is_char_boundary(overlap) {
        overlap -= 1;
    }
    overlap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recorder_appends_cumulative_and_rolled_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.log");
        let mut recorder = OutputRecorder::new(path.clone(), 8);
        recorder.initialize().await;
        recorder.append("line1\n").await.unwrap();
        recorder.append("line1\nline2\n").await.unwrap();
        recorder.append("ne2\nline3\n").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "line1\nline2\nline3\n"
        );
    }
}
