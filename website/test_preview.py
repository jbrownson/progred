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

from preview import PreviewHandler, REDIRECTS, REPOSITORY, main, prepare_editor, read_redirects


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

    def test_existing_editor_is_reused_without_running_a_build_only_when_asked(self):
        self.built_files()
        with patch("preview.subprocess.run") as build, patch("builtins.print"):
            self.assertEqual(prepare_editor(self.root, reuse=True), self.editor)
            build.assert_not_called()

    def test_missing_either_generated_file_builds_with_the_repository_workflow(self):
        for missing in ("progred_bg.wasm", "progred.js"):
            with self.subTest(missing=missing):
                self.built_files()
                (self.editor / "pkg" / missing).unlink()
                with patch("preview.subprocess.run", side_effect=lambda *a, **k: self.built_files()) as build, patch("builtins.print"):
                    self.assertEqual(prepare_editor(self.root, reuse=True), self.editor)
                    build.assert_called_once_with(["make", "build-web"], cwd=self.root, check=True)

    def test_ordinary_launch_builds_even_with_existing_files(self):
        self.built_files()
        with patch("preview.subprocess.run") as build, patch("builtins.print"):
            prepare_editor(self.root)
            build.assert_called_once_with(["make", "build-web"], cwd=self.root, check=True)

    def test_incomplete_build_is_not_served(self):
        with patch("preview.subprocess.run"), patch("builtins.print"):
            with self.assertRaisesRegex(FileNotFoundError, "required editor files"):
                prepare_editor(self.root)

    def test_failed_build_does_not_open_or_serve_an_old_build(self):
        self.built_files()
        with patch("preview.REPOSITORY", self.root), \
             patch("preview.subprocess.run", side_effect=subprocess.CalledProcessError(2, ["make", "build-web"])), \
             patch("preview.ThreadingHTTPServer") as server, \
             patch("preview.webbrowser.open") as browser, \
             patch("builtins.print"), patch("sys.stderr"):
            with self.assertRaises(SystemExit) as raised:
                main([])
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
                build.assert_called_once_with(["make", "build-web"], cwd=self.root, check=True)
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
            main(["--no-build"])
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
                main(["--no-open", "--no-build", *options])
                self.assertEqual(server_type.call_args.args[0], ("127.0.0.1", port))
                browser.assert_not_called()

    def test_occupied_port_does_not_open_browser_or_choose_another_address(self):
        self.built_files()
        with patch("preview.REPOSITORY", self.root), \
             patch("preview.ThreadingHTTPServer", side_effect=OSError("Address already in use")) as server, \
             patch("preview.webbrowser.open") as browser, \
             patch("builtins.print"), patch("sys.stderr"):
            with self.assertRaises(SystemExit) as raised:
                main(["--no-build"])
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

    def test_short_paths_redirect_to_the_editor(self):
        status, headers, _ = self.request("/try")
        self.assertEqual(status, 302)
        self.assertEqual(headers["location"], "/editor/")
        status, headers, _ = self.request("/cad")
        self.assertEqual((status, headers["location"]), (302, "/editor/?example=fidget-shapes"))

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
                self.external_scripts = []

            def handle_starttag(self, tag, attrs):
                attrs = dict(attrs)
                if tag == "script" and urlsplit(attrs.get("src", "")).scheme:
                    self.external_scripts.append(attrs)
                elif tag in ("img", "iframe", "script", "link"):
                    self.paths.append(attrs.get("src", attrs.get("href")))
                if tag == "iframe":
                    self.frames.append(attrs)

        assets = Assets()
        assets.feed((REPOSITORY / "website/public/tutorial.html").read_text())
        self.assertEqual(assets.external_scripts, [])
        self.assertEqual(len(assets.frames), 16)
        documents = []
        for frame in assets.frames:
            self.assertTrue(frame["title"])
            self.assertEqual(frame["loading"], "lazy")
            url = urlsplit(urljoin("http://localhost/", frame["src"]))
            self.assertEqual(url.path, "/editor/")
            params = parse_qs(url.query)
            self.assertEqual(params["menu"], ["hidden"])
            self.assertEqual(params["threads"], ["1"])
            self.assertEqual(params["wheel"], ["auto"])
            # Each editor names its libraries, once each, names among them.
            libraries = params["libraries"][0].split(",")
            self.assertIn("3209ad5d23a0c8513f6bd76324a5cf60", libraries)
            self.assertEqual(len(set(libraries)), len(libraries))
            document = urlsplit(urljoin(url.geturl(), params["document"][0])).path
            self.assertTrue((REPOSITORY / "website/public" / document.lstrip("/")).is_file())
            documents.append(document)
        # The forest opens the page, gets peeled, and ends it as yours.
        self.assertEqual(documents.count("/lessons/growing-forest.gid"), 3)
        self.assertEqual(len(set(documents)), len(documents) - 2)
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

    def test_home_page_funnels_to_zulip_and_links_resolve(self):
        class Page(HTMLParser):
            def __init__(self):
                super().__init__()
                self.links = []
                self.paths = []
                self.frames = []

            def handle_starttag(self, tag, attrs):
                attrs = dict(attrs)
                if tag == "a":
                    self.links.append(attrs["href"])
                elif tag in ("img", "script", "link") and (attrs.get("src") or attrs.get("href")):
                    self.paths.append(attrs.get("src", attrs.get("href")))
                if tag == "iframe":
                    self.frames.append(attrs)

        public = REPOSITORY / "website/public"
        page = Page()
        page.feed((public / "index.html").read_text())
        self.assertGreaterEqual(page.links.count("https://progred.zulipchat.com"), 3)
        # Asks for tutorial feedback go to its channel, not the front page.
        feedback = "https://progred.zulipchat.com/#narrow/channel/640458-tutorial-feedback"
        self.assertIn(feedback, page.links)
        self.assertIn("tutorial.html", page.links)
        tutorial = Page()
        tutorial.feed((public / "tutorial.html").read_text())
        self.assertEqual(tutorial.links.count(feedback), 2)
        for link in page.links:
            url = urlsplit(link)
            if url.scheme or not url.path:
                continue
            with self.subTest(link=link):
                if "/" + url.path in read_redirects(REDIRECTS):
                    continue
                if url.path == "editor/":
                    [example] = parse_qs(url.query)["example"]
                    self.assertTrue((REPOSITORY / "examples" / f"{example}.gid").is_file())
                    continue
                target = public / url.path
                if target.is_dir():
                    target /= "index.html"
                self.assertTrue(target.is_file(), str(target))
                if url.fragment:
                    self.assertIn(f'id="{url.fragment}"', target.read_text())
        # Every width loads the live editor, phones included.
        [frame] = page.frames
        self.assertEqual(urlsplit(urljoin("http://localhost/", frame["src"])).path, "/editor/")
        for path in page.paths:
            with self.subTest(path=path):
                relative = urlsplit(path).path.removeprefix("./")
                target = (
                    REPOSITORY / "web" / relative.removeprefix("editor/")
                    if relative.startswith("editor/")
                    else public / relative
                )
                self.assertTrue(target.exists(), str(target))


if __name__ == "__main__":
    unittest.main()
