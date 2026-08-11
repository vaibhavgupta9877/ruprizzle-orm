# Security policy

## Supported versions

ruprizzle is pre-1.0. Only the most recent `0.x` release receives security
fixes.

| Version | Supported |
|---|---|
| 0.1.x   | ✅ |
| < 0.1   | ❌ |

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
