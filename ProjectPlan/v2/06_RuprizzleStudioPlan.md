# Plan 06: Ruprizzle Studio — Embedded Pure-Rust Visual Data & Schema Workbench

**Date:** 2026-08-22  
**Author:** Vaibhav Gupta <vaibhavgupta9877@gmail.com>  
**Status:** Ready for Execution  
**Milestone:** v1.5.0 (Phase 2 Headline Feature)  
**Primary Crates:** `crates/cli` (`feature = "studio"`)  
**Tech Stack Baseline:** Axum 0.8.x, Askama 0.12.x (Compile-Time Type-Safe Templates), HTMX 2.x, Alpine.js 3.x, Tailwind CSS (Standalone), Cytoscape.js / SVG Canvas, Tokio 1.44.x

---

## 1. Architectural Analysis: Why Pure-Rust + HTMX Beats React for Studio

When building an embedded developer tool inside a high-performance Rust ORM binary, the frontend architecture choices have profound implications on developer experience, binary size, build complexity, and memory overhead.

### 1.1 Technology Comparison Matrix

| Dimension | React 19 + Vite + Node Stack | Leptos 0.7 (Rust WASM) | **Axum + Askama + HTMX 2.x + Alpine + Tailwind (Chosen)** |
|---|---|---|---|
| **Build Dependencies** | Requires Node.js 20+, npm/pnpm, 500+ npm packages in `node_modules` (~350MB) | Requires `wasm32-unknown-unknown` target, `trunk` / `wasm-bindgen` | **100% Pure Rust (`cargo build`). Zero npm, zero Node.js, zero external toolchains** |
| **Compile-Time Safety** | TypeScript compiler (separate build step) | Rust type safety in WASM | **Askama compile-time template type checking directly in `cargo build`** |
| **Startup Latency** | ~50ms initial load, JS parsing delay | ~30ms WASM instantiation | **<10ms instant HTML streaming from memory** |
| **Memory Footprint** | ~80MB – 150MB (V8 runtime / heavy SPA bundle) | ~25MB – 40MB (WASM engine) | **<12MB total RAM usage** |
| **Asset Size** | ~1.8MB JS bundle (React, Radix, Lucide, Flow) | ~800KB – 1.4MB `.wasm` | **~45KB total gzipped vendor assets (HTMX + Alpine + CSS)** |
| **Maintenance Burden** | High (npm security advisories, React ecosystem churn) | Medium (WASM toolchain versioning) | **Extremely Low (self-contained, immutable vendored JS/CSS)** |
| **Developer Ergonomics** | Context switching between Rust and TS/JSX | Rust RSX syntax | **Rust structs directly bound to Askama HTML templates + declarative `hx-*` attributes** |

### 1.2 The Winning Strategy: Hypermedia-Driven Pure-Rust Architecture

1. **Askama (Compile-Time Type-Checked Templates):** HTML templates are parsed and checked by the Rust compiler at build time. Missing fields, invalid types, or typos fail the `cargo build` directly.
2. **HTMX 2.x (Declarative Server Interaction):** Handles dynamic data table paging, sorting, filtering, row additions, deletions, relation drawers, and SQL sandbox execution via declarative HTML swaps (`hx-get`, `hx-post`, `hx-swap="outerHTML"`, `hx-target="#grid-body"`).
3. **Alpine.js 3.x (Zero-Build Client Micro-Interactivity):** Handles client-side UI states such as cell double-click inline editing toggles, keyboard shortcuts (`Ctrl+Enter` to run query, `Escape` to close drawers/modals), theme toggling, and toast notification dismissal.
4. **Tailwind CSS (Standalone CLI / Vendored Modern CSS):** Generates a sleek, modern, dark-mode design (inspired by Linear / shadcn / Supabase / Vercel design systems) with custom CSS variables, glassmorphic panels, glowing status chips, and typography tokens.
5. **Interactive ERD Visualizer:** Rendered using a lightweight, zero-npm standalone graph engine (Cytoscape.js or pure SVG Canvas with pan/zoom) driven directly by JSON schema endpoints.

```mermaid
graph LR
    subgraph "ruprizzle-cli (Single Rust Executable)"
        CLI["CLI Entry (ruprizzle studio)"] --> Server["Embedded Axum 0.8 Server (127.0.0.1:5555)"]
        Templates["Askama Templates (Compile-Time Type Checked)"] --> Server
        Assets["rust-embed (Vendored HTMX, Alpine, Standalone CSS)"] --> Server
        Server --> IR["Schema IR & Query Engine"]
    end
    Browser["User Web Browser"] <--> |HTML Partials & OOB Swaps (HTMX)| Server
    IR <--> DB["Local / Dev Database (Postgres, SQLite, MySQL)"]
```

---

## 2. Core Studio Capabilities & UI Design

### 2.1 Aesthetic & Visual Standards (Linear / shadcn-Grade Dark Mode)
- **Palette:** Ultra-dark slate zinc (`#09090b` background, `#18181b` card surfaces, `#27272a` borders).
- **Accents:** Electric Indigo (`#6366f1`), Emerald (`#10b981` for active connections and safe diffs), Amber (`#f59e0b` for cautions), Rose (`#f43f5e` for destructive queries/deletions).
- **Typography:** Inter for interface copy, JetBrains Mono / Fira Code for SQL queries, types, and primary keys.
- **Glassmorphism:** Frosted translucent top bar and side panels (`backdrop-blur-md bg-zinc-900/80`).

### 2.2 Table Data Browser & Live Editor
- **Paginated Grid:** Server-rendered paginated rows with instant HTMX swaps (`hx-get="/studio/models/{model}/table?page=2&limit=50"`).
- **Multi-Column Sorting & Filtering:** Declarative dropdown filter builder that submits filter criteria and swaps the table body with sub-millisecond response times.
- **Inline Cell Editing:** Double-clicking a cell switches it to an editable input via Alpine.js (`x-data="{ editing: false }"`). On blur or `Enter`, HTMX issues a `hx-patch="/studio/models/{model}/rows/{id}"` and swaps back the formatted cell.
- **Row Insertion & Deletion:** Slide-over drawer and modal forms for adding records with `@default(...)` previews, plus safe confirmation dialogs for multi-row deletion.

### 2.3 Foreign Key Relation Traversal (Drawer Navigation)
- Foreign key values render as clickable relation badges (e.g. `userId: usr_42` with an arrow icon).
- Clicking a relation badge triggers `hx-get="/studio/relations/{target_model}/{target_id}" hx-target="#relation-drawer"` to slide out a detailed relation inspection pane without navigating away from the current view.
- Supports recursive breadcrumb navigation (`User -> Posts (5) -> Comments (23)`).

### 2.4 Interactive ERD Visualizer
- Visual schema graph rendering all models, column names, scalar types, `@id`, `@unique`, `@default`, and foreign key edges.
- Color-coded cardinality indicators (1:1, 1:N, N:M).
- Interactive pan, zoom, model search/filter, and one-click SVG export.

### 2.5 SQL Sandbox & `.to_sql()` Live Playground
- Interactive SQL editor with query execution against the connected database.
- Side-by-side `.to_sql()` transpiler showing how Ruprizzle query builder AST compiles into PostgreSQL, SQLite, and MySQL dialect SQL.
- Latency breakdown and execution timing metrics display.

### 2.6 Migration Safety Diff & `EXPLAIN (ANALYZE)` Visualizer
- **Migration Safety Diff:** Compares `schema.ruprizzle` against the live database catalog, classifying changes into `SAFE`, `CAUTION`, and `DESTRUCTIVE` with side-by-side visual diffs.
- **EXPLAIN Visualizer:** Tree-node visualization of `EXPLAIN (ANALYZE, BUFFERS)` execution plans, highlighting expensive sequential scans and unindexed foreign key lookups.

### 2.7 Safety Guardrails
- **Read-Only Default:** Database modifications are disabled unless `--allow-writes` is explicitly passed.
- **Production Guardrail:** Automatically blocks database URLs containing `prod`, `production`, or remote hostnames unless `--yes-i-know` is explicitly provided.

---

## 3. Pure-Rust Studio Architecture (`crates/cli/src/studio/`)

```
crates/cli/src/studio/
├── mod.rs                   # Studio CLI entry point, Axum server bootstrap
├── config.rs                # Studio server configuration (port, host, flags)
├── routes.rs                # Axum route definitions and middleware
├── handlers/
│   ├── mod.rs
│   ├── dashboard.rs         # Overview, database statistics, model list
│   ├── table.rs             # Table grid, pagination, filtering, inline edits
│   ├── relations.rs         # Foreign key drawer traversal
│   ├── erd.rs               # ERD graph layout and data generator
│   ├── sandbox.rs           # SQL playground and .to_sql() execution
│   ├── diff.rs              # Schema drift and migration safety diff
│   └── explain.rs           # EXPLAIN (ANALYZE) visualizer
├── templates/               # Askama HTML templates (checked at Rust compile time)
│   ├── base.html            # Master layout, navbar, sidebar, toaster, modals
│   ├── dashboard.html       # Overview dashboard
│   ├── table/
│   │   ├── view.html        # Table page container
│   │   ├── grid.html        # Table header & rows partial (for HTMX swaps)
│   │   ├── row.html         # Single row partial
│   │   ├── cell.html        # Single cell partial (read & edit modes)
│   │   └── filter_bar.html  # Dynamic filter bar
│   ├── relations/
│   │   └── drawer.html      # Slide-out relation inspector
│   ├── erd/
│   │   └── view.html        # ERD graph container & layout
│   ├── sandbox/
│   │   └── view.html        # SQL sandbox & execution results
│   ├── diff/
│   │   └── view.html        # Migration safety diff view
│   └── explain/
│       └── view.html        # EXPLAIN execution plan tree
└── assets/                  # Vendored static assets (embedded via rust-embed)
    ├── css/
    │   └── studio.css       # Pre-compiled modern Tailwind CSS (custom dark theme)
    ├── js/
    │   ├── htmx.min.js      # HTMX 2.x (~14KB gzip)
    │   ├── alpine.min.js    # Alpine.js 3.x (~15KB gzip)
    │   └── cytoscape.min.js # Standalone graph layout engine (~40KB gzip)
    └── icons/               # Embedded SVG Lucide icons
```

---

## 4. Endpoint Specification (HTML Partials & JSON APIs)

| Route | Method | Return Type | Description |
|---|---|---|---|
| `/studio` | `GET` | HTML (`dashboard.html`) | Full dashboard layout with model list and stats |
| `/studio/models/:model` | `GET` | HTML (`table/view.html`) | Full table browser view for specified model |
| `/studio/models/:model/table` | `GET` | HTML Partial (`grid.html`) | Filtered/paginated table rows for HTMX swap |
| `/studio/models/:model/rows` | `POST` | HTML Partial (`row.html`) | Insert record and return rendered row for OOB swap |
| `/studio/models/:model/rows/:id/cell` | `PATCH` | HTML Partial (`cell.html`) | Inline cell update with validation |
| `/studio/models/:model/rows/:id` | `DELETE` | HTTP 200 / Empty | Delete row and trigger client row removal |
| `/studio/relations/:model/:id` | `GET` | HTML Partial (`drawer.html`) | Slide-over relation inspection drawer |
| `/studio/erd` | `GET` | HTML (`erd/view.html`) | Interactive ERD diagram view |
| `/studio/erd/data` | `GET` | JSON | Schema node and edge definitions for graph engine |
| `/studio/sandbox` | `GET` | HTML (`sandbox/view.html`) | SQL playground view |
| `/studio/sandbox/execute` | `POST` | HTML Partial | Executes query and renders result table with timing |
| `/studio/sandbox/to_sql` | `POST` | HTML Partial | Compiles query AST to multi-dialect SQL |
| `/studio/diff` | `GET` | HTML (`diff/view.html`) | Migration safety diff view |
| `/studio/explain` | `POST` | HTML Partial (`explain/view.html`) | Renders visual query plan tree |
| `/studio/assets/*` | `GET` | Static Assets | Embedded CSS/JS/SVG served with immutable caching |

---

## 5. Step-by-Step Implementation Tasks

### Task 1: Studio Backend Infrastructure & Templates (`crates/cli`)
- [ ] Add `askama = "0.12"`, `rust-embed = "8.5"`, `opener = "0.7"` behind `feature = "studio"` in `crates/cli/Cargo.toml`.
- [ ] Implement `crates/cli/src/studio/mod.rs`, `config.rs`, and `routes.rs` using Axum 0.8.
- [ ] Implement Askama base layout (`base.html`) with responsive sidebar, dark mode theme tokens, HTMX, and Alpine.js setup.

### Task 2: Data Browser, Grid & Inline Editing
- [ ] Implement `handlers/table.rs` with type-aware pagination, sorting, and dynamic filtering against connected database pools.
- [ ] Implement Askama templates (`table/view.html`, `table/grid.html`, `table/cell.html`) with Alpine.js inline cell edit toggling.
- [ ] Implement row insert modal and deletion confirmation partials with `--allow-writes` guardrail verification.

### Task 3: Foreign Key Traversal & Slide-Out Drawer
- [ ] Implement `handlers/relations.rs` to resolve foreign key lookups and linked relations.
- [ ] Implement `relations/drawer.html` with breadcrumb navigation and nested record inspection.

### Task 4: Interactive ERD Visualizer
- [ ] Implement `handlers/erd.rs` serializing schema models, fields, primary keys, and foreign keys.
- [ ] Implement `erd/view.html` using embedded Cytoscape.js / SVG layout for pan, zoom, search, and SVG export.

### Task 5: SQL Sandbox, Migration Diff & EXPLAIN Plan
- [ ] Implement `handlers/sandbox.rs` with multi-dialect `.to_sql()` transpilation and query execution.
- [ ] Implement `handlers/diff.rs` displaying schema drift with risk classification badges.
- [ ] Implement `handlers/explain.rs` rendering visual query plan execution trees.

### Task 6: CLI Command Integration & Safety Defaults
- [ ] Register `ruprizzle studio` subcommand in `crates/cli/src/main.rs` with flags:
  - `--port <PORT>` (default `5555`)
  - `--host <HOST>` (default `127.0.0.1`)
  - `--allow-writes` (enables insert/update/delete operations)
  - `--yes-i-know` (overrides production URL guardrails)
  - `--no-browser` (disables auto-opening the browser)
- [ ] Integrate automatic browser launch via `opener::open_browser`.

### Task 7: Comprehensive Integration Testing
- [ ] Add `crates/cli/tests/studio_test.rs`:
  - Test Axum endpoints for HTML partial rendering, status codes, and headers.
  - Test read-only guardrail rejection on mutating endpoints when `--allow-writes` is absent.
  - Test production database URL detection and blocking.

---

## 6. Build, Verification & Testing Workflow

```powershell
# 1. Build CLI with embedded studio (Pure Rust, 0 npm/Node required)
cargo build -p ruprizzle-cli --features studio

# 2. Run Studio unit and integration tests
cargo test -p ruprizzle-cli --features studio --test studio_test

# 3. Launch Studio on example schema
cargo run -p ruprizzle-cli --features studio -- studio --schema ./examples/schema.ruprizzle

# 4. Standard mechanical gates
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 7. Definition of Done

1. `ruprizzle studio` compiles **100% via `cargo build`** with **zero Node.js or npm dependencies**.
2. Starts up and opens the browser in **<15ms** with memory consumption **under 15MB RAM**.
3. Live table browsing, multi-column filtering, sorting, inline cell edits, and relation drawer inspection operate smoothly.
4. Interactive ERD graph cleanly displays models, foreign key relationships, and cardinality badges.
5. SQL Sandbox, migration diff, and EXPLAIN plan visualizers render responsive, dark-mode interfaces.
6. Read-only and production URL guardrails rigorously protect user databases.
