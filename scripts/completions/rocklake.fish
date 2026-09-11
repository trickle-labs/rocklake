# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_rocklake_global_optspecs
    string join \n config= h/help V/version
end

function __fish_rocklake_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_rocklake_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_rocklake_using_subcommand
    set -l cmd (__fish_rocklake_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c rocklake -n "__fish_rocklake_needs_command" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_needs_command" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_needs_command" -s V -l version -d 'Print version'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "serve" -d 'Start the PG-Wire sidecar server'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "doctor" -d 'Run a read-only startup preflight'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "status" -d 'Check catalog and server readiness'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "support" -d 'Create a redacted operator support bundle'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "capacity" -d 'Report measured catalog capacity and projected cost'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "catalog" -d 'Catalog lifecycle and maintenance operations'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "catalogs" -d 'Inspect and validate static multi-catalog routes'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "registry" -d 'Initialize and verify the managed catalog registry'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "debug" -d 'Diagnostic and debugging operations'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "config" -d 'Validate or print configuration'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "completions" -d 'Generate shell completion scripts'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "backup" -d 'Create and inspect portable catalog backups'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "restore" -d 'Plan or apply a backup restore'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "gc" -d 'Visibility GC — advance the retain-from watermark'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "excise" -d 'Physical excision of catalog facts before a snapshot'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "checkpoint" -d 'Manage catalog checkpoints'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "export" -d 'Export catalog to NDJSON'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "import" -d 'Import catalog from NDJSON'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "pg-migrate" -d 'Convert NDJSON export to PostgreSQL INSERT statements'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "rebuild" -d 'Rebuild catalog by scanning Parquet files in object storage'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "inspect" -d 'Inspect catalog state (snapshot, API costs, cache utilisation)'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "verify" -d 'Verify catalog integrity'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "repair" -d 'Repair catalog issues'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "warmup" -d 'Warm up the block cache before serving'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "migrate" -d 'Migrate catalog to the current format version'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "corpus" -d 'Wire-corpus operations (diff and validate)'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "tune" -d 'Output recommended settings for a target cost'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "migrate-from-ducklake" -d 'Migrate from an existing DuckLake catalog into RockLake'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "export-catalog" -d 'Export all 28+ DuckLake catalog tables to NDJSON'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "diagnose" -d 'Structured catalog health diagnostic report'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "sweep-orphans" -d 'Identify (and optionally delete) orphan Parquet files'
complete -c rocklake -n "__fish_rocklake_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -s c -l catalog -d 'Catalog URL (`file:///…`, `s3://…`, `gs://…`, `az://…`)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -s b -l bind -d 'Bind address [default: 127.0.0.1:5432]' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l max-sessions -d 'Maximum concurrent sessions' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l metrics-port -d 'Port for the Prometheus `/metrics` HTTP endpoint' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l metrics-path -d 'HTTP path for the metrics endpoint' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l tls-cert -d 'Path to TLS certificate file' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l tls-key -d 'Path to TLS private key file' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l auth-user -d 'Username for PG-Wire authentication' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l auth-password -d 'Password for PG-Wire authentication' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l auth-password-file -d 'Read the PG-Wire authentication password from a file' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l auth-verifier-file -d 'JSON file containing `{"username":"…","scram_verifier":"…"}`' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l mode -d 'Serving mode: `writer` (accepts writes) or `reader` (read-only)' -r -f -a "writer\t''
reader\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l cost-mode -d 'Cost/latency preset' -r -f -a "conservative\t''
balanced\t''
latency\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l s3-endpoint -d 'S3-compatible endpoint URL (e.g. for MinIO)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l encryption-key -d 'AES-256 encryption key (64 hex digits)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l encryption-key-file -d 'Read the AES-256 encryption key from a file' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l extension-schemas -d 'Comma-separated allowed extension schema names' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l otlp-endpoint -d 'OTLP HTTP endpoint for OpenTelemetry tracing' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l idle-connection-timeout -d 'Close idle connections after this many seconds (default: 60)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l drain-timeout -d 'Maximum seconds to wait for in-flight queries during SIGTERM drain (default: 30)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l datafusion-bridge-queue-depth -d 'Capacity of the DataFusion AsyncBridge channel (default: 256)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l max-active-scans -d 'Maximum concurrent catalog scans (default: 25)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l stream-queue-depth -d 'Maximum queued stream items (default: 64)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l max-buffered-rows -d 'Maximum rows buffered for a response (default: 1024)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l max-response-bytes -d 'Optional total response-byte policy; omitted means unlimited' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l slow-operation-threshold-ms -d 'Log operations slower than this threshold in milliseconds (default: 1000)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l tls-required -d 'Require TLS for all connections'
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l read-only -d 'Deprecated compatibility alias for `--mode reader`'
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -l s3-path-style -d 'Use S3 path-style addressing'
complete -c rocklake -n "__fish_rocklake_using_subcommand serve" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -s c -l catalog -d 'Catalog URL or local path to preflight' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l mode -d 'Serving mode to validate' -r -f -a "writer\t''
reader\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l bind -d 'Listener address to validate for unsafe exposure' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l tls-cert -d 'TLS certificate path used by the intended server' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l tls-key -d 'TLS private key path used by the intended server' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l auth-user -d 'Authentication username used by the intended server' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l auth-verifier-file -d 'JSON SCRAM verifier file used by the intended server' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l encryption-key -d 'Encryption key or file to validate without printing its contents' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l encryption-key-file -d 'Encryption key file to validate' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand status" -s c -l catalog -d 'Catalog URL (`file:///…`, `s3://…`, `gs://…`, `az://…`)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand status" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand status" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand status" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and not __fish_seen_subcommand_from bundle help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and not __fish_seen_subcommand_from bundle help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and not __fish_seen_subcommand_from bundle help" -f -a "bundle" -d 'Write a redacted diagnostic bundle to a new directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and not __fish_seen_subcommand_from bundle help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and __fish_seen_subcommand_from bundle" -l output -d 'Output directory. It must not already exist' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and __fish_seen_subcommand_from bundle" -s c -l catalog -d 'Catalog URL or local path to include in the bundle' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and __fish_seen_subcommand_from bundle" -l metrics-url -d 'Running Prometheus endpoint to snapshot' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and __fish_seen_subcommand_from bundle" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and __fish_seen_subcommand_from bundle" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and __fish_seen_subcommand_from help" -f -a "bundle" -d 'Write a redacted diagnostic bundle to a new directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand support; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and not __fish_seen_subcommand_from report help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and not __fish_seen_subcommand_from report help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and not __fish_seen_subcommand_from report help" -f -a "report" -d 'Report catalog facts against a selected evidence envelope'
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and not __fish_seen_subcommand_from report help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l pricing-file -d 'JSON pricing file with provider, region, effective date, and rates' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l read-ops-per-second -d 'Observed GET-equivalent reads per second' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l write-ops-per-second -d 'Observed PUT-equivalent writes per second' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l list-ops-per-second -d 'Observed LIST requests per second' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l delete-ops-per-second -d 'Observed DELETE requests per second' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l read-bytes-per-second -d 'Observed bytes read per second' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l write-bytes-per-second -d 'Observed bytes written per second' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l catalog-bytes -d 'Current catalog metadata size in bytes, when measured' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l cache-size-mb -d 'Global and per-catalog cache budget in MiB' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l max-sessions -d 'Configured maximum concurrent sessions' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l max-active-scans -d 'Configured maximum concurrent catalog scans' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l evidence-profile -d 'Evidence profile: small, medium, or large' -r -f -a "small\t''
medium\t''
large\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from report" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from help" -f -a "report" -d 'Report catalog facts against a selected evidence envelope'
complete -c rocklake -n "__fish_rocklake_using_subcommand capacity; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "backup" -d 'Create and inspect portable catalog backups'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "restore" -d 'Plan or apply a backup restore'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "gc" -d 'Visibility GC — advance the retain-from watermark'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "excise" -d 'Physical excision of catalog facts before a snapshot'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "checkpoint" -d 'Manage catalog checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "export" -d 'Export catalog to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "import" -d 'Import catalog from NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "export-catalog" -d 'Export all 28+ DuckLake catalog tables to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "migrate" -d 'Migrate catalog to the current format version'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "verify" -d 'Verify catalog integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "repair" -d 'Repair catalog issues'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "jobs" -d 'Inspect and control durable administrative jobs'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "maintenance" -d 'Manage durable maintenance policies and due work'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "recovery" -d 'Emit a machine-readable recovery-drill report'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "backup-set" -d 'Create, inspect, and restore managed backup sets'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and not __fish_seen_subcommand_from backup restore gc excise checkpoint export import export-catalog migrate verify repair jobs maintenance recovery backup-set help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup" -f -a "create" -d 'Create a snapshot-consistent backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup" -f -a "inspect" -d 'Validate and inspect a backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from restore" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from restore" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from restore" -f -a "plan" -d 'Validate a backup and show its proposed target changes'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from restore" -f -a "apply" -d 'Validate and import a backup into an empty target catalog'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from restore" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from gc" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from gc" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from gc" -f -a "plan" -d 'Show the GC plan without applying it'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from gc" -f -a "apply" -d 'Apply the GC plan (advance retain-from)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from gc" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from excise" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from excise" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from excise" -f -a "plan" -d 'Show the excision plan without deleting anything'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from excise" -f -a "apply" -d 'Apply the excision plan (physically delete old catalog facts)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from excise" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -f -a "create" -d 'Create a new catalog checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -f -a "list" -d 'List existing checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -f -a "restore" -d 'Restore catalog to a saved checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -f -a "pin" -d 'Pin a snapshot under a durable name'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -f -a "unpin" -d 'Remove a named snapshot pin'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -f -a "pins" -d 'List named snapshot pins'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from checkpoint" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export" -l output -d 'Output file path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export" -l snapshot-id -d 'Export only this snapshot ID (default: latest)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from import" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from import" -l input -d 'Input NDJSON file path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from import" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from import" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from import" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export-catalog" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export-catalog" -l out -d 'Output NDJSON file path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export-catalog" -l at-snapshot -d 'Export only this snapshot ID (default: latest)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export-catalog" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export-catalog" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from export-catalog" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from migrate" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from migrate" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from migrate" -l dry-run -d 'Preview migration without writing'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from migrate" -l apply -d 'Apply the migration'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from migrate" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from verify" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from verify" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from verify" -f -a "catalog" -d 'Verify catalog key-value integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from verify" -f -a "data-files" -d 'Verify that all registered data files are accessible'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from verify" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from repair" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from repair" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from repair" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from repair" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from repair" -l dry-run -d 'Preview repairs without applying them'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from repair" -l apply -d 'Apply repairs'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from repair" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from jobs" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from jobs" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from jobs" -f -a "list" -d 'List jobs, newest update first'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from jobs" -f -a "status" -d 'Show one job and its last safe checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from jobs" -f -a "cancel" -d 'Request cancellation at the next safe checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from jobs" -f -a "resume" -d 'Explicitly requeue a failed, cancelled, or abandoned job'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from jobs" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from maintenance" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from maintenance" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from maintenance" -f -a "schedule" -d 'Create or replace a maintenance schedule'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from maintenance" -f -a "list" -d 'List durable maintenance schedules'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from maintenance" -f -a "remove" -d 'Remove a maintenance schedule'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from maintenance" -f -a "run" -d 'Claim due schedules as durable administrative jobs'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from maintenance" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from recovery" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from recovery" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from recovery" -f -a "report" -d 'Record measured results from a completed drill'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from recovery" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup-set" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup-set" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup-set" -f -a "create" -d 'Create a full-service or selected-catalog backup set'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup-set" -f -a "inspect" -d 'Validate a backup set and every child artifact'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup-set" -f -a "plan" -d 'Plan a restore into new registry and catalog prefixes'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup-set" -f -a "apply" -d 'Restore and verify catalogs, publishing them as read-only routes'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from backup-set" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "backup" -d 'Create and inspect portable catalog backups'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "restore" -d 'Plan or apply a backup restore'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "gc" -d 'Visibility GC — advance the retain-from watermark'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "excise" -d 'Physical excision of catalog facts before a snapshot'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "checkpoint" -d 'Manage catalog checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "export" -d 'Export catalog to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "import" -d 'Import catalog from NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "export-catalog" -d 'Export all 28+ DuckLake catalog tables to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "migrate" -d 'Migrate catalog to the current format version'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "verify" -d 'Verify catalog integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "repair" -d 'Repair catalog issues'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "jobs" -d 'Inspect and control durable administrative jobs'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "maintenance" -d 'Manage durable maintenance policies and due work'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "recovery" -d 'Emit a machine-readable recovery-drill report'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "backup-set" -d 'Create, inspect, and restore managed backup sets'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalog; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "validate" -d 'Validate the static route configuration'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "list" -d 'List configured catalogs without opening them'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "status" -d 'Show configured catalogs and current open-handle state'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "create" -d 'Create a new lazy catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "register" -d 'Register an existing catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "promote" -d 'Assign a catalog to a registered node after a generation check'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "activate" -d 'Mark an acquiring assignment write-ready after epoch acquisition'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "rename" -d 'Rename a catalog route alias'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "set-mode" -d 'Change a catalog route mode'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "disable" -d 'Disable a catalog route without deleting bytes'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "enable" -d 'Re-enable a disabled catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "remove" -d 'Detach a route and retain a tombstone; never deletes bytes'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and not __fish_seen_subcommand_from validate list status create register promote activate rename set-mode disable enable remove help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from validate" -l registry -d 'Optional managed registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from validate" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from validate" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from list" -l registry -d 'Optional managed registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from list" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from list" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from status" -l registry -d 'Optional managed registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from status" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from status" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l id -d 'Stable UUID catalog identity' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l alias -d 'One or more PostgreSQL aliases' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l catalog -d 'SlateDB catalog location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l data -d 'Referenced data location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l mode -d 'Access mode' -r -f -a "read-write\t'Read and write'
read-only\t'Read only'"
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l credential-provider -d 'Named credential provider' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l policy-reference -d 'Optional policy identifier' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l id -d 'Stable UUID catalog identity' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l alias -d 'One or more PostgreSQL aliases' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l catalog -d 'SlateDB catalog location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l data -d 'Referenced data location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l mode -d 'Access mode' -r -f -a "read-write\t'Read and write'
read-only\t'Read only'"
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l credential-provider -d 'Named credential provider' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l policy-reference -d 'Optional policy identifier' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from register" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -l id -d 'Stable catalog UUID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -l node-id -d 'Registered node identity' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -l endpoint -d 'Registered node endpoint' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -l expected-generation -d 'Registry generation expected by this promotion' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from promote" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -l id -d 'Stable catalog UUID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -l node-id -d 'Registered node identity' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -l assignment-generation -d 'Per-catalog assignment generation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -l writer-epoch -d 'Catalog writer epoch acquired by the node' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from activate" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from rename" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from rename" -l id -d 'Stable catalog UUID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from rename" -l alias -d 'New PostgreSQL alias' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from rename" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from rename" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from rename" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from set-mode" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from set-mode" -l id -d 'Stable catalog UUID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from set-mode" -l mode -d 'New access mode' -r -f -a "read-write\t'Read and write'
read-only\t'Read only'"
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from set-mode" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from set-mode" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from set-mode" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from disable" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from disable" -l id -d 'Stable catalog UUID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from disable" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from disable" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from disable" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from enable" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from enable" -l id -d 'Stable catalog UUID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from enable" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from enable" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from enable" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from remove" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from remove" -l id -d 'Stable catalog UUID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from remove" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from remove" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "validate" -d 'Validate the static route configuration'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "list" -d 'List configured catalogs without opening them'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "status" -d 'Show configured catalogs and current open-handle state'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a new lazy catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "register" -d 'Register an existing catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "promote" -d 'Assign a catalog to a registered node after a generation check'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "activate" -d 'Mark an acquiring assignment write-ready after epoch acquisition'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "rename" -d 'Rename a catalog route alias'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "set-mode" -d 'Change a catalog route mode'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "disable" -d 'Disable a catalog route without deleting bytes'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "enable" -d 'Re-enable a disabled catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "remove" -d 'Detach a route and retain a tombstone; never deletes bytes'
complete -c rocklake -n "__fish_rocklake_using_subcommand catalogs; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "init" -d 'Initialize an empty registry'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "status" -d 'Show registry generation and lifecycle counts'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "backup" -d 'Back up registry state and audit entries to a local directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "restore" -d 'Restore a backup into an uninitialized registry location'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "verify" -d 'Verify registry state, indexes, prefixes, and audit sequence'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "migrate-static" -d 'Import the v0.54 static route table from the selected config file'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "register-node" -d 'Register a node with an expiring lease'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "renew-node" -d 'Renew a node lease'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and not __fish_seen_subcommand_from init status backup restore verify migrate-static register-node renew-node help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from init" -l registry -d 'Registry location (`file:///…`, `s3://…`, `gs://…`, `az://…`)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from init" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from init" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from status" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from status" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from status" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from backup" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from backup" -l output -d 'Local output directory' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from backup" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from backup" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from restore" -l registry -d 'Target registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from restore" -l input -d 'Local backup directory' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from restore" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from restore" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from verify" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from verify" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from verify" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from verify" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from migrate-static" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from migrate-static" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from migrate-static" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from migrate-static" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -l node-id -d 'Stable node identity' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -l endpoint -d 'Endpoint advertised to the front door' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -l lease-seconds -d 'Lease lifetime in seconds' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -l lease-id -d 'Optional lease identity; generated when omitted' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from register-node" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from renew-node" -l registry -d 'Registry location' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from renew-node" -l node-id -d 'Stable node identity' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from renew-node" -l lease-id -d 'Current lease identity' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from renew-node" -l lease-seconds -d 'New lease lifetime in seconds' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from renew-node" -l request-id -d 'Caller-supplied idempotency key' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from renew-node" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from renew-node" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "init" -d 'Initialize an empty registry'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "status" -d 'Show registry generation and lifecycle counts'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "backup" -d 'Back up registry state and audit entries to a local directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "restore" -d 'Restore a backup into an uninitialized registry location'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "verify" -d 'Verify registry state, indexes, prefixes, and audit sequence'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "migrate-static" -d 'Import the v0.54 static route table from the selected config file'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "register-node" -d 'Register a node with an expiring lease'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "renew-node" -d 'Renew a node lease'
complete -c rocklake -n "__fish_rocklake_using_subcommand registry; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "diagnose" -d 'Structured catalog health diagnostic report'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "inspect" -d 'Inspect catalog state (snapshot, API costs, cache utilisation)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "corpus" -d 'Wire-corpus operations (diff and validate)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "rebuild" -d 'Rebuild catalog by scanning Parquet files in object storage'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "sweep-orphans" -d 'Identify (and optionally delete) orphan Parquet files'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "pg-migrate" -d 'Convert NDJSON export to PostgreSQL INSERT statements'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "tune" -d 'Output recommended settings for a target cost'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "warmup" -d 'Warm up the block cache before serving'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "migrate-from-ducklake" -d 'Migrate from an existing DuckLake catalog into RockLake'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and not __fish_seen_subcommand_from diagnose inspect corpus rebuild sweep-orphans pg-migrate tune warmup migrate-from-ducklake help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from diagnose" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from diagnose" -l output -d 'Output format. `--json` remains as a compatibility alias' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from diagnose" -l data-root -d 'Object-store root containing the data files (enables data-file checks)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from diagnose" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from diagnose" -l json -d 'Emit JSON output instead of human-readable text'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from diagnose" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from inspect" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from inspect" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from inspect" -f -a "snapshot" -d 'Show the current snapshot metadata'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from inspect" -f -a "api-costs" -d 'Show per-operation API cost estimates'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from inspect" -f -a "cache-utilization" -d 'Show block-cache utilisation statistics'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from inspect" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from corpus" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from corpus" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from corpus" -f -a "diff" -d 'Diff two wire-corpus files'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from corpus" -f -a "validate" -d 'Validate a wire-corpus against the server'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from corpus" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from rebuild" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from rebuild" -l data-root -d 'Object-store root containing the Parquet data files' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from rebuild" -l s3-endpoint -d 'S3-compatible endpoint URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from rebuild" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from rebuild" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from rebuild" -l s3-path-style -d 'Use S3 path-style addressing'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from rebuild" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -l data-root -d 'Object-store prefix for data files (e.g. `s3://bucket/data/`)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -l grace-period-hours -d 'Grace period: files newer than this many hours are never deleted' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -l apply -d 'Delete orphan files (default: dry-run only)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from sweep-orphans" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from pg-migrate" -l input -d 'Input NDJSON export file' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from pg-migrate" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from pg-migrate" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from tune" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from tune" -l target-cost-usd -d 'Target monthly cost in USD' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from tune" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from tune" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from warmup" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from warmup" -l tables -d 'Number of tables to warm up (default: all)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from warmup" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from warmup" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from migrate-from-ducklake" -l source -d 'Source: `sqlite:/path/to/catalog.db` or path to an NDJSON dump' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from migrate-from-ducklake" -s c -l catalog -d 'Destination RockLake catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from migrate-from-ducklake" -l accept-version -d 'Accept DuckLake catalog versions beyond the default (v1.0). May be specified multiple times' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from migrate-from-ducklake" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from migrate-from-ducklake" -l dry-run -d 'Preview migration without writing'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from migrate-from-ducklake" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "diagnose" -d 'Structured catalog health diagnostic report'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "inspect" -d 'Inspect catalog state (snapshot, API costs, cache utilisation)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "corpus" -d 'Wire-corpus operations (diff and validate)'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "rebuild" -d 'Rebuild catalog by scanning Parquet files in object storage'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "sweep-orphans" -d 'Identify (and optionally delete) orphan Parquet files'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "pg-migrate" -d 'Convert NDJSON export to PostgreSQL INSERT statements'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "tune" -d 'Output recommended settings for a target cost'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "warmup" -d 'Warm up the block cache before serving'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "migrate-from-ducklake" -d 'Migrate from an existing DuckLake catalog into RockLake'
complete -c rocklake -n "__fish_rocklake_using_subcommand debug; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and not __fish_seen_subcommand_from check example help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and not __fish_seen_subcommand_from check example help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and not __fish_seen_subcommand_from check example help" -f -a "check" -d 'Validate the selected configuration file'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and not __fish_seen_subcommand_from check example help" -f -a "example" -d 'Print a complete example configuration'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and not __fish_seen_subcommand_from check example help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from check" -l file -d 'Configuration file to validate instead of the global --config path' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from check" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from check" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from check" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from example" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from example" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "check" -d 'Validate the selected configuration file'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "example" -d 'Print a complete example configuration'
complete -c rocklake -n "__fish_rocklake_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand completions" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand completions" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and not __fish_seen_subcommand_from create inspect help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and not __fish_seen_subcommand_from create inspect help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and not __fish_seen_subcommand_from create inspect help" -f -a "create" -d 'Create a snapshot-consistent backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and not __fish_seen_subcommand_from create inspect help" -f -a "inspect" -d 'Validate and inspect a backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and not __fish_seen_subcommand_from create inspect help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -l out -d 'Output backup directory' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -l snapshot-id -d 'Snapshot to back up (latest by default)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -l data-root -d 'Object-store root containing referenced data files' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -l include-data -d 'Include referenced data-file inventory'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -l verify-data -d 'HEAD referenced data files and fail when one is missing'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from inspect" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from inspect" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from inspect" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a snapshot-consistent backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from help" -f -a "inspect" -d 'Validate and inspect a backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand backup; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and not __fish_seen_subcommand_from plan apply help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and not __fish_seen_subcommand_from plan apply help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and not __fish_seen_subcommand_from plan apply help" -f -a "plan" -d 'Validate a backup and show its proposed target changes'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and not __fish_seen_subcommand_from plan apply help" -f -a "apply" -d 'Validate and import a backup into an empty target catalog'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and not __fish_seen_subcommand_from plan apply help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -l backup -d 'Backup directory containing manifest.json and catalog.ndjson' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -s c -l catalog -d 'Destination catalog URL or local path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -l overwrite-token -d 'Confirmation token printed by `restore plan` for an existing target' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -l overwrite -d 'Explicitly allow replacing a target catalog after validation'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from plan" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -l backup -d 'Backup directory containing manifest.json and catalog.ndjson' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -s c -l catalog -d 'Destination catalog URL or local path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -l overwrite-token -d 'Confirmation token printed by `restore plan` for an existing target' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -l overwrite -d 'Explicitly allow replacing a target catalog after validation'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from apply" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from help" -f -a "plan" -d 'Validate a backup and show its proposed target changes'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from help" -f -a "apply" -d 'Validate and import a backup into an empty target catalog'
complete -c rocklake -n "__fish_rocklake_using_subcommand restore; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and not __fish_seen_subcommand_from plan apply help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and not __fish_seen_subcommand_from plan apply help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and not __fish_seen_subcommand_from plan apply help" -f -a "plan" -d 'Show the GC plan without applying it'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and not __fish_seen_subcommand_from plan apply help" -f -a "apply" -d 'Apply the GC plan (advance retain-from)'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and not __fish_seen_subcommand_from plan apply help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from plan" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from plan" -l retention-days -d 'Retention period in days (snapshots older than this are eligible for GC)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from plan" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from plan" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from plan" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from plan" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from apply" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from apply" -l retention-days -d 'Retention period in days (snapshots older than this are eligible for GC)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from apply" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from apply" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from apply" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from apply" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from help" -f -a "plan" -d 'Show the GC plan without applying it'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from help" -f -a "apply" -d 'Apply the GC plan (advance retain-from)'
complete -c rocklake -n "__fish_rocklake_using_subcommand gc; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and not __fish_seen_subcommand_from plan apply help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and not __fish_seen_subcommand_from plan apply help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and not __fish_seen_subcommand_from plan apply help" -f -a "plan" -d 'Show the excision plan without deleting anything'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and not __fish_seen_subcommand_from plan apply help" -f -a "apply" -d 'Apply the excision plan (physically delete old catalog facts)'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and not __fish_seen_subcommand_from plan apply help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from plan" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from plan" -l before -d 'Delete facts for all snapshots strictly before this ID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from plan" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from plan" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from plan" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from plan" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from apply" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from apply" -l before -d 'Delete facts for all snapshots strictly before this ID' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from apply" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from apply" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from apply" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from apply" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from help" -f -a "plan" -d 'Show the excision plan without deleting anything'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from help" -f -a "apply" -d 'Apply the excision plan (physically delete old catalog facts)'
complete -c rocklake -n "__fish_rocklake_using_subcommand excise; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -f -a "create" -d 'Create a new catalog checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -f -a "list" -d 'List existing checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -f -a "restore" -d 'Restore catalog to a saved checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -f -a "pin" -d 'Pin a snapshot under a durable name'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -f -a "unpin" -d 'Remove a named snapshot pin'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -f -a "pins" -d 'List named snapshot pins'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list restore pin unpin pins help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from create" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from create" -l label -d 'Human-readable label for the checkpoint' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from create" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from list" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from list" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from restore" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from restore" -l id -d 'ID of the checkpoint to restore' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from restore" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from restore" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pin" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pin" -l name -d 'Name of the durable pin' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pin" -l snapshot -d 'Snapshot ID to retain' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pin" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pin" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from unpin" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from unpin" -l name -d 'Name of the durable pin' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from unpin" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from unpin" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pins" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pins" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from pins" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a new catalog checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "list" -d 'List existing checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "restore" -d 'Restore catalog to a saved checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "pin" -d 'Pin a snapshot under a durable name'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "unpin" -d 'Remove a named snapshot pin'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "pins" -d 'List named snapshot pins'
complete -c rocklake -n "__fish_rocklake_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand export" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export" -l output -d 'Output file path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export" -l snapshot-id -d 'Export only this snapshot ID (default: latest)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand export" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand import" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand import" -l input -d 'Input NDJSON file path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand import" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand import" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand import" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand pg-migrate" -l input -d 'Input NDJSON export file' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand pg-migrate" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand pg-migrate" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand rebuild" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand rebuild" -l data-root -d 'Object-store root containing the Parquet data files' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand rebuild" -l s3-endpoint -d 'S3-compatible endpoint URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand rebuild" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand rebuild" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand rebuild" -l s3-path-style -d 'Use S3 path-style addressing'
complete -c rocklake -n "__fish_rocklake_using_subcommand rebuild" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and not __fish_seen_subcommand_from snapshot api-costs cache-utilization help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and not __fish_seen_subcommand_from snapshot api-costs cache-utilization help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and not __fish_seen_subcommand_from snapshot api-costs cache-utilization help" -f -a "snapshot" -d 'Show the current snapshot metadata'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and not __fish_seen_subcommand_from snapshot api-costs cache-utilization help" -f -a "api-costs" -d 'Show per-operation API cost estimates'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and not __fish_seen_subcommand_from snapshot api-costs cache-utilization help" -f -a "cache-utilization" -d 'Show block-cache utilisation statistics'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and not __fish_seen_subcommand_from snapshot api-costs cache-utilization help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from snapshot" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from snapshot" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from snapshot" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from snapshot" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from api-costs" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from api-costs" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from api-costs" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from api-costs" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from cache-utilization" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from cache-utilization" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from cache-utilization" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from cache-utilization" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from help" -f -a "snapshot" -d 'Show the current snapshot metadata'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from help" -f -a "api-costs" -d 'Show per-operation API cost estimates'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from help" -f -a "cache-utilization" -d 'Show block-cache utilisation statistics'
complete -c rocklake -n "__fish_rocklake_using_subcommand inspect; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and not __fish_seen_subcommand_from catalog data-files help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and not __fish_seen_subcommand_from catalog data-files help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and not __fish_seen_subcommand_from catalog data-files help" -f -a "catalog" -d 'Verify catalog key-value integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and not __fish_seen_subcommand_from catalog data-files help" -f -a "data-files" -d 'Verify that all registered data files are accessible'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and not __fish_seen_subcommand_from catalog data-files help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from catalog" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from catalog" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from catalog" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from catalog" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from catalog" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from data-files" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from data-files" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from data-files" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from data-files" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from data-files" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from help" -f -a "catalog" -d 'Verify catalog key-value integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from help" -f -a "data-files" -d 'Verify that all registered data files are accessible'
complete -c rocklake -n "__fish_rocklake_using_subcommand verify; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand repair" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand repair" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand repair" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand repair" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand repair" -l dry-run -d 'Preview repairs without applying them'
complete -c rocklake -n "__fish_rocklake_using_subcommand repair" -l apply -d 'Apply repairs'
complete -c rocklake -n "__fish_rocklake_using_subcommand repair" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand warmup" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand warmup" -l tables -d 'Number of tables to warm up (default: all)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand warmup" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand warmup" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate" -l dry-run -d 'Preview migration without writing'
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate" -l apply -d 'Apply the migration'
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and not __fish_seen_subcommand_from diff validate help" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and not __fish_seen_subcommand_from diff validate help" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and not __fish_seen_subcommand_from diff validate help" -f -a "diff" -d 'Diff two wire-corpus files'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and not __fish_seen_subcommand_from diff validate help" -f -a "validate" -d 'Validate a wire-corpus against the server'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and not __fish_seen_subcommand_from diff validate help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from diff" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from diff" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from validate" -s c -l catalog -d 'Catalog URL to validate against' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from validate" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from validate" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from help" -f -a "diff" -d 'Diff two wire-corpus files'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from help" -f -a "validate" -d 'Validate a wire-corpus against the server'
complete -c rocklake -n "__fish_rocklake_using_subcommand corpus; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand tune" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand tune" -l target-cost-usd -d 'Target monthly cost in USD' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand tune" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand tune" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate-from-ducklake" -l source -d 'Source: `sqlite:/path/to/catalog.db` or path to an NDJSON dump' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate-from-ducklake" -s c -l catalog -d 'Destination RockLake catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate-from-ducklake" -l accept-version -d 'Accept DuckLake catalog versions beyond the default (v1.0). May be specified multiple times' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate-from-ducklake" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate-from-ducklake" -l dry-run -d 'Preview migration without writing'
complete -c rocklake -n "__fish_rocklake_using_subcommand migrate-from-ducklake" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand export-catalog" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export-catalog" -l out -d 'Output NDJSON file path' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export-catalog" -l at-snapshot -d 'Export only this snapshot ID (default: latest)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export-catalog" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand export-catalog" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand export-catalog" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand diagnose" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand diagnose" -l output -d 'Output format. `--json` remains as a compatibility alias' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand diagnose" -l data-root -d 'Object-store root containing the data files (enables data-file checks)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand diagnose" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand diagnose" -l json -d 'Emit JSON output instead of human-readable text'
complete -c rocklake -n "__fish_rocklake_using_subcommand diagnose" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -s c -l catalog -d 'Catalog URL' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -l data-root -d 'Object-store prefix for data files (e.g. `s3://bucket/data/`)' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -l grace-period-hours -d 'Grace period: files newer than this many hours are never deleted' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -l idempotency-key -d 'Reuse the same job when retrying this operation' -r
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -l output -d 'Output format' -r -f -a "human\t''
json\t''"
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -l config -d 'Optional TOML configuration file (defaults to ./rocklake.toml when present)' -r -F
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -l apply -d 'Delete orphan files (default: dry-run only)'
complete -c rocklake -n "__fish_rocklake_using_subcommand sweep-orphans" -s h -l help -d 'Print help'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "serve" -d 'Start the PG-Wire sidecar server'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "doctor" -d 'Run a read-only startup preflight'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "status" -d 'Check catalog and server readiness'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "support" -d 'Create a redacted operator support bundle'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "capacity" -d 'Report measured catalog capacity and projected cost'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "catalog" -d 'Catalog lifecycle and maintenance operations'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "catalogs" -d 'Inspect and validate static multi-catalog routes'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "registry" -d 'Initialize and verify the managed catalog registry'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "debug" -d 'Diagnostic and debugging operations'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "config" -d 'Validate or print configuration'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "completions" -d 'Generate shell completion scripts'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "backup" -d 'Create and inspect portable catalog backups'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "restore" -d 'Plan or apply a backup restore'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "gc" -d 'Visibility GC — advance the retain-from watermark'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "excise" -d 'Physical excision of catalog facts before a snapshot'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "checkpoint" -d 'Manage catalog checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "export" -d 'Export catalog to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "import" -d 'Import catalog from NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "pg-migrate" -d 'Convert NDJSON export to PostgreSQL INSERT statements'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "rebuild" -d 'Rebuild catalog by scanning Parquet files in object storage'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "inspect" -d 'Inspect catalog state (snapshot, API costs, cache utilisation)'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "verify" -d 'Verify catalog integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "repair" -d 'Repair catalog issues'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "warmup" -d 'Warm up the block cache before serving'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "migrate" -d 'Migrate catalog to the current format version'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "corpus" -d 'Wire-corpus operations (diff and validate)'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "tune" -d 'Output recommended settings for a target cost'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "migrate-from-ducklake" -d 'Migrate from an existing DuckLake catalog into RockLake'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "export-catalog" -d 'Export all 28+ DuckLake catalog tables to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "diagnose" -d 'Structured catalog health diagnostic report'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "sweep-orphans" -d 'Identify (and optionally delete) orphan Parquet files'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and not __fish_seen_subcommand_from serve doctor status support capacity catalog catalogs registry debug config completions backup restore gc excise checkpoint export import pg-migrate rebuild inspect verify repair warmup migrate corpus tune migrate-from-ducklake export-catalog diagnose sweep-orphans help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from support" -f -a "bundle" -d 'Write a redacted diagnostic bundle to a new directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from capacity" -f -a "report" -d 'Report catalog facts against a selected evidence envelope'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "backup" -d 'Create and inspect portable catalog backups'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "restore" -d 'Plan or apply a backup restore'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "gc" -d 'Visibility GC — advance the retain-from watermark'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "excise" -d 'Physical excision of catalog facts before a snapshot'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "checkpoint" -d 'Manage catalog checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "export" -d 'Export catalog to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "import" -d 'Import catalog from NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "export-catalog" -d 'Export all 28+ DuckLake catalog tables to NDJSON'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "migrate" -d 'Migrate catalog to the current format version'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "verify" -d 'Verify catalog integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "repair" -d 'Repair catalog issues'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "jobs" -d 'Inspect and control durable administrative jobs'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "maintenance" -d 'Manage durable maintenance policies and due work'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "recovery" -d 'Emit a machine-readable recovery-drill report'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalog" -f -a "backup-set" -d 'Create, inspect, and restore managed backup sets'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "validate" -d 'Validate the static route configuration'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "list" -d 'List configured catalogs without opening them'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "status" -d 'Show configured catalogs and current open-handle state'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "create" -d 'Create a new lazy catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "register" -d 'Register an existing catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "promote" -d 'Assign a catalog to a registered node after a generation check'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "activate" -d 'Mark an acquiring assignment write-ready after epoch acquisition'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "rename" -d 'Rename a catalog route alias'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "set-mode" -d 'Change a catalog route mode'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "disable" -d 'Disable a catalog route without deleting bytes'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "enable" -d 'Re-enable a disabled catalog route'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from catalogs" -f -a "remove" -d 'Detach a route and retain a tombstone; never deletes bytes'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "init" -d 'Initialize an empty registry'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "status" -d 'Show registry generation and lifecycle counts'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "backup" -d 'Back up registry state and audit entries to a local directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "restore" -d 'Restore a backup into an uninitialized registry location'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "verify" -d 'Verify registry state, indexes, prefixes, and audit sequence'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "migrate-static" -d 'Import the v0.54 static route table from the selected config file'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "register-node" -d 'Register a node with an expiring lease'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from registry" -f -a "renew-node" -d 'Renew a node lease'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "diagnose" -d 'Structured catalog health diagnostic report'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "inspect" -d 'Inspect catalog state (snapshot, API costs, cache utilisation)'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "corpus" -d 'Wire-corpus operations (diff and validate)'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "rebuild" -d 'Rebuild catalog by scanning Parquet files in object storage'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "sweep-orphans" -d 'Identify (and optionally delete) orphan Parquet files'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "pg-migrate" -d 'Convert NDJSON export to PostgreSQL INSERT statements'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "tune" -d 'Output recommended settings for a target cost'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "warmup" -d 'Warm up the block cache before serving'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from debug" -f -a "migrate-from-ducklake" -d 'Migrate from an existing DuckLake catalog into RockLake'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from config" -f -a "check" -d 'Validate the selected configuration file'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from config" -f -a "example" -d 'Print a complete example configuration'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from backup" -f -a "create" -d 'Create a snapshot-consistent backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from backup" -f -a "inspect" -d 'Validate and inspect a backup directory'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from restore" -f -a "plan" -d 'Validate a backup and show its proposed target changes'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from restore" -f -a "apply" -d 'Validate and import a backup into an empty target catalog'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from gc" -f -a "plan" -d 'Show the GC plan without applying it'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from gc" -f -a "apply" -d 'Apply the GC plan (advance retain-from)'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from excise" -f -a "plan" -d 'Show the excision plan without deleting anything'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from excise" -f -a "apply" -d 'Apply the excision plan (physically delete old catalog facts)'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "create" -d 'Create a new catalog checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "list" -d 'List existing checkpoints'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "restore" -d 'Restore catalog to a saved checkpoint'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "pin" -d 'Pin a snapshot under a durable name'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "unpin" -d 'Remove a named snapshot pin'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "pins" -d 'List named snapshot pins'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from inspect" -f -a "snapshot" -d 'Show the current snapshot metadata'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from inspect" -f -a "api-costs" -d 'Show per-operation API cost estimates'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from inspect" -f -a "cache-utilization" -d 'Show block-cache utilisation statistics'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from verify" -f -a "catalog" -d 'Verify catalog key-value integrity'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from verify" -f -a "data-files" -d 'Verify that all registered data files are accessible'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from corpus" -f -a "diff" -d 'Diff two wire-corpus files'
complete -c rocklake -n "__fish_rocklake_using_subcommand help; and __fish_seen_subcommand_from corpus" -f -a "validate" -d 'Validate a wire-corpus against the server'
