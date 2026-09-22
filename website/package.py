"""Assemble only public website/editor assets into target/website."""

from pathlib import Path
import shutil
import tempfile


REPOSITORY = Path(__file__).resolve().parent.parent
EDITOR_FILES = ("index.html", "icon.svg", "worker-host.js", "worker.js", "platform.mjs")


def assemble(repository, destination):
    website = repository / "website"
    editor = repository / "web"
    wasm = editor / "pkg/progred_bg.wasm"
    with wasm.open("rb") as source:
        if source.read(8) != b"\0asm\x01\0\0\0":
            raise ValueError("Build the browser editor before packaging the website")
    if not (editor / "pkg/progred.js").is_file():
        raise ValueError("Missing wasm-bindgen JavaScript; run make build-web")

    shutil.copytree(website / "public", destination)
    shutil.copyfile(website / "_headers", destination / "_headers")
    (destination / "editor").mkdir()
    for name in EDITOR_FILES:
        shutil.copyfile(editor / name, destination / "editor" / name)
    for source in sorted((editor / "pkg").rglob("*")):
        if source.is_file() and source.suffix in (".js", ".wasm"):
            target = destination / "editor/pkg" / source.relative_to(editor / "pkg")
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)

    for asset in destination.rglob("*"):
        if asset.is_file() and asset.stat().st_size > 25 * 1024 * 1024:
            raise ValueError(f"Cloudflare's 25 MiB per-file limit exceeded: {asset.name}")


def main():
    target = REPOSITORY / "target"
    target.mkdir(exist_ok=True)
    output = target / "website"
    with tempfile.TemporaryDirectory(prefix="website-", dir=target) as temporary:
        staging = Path(temporary) / "site"
        assemble(REPOSITORY, staging)
        if output.exists():
            shutil.rmtree(output)
        staging.rename(output)
    print(f"Website ready: {output}")


if __name__ == "__main__":
    main()
