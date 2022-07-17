//!  mostly adapted from: https://docs.rs/wasmtime/latest/wasmtime/
use anyhow::Result;
use wasmtime::Engine;
use wasmtime::Linker;
use wasmtime::Module;
use wasmtime::Instance;
use wasmtime::Store;
use wasmtime_wasi::{WasiCtx, sync::WasiCtxBuilder};


use std::ffi::CString;

use std::ffi::CStr;

struct MyState {
    wasi: WasiCtx
}

/// Main function that loads a WASM module 
fn main() {
    println!("Initializing WASM engine...");
    let engine: Engine = init_wasm_engine().unwrap();
    println!("Loading WASM module...");
    let module: Module = init_wasm_module(&engine).unwrap();
    println!("Running WASM function answer...");
    let result_answer = wrapper_answer(&engine, &module).unwrap();
    println!("Result from WASM function \"answer\": {}",result_answer);
    println!("Running WASM function c_format_hello_world...");
    let result_c_format_hello_world = wrapper_wasm_memory_c_format_hello_world (&engine, &module,  "Rust".to_string()).unwrap();
    println!("Result from WASM function \"c_format_hello_world\": {}",result_c_format_hello_world);
}


/// Init the WASM Engine
/// returns the WASM engine
fn init_wasm_engine() -> Result<Engine>{
    // Create an "Engine" to run wasm modules
    let engine = Engine::default();
    Ok(engine)
}


/// Initialize WASM module
/// # Arguments
/// * `engine` - wasmtime engine to use for the store
/// * `store` - in-memory store to use to exchange data with the function
/// returns the module
fn init_wasm_module(engine: &Engine) -> Result<Module>  {
    // load WASM module
    let module = Module::from_file(&engine, "../../../wasm-module1/target/wasm32-wasi/debug/wasm_module1.wasm")?;
    Ok(module)
}


/// Wrapper around the function answer of the WASM Module. This is needed as the standardization of the componennt model and webassembly interface types is still work-in-progress
/// # Arguments (note the function `answer` of the WASM module itself has no parameters. The parameters are just to initialize the runtime environment)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// returns the result of the function `answer`
fn wrapper_answer(engine: &Engine, module: &Module) -> Result<i32> {
    // Load function an instantiate it
    let mut linker =  Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |state: &mut MyState| &mut state.wasi)?;
    // store to exchange data with the WASM module
    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_args()?
        .build();
    let mut store = Store::new(&engine,MyState{wasi: wasi});
    // instantiate module
   // let instance = Instance::new(&mut store, &module, &[])?;
    linker.module(&mut store, "", &module)?;
    let instance: Instance= linker.instantiate(&mut store, &module).unwrap();
    // get the function
    let func_def = instance.get_func(&mut store, "answer")
    .expect("`answer` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<(), i32, _>(&store)?;
    // call function
    let result = func_validated.call(&mut store, ())?;
    Ok(result)
}


/// Wrapper around the function format_hello_world of the WASM Module. This is needed as the standardization of the componennt model and webassembly interface types is still work-in-progress
/// # Arguments (note the function `format_hello_world` of the WASM module itself has just one parameter: `func_name`. The pther parameters are just to initialize the runtime environment)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// * `func_name` - Parameter `name` for the function
/// returns the result of the function `format_hello_world`
fn wrapper_wasm_memory_c_format_hello_world(engine: &Engine, module: &Module,  func_name: String) -> Result<String> {
    // Load function an instantiate it
    let mut linker =  Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |state: &mut MyState| &mut state.wasi)?;
    // store to exchange data with the WASM module
    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_args()?
        .build();
    let mut store = Store::new(&engine,MyState{wasi: wasi});
    // instantiate module
   // let instance = Instance::new(&mut store, &module, &[])?;
    linker.module(&mut store, "", &module)?;
    let instance: Instance= linker.instantiate(&mut store, &module).unwrap();
    // get the function
    let func_def = instance.get_func(&mut store, "wasm_memory_c_format_hello_world")
    .expect("`wasm_memory_c_format_hello_world` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<i32, i32, _>(&store)?;

    // prepare handing over CString as input
     // instantiate memory
     let memory = instance
     .get_memory(&mut store, "memory")
     .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
     let param_name_str = func_name;
     let param_name_cstring: CString = CString::new(param_name_str).unwrap();
     let offset: i32 = 0;
     memory.write(&mut store, offset.try_into().unwrap(), param_name_cstring.as_bytes());
    // call function answer
    let mut result_offset = func_validated.call(&mut store, offset)?;
   
    // read answer
    let mut buffer = [1u8; 1];
    let mut result_v_u8: Vec<u8> = Vec::new();
    while buffer[0]!=0u8 {
        memory.read(&store, result_offset.try_into().unwrap(), &mut buffer)?;
        result_v_u8.push(buffer[0]);
        result_offset+=1;
    }
    // convert answer
     let c_str: &CStr = unsafe {CStr::from_ptr(result_v_u8.as_ptr() as *const i8)};
    let result_str: &str = c_str.to_str().unwrap();
    Ok(result_str.to_string())
} 