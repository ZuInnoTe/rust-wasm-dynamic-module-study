use std::ffi::CString;

use std::collections::HashMap;
use std::ffi::CStr;
use std::mem;

use std::cell::Cell;
use std::cell::RefCell;



/// A simple function returning a number as this is the most simple and native data type supported by WASM
/// returns a number
#[no_mangle]
pub extern "C" fn answer() -> i32 {
    return 42;
}

// Global variable to keep track of allocated memory
// Note: This is really an execption as allocate by the app to the module should have only for parameters
// Otherwise it would be really bad for performance.
thread_local!( static memory_areas: RefCell<HashMap<*const u8, u32>> = RefCell::new(HashMap::new()););

enum MemoryAreasReturnCode {
    Success=0,
    ErrorMemmoryNotAllocated=-1
}

/// Allocate some memory for the application to write data for the module
/// Note: It is up to the application (and not the WASM module) to provide enough pages, so the module does not run out of memory
 /// # Arguments
/// * `size` - size of memory to allocaten
/// returns a pointer to the allocated memory area
#[no_mangle]
pub extern "C" fn allocate(size: u32) -> *const u8 {
    // allocate the memory
    let alloc_mem: Vec<u8> = vec![0u8; size as usize];
    // create a Box out of it
    let alloc_box: Box<[u8]> = alloc_mem.into_boxed_slice();
    let result_ptr: *const u8 = alloc_box.as_ptr();
    // leak box
    Box::leak(alloc_box);
    // save allocated memory
    memory_areas.with(|mem_map| mem_map.borrow_mut().insert(result_ptr, size));
    // with box.leak
    return result_ptr;
}

/// Deallocates existing memory for the purpose of the application
/// # Arguments
/// * `ptr` - mutuable pointer to the memory to deallocate
/// returns a code if it was successful or not
#[no_mangle]
pub extern "C" fn deallocate(mut ptr: *const u8) -> i32 {
    // check if the ptr exists
    let removed_ptr: Option<u32>=None;
   let cell = Cell::new(removed_ptr);
    memory_areas.with(|mem_map|  cell.set(mem_map.borrow_mut().remove(&ptr)));
    match cell.get() {
       Some(x) => (),
       None => return MemoryAreasReturnCode::ErrorMemmoryNotAllocated as i32,
    }
    // undo box.leak with from_raw and then destroy
    let alloc_box = unsafe { Box::from_raw(&mut ptr) };
    return MemoryAreasReturnCode::Success as i32;
}

/// A hello world function that takes as input a pointer to a C string in the WASM module memory and outputs a pointer to a C string in the WASM module memory containing a greeting
/// # Arguments
/// * `name` - pointer to a c string containing a name to greet
/// Returns a pointer to a C string
///
#[no_mangle]
pub extern "C" fn wasm_memory_c_format_hello_world(name: *const i8) -> *const u8 {
    // convert parameter to Rust
    let c_str: &CStr = unsafe { CStr::from_ptr(name) };
    let name_str: &str = c_str.to_str().unwrap();
    // execute the real native function
    let result_str: String = format_hello_world(&name_str);

    // convert result to C
    let return_cstring = CString::new(result_str).unwrap();
    let return_u8: *const u8 = return_cstring.as_ptr() as *const u8;
    // we need to instruct rust to not clean up the memory
    mem::forget(return_cstring);
  
    return return_u8;
}

/// A hello world function that takes as input a pointer (offset, length) in the WASM module memory containing the name (in Rust str format)
/// # Arguments
/// * `offset` - position of the start of the Rust str
/// * `length` - length of the Rust str
/// Returns an offset in the WASM module memory where an offset and length of the result greeting (a Rust str) are stored
#[no_mangle]
pub extern "C" fn wasm_memory_rust_format_hello_world(offset: *mut u32, length: u32) -> u32 {
    // fetch from WASM module memory
    let mut input_vec: Vec<u8> = Vec::new();
    unsafe {
        Vec::extend_from_slice(
            &mut input_vec,
            std::slice::from_raw_parts(offset as *mut u8, length as usize),
        )
    };
    let name_str: String = unsafe { String::from_utf8_unchecked(input_vec) };
    // execute the real native function
    let result_str: String = format_hello_world(&name_str);
    // return position of WASM memory where we can find a offset, length pair
    let string_ptr = result_str.as_ptr();
    let length = result_str.len();
    let mut str_meta: Vec<(*const u8, usize)> = Vec::new();
    str_meta.push((string_ptr, length));
    str_meta.shrink_to_fit();
    let str_meta_ptr = str_meta.as_ptr() as *const u8;
    // the reason is that Rust only support one return value. Although it can be a tuple, this is translated by wasm to one return type and not multi-value
    // we need to instruct rust to not clean up the memory
    mem::forget(result_str);
    mem::forget(str_meta);
    return str_meta_ptr as u32;
}

/// The native hello_world function in rust
/// # Arguments
/// * `name` - a str containing the name to greet
/// Returns a string with the greeting
fn format_hello_world(name: &str) -> String {
    return format!("Hello World, {name}!");
}
