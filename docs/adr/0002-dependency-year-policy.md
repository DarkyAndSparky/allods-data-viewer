# ADR-0002: Direct dependencies must be releases of the current year
Status: Accepted
Date: 2026-11-08
Owner: maintainer

## Context
Abandoned crates carry unfixed bugs and advisories.
## Decision
Direct dependencies and tooling are releases of the current calendar year, checked against real registry dates. Transitive ones are the newest compatible version.
## Alternatives considered
No policy; "latest stable" only.
## Consequences
lewton, walkdir, hound, csv, ogg and tracing were rejected on 2026-11-09 (see the roadmap document, tab Dependencies). Some audio and CSV code is written in-house.
## Security, license and maintenance impact
Smaller dependency surface; more own code to test.
## Revisit when
A rejected crate publishes a release this year.
