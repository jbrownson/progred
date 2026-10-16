#!/usr/bin/env python3
"""Run Progred's performance canaries and compare them with earlier runs.

Builds the progred library tests, runs each canary serially in an optimized
build, and compares the medians with the latest logged run on the same machine
and GPU adapter, built with the same Cargo features. `--record NOTE` appends
the results to the log. See docs/performance.md.
"""

import argparse
import csv
import datetime
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

import test_binaries

ROOT = Path(__file__).resolve().parent.parent
LOG = ROOT / "docs" / "performance-log.tsv"
COLUMNS = [
    "date", "commit", "machine", "gpu", "environment", "workload",
    "median_ms", "p95_ms", "samples", "rounds", "spread_pct", "note",
]

# Test paths in the progred library's test binary, and the measured frame
# count where the default 60 doesn't suit: fewer for canaries that take about
# a second per frame on slower machines, and more for sub-millisecond ones,
# whose 60 frames would last only a few milliseconds after the test starts.
# Those measure for about a third of a second on a fast machine.
SUITE = [
    ("projection::tests::frame::profile::iop_tree_profile_loop", None),
    ("projection::tests::frame::profile::iop_tree_source_profile_loop", None),
    ("projection::tests::frame::profile::color_picker_profile_loop", 20000),
    ("projection::tests::frame::profile::forest::forest_slider_profile_loop", 600),
    ("projection::tests::frame::profile::forest::forest_source_hover_profile_loop", 600),
    ("projection::tests::frame::profile::layout_ffi::grap_layout_ffi_profile_loop", 1500),
    ("projection::tests::frame::profile::cam_controls_profile_loop", 600),
    ("projection::tests::frame::profile::cam_source_profile_loop", None),
    ("projection::tests::frame::profile::cam_source_scroll_profile_loop", None),
    ("projection::tests::frame::profile::cam_editor_scroll_profile_loop", None),
    ("libraries::toolpath::computation::tests::profile_program_tree_construction", None),
    ("libraries::toolpath::tests::stock_meshing_profile", None),
    ("libraries::toolpath::tests::render_profile::stock_render_profile", None),
    ("projection::tests::frame::profile::fidget_cube_profile_loop", None),
    ("projection::tests::frame::profile::fidget_toolpaths_profile_loop", None),
    ("projection::tests::frame::profile::fidget_orbit_profile_loop", 12),
    ("projection::tests::frame::profile::fidget_torus_profile_loop", 12),
    ("projection::tests::frame::profile::fidget_tanglecube_profile_loop", 12),
    ("projection::tests::frame::profile::fidget_gyroid_profile_loop", 12),
]
# Run when the adapter is a hardware GPU, or with --gpu.
GPU_SUITE = [
    ("projection::tests::frame::compositor::editor_compositor_profile", None),
    ("projection::tests::frame::compositor::mesh::cam_mesh_roundtrip_profile", None),
]
ADAPTER = "libraries::fidget::tests::report_gpu_adapter"


def command(args, **kwargs):
    return subprocess.run(args, capture_output=True, text=True, **kwargs).stdout.strip()


def workload(test, metric):
    name = re.sub(r"(_profile_loop|_profile)$", "", test.rsplit("::", 1)[1])
    name = re.sub(r"^profile_", "", name)
    return f"{name}/{metric}" if metric else name


def run(executable, test, frames):
    """Runs one ignored test outside Cargo and the build sandbox, as the GPU
    tests are run, and returns its stderr lines and test result."""
    env = {**os.environ, "CARGO_TARGET_DIR": str(test_binaries.TARGET_DIR)}
    if frames:
        env["FRAME_PROFILE_ITERATIONS"] = str(frames)
    result = subprocess.run(
        [executable, "--ignored", "--exact", "--nocapture", "--test-threads=1", test],
        cwd=ROOT / "progred", env=env, capture_output=True, text=True,
    )
    if result.returncode != 0 or "1 passed" not in result.stdout:
        sys.stderr.write(result.stdout + result.stderr)
        sys.exit(f"perf: {test} failed or was not found")
    return [line.split("\t") for line in result.stderr.splitlines()]


def records(lines):
    return {
        fields[1]: (int(fields[2]) / 1e6, int(fields[3]) / 1e6, int(fields[4]))
        for fields in lines
        if fields[0] == "perf"
    }


def adapter(executable):
    fields = next(f for f in run(executable, ADAPTER, None) if f[0] == "perf-gpu")
    return fields[1], fields[2] not in ("Cpu", "None")


def sysctl(name):
    return command(["sysctl", "-n", name])


def cpuid(cpuinfo):
    """The processor's family, model and stepping. A virtual machine's
    processor name can be masked, so that one such as "Intel(R) Xeon(R)
    Processor @ 2.80GHz" stands for different hosts; these tell them apart."""
    fields = [
        re.search(rf"^{name}\s*: (\d+)$", cpuinfo, re.M)
        for name in ("cpu family", "model", "stepping")
    ]
    return (
        f" (family {fields[0][1]}, model {fields[1][1]}, stepping {fields[2][1]})"
        if all(fields) else ""
    )


def machine():
    """Hardware, which decides comparability, and the conditions that vary on it."""
    # The toolchain the wrappers build with, not whatever rustc is on PATH.
    rustc = command(
        ["rustup", "run", "stable", "rustc", "--version"] if shutil.which("rustup")
        else ["rustc", "--version"],
        cwd=ROOT,
    ).split(" (")[0]
    if platform.system() == "Darwin":
        cores = "+".join(
            f"{sysctl(f'hw.perflevel{level}.physicalcpu')}{kind}"
            for level, kind in [(0, "P"), (1, "E")]
            if sysctl(f"hw.perflevel{level}.physicalcpu")
        )
        hardware = (
            f"{sysctl('machdep.cpu.brand_string')} ({sysctl('hw.model')}), "
            f"{cores or sysctl('hw.ncpu')} cores, "
            f"{int(sysctl('hw.memsize')) / 2**30:.0f} GiB"
        )
        power = "battery" if "Battery Power" in command(["pmset", "-g", "batt"]) else "AC power"
        if re.search(r"lowpowermode\s+1", command(["pmset", "-g"])):
            power += ", low power mode"
        conditions = f"macOS {command(['sw_vers', '-productVersion'])}, {power}"
    else:
        cpuinfo = Path("/proc/cpuinfo").read_text()
        model = re.search(r"model name\s*: (.*)", cpuinfo)[1]
        memory = int(re.search(r"MemTotal:\s+(\d+)", Path("/proc/meminfo").read_text())[1])
        virtual = re.search(r"^flags.*\bhypervisor\b", cpuinfo, re.M)
        hardware = (
            f"{model}{cpuid(cpuinfo) if virtual else ''}, "
            f"{len(os.sched_getaffinity(0))} CPUs, {memory / 2**20:.0f} GiB"
            + (", virtual machine" if virtual else "")
        )
        conditions = f"{platform.system()} {platform.release()}"
    return hardware, f"{conditions}, {rustc}"


def commit():
    head = command(["git", "rev-parse", "--short=8", "HEAD"], cwd=ROOT)
    dirty = command(
        ["git", "status", "--porcelain", "--untracked-files=no", "--", ".",
         f":(exclude){LOG.relative_to(ROOT)}"],
        cwd=ROOT,
    )
    return head + ("+" if dirty else "")


def logged():
    if not LOG.exists():
        return []
    with LOG.open(newline="") as file:
        return list(csv.DictReader(file, delimiter="\t"))


def features(row):
    """The Cargo features a logged run was built with, kept in its environment."""
    match = re.search(r", features (\S+)$", row["environment"])
    return match[1] if match else ""


def comparable(rows, hardware, gpu, built):
    return [
        row for row in rows
        if row["machine"] == hardware and row["gpu"] == gpu and features(row) == built
    ]


def previous(rows, hardware, gpu, built):
    """The latest logged result for each workload on this machine and adapter
    from a build with the same features, else from the default build."""
    matching = comparable(rows, hardware, gpu, built)
    if built and not matching:
        print(f"no records with features {built}; comparing with the default build")
        matching = comparable(rows, hardware, gpu, "")
    return {row["workload"]: row for row in matching}


def summarize(rounds):
    """Best median across rounds, its p95, and the spread of the medians."""
    results = {}
    for name in rounds[0]:
        runs = sorted(r[name] for r in rounds)
        best, worst = runs[0][0], runs[-1][0]
        results[name] = (*runs[0], 100 * (worst - best) / best if best else 0.0)
    return results


def show(results, before):
    width = max(map(len, results)) + 2
    print(f"\n{'workload':<{width}}{'median':>10}{'p95':>10}{'spread':>8}   change from last record")
    for name, (median, p95, _, spread) in results.items():
        line = f"{name:<{width}}{median:>8.2f}ms{p95:>8.2f}ms{spread:>7.1f}%"
        if name in before:
            old = float(before[name]["median_ms"])
            change = 100 * (median - old) / old
            # Flag changes beyond the larger of 5% and twice either run's spread.
            noise = max(5.0, 2 * spread, 2 * float(before[name]["spread_pct"]))
            flag = "  slower" if change > noise else "  faster" if change < -noise else ""
            line += f"   {change:+6.1f}% vs {old:.2f}ms ({before[name]['commit']}){flag}"
        print(line)


def history(rows, hardware, gpu, built, count, pattern):
    rows = comparable(rows, hardware, gpu, built)
    runs = list(dict.fromkeys((r["date"], r["commit"]) for r in rows))[-count:]
    if not runs:
        sys.exit("perf: no recorded runs on this machine and GPU with these features")
    table = {(r["date"], r["commit"], r["workload"]): r["median_ms"] for r in rows}
    names = [n for n in dict.fromkeys(r["workload"] for r in rows) if re.search(pattern, n)]
    width = max(map(len, names)) + 2
    print(f"{'':<{width}}" + "".join(f"{c:>12}" for _, c in runs))
    print(f"{'':<{width}}" + "".join(f"{d[:10]:>12}" for d, _ in runs))
    for name in names:
        print(f"{name:<{width}}" + "".join(f"{table.get((d, c, name), '-'):>12}" for d, c in runs))
    notes = dict.fromkeys((r["commit"], r["note"]) for r in rows if (r["date"], r["commit"]) in runs)
    print("\n" + "\n".join(f"{c}: {n}" for c, n in notes if n))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--only", default="", metavar="REGEX",
                        help="run only workloads whose test name matches")
    parser.add_argument("--rounds", type=int, default=3,
                        help="run the suite this many times and keep each workload's best median")
    parser.add_argument("--gpu", action="store_true",
                        help="include the GPU compositor canaries on a software adapter")
    parser.add_argument("--record", metavar="NOTE",
                        help="append the results to docs/performance-log.tsv with this note")
    parser.add_argument("--history", type=int, metavar="RUNS", nargs="?", const=8,
                        help="show recorded medians for this machine instead of running")
    parser.add_argument("--features", default="", metavar="LIST",
                        help="build progred with these Cargo features")
    args = parser.parse_args()

    hardware, conditions = machine()
    if args.features:
        conditions += f", features {args.features}"
    [(executable, _)] = test_binaries.build(
        ["--release", "-p", "progred", "--lib"]
        + (["--features", args.features] if args.features else [])
    )
    gpu, hardware_gpu = adapter(executable)
    print(f"machine: {hardware}\nconditions: {conditions}\ngpu: {gpu}")
    if args.history:
        history(logged(), hardware, gpu, args.features, args.history, args.only)
        return
    suite = [
        (test, frames)
        for test, frames in SUITE + (GPU_SUITE if hardware_gpu or args.gpu else [])
        if re.search(args.only, test)
    ]
    if not suite:
        sys.exit(f"perf: no workloads match {args.only!r}")
    head = commit()
    rounds = []
    for index in range(args.rounds):
        print(f"round {index + 1} of {args.rounds}", file=sys.stderr)
        results = {}
        for test, frames in suite:
            print(f"  {workload(test, '')}", file=sys.stderr)
            for metric, result in records(run(executable, test, frames)).items():
                results[workload(test, metric)] = result
        rounds.append(results)
    results = summarize(rounds)
    show(results, previous(logged(), hardware, gpu, args.features))
    if args.record is not None:
        new = not LOG.exists()
        date = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%MZ")
        with LOG.open("a", newline="") as file:
            writer = csv.writer(file, delimiter="\t", lineterminator="\n")
            if new:
                writer.writerow(COLUMNS)
            for name, (median, p95, samples, spread) in results.items():
                writer.writerow([
                    date, head, hardware, gpu, conditions, name, f"{median:.3f}",
                    f"{p95:.3f}", samples, args.rounds, f"{spread:.1f}", args.record,
                ])
        print(f"\nrecorded {len(results)} workloads in {LOG.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
