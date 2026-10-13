use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

pub const CHUNK_EVENTS: usize = 4096;
const MAX_DECODED: u64 = 64 * 1024 * 1024;
const MAX_CELLS: usize = 1_048_576;
const MAX_ESTIMATED_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub millisecond: i64,
    pub actor: i64,
    pub fields: BTreeMap<i64, Value>,
}

#[derive(Serialize, Deserialize)]
struct Columns {
    v: u8,
    t: Vec<i64>,
    a: Vec<i64>,
    c: Vec<(i64, Vec<Value>)>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid chunk: {0}")]
pub struct CodecError(String);

fn fits(rows: usize, keys: usize, value_bytes: usize) -> bool {
    rows <= CHUNK_EVENTS
        && rows.saturating_mul(keys) <= MAX_CELLS
        && value_bytes
            .saturating_add(keys.saturating_mul(rows + 16))
            .saturating_add(rows * 24 + 128)
            <= MAX_ESTIMATED_BYTES
}

/// Bound the dense column matrix before allocating it. Sparse, changing keys and
/// large property values may require fewer events than the 4096-event ceiling.
pub fn partition(rows: &[Row]) -> Result<Vec<&[Row]>, CodecError> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut keys = BTreeSet::new();
    let mut value_bytes = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let row_bytes = row.fields.values().try_fold(0usize, |bytes, value| {
            rmp_serde::to_vec(value)
                .map(|encoded| bytes.saturating_add(encoded.len()))
                .map_err(|e| CodecError(e.to_string()))
        })?;
        keys.extend(row.fields.keys().copied());
        if !fits(
            index - start + 1,
            keys.len(),
            value_bytes.saturating_add(row_bytes),
        ) {
            if index == start {
                return Err(CodecError("single event exceeds chunk budget".into()));
            }
            chunks.push(&rows[start..index]);
            start = index;
            keys = row.fields.keys().copied().collect();
            value_bytes = 0;
            if !fits(1, keys.len(), row_bytes) {
                return Err(CodecError("single event exceeds chunk budget".into()));
            }
        }
        value_bytes = value_bytes.saturating_add(row_bytes);
    }
    if start < rows.len() {
        chunks.push(&rows[start..]);
    }
    Ok(chunks)
}

pub fn encode(rows: &[Row]) -> Result<Vec<u8>, CodecError> {
    if rows.is_empty()
        || rows.len() > CHUNK_EVENTS
        || rows.windows(2).any(|w| w[0].millisecond > w[1].millisecond)
        || rows
            .iter()
            .any(|r| !(0..86_400_000).contains(&r.millisecond))
    {
        return Err(CodecError(
            "rows must be ordered within one UTC day, 1..4096 events".into(),
        ));
    }
    let mut previous = 0;
    let keys: BTreeSet<_> = rows.iter().flat_map(|r| r.fields.keys().copied()).collect();
    if rows.len().saturating_mul(keys.len()) > MAX_CELLS {
        return Err(CodecError("column matrix exceeds cell limit".into()));
    }
    let columns = Columns {
        v: 1,
        t: rows
            .iter()
            .map(|r| {
                let delta = r.millisecond - previous;
                previous = r.millisecond;
                delta
            })
            .collect(),
        a: rows.iter().map(|r| r.actor).collect(),
        c: keys
            .into_iter()
            .map(|key| {
                (
                    key,
                    rows.iter()
                        .map(|r| r.fields.get(&key).cloned().unwrap_or(Value::Null))
                        .collect(),
                )
            })
            .collect(),
    };
    let bytes = rmp_serde::to_vec_named(&columns).map_err(|e| CodecError(e.to_string()))?;
    if bytes.len() as u64 > MAX_DECODED {
        return Err(CodecError("chunk exceeds decoded limit".into()));
    }
    zstd::stream::encode_all(bytes.as_slice(), 3).map_err(|e| CodecError(e.to_string()))
}

pub fn decode(payload: &[u8]) -> Result<Vec<Row>, CodecError> {
    let mut bytes = Vec::new();
    zstd::stream::read::Decoder::new(payload)
        .map_err(|e| CodecError(e.to_string()))?
        .take(MAX_DECODED + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| CodecError(e.to_string()))?;
    if bytes.len() as u64 > MAX_DECODED {
        return Err(CodecError("chunk exceeds decoded limit".into()));
    }
    let c: Columns = rmp_serde::from_slice(&bytes).map_err(|e| CodecError(e.to_string()))?;
    if c.v != 1
        || c.t.is_empty()
        || c.t.len() > CHUNK_EVENTS
        || c.a.len() != c.t.len()
        || c.c.iter().any(|(_, values)| values.len() != c.t.len())
        || c.c.iter().map(|(id, _)| id).collect::<BTreeSet<_>>().len() != c.c.len()
    {
        return Err(CodecError("version or column dimensions".into()));
    }
    let mut total = 0i64;
    c.t.into_iter()
        .enumerate()
        .map(|(index, delta)| {
            total = total
                .checked_add(delta)
                .ok_or_else(|| CodecError("time overflow".into()))?;
            if delta < 0 || !(0..86_400_000).contains(&total) {
                return Err(CodecError("time outside day or unordered".into()));
            }
            Ok(Row {
                millisecond: total,
                actor: c.a[index],
                fields: c
                    .c
                    .iter()
                    .filter(|(_, values)| !values[index].is_null())
                    .map(|(key, values)| (*key, values[index].clone()))
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_sparse_columns_and_deltas() {
        let rows = vec![
            Row {
                millisecond: 5,
                actor: 42,
                fields: BTreeMap::from([(2, serde_json::json!("ios"))]),
            },
            Row {
                millisecond: 100,
                actor: 43,
                fields: BTreeMap::from([(3, serde_json::json!(true))]),
            },
        ];
        assert_eq!(decode(&encode(&rows).unwrap()).unwrap(), rows);
        assert!(encode(&[rows[1].clone(), rows[0].clone()]).is_err());
        assert!(decode(b"corrupt").is_err());
    }

    #[test]
    fn changing_property_keys_split_before_dense_allocation() {
        let rows: Vec<_> = (0..2048)
            .map(|i| Row {
                millisecond: i,
                actor: i,
                fields: BTreeMap::from([(i, serde_json::json!(true))]),
            })
            .collect();
        assert!(encode(&rows).is_err());
        let chunks = partition(&rows).unwrap();
        assert_eq!(chunks.len(), 2);
        let restored: Vec<_> = chunks
            .into_iter()
            .flat_map(|chunk| decode(&encode(chunk).unwrap()).unwrap())
            .collect();
        assert_eq!(restored, rows);
    }
}
