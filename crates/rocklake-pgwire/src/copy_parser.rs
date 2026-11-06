//! Binary PostgreSQL COPY format parser for COPY FROM STDIN streams.
//!
//! DuckDB's ducklake extension initialises the catalog by issuing binary COPY
//! FROM STDIN for several `ducklake_*` tables during the ATTACH sequence.  This
//! module parses those streams so we can bootstrap the catalog store with the
//! exact values DuckDB provides.
//!
//! # Format
//!
//! ```text
//! Signature  : "PGCOPY\n\xff\r\n\0"  (11 bytes)
//! Flags       : int32 (0)
//! Header ext  : int32 length + N bytes
//! For each row:
//!   field_count : int16  (or -1 = end-of-copy trailer)
//!   For each field:
//!     len       : int32  (-1 = NULL)
//!     data      : N bytes (if len >= 0)
//! ```

/// PostgreSQL binary COPY format signature.
const PGCOPY_SIGNATURE: &[u8] = b"PGCOPY\n\xff\r\n\0";

/// Errors produced by [`parse_binary_copy_rows`].
///
/// All variants indicate a malformed or truncated binary COPY stream.
/// The parser is fail-closed: any error means *no* rows are returned.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CopyParseError {
    /// Stream is too short to contain the binary COPY signature and fixed header.
    #[error("binary COPY stream too short to contain a valid header")]
    TooShort,

    /// The 11-byte signature at the start of the stream does not match
    /// `PGCOPY\n\xff\r\n\0`.
    #[error("binary COPY signature mismatch")]
    BadSignature,

    /// The header extension length field overflows the available data.
    #[error("binary COPY header extension truncated")]
    TruncatedHeaderExt,

    /// A row's `field_count` int16 is missing (stream ends mid-row).
    #[error("binary COPY stream truncated: missing field count for row {row}")]
    TruncatedFieldCount { row: usize },

    /// A field's 4-byte length prefix is missing.
    #[error("binary COPY stream truncated: missing field length in row {row}, field {field}")]
    TruncatedFieldLength { row: usize, field: usize },

    /// A field body is shorter than its declared length.
    #[error(
        "binary COPY stream truncated: field body in row {row}, field {field} \
         declares {declared} bytes but only {available} remain"
    )]
    TruncatedFieldBody {
        row: usize,
        field: usize,
        declared: usize,
        available: usize,
    },

    /// The stream ended without the `field_count = -1` end-of-data marker.
    #[error("binary COPY stream missing end-of-data marker")]
    MissingEndMarker,
    /// An inlined row did not contain the three DuckLake system columns and its user columns.
    #[error("inlined COPY row {row} has {actual} fields, expected {expected}")]
    InlinedFieldCount {
        row: usize,
        expected: usize,
        actual: usize,
    },
    /// A binary field could not be converted to the canonical inlined value representation.
    #[error("invalid inlined COPY value at row {row}, field {field}")]
    InvalidInlinedValue { row: usize, field: usize },
}

/// Parse a PostgreSQL binary COPY stream.
///
/// Returns `Ok(rows)` on success where each inner vec represents a row and
/// each `Option<Vec<u8>>` is either `None` (SQL NULL) or `Some(bytes)` (raw
/// field value in PostgreSQL binary encoding).
///
/// Returns `Err(CopyParseError)` on any truncation or format violation.
/// The parser is **fail-closed**: a partial or malformed stream never produces
/// a partial result set — callers either get all rows or an error.
pub fn parse_binary_copy_rows(data: &[u8]) -> Result<Vec<Vec<Option<Vec<u8>>>>, CopyParseError> {
    let mut pos = 0usize;

    // Validate and skip signature.
    if data.len() < PGCOPY_SIGNATURE.len() + 8 {
        return Err(CopyParseError::TooShort);
    }
    if !data.starts_with(PGCOPY_SIGNATURE) {
        return Err(CopyParseError::BadSignature);
    }
    pos += PGCOPY_SIGNATURE.len();

    // Skip flags (int32).
    pos += 4;

    // Skip header extension area (int32 length + that many bytes).
    if pos + 4 > data.len() {
        return Err(CopyParseError::TruncatedHeaderExt);
    }
    let ext_len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
    pos += 4;
    if pos + ext_len as usize > data.len() {
        return Err(CopyParseError::TruncatedHeaderExt);
    }
    pos += ext_len as usize;

    let mut rows: Vec<Vec<Option<Vec<u8>>>> = Vec::new();

    loop {
        // Read field count (int16); -1 signals end-of-copy.
        if pos + 2 > data.len() {
            return Err(CopyParseError::TruncatedFieldCount { row: rows.len() });
        }
        let field_count = i16::from_be_bytes([data[pos], data[pos + 1]]);
        pos += 2;

        if field_count < 0 {
            break; // End-of-copy trailer.
        }

        let field_count = field_count as usize;
        let mut row: Vec<Option<Vec<u8>>> = Vec::with_capacity(field_count);
        let row_idx = rows.len();

        for field_idx in 0..field_count {
            if pos + 4 > data.len() {
                return Err(CopyParseError::TruncatedFieldLength {
                    row: row_idx,
                    field: field_idx,
                });
            }
            let len = i32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;

            if len < 0 {
                row.push(None); // SQL NULL.
            } else {
                let len = len as usize;
                let available = data.len() - pos;
                if len > available {
                    return Err(CopyParseError::TruncatedFieldBody {
                        row: row_idx,
                        field: field_idx,
                        declared: len,
                        available,
                    });
                }
                row.push(Some(data[pos..pos + len].to_vec()));
                pos += len;
            }
        }

        rows.push(row);
    }

    Ok(rows)
}

/// Convert binary COPY rows for a DuckLake inlined-data table into the value
/// representation used by SQL `INSERT` handling.
pub fn decode_inlined_copy_rows(
    rows: &[Vec<Option<Vec<u8>>>],
    column_types: &[String],
) -> Result<Vec<Vec<Option<String>>>, CopyParseError> {
    let expected_fields = column_types.len() + 3;
    rows.iter()
        .enumerate()
        .map(|(row_index, row)| {
            if row.len() != expected_fields {
                return Err(CopyParseError::InlinedFieldCount {
                    row: row_index,
                    expected: expected_fields,
                    actual: row.len(),
                });
            }
            let row_id = extract_i64(row, 0)
                .and_then(|value| u64::try_from(value).ok())
                .ok_or(CopyParseError::InvalidInlinedValue {
                    row: row_index,
                    field: 0,
                })?;
            let mut values = vec![Some(row_id.to_string()), None, None];
            for (column_index, column_type) in column_types.iter().enumerate() {
                values.push(
                    decode_inlined_field(
                        row.get(column_index + 3).and_then(Option::as_deref),
                        column_type,
                    )
                    .map_err(|()| CopyParseError::InvalidInlinedValue {
                        row: row_index,
                        field: column_index + 3,
                    })?,
                );
            }
            Ok(values)
        })
        .collect()
}

fn decode_inlined_field(bytes: Option<&[u8]>, data_type: &str) -> Result<Option<String>, ()> {
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let data_type = data_type.trim().to_ascii_uppercase();
    let text = || String::from_utf8(bytes.to_vec()).map_err(|_| ());
    let value = match data_type.as_str() {
        "BOOLEAN" | "BOOL" => match bytes {
            [0] => "false".to_string(),
            [1] => "true".to_string(),
            _ => return Err(()),
        },
        "TINYINT" | "SMALLINT" | "INT2" | "INT16" => {
            i16::from_be_bytes(bytes.try_into().map_err(|_| ())?).to_string()
        }
        "INTEGER" | "INT" | "INT4" | "INT32" => {
            i32::from_be_bytes(bytes.try_into().map_err(|_| ())?).to_string()
        }
        "BIGINT" | "INT8" | "INT64" => {
            i64::from_be_bytes(bytes.try_into().map_err(|_| ())?).to_string()
        }
        "DATE" => {
            let days = i32::from_be_bytes(bytes.try_into().map_err(|_| ())?);
            let epoch = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).ok_or(())?;
            epoch
                .checked_add_signed(chrono::Duration::days(i64::from(days)))
                .ok_or(())?
                .format("%Y-%m-%d")
                .to_string()
        }
        "TIMESTAMP"
        | "TIMESTAMP WITHOUT TIME ZONE"
        | "TIMESTAMP WITH TIME ZONE"
        | "TIMESTAMPTZ" => {
            const PG_EPOCH_MICROS: i64 = 946_684_800 * 1_000_000;
            let micros = i64::from_be_bytes(bytes.try_into().map_err(|_| ())?);
            let unix_micros = micros.checked_add(PG_EPOCH_MICROS).ok_or(())?;
            chrono::DateTime::<chrono::Utc>::from_timestamp_micros(unix_micros)
                .ok_or(())?
                .naive_utc()
                .format("%Y-%m-%d %H:%M:%S%.6f")
                .to_string()
        }
        "VARCHAR" | "TEXT" | "STRING" | "BLOB" | "BYTEA" | "VARIANT" => {
            let mut hex = String::with_capacity(bytes.len() * 2 + 2);
            hex.push_str("\\x");
            for byte in bytes {
                use std::fmt::Write as _;
                write!(&mut hex, "{byte:02x}").map_err(|_| ())?;
            }
            hex
        }
        _ => text()?,
    };
    Ok(Some(value))
}

// ─── Field extraction helpers ─────────────────────────────────────────────────

/// Extract a big-endian `i64` from field at `idx` (PostgreSQL `BIGINT` binary).
pub fn extract_i64(row: &[Option<Vec<u8>>], idx: usize) -> Option<i64> {
    let bytes = row.get(idx)?.as_ref()?;
    if bytes.len() == 8 {
        Some(i64::from_be_bytes(
            bytes.as_slice().try_into().expect("length checked"),
        ))
    } else {
        None
    }
}

/// Extract a UTF-8 string from field at `idx` (PostgreSQL `VARCHAR`/`TEXT` binary).
pub fn extract_varchar(row: &[Option<Vec<u8>>], idx: usize) -> Option<String> {
    let bytes = row.get(idx)?.as_ref()?;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_binary_copy(rows: &[&[Option<Vec<u8>>]]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(PGCOPY_SIGNATURE);
        buf.extend_from_slice(&0i32.to_be_bytes()); // flags
        buf.extend_from_slice(&0i32.to_be_bytes()); // header ext len
        for row in rows {
            buf.extend_from_slice(&(row.len() as i16).to_be_bytes());
            for field in *row {
                match field {
                    None => buf.extend_from_slice(&(-1i32).to_be_bytes()),
                    Some(v) => {
                        buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
                        buf.extend_from_slice(v);
                    }
                }
            }
        }
        buf.extend_from_slice(&(-1i16).to_be_bytes()); // EOF trailer
        buf
    }

    #[test]
    fn parse_empty_copy() {
        let data = build_binary_copy(&[]);
        let rows = parse_binary_copy_rows(&data).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn parse_single_int8_row() {
        let val: i64 = 42;
        let bytes = val.to_be_bytes().to_vec();
        let row: &[Option<Vec<u8>>] = &[Some(bytes)];
        let data = build_binary_copy(&[row]);
        let rows = parse_binary_copy_rows(&data).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(extract_i64(&rows[0], 0), Some(42));
    }

    #[test]
    fn parse_null_field() {
        let row: &[Option<Vec<u8>>] = &[None, Some(b"hello".to_vec())];
        let data = build_binary_copy(&[row]);
        let rows = parse_binary_copy_rows(&data).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], None);
        assert_eq!(extract_varchar(&rows[0], 1), Some("hello".to_string()));
    }

    #[test]
    fn parse_schema_name_at_field_4() {
        // ducklake_schema: schema_id, schema_uuid, begin_snapshot, end_snapshot, schema_name, path, path_is_relative
        let schema_id = 1i64.to_be_bytes().to_vec();
        let schema_uuid = vec![0u8; 16]; // 16-byte UUID
        let begin_snap = 1i64.to_be_bytes().to_vec();
        let schema_name = b"main".to_vec();
        let row: &[Option<Vec<u8>>] = &[
            Some(schema_id),
            Some(schema_uuid),
            Some(begin_snap),
            None, // end_snapshot NULL
            Some(schema_name),
            None, // path NULL
            None, // path_is_relative NULL
        ];
        let data = build_binary_copy(&[row]);
        let rows = parse_binary_copy_rows(&data).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(extract_varchar(&rows[0], 4), Some("main".to_string()));
    }

    #[test]
    fn decode_inlined_copy_preserves_variant_bytes_and_integer_values() {
        let row: &[Option<Vec<u8>>] = &[
            Some(0i64.to_be_bytes().to_vec()),
            Some(3i64.to_be_bytes().to_vec()),
            None,
            Some(1i32.to_be_bytes().to_vec()),
            Some(b"variant-binary".to_vec()),
        ];
        let rows = decode_inlined_copy_rows(
            &[row.to_vec()],
            &["INTEGER".to_string(), "VARIANT".to_string()],
        )
        .unwrap();
        assert_eq!(rows[0][0].as_deref(), Some("0"));
        assert_eq!(rows[0][3].as_deref(), Some("1"));
        assert_eq!(
            rows[0][4].as_deref(),
            Some("\\x76617269616e742d62696e617279")
        );
    }

    // ── Fail-closed truncation tests ──────────────────────────────────────

    #[test]
    fn error_on_too_short() {
        let err = parse_binary_copy_rows(b"PGCOPY").unwrap_err();
        assert_eq!(err, CopyParseError::TooShort);
    }

    #[test]
    fn error_on_bad_signature() {
        // Valid length but wrong signature bytes.
        let mut data = vec![0u8; PGCOPY_SIGNATURE.len() + 8];
        data[0] = b'X'; // Corrupt first byte.
        let err = parse_binary_copy_rows(&data).unwrap_err();
        assert_eq!(err, CopyParseError::BadSignature);
    }

    #[test]
    fn error_on_truncation_after_signature() {
        // Valid signature + flags but stream ends before header ext length.
        let mut data = Vec::new();
        data.extend_from_slice(PGCOPY_SIGNATURE);
        data.extend_from_slice(&0i32.to_be_bytes()); // flags
                                                     // Deliberately omit the header ext length field.
        let err = parse_binary_copy_rows(&data).unwrap_err();
        assert!(
            matches!(
                err,
                CopyParseError::TooShort | CopyParseError::TruncatedHeaderExt
            ),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn error_on_truncated_field_count() {
        // Well-formed header but stream ends before the first row's field_count.
        let mut data = Vec::new();
        data.extend_from_slice(PGCOPY_SIGNATURE);
        data.extend_from_slice(&0i32.to_be_bytes()); // flags
        data.extend_from_slice(&0i32.to_be_bytes()); // header ext len = 0
                                                     // Only one byte where two are needed for the int16 field_count.
        data.push(0x00);
        let err = parse_binary_copy_rows(&data).unwrap_err();
        assert_eq!(err, CopyParseError::TruncatedFieldCount { row: 0 });
    }

    #[test]
    fn error_on_mid_field_length() {
        // Announce field_count=1 but provide no field length bytes.
        let mut data = Vec::new();
        data.extend_from_slice(PGCOPY_SIGNATURE);
        data.extend_from_slice(&0i32.to_be_bytes()); // flags
        data.extend_from_slice(&0i32.to_be_bytes()); // header ext len = 0
        data.extend_from_slice(&1i16.to_be_bytes()); // field_count = 1
                                                     // Provide only 2 of the 4 needed field-length bytes.
        data.push(0x00);
        data.push(0x00);
        let err = parse_binary_copy_rows(&data).unwrap_err();
        assert_eq!(
            err,
            CopyParseError::TruncatedFieldLength { row: 0, field: 0 }
        );
    }

    #[test]
    fn error_on_mid_field_body() {
        // Announce a 10-byte field but supply only 3 bytes.
        let mut data = Vec::new();
        data.extend_from_slice(PGCOPY_SIGNATURE);
        data.extend_from_slice(&0i32.to_be_bytes()); // flags
        data.extend_from_slice(&0i32.to_be_bytes()); // header ext len = 0
        data.extend_from_slice(&1i16.to_be_bytes()); // field_count = 1
        data.extend_from_slice(&10i32.to_be_bytes()); // field length = 10
        data.extend_from_slice(&[0u8; 3]); // only 3 bytes instead of 10
        let err = parse_binary_copy_rows(&data).unwrap_err();
        assert_eq!(
            err,
            CopyParseError::TruncatedFieldBody {
                row: 0,
                field: 0,
                declared: 10,
                available: 3,
            }
        );
    }

    #[test]
    fn error_on_missing_end_of_data_marker() {
        // A valid row followed by abrupt end (no -1 trailer).
        let mut data = Vec::new();
        data.extend_from_slice(PGCOPY_SIGNATURE);
        data.extend_from_slice(&0i32.to_be_bytes()); // flags
        data.extend_from_slice(&0i32.to_be_bytes()); // header ext len = 0
                                                     // One row with one NULL field.
        data.extend_from_slice(&1i16.to_be_bytes()); // field_count = 1
        data.extend_from_slice(&(-1i32).to_be_bytes()); // NULL field
                                                        // Stream ends here — no end-of-data marker.
        let err = parse_binary_copy_rows(&data).unwrap_err();
        assert_eq!(err, CopyParseError::TruncatedFieldCount { row: 1 });
    }
}
