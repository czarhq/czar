#[cfg(feature = "wasm-embedded")]
use std::path::Path;

#[cfg(feature = "wasm-embedded")]
use anyhow::{Result, anyhow};

/// Execute a wasm module in embedded (in-process) mode using the wasmtime crate.
///
/// ## ABI contract
///
/// The module must export:
/// - `memory`           — the linear memory export
/// - `czar_alloc(i32) -> i32`  — allocate `len` bytes; returns pointer into linear memory
/// - `czar_task_run(i32, i32) -> i64` — process input at `ptr[0..len]`; returns
///   `(out_ptr as i64) << 32 | (out_len as i64)` where the output slice contains
///   either `CZAR_OUTPUT:<json>` or `{"error":"..."}`.
/// - `czar_free(i32, i32)` — release output buffer (optional; skipped if absent)
///
/// A custom entrypoint can be selected by passing a non-empty `function` value in
/// params (stored as `wasm_entrypoint`); the matching `*_free` expectation is waived
/// and only `czar_free` is tried.
#[cfg(feature = "wasm-embedded")]
pub(super) fn execute_wasm_embedded(
    module_path: &Path,
    input_json: &str,
    entrypoint: Option<&str>,
) -> Result<serde_json::Value> {
    use wasmtime::{Engine, Linker, Module, Store};

    let engine = Engine::default();
    let module = Module::from_file(&engine, module_path).map_err(|e| {
        anyhow!(
            "Failed to load wasm module {}: {}",
            module_path.display(),
            e
        )
    })?;

    let linker: Linker<()> = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| anyhow!("Failed to instantiate wasm module: {}", e))?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow!("Wasm module does not export 'memory'"))?;

    let run_name = entrypoint.unwrap_or("czar_task_run");

    // Allocate input buffer in wasm memory
    let alloc = instance
        .get_typed_func::<i32, i32>(&mut store, "czar_alloc")
        .map_err(|e| anyhow!("Failed to find czar_alloc in wasm module: {}", e))?;

    let input_bytes = input_json.as_bytes();
    let input_len = input_bytes.len() as i32;
    let input_ptr = alloc
        .call(&mut store, input_len)
        .map_err(|e| anyhow!("czar_alloc failed: {}", e))?;

    // Write input JSON into wasm linear memory
    memory
        .write(&mut store, input_ptr as usize, input_bytes)
        .map_err(|e| anyhow!("Failed to write input to wasm memory: {}", e))?;

    // Call the task function
    let run = instance
        .get_typed_func::<(i32, i32), i64>(&mut store, run_name)
        .map_err(|e| anyhow!("Failed to find {} in wasm module: {}", run_name, e))?;

    let result = run
        .call(&mut store, (input_ptr, input_len))
        .map_err(|e| anyhow!("Wasm task function {} failed: {}", run_name, e))?;

    let out_ptr = ((result >> 32) & 0xFFFF_FFFF) as usize;
    let out_len = (result & 0xFFFF_FFFF) as usize;

    // Read output from wasm memory — copy it out before releasing the borrow so we
    // can later call czar_free (which requires &mut store).
    let output_owned: String = {
        let mem_data = memory.data(&store);
        if out_ptr + out_len > mem_data.len() {
            return Err(anyhow!("Wasm output pointer out of bounds"));
        }
        let output_bytes = &mem_data[out_ptr..out_ptr + out_len];
        std::str::from_utf8(output_bytes)
            .map_err(|e| anyhow!("Wasm output is not valid UTF-8: {}", e))?
            .to_string()
    };

    // Free output buffer if czar_free is exported
    if let Ok(free) = instance.get_typed_func::<(i32, i32), ()>(&mut store, "czar_free") {
        let _ = free.call(&mut store, (out_ptr as i32, out_len as i32));
    }

    // Parse output — strip CZAR_OUTPUT: prefix if present
    let json_str = output_owned
        .strip_prefix("CZAR_OUTPUT:")
        .map(str::trim)
        .unwrap_or(output_owned.trim());

    serde_json::from_str(json_str)
        .map_err(|e| anyhow!("Failed to parse wasm output as JSON: {}: {}", e, json_str))
}
