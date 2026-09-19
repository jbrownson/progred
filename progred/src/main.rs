fn main() {
    // The browser host starts the editor explicitly, after its worker is ready.
    // Instantiating the same module in that worker must not start another UI.
    #[cfg(not(target_arch = "wasm32"))]
    progred::run();
    #[cfg(target_arch = "wasm32")]
    progred::web_worker::initialize();
}
