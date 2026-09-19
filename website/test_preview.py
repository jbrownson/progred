import http.client
from functools import partial
from html.parser import HTMLParser
from http.server import ThreadingHTTPServer
from pathlib import Path
import tempfile
import threading
import unittest
from urllib.parse import urlsplit

from preview import PreviewHandler, REPOSITORY


class QuietHandler(PreviewHandler):
    def log_message(self, *args):
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
        self.assertEqual(len(assets.frames), 1)
        self.assertEqual(assets.frames[0]["src"], "./editor/")
        self.assertTrue(assets.frames[0]["title"])
        for path in assets.paths:
            with self.subTest(path=path):
                self.assertFalse(urlsplit(path).scheme)
                relative = path.removeprefix("./")
                target = (
                    REPOSITORY / "web" / relative.removeprefix("editor/")
                    if relative.startswith("editor/")
                    else REPOSITORY / "website/public" / relative
                )
                self.assertTrue(target.exists(), str(target))


if __name__ == "__main__":
    unittest.main()
