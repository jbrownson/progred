//! Wasm projection modules called as pure functions. One compiled
//! Module per plugin; a fresh Store per call, so no state survives a
//! call; zero imports, so nothing impure exists for the guest to
//! reach. A watchdog ticks the engine's epoch so a wedged plugin
//! traps out instead of wedging the editor
//! (`../../docs/projections.md`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gid::Value;
use wasmtime::{Config, Engine, Instance, Module, Store};

const ABI_VERSION: u32 = 1;

/// One epoch tick per interval; calls get two ticks, so a plugin has
/// between one and two intervals of wall time before it traps.
const TICK: Duration = Duration::from_millis(50);

pub struct Host {
    engine: Engine,
    stop: Arc<AtomicBool>,
    watchdog: Option<std::thread::JoinHandle<()>>,
}

impl Host {
    pub fn new() -> Result<Host, String> {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config).map_err(|error| error.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let watchdog = {
            let engine = engine.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(TICK);
                    engine.increment_epoch();
                }
            })
        };
        Ok(Host {
            engine,
            stop,
            watchdog: Some(watchdog),
        })
    }

    pub fn load(&self, wasm: &[u8]) -> Result<Plugin, String> {
        let module = Module::new(&self.engine, wasm).map_err(|error| error.to_string())?;
        let plugin = Plugin {
            engine: self.engine.clone(),
            module,
        };
        match plugin.abi_version()? {
            ABI_VERSION => Ok(plugin),
            other => Err(format!(
                "plugin speaks abi {other}, host speaks {ABI_VERSION}"
            )),
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(watchdog) = self.watchdog.take() {
            watchdog.join().ok();
        }
    }
}

#[derive(Clone)]
pub struct Plugin {
    engine: Engine,
    module: Module,
}

impl Plugin {
    fn instantiate(&self) -> Result<(Store<()>, Instance), String> {
        let mut store = Store::new(&self.engine, ());
        // Before any guest code runs: the epoch counts up globally,
        // and a fresh store's default deadline of zero has already
        // passed.
        store.set_epoch_deadline(2);
        let instance =
            Instance::new(&mut store, &self.module, &[]).map_err(|error| error.to_string())?;
        Ok((store, instance))
    }

    fn abi_version(&self) -> Result<u32, String> {
        let (mut store, instance) = self.instantiate()?;
        let version = instance
            .get_typed_func::<(), u32>(&mut store, "abi_version")
            .map_err(|error| error.to_string())?;
        version
            .call(&mut store, ())
            .map_err(|error| error.to_string())
    }

    /// One projection call: input bytes into a fresh instance, output
    /// bytes back out. `None` is the plugin declining the value.
    pub fn project(&self, input: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let (mut store, instance) = self.instantiate()?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or("plugin exports no memory")?;
        let alloc = instance
            .get_typed_func::<u32, u32>(&mut store, "alloc")
            .map_err(|error| error.to_string())?;
        let project = instance
            .get_typed_func::<(u32, u32), u64>(&mut store, "project")
            .map_err(|error| error.to_string())?;
        let ptr = alloc
            .call(&mut store, input.len() as u32)
            .map_err(|error| error.to_string())?;
        memory
            .write(&mut store, ptr as usize, input)
            .map_err(|error| error.to_string())?;
        let packed = project
            .call(&mut store, (ptr, input.len() as u32))
            .map_err(|error| error.to_string())?;
        if packed == 0 {
            return Ok(None);
        }
        let (out_ptr, out_len) = ((packed >> 32) as usize, packed as u32 as usize);
        let mut output = vec![0_u8; out_len];
        memory
            .read(&store, out_ptr, &mut output)
            .map_err(|error| error.to_string())?;
        Ok(Some(output))
    }
}

/// The f64 plugin with its dispatch rule and memo: a record whose one
/// field is the f64 library cell holding an eight-byte blob routes
/// to the plugin; everything else declines. Purity — fresh instance,
/// zero imports — is what makes the memo sound.
pub struct F64Plugin {
    _host: Host,
    plugin: Plugin,
    memo: RefCell<HashMap<[u8; 8], Option<String>>>,
}

impl F64Plugin {
    pub fn load() -> Result<F64Plugin, String> {
        let host = Host::new()?;
        let source = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/guest/f64.rs"));
        let cache = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/experiments/rust-wasm-projection/f64.wasm"
        ));
        let plugin = load_file(&host, source, cache)?;
        Ok(F64Plugin {
            _host: host,
            plugin,
            memo: RefCell::new(HashMap::new()),
        })
    }

    pub fn text(&self, value: &Value) -> Option<String> {
        let bits = f64_bits(value)?;
        self.memo
            .borrow_mut()
            .entry(bits)
            .or_insert_with(|| match self.plugin.project(&bits) {
                Ok(reply) => reply.and_then(|bytes| String::from_utf8(bytes).ok()),
                Err(error) => {
                    eprintln!("f64 plugin: {error}");
                    None
                }
            })
            .clone()
    }
}

fn f64_bits(value: &Value) -> Option<[u8; 8]> {
    // The retained wasm spike receives only the numeric payload. This
    // adapter stays closed because the plugin cannot preserve fields
    // it never receives.
    let Value::Record(fields) = value else {
        return None;
    };
    if fields.len() != 1 {
        return None;
    }
    fields
        .get(&grap_f64::vocabulary::F64)
        .and_then(Value::as_blob)
        .and_then(|bytes| bytes.try_into().ok())
}

/// Compile-if-stale keyed on mtime, the wasm cached with the build
/// products.
fn load_file(host: &Host, source: &Path, cache: &Path) -> Result<Plugin, String> {
    let modified = |path: &Path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    };
    let stale = match (modified(source), modified(cache)) {
        (Some(source), Some(cache)) => cache < source,
        _ => true,
    };
    let wasm = if stale {
        let text = std::fs::read_to_string(source).map_err(|error| error.to_string())?;
        let wasm = crate::compile::compile(&text).map_err(|error| error.to_string())?;
        if let Some(parent) = cache.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(cache, &wasm).ok();
        wasm
    } else {
        std::fs::read(cache).map_err(|error| error.to_string())?
    };
    host.load(&wasm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    const ECHO: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "abi_version") (result i32) i32.const 1)
  (func (export "alloc") (param i32) (result i32) i32.const 1024)
  (func (export "project") (param i32 i32) (result i64)
    (memory.copy (i32.const 2048) (local.get 0) (local.get 1))
    (i64.or
      (i64.shl (i64.const 2048) (i64.const 32))
      (i64.extend_i32_u (local.get 1)))))"#;

    #[test]
    fn bytes_round_trip_through_a_module() {
        let host = Host::new().unwrap();
        let plugin = host.load(ECHO.as_bytes()).unwrap();
        let reply = plugin.project(b"five").unwrap();
        assert_eq!(reply.as_deref(), Some(&b"five"[..]));
    }

    const WEDGED: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "abi_version") (result i32) i32.const 1)
  (func (export "alloc") (param i32) (result i32) i32.const 1024)
  (func (export "project") (param i32 i32) (result i64)
    (loop $forever (br $forever))
    unreachable))"#;

    #[test]
    fn a_wedged_plugin_traps_instead_of_wedging_the_editor() {
        let host = Host::new().unwrap();
        let plugin = host.load(WEDGED.as_bytes()).unwrap();
        let started = std::time::Instant::now();
        assert!(plugin.project(&[]).is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn the_checked_in_plugin_compiles_and_decodes() {
        let wasm = crate::compile::compile(include_str!("../guest/f64.rs")).unwrap();
        let host = Host::new().unwrap();
        let plugin = host.load(&wasm).unwrap();
        let reply = plugin.project(&5.0_f64.to_le_bytes()).unwrap().unwrap();
        assert_eq!(String::from_utf8(reply).unwrap(), "5");
        assert_eq!(
            plugin.project(&2.5_f64.to_le_bytes()).unwrap().unwrap(),
            b"2.5"
        );
        // A wrong-width input is declined, not answered.
        assert_eq!(plugin.project(&[1, 2, 3]).unwrap(), None);
    }

    #[test]
    fn legacy_dispatch_wants_exactly_its_f64_adapter_shape() {
        let f64_value = |bytes: Vec<u8>| {
            Value::record([(grap_f64::vocabulary::F64, Value::from(bytes))])
        };
        assert_eq!(
            f64_bits(&f64_value(2.5_f64.to_le_bytes().to_vec())),
            Some(2.5_f64.to_le_bytes())
        );
        // Wrong width, wrong label, extra field, wrong field kind: all decline.
        assert_eq!(f64_bits(&f64_value(vec![0, 0])), None);
        assert_eq!(
            f64_bits(&Value::record([(
                new_cell_id(),
                Value::from(vec![0; 8]),
            )])),
            None
        );
        assert_eq!(
            f64_bits(&Value::record([
                (
                    grap_f64::vocabulary::F64,
                    Value::from(vec![0; 8]),
                ),
                (new_cell_id(), progred_text::value("x")),
            ])),
            None
        );
        assert_eq!(
            f64_bits(&Value::record([(
                grap_f64::vocabulary::F64,
                progred_text::value("5"),
            )])),
            None
        );
    }
}
