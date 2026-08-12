#!/usr/bin/env python3
"""Rigorous cross-ORM SQLite benchmark.

Runs the ruprizzle (sqlx + rusqlite), Drizzle and Prisma harnesses multiple
warm-up + measurement trials, aggregates statistics, logs raw data, and updates
docs/BenchmarkResults.md with the median numbers.
"""

import json
import math
import os
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Union

# Number of warm-up trials to discard (JIT / cache warm-up).
WARMUP_TRIALS = 1
# Number of measured trials.
MEASURE_TRIALS = 10

REPO_ROOT = Path(__file__).resolve().parents[2]
NODE_DIR = REPO_ROOT / "local" / "cross-orm-bench" / "node"
DOCS_PATH = REPO_ROOT / "docs" / "BenchmarkResults.md"
RUST_EXE = REPO_ROOT / "target" / "release" / "examples" / "cross_orm_bench.exe"


def run_cmd(
    cmd: Union[str, List[str]],
    cwd: Path,
    env_extra: Dict[str, str] | None = None,
    shell: bool = False,
) -> subprocess.CompletedProcess:
    env = os.environ.copy()
    if env_extra:
        env.update(env_extra)
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        shell=shell,
        check=True,
        capture_output=True,
        text=True,
    )


def build_rust() -> None:
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


def seed_db() -> None:
    print("Seeding bench.sqlite3...")
    run_cmd("npm run seed", cwd=NODE_DIR, shell=True)
    # Give SQLite a moment to release WAL / file locks.
    time.sleep(0.5)


def read_json(path: Path) -> List[dict]:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def run_rust_trials(env_extra: Dict[str, str] | None, results_json: str) -> List[List[dict]]:
    seed_db()
    trials: List[List[dict]] = []
    for i in range(WARMUP_TRIALS + MEASURE_TRIALS):
        label = "warmup" if i < WARMUP_TRIALS else f"measure-{i - WARMUP_TRIALS + 1}"
        print(f"  ruprizzle ({env_extra or 'sqlx'}) {label}")
        run_cmd([str(RUST_EXE)], cwd=REPO_ROOT, env_extra=env_extra)
        if i >= WARMUP_TRIALS:
            trials.append(read_json(NODE_DIR / results_json))
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
    }


def aggregate(trials: List[List[dict]]) -> Dict[str, Dict[str, float]]:
    by_op: Dict[str, List[float]] = {}
    for trial in trials:
        for row in trial:
            by_op.setdefault(row["operation"], []).append(row["avg_ms"] * 1000.0)  # us
    return {op: stats(v) for op, v in by_op.items()}


def combine(
    ruprizzle: Dict[str, Dict[str, float]],
    rusqlite: Dict[str, Dict[str, float]],
    drizzle: Dict[str, Dict[str, float]],
    prisma: Dict[str, Dict[str, float]],
) -> Dict[str, Dict[str, Dict[str, float]]]:
    all_ops = set(ruprizzle) | set(rusqlite) | set(drizzle) | set(prisma)
    combined = {}
    for op in sorted(all_ops):
        combined[op] = {
            "ruprizzle (sqlx)": ruprizzle.get(op, {}),
            "ruprizzle (rusqlite)": rusqlite.get(op, {}),
            "prisma": prisma.get(op, {}),
            "drizzle": drizzle.get(op, {}),
        }
    return combined


def format_table_cell(value: float) -> str:
    if value == 0:
        return "—"
    if value >= 1000:
        return f"{value:,.1f}"
    return f"{value:,.1f}"


def write_raw(
    output: Path,
    ruprizzle_trials: List[List[dict]],
    rusqlite_trials: List[List[dict]],
    drizzle_trials: List[List[dict]],
    prisma_trials: List[List[dict]],
) -> None:
    raw = {
        "meta": {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "warmup_trials": WARMUP_TRIALS,
            "measure_trials": MEASURE_TRIALS,
        },
        "ruprizzle (sqlx)": ruprizzle_trials,
        "ruprizzle (rusqlite)": rusqlite_trials,
        "drizzle": drizzle_trials,
        "prisma": prisma_trials,
    }
    with open(output, "w", encoding="utf-8") as f:
        json.dump(raw, f, indent=2)


def write_summary(
    output: Path,
    combined: Dict[str, Dict[str, Dict[str, float]]],
) -> None:
    summary = {
        "meta": {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "warmup_trials": WARMUP_TRIALS,
            "measure_trials": MEASURE_TRIALS,
            "units": "us/op",
        },
        "operations": combined,
    }
    with open(output, "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)


def write_log(
    output: Path,
    combined: Dict[str, Dict[str, Dict[str, float]]],
) -> None:
    lines = [
        "Cross-ORM benchmark log",
        "=======================",
        f"Timestamp: {datetime.now(timezone.utc).isoformat()}",
        f"Warm-up trials: {WARMUP_TRIALS}",
        f"Measured trials: {MEASURE_TRIALS}",
        "Units: us/op (microseconds per operation)",
        "",
        "End-to-end results",
        "------------------",
    ]

    end_to_end_ops = [
        "select_by_pk",
        "find_many_1000",
        "find_filtered_ordered",
        "include_posts",
        "bulk_insert_1000",
    ]
    query_construction_ops = [
        "to_sql_select_by_pk",
        "to_sql_select_filter_order",
    ]

    header = f"{'Operation':<28} {'ruprizzle (sqlx)':>18} {'ruprizzle (rusqlite)':>22} {'Prisma':>12} {'Drizzle':>12}"
    lines.append(header)
    lines.append("-" * len(header))

    for op in end_to_end_ops:
        row = combined.get(op, {})
        vals = []
        for driver in ["ruprizzle (sqlx)", "ruprizzle (rusqlite)", "prisma", "drizzle"]:
            v = row.get(driver, {}).get("median", 0.0)
            vals.append("—" if v == 0 else f"{v:,.1f}")
        lines.append(f"{op:<28} {vals[0]:>18} {vals[1]:>22} {vals[2]:>12} {vals[3]:>12}")

    lines.extend(["", "Query construction (no I/O)", "---------------------------"])
    lines.append(f"{'Operation':<28} {'ruprizzle (sqlx / rusqlite)':>30} {'Drizzle':>12}")
    lines.append("-" * len(lines[-1]))
    for op in query_construction_ops:
        row = combined.get(op, {})
        v_r = row.get("ruprizzle (sqlx)", {}).get("median", 0.0)
        v_d = row.get("drizzle", {}).get("median", 0.0)
        lines.append(
            f"{op:<28} {f'{v_r:,.1f}':>30} {f'{v_d:,.1f}':>12}"
        )

    lines.extend(["", "Per-driver, per-operation statistics", "------------------------------------"])
    for driver in ["ruprizzle (sqlx)", "ruprizzle (rusqlite)", "drizzle", "prisma"]:
        lines.append(f"\n{driver}")
        lines.append(f"{'Operation':<28} {'mean':>12} {'median':>12} {'min':>12} {'max':>12} {'stdev':>12} {'CV%':>8}")
        lines.append("-" * 100)
        for op in end_to_end_ops + query_construction_ops:
            row = combined.get(op, {}).get(driver, {})
            if not row:
                continue
            lines.append(
                f"{op:<28} {row['mean']:>12.1f} {row['median']:>12.1f} "
                f"{row['min']:>12.1f} {row['max']:>12.1f} {row['stdev']:>12.1f} {row['cv']:>8.1f}"
            )

    with open(output, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


def write_markdown(
    path: Path,
    combined: Dict[str, Dict[str, Dict[str, float]]],
) -> None:
    def m(op: str, driver: str) -> str:
        row = combined.get(op, {}).get(driver, {})
        v = row.get("median", 0.0)
        if v == 0:
            return "—"
        return f"{v:,.1f}"

    end_to_end_table = """| Operation | ruprizzle (sqlx) | ruprizzle (rusqlite) | Prisma | Drizzle |
|---|---:|---:|---:|---:|
| `select_by_pk` | {select_by_pk_rz_sqlx} | {select_by_pk_rz_rusqlite} | {select_by_pk_prisma} | {select_by_pk_drizzle} |
| `find_many_1000` | {find_many_1000_rz_sqlx} | {find_many_1000_rz_rusqlite} | {find_many_1000_prisma} | {find_many_1000_drizzle} |
| `find_filtered_ordered` | {find_filtered_ordered_rz_sqlx} | {find_filtered_ordered_rz_rusqlite} | {find_filtered_ordered_prisma} | {find_filtered_ordered_drizzle} |
| `include_posts` (1,000 users + 10,000 posts) | {include_posts_rz_sqlx} | {include_posts_rz_rusqlite} | {include_posts_prisma} | {include_posts_drizzle} |
| `bulk_insert_1000` | {bulk_insert_1000_rz_sqlx} | {bulk_insert_1000_rz_rusqlite} | {bulk_insert_1000_prisma} | {bulk_insert_1000_drizzle} |""".format(
        select_by_pk_rz_sqlx=m("select_by_pk", "ruprizzle (sqlx)"),
        select_by_pk_rz_rusqlite=m("select_by_pk", "ruprizzle (rusqlite)"),
        select_by_pk_prisma=m("select_by_pk", "prisma"),
        select_by_pk_drizzle=m("select_by_pk", "drizzle"),
        find_many_1000_rz_sqlx=m("find_many_1000", "ruprizzle (sqlx)"),
        find_many_1000_rz_rusqlite=m("find_many_1000", "ruprizzle (rusqlite)"),
        find_many_1000_prisma=m("find_many_1000", "prisma"),
        find_many_1000_drizzle=m("find_many_1000", "drizzle"),
        find_filtered_ordered_rz_sqlx=m("find_filtered_ordered", "ruprizzle (sqlx)"),
        find_filtered_ordered_rz_rusqlite=m("find_filtered_ordered", "ruprizzle (rusqlite)"),
        find_filtered_ordered_prisma=m("find_filtered_ordered", "prisma"),
        find_filtered_ordered_drizzle=m("find_filtered_ordered", "drizzle"),
        include_posts_rz_sqlx=m("include_posts", "ruprizzle (sqlx)"),
        include_posts_rz_rusqlite=m("include_posts", "ruprizzle (rusqlite)"),
        include_posts_prisma=m("include_posts", "prisma"),
        include_posts_drizzle=m("include_posts", "drizzle"),
        bulk_insert_1000_rz_sqlx=m("bulk_insert_1000", "ruprizzle (sqlx)"),
        bulk_insert_1000_rz_rusqlite=m("bulk_insert_1000", "ruprizzle (rusqlite)"),
        bulk_insert_1000_prisma=m("bulk_insert_1000", "prisma"),
        bulk_insert_1000_drizzle=m("bulk_insert_1000", "drizzle"),
    )

    query_construction_table = """| Operation | ruprizzle (sqlx / rusqlite) | Drizzle |
|---|---:|---:|
| `to_sql_select_by_pk` | {to_sql_select_by_pk_rz} | {to_sql_select_by_pk_drizzle} |
| `to_sql_select_filter_order` | {to_sql_select_filter_order_rz} | {to_sql_select_filter_order_drizzle} |""".format(
        to_sql_select_by_pk_rz=m("to_sql_select_by_pk", "ruprizzle (sqlx)"),
        to_sql_select_by_pk_drizzle=m("to_sql_select_by_pk", "drizzle"),
        to_sql_select_filter_order_rz=m("to_sql_select_filter_order", "ruprizzle (sqlx)"),
        to_sql_select_filter_order_drizzle=m("to_sql_select_filter_order", "drizzle"),
    )

    # Read existing doc and replace the tables between section headers.
    text = path.read_text(encoding="utf-8")

    # Replace end-to-end table
    start_marker = "## End-to-end results\n\nAll times are microseconds per operation (lower is better).\n\n"
    end_marker = "\n\n## Query construction (no I/O)"
    s = text.find(start_marker)
    e = text.find(end_marker)
    if s != -1 and e != -1:
        text = text[: s + len(start_marker)] + end_to_end_table + text[e:]

    # Replace query construction table
    start_marker2 = "## Query construction (no I/O)\n\nDrizzle exposes `.toSQL()`; ruprizzle exposes `.to_sql()`; Prisma does not expose an equivalent API.\n\n"
    end_marker2 = "\n\n## Codegen / build-step comparison"
    s2 = text.find(start_marker2)
    e2 = text.find(end_marker2)
    if s2 != -1 and e2 != -1:
        text = text[: s2 + len(start_marker2)] + query_construction_table + text[e2:]

    # Update methodology line in caveats
    text = text.replace(
        "5. **This run was on a development machine.** Run-to-run variance can be 5–10% on Windows. The main take-away is the relative shape between backends, not single-digit absolute values.",
        f"5. **This run used {WARMUP_TRIALS} warm-up + {MEASURE_TRIALS} measured trials per driver.** Medians are reported. See `local/cross-orm-bench/BENCHMARKS.log` and `local/cross-orm-bench/raw_results.json` for full per-trial data. Run-to-run variance can be 5–10% on Windows. The main take-away is the relative shape between backends, not single-digit absolute values.",
    )

    path.write_text(text, encoding="utf-8")


def write_median_trial(drivers: Dict[str, tuple[List[List[dict]], Path]]) -> None:
    """Write the trial whose total end-to-end time is closest to the median."""
    for name, (trials, path) in drivers.items():
        totals = [sum(r["avg_ms"] * 1000 for r in t) for t in trials]
        med = statistics.median(totals)
        idx = min(range(len(totals)), key=lambda i: abs(totals[i] - med))
        path.write_text(json.dumps(trials[idx], indent=2), encoding="utf-8")
        print(f"  {path} (median trial for {name})")


def main() -> int:
    build_rust()

    print("Running ruprizzle (sqlx) trials...")
    ruprizzle_trials = run_rust_trials({}, "ruprizzle-results.json")

    print("Running ruprizzle (rusqlite) trials...")
    rusqlite_trials = run_rust_trials({"RUST_BENCH_DRIVER": "rusqlite"}, "ruprizzle-rusqlite-results.json")

    print("Running Drizzle trials...")
    drizzle_trials = run_node_trials("bench-drizzle.js", "drizzle-results.json")

    print("Running Prisma trials...")
    prisma_trials = run_node_trials("bench-prisma.js", "prisma-results.json")

    ruprizzle = aggregate(ruprizzle_trials)
    rusqlite = aggregate(rusqlite_trials)
    drizzle = aggregate(drizzle_trials)
    prisma = aggregate(prisma_trials)

    combined = combine(ruprizzle, rusqlite, drizzle, prisma)

    out_dir = REPO_ROOT / "local" / "cross-orm-bench"
    write_raw(out_dir / "raw_results.json", ruprizzle_trials, rusqlite_trials, drizzle_trials, prisma_trials)
    write_summary(out_dir / "results.json", combined)
    write_log(out_dir / "BENCHMARKS.log", combined)
    write_markdown(DOCS_PATH, combined)

    # Persist the trial closest to the median as the representative per-driver JSON.
    write_median_trial(
        {
            "ruprizzle (sqlx)": (ruprizzle_trials, NODE_DIR / "ruprizzle-results.json"),
            "ruprizzle (rusqlite)": (rusqlite_trials, NODE_DIR / "ruprizzle-rusqlite-results.json"),
            "drizzle": (drizzle_trials, NODE_DIR / "drizzle-results.json"),
            "prisma": (prisma_trials, NODE_DIR / "prisma-results.json"),
        }
    )

    print(f"\nWrote:")
    print(f"  {out_dir / 'raw_results.json'}")
    print(f"  {out_dir / 'results.json'}")
    print(f"  {out_dir / 'BENCHMARKS.log'}")
    print(f"  {DOCS_PATH}")
    print(f"  {NODE_DIR / 'ruprizzle-results.json'}")
    print(f"  {NODE_DIR / 'ruprizzle-rusqlite-results.json'}")
    print(f"  {NODE_DIR / 'drizzle-results.json'}")
    print(f"  {NODE_DIR / 'prisma-results.json'}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
