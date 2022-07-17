use std::mem;

/// A hello world function that takes as input an Avro data structure with one key 'name' and a value. Returns an avro structure with the greeting
/// # Arguments
/// * `offset` - position of the start of the Rust str
/// * `length` - length of the Rust str
/// Returns an offset in the WASM module memory where an offset and length of the result greeting (an avro structure) are stored
#[no_mangle]
pub extern "C" fn wasm_memory_avro_format_hello_world(offset: i32, length: i32) -> i32 {
    // fetch from WASM module memory the Avro data
    // https://arrow.apache.org/docs/python/ipc.html#ipc
    // tbd
    let name_str: &str = "test";
    // execute the real native function
    let result_str: String = format_hello_world(&name_str);
    // put result on WASM module memory
    // tbd
    let result_ptr: i32 = 0;
    // return position of WASM memory where we can find a offset, length pair
    // the reason is that Rust only support one return value. Although it can be a tuple, this is translated by wasm to one return type and not multi-value
    return result_ptr;
}

/// The native hello_world function in rust
/// # Arguments
/// * `name` - a str containing the name to greet
/// Returns a string with the greeting
fn format_hello_world(name: &str) -> String {
    return format!("Hello World, {name}!");
}
