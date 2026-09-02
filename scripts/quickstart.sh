#!/usr/bin/env bash
set -euo pipefail

command -v duckdb >/dev/null || { echo "duckdb is required" >&2; exit 1; }

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
catalog_dir=$(mktemp -d)
data_dir=data
mkdir -p "$catalog_dir/$data_dir"
cd "$catalog_dir"
port=${ROCKLAKE_QUICKSTART_PORT:-$((50000 + $$ % 10000))}
binary=${ROCKLAKE_BINARY:-"$repo_dir/target/debug/rocklake"}

cleanup() {
  if [[ -n "${server_pid:-}" ]]; then kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; fi
  rm -rf "$catalog_dir"
}
trap cleanup EXIT

if [[ ! -x "$binary" ]]; then
  cargo build --manifest-path "$repo_dir/Cargo.toml" -p rocklake-pgwire --bin rocklake
fi

"$binary" serve --catalog "file://$catalog_dir/catalog" --bind "127.0.0.1:$port" >"$catalog_dir/server.log" 2>&1 &
server_pid=$!

ready=false
for _ in $(seq 1 30); do
  if duckdb -csv -noheader -c "LOAD ducklake; ATTACH 'ducklake:postgres:host=127.0.0.1 port=$port dbname=rocklake' AS lake (DATA_PATH '$data_dir'); SELECT 1;" >/dev/null 2>&1; then ready=true; break; fi
  kill -0 "$server_pid" 2>/dev/null || break
  sleep 1
done
[[ "$ready" == true ]] || { cat "$catalog_dir/server.log" >&2; exit 1; }
kill -0 "$server_pid"

duckdb -csv -noheader -c "
  LOAD ducklake;
  ATTACH 'ducklake:postgres:host=127.0.0.1 port=$port dbname=rocklake' AS lake (DATA_PATH '$data_dir', DATA_INLINING_ROW_LIMIT 0);
  CREATE SCHEMA lake.demo;
  CREATE TABLE lake.demo.events (id INTEGER, name VARCHAR);
  INSERT INTO lake.demo.events VALUES (1, 'launch'), (2, 'release');
  SELECT count(*) FROM lake.demo.events;
" | tail -n 1 | grep -qx '2'

snapshot=$(
  duckdb -csv -noheader -c "LOAD ducklake; ATTACH 'ducklake:postgres:host=127.0.0.1 port=$port dbname=rocklake' AS lake (DATA_PATH '$data_dir', DATA_INLINING_ROW_LIMIT 0); SELECT max(snapshot_id) FROM ducklake_snapshots('lake');" | tail -n 1
)
[[ "$snapshot" =~ ^[0-9]+$ && "$snapshot" -gt 0 ]]

duckdb -csv -noheader -c "LOAD ducklake; ATTACH 'ducklake:postgres:host=127.0.0.1 port=$port dbname=rocklake' AS lake (DATA_PATH '$data_dir', DATA_INLINING_ROW_LIMIT 0); INSERT INTO lake.demo.events VALUES (3, 'current'); SELECT count(*) FROM lake.demo.events;" | tail -n 1 | grep -qx '3'

kill "$server_pid"
wait "$server_pid" || true
"$binary" inspect snapshot --catalog "$catalog_dir/catalog" >/dev/null
"$binary" export-catalog --catalog "$catalog_dir/catalog" --at-snapshot "$snapshot" --out "$catalog_dir/historical.ndjson" >/dev/null
grep -q '"snapshot_id":'"$snapshot" "$catalog_dir/historical.ndjson"
grep -q '"table":"ducklake_data_file".*"record_count":2' "$catalog_dir/historical.ndjson"
"$binary" doctor --catalog "$catalog_dir/catalog" --output json | grep '"ready": true' >/dev/null
"$binary" backup create --catalog "$catalog_dir/catalog" --out "$catalog_dir/backup" >/dev/null
"$binary" backup inspect "$catalog_dir/backup" --output json | grep '"version": 1' >/dev/null
"$binary" restore plan --backup "$catalog_dir/backup" --catalog "$catalog_dir/restored" --output json | grep '"target_empty": true' >/dev/null
"$binary" restore apply --backup "$catalog_dir/backup" --catalog "$catalog_dir/restored" --output json | grep '"verified":true\|"verified": true' >/dev/null
"$binary" restore apply --backup "$catalog_dir/backup" --catalog "$catalog_dir/restored" --overwrite --output json | grep '"verified":true\|"verified": true' >/dev/null
"$binary" inspect snapshot --catalog "$catalog_dir/restored" --output json | grep '"latest_snapshot_id"' >/dev/null
"$binary" diagnose --catalog "$catalog_dir/catalog" --json | grep -q 'overall_status.*ok'
echo "v0.51.3 quickstart passed (latest snapshot: $snapshot)"
