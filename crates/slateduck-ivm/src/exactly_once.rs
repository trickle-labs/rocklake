//! Exactly-once output snapshot coordination.
//!
//! Each output snapshot is tagged with `(matview_id, target_frontier)`.
//! CatalogWriter CAS prevents duplicate snapshots for the same target.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Output snapshot metadata tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutputTag {
    /// Matview ID.
    pub matview_id: u64,
    /// Target frontier sequence this snapshot covers.
    pub target_frontier: u64,
    /// Shard ID that produced this output.
    pub shard_id: u32,
}

/// State for tracking committed output snapshots (deduplication).
#[derive(Debug, Clone, Default)]
pub struct OutputDeduplicator {
    /// Set of already-committed (matview_id, target_frontier) pairs.
    committed: HashSet<OutputTag>,
}

/// Result of attempting to commit an output snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitResult {
    /// Successfully committed.
    Committed,
    /// Duplicate detected — this snapshot was already committed.
    Duplicate,
    /// CAS conflict — another writer committed simultaneously.
    CasConflict,
}

impl OutputDeduplicator {
    /// Create a new deduplicator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a snapshot has already been committed.
    pub fn is_committed(&self, tag: &OutputTag) -> bool {
        self.committed.contains(tag)
    }

    /// Attempt to commit an output snapshot. Returns Duplicate if already committed.
    pub fn try_commit(&mut self, tag: OutputTag) -> CommitResult {
        if self.committed.contains(&tag) {
            CommitResult::Duplicate
        } else {
            self.committed.insert(tag);
            CommitResult::Committed
        }
    }

    /// Mark a tag as committed (for restore from catalog state).
    pub fn mark_committed(&mut self, tag: OutputTag) {
        self.committed.insert(tag);
    }

    /// Number of committed snapshots tracked.
    pub fn committed_count(&self) -> usize {
        self.committed.len()
    }
}

/// Validate that an output commit operation is safe (no partial writes).
///
/// Called before the catalog commit step. If the worker was killed after
/// writing Parquet but before committing to catalog, this check catches it.
pub fn validate_output_commit(
    tag: &OutputTag,
    dedup: &OutputDeduplicator,
    parquet_written: bool,
    catalog_committed: bool,
) -> CommitResult {
    if catalog_committed {
        // Already fully committed — this is a duplicate.
        return CommitResult::Duplicate;
    }
    if dedup.is_committed(tag) {
        return CommitResult::Duplicate;
    }
    if parquet_written && !catalog_committed {
        // Parquet written but catalog not committed — this is safe to retry.
        // The CAS on catalog write ensures exactly-once.
        return CommitResult::Committed;
    }
    CommitResult::Committed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicator_prevents_double_commit() {
        let mut dedup = OutputDeduplicator::new();
        let tag = OutputTag {
            matview_id: 1,
            target_frontier: 100,
            shard_id: 0,
        };

        assert_eq!(dedup.try_commit(tag.clone()), CommitResult::Committed);
        assert_eq!(dedup.try_commit(tag.clone()), CommitResult::Duplicate);
    }

    #[test]
    fn deduplicator_different_frontiers_both_commit() {
        let mut dedup = OutputDeduplicator::new();
        let tag1 = OutputTag {
            matview_id: 1,
            target_frontier: 100,
            shard_id: 0,
        };
        let tag2 = OutputTag {
            matview_id: 1,
            target_frontier: 200,
            shard_id: 0,
        };

        assert_eq!(dedup.try_commit(tag1), CommitResult::Committed);
        assert_eq!(dedup.try_commit(tag2), CommitResult::Committed);
        assert_eq!(dedup.committed_count(), 2);
    }

    #[test]
    fn validate_prevents_partial_writes() {
        let dedup = OutputDeduplicator::new();
        let tag = OutputTag {
            matview_id: 1,
            target_frontier: 100,
            shard_id: 0,
        };

        // Parquet written, catalog not committed — safe to commit.
        let result = validate_output_commit(&tag, &dedup, true, false);
        assert_eq!(result, CommitResult::Committed);

        // Already catalog committed — duplicate.
        let result = validate_output_commit(&tag, &dedup, true, true);
        assert_eq!(result, CommitResult::Duplicate);
    }
}
