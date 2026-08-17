#!/usr/bin/env python3
"""Rigorous cross-ORM SQLite benchmark.

Runs ruprizzle (sqlx + rusqlite), prax-orm, sea-orm, diesel, Drizzle and
Prisma harnesses with warm-up + measurement trials, aggregates statistics,
logs raw data, and updates docs/BenchmarkResults.md with the median numbers.

When BENCH_PG_URL and/or BENCH_MYSQL_URL are set, the ruprizzle harness is
also run against PostgreSQL and/or MySQL and a dedicated section is appended
to the results document.

Percentile (p50/p95/p99) and throughput metrics are reported in
results.json, BENCHMARKS.log and the markdown output.
"""

import concurrent.futures
import json
import math
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
import tomllib
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Dict, List, Tuple, Union

# Number of warm-up trials to discard (JIT / cache warm-up).
WARMUP_TRIALS = int(os.environ.get("BENCH_WARMUP", "1"))
# Number of measured trials.
MEASURE_TRIALS = int(os.environ.get("BENCH_TRIALS", "10"))

# Concurrency levels for the throughput benchmark.
BENCH_CONCURRENCY = [
    int(x.strip())
    for x in os.environ.get("BENCH_CONCURRENCY", "1,10,100").split(",")
    if x.strip()
]
# Maximum duration of a single throughput harness run, in seconds.
BENCH_DURATION_SECONDS = float(os.environ.get("BENCH_DURATION_SECONDS", "5"))

REPO_ROOT = Path(__file__).resolve().parents[2]
NODE_DIR = REPO_ROOT / "local" / "cross-orm-bench" / "node"
RUST_DIR = REPO_ROOT / "local" / "cross-orm-bench" / "rust"
BENCH_DIR = REPO_ROOT / "local" / "cross-orm-bench"
DOCS_PATH = REPO_ROOT / "docs" / "BenchmarkResults.md"
DB_PATH = NODE_DIR / "bench.sqlite3"
RUST_EXE = REPO_ROOT / "target" / "release" / "examples" / "cross_orm_bench.exe"
SEED_PY = BENCH_DIR / "seed.py"

DRIVER_ORDER = [
    "ruprizzle (sqlx)",
    "ruprizzle (rusqlite)",
    "prax",
    "sea-orm",
    "diesel",
    "prisma",
    "drizzle",
]

BACKEND_ENV_VARS = {
    "postgres": "BENCH_PG_URL",
    "mysql": "BENCH_MYSQL_URL",
    "sqlite": "BENCH_SQLITE_PATH",
}


@dataclass
class Harness:
    name: str
    build: Callable[[], None]
    run: Callable[[], List[List[dict]]]
    results_json: str


def run_cmd(
    cmd: Union[str, List[str]],
    cwd: Path,
    env_extra: Dict[str, str] | None = None,
    env_unset: List[str] | None = None,
    shell: bool = False,
) -> subprocess.CompletedProcess:
    env = os.environ.copy()
    if env_unset:
        for key in env_unset:
            env.pop(key, None)
    if env_extra:
        env.update(env_extra)
    # Always give a stable absolute path to the SQLite file.
    env.setdefault("BENCH_SQLITE_PATH", str(DB_PATH))
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        shell=shell,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def build_ruprizzle() -> None:
    print("Building ruprizzle cross-ORM bench binary...")
    run_cmd(
        [
            "cargo",
            "build",
            "--example",
            "cross_orm_bench",
            "-p",
            "ruprizzle",
            "--release",
            "--features",
            "sqlite-rusqlite",
        ],
        cwd=REPO_ROOT,
    )


def build_rust_orm(crate_name: str) -> None:
    crate_dir = RUST_DIR / f"{crate_name}-bench"
    if not crate_dir.exists():
        raise FileNotFoundError(f"Benchmark crate not found: {crate_dir}")
    print(f"Building {crate_name} bench binary...")
    run_cmd(["cargo", "build", "--release"], cwd=crate_dir)


def package_name(crate_dir: Path) -> str:
    with open(crate_dir / "Cargo.toml", "rb") as f:
        return tomllib.load(f)["package"]["name"]


def rust_orm_exe(crate_dir: Path) -> Path:
    return crate_dir / "target" / "release" / f"{package_name(crate_dir)}.exe"


def seed_db() -> None:
    print("Seeding bench.sqlite3...")
    run_cmd("npm run seed", cwd=NODE_DIR, shell=True)
    # Give SQLite a moment to release WAL / file locks.
    time.sleep(0.5)


def seed_backend(backend: str) -> bool:
    """Seed the requested backend, returning True if it was seeded.

    Falls back gracefully (with a printed warning) if the required CLI is
    not installed, so the benchmark run can continue on SQLite.
    """
    if backend == "sqlite":
        seed_db()
        return True

    cli = "psql" if backend == "postgres" else "mysql"
    if not shutil.which(cli):
        print(f"warning: {cli} not found; skipping {backend} backend")
        return False

    print(f"Seeding {backend}...")
    run_cmd([sys.executable, str(SEED_PY), backend], cwd=REPO_ROOT)
    return True


def generate_prisma_client() -> None:
    print("Generating Prisma client...")
    run_cmd("npx prisma generate", cwd=NODE_DIR, shell=True)
    time.sleep(0.5)


def read_json(path: Path) -> List[dict]:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def percentile(values: List[float], p: float) -> float:
    """Return the p-th percentile of `values` using linear interpolation."""
    if not values:
        return 0.0
    values = sorted(values)
    if len(values) == 1:
        return values[0]
    try:
        import numpy as np

        return float(np.percentile(values, p))
    except Exception:
        pass
    try:
        import statistics

        # statistics.quantiles returns cut points for 1..n-1; p% is at index p-1.
        qs = statistics.quantiles(values, n=100, method="inclusive")
        return qs[int(p) - 1]
    except Exception:
        pass
    k = (len(values) - 1) * p / 100.0
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return values[int(k)]
    return values[f] * (c - k) + values[c] * (k - f)


def stats(values: List[float]) -> Dict[str, float]:
    values = sorted(values)
    return {
        "mean": statistics.mean(values),
        "median": statistics.median(values),
        "min": min(values),
        "max": max(values),
        "stdev": statistics.stdev(values) if len(values) > 1 else 0.0,
        "cv": (
            100.0 * statistics.stdev(values) / statistics.mean(values)
            if len(values) > 1 and statistics.mean(values) != 0
            else 0.0
        ),
        "p50": percentile(values, 50.0),
        "p95": percentile(values, 95.0),
        "p99": percentile(values, 99.0),
    }


def run_rust_trials(
    env_extra: Dict[str, str] | None,
    results_json: str,
    label: str,
    *,
    results_dir: Path = NODE_DIR,
    seed_before_each: bool = True,
    env_unset: List[str] | None = None,
) -> List[List[dict]]:
    if seed_before_each:
        seed_db()
    trials: List[List[dict]] = []
    for i in range(WARMUP_TRIALS + MEASURE_TRIALS):
        trial_label = "warmup" if i < WARMUP_TRIALS else f"measure-{i - WARMUP_TRIALS + 1}"
        print(f"  ruprizzle ({label}) {trial_label}")
        run_cmd([str(RUST_EXE)], cwd=REPO_ROOT, env_extra=env_extra, env_unset=env_unset)
        if i >= WARMUP_TRIALS:
            trials.append(read_json(results_dir / results_json))
        if i < WARMUP_TRIALS + MEASURE_TRIALS - 1:
            time.sleep(0.2)
    return trials


def run_rust_orm_trials(crate_dir: Path, results_json: str) -> List[List[dict]]:
    seed_db()
    crate_name = package_name(crate_dir)
    exe = rust_orm_exe(crate_dir)
    if not exe.exists():
        raise FileNotFoundError(f"Benchmark binary not found: {exe}")
    results_path = crate_dir / results_json
    trials: List[List[dict]] = []
    for i in range(WARMUP_TRIALS + MEASURE_TRIALS):
        label = "warmup" if i < WARMUP_TRIALS else f"measure-{i - WARMUP_TRIALS + 1}"
        print(f"  {crate_name} {label}")
        run_cmd([str(exe)], cwd=crate_dir)
        if i >= WARMUP_TRIALS:
            trials.append(read_json(results_path))
        if i < WARMUP_TRIALS + MEASURE_TRIALS - 1:
            time.sleep(0.2)
    return trials


def run_node_trials(script: str, results_json: str) -> List[List[dict]]:
    seed_db()
    trials: List[List[dict]] = []
    for i in range(WARMUP_TRIALS + MEASURE_TRIALS):
        label = "warmup" if i < WARMUP_TRIALS else f"measure-{i - WARMUP_TRIALS + 1}"
        print(f"  {script} {label}")
        last_err = None
        for attempt in range(3):
            try:
                run_cmd(["node", script], cwd=NODE_DIR)
                break
            except subprocess.CalledProcessError as e:
                last_err = e
                print(f"    {script} failed (attempt {attempt + 1}): {e}")
                time.sleep(1.0)
        else:
            raise last_err
        if i >= WARMUP_TRIALS:
            trials.append(read_json(NODE_DIR / results_json))
        if i < WARMUP_TRIALS + MEASURE_TRIALS - 1:
            time.sleep(0.5)
    return trials


def aggregate(trials: List[List[dict]]) -> Dict[str, Dict[str, float]]:
    by_op: Dict[str, List[float]] = {}
    for trial in trials:
        for row in trial:
            by_op.setdefault(row["operation"], []).append(row["avg_ms"] * 1000.0)  # us
    return {op: stats(v) for op, v in by_op.items()}


def combine(aggregates: Dict[str, Dict[str, Dict[str, float]]]) -> Dict[str, Dict[str, Dict[str, float]]]:
    all_ops = set()
    for a in aggregates.values():
        all_ops |= set(a.keys())
    combined = {}
    for op in sorted(all_ops):
        combined[op] = {driver: aggregates.get(driver, {}).get(op, {}) for driver in DRIVER_ORDER}
    return combined


def format_table_cell(value: float) -> str:
    if value == 0:
        return "—"
    if value >= 1000:
        return f"{value:,.1f}"
    return f"{value:,.1f}"


def write_raw(
    output: Path,
    all_trials: Dict[str, List[List[dict]]],
    aggregates: Dict[str, Dict[str, Dict[str, float]]] | None = None,
) -> None:
    raw = {
        "meta": {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "warmup_trials": WARMUP_TRIALS,
            "measure_trials": MEASURE_TRIALS,
        },
        "trials": {driver: all_trials.get(driver, []) for driver in DRIVER_ORDER},
    }
    if aggregates:
        raw["aggregates"] = {driver: aggregates.get(driver, {}) for driver in DRIVER_ORDER}
    with open(output, "w", encoding="utf-8") as f:
        json.dump(raw, f, indent=2)


def write_summary(
    output: Path,
    combined: Dict[str, Dict[str, Dict[str, float]]],
    throughput: Dict[str, Dict[str, Dict[str, float]]] | None = None,
) -> None:
    summary = {
        "meta": {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "warmup_trials": WARMUP_TRIALS,
            "measure_trials": MEASURE_TRIALS,
            "concurrency": BENCH_CONCURRENCY,
            "duration_seconds": BENCH_DURATION_SECONDS,
            "units": "us/op",
        },
        "operations": combined,
    }
    if throughput:
        summary["throughput"] = throughput
    with open(output, "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)


def write_log(
    output: Path,
    combined: Dict[str, Dict[str, Dict[str, float]]],
    throughput: Dict[str, Dict[str, Dict[str, float]]] | None = None,
) -> None:
    lines = [
        "Cross-ORM benchmark log",
        "=======================",
        f"Timestamp: {datetime.now(timezone.utc).isoformat()}",
        f"Warm-up trials: {WARMUP_TRIALS}",
        f"Measured trials: {MEASURE_TRIALS}",
        f"Concurrency levels: {BENCH_CONCURRENCY}",
        f"Duration per throughput run: {BENCH_DURATION_SECONDS}s",
        "Units: us/op (microseconds per operation)",
        "",
        "End-to-end results",
        "------------------",
    ]

    end_to_end_ops = [
        "select_by_pk",
        "find_many_1000",
        "find_filtered_ordered",
        "find_filtered_paginated",
        "find_in_list",
        "find_complex_filter",
        "count_filtered",
        "exists_filtered",
        "include_posts",
        "include_author",
        "include_posts_and_comments",
        "include_posts_with_tags",
        "find_popular_posts",
        "prepared_select_by_pk",
        "stream_find_many_1000",
        "bulk_insert_1000",
    ]
    query_construction_ops = [
        "to_sql_select_by_pk",
        "to_sql_select_filter_order",
        "to_sql_select_in_list",
        "to_sql_select_complex_filter",
        "to_sql_select_paginated",
        "to_sql_prepared_select_by_pk",
        "prepared_rebind_select_by_pk",
        "to_sql_conditional_filter",
        "to_sql_select_with_cte",
        "to_sql_select_with_recursive_cte",
        "to_sql_set_union",
        "to_sql_select_with_join",
        "to_sql_select_exists_subquery",
        "to_sql_select_in_subquery",
        "to_sql_nested_insert",
        "to_sql_nested_update",
    ]

    # End-to-end table
    name_width = 28
    col_width = 14
    header = f"{'Operation':<{name_width}}" + "".join(
        f" {d:>{col_width}}" for d in DRIVER_ORDER
    )
    lines.append(header)
    lines.append("-" * len(header))
    for op in end_to_end_ops:
        row = combined.get(op, {})
        vals = [format_table_cell(row.get(driver, {}).get("median", 0.0)) for driver in DRIVER_ORDER]
        lines.append(f"{op:<{name_width}}" + "".join(f" {v:>{col_width}}" for v in vals))

    # Query construction table
    lines.extend(["", "Query construction (no I/O)", "---------------------------"])
    header2 = f"{'Operation':<{name_width}}" + "".join(
        f" {d:>{col_width}}" for d in DRIVER_ORDER
    )
    lines.append(header2)
    lines.append("-" * len(header2))
    for op in query_construction_ops:
        row = combined.get(op, {})
        vals = [format_table_cell(row.get(driver, {}).get("median", 0.0)) for driver in DRIVER_ORDER]
        lines.append(f"{op:<{name_width}}" + "".join(f" {v:>{col_width}}" for v in vals))

    # Percentiles for ruprizzle (sqlx)
    lines.extend(["", "Percentiles (ruprizzle sqlx)", "----------------------------"])
    lines.append(
        f"{'Operation':<{name_width}}"
        f" {'p50':>12} {'p95':>12} {'p99':>12}"
    )
    lines.append("-" * (name_width + 38))
    for op in end_to_end_ops + query_construction_ops:
        row = combined.get(op, {}).get("ruprizzle (sqlx)", {})
        if not row:
            continue
        lines.append(
            f"{op:<{name_width}} {row['p50']:>12.1f} {row['p95']:>12.1f} {row['p99']:>12.1f}"
        )

    # Throughput table
    if throughput:
        lines.extend(["", "Throughput (ops/sec)", "--------------------"])
        lines.append(f"{'Backend':<12} {'Concurrency':>12} {'select_by_pk':>16} {'find_many_1000':>16} {'bulk_insert_1000':>18}")
        lines.append("-" * 80)
        for backend in sorted(throughput.keys()):
            for concurrency in sorted(throughput[backend].keys(), key=int):
                t = throughput[backend][concurrency]
                lines.append(
                    f"{backend:<12} {concurrency:>12} "
                    f"{t.get('select_by_pk', 0.0):>16.1f} "
                    f"{t.get('find_many_1000', 0.0):>16.1f} "
                    f"{t.get('bulk_insert_1000', 0.0):>18.1f}"
                )

    lines.extend(["", "Per-driver, per-operation statistics", "------------------------------------"])
    for driver in DRIVER_ORDER:
        lines.append(f"\n{driver}")
        lines.append(
            f"{'Operation':<{name_width}}"
            f" {'mean':>12} {'median':>12} {'p50':>12} {'p95':>12} {'p99':>12}"
            f" {'min':>12} {'max':>12} {'stdev':>12} {'CV%':>8}"
        )
        lines.append("-" * 130)
        for op in end_to_end_ops + query_construction_ops:
            row = combined.get(op, {}).get(driver, {})
            if not row:
                continue
            lines.append(
                f"{op:<{name_width}} {row['mean']:>12.1f} {row['median']:>12.1f} "
                f"{row['p50']:>12.1f} {row['p95']:>12.1f} {row['p99']:>12.1f} "
                f"{row['min']:>12.1f} {row['max']:>12.1f} {row['stdev']:>12.1f} {row['cv']:>8.1f}"
            )

    with open(output, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


def write_markdown(
    path: Path,
    combined: Dict[str, Dict[str, Dict[str, float]]],
    throughput: Dict[str, Dict[str, Dict[str, float]]] | None = None,
) -> None:
    """Append a new timestamped benchmark-run section to the docs file.

    Previous runs are preserved, so the file becomes a historical record.
    """

    def m(op: str, driver: str) -> str:
        row = combined.get(op, {}).get(driver, {})
        v = row.get("median", 0.0)
        if v == 0:
            return "—"
        return f"{v:,.1f}"

    def p(driver: str, op: str, key: str) -> str:
        row = combined.get(op, {}).get(driver, {})
        v = row.get(key, 0.0)
        if v == 0:
            return "—"
        return f"{v:,.1f}"

    end_to_end_ops = [
        "select_by_pk",
        "find_many_1000",
        "find_filtered_ordered",
        "find_filtered_paginated",
        "find_in_list",
        "find_complex_filter",
        "count_filtered",
        "exists_filtered",
        "include_posts",
        "include_author",
        "include_posts_and_comments",
        "include_posts_with_tags",
        "find_popular_posts",
        "prepared_select_by_pk",
        "stream_find_many_1000",
        "bulk_insert_1000",
    ]
    query_construction_ops = [
        "to_sql_select_by_pk",
        "to_sql_select_filter_order",
        "to_sql_select_in_list",
        "to_sql_select_complex_filter",
        "to_sql_select_paginated",
        "to_sql_prepared_select_by_pk",
        "prepared_rebind_select_by_pk",
        "to_sql_conditional_filter",
        "to_sql_select_with_cte",
        "to_sql_select_with_recursive_cte",
        "to_sql_set_union",
        "to_sql_select_with_join",
        "to_sql_select_exists_subquery",
        "to_sql_select_in_subquery",
        "to_sql_nested_insert",
        "to_sql_nested_update",
    ]

    header = "| Operation | " + " | ".join(DRIVER_ORDER) + " |"
    separator = "|" + "---|" * (len(DRIVER_ORDER) + 1)

    def make_table(ops: List[str]) -> str:
        rows = [header, separator]
        for op in ops:
            cells = " | ".join(m(op, d) for d in DRIVER_ORDER)
            rows.append(f"| `{op}` | {cells} |")
        return "\n".join(rows)

    end_to_end_table = make_table(end_to_end_ops)
    query_construction_table = make_table(query_construction_ops)

    percentile_header = "| Operation | p50 | p95 | p99 |"
    percentile_separator = "|---|---|---|---|"

    def make_percentile_table(ops: List[str], driver: str) -> str:
        rows = [percentile_header, percentile_separator]
        for op in ops:
            rows.append(
                f"| `{op}` | {p(driver, op, 'p50')} | {p(driver, op, 'p95')} | {p(driver, op, 'p99')} |"
            )
        return "\n".join(rows)

    throughput_table = ""
    if throughput:
        throughput_rows = ["| Backend | Concurrency | select_by_pk | find_many_1000 | bulk_insert_1000 |"]
        throughput_rows.append("|---|---|---|---|---|")
        for backend in sorted(throughput.keys()):
            for concurrency in sorted(throughput[backend].keys(), key=int):
                t = throughput[backend][concurrency]
                throughput_rows.append(
                    f"| {backend} | {concurrency} | "
                    f"{t.get('select_by_pk', 0.0):,.1f} | "
                    f"{t.get('find_many_1000', 0.0):,.1f} | "
                    f"{t.get('bulk_insert_1000', 0.0):,.1f} |"
                )
        throughput_table = "\n".join(throughput_rows)

    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    section = [
        "",
        f"## Benchmark run: {timestamp}",
        "",
        "### Environment",
        "",
        f"- **Warm-up trials:** {WARMUP_TRIALS}",
        f"- **Measured trials:** {MEASURE_TRIALS}",
        f"- **Concurrency levels:** {BENCH_CONCURRENCY}",
        f"- **Duration per throughput run:** {BENCH_DURATION_SECONDS}s",
        "- **Dataset:**",
        "  - 1,000 users",
        "  - 20 categories",
        "  - 10,000 posts",
        "  - 50,000 comments",
        "  - 100 tags",
        "  - 30,000 post_tags",
        "  - 5,000 followers",
        "  - 20,000 likes",
        "",
        "### End-to-end results",
        "",
        "All times are microseconds per operation (lower is better).",
        "",
        end_to_end_table,
        "",
        "### Query construction (no I/O)",
        "",
        query_construction_table,
        "",
        "### Latency percentiles (ruprizzle sqlx)",
        "",
        make_percentile_table(end_to_end_ops + query_construction_ops, "ruprizzle (sqlx)"),
        "",
    ]
    if throughput_table:
        section.extend(["### Throughput (ops/sec)", "", throughput_table, ""])

    # If the docs file does not yet exist, create a minimal header.
    if not path.exists():
        path.write_text("# Cross-ORM benchmark results\n\n", encoding="utf-8")

    with open(path, "a", encoding="utf-8") as f:
        f.write("\n".join(section))


def write_backend_markdown(
    path: Path,
    backend: str,
    label: str,
    combined: Dict[str, Dict[str, float]],
    throughput: Dict[str, Dict[str, float]] | None = None,
) -> None:
    """Append a backend-specific (Postgres/MySQL) section to the docs file."""

    def m(op: str) -> str:
        row = combined.get(op, {})
        v = row.get("median", 0.0)
        if v == 0:
            return "—"
        return f"{v:,.1f}"

    def pct(op: str, key: str) -> str:
        row = combined.get(op, {})
        v = row.get(key, 0.0)
        if v == 0:
            return "—"
        return f"{v:,.1f}"

    end_to_end_ops = [
        "select_by_pk",
        "find_many_1000",
        "find_filtered_ordered",
        "find_filtered_paginated",
        "find_in_list",
        "find_complex_filter",
        "count_filtered",
        "exists_filtered",
        "include_posts",
        "include_author",
        "include_posts_and_comments",
        "include_posts_with_tags",
        "find_popular_posts",
        "prepared_select_by_pk",
        "stream_find_many_1000",
        "bulk_insert_1000",
    ]
    query_construction_ops = [
        "to_sql_select_by_pk",
        "to_sql_select_filter_order",
        "to_sql_select_in_list",
        "to_sql_select_complex_filter",
        "to_sql_select_paginated",
        "to_sql_prepared_select_by_pk",
        "prepared_rebind_select_by_pk",
        "to_sql_conditional_filter",
        "to_sql_select_with_cte",
        "to_sql_select_with_recursive_cte",
        "to_sql_set_union",
        "to_sql_select_with_join",
        "to_sql_select_exists_subquery",
        "to_sql_select_in_subquery",
        "to_sql_nested_insert",
        "to_sql_nested_update",
    ]

    header = f"| Operation | {label} |"
    separator = "|---|---|"

    def make_table(ops: List[str]) -> str:
        rows = [header, separator]
        for op in ops:
            rows.append(f"| `{op}` | {m(op)} |")
        return "\n".join(rows)

    percentile_header = "| Operation | p50 | p95 | p99 |"
    percentile_separator = "|---|---|---|---|"

    def make_percentile_table(ops: List[str]) -> str:
        rows = [percentile_header, percentile_separator]
        for op in ops:
            rows.append(f"| `{op}` | {pct(op, 'p50')} | {pct(op, 'p95')} | {pct(op, 'p99')} |")
        return "\n".join(rows)

    throughput_table = ""
    if throughput:
        throughput_rows = ["| Concurrency | select_by_pk | find_many_1000 | bulk_insert_1000 |"]
        throughput_rows.append("|---|---|---|---|")
        for concurrency in sorted(throughput.keys(), key=int):
            t = throughput[concurrency]
            throughput_rows.append(
                f"| {concurrency} | "
                f"{t.get('select_by_pk', 0.0):,.1f} | "
                f"{t.get('find_many_1000', 0.0):,.1f} | "
                f"{t.get('bulk_insert_1000', 0.0):,.1f} |"
            )
        throughput_table = "\n".join(throughput_rows)

    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    section = [
        "",
        f"## Benchmark run: {timestamp} ({backend})",
        "",
        f"### Environment ({backend})",
        "",
        f"- **Warm-up trials:** {WARMUP_TRIALS}",
        f"- **Measured trials:** {MEASURE_TRIALS}",
        f"- **Concurrency levels:** {BENCH_CONCURRENCY}",
        f"- **Duration per throughput run:** {BENCH_DURATION_SECONDS}s",
        "- **Dataset:**",
        "  - 1,000 users",
        "  - 20 categories",
        "  - 10,000 posts",
        "  - 50,000 comments",
        "  - 100 tags",
        "  - 30,000 post_tags",
        "  - 5,000 followers",
        "  - 20,000 likes",
        "",
        "### End-to-end results",
        "",
        "All times are microseconds per operation (lower is better).",
        "",
        make_table(end_to_end_ops),
        "",
        "### Query construction (no I/O)",
        "",
        make_table(query_construction_ops),
        "",
        "### Latency percentiles",
        "",
        make_percentile_table(end_to_end_ops + query_construction_ops),
        "",
    ]
    if throughput_table:
        section.extend(["### Throughput (ops/sec)", "", throughput_table, ""])

    if not path.exists():
        path.write_text("# Cross-ORM benchmark results\n\n", encoding="utf-8")

    with open(path, "a", encoding="utf-8") as f:
        f.write("\n".join(section))


def write_median_trial(
    driver: str, trials: List[List[dict]], path: Path
) -> None:
    """Write the trial whose total end-to-end time is closest to the median."""
    end_to_end_ops = {
        "select_by_pk",
        "find_many_1000",
        "find_filtered_ordered",
        "find_filtered_paginated",
        "find_in_list",
        "find_complex_filter",
        "count_filtered",
        "exists_filtered",
        "include_posts",
        "include_author",
        "include_posts_and_comments",
        "include_posts_with_tags",
        "find_popular_posts",
        "prepared_select_by_pk",
        "stream_find_many_1000",
        "bulk_insert_1000",
    }
    totals = [
        sum(r["avg_ms"] * 1000 for r in t if r["operation"] in end_to_end_ops)
        for t in trials
    ]
    med = statistics.median(totals)
    idx = min(range(len(totals)), key=lambda i: abs(totals[i] - med))
    path.write_text(json.dumps(trials[idx], indent=2), encoding="utf-8")
    print(f"  {path} (median trial for {driver})")


def _run_harness_worker_loop(
    worker_dir: Path,
    env_extra: Dict[str, str] | None,
    env_unset: List[str] | None,
    duration: float,
    worker_id: int,
) -> List[List[dict]]:
    """Repeatedly run the harness for `duration` seconds and return all trials."""
    worker_dir.mkdir(parents=True, exist_ok=True)
    results: List[List[dict]] = []
    start = time.perf_counter()
    run_id = 0
    while time.perf_counter() - start < duration:
        run_dir = worker_dir / f"run-{run_id}"
        run_dir.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        if env_unset:
            for key in env_unset:
                env.pop(key, None)
        env.setdefault("BENCH_SQLITE_PATH", str(DB_PATH))
        if env_extra:
            env.update(env_extra)
        env["BENCH_RESULTS_DIR"] = str(run_dir)
        remaining = max(0.1, start + duration - time.perf_counter())
        try:
            subprocess.run(
                [str(RUST_EXE)],
                cwd=REPO_ROOT,
                env=env,
                check=True,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=remaining,
            )
            results.append(read_json(run_dir / "ruprizzle-results.json"))
        except subprocess.TimeoutExpired:
            break
        except subprocess.CalledProcessError as e:
            print(f"    worker {worker_id} run {run_id} failed: {e}")
            break
        run_id += 1
    return results


def run_throughput(
    backend: str,
    concurrency: int,
    duration: float,
    env_extra: Dict[str, str] | None,
    env_unset: List[str] | None,
) -> Dict[str, float] | None:
    """Run `cross_orm_bench` `concurrency` times in parallel for `duration`."""
    worker_dirs = [
        BENCH_DIR / "throughput" / f"{backend}-{concurrency}-{i}"
        for i in range(concurrency)
    ]

    start = time.perf_counter()
    all_rows: List[List[dict]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [
            executor.submit(
                _run_harness_worker_loop, d, env_extra, env_unset, duration, i
            )
            for i, d in enumerate(worker_dirs)
        ]
        for future in futures:
            try:
                all_rows.extend(future.result(timeout=duration + 60))
            except Exception as e:
                print(f"    throughput worker error: {e}")
    elapsed = time.perf_counter() - start
    if not all_rows:
        return None

    by_op: Dict[str, List[dict]] = {}
    for trial in all_rows:
        for row in trial:
            by_op.setdefault(row["operation"], []).append(row)

    throughput: Dict[str, float] = {}
    for op, rows in by_op.items():
        total_ops = sum(r["iters"] for r in rows)
        if elapsed > 0:
            throughput[op] = round(total_ops / elapsed, 2)
        else:
            throughput[op] = 0.0
    return throughput


def collect_throughput(
    backend: str,
    label: str,
    env_extra: Dict[str, str] | None,
    env_unset: List[str] | None,
) -> Dict[str, Dict[str, float]]:
    """Run throughput measurements for all configured concurrency levels."""
    results: Dict[str, Dict[str, float]] = {}
    for concurrency in BENCH_CONCURRENCY:
        print(f"  Running {label} throughput with concurrency={concurrency}...")
        t = run_throughput(backend, concurrency, BENCH_DURATION_SECONDS, env_extra, env_unset)
        if t:
            results[str(concurrency)] = t
        time.sleep(0.5)
    return results


def main() -> int:
    build_ruprizzle()

    all_trials: Dict[str, List[List[dict]]] = {}

    # Ensure SQLite is not shadowed by backend env vars during SQLite runs.
    unset_for_sqlite = [
        BACKEND_ENV_VARS["postgres"],
        BACKEND_ENV_VARS["mysql"],
    ]

    print("Running ruprizzle (sqlx) trials...")
    all_trials["ruprizzle (sqlx)"] = run_rust_trials(
        {}, "ruprizzle-results.json", "sqlx", env_unset=unset_for_sqlite
    )

    print("Running ruprizzle (rusqlite) trials...")
    all_trials["ruprizzle (rusqlite)"] = run_rust_trials(
        {"RUST_BENCH_DRIVER": "rusqlite"},
        "ruprizzle-rusqlite-results.json",
        "rusqlite",
        env_unset=unset_for_sqlite,
    )

    for crate_name in ["prax", "sea-orm", "diesel"]:
        print(f"Building and running {crate_name} trials...")
        crate_dir = RUST_DIR / f"{crate_name}-bench"
        build_rust_orm(crate_name)
        all_trials[crate_name] = run_rust_orm_trials(crate_dir, f"{crate_name}-results.json")

    print("Running Drizzle trials...")
    all_trials["drizzle"] = run_node_trials("bench-drizzle.js", "drizzle-results.json")

    generate_prisma_client()

    print("Running Prisma trials...")
    all_trials["prisma"] = run_node_trials("bench-prisma.js", "prisma-results.json")

    aggregates = {driver: aggregate(trials) for driver, trials in all_trials.items()}
    combined = combine(aggregates)

    throughput: Dict[str, Dict[str, Dict[str, float]]] = {}

    # Throughput for the SQLite ruprizzle (sqlx) configuration.
    throughput["sqlite"] = collect_throughput(
        "sqlite",
        "ruprizzle (sqlx)",
        {},
        unset_for_sqlite,
    )

    out_dir = REPO_ROOT / "local" / "cross-orm-bench"
    write_raw(out_dir / "raw_results.json", all_trials, aggregates=aggregates)
    write_summary(out_dir / "results.json", combined, throughput=throughput)
    write_log(out_dir / "BENCHMARKS.log", combined, throughput=throughput)
    write_markdown(DOCS_PATH, combined, throughput=throughput)

    # Persist the trial closest to the median as the representative per-driver JSON.
    for driver in DRIVER_ORDER:
        trials = all_trials.get(driver)
        if not trials:
            continue
        if driver in ("ruprizzle (sqlx)", "ruprizzle (rusqlite)"):
            filename = {
                "ruprizzle (sqlx)": "ruprizzle-results.json",
                "ruprizzle (rusqlite)": "ruprizzle-rusqlite-results.json",
            }[driver]
        else:
            filename = f"{driver}-results.json"
        write_median_trial(driver, trials, NODE_DIR / filename)

    # Run the ruprizzle harness against PostgreSQL and/or MySQL when configured.
    for backend in ("postgres", "mysql"):
        env_var = BACKEND_ENV_VARS[backend]
        url = os.environ.get(env_var)
        if not url:
            continue
        if not seed_backend(backend):
            continue

        label = f"ruprizzle ({backend})"
        print(f"Running {backend} ruprizzle trials...")
        results_dir = BENCH_DIR / backend
        env_unset = [
            v
            for k, v in BACKEND_ENV_VARS.items()
            if k != backend
        ]
        trials = run_rust_trials(
            {env_var: url},
            "ruprizzle-results.json",
            backend,
            results_dir=results_dir,
            seed_before_each=False,
            env_unset=env_unset,
        )
        backend_combined = aggregate(trials)

        print(f"  Running {backend} throughput...")
        backend_throughput = collect_throughput(
            backend,
            label,
            {env_var: url},
            env_unset,
        )
        throughput[backend] = backend_throughput

        write_backend_markdown(
            DOCS_PATH, backend, label, backend_combined, throughput=backend_throughput
        )
        backend_median_path = results_dir / "ruprizzle-results.json"
        write_median_trial(label, trials, backend_median_path)
        print(f"  Wrote {backend_median_path}")

    # Rewrite summary/log/markdown with the full throughput data now that PG/MySQL
    # (if present) have been measured.
    write_summary(out_dir / "results.json", combined, throughput=throughput)
    write_log(out_dir / "BENCHMARKS.log", combined, throughput=throughput)
    # The main markdown section was already written before PG/MySQL; the backend
    # sections above are appended separately. Rewriting here would duplicate the
    # main section, so we leave it as is.

    print(f"\nWrote:")
    print(f"  {out_dir / 'raw_results.json'}")
    print(f"  {out_dir / 'results.json'}")
    print(f"  {out_dir / 'BENCHMARKS.log'}")
    print(f"  {DOCS_PATH}")
    for driver in DRIVER_ORDER:
        if driver in all_trials:
            if driver in ("ruprizzle (sqlx)", "ruprizzle (rusqlite)"):
                filename = {
                    "ruprizzle (sqlx)": "ruprizzle-results.json",
                    "ruprizzle (rusqlite)": "ruprizzle-rusqlite-results.json",
                }[driver]
            else:
                filename = f"{driver}-results.json"
            print(f"  {NODE_DIR / filename}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
