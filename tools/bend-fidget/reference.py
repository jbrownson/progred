"""Build with sandbox-cargo, then run only the headless reference diagnostic."""
import json
import subprocess
from build import ROOT, HERE, OUT

OUT.mkdir(parents=True, exist_ok=True)
build = subprocess.run([str(ROOT/"tools/sandbox-cargo"), "test", "-p", "progred", "--release", "--lib", "--no-run", "--message-format=json"], cwd=ROOT, check=True, capture_output=True, text=True)
executables = [r["executable"] for line in build.stdout.splitlines() if line.startswith("{")
               for r in [json.loads(line)] if r.get("reason") == "compiler-artifact" and r.get("executable") and r["target"]["name"] == "progred" and r["profile"]["test"]]
assert len(executables) == 1, executables
with (OUT/"reference.log").open("w") as log:
    proc = subprocess.run(["sh", str(HERE/"sandbox"), "/usr/bin/env", "BEND_FIDGET_JIT=1",
                           f"CARGO_TARGET_DIR={ROOT/'target/sandbox/build'}", executables[0],
                           "bend_fidget_export", "--ignored", "--nocapture"], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=60)
print((OUT/"reference.log").read_text())
raise SystemExit(proc.returncode)
