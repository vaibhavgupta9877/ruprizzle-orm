# Security policy

## Supported versions

`1.5.1` is the current stable release on crates.io, published 2026-09-13 for all
twelve crates. The `v1.0.0`, `v1.0.1` and `v1.5.0` git tags never produced a
package — their publish runs did not upload anything. Confirm the current
registry state with `scripts/check-release-state.sh`.

Security fixes are delivered as new published releases on the `1.x` line.
Fixes are developed against `main`.

| Version | Supported |
|---|---|
| `1.5.1` (current on crates.io) | ✅ |
| `1.0.0-rc.1` (previous published version) | ❌ — upgrade to `1.5.1` |
| `main` / source builds | ✅ — best effort, no published artifact |
| `0.x` (alpha and beta lines) | ❌ |

If you are running a source build of the `1.5.0` tag, say so in your report and
move to the published `1.5.1`: that tag records a failed release.

## Known accepted dependency risk

`sqlx-mysql` depends on `rsa 0.9.x`, which is affected by
[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071). No patched
release is available; the exception is recorded in `deny.toml`. It is reachable
only through MySQL's `caching_sha2_password` RSA key exchange — connect over TLS
or a unix socket to avoid that path. Postgres and SQLite are unaffected.

## Reporting a vulnerability

**Do not open a public issue.**

Report privately through GitHub's
[private vulnerability reporting](https://github.com/vaibhavgupta9877/ruprizzle-orm/security/advisories/new),
or email the author listed in `Cargo.toml` at `vaibhavgupta9877@gmail.com`.

Expect an acknowledgement within 72 hours and an assessment within seven days.
If a fix is warranted we will agree a disclosure date with you, defaulting to
90 days or the release of the fix, whichever is sooner.

## Scope

In scope:

- SQL injection through any public API, including identifier handling and the
  `Value` binding path.
- Migration application that corrupts, loses, or silently alters data.
- Credential leakage through errors, logs, or generated code.
- Generated code that introduces a vulnerability into a consuming project.

Out of scope:

- Vulnerabilities in `sqlx` or other dependencies — report those upstream,
  though we appreciate a heads-up.
- Denial of service through deliberately pathological schemas fed to the CLI.
- Anything requiring an attacker who already controls the schema file, since
  that file is trusted input equivalent to source code.
