//! Tree recorder: optional per-node recording of the search tree.
//!
//! When disabled (None passed to search), no overhead is incurred.
//! When enabled, each node is written the moment its subtree is searched
//! and its score known, so memory stays flat no matter how large the tree
//! grows.
//!
//! Output order is therefore post-order (a node follows all its children)
//! and interleaved across the parallel root searches: `id`s are unique but
//! not sorted. Rebuild the tree from `parent_id`; the root position itself
//! is the implicit node `0`, root moves have `depth` 0.
//!
//! Two formats ([`TreeFormat`]):
//! - **JSONL**: one `{"id","parent_id","depth","move","unstack","score"}`
//!   object per line, `move` in `A1-B2[-]` notation.
//! - **Binary**: fixed [`BINARY_RECORD_BYTES`]-byte records, little-endian
//!   like the wire protocol, no header or separator:
//!
//!   | offset | type  | field                                             |
//!   |--------|-------|---------------------------------------------------|
//!   | 0      | `u32` | `id`                                              |
//!   | 4      | `u32` | `parent_id`                                       |
//!   | 8      | `u8`  | `depth`                                           |
//!   | 9      | `u16` | move, `Move::to_u16` layout (bit 14 = unstack)    |
//!   | 11     | `i16` | `score`, saturated to the `i16` range             |

use crate::moves::Move;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Write buffer size: large enough that the lock-and-write per node rarely
/// turns into a syscall.
const WRITE_BUFFER_BYTES: usize = 1 << 20;

/// Size of one [`TreeFormat::Binary`] record.
pub const BINARY_RECORD_BYTES: usize = 13;

/// Output encoding of a [`TreeRecorder`]; see the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeFormat {
    Jsonl,
    Binary,
}

/// Thread-safe streaming tree recorder.
pub struct TreeRecorder {
    counter: AtomicU64,
    format: TreeFormat,
    writer: Mutex<BufWriter<Box<dyn Write + Send>>>,
}

impl TreeRecorder {
    /// Create a new TreeRecorder writing `format` to the given sink.
    pub fn new(sink: Box<dyn Write + Send>, format: TreeFormat) -> Self {
        TreeRecorder {
            counter: AtomicU64::new(1),
            format,
            writer: Mutex::new(BufWriter::with_capacity(WRITE_BUFFER_BYTES, sink)),
        }
    }

    /// Create a recorder that writes `format` to stdout.
    pub fn stdout(format: TreeFormat) -> Self {
        Self::new(Box::new(std::io::stdout()), format)
    }

    /// Reserve the ID of a node about to be searched, so its children can
    /// name it as their `parent_id` before the node itself is written.
    ///
    /// # Panics
    /// In [`TreeFormat::Binary`], once IDs no longer fit the `u32` field:
    /// stopping beats emitting a tree whose IDs silently wrap.
    pub fn next_id(&self) -> u64 {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        assert!(
            self.format != TreeFormat::Binary || id <= u64::from(u32::MAX),
            "search tree exceeds {} nodes, the binary format's u32 id limit",
            u32::MAX
        );
        id
    }

    /// Write a searched node. `score` is from the point of view of the side
    /// that played `mv`.
    pub fn record(&self, id: u64, parent_id: u64, depth: u8, mv: &Move, score: i32) {
        let Ok(mut w) = self.writer.lock() else {
            return;
        };
        let _ = match self.format {
            // Every field is a number, a bool, or a move in `A1-B2[-]`
            // notation, so nothing needs JSON escaping.
            TreeFormat::Jsonl => writeln!(
                w,
                r#"{{"id":{id},"parent_id":{parent_id},"depth":{depth},"move":"{mv}","unstack":{},"score":{score}}}"#,
                mv.unstack,
            ),
            TreeFormat::Binary => {
                // `next_id` guarantees both ids fit.
                let score = score.clamp(i16::MIN.into(), i16::MAX.into()) as i16;
                let mut buf = [0u8; BINARY_RECORD_BYTES];
                buf[0..4].copy_from_slice(&(id as u32).to_le_bytes());
                buf[4..8].copy_from_slice(&(parent_id as u32).to_le_bytes());
                buf[8] = depth;
                buf[9..11].copy_from_slice(&mv.to_u16().to_le_bytes());
                buf[11..13].copy_from_slice(&score.to_le_bytes());
                w.write_all(&buf)
            }
        };
    }
    /// Flush buffered lines to the sink. Call once the search is complete.
    pub fn flush(&self) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;
    use std::sync::Arc;

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

    fn recorded(format: TreeFormat) -> Vec<u8> {
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let recorder = TreeRecorder::new(Box::new(SharedVec(buf.clone())), format);
        let mv = Move {
            from: Position::new(0, 8),
            to: Position::new(1, 7),
            unstack: true,
        };
        let parent = recorder.next_id();
        let child = recorder.next_id();
        recorder.record(child, parent, 1, &mv, -42);
        recorder.record(parent, 0, 0, &mv, 100_000);
        recorder.flush();
        let out = buf.lock().unwrap().clone();
        out
    }

    #[test]
    fn jsonl_writes_one_object_per_node() {
        let text = String::from_utf8(recorded(TreeFormat::Jsonl)).unwrap();
        let nodes: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("valid JSON"))
            .collect();
        assert_eq!(
            nodes,
            vec![
                serde_json::json!({"id": 2, "parent_id": 1, "depth": 1,
                    "move": "A1-B2-", "unstack": true, "score": -42}),
                serde_json::json!({"id": 1, "parent_id": 0, "depth": 0,
                    "move": "A1-B2-", "unstack": true, "score": 100_000}),
            ]
        );
    }

    #[test]
    fn binary_writes_fixed_little_endian_records_with_saturated_score() {
        let mv = Move {
            from: Position::new(0, 8),
            to: Position::new(1, 7),
            unstack: true,
        };
        let mv_bits = mv.to_u16().to_le_bytes();
        assert_ne!(
            mv.to_u16() & 0x4000,
            0,
            "the unstack bit travels in the move"
        );
        let mut expected = Vec::new();
        expected.extend_from_slice(&[2, 0, 0, 0, 1, 0, 0, 0, 1]);
        expected.extend_from_slice(&mv_bits);
        expected.extend_from_slice(&(-42i16).to_le_bytes());
        expected.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0]);
        expected.extend_from_slice(&mv_bits);
        expected.extend_from_slice(&i16::MAX.to_le_bytes());
        assert_eq!(recorded(TreeFormat::Binary), expected);
    }

    #[test]
    #[should_panic(expected = "u32 id limit")]
    fn binary_refuses_ids_beyond_u32() {
        let recorder = TreeRecorder::new(Box::new(std::io::sink()), TreeFormat::Binary);
        recorder
            .counter
            .store(u64::from(u32::MAX) + 1, Ordering::Relaxed);
        recorder.next_id();
    }
}
