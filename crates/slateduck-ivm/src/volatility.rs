//! Function volatility classification for IVM correctness gates.
//!
//! DuckDB functions fall into IMMUTABLE, STABLE, and VOLATILE categories.
//! Without this gate, views using `random()` or `clock_timestamp()` produce
//! silently wrong incremental results.
//!
//! This module provides a hardcoded volatility lookup table covering all ~300
//! DuckDB scalar functions. Unknown functions default to VOLATILE (safe-by-default).
//!
//! ## Forward-compatibility (v0.16)
//! v0.16 introduces capture semantics that allow-list specific volatile functions
//! (`random()`, `gen_random_uuid()`, `now()`) with deterministic per-batch sampling.
//! At that point, this module gains a `CaptureEligible` category.

/// Function volatility classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Volatility {
    /// Result depends only on input arguments; same inputs always give same output.
    Immutable,
    /// Result can change between statements but is stable within a single statement/query.
    /// Examples: `now()`, `current_timestamp`, `current_user`.
    Stable,
    /// Result can change between rows within a single statement.
    /// Examples: `random()`, `gen_random_uuid()`, `nextval()`.
    Volatile,
    /// Function is not in the static lookup table.
    Unknown,
}

/// Look up the volatility of a DuckDB scalar function by name.
///
/// Names should be lowercase. Returns `Volatility::Unknown` for functions
/// not in the static table.
pub fn volatility_of(name: &str) -> Volatility {
    match name {
        // ── VOLATILE functions ──────────────────────────────────────────
        "random" | "setseed" | "gen_random_uuid" | "uuid" | "nextval"
        | "currval" | "clock_timestamp" | "statement_timestamp"
        | "transaction_timestamp" | "timeofday" => Volatility::Volatile,

        // ── STABLE functions ───────────────────────────────────────────
        "now" | "current_timestamp" | "current_date" | "current_time"
        | "localtime" | "localtimestamp" | "current_user" | "current_schema"
        | "current_database" | "current_catalog" | "current_role"
        | "session_user" | "user" | "version" | "current_setting"
        | "inet_client_addr" | "inet_client_port" | "inet_server_addr"
        | "inet_server_port" | "pg_backend_pid" | "pg_postmaster_start_time"
        | "age" => Volatility::Stable,

        // ── IMMUTABLE: Math functions ──────────────────────────────────
        "abs" | "acos" | "acosh" | "asin" | "asinh" | "atan" | "atanh"
        | "atan2" | "cbrt" | "ceil" | "ceiling" | "cos" | "cosh" | "cot"
        | "degrees" | "exp" | "factorial" | "floor" | "gamma" | "gcd"
        | "greatest" | "isfinite" | "isinf" | "isnan" | "lcm" | "least"
        | "ln" | "log" | "log10" | "log2" | "lgamma" | "mod" | "pi"
        | "pow" | "power" | "radians" | "round" | "sign" | "sin" | "sinh"
        | "sqrt" | "tan" | "tanh" | "trunc" | "truncate" | "even"
        | "bit_count" => Volatility::Immutable,

        // ── IMMUTABLE: String functions ────────────────────────────────
        "ascii" | "bit_length" | "char" | "chr" | "char_length"
        | "character_length" | "concat" | "concat_ws" | "contains"
        | "ends_with" | "starts_with" | "format" | "printf" | "hash"
        | "hex" | "instr" | "left" | "length" | "len" | "like_escape"
        | "lower" | "lpad" | "ltrim" | "md5" | "nfc_normalize"
        | "octet_length" | "ord" | "position" | "prefix" | "repeat"
        | "replace" | "reverse" | "right" | "rpad" | "rtrim" | "split"
        | "split_part" | "string_split" | "string_split_regex"
        | "strip_accents" | "strlen" | "strpos" | "substr" | "substring"
        | "suffix" | "to_base" | "from_base" | "translate" | "trim"
        | "unicode" | "upper" | "levenshtein" | "mismatches"
        | "jaccard" | "jaro_similarity" | "jaro_winkler_similarity"
        | "damerau_levenshtein" | "editdist3" | "hamming" => Volatility::Immutable,

        // ── IMMUTABLE: Regex / pattern functions ───────────────────────
        "regexp_matches" | "regexp_replace" | "regexp_extract"
        | "regexp_extract_all" | "regexp_full_match" | "regexp_split_to_array"
        | "regexp_split_to_table" => Volatility::Immutable,

        // ── IMMUTABLE: Date/time functions ─────────────────────────────
        "date_part" | "datepart" | "date_diff" | "datediff" | "date_trunc"
        | "datetrunc" | "date_add" | "date_sub" | "extract" | "epoch"
        | "epoch_ms" | "epoch_us" | "epoch_ns" | "make_date" | "make_time"
        | "make_timestamp" | "strftime" | "strptime" | "to_timestamp"
        | "to_date" | "to_time" | "year" | "month" | "day" | "hour"
        | "minute" | "second" | "millisecond" | "microsecond" | "dayname"
        | "dayofmonth" | "dayofweek" | "dayofyear" | "era" | "isodow"
        | "isoyear" | "julian" | "last_day" | "monthname" | "quarter"
        | "timezone" | "timezone_hour" | "timezone_minute" | "week"
        | "weekday" | "weekofyear" | "yearweek" => Volatility::Immutable,

        // ── IMMUTABLE: Type cast / conversion ──────────────────────────
        "cast" | "try_cast" | "typeof" | "type_of" | "coalesce"
        | "ifnull" | "nullif" | "if" | "iff" | "case" | "decode"
        | "encode" | "to_char" | "to_hex" | "from_hex" | "to_binary"
        | "from_binary" | "base64" | "from_base64" => Volatility::Immutable,

        // ── IMMUTABLE: Comparison / logic ──────────────────────────────
        "between" | "in" | "not" | "and" | "or" | "is_null" | "is_not_null"
        | "equal" | "not_equal" | "less_than" | "less_than_or_equal"
        | "greater_than" | "greater_than_or_equal" => Volatility::Immutable,

        // ── IMMUTABLE: List/Array functions ────────────────────────────
        "array_length" | "array_extract" | "array_slice" | "array_pop_back"
        | "array_pop_front" | "array_push_back" | "array_push_front"
        | "array_contains" | "array_position" | "array_append"
        | "array_prepend" | "array_cat" | "array_reverse" | "array_sort"
        | "array_distinct" | "array_unique" | "array_to_string"
        | "list_value" | "list_element" | "list_extract" | "list_slice"
        | "list_contains" | "list_position" | "list_sort" | "list_reverse_sort"
        | "list_distinct" | "list_unique" | "list_cat" | "list_append"
        | "list_prepend" | "list_aggregate" | "list_filter" | "list_transform"
        | "list_reduce" | "list_any_value" | "flatten" | "unnest"
        | "generate_series" | "range" => Volatility::Immutable,

        // ── IMMUTABLE: Map/Struct functions ────────────────────────────
        "map" | "map_from_entries" | "map_entries" | "map_keys" | "map_values"
        | "map_extract" | "map_contains" | "struct_pack" | "struct_extract"
        | "struct_insert" | "row" => Volatility::Immutable,

        // ── IMMUTABLE: JSON functions ──────────────────────────────────
        "json" | "to_json" | "from_json" | "json_extract" | "json_extract_string"
        | "json_extract_path" | "json_extract_path_text" | "json_type"
        | "json_valid" | "json_array_length" | "json_keys" | "json_contains"
        | "json_object" | "json_array" | "json_merge_patch"
        | "json_transform" | "json_serialize_sql" | "json_deserialize_sql"
        | "json_quote" => Volatility::Immutable,

        // ── IMMUTABLE: Bitwise / binary functions ──────────────────────
        "bit_and" | "bit_or" | "bit_xor" | "bit_not" | "bit_shift_left"
        | "bit_shift_right" | "get_bit" | "set_bit" => Volatility::Immutable,

        // ── IMMUTABLE: Hashing / crypto ────────────────────────────────
        "sha1" | "sha256" | "sha224" | "sha384" | "sha512"
        | "crc32" => Volatility::Immutable,

        // ── IMMUTABLE: Misc scalar functions ───────────────────────────
        "alias" | "current_query" | "error"
        | "rowid" | "pg_typeof" | "oid" | "txid_current"
        | "constant_or_null" | "stats" | "format_bytes" | "bar"
        => Volatility::Immutable,

        // Unknown function — not in our static table.
        _ => Volatility::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volatile_functions_classified() {
        assert_eq!(volatility_of("random"), Volatility::Volatile);
        assert_eq!(volatility_of("gen_random_uuid"), Volatility::Volatile);
        assert_eq!(volatility_of("nextval"), Volatility::Volatile);
        assert_eq!(volatility_of("clock_timestamp"), Volatility::Volatile);
    }

    #[test]
    fn stable_functions_classified() {
        assert_eq!(volatility_of("now"), Volatility::Stable);
        assert_eq!(volatility_of("current_timestamp"), Volatility::Stable);
        assert_eq!(volatility_of("current_user"), Volatility::Stable);
        assert_eq!(volatility_of("current_date"), Volatility::Stable);
    }

    #[test]
    fn immutable_functions_classified() {
        assert_eq!(volatility_of("abs"), Volatility::Immutable);
        assert_eq!(volatility_of("lower"), Volatility::Immutable);
        assert_eq!(volatility_of("round"), Volatility::Immutable);
        assert_eq!(volatility_of("length"), Volatility::Immutable);
        assert_eq!(volatility_of("date_part"), Volatility::Immutable);
        assert_eq!(volatility_of("coalesce"), Volatility::Immutable);
        assert_eq!(volatility_of("json_extract"), Volatility::Immutable);
    }

    #[test]
    fn unknown_function_returns_unknown() {
        assert_eq!(volatility_of("my_custom_udf"), Volatility::Unknown);
        assert_eq!(volatility_of("nonexistent_func"), Volatility::Unknown);
    }
}
