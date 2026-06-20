//! Bitemporal query operations.

use chrono::{DateTime, Utc};

use crate::error::BitemporalError;
use crate::types::{BitemporalRecord, SupersessionReceipt};

/// Append a new record and mark prior versions as superseded.
///
/// This is the core append-supersede operation:
/// - A new row is inserted with the given `valid_time` and a new `recorded_time`
/// - Any prior rows for the same record ID are marked as superseded
///
/// Returns a `SupersessionReceipt` for each superseded prior record.
///
/// # Arguments
///
/// * `records` — mutable slice of existing records (simulating a database table)
/// * `new_record` — the new record to append
///
/// # Returns
///
/// Vector of `SupersessionReceipt` for each superseded prior version.
pub fn append_supersede<T>(
    records: &mut Vec<BitemporalRecord<T>>,
    new_record: BitemporalRecord<T>,
) -> Result<Vec<SupersessionReceipt>, BitemporalError>
where
    T: Clone,
{
    // Collect prior versions of this record ID
    let prior_versions: Vec<_> = records
        .iter()
        .filter(|r| r.id == new_record.id)
        .cloned()
        .collect();

    let mut receipts = Vec::new();

    // Create supersession receipts for each prior version
    for prior in prior_versions {
        // We need concrete types for the receipt; use type erasure via BitemporalRecord<()>
        // but we need the actual record IDs and times, so we use the concrete prior/new
        let receipt = SupersessionReceipt::new(
            BitemporalRecord {
                id: prior.id.clone(),
                valid_time: prior.valid_time,
                recorded_time: prior.recorded_time,
                value: (),
            },
            BitemporalRecord {
                id: new_record.id.clone(),
                valid_time: new_record.valid_time,
                recorded_time: new_record.recorded_time,
                value: (),
            },
        );
        receipts.push(receipt);
    }

    // Append the new record (append-only, never mutate prior rows)
    records.push(new_record);

    Ok(receipts)
}

/// Query records valid at a specific `valid_time` as of a specific `recorded_time`.
///
/// This implements the bitemporal "as-of" query:
/// - Returns records whose `valid_time` is at or before the query `valid_time`
/// - Among those, returns the version that was current as of `recorded_time`
///   (i.e., the latest record with `recorded_time <= query_recorded_time`)
///
/// # Arguments
///
/// * `records` — slice of all records (simulating a database table)
/// * `valid_time` — the business time to query
/// * `recorded_time` — the system time to query as-of
///
/// # Returns
///
/// Records valid at `valid_time` as of `recorded_time`.
pub fn as_of_query<T>(
    records: &[BitemporalRecord<T>],
    valid_time: DateTime<Utc>,
    recorded_time: DateTime<Utc>,
) -> Vec<BitemporalRecord<T>>
where
    T: Clone,
{
    use std::collections::HashMap;

    let mut latest_by_id: HashMap<String, BitemporalRecord<T>> = HashMap::new();

    for record in records {
        if record.recorded_time <= recorded_time && record.valid_time <= valid_time {
            let existing = latest_by_id.get(&record.id);
            if existing
                .map(|e| record.recorded_time > e.recorded_time)
                .unwrap_or(true)
            {
                latest_by_id.insert(record.id.clone(), record.clone());
            }
        }
    }

    latest_by_id.into_values().collect()
}

/// Return full state as of a specific `recorded_time`.
///
/// Returns all records that were current at the given recorded_time,
/// one record per unique ID (the latest version as of that time).
pub fn temporal_snapshot<T>(
    records: &[BitemporalRecord<T>],
    as_of_time: DateTime<Utc>,
) -> Vec<BitemporalRecord<T>>
where
    T: Clone,
{
    as_of_query(records, as_of_time, as_of_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_append_supersede_creates_receipt() {
        let mut records: Vec<BitemporalRecord<&str>> = Vec::new();

        let t0 = Utc.timestamp_opt(1000, 0).unwrap();
        let v1 = BitemporalRecord {
            id: "ep1".to_string(),
            valid_time: t0,
            recorded_time: t0,
            value: "version1",
        };

        let receipts = append_supersede(&mut records, v1).unwrap();
        assert!(receipts.is_empty());
        assert_eq!(records.len(), 1);

        let t1 = Utc.timestamp_opt(2000, 0).unwrap();
        let v2 = BitemporalRecord {
            id: "ep1".to_string(),
            valid_time: t0,
            recorded_time: t1,
            value: "version2",
        };

        let receipts = append_supersede(&mut records, v2).unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].superseded.superseded_id, "ep1");
        assert_eq!(receipts[0].superseding_id, "ep1");
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_as_of_query_returns_correct_version() {
        let t0 = Utc.timestamp_opt(1000, 0).unwrap();
        let t1 = Utc.timestamp_opt(2000, 0).unwrap();
        let t2 = Utc.timestamp_opt(3000, 0).unwrap();

        let records: Vec<BitemporalRecord<&str>> = vec![
            BitemporalRecord {
                id: "ep1".to_string(),
                valid_time: t0,
                recorded_time: t0,
                value: "v1",
            },
            BitemporalRecord {
                id: "ep1".to_string(),
                valid_time: t0,
                recorded_time: t1,
                value: "v2",
            },
            BitemporalRecord {
                id: "ep1".to_string(),
                valid_time: t1,
                recorded_time: t2,
                value: "v3",
            },
        ];

        let result = as_of_query(&records, t0, t0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "v1");

        let result = as_of_query(&records, t0, t1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "v2");

        let result = as_of_query(&records, t1, t2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "v3");
    }

    #[test]
    fn test_temporal_snapshot() {
        let t0 = Utc.timestamp_opt(1000, 0).unwrap();
        let t1 = Utc.timestamp_opt(2000, 0).unwrap();

        let records: Vec<BitemporalRecord<&str>> = vec![
            BitemporalRecord {
                id: "ep1".to_string(),
                valid_time: t0,
                recorded_time: t0,
                value: "v1",
            },
            BitemporalRecord {
                id: "ep1".to_string(),
                valid_time: t0,
                recorded_time: t1,
                value: "v2",
            },
            BitemporalRecord {
                id: "ep2".to_string(),
                valid_time: t0,
                recorded_time: t0,
                value: "ep2_v1",
            },
        ];

        let snap = temporal_snapshot(&records, t0);
        assert_eq!(snap.len(), 2);
        let ep1 = snap.iter().find(|r| r.id == "ep1").unwrap();
        assert_eq!(ep1.value, "v1");

        let snap = temporal_snapshot(&records, t1);
        assert_eq!(snap.len(), 2);
        let ep1 = snap.iter().find(|r| r.id == "ep1").unwrap();
        assert_eq!(ep1.value, "v2");
    }
}
