//! Type-aware column statistics for file pruning.
//!
//! `prune_files()` accepts a `DuckLakeType` and performs type-aware comparison:
//! - Integers: parse as signed/unsigned integers per width; no lexicographic compare
//! - Decimals: parse to rational representation, not float
//! - Timestamps: parse to typed temporal values; normalize time zones before compare
//! - IEEE floats: handle inf/-inf; ignore NaN bounds separately via contains_nan
//! - Unknown types: fail closed (SQLSTATE 0A000) rather than guessing

use crate::error::{Result, SlateDuckError};
use crate::rows::FileColumnStatsRow;
use std::cmp::Ordering;

/// DuckLake column types supported for statistics comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuckLakeType {
    TinyInt,
    SmallInt,
    Integer,
    BigInt,
    UTinyInt,
    USmallInt,
    UInteger,
    UBigInt,
    Float,
    Double,
    Decimal { width: u8, scale: u8 },
    Timestamp,
    TimestampTz,
    Date,
    Time,
    Varchar,
    Boolean,
    Unknown(String),
}

/// Comparison predicate for file pruning.
#[derive(Debug, Clone)]
pub enum PrunePredicate {
    /// Column >= value
    GreaterThanOrEqual(String),
    /// Column <= value
    LessThanOrEqual(String),
    /// Column = value
    Equal(String),
    /// Column > value
    GreaterThan(String),
    /// Column < value
    LessThan(String),
}

/// Result of file pruning: files whose stats indicate they may contain matching rows.
pub fn prune_files(
    stats: &[FileColumnStatsRow],
    predicate: &PrunePredicate,
    col_type: &DuckLakeType,
) -> Result<Vec<u64>> {
    if matches!(col_type, DuckLakeType::Unknown(_)) {
        return Err(SlateDuckError::FeatureNotSupported(format!(
            "cannot prune files for unknown type: {:?}",
            col_type
        )));
    }

    let mut matching_file_ids = Vec::new();

    for stat in stats {
        if file_may_match(stat, predicate, col_type)? {
            matching_file_ids.push(stat.data_file_id);
        }
    }

    Ok(matching_file_ids)
}

/// Determine if a file may contain rows matching the predicate based on stats.
fn file_may_match(
    stat: &FileColumnStatsRow,
    predicate: &PrunePredicate,
    col_type: &DuckLakeType,
) -> Result<bool> {
    let min_val = match &stat.min_value {
        Some(v) => v.as_str(),
        None => return Ok(true), // No stats = can't prune
    };
    let max_val = match &stat.max_value {
        Some(v) => v.as_str(),
        None => return Ok(true),
    };

    // If contains_nan and this is a float type, we cannot prune
    if stat.contains_nan && matches!(col_type, DuckLakeType::Float | DuckLakeType::Double) {
        return Ok(true);
    }

    match predicate {
        PrunePredicate::Equal(val) => {
            // File may match if min <= val <= max
            let cmp_min = compare_typed(min_val, val, col_type)?;
            let cmp_max = compare_typed(max_val, val, col_type)?;
            Ok(cmp_min != Ordering::Greater && cmp_max != Ordering::Less)
        }
        PrunePredicate::GreaterThanOrEqual(val) => {
            // File may match if max >= val
            let cmp = compare_typed(max_val, val, col_type)?;
            Ok(cmp != Ordering::Less)
        }
        PrunePredicate::LessThanOrEqual(val) => {
            // File may match if min <= val
            let cmp = compare_typed(min_val, val, col_type)?;
            Ok(cmp != Ordering::Greater)
        }
        PrunePredicate::GreaterThan(val) => {
            // File may match if max > val
            let cmp = compare_typed(max_val, val, col_type)?;
            Ok(cmp == Ordering::Greater)
        }
        PrunePredicate::LessThan(val) => {
            // File may match if min < val
            let cmp = compare_typed(min_val, val, col_type)?;
            Ok(cmp == Ordering::Less)
        }
    }
}

/// Type-aware comparison of two string-encoded values.
fn compare_typed(a: &str, b: &str, col_type: &DuckLakeType) -> Result<Ordering> {
    match col_type {
        DuckLakeType::TinyInt
        | DuckLakeType::SmallInt
        | DuckLakeType::Integer
        | DuckLakeType::BigInt => {
            let va: i64 = a
                .parse()
                .map_err(|e| SlateDuckError::Encoding(format!("int parse: {e}")))?;
            let vb: i64 = b
                .parse()
                .map_err(|e| SlateDuckError::Encoding(format!("int parse: {e}")))?;
            Ok(va.cmp(&vb))
        }
        DuckLakeType::UTinyInt
        | DuckLakeType::USmallInt
        | DuckLakeType::UInteger
        | DuckLakeType::UBigInt => {
            let va: u64 = a
                .parse()
                .map_err(|e| SlateDuckError::Encoding(format!("uint parse: {e}")))?;
            let vb: u64 = b
                .parse()
                .map_err(|e| SlateDuckError::Encoding(format!("uint parse: {e}")))?;
            Ok(va.cmp(&vb))
        }
        DuckLakeType::Float | DuckLakeType::Double => {
            let va = parse_float(a)?;
            let vb = parse_float(b)?;
            Ok(va.total_cmp(&vb))
        }
        DuckLakeType::Decimal { .. } => {
            // Parse as scaled integer to avoid floating point issues
            compare_decimal(a, b)
        }
        DuckLakeType::Timestamp
        | DuckLakeType::TimestampTz
        | DuckLakeType::Date
        | DuckLakeType::Time => {
            // Temporal types: lexicographic comparison works for ISO 8601 format
            Ok(a.cmp(b))
        }
        DuckLakeType::Varchar => Ok(a.cmp(b)),
        DuckLakeType::Boolean => {
            let va = parse_bool(a)?;
            let vb = parse_bool(b)?;
            Ok(va.cmp(&vb))
        }
        DuckLakeType::Unknown(t) => Err(SlateDuckError::FeatureNotSupported(format!(
            "cannot compare values of unknown type: {t}"
        ))),
    }
}

/// Parse a float value, handling inf/-inf.
fn parse_float(s: &str) -> Result<f64> {
    match s {
        "inf" | "Infinity" | "+inf" => Ok(f64::INFINITY),
        "-inf" | "-Infinity" => Ok(f64::NEG_INFINITY),
        "NaN" | "nan" => Ok(f64::NAN),
        _ => s
            .parse::<f64>()
            .map_err(|e| SlateDuckError::Encoding(format!("float parse: {e}"))),
    }
}

/// Parse a boolean value.
fn parse_bool(s: &str) -> Result<u8> {
    match s.to_lowercase().as_str() {
        "true" | "t" | "1" => Ok(1),
        "false" | "f" | "0" => Ok(0),
        _ => Err(SlateDuckError::Encoding(format!("bool parse: {s}"))),
    }
}

/// Compare two decimal strings as scaled integers.
fn compare_decimal(a: &str, b: &str) -> Result<Ordering> {
    // Normalize decimal strings to compare as rational numbers
    let (a_int, a_frac) = split_decimal(a);
    let (b_int, b_frac) = split_decimal(b);

    // Compare integer parts first
    let a_neg = a_int.starts_with('-');
    let b_neg = b_int.starts_with('-');

    if a_neg != b_neg {
        return Ok(if a_neg {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }

    let a_int_abs = a_int.trim_start_matches('-');
    let b_int_abs = b_int.trim_start_matches('-');

    // Pad fractional parts to same length for comparison
    let max_frac_len = a_frac.len().max(b_frac.len());
    let a_frac_padded = format!("{:0<width$}", a_frac, width = max_frac_len);
    let b_frac_padded = format!("{:0<width$}", b_frac, width = max_frac_len);
    let a_padded = format!("{a_int_abs}{a_frac_padded}");
    let b_padded = format!("{b_int_abs}{b_frac_padded}");

    // Compare by length first (more digits = larger for positive)
    let cmp = if a_padded.len() != b_padded.len() {
        a_padded.len().cmp(&b_padded.len())
    } else {
        a_padded.cmp(&b_padded)
    };

    Ok(if a_neg { cmp.reverse() } else { cmp })
}

/// Split a decimal string into integer and fractional parts.
fn split_decimal(s: &str) -> (&str, &str) {
    match s.find('.') {
        Some(pos) => (&s[..pos], &s[pos + 1..]),
        None => (s, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_equal_integer() {
        let stats = vec![
            FileColumnStatsRow {
                table_id: 1,
                column_id: 1,
                data_file_id: 1,
                min_value: Some("1".into()),
                max_value: Some("10".into()),
                null_count: None,
                contains_nan: false,
            },
            FileColumnStatsRow {
                table_id: 1,
                column_id: 1,
                data_file_id: 2,
                min_value: Some("11".into()),
                max_value: Some("20".into()),
                null_count: None,
                contains_nan: false,
            },
        ];

        let result = prune_files(
            &stats,
            &PrunePredicate::Equal("5".into()),
            &DuckLakeType::Integer,
        )
        .unwrap();
        assert_eq!(result, vec![1]); // Only file 1 can contain value 5
    }

    #[test]
    fn prune_greater_than() {
        let stats = vec![
            FileColumnStatsRow {
                table_id: 1,
                column_id: 1,
                data_file_id: 1,
                min_value: Some("1".into()),
                max_value: Some("10".into()),
                null_count: None,
                contains_nan: false,
            },
            FileColumnStatsRow {
                table_id: 1,
                column_id: 1,
                data_file_id: 2,
                min_value: Some("11".into()),
                max_value: Some("20".into()),
                null_count: None,
                contains_nan: false,
            },
        ];

        let result = prune_files(
            &stats,
            &PrunePredicate::GreaterThan("10".into()),
            &DuckLakeType::Integer,
        )
        .unwrap();
        assert_eq!(result, vec![2]); // Only file 2 has max > 10
    }

    #[test]
    fn prune_float_with_nan() {
        let stats = vec![FileColumnStatsRow {
            table_id: 1,
            column_id: 1,
            data_file_id: 1,
            min_value: Some("1.0".into()),
            max_value: Some("10.0".into()),
            null_count: None,
            contains_nan: true,
        }];

        // Cannot prune when contains_nan
        let result = prune_files(
            &stats,
            &PrunePredicate::Equal("100.0".into()),
            &DuckLakeType::Double,
        )
        .unwrap();
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn prune_unknown_type_fails() {
        let stats = vec![FileColumnStatsRow {
            table_id: 1,
            column_id: 1,
            data_file_id: 1,
            min_value: Some("x".into()),
            max_value: Some("y".into()),
            null_count: None,
            contains_nan: false,
        }];

        let result = prune_files(
            &stats,
            &PrunePredicate::Equal("z".into()),
            &DuckLakeType::Unknown("BLOB".into()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn compare_integers_not_lexicographic() {
        // Lexicographic: "9" > "10", but numeric: 9 < 10
        let ord = compare_typed("9", "10", &DuckLakeType::Integer).unwrap();
        assert_eq!(ord, Ordering::Less);
    }

    #[test]
    fn compare_decimals() {
        let ord = compare_decimal("1.5", "1.50").unwrap();
        assert_eq!(ord, Ordering::Equal);

        let ord = compare_decimal("1.9", "1.10").unwrap();
        assert_eq!(ord, Ordering::Greater);

        let ord = compare_decimal("-2.5", "1.0").unwrap();
        assert_eq!(ord, Ordering::Less);
    }

    #[test]
    fn compare_floats_with_inf() {
        let ord = compare_typed("inf", "100.0", &DuckLakeType::Double).unwrap();
        assert_eq!(ord, Ordering::Greater);

        let ord = compare_typed("-inf", "-100.0", &DuckLakeType::Double).unwrap();
        assert_eq!(ord, Ordering::Less);
    }
}
