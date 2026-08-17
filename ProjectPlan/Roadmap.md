# Master 12-Week Roadmap: T3-for-Rust Stack (Command Center)

**Start:** Monday, Week 1  
**Ship:** Friday, Week 12  
**Team:** Vaibhav Gupta (architecture, design docs) + Vaibhav Gupta (implementation)  
**Status:** P0–P7 and P2-4 shipped; the 12-week roadmap is superseded by `v1/PathToStableV1.md` for 1.0 work.

---

## Weekly Sync Structure

### Vaibhav Gupta's Job (10–15 hours/week)

1. **Design docs before Vaibhav Gupta starts** (MON 9am before Vaibhav Gupta begins)
   - Architecture decision
   - API surface (copy-paste examples)
   - Integration points
   - Success criteria

2. **Daily async review** (MON–FRI 5pm)
   - Does code match design?
   - Are tests passing?
   - Spotted issues?

3. **Weekly pivot meeting** (FRI 3pm)
   - Review what shipped
   - Adjust next week if needed
   - Unblock if Vaibhav Gupta is stuck

### Vaibhav Gupta's Job (20–30 hours/week)

1. **Implement exactly to spec** (no improvisation on architecture)
2. **Daily test progress** (no surprises Friday)
3. **Weekly demo** (show what works, what doesn't)
4. **Ask questions early** (don't code yourself into a corner)

### You (5–10 hours/week)

1. **Approve architecture** (MON design review)
2. **Checkpoint decisions** (WED mid-week go/no-go)
3. **FRI 3pm sync** with Vaibhav Gupta + Vaibhav Gupta (30 min)
4. **Unblock if needed** (decision authority)

---

## The 12-Week Timeline

### Week 1: ORM Foundation (rustorm)

**Design:** Already done (WEEK1-ORM-DESIGN-FINAL.md)

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Final handoff | Scaffold + Pest grammar study | None | — |
| **TUE** | — | Pest parser day 1 | Async review | — |
| **WED** | Grammar review | Pest parser day 2 | 30min (grammar approved?) | YES if parser can't parse sample schema |
| **THU** | AST review | PostgresDialect + entity codegen | Async | YES if no Rust codegen output |
| **FRI** | Code review | Migration SQL + CLI working | 30min demo | YES if can't generate both entities + migrations |

**Deliverable:** `rustorm` crate with Pest parser, PostgresDialect, working codegen.  
**Success:** Parse schema.rustorm → generate entities.rs + 001_*.sql + query_builders.rs for Postgres.

---

### Week 2: Auth Architecture + ORM Polish (rustauth sketch)

**Vaibhav Gupta's task:** Auth architecture doc (3–4 pages)

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Write auth design | SQLite codegen (dial trait exists) | None | — |
| **TUE–WED** | — | SQLite tests + fix codegen bugs | Async | YES if SQLite codegen >20% broken |
| **THU** | Auth design review | ORM bug fixes / cleanup | 30min | — |
| **FRI** | — | Package rustorm for pub release | 30min demo | YES if ORM isn't releasable |

**Deliverable:** rustorm works for Postgres + SQLite. Auth design doc ready for Week 3 impl.

**Checkpoint (FRI EOD):**
- [ ] rustorm generates working SQL for both DBs
- [ ] Entities compile without warnings
- [ ] SelectBuilder + InsertBuilder work
- [ ] If not: **ABORT ORM, adopt SeaORM, recover 1 week**

---

### Week 3: Auth Implementation + RPC Sketch (rustauth + rRPC)

**Vaibhav Gupta tasks:**
- Auth implementation doc (OAuth flow, session handling)
- rRPC macro design

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Auth + RPC design | Start auth Axum integration | None | — |
| **TUE–WED** | — | Email/password + JWT | Async | YES if OAuth client broken |
| **THU** | Review OAuth impl | Google OAuth integration | 30min | — |
| **FRI** | — | End-to-end auth test (login → session) | 30min demo | YES if OAuth isn't working |

**Deliverable:** Auth works: email/password + Google OAuth, session creation + validation.

**Checkpoint (FRI EOD):**
- [ ] Email/password signup + login works
- [ ] Google OAuth callback creates user + session
- [ ] Session is validated on subsequent requests
- [ ] If not: **Use existing crate + wrap, move on**

---

### Week 4: ORM + Auth Integration + rRPC Start

**Vaibhav Gupta tasks:**
- rRPC macro design (finalize)
- Reference app schema design

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | rRPC design doc | Scaffold rRPC macro | None | — |
| **TUE–WED** | — | RPC macro + client codegen | Async | — |
| **THU** | RPC review | rRPC tests | 30min | — |
| **FRI** | Ref app schema design | Server function ↔ client call works | 30min | YES if RPC macro broken |

**Deliverable:** rRPC macro works: `#[server] async fn foo(x: T) -> R` generates client code.

---

### Week 5: Components Foundation + Reference App Bootstrap

**Vaibhav Gupta tasks:**
- shadcn-dioxus component API design
- Reference app architecture doc

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Component design + app schema | Bootstrap Dioxus + Tailwind | None | — |
| **TUE–WED** | — | Auth UI (login, signup) | Async | — |
| **THU** | Component lib review | Dashboard page stub | 30min | — |
| **FRI** | — | Integrate rustauth + rustorm with app | 30min demo | YES if app doesn't build |

**Deliverable:** Reference app boots, auth flows work (Google OAuth redirects correctly).

---

### Week 6: Components Shipping + App CRUD

**Vaibhav Gupta tasks:**
- Component installer design (dx components add)
- Test reference app schema

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Installer + component lib design | Impl Button, Input components | None | — |
| **TUE–WED** | — | Impl Dialog, Dropdown, Card | Async | — |
| **THU** | Review components | Component tests + copy-paste works | 30min | — |
| **FRI** | — | Build CRUD screen (projects: add/edit/delete) | 30min demo | YES if can't copy-paste component into app |

**Deliverable:** 6+ components work, copy-paste installation works, reference app has working CRUD.

**Checkpoint (FRI EOD):**
- [ ] 6+ components are usable
- [ ] Installer (dx components add) works
- [ ] CRUD screen works (no auth bugs, data persists)
- [ ] If <6 components: **OK, ship without custom, full reference app**

---

### Week 7: App Features + Component Polish

**Vaibhav Gupta tasks:**
- Reference app features (invites, roles, etc.)
- Documentation plan

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Document features todo | Invite flow (send email, accept) | None | — |
| **TUE–WED** | — | Role-based access control stubs | Async | — |
| **THU** | Feature review | Workspace/org management | 30min | — |
| **FRI** | — | Refine UI polish | 30min demo | YES if invite flow broken |

**Deliverable:** Reference app is feature-complete for demo (users, workspaces, basic CRUD, invite flow).

---

### Week 8: Deployment + Performance

**Vaibhav Gupta tasks:**
- Deployment strategy doc
- Performance baseline measurement

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Deploy plan (web SSR + desktop) | Build web bundle for Netlify | None | — |
| **TUE–WED** | — | Configure Dioxus SSR + HTML output | Async | YES if can't build for web |
| **THU** | Performance review | Desktop binary (Windows + Linux) | 30min | — |
| **FRI** | — | Deploy to Netlify, test live | 30min demo | YES if web deploy broken |

**Deliverable:** Reference app deployed to web + desktop binaries exist.

---

### Week 9: Additional Components + Hardening

**Vaibhav Gupta tasks:**
- Component roadmap (what's 0.2)
- Known limitations doc

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Known limitations + roadmap | Add 2–4 more components | None | — |
| **TUE–WED** | — | Fix bugs found in testing | Async | — |
| **THU** | Review known issues | Performance profiling | 30min | — |
| **FRI** | — | UI polish pass | 30min demo | YES if <10 components total |

**Deliverable:** 10+ components, reference app fully functional, known limitations documented.

---

### Week 10: Documentation Starts

**Vaibhav Gupta tasks:**
- Write architectural walkthrough (5 pages)
- Write quickstart guide
- Write API reference template

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Architecture walkthrough | Integration test suite | None | — |
| **TUE–WED** | Quickstart + API docs | Tests for all crates | Async | — |
| **THU** | Review + finish docs | Fix test failures | 30min | — |
| **FRI** | Polish docs | Final polishing | 30min | YES if docs are still empty |

**Deliverable:** Comprehensive docs for all three crates + reference app.

---

### Week 11: Final Push + Polish

**Vaibhav Gupta tasks:**
- Community engagement plan
- Demo script / launch narrative

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Launch narrative doc | Security audit (basic, internal) | None | — |
| **TUE–WED** | — | Fix found bugs | Async | — |
| **THU** | Demo + marketing prep | Final tests on real machines | 30min | — |
| **FRI** | — | Final fixes + release prep | 30min | YES if critical bugs remain |

**Deliverable:** All repos are production-ready (README, examples, working).

---

### Week 12: Launch

**Vaibhav Gupta tasks:**
- Finalize marketing copy
- Coordinated GitHub release

| Day | Vaibhav Gupta | Vaibhav Gupta | Sync | Stop? |
|---|---|---|---|---|
| **MON** | Release checklist | Final video demo (2–3 min) | None | — |
| **TUE–WED** | — | Polish video, test deploys | Async | — |
| **THU** | Final review | GitHub release + tags | 30min | — |
| **FRI** | Publish + tweet | Monitor for issues | 30min | SHIP 🚀 |

**Deliverable:** Three crates on crates.io, GitHub repos trending, reference app live.

---

## Critical Path: Kill Criteria

### Red Flags That Trigger Pivots

#### Week 1 (ORM)

**Wednesday EOD:** Parser doesn't parse sample schema  
→ **Action:** Use Nom or regex; keep DbDialect abstraction  
→ **Cost:** 2 days architecture adjustment; proceed

**Friday EOD:** Codegen doesn't produce valid Rust  
→ **Action:** Fix codegen, or fall back to SeaORM wrapper  
→ **Cost:** Lose rustorm uniqueness; keep DX abstraction, partner with SeaORM

#### Week 2 (ORM Polish + Auth Design)

**Friday EOD:** SQLite codegen broken (>30% failures)  
→ **Action:** OK to delay to Week 3; focus Postgres stability  
→ **Cost:** Launch without SQLite, add week 2 of month 2

#### Week 3 (Auth)

**Friday EOD:** OAuth flow doesn't work  
→ **Action:** Use `oauth2-passkey-axum` crate, wrap it  
→ **Cost:** Less control; faster shipping; acceptable tradeoff

**Friday EOD:** rRPC macro broken  
→ **Action:** Use Dioxus server functions directly  
→ **Cost:** Lose rRPC differentiation; use Dioxus as-is

#### Week 6 (Components)

**Friday EOD:** <6 components are working  
→ **Action:** Ship without shadcn-dioxus; use Tailwind + semantic HTML  
→ **Cost:** Less polish; still shipable; components are 0.2 scope

#### Week 8 (Deployment)

**Friday EOD:** Can't deploy web to Netlify  
→ **Action:** Deploy to Vercel or self-host  
→ **Cost:** Different DevOps; not deal-breaking

**Friday EOD:** Desktop binary won't build on Windows  
→ **Action:** Ship Windows binary built on CI (GitHub Actions on Windows runner)  
→ **Cost:** Accept binary bloat; test on real machine weekly

#### Week 11 (Final Push)

**Monday EOD:** Security audit finds critical issue  
→ **Action:** Fix immediately; slip week 12 by 3–5 days if needed  
→ **Cost:** Launch delayed to mid-week 12

---

## Must-Have Deliverables at Week 12

### GitHub Repos (3 minimum)

**Repo 1: rustorm**
- [ ] Crate on crates.io
- [ ] README with schema DSL example
- [ ] API docs (rustdoc comments)
- [ ] 5+ example schemas in `examples/`
- [ ] Supports Postgres + SQLite codegen
- [ ] CLI tool works (`cargo rustorm-cli -- generate`)

**Repo 2: rustauth**
- [ ] Crate on crates.io
- [ ] Docs: auth flows, provider setup
- [ ] 2–3 OIDC providers working (Google, GitHub, at least one more)
- [ ] Email/password + JWT
- [ ] Session middleware
- [ ] Example Axum route setup

**Repo 3: shadcn-dioxus**
- [ ] Crate on crates.io
- [ ] Component installer (dx components add)
- [ ] 10+ components (Button, Input, Dialog, Dropdown, Card, Alert, Badge, Skeleton, Tabs, Form)
- [ ] Copy-paste examples for each
- [ ] Tailwind compatible
- [ ] README

**Repo 4: reference-app**
- [ ] Deployed on web (Netlify/Vercel)
- [ ] Desktop binaries for Windows + Linux
- [ ] Uses all three crates (rustorm, rustauth, shadcn-dioxus)
- [ ] Shows full auth flow + CRUD
- [ ] README walkthrough
- [ ] Architecture doc

### Metrics

- [ ] Cold compile time: <3 min
- [ ] Incremental: <40s
- [ ] WASM bundle: <120 KB gzipped
- [ ] Desktop binary: <10 MB
- [ ] Reference app SSR: <200ms time-to-first-byte

### Social

- [ ] GitHub trending (aim for top 50 Rust repos for 1 week)
- [ ] 50+ stars across all repos
- [ ] 5+ real issues/discussions from users
- [ ] Twitter/HackerNews post (optional but good)

---

## Success Narrative (What You're Selling)

**The Story:**

"We built the Rust equivalent of T3: Dioxus (Next.js role) + rustorm (Prisma/Drizzle role) + rustauth (NextAuth role) + shadcn-dioxus (component role). One language end-to-end. Type-safe client↔server. Compiled for web (SSR), desktop, and webview mobile. Better DX than cobbling together SeaORM + custom auth + community components."

**The Proof:**

- Reference app: a real SaaS (auth, CRUD, multi-tenant) built in 12 weeks by one person using Vaibhav Gupta + Vaibhav Gupta
- All crates are beta/alpha but usable today
- Roadmap is honest (what's 0.2, what's not done)
- Community can contribute

**The Position:**

"Not production yet, but first truly integrated full-stack Rust framework. Choose us if you want cohesion + DX + forward momentum. Choose Dioxus/SeaORM if you want stability + ecosystem maturity."

---

## Daily Standup Template (Async, 5 min read)

**For Vaibhav Gupta to post MON–FRI 5pm (or whenever done):**

```
## [Day] [Week] - Standup

**What I did today:**
- [x] Task 1 (completed)
- [x] Task 2 (completed)
- [ ] Task 3 (in progress)

**Status:**
- Blockers: None / [specific issue]
- Tests: X passing, Y failing
- Compile time: [minutes]

**Tomorrow:**
- Task 4
- Task 5

**Questions for Vaibhav Gupta:**
- [If any]
```

**For Vaibhav Gupta to reply (5 min):**

```
## Vaibhav Gupta Review

**Good:**
- [What's working well]

**Adjust:**
- [If architecture needs tweaks]

**Questions back:**
- [If need clarification from Vaibhav Gupta]

**Approval to proceed:** ✅ or 🟡 (discuss) or 🔴 (stop, pivot)
```

---

## The Week 12 Launch Checklist

### Wednesday (Pre-Launch)

- [ ] All tests pass
- [ ] No compiler warnings
- [ ] Documentation is complete + readable
- [ ] Demo video (2–3 min) is recorded
- [ ] GitHub README is polished
- [ ] crates.io metadata is correct (description, keywords, links)

### Thursday (Soft Launch)

- [ ] Publish crates to crates.io
- [ ] Tag repos on GitHub with 0.1-alpha
- [ ] Deploy reference app
- [ ] Share with trusted network (get feedback)
- [ ] Fix any critical bugs found

### Friday (Public Launch)

- [ ] Post on Twitter/HackerNews/Reddit r/rust
- [ ] Post on Rust forum (users.rust-lang.org)
- [ ] Pin README to GitHub orgs
- [ ] Monitor for issues + questions
- [ ] Reply to comments (community engagement)

---

## Risk Summary

| Risk | Severity | Mitigation | Likelihood |
|---|---|---|---|
| **ORM takes 8 weeks** | 🔴 | Pivot to SeaORM wrap by week 2 | 40% |
| **Auth is broken week 3** | 🔴 | Adopt existing crate + wrap | 30% |
| **Component library incomplete** | 🟡 | Ship without; it's 0.2 scope | 60% (acceptable) |
| **Desktop deploy fails week 8** | 🟡 | Accept GitHub Actions CI binaries | 30% |
| **Dioxus ships competing stack** | 🟡 | Ship first + better docs wins | 35% |

**Net risk: MEDIUM-HIGH but manageable. No single blocker kills the ship.**

---

## Weeks 13–16: Month 2 (Iteration)

### Not part of 12-week commitment, but plan for it:

- Week 13: Feedback incorporation + bug fixes
- Week 14: Password reset, email verification, more components
- Week 15: CLI scaffolding tool, better docs
- Week 16: Performance optimization, security audit

---

## Final Checklist Before Monday Start

- [ ] Vaibhav Gupta has access to all design docs
- [ ] Vaibhav Gupta has Pest tutorial link (2 hours prep)
- [ ] Cargo projects are bootstrapped
- [ ] CI/CD pipeline exists (GitHub Actions)
- [ ] Postgres + SQLite available locally (Docker compose)
- [ ] Daily standup channel is set up (Slack/Discord)
- [ ] Weekly FRI 3pm sync is on calendar (30 min)
- [ ] Kill criteria are understood (not just read)

---

## You're Ready. Launch Monday.

This is the most ambitious 12-week sprint you'll do. The upside is significant:
- You're building *the* full-stack Rust framework Rust devs wanted
- It's cohesive + documented + backed by a real reference app
- You're earning credit as the person who made this happen

The downside is manageable:
- Some stuff will break; pivots are planned
- You might ship a few features in 0.2 instead of 0.1
- You'll be working hard (but focused)

**You're not betting the company. You're betting 12 weeks to prove the concept. If it works, you iterate. If it doesn't, you learned fast and pivoted.**

Go ship. 🚀
