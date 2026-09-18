"""Fetch (only with --fetch) and build the isolated, pinned Bend experiment."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
OUT = ROOT / "target/sandbox/bend-fidget"
PIN = "46df6bef271702221dafac6c85dfb36012dd0ef1"
FILES = ["bend2/main.ts", "bend2/bend.ts", "bend2/comp.ts", "bend2/base.bend"]

def call(*args):
    subprocess.run([str(a) for a in args], cwd=ROOT, check=True)

def sandbox(*args):
    call("sh", HERE / "sandbox", *args)

def build(gpu):
    out = OUT / ("gpu" if gpu else "eval")
    source = HERE / "eval.bend"
    if gpu:
        source = OUT / "gpu.bend"
        source.write_text((HERE / "eval.bend").read_text().replace("rows : Results = batch(", "rows : Results = batch!("))
        shutil.copyfile(HERE / "io.c", OUT / "io.c")
    t = time.perf_counter()
    sandbox("node", OUT / "upstream/bend2/main.ts", source, "-o", out.with_suffix(".c"))
    emitted = time.perf_counter()
    args = ["-DBEND_METAL=1", "-x", "objective-c", "-fobjc-arc", "-fmodules"] if gpu else []
    sandbox("clang", *args, "-std=c11", "-O3", "-ffp-contract=off", out.with_suffix(".c"), "-lpthread", "-lm", "-o", out)
    result = {"bend_commit": PIN, "gpu": gpu, "emit_ms": (emitted-t)*1000, "clang_ms": (time.perf_counter()-emitted)*1000,
              "sources_sha256": {f: hashlib.sha256((OUT/"upstream"/f).read_bytes()).hexdigest() for f in FILES}}
    if gpu:
        # Standard App Sandbox grants Metal access while keeping this headless
        # executable isolated. It has no network, user-file, or JIT entitlement.
        app = OUT / "BendFidget.app"
        contents = app / "Contents"
        (contents / "MacOS").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(out, contents / "MacOS/benchmark")
        (contents / "MacOS/benchmark").chmod(0o755)
        with (contents / "Info.plist").open("wb") as f:
            plistlib.dump({"CFBundleIdentifier": "com.progred.experiment.bend-fidget", "CFBundleName": "Headless Bend Fidget benchmark",
                          "CFBundleExecutable": "benchmark", "CFBundlePackageType": "APPL"}, f)
        entitlements = OUT / "gpu.entitlements"
        with entitlements.open("wb") as f:
            plistlib.dump({"com.apple.security.app-sandbox": True}, f)
        call("/usr/bin/codesign", "--force", "--sign", "-", "--identifier", "com.progred.experiment.bend-fidget", "--options", "runtime",
             "--timestamp=none", "--entitlements", entitlements, app)
        call("/usr/bin/codesign", "--verify", "--strict", app)
    (OUT / ("gpu-build.json" if gpu else "cpu-build.json")).write_text(json.dumps(result, indent=2)+"\n")
    print(json.dumps(result))

if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--fetch", action="store_true", help="download source only; never executes it")
    p.add_argument("--gpu", action="store_true")
    args = p.parse_args()
    if args.fetch:
        for name in FILES:
            data = urllib.request.urlopen(f"https://raw.githubusercontent.com/bendlang/bend/{PIN}/{name}", timeout=30).read()
            dest = OUT / "upstream" / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
        (OUT / "upstream/COMMIT").write_text(PIN+"\n")
    else:
        assert (OUT/"upstream/COMMIT").read_text().strip() == PIN, "fetch the pinned sources first"
        build(args.gpu)
