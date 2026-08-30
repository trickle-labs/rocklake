# Verify and Repair

Verification is read-only. Repair is dry-run by default and requires
`--apply` to write changes.

## Verify

Verify catalog key-value integrity:

```bash
rocklake verify catalog --catalog ./catalog
```

Verify that registered data files are accessible:

```bash
rocklake verify data-files --catalog ./catalog
```

Both commands return a non-zero exit status when verification fails. Run them
before and after a migration, restore, or storage change.

## Repair

Preview repairs without changing the catalog:

```bash
rocklake repair --catalog ./catalog --dry-run
```

Apply the reported repairs only after reviewing the plan:

```bash
rocklake repair --catalog ./catalog --apply
```

Keep a logical export before applying repairs:

```bash
rocklake export-catalog --catalog ./catalog --out before-repair.ndjson
```

Repair does not restore missing Parquet data files. Use
`verify data-files` to identify data-plane failures and restore or excise
those files through the appropriate operational process.
