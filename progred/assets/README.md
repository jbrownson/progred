# Embedded browser assets

`NotoSans-Regular.ttf` and `NotoSansMono-Regular.ttf` are embedded only in the
WASM build because browsers do not expose their installed font bytes to Parley.
They remain covered by the SIL Open Font License in
`NotoSans-LICENSE.txt`.

Browser UI text prefers Noto Sans, with Noto Sans Mono as a fallback for
missing glyphs such as `→`. Native builds use system fonts; headless browser-font
tests and website lesson captures use the bundled collection with system fonts
disabled.
