//!  mostly adapted from: https://docs.rs/wasmtime/latest/wasmtime/
use anyhow::Result;
use wasmtime::AsContextMut;
use wasmtime::Engine;
use wasmtime::Instance;
use wasmtime::Linker;
use wasmtime::Module;
use wasmtime::Store;
use wasmtime_wasi::{sync::WasiCtxBuilder, WasiCtx};

use std::ffi::CString;

use std::ffi::CStr;

struct MyState {
    wasi: WasiCtx,
}

/// Main function that loads a WASM module
fn main() {
    println!("Initializing WASM engine...");
    let engine: Engine = init_wasm_engine().unwrap();
    println!("Loading WASM module...");
    let module: Module = init_wasm_module(&engine).unwrap();
    println!("Running WASM function answer...");
    let result_answer = wrapper_answer(&engine, &module).unwrap();
    println!("Result from WASM function \"answer\": {}", result_answer);
    println!("Running WASM function c_format_hello_world...");
    let result_c_format_hello_world =
        wrapper_wasm_c_format_hello_world(&engine, &module, "Rust (C ABI)")
            .unwrap();
    println!(
        "Result from WASM function \"c_format_hello_world\": {}",
        result_c_format_hello_world
    );
    println!("Running WASM function rust_format_hello_world...");
    let result_rust_format_hello_world = wrapper_wasm_rust_format_hello_world(
        &engine,
        &module,
        "Rust (Rust ABI)".to_string(),
    )
    .unwrap();
    println!(
        "Result from WASM function \"rust_format_hello_world\": {}",
        result_rust_format_hello_world
    );
}

/// Init the WASM Engine
/// returns the WASM engine
fn init_wasm_engine() -> Result<Engine> {
    // Create an "Engine" to run wasm modules
    let engine = Engine::default();
    Ok(engine)
}

/// Initialize WASM module
/// # Arguments
/// * `engine` - wasmtime engine to use for the store
/// * `store` - in-memory store to use to exchange data with the function
/// returns the module
fn init_wasm_module(engine: &Engine) -> Result<Module> {
    // load WASM module
    let module = Module::from_file(
        &engine,
        "../../../wasm-module1/target/wasm32-wasi/debug/wasm_module1.wasm",
    )?;
    Ok(module)
}

/// Wrapper around the function answer of the WASM Module. This is needed as the standardization of the componennt model and webassembly interface types is still work-in-progress
/// # Arguments (note the function `answer` of the WASM module itself has no parameters. The parameters are just to initialize the runtime environment)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// returns the result of the function `answer`
fn wrapper_answer(engine: &Engine, module: &Module) -> Result<i32> {
    // Load function an instantiate it
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |state: &mut MyState| &mut state.wasi)?;
    // store to exchange data with the WASM module
    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_args()?
        .build();
    let mut store = Store::new(&engine, MyState { wasi: wasi });
    // instantiate module
    // let instance = Instance::new(&mut store, &module, &[])?;
    linker.module(&mut store, "", &module)?;
    let instance: Instance = linker.instantiate(&mut store, &module).unwrap();
    // get the function
    let func_def = instance
        .get_func(&mut store, "answer")
        .expect("`answer` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<(), i32, _>(&store)?;
    // call function
    let result = func_validated.call(&mut store, ())?;
    Ok(result)
}

/// Wrapper around the function format_hello_world (C ABI) of the WASM Module. This is needed as the standardization of the component model and webassembly interface types is still work-in-progress
/// # Arguments (note the function `format_hello_world` of the WASM module itself has just one parameter: `func_name`. The pther parameters are just to initialize the runtime environment)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// * `func_name` - Parameter `name` for the function
/// returns the result of the function `format_hello_world`
fn wrapper_wasm_c_format_hello_world(
    engine: &Engine,
    module: &Module,
    func_name: &str,
) -> Result<String> {
    // convert param to CString
    let param_name_str = func_name;
    let param_name_cstring: CString = CString::new(param_name_str).unwrap();
    let param_name_cstring_as_bytes: &[u8] = param_name_cstring.to_bytes_with_nul();
    // Load function an instantiate it
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |state: &mut MyState| &mut state.wasi)?;
    // store to exchange data with the WASM module
    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_args()?
        .build();
    let mut store = Store::new(&engine, MyState { wasi: wasi });
    // instantiate module
    // let instance = Instance::new(&mut store, &module, &[])?;
    linker.module(&mut store, "", &module)?;
    let instance: Instance = linker.instantiate(&mut store, &module).unwrap();
    // allocate shared memory for the parameter
    // allocate some memory within the WASM module
    let offset: u32 = wrapper_wasm_allocate(&engine, &module, instance, &mut store, param_name_cstring_as_bytes.len() as u32).unwrap() as u32;
  
    // get the function
    let func_def = instance
        .get_func(&mut store, "wasm_memory_c_format_hello_world")
        .expect("`wasm_memory_c_format_hello_world` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<u32, i32, _>(&store)?;

    // prepare handing over CString as input
    // instantiate memory
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
    memory.write(
        &mut store,
        offset.try_into().unwrap(),
        param_name_cstring_as_bytes
    );
    // call function answer
    let mut result_offset = func_validated.call(&mut store, offset)?;

    // read answer
    let mut buffer = [1u8; 1];
    let mut result_v_u8: Vec<u8> = Vec::new();
    while buffer[0] != 0u8 {
        memory.read(&store, result_offset.try_into().unwrap(), &mut buffer)?;
        result_v_u8.push(buffer[0]);
        result_offset += 1;
    }
        // deallocate shared WASM Module memory
        let dealloc_code: i32 = wrapper_wasm_deallocate(engine, module, instance, &mut store, offset as  *const u8).unwrap();
        if dealloc_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory");
        }
    // convert answer
    let c_str: &CStr = unsafe { CStr::from_ptr(result_v_u8.as_ptr() as *const i8) };
    let result_str: &str = c_str.to_str().unwrap();

    Ok(result_str.to_string())
}

/// Wrapper around the function format_hello_world (Rust ABI) of the WASM Module. This is needed as the standardization of the componennt model and webassembly interface types is still work-in-progress
/// # Arguments (note the function `format_hello_world` of the WASM module itself has just one parameter: `func_name`. The pther parameters are just to initialize the runtime environment)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// * `func_name` - Parameter `name` for the function
/// returns the result of the function `format_hello_world`
fn wrapper_wasm_rust_format_hello_world(
    engine: &Engine,
    module: &Module,
    func_name: String,
) -> Result<String> {
    // Load function an instantiate it
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |state: &mut MyState| &mut state.wasi)?;
    // store to exchange data with the WASM module
    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_args()?
        .build();
    let mut store = Store::new(&engine, MyState { wasi: wasi });
    // instantiate module
    // let instance = Instance::new(&mut store, &module, &[])?;
    linker.module(&mut store, "", &module)?;
    let instance: Instance = linker.instantiate(&mut store, &module).unwrap();
    // get the function
    let func_def = instance
        .get_func(&mut store, "wasm_memory_rust_format_hello_world")
        .expect("`wasm_memory_rust_format_hello_world` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<(u32, u32), u32, _>(&store)?;

    // prepare handing over Rust as input
    // instantiate memory
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
    let param_name_str: String = func_name.to_string();
    let param_name_string_as_bytes: &[u8] = param_name_str.as_bytes();
    // allocate some memory within the WASM module
    let offset: u32 = wrapper_wasm_allocate(&engine, &module, instance, &mut store, param_name_string_as_bytes.len() as u32).unwrap() as u32;
    let length: u32 = param_name_str.len() as u32;
    memory.write(
        &mut store,
        offset.try_into().unwrap(),
        param_name_string_as_bytes
    );
    // call function answer
    let mut result_offset = func_validated.call(&mut store, (offset, length))?;
    // read answer from memory: these are two values: offset and length of the return string
    // read metadata (offset and length of the sring)
    // note: WebAssembly is by default 32 bit
    let mut ptr_buffer = [0u8; (u32::BITS / 8) as usize];
    let mut len_buffer = [0u8; (u32::BITS / 8) as usize];
    memory.read(&store, result_offset.try_into().unwrap(), &mut ptr_buffer)?;
    result_offset += (u32::BITS / 8) as u32;
    memory.read(&store, result_offset.try_into().unwrap(), &mut len_buffer)?;
    let result_ptr = u32::from_le_bytes(ptr_buffer);
    let result_len = u32::from_le_bytes(len_buffer);
    // read the string
    let mut result_vec: Vec<u8> = vec![0; result_len as usize];
    let mut result_str_buffer = result_vec.as_mut_slice();
    memory.read(
        &store,
        result_ptr.try_into().unwrap(),
        &mut result_str_buffer,
    )?;
           // deallocate shared WASM Module memory
           let dealloc_code: i32 = wrapper_wasm_deallocate(engine, module, instance, &mut store, offset as  *const u8).unwrap();
           if dealloc_code != 0 {
               println!("Error: Could not deallocate shared WASM module memory");
           }
    let result_str: String = unsafe { String::from_utf8_lossy(&result_str_buffer).into_owned() };
    Ok(result_str.to_string())
}


/// Wrapper around the allocate function of the WASM module to allocate shared WASM memory. Allocate some memory for the application to write data for the module
/// Note: It is up to the application (and not the WASM module) to provide enough pages, so the module does not run out of memory
 /// # Arguments
/// * `size` - size of memory to allocaten
/// returns a pointer to the allocated memory area
fn wrapper_wasm_allocate(engine: &Engine, module: &Module, instance: Instance, mut store : impl AsContextMut<Data = MyState>, size: u32) -> Result<*const u8> { // Load function an instantiate it
  
    // get the function
    let func_def = instance
        .get_func(&mut store, "allocate")
        .expect("`allocate` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<u32, u32, _>(&store)?;
    // call function
    let result = func_validated.call(&mut store, size)?;
    Ok(result as *const u8)

}


///  Wrapper around the deallocate function of the WASM module to deallocate shared WASM memory. Deallocates existing memory for the purpose of the application
/// # Arguments
/// * `ptr` - mutuable pointer to the memory to deallocate
/// returns a code if it was successful or not
fn wrapper_wasm_deallocate(engine: &Engine, module: &Module, instance: Instance, mut store : impl AsContextMut<Data = MyState>, mut ptr: *const u8) -> Result<i32> {
     // get the function
    let func_def = instance
        .get_func(&mut store, "deallocate")
        .expect("`deallocate` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<u32, i32, _>(&store)?;
    // call function
    let result = func_validated.call(&mut store, ptr as u32)?;
    Ok(result)
}