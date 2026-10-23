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
        shutil.copyfile(REPOSITORY / "website/_redirects", self.repository / "website/_redirects")
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
        (editor / "pkg-fallback").mkdir()
        (editor / "pkg-fallback/progred_bg.wasm").write_bytes(b"\0asm\x01\0\0\0")
        (editor / "pkg-fallback/progred.js").write_text("// generated editor without workers")

    def test_includes_the_complete_worker_module_tree(self):
        assemble(self.repository, self.destination)
        for module in self.destination.rglob("*.js"):
            for reference in re.findall(r"from\s+['\"]([^'\"]+)['\"]", module.read_text()):
                self.assertTrue((module.parent / reference).is_file(), (module, reference))
        for name in EDITOR_FILES:
            self.assertTrue((self.destination / "editor" / name).is_file())

    def test_every_page_loads_the_analytics_script_once(self):
        assemble(self.repository, self.destination)
        self.assertTrue((self.destination / "analytics.js").is_file())
        pages = sorted(self.destination.rglob("*.html"))
        self.assertIn(self.destination / "editor/index.html", pages)
        for page in pages:
            relative = page.relative_to(self.destination).as_posix()
            with self.subTest(page=relative):
                url = urljoin("https://prog.red/", relative)
                scripts = [urlsplit(urljoin(url, src)).path
                           for src in re.findall(r'<script\s[^>]*src="([^"]+)"', page.read_text())]
                self.assertEqual(scripts.count("/analytics.js"), 1)

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

    def test_requires_both_built_editors(self):
        for package in ("pkg", "pkg-fallback"):
            with self.subTest(package=package):
                wasm = self.repository / "web" / package / "progred_bg.wasm"
                wasm.rename(wasm.with_suffix(".moved"))
                with self.assertRaises(FileNotFoundError):
                    assemble(self.repository, self.destination)
                self.assertFalse(self.destination.exists())
                wasm.with_suffix(".moved").rename(wasm)

    def test_includes_the_fallback_editor(self):
        assemble(self.repository, self.destination)
        for name in ("progred_bg.wasm", "progred.js"):
            self.assertTrue((self.destination / "editor/pkg-fallback" / name).is_file())

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
        # Test versions are served before they go live.
        self.assertTrue(config["preview_urls"])

    def test_short_paths_lead_to_examples(self):
        assemble(self.repository, self.destination)
        sources = []
        for line in (self.destination / "_redirects").read_text().splitlines():
            source, destination, status = line.split()
            sources.append(source)
            with self.subTest(source=source):
                self.assertEqual(status, "302")
                url = urlsplit(destination)
                self.assertEqual(url.path, "/editor/")
                for example in parse_qs(url.query).get("example", []):
                    self.assertTrue((REPOSITORY / "examples" / f"{example}.gid").is_file())
        self.assertTrue({"/try", "/iop", "/cad", "/cam"} <= set(sources))

    def test_ci_entry_point_refuses_an_ordinary_local_invocation(self):
        result = subprocess.run(
            ["sh", str(REPOSITORY / "website/build-ci.sh")],
            env={**os.environ, "CI": "false"}, capture_output=True, text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("make build-website", result.stderr)


if __name__ == "__main__":
    unittest.main()
