//!  mostly adapted from: https://docs.rs/wasmtime/latest/wasmtime/
use anyhow::Result;
use wasmtime::Engine;
use wasmtime::ExternRef;
use wasmtime::Module;
use wasmtime::Instance;
use wasmtime::Store;


/// Main function that loads a WASM module 
fn main() {
    println!("Initializing WASM engine...");
    let engine: Engine = init_wasm_engine().unwrap();
    println!("Loading WASM module...");
    let module: Module = init_wasm_module(&engine).unwrap();
    println!("Running WASM function answer...");
    let result_answer = wrapper_answer(&engine, &module).unwrap();
    println!("Result from WASM function \"answer\": {}",result_answer);
    println!("Running WASM function format_hello_world...");
    let result_format_hello_world = wrapper_format_hello_world (&engine, &module,  "Rust".to_string()).unwrap();
    println!("Result from WASM function \"format_hello_world\": {}",result_format_hello_world);
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
    // store to exchange data with the WASM module
    let mut store = Store::new(&engine,());
    // instantiate module
    let instance = Instance::new(&mut store, &module, &[])?;
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
fn wrapper_format_hello_world(engine: &Engine, module: &Module, func_name: String) -> Result<String> {

      // store to exchange data with the WASM module, we put in the data the func_name
      let mut store = Store::new(&engine,());
    // instantiate module
    let instance = Instance::new(&mut store, &module, &[])?;
    // get the function
    let func_def = instance.get_func(&mut store, "format_hello_world")
    .expect("`format_hello_world` was not an exported function");

    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<(i32,i32), (), _>(&store)?;
    // call function answer
   // let result = func_validated.call(&mut store, Some(ExternRef::new(func_name)))?;
    Ok("".to_string())
} 