"""Serve the website and browser editor locally, without exposing the repository."""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit
import webbrowser


REPOSITORY = Path(__file__).resolve().parent.parent


class PreviewHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args, website, editor, **kwargs):
        self.website = Path(website).resolve()
        self.editor = Path(editor).resolve()
        super().__init__(*args, directory=str(self.website), **kwargs)

    def target(self, path):
        path = unquote(urlsplit(path).path)
        root, relative = (
            (self.editor, path[len("/editor"):])
            if path == "/editor" or path.startswith("/editor/")
            else (self.website, path)
        )
        return root, (root / relative.lstrip("/")).resolve()

    def translate_path(self, path):
        return str(self.target(path)[1])

    def send_head(self):
        root, target = self.target(self.path)
        if not target.is_relative_to(root):
            self.send_error(403, "Outside the website")
            return None
        return super().send_head()

    def list_directory(self, path):
        self.send_error(404, "Directory listing is disabled")
        return None

    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        super().end_headers()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-open", action="store_true", help="Do not open a browser")
    parser.add_argument("--port", type=int, default=0, help="Local port (default: choose a free one)")
    args = parser.parse_args()
    editor = REPOSITORY / "web"
    if not (editor / "pkg/progred_bg.wasm").is_file():
        parser.error("Build the editor first with make build-web, or double-click Preview.command.")
    handler = partial(
        PreviewHandler, website=REPOSITORY / "website/public", editor=editor,
    )
    with ThreadingHTTPServer(("127.0.0.1", args.port), handler) as server:
        url = f"http://127.0.0.1:{server.server_port}/"
        print(f"Progred website: {url}", flush=True)
        print("Refresh after website edits. Stop this preview with Control+C.", flush=True)
        if not args.no_open:
            webbrowser.open(url)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
