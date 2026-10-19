fn main() {
    // The browser host starts the editor explicitly, after its worker is ready.
    // Instantiating the same module in that worker must not start another UI.
    #[cfg(not(target_arch = "wasm32"))]
    progred::run();
    // Calling into the library also links its browser exports into this
    // binary, which the page and its workers call.
    #[cfg(target_arch = "wasm32")]
    progred::install_panic_hook();
}
