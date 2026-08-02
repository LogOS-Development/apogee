//! WASM runtime for sandboxed dynamic services.

use apogee_common::ApogeeResult;

/// A loaded WASM dynamic service.
pub struct WasmService;

impl WasmService {
    pub fn load(_bytes: &[u8]) -> ApogeeResult<Self> {
        // TODO: integrate wasmtime, validate module, set memory/timeout limits
        Ok(Self)
    }

    pub fn process(&mut self, _input: &str) -> ApogeeResult<String> {
        // TODO: call WASM entry point with input, return output
        Ok(String::new())
    }
}
