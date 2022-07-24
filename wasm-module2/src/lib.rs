
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;

// Global variable to keep track of allocated memory
// Note: This is really an execption as allocate by the app to the module should have only for parameters
// Otherwise it would be really bad for performance.
thread_local!(
    static memory_areas: RefCell<HashMap<*const u8, (usize, ManuallyDrop<Box<[u8]>>)>> =
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
#[no_mangle]
pub extern "C" fn wasm_allocate(size: u32) -> *const u8 {
    // create a Box with empty memory
    let alloc_box = ManuallyDrop::new(vec![0u8; size as usize].into_boxed_slice());
    return allocate(size as usize, alloc_box);
}

/// Deallocates existing memory for the purpose of the application
/// # Arguments
/// * `ptr` - mutuable pointer to the memory to deallocate
/// returns a code if it was successful or not
#[no_mangle]
pub extern "C" fn wasm_deallocate(mut ptr: *const u8) -> i32 {
    // check if the ptr exists
    let cell: Cell<Option<(usize, ManuallyDrop<Box<[u8]>>)>> = Cell::new(None);
    memory_areas.with(|mem_map| cell.set(mem_map.borrow_mut().remove(&ptr)));
    let memory_area: Option<(usize, ManuallyDrop<Box<[u8]>>)> = cell.into_inner();
    match memory_area {
        Some(x) => ManuallyDrop::into_inner(x.1), // will then be deleted after function returns
        None => return MemoryAreasReturnCode::ErrorMemmoryNotAllocated as i32,
    };
    // return success
    return MemoryAreasReturnCode::Success as i32;
}


/// Validates if a pointer has been properly allocated in this module
/// # Arguments
/// * `ptr` - pointer
/// returns the size of the allocated memory area. It is 0 if the pointer is invalid
pub fn validate_pointer(ptr: *const u8) -> usize {
    let cell: Cell<usize> = Cell::new(0);
    memory_areas.with(|mem_map| match mem_map.borrow().get(&ptr) {
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
    memory_areas.with(|mem_map| mem_map.borrow_mut().insert(result_ptr, (size, alloc_box)));
    return result_ptr;
}
