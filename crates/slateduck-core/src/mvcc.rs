//! MVCC filtering for DuckLake catalog rows.
//!
//! Terminology (enforced throughout the codebase):
//! - `dl_snapshot_id` / `catalog_version`: DuckLake logical snapshot identifier
//! - `kv_read_view` / `kv_snapshot`: SlateDB-level read view (physical)
//!
//! The MVCC filter determines row visibility:
//! `begin_snapshot <= dl_snapshot_id AND (end_snapshot IS NULL OR dl_snapshot_id < end_snapshot)`

use serde::{Deserialize, Serialize};

/// A DuckLake snapshot ID (logical MVCC version).
pub type SnapshotId = u64;

/// MVCC visibility fields stored in versioned row values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MvccFields {
    pub begin_snapshot: SnapshotId,
    pub end_snapshot: Option<SnapshotId>,
}

impl MvccFields {
    pub fn new(begin_snapshot: SnapshotId) -> Self {
        Self {
            begin_snapshot,
            end_snapshot: None,
        }
    }

    /// Check if this row is visible at the given snapshot.
    pub fn is_visible_at(&self, dl_snapshot_id: SnapshotId) -> bool {
        self.begin_snapshot <= dl_snapshot_id
            && match self.end_snapshot {
                None => true,
                Some(end) => dl_snapshot_id < end,
            }
    }

    /// Mark this row as ended at the given snapshot.
    pub fn end_at(&mut self, snapshot_id: SnapshotId) {
        self.end_snapshot = Some(snapshot_id);
    }

    /// Check if this row is eligible for GC given the oldest retained snapshot.
    pub fn is_gc_eligible(&self, oldest_retained: SnapshotId) -> bool {
        match self.end_snapshot {
            None => false,
            Some(end) => end <= oldest_retained,
        }
    }
}

/// Filter a collection of versioned rows to those visible at a given snapshot.
pub fn filter_visible<T, F>(
    rows: impl Iterator<Item = T>,
    dl_snapshot_id: SnapshotId,
    get_mvcc: F,
) -> Vec<T>
where
    F: Fn(&T) -> &MvccFields,
{
    rows.filter(|row| get_mvcc(row).is_visible_at(dl_snapshot_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_at_creation_snapshot() {
        let mvcc = MvccFields::new(5);
        assert!(mvcc.is_visible_at(5));
        assert!(mvcc.is_visible_at(10));
        assert!(!mvcc.is_visible_at(4));
    }

    #[test]
    fn invisible_after_end() {
        let mvcc = MvccFields {
            begin_snapshot: 5,
            end_snapshot: Some(10),
        };
        assert!(mvcc.is_visible_at(5));
        assert!(mvcc.is_visible_at(9));
        assert!(!mvcc.is_visible_at(10));
        assert!(!mvcc.is_visible_at(11));
        assert!(!mvcc.is_visible_at(4));
    }

    #[test]
    fn gc_eligibility() {
        let live = MvccFields::new(5);
        assert!(!live.is_gc_eligible(100));

        let ended = MvccFields {
            begin_snapshot: 5,
            end_snapshot: Some(10),
        };
        assert!(ended.is_gc_eligible(10));
        assert!(ended.is_gc_eligible(11));
        assert!(!ended.is_gc_eligible(9));
    }

    #[test]
    fn filter_visible_rows() {
        let rows = [
            MvccFields {
                begin_snapshot: 1,
                end_snapshot: Some(5),
            },
            MvccFields {
                begin_snapshot: 3,
                end_snapshot: None,
            },
            MvccFields {
                begin_snapshot: 7,
                end_snapshot: None,
            },
        ];
        let visible = filter_visible(rows.iter(), 4, |r| r);
        assert_eq!(visible.len(), 2); // rows at begin=1 and begin=3
    }
}
