import http.client
from contextlib import redirect_stderr, redirect_stdout
from functools import partial
from html.parser import HTMLParser
from http.server import ThreadingHTTPServer
from pathlib import Path
import io
import subprocess
import tempfile
import threading
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urljoin, urlsplit

from preview import PreviewHandler, REPOSITORY, main, prepare_editor


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.editor = self.root / "web"
        (self.editor / "pkg").mkdir(parents=True)

    def built_files(self):
        (self.editor / "pkg/progred_bg.wasm").write_bytes(b"\0asm")
        (self.editor / "pkg/progred.js").write_text("// generated editor")

    def test_existing_editor_is_reused_without_running_a_build(self):
        self.built_files()
        with patch("preview.subprocess.run") as build, patch("builtins.print"):
            self.assertEqual(prepare_editor(self.root), self.editor)
            build.assert_not_called()

    def test_missing_either_generated_file_builds_with_the_repository_workflow(self):
        for missing in ("progred_bg.wasm", "progred.js"):
            with self.subTest(missing=missing):
                self.built_files()
                (self.editor / "pkg" / missing).unlink()
                with patch("preview.subprocess.run", side_effect=lambda *a, **k: self.built_files()) as build, patch("builtins.print"):
                    self.assertEqual(prepare_editor(self.root), self.editor)
                    build.assert_called_once_with(["make", "build-web"], cwd=self.root, check=True)

    def test_explicit_rebuild_does_not_reuse_existing_files(self):
        self.built_files()
        with patch("preview.subprocess.run") as build, patch("builtins.print"):
            prepare_editor(self.root, rebuild=True)
            build.assert_called_once_with(["make", "build-web"], cwd=self.root, check=True)

    def test_incomplete_build_is_not_served(self):
        with patch("preview.subprocess.run"), patch("builtins.print"):
            with self.assertRaisesRegex(FileNotFoundError, "required editor files"):
                prepare_editor(self.root)

    def test_failed_rebuild_does_not_open_or_serve_an_old_build(self):
        self.built_files()
        with patch("preview.REPOSITORY", self.root), \
             patch("preview.subprocess.run", side_effect=subprocess.CalledProcessError(2, ["make", "build-web"])), \
             patch("preview.ThreadingHTTPServer") as server, \
             patch("preview.webbrowser.open") as browser, \
             patch("builtins.print"), patch("sys.stderr"):
            with self.assertRaises(SystemExit) as raised:
                main(["--rebuild"])
            self.assertEqual(raised.exception.code, 1)
            server.assert_not_called()
            browser.assert_not_called()

    def test_launcher_shows_its_url_last_and_opens_a_browser_only_when_requested(self):
        self.built_files()
        for options in ([], ["--no-open"], ["--open"]):
            with self.subTest(options=options), \
                 patch("preview.REPOSITORY", self.root), \
                 patch("preview.ThreadingHTTPServer") as server_type, \
                 patch("preview.webbrowser.open") as browser, \
                 patch("preview.subprocess.run") as build, patch("builtins.print") as output:
                server = server_type.return_value
                server.server_port = 8123
                main(["--port", "8123", *options])
                build.assert_not_called()
                self.assertEqual(server_type.call_args.args[0], ("127.0.0.1", 8123))
                server.serve_forever.assert_called_once_with()
                self.assertEqual(output.call_args.args, ("\nhttp://127.0.0.1:8123/",))
                if options == ["--open"]:
                    browser.assert_called_once_with("http://127.0.0.1:8123/")
                else:
                    browser.assert_not_called()

    def test_stopping_prints_status_and_closes_the_server(self):
        self.built_files()
        with patch("preview.REPOSITORY", self.root), \
             patch("preview.ThreadingHTTPServer") as server_type, \
             patch("preview.webbrowser.open") as browser, patch("builtins.print") as output:
            server = server_type.return_value
            server.serve_forever.side_effect = KeyboardInterrupt
            main([])
            self.assertEqual(output.call_args.args, ("\nPreview stopped.",))
            server.__exit__.assert_called_once()
            browser.assert_not_called()

    def test_default_address_stays_fixed_and_random_ports_are_opt_in(self):
        self.built_files()
        for options, port in (([], 8081), (["--port", "0"], 0)):
            with self.subTest(options=options), \
                 patch("preview.REPOSITORY", self.root), \
                 patch("preview.ThreadingHTTPServer") as server_type, \
                 patch("preview.webbrowser.open") as browser, \
                 patch("builtins.print"):
                main(["--no-open", *options])
                self.assertEqual(server_type.call_args.args[0], ("127.0.0.1", port))
                browser.assert_not_called()

    def test_occupied_port_does_not_open_browser_or_choose_another_address(self):
        self.built_files()
        with patch("preview.REPOSITORY", self.root), \
             patch("preview.ThreadingHTTPServer", side_effect=OSError("Address already in use")) as server, \
             patch("preview.webbrowser.open") as browser, \
             patch("builtins.print"), patch("sys.stderr"):
            with self.assertRaises(SystemExit) as raised:
                main([])
            self.assertEqual(raised.exception.code, 1)
            server.assert_called_once()
            browser.assert_not_called()

    def test_invalid_port_fails_before_building(self):
        for port in ("-1", "65536"):
            with self.subTest(port=port), patch("preview.prepare_editor") as prepare, patch("sys.stderr"):
                with self.assertRaises(SystemExit):
                    main(["--port", port])
                prepare.assert_not_called()


class QuietHandler(PreviewHandler):
    def log_error(self, *args):
        pass


class PreviewTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.website = self.root / "public"
        self.editor = self.root / "web"
        self.website.mkdir()
        self.editor.mkdir()
        (self.website / "index.html").write_text("website")
        (self.editor / "index.html").write_text("editor")
        (self.editor / "test.wasm").write_bytes(b"\0asm")
        (self.root / "private.txt").write_text("not public")
        (self.website / "outside").symlink_to(self.root)
        self.server = ThreadingHTTPServer(
            ("127.0.0.1", 0),
            partial(QuietHandler, website=self.website, editor=self.editor),
        )
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.directory.cleanup()

    def request(self, path, method="GET"):
        connection = http.client.HTTPConnection("127.0.0.1", self.server.server_port)
        connection.request(method, path)
        response = connection.getresponse()
        result = (
            response.status,
            {name.lower(): value for name, value in response.getheaders()},
            response.read(),
        )
        connection.close()
        return result

    def test_mounts_website_and_editor(self):
        self.assertEqual(self.request("/")[2], b"website")
        self.assertEqual(self.request("/editor/?example=anything")[2], b"editor")
        status, headers, _ = self.request("/editor")
        self.assertEqual(status, 301)
        self.assertEqual(headers["location"], "/editor/")

    def test_wasm_mime_and_reload_headers(self):
        status, headers, body = self.request("/editor/test.wasm", "HEAD")
        self.assertEqual(status, 200)
        self.assertEqual(headers["content-type"], "application/wasm")
        self.assertEqual(headers["cache-control"], "no-store")
        self.assertEqual(body, b"")

    def test_shared_memory_headers_cover_website_and_editor(self):
        for path in ("/", "/editor/", "/editor/test.wasm"):
            with self.subTest(path=path):
                _, headers, _ = self.request(path)
                self.assertEqual(headers["cross-origin-opener-policy"], "same-origin")
                self.assertEqual(headers["cross-origin-embedder-policy"], "require-corp")

    def test_cannot_serve_repository_files_or_follow_outside_symlinks(self):
        for path in (
            "/../private.txt",
            "/%2e%2e/private.txt",
            "/editor/../private.txt",
            "/outside/private.txt",
        ):
            with self.subTest(path=path):
                self.assertEqual(self.request(path)[0], 403)
        self.assertEqual(self.request("/private.txt")[0], 404)

    def test_no_directory_listing(self):
        (self.editor / "pkg").mkdir()
        self.assertEqual(self.request("/editor/pkg/")[0], 404)

    def test_requests_are_quiet_but_errors_remain_visible_with_the_link_below(self):
        self.server.RequestHandlerClass = partial(
            PreviewHandler, website=self.website, editor=self.editor,
        )
        output, errors = io.StringIO(), io.StringIO()
        with redirect_stdout(output), redirect_stderr(errors):
            self.assertEqual(self.request("/")[0], 200)
            self.assertEqual(self.request("/editor/test.wasm", "HEAD")[0], 200)
            self.assertEqual(output.getvalue(), "")
            self.assertEqual(errors.getvalue(), "")
            self.assertEqual(self.request("/missing")[0], 404)
        self.assertIn("404", errors.getvalue())
        self.assertEqual(output.getvalue().strip(), f"http://127.0.0.1:{self.server.server_port}/")


class AssetTests(unittest.TestCase):
    def test_page_uses_existing_local_assets_and_the_real_editor(self):
        class Assets(HTMLParser):
            def __init__(self):
                super().__init__()
                self.paths = []
                self.frames = []

            def handle_starttag(self, tag, attrs):
                attrs = dict(attrs)
                if tag in ("img", "iframe", "script", "link"):
                    self.paths.append(attrs.get("src", attrs.get("href")))
                if tag == "iframe":
                    self.frames.append(attrs)

        assets = Assets()
        assets.feed((REPOSITORY / "website/public/index.html").read_text())
        self.assertEqual(len(assets.frames), 8)
        documents = []
        for frame in assets.frames:
            self.assertTrue(frame["title"])
            self.assertEqual(frame["loading"], "lazy")
            url = urlsplit(urljoin("http://localhost/", frame["src"]))
            self.assertEqual(url.path, "/editor/")
            params = parse_qs(url.query)
            self.assertEqual(params["menu"], ["hidden"])
            self.assertEqual(params["threads"], ["1"])
            libraries = params["libraries"][0].split(",")
            basic = [
                "3209ad5d23a0c8513f6bd76324a5cf60",  # name
                "eaaf309c36a65d2811083944da29aec9",  # text
                "4ab5da466a7c5f1202f5ef862f5ff915",  # blob
            ]
            drawing = params["document"][0] in ("../lessons/drawing.gid", "../lessons/forest.gid")
            evaluation = ["873c68ac371dbbb98a4f198546d60241"] if params["document"][0] in ("../lessons/grap.gid", "../lessons/functions.gid", "../lessons/drawing.gid", "../lessons/forest.gid") else []
            drawing_libraries = ["25d0e2034b4bd65bebb4811d65eab89c", "ec17915df2d42377574dc90f22500fe2"] if drawing else []
            numeric = [
                "c46d010325d3a1ec0f2a84dd3a9570ae",  # number
                "1fdb573a2c56a7063546c195318214bc",  # f64
            ] if params["document"] != ["../lessons/lists.gid"] else []
            grap = ["f7735b90f6826b25c350a8fd83af8c47"] if evaluation else []
            layout = ["fb2a4dac87512d69448650bc0e29dc80"] if drawing else []
            self.assertEqual(libraries, basic + evaluation + drawing_libraries + numeric + grap + layout)
            if evaluation or params["document"] == ["../lessons/create.gid"]:
                self.assertEqual(params["tutorial-slots"], [
                    "5e716c07490849f072b4e9017dd6230d,9940ece27410c72a5308a544890ccc71,f717b766d250a7b86c5eb842885c4417" if drawing else
                    "9940ece27410c72a5308a544890ccc71,f717b766d250a7b86c5eb842885c4417,5e716c07490849f072b4e9017dd6230d"
                ])
            else:
                self.assertNotIn("tutorial-slots", params)
            document = urlsplit(urljoin(url.geturl(), params["document"][0])).path
            self.assertTrue((REPOSITORY / "website/public" / document.lstrip("/")).is_file())
            documents.append(document)
        self.assertEqual(len(set(documents)), len(documents))
        self.assertEqual(set(documents), {"/lessons/forest.gid", "/lessons/create.gid", "/lessons/values.gid", "/lessons/lists.gid", "/lessons/cells.gid", "/lessons/grap.gid", "/lessons/functions.gid", "/lessons/drawing.gid"})
        for path in assets.paths:
            with self.subTest(path=path):
                self.assertFalse(urlsplit(path).scheme)
                relative = urlsplit(path).path.removeprefix("./")
                target = (
                    REPOSITORY / "web" / relative.removeprefix("editor/")
                    if relative.startswith("editor/")
                    else REPOSITORY / "website/public" / relative
                )
                self.assertTrue(target.exists(), str(target))


if __name__ == "__main__":
    unittest.main()
