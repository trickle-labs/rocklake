// Test program to debug DELETE statement classification
use rocklake_sql::classifier::classify_statement;

fn main() {
    let test_statements = vec![
        // Original with schema prefix
        r#"DELETE FROM "public".ducklake_data_file WHERE data_file_id IN (1, 2, 3)"#,
        // Normalized (after strip_public_schema_prefix)
        r#"DELETE FROM ducklake_data_file WHERE data_file_id IN (1, 2, 3)"#,
        // Without quotes
        r#"DELETE FROM public.ducklake_data_file WHERE data_file_id IN (1, 2, 3)"#,
        // Lowercase
        r#"delete from ducklake_data_file where data_file_id in (1, 2, 3)"#,
    ];

    for sql in &test_statements {
        println!("\n=== Testing: {} ===", sql);
        match classify_statement(sql) {
            Ok(kind) => println!("✓ Classified as: {:?}", kind),
            Err(e) => println!("✗ Error: {}", e),
        }
    }
}
