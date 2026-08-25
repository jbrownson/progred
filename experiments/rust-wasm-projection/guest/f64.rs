//! The f64 projection: eight little-endian bytes in, decimal text
//! out. Compiled to wasm by the editor's compile service and called
//! per novel value; the ABI is three exports and a shared linear
//! memory (`../../docs/projections.md`).

#[unsafe(no_mangle)]
pub extern "C" fn abi_version() -> u32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut buffer: Vec<u8> = Vec::with_capacity(len as usize);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr as u32
}

fn reply(text: String) -> u64 {
    let bytes = text.into_bytes();
    let packed = ((bytes.as_ptr() as u64) << 32) | bytes.len() as u64;
    std::mem::forget(bytes);
    packed
}

#[unsafe(no_mangle)]
pub extern "C" fn project(ptr: u32, len: u32) -> u64 {
    if len != 8 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, 8) };
    reply(format!("{}", f64::from_le_bytes(bytes.try_into().unwrap())))
}
