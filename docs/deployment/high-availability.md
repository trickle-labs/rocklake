# High availability

RockLake uses one writer per catalog and supports read-only readers against the
same object-store catalog. Failover is a process-supervision and storage
ownership concern; no Docker, Kubernetes, or automatic failover package is
published in v0.49.0.

Start a writer with `rocklake serve --catalog ... --mode writer` and start a
reader with the same catalog and `--mode reader`. Ensure only one process owns
writer mode at a time.
