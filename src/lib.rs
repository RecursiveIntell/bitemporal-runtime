//! # bitemporal-runtime
//!
//! Bitemporal truth primitives for append-supersede temporal data.
//!
//! ## Core concepts
//!
//! - **valid_time**: When something is true in the domain (business time)
//! - **recorded_time**: When the system learned about it (system time)
//! - **append-supersede**: Updates never mutate; they append a new row and mark prior rows superseded
//!
//! ## Key types
//!
//! - [`BitemporalRecord<T>`]: A temporal record with valid_time, recorded_time, and value
//! - [`SupersessionReceipt`]: Cryptographic receipt for every supersession event
//!
//! ## Functions
//!
//! - [`append_supersede`]: Insert a new record, mark prior records superseded
//! - [`as_of_query`]: Query records valid at a specific valid_time as of a specific recorded_time
//! - [`temporal_snapshot`]: Retrieve full state as of a specific recorded_time

mod error;
mod queries;
#[cfg(feature = "schema")]
pub mod schema;
#[cfg(feature = "sqlite")]
mod sqlite;
mod types;

pub use error::BitemporalError;
pub use queries::{append_supersede, as_of_query, temporal_snapshot};
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDb;
pub use types::{BitemporalRecord, RecordId, SupersessionReceipt, SupersessionTarget};

use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

/// In-memory bitemporal store for testing and development.
#[derive(Debug, Clone)]
pub struct InMemoryDb {
    /// Map from record_id -> Vec of BitemporalRecord (one per version)
    records: BTreeMap<String, Vec<types::BitemporalRecord>>,
}

impl InMemoryDb {
    /// Create a new empty in-memory bitemporal store.
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    /// Insert a bitemporal record into this store.
    pub fn insert(&mut self, record: types::BitemporalRecord) {
        let id = record.id.clone();
        self.records.entry(id).or_default().push(record);
    }

    /// Get all versions of a record by ID.
    pub fn get_versions(&self, id: &str) -> Option<&Vec<types::BitemporalRecord>> {
        self.records.get(id)
    }

    /// Get all records as a snapshot at a specific recorded_time.
    pub fn snapshot_at(&self, recorded_time: DateTime<Utc>) -> Vec<types::BitemporalRecord> {
        let mut result = Vec::new();
        for versions in self.records.values() {
            let mut best: Option<&types::BitemporalRecord> = None;
            for v in versions {
                if v.recorded_time <= recorded_time
                    && best
                        .map(|b| v.recorded_time > b.recorded_time)
                        .unwrap_or(true)
                {
                    best = Some(v);
                }
            }
            if let Some(r) = best {
                result.push(r.clone());
            }
        }
        result
    }

    /// Get all records valid at a specific valid_time as of a specific recorded_time.
    pub fn as_of(
        &self,
        valid_time: DateTime<Utc>,
        recorded_time: DateTime<Utc>,
    ) -> Vec<types::BitemporalRecord> {
        let mut result = Vec::new();
        for versions in self.records.values() {
            let mut best: Option<&types::BitemporalRecord> = None;
            for v in versions {
                if v.recorded_time <= recorded_time
                    && v.valid_time <= valid_time
                    && best
                        .map(|b| v.recorded_time > b.recorded_time)
                        .unwrap_or(true)
                {
                    best = Some(v);
                }
            }
            if let Some(r) = best {
                result.push(r.clone());
            }
        }
        result
    }

    /// Returns the count of unique record IDs in this store.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if this store has no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl Default for InMemoryDb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_basic_insert() {
        let mut db = InMemoryDb::new();
        let record = types::BitemporalRecord {
            id: "ep1".to_string(),
            valid_time: Utc::now(),
            recorded_time: Utc::now(),
            value: (),
        };
        db.insert(record);
        let versions = db.get_versions("ep1").unwrap();
        assert_eq!(versions.len(), 1);
    }
}
