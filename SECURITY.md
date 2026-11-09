# Security policy

Only the latest release is supported. Before 1.0 the development branch is the only supported version.

## Reporting a vulnerability

Do not open a public issue. Use GitHub private reporting (Security, Report a vulnerability). Include the version, a minimal way to reproduce and, for a parser bug, a short hex dump (do not attach game files).

This is a one-person project; targets are acknowledgement in 7 days and a fix or mitigation for a confirmed high-severity issue in 30 days.

## Scope

In scope: `adv-core` parsers and extraction (out-of-bounds, unbounded allocation, decompression bombs, path traversal on extract, panics), the `adv` CLI, the desktop app (IPC, CSP, file access), the build and release workflows.

Out of scope: vulnerabilities in third-party packages already public (report upstream; Dependabot and the audit job follow them), anything needing a compromised machine, game client or server vulnerabilities.

## Safe harbour

Good-faith research that follows this policy will not be pursued.
