use std::ffi::CString;

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::mem::ManuallyDrop;
use std::ptr;

/// A simple function returning a number as this is the most simple and native data type supported by WASM
/// returns a number
#[unsafe(no_mangle)]
pub extern "C" fn answer() -> i32 {
    return 42;
}

// Global variable to keep track of allocated memory
// Note: This is really an execption as allocate by the app to the module should have only for parameters
// Otherwise it would be really bad for performance.
thread_local!(
    static MEMORY_AREAS: RefCell<HashMap<*const u8, (usize, ManuallyDrop<Box<[u8]>>)>> =
        RefCell::new(HashMap::new());
);

enum MemoryAreasReturnCode {
    Success = 0,
    ErrorMemmoryNotAllocated = -1,
}

/// Allocate some memory for the application to write data for the module
/// Note: It is up to the application (and not the WASM module) to provide enough pages, so the module does not run out of memory
/// # Arguments
/// * `size` - size of memory to allocaten
/// returns a pointer to the allocated memory area
#[unsafe(no_mangle)]
pub extern "C" fn wasm_allocate(size: u32) -> *const u8 {
    // create a Box with empty memory
    let alloc_box = ManuallyDrop::new(vec![0u8; size as usize].into_boxed_slice());
    return allocate(size as usize, alloc_box);
}

/// Deallocates existing memory for the purpose of the application
/// # Arguments
/// * `ptr` - mutuable pointer to the memory to deallocate
/// returns a code if it was successful or not
#[unsafe(no_mangle)]
pub extern "C" fn wasm_deallocate(ptr: *const u8) -> i32 {
    // check if the ptr exists
    let cell: Cell<Option<(usize, ManuallyDrop<Box<[u8]>>)>> = Cell::new(None);
    MEMORY_AREAS.with(|mem_map| cell.set(mem_map.borrow_mut().remove(&ptr)));
    let memory_area: Option<(usize, ManuallyDrop<Box<[u8]>>)> = cell.into_inner();
    match memory_area {
        Some(x) => ManuallyDrop::into_inner(x.1), // will then be deleted after function returns
        None => return MemoryAreasReturnCode::ErrorMemmoryNotAllocated as i32,
    };
    // return success
    return MemoryAreasReturnCode::Success as i32;
}

/// A hello world function that takes as input a pointer to a C string in the WASM module memory and outputs a pointer to a C string in the WASM module memory containing a greeting
/// # Arguments
/// * `name` - pointer to a c string containing a name to greet
/// Returns a pointer to a C string. Note: The calling application must signal to the module that the memory can be fred by calling deallocate on the returned pointer
///
#[unsafe(no_mangle)]
pub extern "C" fn wasm_memory_c_format_hello_world(name: *const i8) -> *const u8 {
    // validate pointer
    let expected_size: usize = validate_pointer(name as *const u8);
    if expected_size == 0 {
        return ptr::null();
    }; // return if no valid allocated memory was provided
       // convert parameter to Rust
    let c_str: &CStr = unsafe { CStr::from_ptr(name) };
    // check valid memory representation
    if c_str.to_bytes_with_nul().len() != expected_size {
        return ptr::null();
    }; // return if allocated memory does not match expected memory
    let name_str: &str = c_str.to_str().unwrap();
    // execute the real native function
    let result_str: String = format_hello_world(&name_str);

    // convert result to C and allocate the memory so the application can release it after reading
    let result_cstring: Box<[u8]> = CString::new(result_str)
        .unwrap()
        .into_bytes_with_nul()
        .into_boxed_slice();
    let result_cstring_len: usize = result_cstring.len();
    let allocated_cstring: ManuallyDrop<Box<[u8]>> = ManuallyDrop::new(result_cstring);

    let return_u8: *const u8 = allocate(result_cstring_len, allocated_cstring);
    return return_u8;
}

/// A hello world function that takes as input a pointer (offset, length) in the WASM module memory containing the name (in Rust str format)
/// # Arguments
/// * `offset` - position of the start of the Rust str
/// * `length` - length of the Rust str
/// Returns an offset in the WASM module memory where an offset and length of the result greeting (a Rust str) are stored
#[unsafe(no_mangle)]
pub extern "C" fn wasm_memory_rust_format_hello_world(offset: *mut u32, length: u32) -> u32 {
    // validate pointer
    let expected_size_param: usize = validate_pointer(offset as *const u8);
    if (expected_size_param == 0) | (expected_size_param != length as usize) {
        return 0;
    }; // return if no valid allocated memory was provided

    // fetch from WASM module memory
    let mut input_vec: Vec<u8> = Vec::new();
    unsafe {
        Vec::extend_from_slice(
            &mut input_vec,
            std::slice::from_raw_parts(offset as *mut u8, length as usize),
        )
    };

    let name_str: String = unsafe { String::from_utf8_unchecked(input_vec) };
    // execute the real native function ad make sure that String is not dropped
    let result_string: Box<[u8]> = format_hello_world(&name_str)
        .into_bytes()
        .into_boxed_slice();
    let result_string_len: usize = result_string.len();
    let allocated_result_string: ManuallyDrop<Box<[u8]>> = ManuallyDrop::new(result_string);
    // return position of WASM memory where we can find a offset, length pair
    let string_ptr = allocate(result_string_len, allocated_result_string);
    // prepare metadata
    let mut vec_meta: Vec<u8> = Vec::new();
    let string_ptr_array: [u8; (usize::BITS / 8) as usize] = (string_ptr as usize).to_le_bytes();
    let length_array: [u8; (usize::BITS / 8) as usize] = result_string_len.to_le_bytes();
    for byte in string_ptr_array {
        vec_meta.push(byte);
    }
    for byte in length_array {
        vec_meta.push(byte);
    }
    let str_meta: Box<[u8]> = vec_meta.into_boxed_slice();
    let str_meta_len: usize = str_meta.len();
    let str_meta_ptr = allocate(str_meta_len, ManuallyDrop::new(str_meta));
    // the reason is that Rust only support one return value. Although it can be a tuple, this is translated by wasm to one return type and not multi-value
    return str_meta_ptr as u32;
}

/// Validates if a pointer has been properly allocated in this module
/// # Arguments
/// * `ptr` - pointer
/// returns the size of the allocated memory area. It is 0 if the pointer is invalid
pub fn validate_pointer(ptr: *const u8) -> usize {
    let cell: Cell<usize> = Cell::new(0);
    MEMORY_AREAS.with(|mem_map| match mem_map.borrow().get(&ptr) {
        Some(x) => cell.set(x.0),
        None => cell.set(0),
    });
    return cell.get();
}

/// Allocate some memory for the application to write data for the module
/// Note: It is up to the application (and not the WASM module) to provide enough pages, so the module does not run out of memory
/// This function can also be used internally by the WASM module to return data to the calling application of the module
/// # Arguments
/// * `size` - size of memory to allocaten
/// returns a pointer to the allocated memory area
pub fn allocate(size: usize, alloc_box: ManuallyDrop<Box<[u8]>>) -> *const u8 {
    let result_ptr: *const u8 = alloc_box.as_ptr();
    // save allocated memory to avoid it is cleaned up after function exits
    MEMORY_AREAS.with(|mem_map| mem_map.borrow_mut().insert(result_ptr, (size, alloc_box)));
    return result_ptr;
}

/// The native hello_world function in rust
/// # Arguments
/// * `name` - a str containing the name to greet
/// Returns a string with the greeting
fn format_hello_world(name: &str) -> String {
    return format!("Hello World, {name}!");
}
