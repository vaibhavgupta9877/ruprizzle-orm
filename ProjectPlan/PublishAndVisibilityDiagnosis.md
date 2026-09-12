# Why nothing is published, and why nobody can find "ruprizzle"

**Investigated:** 2026-09-12
**Branch:** `docs/seo-and-release-fixes`, cut from `origin/main` at `37b3ef4`
**Companion documents:** [`ProductionReadinessSol.md`](ProductionReadinessSol.md),
[`ProductionReadinessSolPlan.md`](ProductionReadinessSolPlan.md)

Two questions were asked: why did the latest version not publish, and why does the
brand not appear on the internet. They have separate causes, and the second is far
worse than it looks.

## 1. Nothing has ever been published beyond a release candidate

Verified live against the crates.io API on 2026-09-12, reproducible with
`scripts/check-release-state.sh`:

| Crate | Latest on crates.io |
|---|---|
| `ruprizzle` | `1.0.0-rc.1` |
| `ruprizzle-core` | `1.0.0-rc.1` |
| `ruprizzle-parser` | `1.0.0-rc.1` |
| `ruprizzle-dialect` | `1.0.0-rc.1` |
| `ruprizzle-macros` | `1.0.0-rc.1` |
| `ruprizzle-check` | `1.0.0-rc.1` |
| `ruprizzle-lsp` | `1.0.0-rc.1` |
| `ruprizzle-migrate` | `1.0.0-rc.1` |
| `ruprizzle-codegen` | `1.0.0-rc.1` |
| `ruprizzle-cli` | `1.0.0-rc.1` |
| `ruprizzle-turso` | **never published** |
| `ruprizzle-d1` | **never published** |

The `v1.0.0`, `v1.0.1` and `v1.5.0` tags all exist in git with no corresponding
package. The registry's own `updated_at` for every crate is 2026-08-21, the
`1.0.0-rc.1` upload date.

### The two `v1.5.0` runs

1. **Run 33616540575** (2026-09-02 09:57) failed in `cargo test --workspace`, at
   `crates/runtime/tests/conditional_building.rs`, with
   `SqliteError { code: 14, message: "unable to open database file" }` across five
   tests. Root cause: the test helper dropped its `TempDir` before the pool, so on
   Linux the database file was unlinked while connections were still being opened.
   The in-repo `release_33616540575.log` records it. **Already fixed** in `0e4d96b`
   by the `PoolWithDir` helper — this failure is history, not a current blocker.
2. **Run 33633504583**, re-triggered after that fix, failed earlier still, in
   `cargo fmt --all --check`. No `cargo publish` step ran. The same run rendered
   `CARGO_REGISTRY_TOKEN` as empty, so it could not have uploaded anything even
   with a green gate.

So the publish failure is not one bug. It is a pipeline in which the gate is red
and the credential is absent, and neither condition is detected until twenty
minutes into a run.

### What this branch changes

- **`.github/workflows/release.yml`** now runs a credential preflight *before* the
  gate. An empty `CARGO_REGISTRY_TOKEN` fails the job in seconds with an
  actionable message, and the token value is never echoed — only its length.
- **`.github/workflows/release.yml`** now verifies publication after it claims to
  have published, by asking crates.io what it actually serves. A run can no longer
  go green while the registry still serves the previous version.
- **`scripts/check-release-state.sh`** makes the registry the single authority on
  what is released, for CI and for humans writing release notes.
- **`RELEASES.md`** gains the credential check and the post-publish verification as
  numbered runbook steps.

### What is still blocking, and needs a Rust toolchain

This machine has no working Rust toolchain — `~/.cargo` is absent, `~/.rustup` is
empty, and `cargo`/`rustc` are not on `PATH` — so none of the cargo gates could be
run or verified here. The following remain open and are specified in
`ProductionReadinessSolPlan.md`:

- **PR-01** `cargo fmt --all --check` fails in `crates/runtime/tests/insert_validation.rs`
  and `crates/testkit/src/lib.rs`; the trybuild `.stderr` fixture and a codegen
  snapshot are stale. This is the immediate cause of the second failed run.
- **PR-02** `cargo-semver-checks` reports major-version-level breaks in `core`,
  `dialect` and `check`. A prerelease-to-stable waiver may be legitimate but has
  not been reviewed or encoded.
- **PR-03** `stream_unbuffered` buffers on every backend except native
  `tokio-postgres`, contradicting its contract.

The `v1.5.0` tag should not be moved. It is the immutable record of a source tree
whose release failed; the next attempt should take a new version.

## 2. The documentation site does not exist

This is the answer to "I cannot see 'ruprizzle' on the internet", and it is
absolute rather than a matter of ranking.

Every URL on the documented site returns **404**, including the site root:

```
https://vaibhavgupta9877.github.io/ruprizzle-orm/              404
https://vaibhavgupta9877.github.io/ruprizzle-orm/quickstart.html   404
https://vaibhavgupta9877.github.io/ruprizzle-orm/sitemap.xml       404
https://vaibhavgupta9877.github.io/ruprizzle-orm/robots.txt        404
https://vaibhavgupta9877.github.io/                            404
```

The account root 404s too, which means **GitHub Pages has never been provisioned**.
`.github/workflows/pages.yml` is correct and uses `actions/deploy-pages`, but that
action requires Pages to be enabled with "GitHub Actions" as the source in the
repository settings. Until that is switched on, there is no site, nothing to crawl,
nothing to index and nothing to cite.

**This is the single highest-impact action, it is a repository setting, and it
cannot be done from a commit.** Settings → Pages → Build and deployment → Source →
GitHub Actions, then re-run the Docs workflow.

Everything below was broken *as well*, and would have kept the site nearly
invisible even once Pages was turned on.

### 2.1 Every page canonicalised to the site root

`theme/head.hbs` emitted `<link rel="canonical" href="{{ base_url }}">`. mdBook
renders that partial with an identical context on every page, so every chapter
told search engines "the real version of this page is the homepage" — an
instruction to drop the entire book from the index bar one page. `og:url` had the
same defect, so every social share would have resolved to the homepage.

Fixed by removing the broken tag from the template and injecting a correct
per-page canonical and `og:url` after the build, in
`scripts/postprocess-docs.sh`.

### 2.2 The sitemap advertised eleven URLs, seven of which could never exist

`sitemap.xml` was hand-maintained and listed `quickstart.html`,
`schema-reference.html`, `query-guide.html`, `relations-guide.html`,
`migrations-guide.html`, `dialect-notes.html` and `known-limitations.html`.

Six of those pointed at three-line stub files — `docs/query-guide.md` and friends,
each saying only "the canonical document has moved to `QueryGuide.md`". Those stubs
were not in `SUMMARY.md`, and mdBook only builds what `SUMMARY.md` lists, so no
HTML was ever generated for them. The sitemap was therefore directing crawlers at
pages that would 404 even on a working site, while the real content —
`QueryGuide.html`, `SchemaReference.html`, `RelationsGuide.html` — was not listed
at all.

Fixed by deleting the seven dead stubs (nothing outside the planning documents
linked to them) and generating `sitemap.xml` from the pages that actually build.

### 2.3 Published pages linked to chapters that were never built

`SUMMARY.md` omitted several documents that published pages link to, so those
links 404 on the live site:

| Missing chapter | Linked from |
|---|---|
| `Stability.md` | `README.md`, `faq.md`, `announcement.md` |
| `SoakReport.md` | `README.md` |
| `FeaturesMasterComparison.md` | `performance.md`, `BenchmarkResults.md` |
| 12 × `adr/ADR-0NN-*.md` | `adr/index.md` |

`FeaturesMasterComparison.md` is the costliest omission: 248 lines comparing
ruprizzle against Diesel, SeaORM, SQLx and Prisma. Comparison content is the single
most-cited format in AI answers, and it was not on the site. `ADR-012` was also
missing from the ADR index itself.

Fixed: `SUMMARY.md` now publishes all of them, grouped into sections, and
`adr/index.md` lists ADR-012.

### 2.4 The install instructions do not work

`README.md`, `docs/quickstart.md`, `docs/README.md` and `crates/cli/README.md` all
told readers to run `cargo install ruprizzle-cli` and `cargo add ruprizzle`. Cargo
ignores prereleases unless one is named explicitly, and `1.0.0-rc.1` is a
prerelease, so both commands fail with "could not find ... in registry". Anyone who
did find the project could not install it.

Fixed: every install snippet now pins `1.0.0-rc.1`, explains why the version is
required, and offers a git dependency for the v1.5 line.

### 2.5 Four documents disagreed about what was released

`CHANGELOG.md` said `1.5.0` was published; `README.md` and `docs/README.md` said
`1.0.1` was; `SECURITY.md` said `1.0.0` was; the registry served `1.0.0-rc.1`. A
visitor could not learn the truth from any of them, and a security reporter was
being pointed at a version that does not exist.

Fixed across all of them, with `scripts/check-release-state.sh` as the mechanism
that keeps them honest.

### 2.6 Missing AI-answer-engine surface

- **No `llms.txt`.** Added, describing what ruprizzle is, how it differs from
  Diesel, SeaORM, SQLx and Prisma Client Rust, the real release status, and a
  linked map of the documentation.
- **No FAQ structured data.** `scripts/postprocess-docs.sh` now generates
  `FAQPage` JSON-LD from the FAQ's own headings and the prose beneath them, so the
  markup cannot drift from the visible copy.
- **`robots.txt` did not name the AI crawlers.** The wildcard already allowed them,
  but GPTBot, ClaudeBot, PerplexityBot, Google-Extended, OAI-SearchBot and the
  others are now listed explicitly so a future tightening cannot silently remove
  the project from AI answers.
- **Stale structured data.** The `SoftwareApplication` block advertised
  `softwareVersion: 0.1.1-beta.1`. Replaced with a `SoftwareSourceCode` entity
  carrying no hand-written version to drift.
- **`print.html` was an unmanaged duplicate.** mdBook concatenates the whole book
  into one page; it is now `noindex` and excluded from the sitemap, and
  deliberately *not* disallowed in `robots.txt`, because a blocked page's noindex
  can never be read.
- **The FAQ answered the wrong questions.** Added "Which version should I
  install?", "What databases does ruprizzle support?" and "Is ruprizzle free?",
  and rewrote "Is it production-ready?" to lead with a direct answer.

## 3. What to do next, in order

1. **Enable GitHub Pages** (Settings → Pages → Source → GitHub Actions) and re-run
   the Docs workflow. Nothing else in section 2 matters until this is done.
2. Verify the deployed site: root 200, `sitemap.xml` 200, `llms.txt` 200, and one
   canonical tag per page (the Docs workflow now asserts the last of these).
3. Submit the site to Google Search Console and Bing Webmaster Tools. Neither can
   be done from a commit and neither will happen on its own.
4. Restore the Rust toolchain, then close PR-01 through PR-03 and re-run the gate.
5. Confirm `CARGO_REGISTRY_TOKEN` is present, then rehearse with a
   `workflow_dispatch` run with `publish` false.
6. Cut `1.5.1` — not `1.5.0` — and publish. Verify with
   `scripts/check-release-state.sh --expect 1.5.1` before updating any document to
   say a release exists.
7. Only then pursue off-site presence. A Rust crate's discovery surfaces are
   crates.io, docs.rs, /r/rust and This Week in Rust, and all four are worthless
   while the registry serves a month-old release candidate and the docs 404.
