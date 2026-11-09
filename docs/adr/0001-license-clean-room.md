# ADR-0001: MIT license, clean-room from format descriptions
Status: Accepted
Date: 2026-11-08
Owner: maintainer

## Context
Reference projects have different licenses: EditorTools MIT, AOParser MIT, SarnautCore tools Apache-2.0, SarnautCore client AGPL-3.0, AODataViewer none.
## Decision
The project is MIT. Code is written from our own format notes. Code from MIT/Apache projects may be ported with a NOTICE entry. AGPL and unlicensed code is never copied.
## Alternatives considered
GPL/AGPL for the whole project (would allow reuse of the client code, but blocks embedding in other tools).
## Consequences
Format notes must be independent; reference projects are read for facts only.
## Security, license and maintenance impact
Needs the NOTICE file and a review of each port.
## Revisit when
A reference project changes its license.
