# Deployment

SlateDuck is designed to be deployed simply — it is a single static binary with no runtime dependencies beyond network access to your object storage provider. This section covers the various deployment options, from a simple binary on a VM to production Kubernetes clusters, serverless functions, and multi-region setups.

## Deployment Strategies

- **[Binary](binary.md)** — Running the SlateDuck binary directly on a VM or bare metal
- **[Docker](docker.md)** — Container images and Docker Compose setups
- **[Kubernetes](kubernetes.md)** — Helm charts, StatefulSets, and production configuration
- **[Lambda / Serverless](lambda.md)** — Running SlateDuck as a serverless function

## Configuration & Security

- **[Configuration](configuration.md)** — Environment variables, flags, and configuration files
- **[TLS](tls.md)** — Encrypting connections with TLS certificates
- **[Networking](networking.md)** — Network topology, firewall rules, and service discovery

## Advanced

- **[High Availability](high-availability.md)** — Achieving uptime SLAs with failover
- **[Multi-Region](multi-region.md)** — Cross-region read replicas and disaster recovery
- **[Fly.io](fly-io.md)** — Deploying on Fly.io with global edge routing
