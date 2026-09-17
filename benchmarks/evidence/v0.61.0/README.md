# v0.61.0 capacity evidence

The v0.61.0 capacity contract stores historical LocalFS and MinIO profile
inputs. The raw v0.52 scale reports are absent, so these profiles are guidance,
not certified envelopes or universal performance claims. Request rates, byte
rates, and pricing inputs are supplied separately so a report can be reproduced
with the exact workload and dated provider prices.

The release gate checks the capacity report schema, profile ceilings, pricing
validation, and the existing benchmark regression thresholds. New performance
claims require a fresh evidence run with its machine, dependency, correctness,
latency, RSS, request, and byte measurements committed here.
