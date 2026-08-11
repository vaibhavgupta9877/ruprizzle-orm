# ruprizzle-orm Marketing Plan

*Prepared for the ruprizzle open-source project.*  
*Date: 2026-08-11*  
*Status: 0.1.0-alpha.3 published on crates.io; 90-day priorities in execution.*

---

## 1. Executive Summary

**Three big bets for the next 12 months:**

1. **Own the "schema-first + SQL-transparent" narrative.** Rust now has dozens of new ORMs, but most copy either Prisma's magic or Drizzle's TypeScript ergonomics without committing to *both* schema-first codegen and visible SQL. ruprizzle is positioned to be the pure-Rust, no-sidecar answer.
2. **Win the founder-and-prototype market first, then climb to production.** The first 12 months are about becoming the default ORM a Rust developer reaches for when they are tired of `sqlx` boilerplate or SeaORM's drift. Production adoption will follow a stable 0.2 and a small set of public case studies.
3. **Move fast in public.** Because `ferriorm`, `prax-orm`, `vitrail`, `saola`, and `kosame` are all early and largely single-contributor, the winner in this wave will be the project that ships consistently, documents honestly, and builds the most visible community around schema-first Rust.

**90-day priorities:**
- Sharpen positioning and homepage copy (Weeks 1–2).
- Ship three concrete starter examples and a comparison page (Weeks 3–4).
- Publish 4–6 technical essays and a "State of Rust ORMs" teardown (Weeks 5–8).
- Launch `v0.2` on Product Hunt, Hacker News, and r/rust (Weeks 9–12).

**12-month outcome:**
- Stable `v0.2` release with MySQL/MariaDB support.
- 1,000+ GitHub stars and 50+ active early-adopter projects.
- A recognized voice in the Rust database conversation (This Week in Rust, conference talk, 3+ case studies).
- A clear path to optional commercial support, migrations-as-a-service, or IDE tooling in Year 2.

---

## 2. Strategic Frame

### Category claim

**"The schema-first ORM for Rust that keeps SQL visible."**

This is a deliberate middle ground:
- Not a query builder (too low-level, too much boilerplate).
- Not a black-box ORM (too magical, too hard to debug).
- Not a sidecar-dependent client (too heavy, too many moving parts).

### ICP distilled

| Segment | Why they care | How they find us |
|---------|--------------|------------------|
| **TypeScript converts** (Drizzle/Prisma users moving to Rust) | Want a familiar schema-first workflow without the Node sidecar. | Search "Rust ORM like Prisma", HN/Reddit comparisons, framework docs. |
| **Rust web teams** (Axum/Actix/Tower) | Need async, type-safe CRUD with migration tooling. | This Week in Rust, GitHub search, word of mouth. |
| **Indie hackers / solo founders** | Need to ship a Rust MVP fast; want schema as source of truth. | Product Hunt, Rust starter templates, YouTube tutorials. |
| **Tooling & CLI builders** | Want SQLite + Postgres with one API and minimal compile cost. | lib.rs, docs.rs, niche database content. |

### Business-model logic

Open-source MIT/Apache-2.0 now. Revenue is not the Year 1 goal. In Year 2, commercialization options include:
- **Professional support / consulting** for teams migrating to ruprizzle.
- **Managed migration review / CI gatekeeping** (a SaaS layer on top of `ruprizzle migrate`).
- **IDE / LSP extensions** or schema-inspection tooling.

The open-core model is not appropriate yet because the category is too young and the project is too small. Building trust and usage come first.

### Brand voice non-negotiables

1. **Honest about alpha.** Never claim "production-ready" before 0.2. Publish limitations.
2. **SQL-first.** Show the query. `.to_sql()` is a feature, not a footnote.
3. **No magic.** Every abstraction is explained with a clear mapping to SQL or Rust types.
4. **Respect the user.** Assume they know SQL and Rust; don't talk down, don't oversell.

---

## 3. Current State

### Project snapshot (scored 0–5)

| Rubric area | Score | Notes |
|-------------|-------|-------|
| **Positioning clarity** | 4 | Strong differentiation on schema-first + SQL transparency; README comparison table is good. |
| **Product completeness** | 3 | P0–P8 done; Postgres/SQLite; MySQL, db pull, LSP, offline compile checks still pending. |
| **Documentation** | 3 | README is strong; docs site and examples need to be expanded and surfaced. |
| **Community / distribution** | 1 | 0 GitHub stars, 0 public discussion, 1 contributor. |
| **Website / landing page** | 2 | GitHub Pages exists (vaibhavgupta9877.github.io/ruprizzle-orm); needs homepage polish. |
| **Content engine** | 1 | No blog, no newsletter, no social presence. |
| **Analytics** | 1 | No visible tracking on docs, crates, or site. |
| **SEO / discoverability** | 2 | Good crate names; poor non-branded search presence. |
| **Activation / onboarding** | 3 | `init → generate → migrate dev` workflow is clear; needs starter templates and a web tutorial. |
| **Retention / support** | 2 | GitHub issues enabled; no Discord/forum; changelog present. |
| **Partnerships** | 0 | No framework integrations, no co-marketing. |
| **Revenue / monetization** | 0 | None; not the focus yet. |
| **Team / execution bandwidth** | 2 | Solo founder; needs to automate or delegate content and community. |
| **Milestones / roadmap** | 4 | MasterPlan and README roadmap are explicit and public. |
| **Social proof** | 0 | No users, no case studies, no testimonials. |
| **Security / trust** | 3 | MIT/Apache-2.0, honest limitations, SECURITY.md, CI present. |
| **Funding runway** | N/A | Bootstrapped; marketing is founder time + free channels. |

### Key blockers

1. **No social proof.** The project is brand new (alpha.3 published today). Everything must be built from zero.
2. **Category noise.** At least 15+ new Rust ORMs launched since 2024; several mimic the same Prisma/Drizzle pitch.
3. **Single-contributor risk.** A one-person project is harder to trust for teams.
4. **Alpha limitations.** Only Postgres and SQLite; no LSP; no compile-time offline checks yet.

---

## 4. Acquisition — How strangers become aware

### Channels: now, next, skip

| Channel | Status | 90-day move | 12-month move |
|---------|--------|-------------|---------------|
| **Hacker News** | Now | "Show HN" for v0.2; thoughtful comments on ORM threads. | Quarterly posts and follow-ups tied to releases. |
| **Reddit r/rust** | Now | Weekly value-add comments; one launch post per major release. | AMA / "Ask Me Anything" once there are users and lessons. |
| **This Week in Rust (TWIR)** | Now | Submit project announcement and each major blog post. | Regular "Project Update" mentions. |
| **Rust users forum** | Now | Announce in "Showcase" and answer questions. | Long-form technical threads. |
| **GitHub Topics / lib.rs / libs.tech** | Now | Ensure correct keywords; request listing. | Keep metadata updated as backends expand. |
| **Product Hunt** | Next | Pre-launch page for v0.2; launch day campaign. | Re-launch on major versions (0.3, 1.0). |
| **SEO / long-form blog** | Now | "State of Rust ORMs 2026" and comparison articles. | Programmatic comparison pages (e.g. `/vs-seaorm`, `/vs-sqlx`). |
| **YouTube / demos** | Next | 2 short Loom/YouTube demos of the workflow. | Monthly deep-dive videos. |
| **Twitter / X** | Now | Personal founder account; share milestones and snippets. | Build project account once there is momentum. |
| **Discord / community** | Now | Create a Discord or GitHub Discussions. | Migrate to Discord once >100 active members. |
| **Conference / meetup talks** | Next | CFP for RustConf / EuroRust / local meetups in Q2. | 2+ talks in Year 1. |
| **Paid ads** | Skip | No budget; organic only. | Re-evaluate after Seed funding or sponsor budget. |
| **Cold outreach / SDR** | Skip | Not applicable for open-source. | Enterprise support leads in Year 2. |

### Positioning by channel

- **Hacker News / Reddit:** Lead with the engineering trade-off — "We built the Rust ORM we wanted: schema-first like Prisma, SQL-transparent like Drizzle, no sidecar."
- **TWIR / Rust forum:** Lead with the project status and call for feedback — "P0–P8 complete; looking for early adopters and contributors."
- **SEO / blog:** Lead with problem-aware search — "Why I stopped using SeaORM for new Rust projects" or "Schema-first ORMs in Rust: a comparison."
- **Product Hunt:** Lead with the founder story and the 60-second demo — "A Prisma-style schema file that generates a type-safe Rust client."

---

## 5. Activation — How a new user experiences value

### Onboarding journey

1. **Landing / README** → sees comparison table and 60-second code snippet.
2. **Install** → `cargo install ruprizzle-cli` and `cargo add ruprizzle`.
3. **Scaffold** → `ruprizzle init --provider postgres` creates `schema.ruprizzle`, `.env`, `migrations/`.
4. **Model** → edit schema; run `ruprizzle generate`.
5. **Migrate** → `ruprizzle migrate dev --name init`.
6. **Query** → write the first `find_many` with a filter and call `.to_sql()`.
7. **Aha moment** → sees a compile error catch a type mismatch, or runs a nested `include` that is not N+1.

### Activation friction to remove

| Friction | Fix |
|----------|-----|
| No live playground / REPL | Add a `try-ruprizzle` repo with Docker Compose for one-command testing. |
| Schema syntax not familiar | Add a schema cheat-sheet and VS Code/TextMate grammar install instructions. |
| No LSP | Promote the current TextMate grammar; publish a short "editor setup" guide. |
| No MySQL | State clearly that MySQL is next; provide a public roadmap check. |
| No obvious starter template | Publish `ruprizzle-starter-axum` and `ruprizzle-starter-tauri-sqlite` repos. |

### First-visit CTA stack

- GitHub README: "Install the CLI and run the quickstart →"
- Docs homepage: "Try the 5-minute quickstart"
- Blog posts: "Star ruprizzle and get the starter template"
- Product Hunt: "Try the live example repo"

---

## 6. Retention — How users stay and deepen

### Lifecycle touchpoints

| Stage | Touchpoint | Owner | Goal |
|-------|------------|-------|------|
| Day 0 | README + quickstart + `to_sql()` demo | Docs | First successful query. |
| Day 1-3 | GitHub Discussions / Discord check-in | Community | Answer setup questions. |
| Week 1 | Newsletter #1: "What we shipped this week" | Founder | Keep project top of mind. |
| Month 1 | Migration guide from SeaORM / sqlx | Content | Reduce switching anxiety. |
| Ongoing | Changelog + releases + migration safety notes | Engineering | Build trust. |

### Churn risks and prevention

| Risk | Prevention |
|------|------------|
| Alpha limitation blocks real use | Publish honest capability matrix; offer `raw!` and `sqlx` interop as escape hatch. |
| Missing feature (MySQL, LSP, offline checks) | Public roadmap with target versions; invite contributions. |
| Ferriorm / Prax / Vitrail add the same feature first | Ship more transparently and faster; emphasize SQL visibility and bounded `include`. |
| Solo-maintainer concern | Add a `CONTRIBUTING.md` and a public governance plan; recruit one co-maintainer in 90 days. |

### Content retention loop

- **Changelog as marketing:** every release gets a tweet/Reddit comment/TWIR update.
- **Migration recipe series:** "Migrating from SeaORM to ruprizzle in 10 minutes."
- **Schema design patterns:** weekly short posts on real-world schema decisions.

---

## 7. Referral — How users bring more users

### Referral mechanics

1. **GitHub Stars as social proof** — make starring a low-friction CTA in docs and blog.
2. **Show-and-tell in Discussions** — users post their schema; founder highlights the best ones.
3. **Starter template program** — give users a `built-with-ruprizzle` badge for their repo.
4. **Contributor recognition** — "first PR" shoutouts on social and in changelog.
5. **Framework integrations** — co-marketing with Axum/Actix/Tower example repos.

### Ambassador / evangelist plan

- **Phase 1 (0–6 months):** Founder is the only evangelist. Focus on being helpful in public forums.
- **Phase 2 (6–12 months):** Identify 3–5 active users and invite them to write guest posts, give talks, or co-maintain a starter template.
- **Phase 3 (12+ months):** Formalize a "ruprizzle Champions" program with badges, early access, and swag.

---

## 8. Revenue — How the project monetizes

### Current stance

Year 1 is **zero-revenue, usage-first**. The goal is to become a standard before introducing commercial products.

### Future revenue options (2027+)

| Option | When it fits | Risk |
|--------|--------------|------|
| **Professional support** | Teams ask for migration help or production support. | Time-intensive; one-person bottleneck. |
| **Managed migration CI / SaaS** | Teams want `ruprizzle migrate` in CI with drift detection. | Requires infrastructure and trust. |
| **IDE / LSP tooling** | Schema DSL matures and an LSP becomes valuable. | Distribution through editors; maintenance burden. |
| **Training / workshops** | Conference demand and enterprise pilots. | Low scale. |

### Pricing principles

- The core ORM stays free and open-source forever.
- Commercial offerings are additive tooling or support, not feature gates.
- If support is offered, price it as a monthly retainer ($1,000–5,000/mo for early enterprise pilots).

---

## 9. 90-Day Roadmap

### Weeks 1–2: Unblock

- [ ] Finalize tagline and 30-second pitch.
- [ ] Rewrite README above-the-fold to lead with the problem and the `.to_sql()` hook.
- [ ] Add a "Comparison with new ORMs" section (ferriorm, prax, kosame, toasty, vitrail).
- [ ] Set up GitHub Discussions or Discord.
- [ ] Create 3 starter repos: `ruprizzle-starter-axum`, `ruprizzle-starter-tauri`, `ruprizzle-starter-cli`.
- [ ] Add a `try-ruprizzle` Docker Compose one-liner.
- [ ] Set up simple analytics (GitHub traffic, docs.rs, Plausible on homepage).

### Weeks 3–4: Foundation

- [ ] Publish comparison landing page: `ruprizzle-orm.dev/compare` (or GitHub Pages path).
- [ ] Publish "The State of Rust ORMs in 2026" blog post.
- [ ] Submit to lib.rs, libs.tech, Rust-LibHunt, GitHub Topics, and TWIR.
- [ ] Create a public `Roadmap.md` linked from README.
- [ ] Add VS Code / editor setup instructions and schema syntax highlighting.
- [ ] Start a monthly changelog newsletter.

### Weeks 5–8: Velocity

- [ ] Publish 2 technical deep-dives per week (8 posts total):
  - How `include` stays N+1-free.
  - How the `DbDialect` model keeps backends additive.
  - How `to_sql()` works on every builder.
  - Migrations: `dev` vs `deploy` safety.
  - Benchmarking query construction vs SeaORM/Diesel.
  - Schema-first vs entity-first in Rust.
  - `raw!` and `sqlx` interop.
  - Testing with `ruprizzle-testkit` across Postgres and SQLite.
- [ ] Record 2 demo videos and post to YouTube/Twitter.
- [ ] Comment helpfully on 20+ relevant GitHub issues/Reddit threads per week.
- [ ] Publish first case study / early-adopter interview (even if it's the founder's own project).
- [ ] Build Product Hunt pre-launch page for v0.2.

### Weeks 9–12: Compound

- [ ] Ship `v0.2` stable.
- [ ] Launch on Product Hunt, Hacker News, r/rust, TWIR, and Rust users forum.
- [ ] Host a live demo / AMA.
- [ ] Publish a "Migration from SeaORM / sqlx / Diesel" guide.
- [ ] Recruit first external contributor and update `CONTRIBUTING.md`.
- [ ] Measure: 250 GitHub stars, 5+ active community members, 3+ external issues/PRs.

---

## 10. 12-Month Outlook

| Quarter | Milestone | Capability unlock |
|---------|-----------|-------------------|
| **Q3 2026** | `v0.2` stable; MySQL/MariaDB support; 250 stars; first case studies. | Validation that the schema-first model works across three SQL dialects. |
| **Q4 2026** | 1,000 GitHub stars; 50 active projects; first conference talk / meetup; LSP alpha. | Becomes one of the top 3 schema-first Rust ORMs by search and mindshare. |
| **Q1 2027** | 2,000 stars; db pull / introspection; 5+ contributors; optional support pilots. | Commercial viability begins to emerge. |
| **Q2 2027** | `v0.3`; 3,000+ stars; 10+ contributors; first paid support customer or CI SaaS pilot. | Transition from solo project to small open-source business. |

### Funding-stage capability unlocks

- **Bootstrapped (now):** organic content, founder-led community, one release per month.
- **Seed close ($5–15K/mo marketing budget):** hire a technical writer / part-time community manager; run Product Hunt ads; sponsor one Rust meetup.
- **Series A ($50–150K/mo):** full-time marketing, dedicated docs/website, conference sponsorship, commercial support team.

---

## 11. Marketing Operations Stack

| AARRR stage | Skills / tactics | Tools / integrations | Owner |
|-------------|------------------|----------------------|-------|
| **Acquisition** | content-strategy, seo-audit, programmatic-seo, public-relations, social, directory-submissions, launch | GitHub Pages, docs.rs, Plausible, Reddit, HN, TWIR, YouTube, Product Hunt | Founder |
| **Activation** | onboarding, copywriting, cro | README, starter repos, quickstart docs, GitHub Discussions | Founder |
| **Retention** | emails, community-marketing, changelog | Substack/ConvertKit, Discord/Discussions, CHANGELOG.md | Founder + early contributors |
| **Referral** | community-marketing, public-relations, referrals | GitHub Stars, starter-template badges, case studies | Founder + champions |
| **Revenue** | pricing, sales-enablement | Stripe (future), Calendly, support email | Founder |

### Capability unlocks by stage

- **Pre-seed (now):** All execution is founder-led. Tools must be free or <$50/mo.
- **Seed:** First contractor (content/community); paid analytics and hosting.
- **Series A:** Marketing hire; sponsored content; conference presence; potential SaaS layer.

---

## 12. Tactical Idea Bank

| # | Idea | AARRR | Status | Timeline | Notes |
|---|------|-------|--------|----------|-------|
| 1 | "State of Rust ORMs 2026" teardown | Acquisition | Planned | Week 3 | High-SEO, high-shareability. |
| 2 | Comparison page vs SeaORM / Diesel / sqlx | Acquisition | Planned | Week 2 | Drives non-branded search. |
| 3 | Comparison page vs ferriorm / prax / toasty | Acquisition | Planned | Week 4 | Addresses the new cohort directly. |
| 4 | `ruprizzle-starter-axum` template | Activation | Planned | Week 2 | Biggest Rust web framework. |
| 5 | `try-ruprizzle` Docker one-liner | Activation | Planned | Week 2 | Removes setup friction. |
| 6 | Weekly schema-design / migration pattern post | Retention | Planned | Weeks 5+ | Builds authority and repeat visits. |
| 7 | YouTube "5-minute ruprizzle" demo | Acquisition | Planned | Week 5 | Rust community is YouTube-hungry. |
| 8 | TWIR project updates every 4–6 weeks | Retention | Planned | Ongoing | Low effort, high trust. |
| 9 | Product Hunt v0.2 launch | Acquisition | Planned | Week 12 | Major visibility event. |
| 10 | Hacker News "Show HN" for v0.2 | Acquisition | Planned | Week 12 | Use for launch day. |
| 11 | Reddit r/rust launch + follow-up comments | Acquisition | Planned | Week 12 | Be transparent; don't spam. |
| 12 | GitHub Discussions "Built with ruprizzle" | Referral | Planned | Week 4 | Social proof loop. |
| 13 | Contributor-first-PR shoutouts | Referral | Planned | Ongoing | Encourages repeat contributors. |
| 14 | Migration guides from SeaORM / sqlx / Diesel | Activation | Planned | Week 10 | Reduce switching friction. |
| 15 | Live demo / AMA at v0.2 | Acquisition | Planned | Week 12 | Engage the community. |
| 16 | Conference CFP (RustConf / EuroRust) | Acquisition | Planned | Q4 | Build credibility. |
| 17 | Co-marketing with Axum/Actix starter examples | Referral | Planned | Q2 | Borrow framework distribution. |
| 18 | Schema formatter + CI check `ruprizzle validate` | Retention | Planned | Q1 | Keeps schema clean in teams. |
| 19 | Public benchmarks vs competitors | Acquisition | Planned | Q2 | Use README numbers; expand. |
| 20 | Optional commercial support waitlist | Revenue | Future | Q4 | Gauge enterprise interest. |

---

## 13. Measurement, RACI, Open Decisions, and Appendix

### North star and leading indicators

**North star metric:** Number of active projects using ruprizzle (measured by GitHub dependents, issue/PR activity, and a quarterly survey).  
**Proxy for now:** GitHub stars + crates.io downloads + docs.rs build traffic.

| Stage | Leading indicator | Target (90 days) | Target (12 months) |
|-------|-------------------|------------------|--------------------|
| Acquisition | Weekly unique visitors to README/docs | 1,000 | 20,000 |
| Activation | Starter repo clones / quickstart completions | 50 | 1,000 |
| Retention | Returning GitHub visitors / Discord members | 20 members | 500 members |
| Referral | GitHub stars per month | 50/mo | 100/mo |
| Revenue | Support waitlist signups | 5 | 50 |

### RACI

| Activity | Responsible | Accountable | Consulted | Informed |
|----------|-------------|-------------|-----------|----------|
| Engineering / releases | Founder | Founder | Early contributors | Community |
| Content / blog / social | Founder | Founder | — | Community |
| Community / support | Founder | Founder | Contributors | Users |
| Docs / examples | Founder | Founder | Contributors | Users |
| Partnerships / integrations | Founder | Founder | — | Community |

### Open decisions

1. **MySQL before or after v0.2?** Recommendation: after v0.2 to ship a stable core; communicate clearly.
2. **Discord vs. GitHub Discussions?** Start with GitHub Discussions; move to Discord when >100 active members or >20 daily messages.
3. **Freemium support / paid tier?** Not before Q4 2026; collect waitlist only.
4. **Competitor response strategy?** Do not engage in public disputes; differentiate on transparency and consistency.
5. **Target first audience?** TypeScript converts and Rust web teams; both can be reached with the same content.

---

## Appendix A: Competitive Landscape

### Feature matrix (schema-first and new-generation ORMs)

| Project | GH stars | First commit | Downloads (main crate) | DBs | Schema-first | SQL transp. | Sidecar | Migrations | `include`/relations | Status |
|---------|----------|--------------|------------------------|-----|--------------|-------------|---------|------------|---------------------|--------|
| **ruprizzle** | 0 | 2026 | alpha.3 (new) | Postgres, SQLite | ✅ `schema.ruprizzle` | ✅ `.to_sql()` | ❌ none | ✅ auto-diff | ✅ bounded | Alpha |
| ferriorm | 2 | 2026-04 | 305 | Postgres, SQLite | ✅ `.ferriorm` | partial | ❌ | ✅ shadow DB | ✅ | Early, 1 contributor |
| prax-orm | 16 | 2025-12 | 452 | Postgres, MySQL, SQLite, MSSQL, MongoDB, DuckDB, Scylla | ✅ `.prax` | partial | ❌ | ✅ | ✅ | WIP, very broad |
| vitrail | 1 | 2026-03 | 490 (pg) | Postgres, SQLite, Cloudflare D1 | ✅ `schema!` | partial | ❌ | ✅ | ✅ | Early, D1 wedge |
| saola | 1 | 2026-04 | 81 (core) | Postgres, MySQL, SQLite, MSSQL, MongoDB, Cockroach | ✅ `schema.prisma` | no | ✅ Prisma engine | ✅ | ✅ | Uses Prisma sidecar |
| kosame | 89 | 2025-09 | 892 | Postgres only | macro-based (`pg_table!`) | no | ❌ | planned | ✅ | Prototype, no build step |
| drizzle-rs | 41 | 2025-07 | 579 | SQLite, libsql, Turso, Postgres (sync + async) | Rust DSL | yes | ❌ | manual + build.rs | partial | WIP, git-only |
| tiny-orm | 53 | 2024-10 | 8,768 | Postgres, MySQL, SQLite | derive macro | no | ❌ | ❌ | ❌ | CRUD only |
| taitan-orm | 124 | 2025-01 | 9,057 | ? (sqlx) | sqlx-based | partial | ❌ | ? | ? | Active, Chinese community |
| georm | 11 | 2025-01 | 1,995 | Postgres | derive macro | no | ❌ | ❌ | ✅ | SQLx ORM |
| lume | 3 | 2025-08 | 5,611 | MySQL, Postgres, SQLite | ? | partial | ❌ | ? | ? | Drizzle-inspired |
| toasty | 2,654 | 2024-10 | (new on crates.io) | Postgres, MySQL, SQLite, Turso, DynamoDB | derive macro | partial | ❌ | ✅ | ✅ | High-visibility, tokio-rs |

### Established alternatives

| Project | GH stars | Positioning | Why ruprizzle wins | Why ruprizzle loses |
|---------|----------|-------------|--------------------|---------------------|
| Diesel | 14,071 | Sync-first, type-safe DSL | Schema-first, async, auto migrations, no macros in app code | Maturity, contributors, battle-tested |
| SeaORM | 9,856 | Async ActiveRecord | Schema-first source of truth, SQL transparency, no drift | Large community, many integrations |
| sqlx | 17,122 | Compile-time checked raw SQL | Generated client, query builder, migrations, less boilerplate | Ecosystem size, raw SQL control |
| Prisma Client Rust | — | Prisma engine + Rust client | No sidecar, pure Rust, transparent SQL | Feature parity with Prisma ecosystem |

### Key strategic takeaways

1. **The new-ORM wave is real and crowded.** Every week another Prisma/Drizzle-inspired project appears. Differentiation must be concrete (`.to_sql()`, bounded `include`, build-time codegen, additive dialects), not just positioning.
2. **`toasty` is the awareness threat.** It has 2,600+ stars and the tokio-rs brand. Its model is derive-macro-based, not schema-file-first, and it targets SQL+NoSQL. Counter by owning the schema-first, SQL-transparent niche.
3. **`ferriorm` is the closest direct clone.** It is further along on some CRUD features and has a cleaner landing page. Counter by shipping faster, being more transparent, and building community.
4. **`prax` is the overreach risk.** It claims every database and feature under the sun. Counter by staying focused and correct.
5. **Downloads do not equal mindshare yet.** Many new crates have low star counts but decent download numbers from CI / mirrors. GitHub stars, issue activity, and real user posts are better signals.

---

## Appendix B: Customer Research — What the Market Actually Says

### Research method

- Digital watering holes: Hacker News, Reddit r/rust, Rust users forum, GitHub issues/discussions for SeaORM / sqlx / Diesel.
- Recency window: 2024–2026.
- Confidence levels: high (3+ independent sources), medium (2 sources), low (single source).

### Top themes

#### Theme 1: "sqlx is great until you need a query builder"

**Summary:** Developers love `sqlx` for compile-time raw SQL but hit a wall with dynamic/conditional queries and unnameable macro result types.

**Frequency:** High.  
**Intensity:** High (frustrated, specific).  
**Confidence:** High.

**Representative quotes:**
- "SQLx is just providing some 'core' foundations, which means yes there is no good query builder or anything like that." — Hacker News
- "SQLx sucks at dynamic queries. Dynamic predicates, WHERE IN clauses, etc." — Hacker News
- "The structs sqlx creates through the macros are unnameable types, they are created on the fly at the invocation site. You can't return them without transforming them into a type you own." — Hacker News
- "Sqlx is completely lacking in the query composability department, and leads to a very large amount of boilerplate." — Hacker News

**Implications for ruprizzle:** Position as "the generated client you wanted on top of sqlx." Show how `to_sql()` and the query builder handle dynamic filters without losing type safety.

#### Theme 2: "Diesel is powerful but heavy and not async"

**Summary:** Diesel's type system is respected, but its sync model, macro usage, and hand-written migrations are friction for modern async web stacks.

**Frequency:** High.  
**Intensity:** Medium-high.  
**Confidence:** High.

**Representative quotes:**
- "Diesel was ok, but I never use it anymore since rocket moved to async." — Hacker News
- "I found their query building catastrophically bad the moment the query isn't built all in one place." — Hacker News
- "Diesel's macro-heavy approach... compile times that punish rapid iteration." — The Editorial, 2026

**Implications:** Emphasize async-first, schema-first, and no hand-written migration files. Compare compile-time overhead directly.

#### Theme 3: "SeaORM is too opinionated and its schema is not the source of truth"

**Summary:** SeaORM is popular but developers complain about limited joins, migration DSL friction, `sqlx` SemVer breakage, and entity drift.

**Frequency:** High.  
**Intensity:** High.  
**Confidence:** High.

**Representative quotes:**
- "Sea ORM is too opinionated in my experience. Even making migration is not trivial with their own DSL." — Hacker News
- "I tried sea-orm, but I find its ORM API way too limited (it can't even do multiple joins)." — Hacker News
- "Sqlx 0.8.4 breaks sea-orm" — GitHub issue, 2025
- "It is insane that we can't even select three models of different tables with multiple joins by one query." — SeaORM GitHub discussion

**Implications:** Make the schema-first, no-drift, auto-migration value prop front and center. Show multi-level `include` examples that don't require manual joins.

#### Theme 4: "I want schema-first, zero boilerplate, and clean domain models"

**Summary:** There is strong demand for a single schema file that generates everything — and a desire to keep domain structs free of ORM attributes.

**Frequency:** Medium-High.  
**Intensity:** High.  
**Confidence:** Medium-High.

**Representative quotes:**
- "I just want to keep my business logic clean and don't want it to know about database or ORM related details." — Rust users forum
- "Existing Rust ORMs require manually defining structs, writing migrations, and wiring everything together. ferriorm takes the Prisma approach: define your schema once." — ferriorm README
- "Define your schema once, and everything else — type-safe Rust client, migrations, query builders — is generated for you." — ferriorm

**Implications:** This is the core pitch. "One `schema.ruprizzle` file, generated client, no derive macros in your app."

#### Theme 5: "No hidden engine / no sidecar is a real advantage"

**Summary:** Developers are wary of Prisma's Node sidecar and hidden query engines. Pure Rust, thin runtime is valued.

**Frequency:** Medium.  
**Intensity:** Medium.  
**Confidence:** Medium.

**Representative quotes:**
- "No bells and whistles, no Rust binaries, no serverless adapters, everything just works out of the box." — Drizzle marketing
- "Prisma Client Rust ... ships a Rust query engine and uses a Node sidecar for migration generation. ruprizzle is pure Rust with no sidecar binary." — ruprizzle README

**Implications:** Lead with "no sidecar, no hidden engine, no WASM runtime" in every comparison.

#### Theme 6: "Migrations should be generated, not hand-written"

**Summary:** Teams want schema diffing and automatic migration generation.

**Frequency:** Medium.  
**Intensity:** Medium.  
**Confidence:** Medium.

**Representative quotes:**
- "ferriorm diffs your schema against the database and generates SQL migrations for you." — ferriorm docs
- "Prisma's declarative schema as the single source of truth, with a generated typed client, automatic migration diffing." — ruprizzle README

**Implications:** Highlight `migrate dev` and the deliberate `migrate deploy` safety split.

### Customer language cheat sheet

Use these exact phrases in copy, headlines, and comparisons:
- "schema-first"
- "type-safe"
- "no sidecar"
- "no hidden query engine"
- "SQL transparency"
- ".to_sql()"
- "generated client"
- "automatic migrations"
- "no N+1"
- "pure Rust"
- "built on sqlx"
- "honest alpha"

### Research gaps

1. We have not run a survey or 1:1 interviews. First 20 active users should be interviewed.
2. We do not yet know the exact conversion rate from README → install → first query.
3. We have not tested which headline resonates most with TypeScript converts vs. existing Rust developers.

---

*End of marketing plan.*

*Next action: Review this plan, decide on the MySQL/LSP roadmap, set up GitHub Discussions, and begin the 90-day execution sprint.*
