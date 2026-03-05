//! Tree recorder: optional per-node recording for JSONL output.
//!
//! When disabled (None passed to search), no overhead is incurred.
//! When enabled, every visited node is stored in memory, then flushed as
//! proper JSONL (one complete JSON object per line) when `flush()` is called.

use crate::moves::Move;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// A single recorded node in the search tree.
#[derive(Debug, serde::Serialize)]
pub struct NodeRecord {
    pub id: u64,
    pub parent_id: u64,
    pub depth: u8,
    #[serde(rename = "move")]
    pub mv: String,
    pub score: i32,
}

/// Thread-safe tree recorder.
///
/// Nodes are buffered in memory with initial `score=0`, then updated when the
/// score is known.  Call `flush()` once the search is complete to write all
/// records as JSONL to the underlying sink.
pub struct TreeRecorder {
    counter: AtomicU64,
    records: Mutex<std::collections::HashMap<u64, NodeRecord>>,
    writer: Mutex<BufWriter<Box<dyn Write + Send>>>,
}

impl TreeRecorder {
    /// Create a new TreeRecorder writing to the given `Write` sink.
    pub fn new(sink: Box<dyn Write + Send>) -> Self {
        TreeRecorder {
            counter: AtomicU64::new(1),
            records: Mutex::new(std::collections::HashMap::new()),
            writer: Mutex::new(BufWriter::new(sink)),
        }
    }

    /// Create a recorder that writes to stdout.
    pub fn stdout() -> Self {
        Self::new(Box::new(std::io::stdout()))
    }

    /// Record a search node.  Returns the node's unique ID (used as `parent_id`
    /// for child nodes).  The score is initially stored as 0; call `update_score`
    /// with the final value after the subtree is searched.
    pub fn record_node(&self, parent_id: u64, depth: u8, mv: &Move, _score: i32) -> u64 {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let record = NodeRecord {
            id,
            parent_id,
            depth,
            mv: mv.to_string(),
            score: 0,
        };
        if let Ok(mut map) = self.records.lock() {
            map.insert(id, record);
        }
        id
    }

    /// Update the score of a previously recorded node.
    pub fn update_score(&self, id: u64, score: i32) {
        if let Ok(mut map) = self.records.lock() {
            if let Some(record) = map.get_mut(&id) {
                record.score = score;
            }
        }
    }

    /// Flush all buffered records to the sink as JSONL.  Each line contains
    /// exactly the fields: `id, parent_id, depth, move, score`.
    pub fn flush(&self) {
        let records: Vec<NodeRecord> = {
            let map = match self.records.lock() {
                Ok(m) => m,
                Err(_) => return,
            };
            let mut v: Vec<NodeRecord> = map
                .values()
                .map(|r| NodeRecord {
                    id: r.id,
                    parent_id: r.parent_id,
                    depth: r.depth,
                    mv: r.mv.clone(),
                    score: r.score,
                })
                .collect();
            v.sort_unstable_by_key(|r| r.id);
            v
        };

        if let Ok(mut w) = self.writer.lock() {
            for record in &records {
                if let Ok(json) = serde_json::to_string(record) {
                    let _ = writeln!(w, "{}", json);
                }
            }
            let _ = w.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;
    use std::sync::Arc;

    fn cursor_recorder() -> (TreeRecorder, Arc<Mutex<Vec<u8>>>) {
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = buf.clone();
        let sink: Box<dyn Write + Send> = Box::new(SharedVec(buf_clone));
        (TreeRecorder::new(sink), buf)
    }

    struct SharedVec(Arc<Mutex<Vec<u8>>>);
    impl Write for SharedVec {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn record_node_and_flush_produces_valid_jsonl() {
        let (recorder, buf) = cursor_recorder();
        let mv = Move {
            from: Position::new(0, 0),
            to: Position::new(1, 1),
            unstack: false,
        };
        let id = recorder.record_node(0, 1, &mv, 0);
        recorder.update_score(id, 42);
        recorder.flush();
        let data = buf.lock().unwrap().clone();
        let text = String::from_utf8(data).unwrap();
        for line in text.lines() {
            let obj: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            assert!(obj.get("id").is_some());
            assert!(obj.get("parent_id").is_some());
            assert!(obj.get("depth").is_some());
            assert!(obj.get("move").is_some());
            assert!(obj.get("score").is_some());
        }
        assert_eq!(id, 1);
    }

    #[test]
    fn update_score_is_reflected_in_flush() {
        let (recorder, buf) = cursor_recorder();
        let mv = Move {
            from: Position::new(0, 0),
            to: Position::new(1, 1),
            unstack: false,
        };
        let id = recorder.record_node(0, 0, &mv, 0);
        recorder.update_score(id, 99);
        recorder.flush();
        let data = buf.lock().unwrap().clone();
        let text = String::from_utf8(data).unwrap();
        assert!(
            text.contains("99"),
            "expected score 99 in output, got: {}",
            text
        );
    }

    #[test]
    fn ids_are_monotonically_increasing() {
        let (recorder, _) = cursor_recorder();
        let mv = Move {
            from: Position::new(0, 0),
            to: Position::new(1, 1),
            unstack: false,
        };
        let id1 = recorder.record_node(0, 0, &mv, 0);
        let id2 = recorder.record_node(0, 0, &mv, 0);
        assert!(id2 > id1);
    }
}
