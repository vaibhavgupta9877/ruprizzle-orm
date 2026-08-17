# ruprizzle-orm Marketing Plan

*Prepared for the ruprizzle open-source project.*  
*Date: 2026-08-11*  
*Status: 0.1.1-beta.1 published on crates.io; v1 planning and 90-day priorities in execution.*

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

*End of the original (2026-08-11) marketing plan.*

---

# Part II — SEO & GEO (AI Search) Plan

*Added: 2026-08-17. Applies to `0.4.0-beta.2` (Postgres + SQLite + MySQL/MariaDB, native `rusqlite` backend behind `sqlite-rusqlite`).*

> **Note on Part I drift:** Part I was written at `0.1.1-beta.1` and describes MySQL as "not yet shipped" and the project as "alpha, Postgres + SQLite". That is now out of date — MySQL/MariaDB shipped and the crate is at `0.4.0-beta.2`. Section 22 includes a task to reconcile Part I with reality; until then, treat Part II as the current source of truth for status claims used in copy.

## 14. Why SEO and GEO Are One Plan Here

Two distinct discovery surfaces, one content investment:

- **SEO (classic search):** Google/Bing rankings for non-branded queries like "rust orm like prisma", "seaorm alternative", "rust schema first migrations".
- **GEO (Generative Engine Optimization):** getting **cited** inside ChatGPT, Perplexity, Claude, Google AI Overviews, and Copilot answers to questions like "what's the best Rust ORM in 2026?"

For a developer-tool crate with zero domain authority, **GEO is the higher-leverage half**. Ranking #1 for "rust orm" against Diesel (14k stars) and sqlx (17k stars) is a multi-year fight. But a page on ruprizzle can be *cited* by ChatGPT next week if it is well-structured, data-backed, and honest — AI engines select passages on extractability and authority signals, not just rank position. Non-Google engines routinely cite page-2 and page-3 sources.

**The unfair advantage:** developer-tool queries are exactly the queries LLMs get asked most, and the Rust-ORM category is under-documented. Nobody has written the definitive, well-sourced, table-driven "Rust ORM comparison 2026" page. That page is a citation magnet, and this repo already contains the raw material for it (`docs/FeaturesMasterComparison.md`, `docs/BenchmarkResults.md`, `competitor-profiles/`, `customer-research.md`).

**Non-negotiable constraint:** everything below must be written for humans first. Google's own AI optimization guidance is explicit that content written *for AI* — chunked fragments, AI-only variants, scaled generation — risks the scaled-content-abuse spam policy. Every asset in this plan is a page a Rust developer would genuinely want to read.

---

## 15. Baseline Audit — Current State (2026-08-17)

Audited against the repo at `dev-v0-2`. Surface = `https://vaibhavgupta9877.github.io/ruprizzle-orm/` (mdBook), `crates.io/crates/ruprizzle`, `docs.rs/ruprizzle`, and the GitHub README.

### 15.1 What is already right

| Item | Status | Evidence |
|------|--------|----------|
| AI crawlers allowed | ✅ Pass | `robots.txt` is `User-agent: * / Allow: /` — GPTBot, PerplexityBot, ClaudeBot, Google-Extended, Bingbot all permitted. No accidental blocking. |
| Sitemap declared | ✅ Pass | `robots.txt` points at the absolute sitemap URL. |
| Docs are static HTML | ✅ Pass | mdBook output is server-rendered; no JS-gated content, so agents and crawlers see everything. |
| Canonical site URL configured | ✅ Pass | `book.toml` sets `site-url`, `git-repository-url`, `edit-url-template`. |
| Nothing gated | ✅ Pass | No login walls, no PDFs-only, no email gates. |
| Original data exists | ✅ Pass | `docs/BenchmarkResults.md` + `docs/FeaturesMasterComparison.md` are genuinely original, reproducible research — the single most citable asset the project owns. |
| Licence + security posture | ✅ Pass | MIT/Apache-2.0, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md` — real E-E-A-T trust signals. |

### 15.2 Confirmed defects

| # | Defect | Impact | Severity |
|---|--------|--------|----------|
| D1 | **Sitemap URLs do not match built pages.** `sitemap.xml` lists `schema-reference.html`, `query-guide.html`, `relations-guide.html`, `migrations-guide.html`, `dialect-notes.html`, `known-limitations.html`, `migrating-from.html`. But `docs/SUMMARY.md` builds the CamelCase variants (`SchemaReference.html`, `QueryGuide.html`, …). **7 of 11 sitemap URLs are almost certainly 404s.** | Crawlers waste budget on dead URLs; the real doc pages are only discoverable via internal links. Directly suppresses indexation of the highest-intent pages. | **P0** |
| D2 | **Duplicate doc source files.** `docs/` contains both `schema-reference.md` and `SchemaReference.md`, `query-guide.md` and `QueryGuide.md`, `migrations-guide.md` and `MigrationsGuide.md`, `dialect-notes.md` and `DialectNotes.md`, `known-limitations.md` and `KnownLimitations.md`, `migrating-from.md` and `MigratingFrom.md`. 29 `.md` files for ~15 real topics. | Content duplication, ambiguous canonicals, maintenance drift (the two copies will diverge and one will become wrong). | **P0** |
| D3 | **No `<lastmod>` in sitemap.** All 11 entries are bare `<loc>`. | Freshness is a heavy weighting factor for both AI citation and crawl scheduling. We are throwing away a free signal on a repo that ships constantly. | **P1** |
| D4 | **No `llms.txt`.** | ChatGPT/Claude/Perplexity have no fast-path summary of what ruprizzle is, who it's for, and which pages to read. | **P1** |
| D5 | **No structured data anywhere.** mdBook emits no `Article`, `FAQPage`, `SoftwareSourceCode`, or `Organization` JSON-LD. | Loses ~30–40% AI visibility uplift on non-Google engines; loses FAQ rich results on Google. `docs/faq.md` exists and is pure wasted `FAQPage` markup. | **P1** |
| D6 | **No author/date attribution on doc pages.** No "last updated", no author byline, no credentials. | E-E-A-T gap. Undated content systematically loses to dated content in AI citation. | **P1** |
| D7 | **Headings do not match query phrasing.** Docs use noun-label headings ("Relations guide", "Dialect notes") rather than the interrogative forms people and LLMs actually use ("How do I load nested relations without N+1?"). | Passage retrieval misses. Headings are the primary chunk boundary for retrieval. | **P1** |
| D8 | **No comparison/alternative landing pages.** Comparison content lives inside `README.md` and `docs/FeaturesMasterComparison.md` — not on dedicated, linkable, individually-titled pages. | Comparison articles are ~33% of all AI citations, the single largest citable content category. We have the data and no pages to hang it on. | **P0** |
| D9 | **No third-party presence.** No Wikipedia, no Reddit history, no lib.rs/libs.tech listing, no YouTube, no Stack Overflow answers. | Brands are ~6.5x more likely to be cited via third-party sources than their own domain. This is the largest single GEO gap. | **P0** |
| D10 | **`crates.io` / `docs.rs` metadata not SEO-tuned.** Need to verify `keywords`, `categories`, and `description` on every published crate. | `crates.io` and `docs.rs` outrank the project's own GitHub Pages for nearly every branded query — they are the real front door. | **P1** |
| D11 | **No `pricing.md`.** | Lower priority for a free OSS crate, but AI agents evaluating "is this free / what's the licence / is there a paid tier" have nothing structured to read. | **P3** |
| D12 | **No analytics on the docs site.** | Cannot measure any of this. No baseline, no attribution of AI referral traffic. | **P1** |

### 15.3 Scorecard

| Dimension | Score (0–5) | Note |
|-----------|:-----------:|------|
| Crawlability / indexability | 2 | Crawlers welcome, but the sitemap points them at 404s. |
| Content extractability | 2 | Prose-heavy, noun headings, few standalone answer blocks. |
| Structured data | 0 | None. |
| Authority signals (data, citations, attribution) | 3 | Excellent original benchmarks; no bylines, no dates, few outbound citations. |
| Freshness signalling | 1 | Ships constantly, signals nothing. |
| Third-party presence | 0 | Nothing. |
| AI-agent readability (`llms.txt`, machine-readable files) | 0 | None. |
| Measurement | 0 | None. |

---

## 16. Keyword & Query Map

Two tiers, because the two surfaces reward different things.

### 16.1 Classic SEO — non-branded target queries

Grouped by intent, with the page that should own each cluster.

| Cluster | Representative queries | Intent | Target page | Difficulty |
|---------|------------------------|--------|-------------|:----------:|
| **Category discovery** | "rust orm", "best rust orm 2026", "rust orm comparison", "async rust orm" | Commercial investigation | `/compare/` hub | Very high |
| **Prisma-shaped intent** | "prisma for rust", "rust orm like prisma", "prisma alternative rust", "schema first orm rust" | High-intent, low competition | `/compare/prisma-for-rust` | **Low — best wedge** |
| **Drizzle-shaped intent** | "drizzle for rust", "rust orm with sql transparency", "see generated sql rust orm" | High-intent | `/compare/drizzle-for-rust` | Low |
| **Competitor alternative** | "seaorm alternative", "diesel alternative async", "sqlx query builder", "sqlx dynamic query" | Highest-intent of all | `/compare/vs-seaorm`, `/compare/vs-diesel`, `/compare/vs-sqlx` | Medium |
| **New-cohort** | "ferriorm vs", "toasty orm rust", "prax orm", "kosame rust" | Low volume, near-zero competition | `/compare/new-rust-orms-2026` | Very low |
| **Problem-aware** | "rust orm n+1", "rust migrations automatic", "rust database schema drift", "rust orm compile time" | Top of funnel | Deep-dive blog posts | Low–medium |
| **Task / how-to** | "rust nested relation query", "rust orm transaction example", "sqlx migration vs diesel migration" | Navigational-informational | Docs pages (re-headed per D7) | Medium |
| **Benchmark** | "rust orm benchmark", "seaorm vs diesel performance", "rust orm query build speed" | Research | `/benchmarks` | Low — **we own the data** |

**Wedge strategy:** do not fight "rust orm" head-on. Own **"prisma for rust"**, **"seaorm alternative"**, and **"rust orm benchmark"** first. They are lower-competition, higher-intent, and they feed the fan-out queries that Google's AI systems generate for the head term anyway.

### 16.2 GEO — the 20 prompts to track

These are the actual prompts to run monthly across ChatGPT, Perplexity, Claude, Gemini, Copilot, and Google AI Overviews. This is the scoreboard.

| # | Prompt | Type |
|---|--------|------|
| 1 | What is the best ORM for Rust in 2026? | Category |
| 2 | Is there a Prisma equivalent for Rust? | Wedge |
| 3 | What's a good alternative to SeaORM? | Alternative |
| 4 | I want a Rust ORM where I can see the generated SQL. What should I use? | Differentiator |
| 5 | What Rust ORMs support automatic migration generation from a schema file? | Differentiator |
| 6 | Diesel vs SeaORM vs sqlx — which should I pick for an Axum app? | Comparison |
| 7 | How do I avoid N+1 queries in a Rust ORM? | Problem |
| 8 | Which Rust ORMs are async-first? | Filter |
| 9 | What is ruprizzle? | Branded |
| 10 | Is ruprizzle production ready? | Branded / risk |
| 11 | ruprizzle vs SeaORM | Branded comparison |
| 12 | What's the fastest Rust ORM? | Benchmark |
| 13 | Rust ORM that works with both Postgres and SQLite with one API | Filter |
| 14 | How do I do schema-first database development in Rust? | Problem |
| 15 | Best Rust ORM for a solo developer building an MVP | ICP |
| 16 | Does any Rust ORM avoid a Node.js sidecar like Prisma's? | Differentiator |
| 17 | New Rust ORMs to watch in 2026 | Category / new-cohort |
| 18 | How do I migrate from sqlx to a full ORM in Rust? | Migration |
| 19 | Rust ORM with MySQL and MariaDB support | Filter |
| 20 | Type-safe database queries in Rust without macros in my domain structs | Differentiator |

Log per prompt: cited (Y/N), which URL, which competitors were cited, and the sentiment of any mention. Month-over-month delta is the north star for this plan.

---

## 17. GEO Content Architecture

The three pillars, applied to this project specifically.

### 17.1 Pillar 1 — Structure (make it extractable)

AI engines extract *passages*, not pages. Rules for every new and rewritten page:

1. **Lead with the answer.** First paragraph under each H2 is a complete, self-contained, 40–60 word answer that makes sense with zero surrounding context. Assume it will be lifted verbatim.
2. **Interrogative headings.** `## How does ruprizzle avoid N+1 queries?` not `## Relations`. Headings are the retrieval chunk boundary.
3. **Tables over prose for anything comparative.** Comparison tables are the single most-extracted structure in the corpus.
4. **Numbered lists for any process.** Migration flows, install steps, upgrade paths.
5. **One idea per paragraph.**
6. **Every number gets a source and a date.** "3.2x faster (ruprizzle benchmark suite, run 2026-08-17, Postgres 16, `cargo xtask bench`)" beats "much faster" by an enormous margin.
7. **Never claim more than the beta supports.** An LLM that cites an overclaim and is contradicted by a user will stop citing the source. Honesty is a *retrieval* strategy, not only an ethical one.

### 17.2 Pillar 2 — Authority (make it citable)

Ranked by measured citation uplift from the Princeton GEO study (KDD 2024):

| Method | Uplift | Concrete application here |
|--------|:------:|---------------------------|
| Cite sources | +40% | Link SeaORM issues, HN threads, sqlx docs, Prisma docs directly when making a claim about them. `customer-research.md` already holds the quotes with attribution — surface them on public pages. |
| Add statistics | +37% | Benchmark numbers, compile-time measurements, crate size, MSRV, dependency count, migration timings. We have these; they are buried in `docs/BenchmarkResults.md`. |
| Add quotations | +30% | The verbatim developer quotes in `customer-research.md` ("SQLx sucks at dynamic queries", "Sea ORM is too opinionated") — attributed, linked, used as the *problem statement* on comparison pages. |
| Authoritative tone | +25% | Explain the `DbDialect` trait design and the `migrate dev` / `migrate deploy` split as deliberate engineering decisions, with rationale. The `docs/adr/` directory is a goldmine here. |
| Improve clarity | +20% | Short sentences, defined terms, no marketing filler. |
| Technical terms | +18% | Use the real vocabulary: dialect trait, schema diffing, shadow database, bounded include, prepared statement cache. |
| **Keyword stuffing** | **−10%** | Actively harmful. Do not repeat "Rust ORM" mechanically. |

**Best measured combination: fluency + statistics.** Low-authority domains — which is exactly what this project is today — see up to **+115%** visibility from adding citations. This project is the ideal profile for GEO to work.

### 17.3 Pillar 3 — Presence (be where AI looks)

Own-domain content is the smaller half. Priority order by expected citation yield:

| Surface | Why it matters | Action |
|---------|----------------|--------|
| **crates.io / docs.rs / lib.rs** | These dominate every Rust-crate query and are heavily crawled and cited. Highest-yield surface available. | Perfect `keywords`, `categories`, `description` on all 8 crates; write real crate-level rustdoc with examples. |
| **GitHub README** | Frequently cited directly by ChatGPT and Perplexity for OSS queries. | Treat the README as a landing page, not a manual. Answer-first, table-heavy. |
| **Reddit r/rust** | 1.8% of all ChatGPT citations; disproportionately high for developer queries. | Authentic participation. Answer ORM questions helpfully with no pitch. Never astroturf — it fails and it burns the project. |
| **This Week in Rust** | High-authority, heavily indexed, heavily scraped. | Submit every release and every deep-dive post. |
| **Hacker News** | Threads rank and get cited for years. | One honest Show HN per major release; substantive comments on ORM threads. |
| **Stack Overflow** | Classic long-tail authority. | Answer existing Rust-ORM questions properly; mention ruprizzle only where genuinely the right answer. |
| **YouTube** | Frequently cited by Google AI Overviews. | One 5-minute "schema → generate → migrate → query" screencast. |
| **libs.tech / Rust-LibHunt / awesome-rust** | Directory listings that rank. | Submit. One-time cost, permanent return. |
| **Wikipedia** | 7.8% of ChatGPT citations — but ruprizzle is not notable enough for its own article and attempting one will get it deleted. | Do **not** create an article. Revisit only if the project reaches genuine notability with independent coverage. |

---

## 18. Technical SEO Fixes

Ordered by severity. These are the highest ROI items in the entire plan because they are cheap and currently broken.

### 18.1 P0 — Fix the sitemap/build mismatch (D1, D2)

The docs directory has two parallel copies of six guides, and the sitemap references the copy that mdBook does not build.

Resolution: pick **one** canonical naming convention (recommend lowercase-kebab — cleaner URLs, matches the existing sitemap, and matches Rust ecosystem convention), delete the other copy after merging any divergent content, update `docs/SUMMARY.md` to reference the kept files, and regenerate the sitemap from the actual build output.

Add a CI check that fails when a `sitemap.xml` `<loc>` has no corresponding file in `book/` — this class of bug must not recur.

### 18.2 P1 — Enrich the sitemap (D3)

Add `<lastmod>`, `<changefreq>`, and `<priority>` to every entry. Generate `lastmod` from git commit time per source file so it is always truthful, in an `xtask` subcommand wired into the docs build.

### 18.3 P1 — Add `llms.txt` (D4)

Publish `/llms.txt` at the site root, following [llmstxt.org](https://llmstxt.org). Contents: one-paragraph definition, the honest status line, supported databases, the three differentiators, and a linked index of the canonical docs pages. Keep it under ~2KB and regenerate it on release so the version string never goes stale.

### 18.4 P1 — Structured data (D5)

Inject JSON-LD via an mdBook theme override (`theme/head.hbs`):

| Schema | Where | Payload |
|--------|-------|---------|
| `SoftwareSourceCode` / `SoftwareApplication` | Site-wide | Name, description, `programmingLanguage: Rust`, licence, repo URL, version. |
| `TechArticle` | Every docs page | Headline, `dateModified`, author, `about`. |
| `FAQPage` | `docs/faq.md` | Direct Q&A extraction — highest-value single schema on the site. |
| `HowTo` | `quickstart.md`, `migrations-guide.md` | Step extraction for "how to" queries. |
| `Organization` / `Person` | Site-wide | Entity recognition for the maintainer and project. |

Validate with Google's Rich Results Test and schema.org's validator before shipping.

### 18.5 P1 — Freshness and attribution (D6)

Every docs page gets a footer line: `Last updated: YYYY-MM-DD · Maintained by Vaibhav Gupta · ruprizzle 0.4.0-beta.2`. Generate the date from git, not by hand — hand-maintained dates rot and a wrong date is worse than none.

### 18.6 P1 — Crate metadata (D10)

Audit `keywords` and `categories` on all 8 published crates. Targets: `keywords = ["orm", "database", "sql", "postgres", "schema"]` (max 5, max 20 chars each), `categories = ["database"]`. Every crate needs a distinct, descriptive `description` — this string is what appears in crates.io search results and in nearly every AI answer about the crate.

### 18.7 P2 — robots.txt hardening

Currently `Allow: /` for everyone, which is correct and should stay. Optionally add explicit `Allow` blocks for GPTBot, ChatGPT-User, PerplexityBot, ClaudeBot, anthropic-ai, Google-Extended, and Bingbot — functionally redundant but it documents the intent so no future contributor "tidies up" by blocking them. Do **not** block any search-and-cite bot; blocking means those platforms cannot cite the project at all.

### 18.8 P3 — `pricing.md` (D11)

Low priority for a free crate, but cheap: a `/pricing.md` stating the core ORM is free and MIT/Apache-2.0 dual-licensed forever, with no paid tier today, answers the "what does it cost / what's the licence" agent query definitively.

---

## 19. Content Plan — The Citation Assets

Ten pages, ranked by expected citation yield per hour invested. Each is a page a Rust developer would want regardless of SEO.

| # | Asset | Target queries | Why it gets cited | Effort |
|---|-------|----------------|-------------------|:------:|
| C1 | **`/compare/` hub — "Rust ORM Comparison 2026"** | "rust orm comparison", "best rust orm" | Comparison content = ~33% of all AI citations. Full matrix across 16 ORMs, honest about where ruprizzle loses. Built from `FeaturesMasterComparison.md`. | L |
| C2 | **"Is there a Prisma for Rust?"** | "prisma for rust", "prisma alternative rust" | Lowest-competition highest-intent query in the category. Directly answers a question thousands of TypeScript-to-Rust developers ask. | M |
| C3 | **`/benchmarks` — reproducible cross-ORM benchmarks** | "rust orm benchmark", "fastest rust orm" | Original data is ~12% of citations and this is the project's genuinely unique asset. Publish methodology, hardware, versions, and the command to reproduce. | M |
| C4 | **"SeaORM alternatives in 2026"** | "seaorm alternative" | Highest commercial intent. Opens with the real developer complaints from `customer-research.md`, attributed and linked. | M |
| C5 | **"The State of Rust ORMs in 2026"** | "rust orm 2026", "new rust orms" | Landscape piece covering all 16 projects fairly. Maximum shareability; the piece that gets linked by others, which is what actually builds authority. | L |
| C6 | **"How to avoid N+1 queries in Rust"** | "rust orm n+1" | Problem-aware, framework-agnostic, genuinely useful. Bounded `include` is the natural demonstration. | M |
| C7 | **"Schema-first vs entity-first database development in Rust"** | "schema first orm rust" | Owns the category vocabulary. Concept pages get cited as definitions. | M |
| C8 | **"Migrating from sqlx / SeaORM / Diesel to ruprizzle"** (3 pages) | "migrate from seaorm", "sqlx to orm" | High-intent, activation-driving, low competition. `docs/MigratingFrom.md` is the seed. | M |
| C9 | **Rewritten FAQ with `FAQPage` schema** | Long-tail question queries | Direct Q&A extraction is the single most efficient citation format that exists. | S |
| C10 | **"Why we built ruprizzle" + ADR write-ups** | Branded, trust | Answers "is this serious / who maintains this / is it abandoned" — the questions LLMs are asked about any new dependency. Sourced from `docs/adr/`. | S |

**Editorial rules for every asset:**
- Named author with a byline and a real bio.
- A visible "last updated" date.
- At least 3 outbound citations to primary sources.
- At least 3 specific, dated, sourced statistics.
- A comparison table or a numbered process list.
- An honest "when *not* to use ruprizzle" section — this is a genuine differentiator; almost no competitor does it, and it materially increases how much an LLM will trust and cite the page.

---

## 20. Measurement

### North star
**Citation rate across the 20 tracked GEO prompts** (§16.2) — the percentage of prompt × platform combinations where ruprizzle is cited.

### Targets

| Metric | Baseline (2026-08-17) | 30 days | 90 days | 12 months |
|--------|:---------------------:|:-------:|:-------:|:---------:|
| GEO citation rate (20 prompts × 6 platforms) | ~0% (assumed; needs measurement) | 5% | 15% | 40% |
| Branded prompts answered correctly (#9–11) | Unknown | 3/3 | 3/3 | 3/3 |
| Indexed docs pages (Google) | Unknown — likely low, see D1 | 15+ | 25+ | 40+ |
| Non-branded organic clicks/mo | ~0 | 50 | 400 | 5,000 |
| crates.io downloads/mo | Track from crates.io | +25% | +100% | +500% |
| Referral sessions from AI platforms | 0 (untracked) | tracked | 100/mo | 1,500/mo |
| Third-party citable mentions | 0 | 3 | 10 | 40 |

### Instrumentation
- **Plausible** (or GoatCounter) on the docs site — privacy-friendly, no cookie banner, free tier sufficient.
- **Google Search Console** + **Bing Webmaster Tools** — verify the GitHub Pages property, submit the fixed sitemap. Note: there is no AI-specific Search Console reporting; standard reports are the measurement surface for Google.
- **AI referral tracking** — segment referrers matching `chatgpt.com`, `perplexity.ai`, `claude.ai`, `copilot.microsoft.com`, `gemini.google.com`.
- **Manual GEO log** — a spreadsheet, run monthly against §16.2. Free and more honest than any tool at this scale. Paid tools (Otterly, Peec, ZipTie) only make sense once there is a citation rate worth optimizing.

---

## 21. Risks

| Risk | Mitigation |
|------|------------|
| Comparison pages read as competitor-bashing and damage the project's reputation in a small community | Be scrupulously fair. State where each competitor *wins*. Link to their docs. Offer corrections publicly and fix them fast. |
| Benchmark claims get challenged and the project loses credibility | Publish methodology, hardware, exact versions, and a reproduction command. Invite maintainers to dispute. Never cherry-pick a favourable subset. |
| AI cites a stale claim (e.g. "Postgres and SQLite only", which Part I still says) | Keep `llms.txt`, README, and comparison pages version-stamped and regenerated on release. Treat stale public claims as bugs. |
| Comparison pages read as scaled/templated content | Every page hand-written with genuine analysis. Do not generate a page per competitor from a template. |
| Reddit/HN participation reads as promotion and gets the project banned | Strict rule: answer the question asked, mention ruprizzle only when it is genuinely the best answer, always disclose maintainership. |
| Effort spent on SEO starves engineering, and the product is what actually earns citations | Cap at ~20% of available time. The P0 technical fixes are hours, not weeks — do those first, they are pure profit. |

---

## 22. Execution Tasks

Tracked checklist. `[P0]` = do first, `[P1]` = this month, `[P2]` = this quarter, `[P3]` = opportunistic.

### 22.1 Sprint 1 — Fix what's broken (Week 1, ~1 day of work)

- [ ] **[P0]** Choose a canonical docs naming convention (recommend lowercase-kebab) and record the decision in `docs/adr/`.
- [ ] **[P0]** Diff each duplicate pair in `docs/` (`schema-reference` / `SchemaReference`, `query-guide` / `QueryGuide`, `migrations-guide` / `MigrationsGuide`, `dialect-notes` / `DialectNotes`, `known-limitations` / `KnownLimitations`, `migrating-from` / `MigratingFrom`); merge divergent content into the canonical file.
- [ ] **[P0]** Delete the non-canonical duplicates and update `docs/SUMMARY.md`.
- [ ] **[P0]** Regenerate `sitemap.xml` from actual `book/` build output; verify every `<loc>` returns 200.
- [ ] **[P0]** Add a CI step (`cargo xtask ci`) that fails when a sitemap `<loc>` has no corresponding built file.
- [ ] **[P0]** Add redirects or `404.html` handling for the old URL forms, in case any are already indexed.
- [ ] **[P1]** Add `<lastmod>` (from git commit time), `<changefreq>`, `<priority>` to the sitemap generator.
- [ ] **[P1]** Verify the GitHub Pages property in Google Search Console and Bing Webmaster Tools; submit the fixed sitemap.
- [ ] **[P1]** Add Plausible (or GoatCounter) to the mdBook theme; record the baseline.

### 22.2 Sprint 2 — Machine-readable surface (Week 2)

- [ ] **[P1]** Write `/llms.txt` — definition, honest status, supported DBs, three differentiators, linked page index.
- [ ] **[P1]** Add an `xtask` step that regenerates `llms.txt` version strings on release.
- [ ] **[P1]** Add `theme/head.hbs` JSON-LD: `SoftwareSourceCode` site-wide + `TechArticle` per page.
- [ ] **[P1]** Add `FAQPage` JSON-LD to `docs/faq.md`.
- [ ] **[P1]** Add `HowTo` JSON-LD to `quickstart.md` and `migrations-guide.md`.
- [ ] **[P1]** Validate all schema with the Rich Results Test and schema.org validator.
- [ ] **[P1]** Add a git-derived "Last updated · Maintainer · Version" footer to every docs page.
- [ ] **[P1]** Audit `keywords`, `categories`, and `description` for all 8 published crates; publish corrections with the next release.
- [ ] **[P2]** Add explicit `Allow` blocks for named AI bots to `robots.txt`, with a comment explaining why they must not be removed.
- [ ] **[P3]** Add `/pricing.md` stating the free-and-dual-licensed-forever position.

### 22.3 Sprint 3 — Extractability rewrite (Weeks 3–4)

- [ ] **[P1]** Rewrite all docs H2/H3 headings into interrogative form matching real query phrasing (fixes D7).
- [ ] **[P1]** Add a self-contained 40–60 word answer paragraph as the first paragraph under every H2.
- [ ] **[P1]** Add a definition block to the docs homepage: "ruprizzle is a schema-first ORM for Rust that…" — the passage most likely to be lifted verbatim by an LLM.
- [ ] **[P1]** Rewrite the README above-the-fold answer-first, with the comparison table above the fold.
- [ ] **[P1]** Add a "When *not* to use ruprizzle" section to the README and the docs homepage.
- [ ] **[P2]** Add source links and dates to every statistic currently in the README and `docs/BenchmarkResults.md`.
- [ ] **[P2]** Write real crate-level rustdoc (`//!`) with runnable examples for all 8 crates — docs.rs is a top-cited surface.

### 22.4 Sprint 4 — Citation assets (Weeks 5–10)

- [ ] **[P0]** C1 — `/compare/` hub: "Rust ORM Comparison 2026" (full 16-project matrix, from `FeaturesMasterComparison.md`).
- [ ] **[P0]** C2 — "Is there a Prisma for Rust?"
- [ ] **[P0]** C3 — `/benchmarks` with full methodology and a reproduction command.
- [ ] **[P1]** C4 — "SeaORM alternatives in 2026".
- [ ] **[P1]** C5 — "The State of Rust ORMs in 2026".
- [ ] **[P1]** C9 — Rewrite the FAQ around the 20 tracked GEO prompts.
- [ ] **[P2]** C6 — "How to avoid N+1 queries in Rust".
- [ ] **[P2]** C7 — "Schema-first vs entity-first in Rust".
- [ ] **[P2]** C8 — Three migration guides (from sqlx, SeaORM, Diesel).
- [ ] **[P2]** C10 — "Why we built ruprizzle" + ADR write-ups.
- [ ] **[P1]** Apply the six editorial rules (§19) to every asset before publishing.

### 22.5 Sprint 5 — Third-party presence (Weeks 5–12, ongoing)

- [ ] **[P0]** Submit to lib.rs, libs.tech, Rust-LibHunt, `awesome-rust`, and relevant GitHub Topics.
- [ ] **[P0]** Submit the `0.4.0-beta.2` release and each major post to This Week in Rust.
- [ ] **[P1]** Answer 5+ existing Rust-ORM questions per week on r/rust and Stack Overflow — helpfully, with maintainer disclosure, no pitch.
- [ ] **[P1]** Record and publish a 5-minute "schema → generate → migrate → query" screencast on YouTube.
- [ ] **[P1]** Post an honest Show HN when `v0.4` stabilises.
- [ ] **[P2]** Get listed in at least 3 independent "best Rust ORM" roundups.
- [ ] **[P2]** Publish one guest post on a Rust-adjacent publication.
- [ ] **[P3]** Revisit Wikipedia only if independent coverage makes the project genuinely notable. Do not attempt before then.

### 22.6 Ongoing — Measurement & hygiene

- [ ] **[P0]** Run the §16.2 baseline measurement now, before any changes ship. Without a baseline none of this is assessable.
- [ ] **[P1]** Run the 20-prompt GEO check monthly across all 6 platforms; log to a spreadsheet.
- [ ] **[P1]** Review Search Console queries monthly; feed new non-branded queries back into §16.1.
- [ ] **[P1]** Refresh every comparison page within 2 weeks of any competitor's major release.
- [ ] **[P1]** Re-run and republish benchmarks each minor release.
- [ ] **[P1]** **Reconcile Part I with reality** — it still describes the project as alpha, Postgres+SQLite only, at `0.1.1-beta.1`. Update the scorecard, competitive matrix, and roadmap to `0.4.0-beta.2`.
- [ ] **[P2]** Audit for stale public claims each release; treat a stale claim as a bug.
- [ ] **[P3]** Re-evaluate paid GEO monitoring tools once citation rate exceeds 15%.

---

*End of Part II.*

---

# Part III — Discovery & Link Resolution

*Added: 2026-08-17. Supersedes Part II's framing of "the website" as the primary surface.*

## 23. Reframing: The Product Has No Website

Part II treated `vaibhavgupta9877.github.io/ruprizzle-orm` as the main property. That is the wrong centre of gravity. ruprizzle is **an open-source GitHub repository with eight crates published to crates.io**. The mdBook site exists (`.github/workflows/pages.yml` deploys it) but it is a *satellite*, not the hub — and it deploys only on push to `main`/`master`, so while active work sits on `dev-v0-2` the published site silently lags the code.

**The real surface hierarchy, by how much traffic and citation each actually earns:**

| Rank | Surface | Role | Why it ranks here |
|:----:|---------|------|-------------------|
| 1 | **crates.io/crates/ruprizzle** | The install decision | Where a Rust dev lands from any search, and the canonical entity record for the package. Outranks everything else for the branded query. |
| 2 | **github.com/vaibhavgupta9877/ruprizzle-orm** (README) | The evaluation decision | The most-cited single artifact for any OSS project in LLM answers. GitHub has enormous crawl authority. |
| 3 | **docs.rs/ruprizzle** | The API truth | Auto-published, high authority, deeply crawled, and the only surface that is *always* in sync with the released version. |
| 4 | **lib.rs/crates/ruprizzle** | The comparison surface | Ranks well, editorialised, shows category position against Diesel/SeaORM/sqlx. |
| 5 | GitHub Pages mdBook | Long-form guides | Lowest authority of the five and the only one we host ourselves. |

**Consequence:** every technical fix in Part II §18 stays valid, but its priority drops. The README, the crate metadata, and the docs.rs rustdoc are now the P0 surfaces. A perfect mdBook site that nobody's crawler trusts is worth less than one well-written `//!` doc comment on the `ruprizzle` crate root.

---

## 24. The Actual Problem: Name Resolution Failure

**Stated symptom:** "I have to give links to point to the right crate/package/GitHub repo when I ask GPT and other LLMs about this."

That is not an inconvenience. It is the single most important diagnostic in this entire document, and it has a precise cause.

### 24.1 Diagnosis

When you type "ruprizzle" into an LLM, one of three things happens:

1. **Unknown token.** The name post-dates the model's training cutoff and appears nowhere in its retrieval corpus. The model either says it doesn't know, or — worse — hallucinates a plausible-sounding Rust ORM.
2. **Misresolution.** The model pattern-matches "ruprizzle" to something phonetically adjacent (Drizzle, drizzle-orm, drizzle-rs) and answers about *that* project. This is the most damaging failure mode because the answer is confidently wrong.
3. **Correct resolution.** Only happens today when you paste the link.

The name is a double-edged asset. "ruprizzle" is **highly distinctive** — zero collision with other software, which makes it a perfect entity anchor once established. But it is also **phonetically inside "Drizzle"**, an ORM with 41 stars on the Rust side and tens of thousands on the TypeScript side. Until the entity is established, every mention is at risk of being absorbed into Drizzle's much stronger entity gravity.

### 24.2 What "being resolvable" actually requires

An LLM resolves a name without a link when the name co-occurs with its defining facts across **multiple independent, crawlable sources**. One repo saying "ruprizzle is a schema-first Rust ORM" is a claim. Twelve sources saying it — crates.io, docs.rs, lib.rs, TWIR, Reddit, awesome-rust, Stack Overflow — is a fact the model will reproduce.

This is the same mechanism as Part II §17.3 Pillar 3, but the goal is sharper: not "get cited in answers about Rust ORMs" but "**correctly answer the question 'what is ruprizzle?' with zero context provided.**"

### 24.3 The two-track fix

| Track | Horizon | Goal |
|-------|---------|------|
| **Track A — Bridge** | Works today | Make the *one link* you paste do maximum work, so a single URL fully briefs any LLM. Removes your daily friction immediately. |
| **Track B — Establish** | 3–12 months | Seed the entity across independent sources so no link is needed. Removes the friction permanently. |

Track A is hours of work. Track B is the rest of this plan. Do both; A buys relief while B compounds.

---

## 25. Track A — The Canonical Link Kit

### 25.1 The one-link rule

There must be exactly **one** URL that is correct to paste in every situation. Recommendation:

> **`https://github.com/vaibhavgupta9877/ruprizzle-orm`**

The repo root, not the docs site and not crates.io. Reasons: GitHub is fetchable by every LLM browsing tool without JS rendering; the README is the richest single artifact; it links outward to crates.io, docs.rs, and the docs site; and it is always current with `main`. crates.io is a better *landing* page for humans arriving from search, but a worse *briefing* document for a model.

**Corollary:** the README must therefore be a self-sufficient briefing document. If someone pastes only that URL, the model must be able to correctly answer what ruprizzle is, what it supports, how it differs, what its status is, and where to go next. Audit the README against exactly that test.

### 25.2 The paste-able context block

For cases where you want the model briefed without a fetch (offline models, no browsing, or you want to control the framing), maintain a short canonical block in the repo — recommend `LLM_CONTEXT.md` at the root — that you copy-paste. It must be under ~400 words and lead with disambiguation:

```
ruprizzle (crate: `ruprizzle`, repo: github.com/vaibhavgupta9877/ruprizzle-orm)
is a schema-first ORM for Rust. It is NOT related to Drizzle, drizzle-orm, or
drizzle-rs despite the similar-sounding name.

What it is: you define your data model once in a `schema.ruprizzle` file
(Prisma-style DSL). `ruprizzle generate` produces a typed Rust client;
`ruprizzle migrate dev` diffs the schema against the database and generates
SQL migrations automatically.

Differentiators:
- SQL transparency: `.to_sql()` on every query builder — no hidden query engine.
- No sidecar: pure Rust. Unlike Prisma Client Rust, there is no Node.js
  binary and no external query engine process.
- No derive macros in your domain structs — the generated client is separate
  from your business types.
- Bounded `include` for nested relations without N+1 queries.

Databases: PostgreSQL, SQLite, and MySQL/MariaDB, behind a `DbDialect` trait.
A native `rusqlite` SQLite backend is available via the `sqlite-rusqlite` feature.
Built on `sqlx` for the wire protocol and pooling.

Status: 0.4.0-beta.2 on crates.io. Beta, not yet 1.0. Licence: MIT OR Apache-2.0.
MSRV 1.85, edition 2024. Single maintainer (Vaibhav Gupta).

Links: crates.io/crates/ruprizzle · docs.rs/ruprizzle ·
github.com/vaibhavgupta9877/ruprizzle-orm
```

Regenerate the version line on release so it never goes stale. A stale context block is worse than none — it teaches the model a wrong fact you will then have to correct.

### 25.3 `llms.txt` — the fetchable version of the same thing

Part II §18.3 already schedules `llms.txt`. Its content should be the §25.2 block plus a linked page index. But note the placement problem: `llms.txt` conventionally lives at a *domain* root, and this project's domain root is a GitHub Pages subpath. Publish it in both places:

- `https://vaibhavgupta9877.github.io/ruprizzle-orm/llms.txt` (via the existing `pages.yml` copy step)
- `/llms.txt` in the repo root, so `raw.githubusercontent.com` serves it and the repo file listing exposes it

### 25.4 The disambiguation sentence

One sentence, used verbatim and unchanged everywhere the project is introduced:

> **ruprizzle is a schema-first ORM for Rust — a Prisma-style schema file that generates a typed client, with Drizzle-style SQL transparency and no sidecar binary.**

Consistency is the mechanism. LLMs establish entity facts through repetition of *near-identical* phrasing across sources. Rewriting the pitch for every channel actively slows entity formation. Put this sentence in: the GitHub repo description field, `Cargo.toml` descriptions, the README's first line, `llms.txt`, every directory submission, every forum post, every release note.

---

## 26. Track B — Entity Establishment

Ranked by how much each source contributes to name resolution per hour spent.

| # | Source | Resolution value | Why | Effort |
|---|--------|:----------------:|-----|:------:|
| 1 | **crates.io metadata** | Very high | The canonical package record. Scraped by nearly every code-aware retrieval corpus. Already good — see §27 for the gaps. | S |
| 2 | **docs.rs crate-root rustdoc** | Very high | Auto-published, permanently hosted, high authority, versioned. Currently thin. Biggest underexploited asset. | M |
| 3 | **GitHub repo description + topics** | Very high | The description field is what GitHub search, API consumers, and most scrapers read *first*. Topics drive GitHub's own discovery. Unverified — see task 29.1. | S |
| 4 | **lib.rs listing** | High | Ranks for crate queries, shows category position, editorially curated. | S |
| 5 | **This Week in Rust** | High | Every issue is archived, crawled, and heavily represented in Rust-related retrieval. A single TWIR mention does more for name resolution than a month of docs work. | S |
| 6 | **awesome-rust / Rust-LibHunt / libs.tech** | High | Curated lists are disproportionately weighted as authority signals. | S |
| 7 | **Reddit r/rust** | High | ~1.8% of ChatGPT citations, and much higher for developer queries specifically. | M, ongoing |
| 8 | **Stack Overflow** | Medium-high | Long-lived, heavily crawled. Answer real questions; mention ruprizzle only where genuinely correct. | M, ongoing |
| 9 | **Hacker News** | Medium-high | Threads persist and get cited for years. | S, event-based |
| 10 | **Blog posts / guest posts** | Medium | Independent third-party mentions carry more entity weight than self-published. | L |
| 11 | **YouTube** | Medium | Frequently cited by Google AI Overviews. | M |
| 12 | **Wikipedia** | Very high *if achievable* | Not achievable yet — the project fails notability and an article would be deleted. Do not attempt. | — |

**The compounding rule:** entity resolution is a threshold effect, not a gradient. Nothing appears to work for months, then the name starts resolving correctly across all platforms at once. Do not judge Track B on 30-day results.

---

## 27. Crate Metadata Audit (2026-08-17)

Audited all eight `crates/*/Cargo.toml`. **This is in better shape than Part II §15.2 D10 assumed** — every crate has a description, keywords, categories, an explicit `documentation` URL, a README, and workspace-inherited `repository`/`homepage`. D10 is downgraded from P1 to P2. The remaining gaps are specific:

| # | Finding | Severity |
|---|---------|:--------:|
| M1 | **`ruprizzle` (runtime) keywords omit MySQL.** Currently `["orm", "sql", "database", "postgres", "sqlite"]`. MySQL/MariaDB shipped and is a headline feature, but is not searchable on crates.io. Keywords are capped at 5, so this is a swap decision — recommend dropping `"sql"` (subsumed by `"database"`) for `"mysql"`. Same issue in `ruprizzle-dialect`. | **P1** |
| M2 | **Runtime description omits the differentiators.** `"A schema-first ORM for Rust: typed queries, relations, and automatic migrations"` is accurate but generic — it does not say *no sidecar*, *SQL transparency*, or which databases. This string is what appears in crates.io search results and in most LLM answers about the crate. It is the highest-leverage 100 characters in the project. | **P1** |
| M3 | **`ruprizzle-testkit` description says "(not published)"** yet the crate sets `documentation = "https://docs.rs/ruprizzle-testkit"`. Either it publishes or it doesn't — reconcile, because a contradictory record is exactly what produces confused LLM answers. | **P2** |
| M4 | **Keyword strings are near-identical across all 8 crates** (`orm`, `sql`, `database` + 2). Fine for the sub-crates, but it means crates.io keyword pages surface eight ruprizzle crates and a user cannot tell which one to install. The runtime crate should be unmistakably the entry point. | **P2** |
| M5 | **GitHub repo description and topics unverified** — no `gh` CLI available in this environment. Must be checked manually. | **P1** |

---

## 28. Measuring Name Resolution

Part II §20 measures *citation rate*. Track B needs its own, simpler scoreboard, because it answers a different question.

**The zero-context test.** Monthly, in a **fresh session with no prior context and no links**, ask each platform exactly:

1. `What is ruprizzle?`
2. `What is the ruprizzle Rust crate?`
3. `How do I install ruprizzle?`
4. `Does ruprizzle support MySQL?`
5. `Is ruprizzle related to Drizzle?`

Score each response:

| Score | Meaning |
|:-----:|---------|
| **0** | Doesn't know the name. |
| **1** | **Misresolves** — answers about Drizzle/drizzle-orm/drizzle-rs, or hallucinates. *Worse than 0.* |
| **2** | Knows it's a Rust ORM but details are wrong or vague. |
| **3** | Correct definition, correct install, correct databases, correct status. |

Track the score per platform per month across ChatGPT, Claude, Perplexity, Gemini, Copilot, and Google AI Overviews.

| Milestone | Target | Meaning |
|-----------|--------|---------|
| **Baseline (now)** | Measure before changing anything | Assume mostly 0s and 1s. Must be recorded to prove anything later. |
| **90 days** | No platform scoring 1 (misresolution eliminated) | The Drizzle-confusion risk is contained. |
| **6 months** | Average ≥ 2 across platforms | The name resolves to the right entity. |
| **12 months** | Average ≥ 3 on at least 3 platforms | **You stop needing to paste links.** This is the actual goal. |

Score-1 responses are the priority to eliminate. A model confidently describing Drizzle when asked about ruprizzle does active damage — it produces wrong answers for anyone else who asks, and it reinforces the wrong association in future retrieval.

---

## 29. Execution Tasks — Part III

### 29.1 Immediate — Track A (this week, ~half a day)

- [~] **[P0]** Record the §28 zero-context baseline across all 6 platforms *before* changing anything. This is the only chance to capture it. — **Instrument ready, run pending.** `ProjectPlan/NameResolutionBaseline.md` holds the protocol, the 5 verbatim questions, the rubric, the correctness key, and the empty grid. Requires a human with accounts on ChatGPT, Claude, Perplexity, Gemini, Copilot, and Google AI Overviews; must be filled in *before* the repo-metadata changes below go live.
- [ ] **[P0]** Ratify the §25.4 disambiguation sentence as the single canonical description. Do not vary it per channel.
- [ ] **[P0]** Set the **GitHub repo description** field to the disambiguation sentence (verify current value — unverified, M5).
- [ ] **[P0]** Set **GitHub repo topics**: `rust`, `orm`, `database`, `postgres`, `mysql`, `sqlite`, `sqlx`, `prisma`, `schema-first`, `migrations`, `type-safe`. Verify what's currently set first.
- [ ] **[P0]** Confirm the repo's **homepage field** points somewhere useful (docs site or crates.io), not blank.
- [ ] **[P0]** Write `LLM_CONTEXT.md` at the repo root from the §25.2 draft.
- [ ] **[P0]** Audit the README against the one-link test (§25.1): if this URL is the *only* thing pasted, can a model correctly state what ruprizzle is, its databases, its differentiators, and its status? Fix whatever fails.
- [ ] **[P0]** Add an explicit "Not related to Drizzle / drizzle-orm / drizzle-rs" disambiguation line to the README, `LLM_CONTEXT.md`, and `llms.txt`. This directly targets the score-1 failure mode.
- [ ] **[P1]** Publish `llms.txt` to *both* the repo root and the Pages site root (extend the `cp` step in `pages.yml`).
- [ ] **[P1]** Add an `xtask` release step regenerating the version string in `LLM_CONTEXT.md` and `llms.txt`.

### 29.2 Crate metadata (next release)

- [ ] **[P1]** M1 — Add `mysql` to `ruprizzle` and `ruprizzle-dialect` keywords; drop `sql` to stay within the 5-keyword cap.
- [ ] **[P1]** M2 — Rewrite the `ruprizzle` runtime description to carry a differentiator and the database list within the 100-char budget.
- [ ] **[P2]** M3 — Reconcile `ruprizzle-testkit`: either drop "(not published)" or drop the docs.rs `documentation` URL.
- [ ] **[P2]** M4 — Differentiate sub-crate keywords so the runtime crate is unmistakably the entry point; add a "you probably want the `ruprizzle` crate" line to every sub-crate README.
- [ ] **[P1]** Write real crate-root rustdoc (`//!`) for `ruprizzle`: the disambiguation sentence, a runnable quickstart, the database list, and links out. docs.rs is surface #3 and currently the most underexploited.
- [ ] **[P2]** Verify `docs.rs` builds all features correctly; add `[package.metadata.docs.rs]` with `all-features = true` if not already present.

### 29.3 Track B — entity seeding (Weeks 1–12)

- [ ] **[P0]** Submit to **lib.rs**, **libs.tech**, **Rust-LibHunt**, and **awesome-rust**. Highest resolution-value-per-hour items in this document.
- [ ] **[P0]** Submit `0.4.0-beta.2` to **This Week in Rust**, using the disambiguation sentence verbatim.
- [ ] **[P1]** Post to **r/rust** — an honest beta announcement leading with the sidecar-free and SQL-transparency differentiators, and with the Drizzle disambiguation stated up front.
- [ ] **[P1]** Announce on the **Rust users forum** "Showcase" category.
- [ ] **[P1]** Answer 5+ existing Rust-ORM questions per week on **r/rust** and **Stack Overflow**, with maintainer disclosure and no pitch.
- [ ] **[P1]** Ensure **GitHub Discussions** is enabled — discussion threads are crawled and are a cheap source of independent-looking co-occurrence.
- [ ] **[P2]** Record a 5-minute screencast; title it with the exact disambiguation phrasing.
- [ ] **[P2]** Submit a **Show HN** when `v0.4` stabilises.
- [ ] **[P2]** Target 3 independent third-party mentions (roundups, comparison posts, newsletters).
- [ ] **[P3]** Revisit **Wikipedia** only after genuine independent coverage exists. Do not attempt before then.

### 29.4 Ongoing

- [ ] **[P1]** Run the §28 zero-context test monthly across all 6 platforms; log scores.
- [ ] **[P1]** Treat any score-1 (misresolution) as a **bug with an owner**, not a marketing metric.
- [ ] **[P1]** Re-run §27 crate metadata audit each release.
- [ ] **[P2]** Fix the `pages.yml` lag: the docs site deploys only from `main`/`master`, so the published site trails `dev-v0-2`. Either accept it explicitly or deploy a preview from the active branch.
- [ ] **[P2]** Re-verify the one-link test (§25.1) after every README change.

---

## 30. Revised Priority Order

Parts II and III merged, ordered by return per hour. This supersedes the sprint ordering in §22.

| Priority | Work | Part | Effort | Why first |
|:--------:|------|:----:|:------:|-----------|
| **1** | §28 baseline measurement | III | 1h | One-time chance; everything else is unassessable without it. |
| **2** | Repo description + topics + `LLM_CONTEXT.md` + README disambiguation | III | 3h | Directly attacks the stated problem; the Drizzle-misresolution fix is nearly free. |
| **3** | Crate metadata fixes (M1, M2) | III | 1h | The 100 highest-leverage characters in the project. |
| **4** | Directory submissions (lib.rs, awesome-rust, TWIR, libs.tech) | III | 3h | Highest entity-resolution value per hour that exists. |
| **5** | Sitemap / duplicate-docs fix (D1, D2) | II | 1d | Currently broken; 7 of 11 sitemap URLs are dead. |
| **6** | Crate-root rustdoc for `ruprizzle` | III | 4h | docs.rs is surface #3 and near-empty. |
| **7** | `llms.txt` in both locations | II/III | 2h | Cheap, compounding. |
| **8** | r/rust + Rust forum announcements | III | 3h | First independent co-occurrence signals. |
| **9** | Structured data, freshness footers (D3, D5, D6) | II | 1d | Real but slower-acting. |
| **10** | Comparison + benchmark content assets (C1–C3) | II | weeks | Highest ceiling, longest lead time. Start once 1–9 are done. |

Items 1–4 total roughly **one working day** and address the stated problem directly. Everything from item 5 onward compounds.

---

*End of marketing plan (Parts I, II, and III).*

*Next action: run the §28 zero-context baseline today, then execute priority items 2–4 — repo metadata, crate metadata, and directory submissions. Roughly one day of work to stop needing to paste links as often, and to start the entity clock running.*
