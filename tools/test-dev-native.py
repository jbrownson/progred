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
def session(commands):
    program = (
        f"import runpy; dev = runpy.run_path({str(ROOT / 'tools/dev-native')!r}); "
        f"raise SystemExit(dev['develop']({commands!r}))"
    )
    process = subprocess.Popen(
        [PYTHON, "-u", "-c", program],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
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
        result = subprocess.run(
            ["make", "-n", "dev"], cwd=ROOT, capture_output=True, text=True, timeout=3,
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
