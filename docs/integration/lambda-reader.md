# Lambda and serverless readers

Running RockLake inside AWS Lambda or another serverless function is not a
supported v0.51.2 integration. The supported runtime is the long-running
`rocklake` binary, with optional read-only reader processes sharing the same
object-store catalog.

For embedded applications, use the stable C ABI or language bindings described
in [Client Library](client-library.md). A future serverless integration needs
its own maintained build, lifecycle, credential, and end-to-end test path
before it can be documented as supported.
