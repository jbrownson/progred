#!/usr/bin/env python3
"""Build Cargo tests through the platform's wrapper, then run the executables.

    python3 tools/test_binaries.py [CARGO TEST ARGUMENTS] [-- TEST ARGUMENTS]

Building stays inside the build sandbox, but the tests run outside it, so they
can reach the GPU the sandbox withholds. Each runs from its package directory,
with the sandbox's target directory, as `cargo test` would run it. See
docs/testing.md.
"""

import json
import os
import platform
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Where the wrappers build, and where tests that write files put them.
TARGET_DIR = ROOT / "target" / "sandbox" / "build"


def build(cargo_args):
    """Builds the selected tests and returns (executable, package directory) pairs."""
    cargo = "./tools/sandbox-cargo" if platform.system() == "Darwin" else "./tools/linux-cargo"
    result = subprocess.run(
        [cargo, "test", "--no-run", "--message-format=json-render-diagnostics", *cargo_args],
        cwd=ROOT, stdout=subprocess.PIPE, text=True,
    )
    if result.returncode != 0:
        sys.exit("test build failed")
    return [
        (message["executable"], Path(message["manifest_path"]).parent)
        for message in map(json.loads, filter(None, result.stdout.splitlines()))
        if message.get("reason") == "compiler-artifact"
        and message.get("executable")
        and message["profile"]["test"]
    ]


def main():
    args = sys.argv[1:]
    split = args.index("--") if "--" in args else len(args)
    failed = [
        Path(executable).name
        for executable, directory in build(args[:split])
        if subprocess.run(
            [executable, *args[split + 1:]],
            cwd=directory,
            env={**os.environ, "CARGO_TARGET_DIR": str(TARGET_DIR)},
        ).returncode != 0
    ]
    if failed:
        sys.exit(f"failed: {', '.join(failed)}")


if __name__ == "__main__":
    main()
