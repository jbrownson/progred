#!/usr/bin/env python3
"""Deletes the development build outputs no current build uses.

Cargo never removes what an earlier build left: after a dependency upgrade, a
compiler update or a changed setting, every crate rebuilds under new hashed
names beside the old ones, so target/sandbox/build/debug only grows. This asks
Cargo which outputs the given commands use, building whatever they lack, and
deletes every other hashed output in deps, build, examples and .fingerprint.
Incremental caches carry no such hash; those untouched for a week go. Other
profiles, such as `make perf`'s, are left alone.

    sweep.py CARGO -- ARGUMENTS [-- ARGUMENTS ...]

CARGO is the platform's sandboxed wrapper; each ARGUMENTS is one Cargo command
whose outputs to keep, run with `--message-format=json`.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROFILE = os.path.join(ROOT, "target", "sandbox", "build", "debug")
HASHED = ("deps", "build", "examples", ".fingerprint")
HASH = re.compile(r"-([0-9a-f]{16})(?=[./]|$)")
WEEK = 7 * 24 * 60 * 60


def commands(arguments):
    """The Cargo commands among `arguments`, each after a `--`."""
    command = None
    for argument in arguments:
        if argument == "--":
            if command:
                yield command
            command = []
        elif command is not None:
            command.append(argument)
    if command:
        yield command


def paths(message):
    """Every path a Cargo JSON message names."""
    for key in ("filenames", "executable", "out_dir"):
        value = message.get(key)
        if isinstance(value, str):
            yield value
        elif isinstance(value, list):
            yield from value


def live(cargo, command):
    """The output hashes `command` uses, building what it lacks."""
    output = subprocess.run(
        [cargo, *command, "--message-format=json"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        text=True,
        check=True,
    ).stdout
    hashes = set()
    for line in output.splitlines():
        if line.startswith("{"):
            for path in paths(json.loads(line)):
                hashes.update(HASH.findall(path))
    return hashes


def size(path):
    if os.path.isdir(path) and not os.path.islink(path):
        return sum(
            os.path.getsize(os.path.join(folder, name))
            for folder, _, names in os.walk(path)
            for name in names
            if not os.path.islink(os.path.join(folder, name))
        )
    return os.path.getsize(path)


def remove(path):
    freed = size(path)
    if os.path.isdir(path) and not os.path.islink(path):
        shutil.rmtree(path)
    else:
        os.remove(path)
    return freed


def main():
    cargo = sys.argv[1]
    keep = set()
    for command in commands(sys.argv[2:]):
        print(f"sweep: keeping what `cargo {' '.join(command)}` uses", file=sys.stderr)
        keep |= live(cargo, command)
    if not keep:
        sys.exit("sweep: the commands named no outputs, so nothing was deleted")
    freed = 0
    for folder in HASHED:
        directory = os.path.join(PROFILE, folder)
        if not os.path.isdir(directory):
            continue
        for name in os.listdir(directory):
            found = HASH.search(name)
            if found and found.group(1) not in keep:
                freed += remove(os.path.join(directory, name))
    incremental = os.path.join(PROFILE, "incremental")
    if os.path.isdir(incremental):
        cutoff = time.time() - WEEK
        for name in os.listdir(incremental):
            path = os.path.join(incremental, name)
            if os.path.getmtime(path) < cutoff:
                freed += remove(path)
    print(f"sweep: freed {freed / 1e9:.1f} GB", file=sys.stderr)


if __name__ == "__main__":
    main()
