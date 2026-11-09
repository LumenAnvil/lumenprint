# Security policy

Lumenprint is a scaffolding CLI: it writes new project files from embedded
templates. It handles no keys and makes no network calls. The main security
concern is that a generated project, or the generator itself, could write
outside the intended directory or embed something unsafe.

## Reporting a vulnerability

Please report security issues privately through GitHub Security Advisories
rather than as a public issue:

<https://github.com/LumenAnvil/lumenprint/security/advisories/new>

Things worth reporting:

- a template path that escapes the destination directory (the generator rejects
  absolute paths and `..`; a bypass would be a vulnerability),
- a template that generates a project with an unsafe or misleading default,
- a supply-chain concern in the dependencies.

You can expect an acknowledgement, and we will keep you updated as we work on a
fix.
