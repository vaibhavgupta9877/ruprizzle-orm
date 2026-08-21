# Security policy

## Supported versions

ruprizzle is pre-1.0. Only the most recent published release receives security
fixes. Once `1.0.0` ships, the `1.x` line becomes the supported line.

| Version | Supported |
|---|---|
| `1.0.0-rc.x` (once published) | ✅ |
| `0.4.x` (current latest on crates.io) | ✅ |
| < `0.4` | ❌ |

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
