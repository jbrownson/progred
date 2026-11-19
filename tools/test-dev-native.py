"""Exercise development-session signals with disposable processes, never Progred."""

import contextlib
import os
from pathlib import Path
import pty
import selectors
import signal
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parent.parent
PYTHON = sys.executable


@contextlib.contextmanager
def session(commands, pulling=False, cwd=None):
    program = (
        f"import runpy; dev = runpy.run_path({str(ROOT / 'tools/dev-native')!r}); "
        f"raise SystemExit(dev['develop']({commands!r}, {pulling!r}))"
    )
    process = subprocess.Popen(
        [PYTHON, "-u", "-c", program],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        cwd=cwd,
        env={**os.environ, **GIT_IDENTITY},
    )
    try:
        yield process
    finally:
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
        try:
            process.wait(timeout=8)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stdout.close()


def read_until_fd(fd, needle, timeout=5):
    deadline = time.monotonic() + timeout
    output = b""
    with selectors.DefaultSelector() as selector:
        selector.register(fd, selectors.EVENT_READ)
        while needle.encode() not in output:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                raise AssertionError(f"missing {needle!r}: {output.decode()}")
            chunk = os.read(fd, 4096)
            if not chunk:
                raise AssertionError(f"process exited before {needle!r}: {output.decode()}")
            output += chunk
    return output.decode()


def read_until(process, needle, timeout=5):
    return read_until_fd(process.stdout.fileno(), needle, timeout)


def command(program):
    return [PYTHON, "-u", "-c", program]


GIT_IDENTITY = {
    "GIT_AUTHOR_NAME": "test",
    "GIT_AUTHOR_EMAIL": "test@example.invalid",
    "GIT_COMMITTER_NAME": "test",
    "GIT_COMMITTER_EMAIL": "test@example.invalid",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
}


def git(directory, *arguments):
    subprocess.run(
        ["git", *arguments], cwd=directory, check=True, capture_output=True,
        env={**os.environ, **GIT_IDENTITY},
    )


@contextlib.contextmanager
def repositories():
    """An upstream, a clone the session pulls into, and another clone that
    pushes to the upstream, each with one commit."""
    with tempfile.TemporaryDirectory() as directory:
        upstream, here, there = (Path(directory) / name for name in ("upstream", "here", "there"))
        git(directory, "init", "--quiet", "--bare", "--initial-branch=main", str(upstream))
        git(directory, "clone", "--quiet", str(upstream), str(there))
        git(there, "commit", "--quiet", "--allow-empty", "-m", "first")
        git(there, "push", "--quiet", "origin", "HEAD:main")
        git(directory, "clone", "--quiet", "--branch", "main", str(upstream), str(here))
        yield here, there


WAITING = """
import os, signal, time
def stop(*_):
    print('STOPPED', os.getpid(), flush=True)
    raise SystemExit(0)
signal.signal(signal.SIGTERM, stop)
print('RUNNING', os.getpid(), flush=True)
while True:
    time.sleep(1)
"""


class DevelopmentTests(unittest.TestCase):
    def test_make_dry_run_does_not_enter_the_loop(self):
        # Run from `make test`, this make would otherwise take itself for a
        # recursive one and announce the directory it enters.
        apart = {
            name: value
            for name, value in os.environ.items()
            if name not in ("MAKEFLAGS", "MFLAGS", "MAKELEVEL")
        }
        result = subprocess.run(
            ["make", "-n", "dev"], cwd=ROOT, capture_output=True, text=True, timeout=3, env=apart,
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout.strip(), "./tools/dev-native")

    def test_cli_requires_a_terminal_before_building(self):
        result = subprocess.run(
            [PYTHON, str(ROOT / "tools/dev-native")], stdin=subprocess.DEVNULL,
            capture_output=True, text=True, timeout=3,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("interactive terminal", result.stderr)

    def test_failed_build_waits_for_retry_and_never_runs_the_app(self):
        build = command("print('BUILD'); raise SystemExit(9)")
        run = command("print('APP MUST NOT RUN')")
        with session([build, run]) as process:
            initial = read_until(process, "Ctrl+C retries")
            self.assertEqual(initial.count("BUILD"), 1)
            self.assertNotIn("APP MUST NOT RUN", initial)
            time.sleep(0.2)
            process.send_signal(signal.SIGINT)
            retry = read_until(process, "Ctrl+C retries")
            self.assertEqual(retry.count("BUILD"), 1)
            self.assertNotIn("APP MUST NOT RUN", retry)
            process.send_signal(signal.SIGQUIT)
            self.assertEqual(process.wait(timeout=3), 0)

    def test_app_exit_rebuilds_and_restarts_without_a_terminal_signal(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = str(Path(directory) / "started")
            run = command(
                "from pathlib import Path\n"
                f"marker = Path({marker!r})\n"
                "if not marker.exists():\n"
                "    marker.touch()\n"
                "    print('FIRST EXIT', flush=True)\n"
                "else:\n"
                f"    exec({WAITING!r})\n"
            )
            with session([command("print('BUILD')"), run]) as process:
                restarted = read_until(process, "RUNNING")
                self.assertEqual(restarted.count("BUILD"), 2)
                self.assertLess(restarted.index("FIRST EXIT"), restarted.rindex("BUILD"))
                process.send_signal(signal.SIGQUIT)
                read_until(process, "STOPPED")
                self.assertEqual(process.wait(timeout=3), 0)

    def test_restart_stops_its_child_before_rebuilding_and_leaves_other_processes(self):
        unrelated = subprocess.Popen([PYTHON, "-c", "import time; time.sleep(30)"])
        try:
            with session([command("print('BUILD')"), command(WAITING)]) as process:
                read_until(process, "RUNNING")
                process.send_signal(signal.SIGINT)
                restarted = read_until(process, "RUNNING")
                self.assertLess(restarted.index("STOPPED"), restarted.index("BUILD"))
                self.assertIsNone(unrelated.poll())
                process.send_signal(signal.SIGQUIT)
                read_until(process, "STOPPED")
                self.assertEqual(process.wait(timeout=3), 0)
                self.assertIsNone(unrelated.poll())
        finally:
            unrelated.terminate()
            unrelated.wait()

    def test_terminal_hangup_and_termination_stop_the_child(self):
        for signum in (signal.SIGHUP, signal.SIGTERM):
            with self.subTest(signal=signum), session([command(WAITING)]) as process:
                read_until(process, "RUNNING")
                process.send_signal(signum)
                read_until(process, "STOPPED")
                self.assertEqual(process.wait(timeout=3), 0)

    def test_interrupt_during_build_does_not_launch_the_app(self):
        with session([command(WAITING), command("print('APP MUST NOT RUN')")]) as process:
            read_until(process, "RUNNING")
            process.send_signal(signal.SIGINT)
            restarted = read_until(process, "RUNNING")
            self.assertIn("STOPPED", restarted)
            self.assertNotIn("APP MUST NOT RUN", restarted)

    def test_pull_fast_forwards_before_each_build_and_says_what_arrived(self):
        with repositories() as (here, there):
            with session([command("print('BUILD')"), command(WAITING)], True, here) as process:
                first = read_until(process, "RUNNING")
                self.assertNotIn("Pulled", first)
                git(there, "commit", "--quiet", "--allow-empty", "-m", "second")
                git(there, "push", "--quiet", "origin", "HEAD:main")
                process.send_signal(signal.SIGINT)
                restarted = read_until(process, "RUNNING")
                self.assertIn("Fast-forward", restarted)
                self.assertIn("Pulled:", restarted)
                self.assertLess(restarted.index("second"), restarted.index("BUILD"))
                process.send_signal(signal.SIGQUIT)
                read_until(process, "STOPPED")
                self.assertEqual(process.wait(timeout=3), 0)

    def test_a_pull_that_cannot_fast_forward_builds_what_is_here(self):
        with repositories() as (here, there):
            git(here, "commit", "--quiet", "--allow-empty", "-m", "mine")
            git(there, "commit", "--quiet", "--allow-empty", "-m", "theirs")
            git(there, "push", "--quiet", "origin", "HEAD:main")
            with session([command("print('BUILD')"), command(WAITING)], True, here) as process:
                started = read_until(process, "RUNNING")
                self.assertIn("Not possible to fast-forward", started)
                self.assertIn("Not pulled.", started)
                self.assertLess(started.index("Not pulled"), started.index("BUILD"))
                process.send_signal(signal.SIGQUIT)
                read_until(process, "STOPPED")
                self.assertEqual(process.wait(timeout=3), 0)

    def test_make_passes_pull_as_the_loop_s_flag(self):
        apart = {
            name: value
            for name, value in os.environ.items()
            if name not in ("MAKEFLAGS", "MFLAGS", "MAKELEVEL")
        }
        for pull, expected in (("1", "./tools/dev-native --pull"), ("0", "./tools/dev-native")):
            result = subprocess.run(
                ["make", "-n", "dev", f"PULL={pull}"], cwd=ROOT, capture_output=True, text=True,
                timeout=3, env=apart,
            )
            self.assertEqual(result.stdout.strip(), expected)

    def test_terminal_keys_work_through_make(self):
        commands = [command("print('BUILD')"), command(WAITING)]
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / "session.py"
            script.write_text(
                f"import runpy; dev = runpy.run_path({str(ROOT / 'tools/dev-native')!r}); "
                f"raise SystemExit(dev['develop']({commands!r}))"
            )
            makefile = Path(directory) / "Makefile"
            makefile.write_text(f"dev:\n\t@{PYTHON} -u '{script}'\n")
            pid, fd = pty.fork()
            if pid == 0:
                os.execvp("make", ["make", "-f", str(makefile), "dev"])
            try:
                read_until_fd(fd, "RUNNING")
                os.write(fd, b"\x03")
                restarted = read_until_fd(fd, "RUNNING")
                self.assertLess(restarted.index("STOPPED"), restarted.index("BUILD"))
                os.write(fd, b"\x1c")
                read_until_fd(fd, "STOPPED")
                deadline = time.monotonic() + 3
                while time.monotonic() < deadline:
                    ended, _ = os.waitpid(pid, os.WNOHANG)
                    if ended:
                        pid = None
                        break
                    time.sleep(0.05)
                self.assertIsNone(pid, "make must finish after Ctrl+\\")
            finally:
                if pid is not None:
                    os.killpg(pid, signal.SIGTERM)
                    os.waitpid(pid, 0)
                os.close(fd)


if __name__ == "__main__":
    unittest.main()
