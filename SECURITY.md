# Security policy

## Supported versions

Only the latest v0.49.x release receives security fixes. Older releases are
unsupported. Upgrade before reporting a problem when the issue is fixed in a
newer release.

## Report a vulnerability

Do not open a public issue for a suspected vulnerability. Send a report to
`security@trickle-labs.com` with:

- the affected RockLake version and deployment type,
- the affected component or dependency,
- the steps or code needed to reproduce the issue,
- the impact, and
- any proof of concept or proposed mitigation.

If email is not available, open a private security advisory in the repository.
Remove credentials and customer data from the report. We may ask for a
minimal reproduction or a safe way to test the report.

## Response and disclosure

We acknowledge reports within five business days. We assess severity, keep the
reporter informed while we work, and publish a fix or mitigation when one is
available. The timeline depends on impact and on the affected upstream
dependency.

We coordinate the disclosure date with the reporter. We publish a security
advisory and credit the reporter unless the reporter asks to remain
anonymous. We do not disclose exploit details before users have a reasonable
upgrade path.

## Dependency policy

RockLake runs `cargo-deny` advisory checks in CI. Every ignored advisory in
`deny.toml` has an owner-facing reason and an expiry date. Maintainers review
ignored advisories before expiry, upgrade or remove the affected dependency
when possible, and record a new dated exception only when the risk remains
understood and no compatible fix exists.

Transitive advisories remain tracked until an upstream release removes them.
Security fixes take priority over feature work. A report about a dependency
may also be sent to the upstream maintainer when the vulnerable code is not
controlled by RockLake.
