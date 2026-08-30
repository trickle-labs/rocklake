# Inspect

`rocklake inspect` reports current catalog state. It is read-only and has
three subcommands:

```bash
rocklake inspect snapshot --catalog ./catalog
rocklake inspect api-costs --catalog ./catalog
rocklake inspect cache-utilization --catalog ./catalog
```

Use `--catalog` with a local path or supported object-store URL. The command
does not expose raw SlateDB keys or arbitrary snapshot formatting; use
`rocklake export-catalog --at-snapshot <id>` for an exact historical catalog
export.

For integrity checks, use the separate commands:

```bash
rocklake verify catalog --catalog ./catalog
rocklake verify data-files --catalog ./catalog
rocklake diagnose --catalog ./catalog
```

Run `rocklake inspect --help` or a subcommand's `--help` for the installed
binary's exact options.
