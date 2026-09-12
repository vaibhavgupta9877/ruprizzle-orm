# Security policy

## Supported versions

`1.0.0-rc.1` is the only version published on crates.io. No stable release has
been published: `v1.0.1` and `v1.5.0` are git tags whose publish runs did not
upload anything. Confirm the current registry state with
`scripts/check-release-state.sh`.

Because `1.0.0-rc.1` is the only thing anyone can install, it is the version that
receives security fixes, delivered as a new published release. Fixes are developed
against `main`, which is ahead of that package.

| Version | Supported |
|---|---|
| `1.0.0-rc.1` (the only package on crates.io) | ✅ |
| `main` / source builds | ✅ — best effort, no published artifact |
| `0.x` (alpha and beta lines) | ❌ |

If you are running a source build of the `1.5.0` line, say so in your report: it
contains code that has never been through a completed release gate.

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
