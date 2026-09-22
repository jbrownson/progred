"""Serve the website and browser editor locally, without exposing the repository."""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import subprocess
from urllib.parse import unquote, urlsplit
import webbrowser


REPOSITORY = Path(__file__).resolve().parent.parent
DEFAULT_PORT = 8081


def preview_url(server):
    return f"http://127.0.0.1:{server.server_port}/"


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

    def log_request(self, code="-", size="-"):
        pass

    def log_error(self, format, *args):
        super().log_error(format, *args)
        print(f"\n{preview_url(self.server)}", flush=True)

    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        super().end_headers()


def prepare_editor(repository, rebuild=False):
    editor = repository / "web"
    required = [editor / "pkg/progred_bg.wasm", editor / "pkg/progred.js"]
    if rebuild or not all(path.is_file() for path in required):
        print("Building Progred's browser editor…", flush=True)
        subprocess.run(["make", "build-web"], cwd=repository, check=True)
        if not all(path.is_file() for path in required):
            raise FileNotFoundError("The browser build did not produce the required editor files.")
    else:
        print("Using the existing browser editor. Use --rebuild after editor code changes.", flush=True)
    return editor


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    browser = parser.add_mutually_exclusive_group()
    browser.add_argument("--open", action="store_true", dest="open_browser",
                         help="Also open the default browser")
    browser.add_argument("--no-open", action="store_false", dest="open_browser",
                         help="Only show the link (the default)")
    parser.set_defaults(open_browser=False)
    parser.add_argument("--rebuild", action="store_true", help="Rebuild the browser editor before opening")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT,
                        help=f"Local port (default: {DEFAULT_PORT}; 0 chooses a free one)")
    args = parser.parse_args(argv)
    if not 0 <= args.port <= 65535:
        parser.error("--port must be between 0 and 65535")
    try:
        editor = prepare_editor(REPOSITORY, args.rebuild)
    except (OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Could not prepare the browser editor: {error}\n")
    handler = partial(
        PreviewHandler, website=REPOSITORY / "website/public", editor=editor,
    )
    try:
        server = ThreadingHTTPServer(("127.0.0.1", args.port), handler)
    except OSError as error:
        parser.exit(1, f"Could not start the preview on port {args.port}: {error}\n"
                      "Choose a different port with --port PORT if needed.\n")
    with server:
        url = preview_url(server)
        print("\nProgred local website", flush=True)
        print("Leave this terminal open. Control+C stops the server.", flush=True)
        print("Open the link in your browser; refresh after website edits.", flush=True)
        print("Closing a browser tab is fine—you can reopen the same link.", flush=True)
        if args.open_browser:
            webbrowser.open(url)
        print(f"\n{url}", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            print("\nPreview stopped.", flush=True)


if __name__ == "__main__":
    main()
