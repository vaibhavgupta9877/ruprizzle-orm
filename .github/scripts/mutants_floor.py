#!/usr/bin/env python3
"""Fail when a cargo-mutants run catches fewer mutants than a floor.

Usage: mutants_floor.py <outcomes.json> <floor-percent>

Kill rate is caught / (caught + missed). Unviable mutants (those that do not
build) and timeouts are reported but left out of the rate. The summary also
goes to $GITHUB_STEP_SUMMARY when it is set, so the numbers are visible on the
run page without downloading the artifact.
"""

import json
import os
import sys


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 64
    path, floor_arg = sys.argv[1], sys.argv[2]
    floor = float(floor_arg)

    with open(path, encoding="utf-8") as f:
        outcomes = json.load(f)

    caught = int(outcomes.get("caught", 0))
    missed = int(outcomes.get("missed", 0))
    timeout = int(outcomes.get("timeout", 0))
    unviable = int(outcomes.get("unviable", 0))
    tested = caught + missed
    rate = 100.0 * caught / tested if tested else 0.0

    summary = (
        f"caught {caught}, missed {missed}, timeout {timeout}, unviable {unviable}: "
        f"kill rate {rate:.1f}% (floor {floor:.1f}%)"
    )
    print(summary)
    step_summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if step_summary:
        with open(step_summary, "a", encoding="utf-8") as f:
            f.write(f"**cargo-mutants:** {summary}\n")

    if tested == 0:
        print("no mutants were tested; the run did not produce results", file=sys.stderr)
        return 1
    if rate < floor:
        print(f"kill rate {rate:.1f}% is below the floor {floor:.1f}%", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
