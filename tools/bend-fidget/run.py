"""Run serial benchmark processes; save raw timings and correctness results."""
import argparse
import json
from pathlib import Path
import subprocess
import time
from prepare import prepare
from build import ROOT, HERE, OUT

def run(gpu, fixtures, depths, threads):
    results = []
    log = OUT / ("gpu-results.json" if gpu else "cpu-results.json")
    for name in fixtures:
        for depth in depths:
            data = json.loads((ROOT/f"target/sandbox/build/bend-fidget/{name}.json").read_text())
            if (1 << depth) > data["samples"]:
                continue
            fixture = OUT / f"{name}-{depth}.bin"
            prepare(ROOT/f"target/sandbox/build/bend-fidget/{name}.json", fixture, depth)
            for thread in threads:
                if gpu:
                    executable = OUT/"BendFidget.app/Contents/MacOS/benchmark"
                    subprocess.run(["/usr/bin/codesign", "--verify", "--strict", str(OUT/"BendFidget.app")], check=True)
                    cmd = [str(executable), "--gpu", "1GB", "--threads", str(thread)]
                else:
                    cmd = ["sh", str(HERE/"sandbox"), str(OUT/"eval"), "--threads", str(thread)]
                start = time.perf_counter()
                with fixture.open("rb") as f:
                    proc = subprocess.run(cmd, stdin=f, capture_output=True, text=True, timeout=60, cwd=ROOT)
                result = {"fixture": name, "depth": depth, "threads": thread, "gpu": gpu,
                          "process_ms": (time.perf_counter()-start)*1000, "returncode":proc.returncode,
                          "records": [json.loads(s) for s in proc.stdout.splitlines() if s.startswith("{")], "stderr":proc.stderr}
                results.append(result)
                log.write_text(json.dumps(results, indent=2)+"\n")
                print(json.dumps(result), flush=True)
                if proc.returncode:
                    raise SystemExit(proc.returncode)

if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--gpu", action="store_true")
    p.add_argument("--fixtures", nargs="+", default=["small", "medium", "large"])
    p.add_argument("--depths", type=int, nargs="+", default=[3, 6])
    p.add_argument("--threads", type=int, nargs="+", default=[1, 8])
    args = p.parse_args()
    run(args.gpu, args.fixtures, args.depths, args.threads)
