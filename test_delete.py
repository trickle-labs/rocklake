#!/usr/bin/env python3
"""Test RockLake DELETE support for DuckLake CHECKPOINT"""

import subprocess
import time

# Start RockLake server
print("Starting RockLake server...")
proc = subprocess.Popen(
    ["python3", "-m", "rocklake_server"],
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True
)

# Wait for server to start
time.sleep(2)

try:
    # Connect with psql and test DELETE
    test_sql = """
    -- Test DELETE statement parsing
    CREATE TABLE test_delete AS SELECT 1 as id;
    
    -- This should show if DELETE is supported
    DELETE FROM test_delete WHERE id = 1;
    
    SELECT 'DELETE test passed' as result;
    """
    
    result = subprocess.run(
        ["psql", "-h", "localhost", "-p", "5432", "-U", "postgres", "-d", "rocklake"],
        input=test_sql,
        capture_output=True,
        text=True,
        timeout=10
    )
    
    print("Output:", result.stdout)
    print("Errors:", result.stderr)
    
finally:
    # Cleanup
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except:
        proc.kill()
