# Versioning policy

## One number for everything

Every crate in the workspace and the VS Code extension in `editor/vscode` carry the
**same version number**, moved in lockstep. There are no independently-versioned
components.

The reason is that they are not independently useful. `ruprizzle-cli` embeds the
parser, the codegen and the migration engine; the extension talks to the LSP binary
those crates build. A user who has `ruprizzle 1.5.0` and extension `1.2.0` cannot tell
from the numbers whether they match, and the only honest answer would be a
compatibility table nobody maintains. One number removes the question.

The cost is accepted deliberately: a crate with no changes in a release still gets a
new version. That is cheaper than the table.

## Where the number lives

| Place | How it is set |
|---|---|
| `[workspace.package] version` in the root `Cargo.toml` | The source of truth. |
| Every crate manifest | `version.workspace = true`. Never write a literal. |
| `[workspace.dependencies]` internal pins in the root `Cargo.toml` | Literal, and must equal the workspace version. `cargo publish` needs a version alongside `path`. |
| `editor/vscode/package.json` | Literal. Must equal the workspace version. |

`editor/vscode` drifted to `1.2.0` while every crate sat at `1.0.0`, because nothing
tied the two together and no policy said which was right. That is what this document
exists to prevent
(`ProjectPlan/v2/ProductionReadinessV1_5.md` §5.2).

## Semver

The public API is covered by semantic versioning from the first published stable
release — `1.5.1` — onward. What counts as public, and what a breaking change is, is
defined in [Stability](Stability.md).

Crates marked `publish = false` are outside the semver promise: `ruprizzle-testkit`
and `xtask`. They still carry the workspace version, because they are built from the
same tree.

## Cutting a release

1. Bump `[workspace.package] version` and the internal `[workspace.dependencies]` pins
   in the root `Cargo.toml`.
2. Bump `editor/vscode/package.json` to match.
3. Add the version heading to `CHANGELOG.md`, dated.
4. `cargo xtask release-check --tag vX.Y.Z` — this verifies the tag, the workspace
   version and the `CHANGELOG.md` heading agree, and fails if they do not.
5. `cargo xtask harden` — among other audits this checks that every publishable crate
   is in `PUBLISH_ORDER` and that `.github/workflows/release.yml` publishes exactly
   that list, in that order.
6. Tag and push. `release.yml` runs the full gate and publishes.

Steps 4 and 5 are mechanical gates, not conventions. Neither can be satisfied by
remembering to do something.
