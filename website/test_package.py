import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tempfile
import unittest
from urllib.parse import parse_qs, urljoin, urlsplit

from package import assemble, EDITOR_FILES, REPOSITORY


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.repository = self.root / "repo"
        self.destination = self.root / "site"
        shutil.copytree(REPOSITORY / "website/public", self.repository / "website/public")
        shutil.copyfile(REPOSITORY / "website/_headers", self.repository / "website/_headers")
        editor = self.repository / "web"
        editor.mkdir()
        for name in EDITOR_FILES:
            shutil.copyfile(REPOSITORY / "web" / name, editor / name)
        (editor / "pkg/snippets/rayon/src").mkdir(parents=True)
        (editor / "pkg/progred_bg.wasm").write_bytes(b"\0asm\x01\0\0\0")
        (editor / "pkg/progred.js").write_text(
            "import { submitJob } from '../worker-host.js';\n"
            "import { startWorkers } from './snippets/rayon/src/worker.js';\n"
        )
        (editor / "pkg/snippets/rayon/src/worker.js").write_text("export function startWorkers() {}")

    def test_includes_lessons_and_complete_worker_module_tree(self):
        assemble(self.repository, self.destination)
        html = (self.destination / "index.html").read_text()
        frames = re.findall(r'<iframe\s[^>]*src="([^"]+)"', html)
        self.assertEqual(len(frames), 8)
        for src in frames:
            url = urljoin("https://prog.red/", src.replace("&amp;", "&"))
            params = parse_qs(urlsplit(url).query)
            self.assertEqual(params["wheel"], ["auto"])
            document = params["document"][0]
            path = urlsplit(urljoin(url, document)).path
            self.assertTrue((self.destination / path.lstrip("/")).is_file())
        for module in self.destination.rglob("*.js"):
            for reference in re.findall(r"from\s+['\"]([^'\"]+)['\"]", module.read_text()):
                self.assertTrue((module.parent / reference).is_file(), (module, reference))
        for name in EDITOR_FILES:
            self.assertTrue((self.destination / "editor" / name).is_file())
        self.assertTrue((self.destination / "logo.svg").is_file())

    def test_excludes_source_and_diagnostic_pages(self):
        (self.repository / "web/secret.env").write_text("private")
        (self.repository / "web/cam-profile.html").write_text("diagnostic")
        (self.repository / "web/pkg/progred_bg.wasm.d.ts").write_text("types")
        (self.repository / "Cargo.lock").write_text("lockfile")
        assemble(self.repository, self.destination)
        paths = {p.relative_to(self.destination).as_posix() for p in self.destination.rglob("*")}
        self.assertNotIn("editor/secret.env", paths)
        self.assertNotIn("editor/cam-profile.html", paths)
        self.assertNotIn("editor/pkg/progred_bg.wasm.d.ts", paths)
        self.assertNotIn("Cargo.lock", paths)

    def test_requires_built_editor(self):
        (self.repository / "web/pkg/progred_bg.wasm").unlink()
        with self.assertRaises(FileNotFoundError):
            assemble(self.repository, self.destination)
        self.assertFalse(self.destination.exists())

    def test_rejects_invalid_wasm_and_does_not_overlay_an_old_package(self):
        wasm = self.repository / "web/pkg/progred_bg.wasm"
        wasm.write_bytes(b"invalid")
        with self.assertRaises(ValueError):
            assemble(self.repository, self.destination)
        wasm.write_bytes(b"\0asm\x01\0\0\0")
        self.destination.mkdir()
        with self.assertRaises(FileExistsError):
            assemble(self.repository, self.destination)

    def test_rejects_oversized_assets_before_deployment(self):
        with (self.repository / "website/public/too-big.bin").open("wb") as asset:
            asset.truncate(25 * 1024 * 1024 + 1)
        with self.assertRaisesRegex(ValueError, "25 MiB"):
            assemble(self.repository, self.destination)

    def test_isolation_headers_apply_to_every_asset_without_spa_fallback(self):
        assemble(self.repository, self.destination)
        lines = (self.destination / "_headers").read_text().splitlines()
        self.assertEqual(lines[0], "/*")
        headers = dict(line.strip().split(": ", 1) for line in lines[1:])
        self.assertEqual(headers["Cross-Origin-Opener-Policy"], "same-origin")
        self.assertEqual(headers["Cross-Origin-Embedder-Policy"], "require-corp")
        self.assertEqual(headers["X-Content-Type-Options"], "nosniff")
        config = json.loads((REPOSITORY / "website/wrangler.jsonc").read_text())
        self.assertEqual(config["assets"]["not_found_handling"], "404-page")
        self.assertNotIn("main", config)

    def test_ci_entry_point_refuses_an_ordinary_local_invocation(self):
        result = subprocess.run(
            ["sh", str(REPOSITORY / "website/build-ci.sh")],
            env={**os.environ, "CI": "false"}, capture_output=True, text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("make build-website", result.stderr)

    def test_builds_request_the_linked_browser_binary(self):
        scripts = (
            ("website/build-ci.sh", 'cargo +"$web_toolchain" build'),
            ("Makefile", "./tools/sandbox-cargo web-threaded build"),
        )
        for file, prefix in scripts:
            with self.subTest(file=file):
                source = (REPOSITORY / file).read_text().replace("\\\n", " ")
                commands = [line.strip() for line in source.splitlines() if line.strip().startswith(prefix)]
                self.assertEqual(len(commands), 1)
                arguments = shlex.split(commands[0])
                self.assertNotIn("--lib", arguments)
                self.assertEqual(arguments[arguments.index("--bin") + 1], "progred")


if __name__ == "__main__":
    unittest.main()
