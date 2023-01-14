//!  mostly adapted from: https://docs.rs/wasmtime/latest/wasmtime/
use anyhow;
use wasmtime::AsContextMut;
use wasmtime::Engine;
use wasmtime::Instance;
use wasmtime::Linker;
use wasmtime::Module;
use wasmtime::Store;
use wasmtime_wasi::{sync::WasiCtxBuilder, WasiCtx};

use std::ffi::CStr;
use std::ffi::CString;
use std::sync::Arc;

use arrow::array::{
    Array, Float64Array, StringArray, StructArray, TimestampSecondArray, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use arrow::util::pretty::print_batches;

use time::macros::datetime;

struct MyState {
    wasi: WasiCtx,
}

/// Main function that loads a WASM module
fn main() {
    println!("Initializing WASM engine...");
    let engine: Engine = init_wasm_engine().unwrap();
    println!("Loading WASM module 1...");
    let module: Module = init_wasm_module_1(&engine).unwrap();
    println!("Module1: Running WASM function answer...");
    let result_answer = wrapper_answer(&engine, &module).unwrap();
    println!("Result from WASM function \"answer\": {}", result_answer);
    println!("Module 1: Running WASM function c_format_hello_world...");
    let result_c_format_hello_world =
        wrapper_wasm_c_format_hello_world(&engine, &module, "Rust (C ABI)").unwrap();
    println!(
        "Result from WASM function \"c_format_hello_world\": {}",
        result_c_format_hello_world
    );
    println!("Module 1: Running WASM function rust_format_hello_world...");
    let result_rust_format_hello_world =
        wrapper_wasm_rust_format_hello_world(&engine, &module, "Rust (Rust ABI)".to_string())
            .unwrap();
    println!(
        "Result from WASM function \"rust_format_hello_world\": {}",
        result_rust_format_hello_world
    );
    println!("Loading WASM module 2...");
    let module: Module = init_wasm_module_2(&engine).unwrap();
    println!("Module 2: Running WASM function arrow_process_document...");
    wrapper_wasm_process_data_arrow(&engine, &module).unwrap();
}

/// Init the WASM Engine
/// returns the WASM engine
fn init_wasm_engine() -> anyhow::Result<Engine> {
    // Create an "Engine" to run wasm modules
    let engine = Engine::default();
    Ok(engine)
}

/// Initialize WASM module 1
/// # Arguments
/// * `engine` - wasmtime engine to use for the store
/// * `store` - in-memory store to use to exchange data with the function
/// returns the module
fn init_wasm_module_1(engine: &Engine) -> anyhow::Result<Module> {
    // load WASM module
    let module = Module::from_file(
        &engine,
        "../../../wasm-module1/target/wasm32-wasi/release/wasm_module1.wasm",
    )?;
    Ok(module)
}

/// Initialize WASM module 2
/// # Arguments
/// * `engine` - wasmtime engine to use for the store
/// * `store` - in-memory store to use to exchange data with the function
/// returns the module
fn init_wasm_module_2(engine: &Engine) -> anyhow::Result<Module> {
    // load WASM module
    let module = Module::from_file(
        &engine,
        "../../../wasm-module2/target/wasm32-wasi/release/wasm_module2.wasm",
    )?;
    Ok(module)
}

/// Wrapper around the function answer of the WASM Module. This is needed as the standardization of the componennt model and webassembly interface types is still work-in-progress
/// # Arguments (note the function `answer` of the WASM module itself has no parameters. The parameters are just to initialize the runtime environment)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// returns the result of the function `answer`
fn wrapper_answer(engine: &Engine, module: &Module) -> anyhow::Result<i32> {
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
    let func_validated = func_def.typed::<(), i32>(&store)?;
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
) -> anyhow::Result<String> {
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
    let offset: u32 = wrapper_wasm_allocate(
        instance,
        &mut store,
        param_name_cstring_as_bytes.len() as u32,
    )
    .unwrap() as u32;

    // get the function
    let func_def = instance
        .get_func(&mut store, "wasm_memory_c_format_hello_world")
        .expect("`wasm_memory_c_format_hello_world` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<u32, i32>(&store)?;

    // prepare handing over CString as input
    // instantiate memory
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
    memory
        .write(
            &mut store,
            offset.try_into().unwrap(),
            param_name_cstring_as_bytes,
        )
        .unwrap();
    // call function answer
    let result_offset = func_validated.call(&mut store, offset)?;
    if result_offset == 0 {
        anyhow::bail!("Error: No valid answer received from function")
    } else {
        let mut result_offset_position = result_offset;
        // read answer
        let mut buffer = [1u8; 1];
        let mut result_v_u8: Vec<u8> = Vec::new();
        while buffer[0] != 0u8 {
            memory.read(
                &store,
                result_offset_position.try_into().unwrap(),
                &mut buffer,
            )?;
            result_v_u8.push(buffer[0]);
            result_offset_position += 1;
        }
        // deallocate shared WASM Module memory
        let dealloc_param_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, offset as *const u8).unwrap();
        if dealloc_param_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for parameter");
        }
        let dealloc_return_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, result_offset as *const u8).unwrap();
        if dealloc_return_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for result");
        }
        // convert answer
        let c_str: &CStr = unsafe { CStr::from_ptr(result_v_u8.as_ptr() as *const i8) };
        let result_str: &str = c_str.to_str().unwrap();
        Ok(result_str.to_string())
    }
}

/// Wrapper around the function format_hello_world (Rust ABI) of the WASM Module. This is needed as the standardization of the component model and webassembly interface types is still work-in-progress
/// # Arguments (note the function `format_hello_world` of the WASM module itself has just one parameter: `func_name`. The other parameters are just to initialize the runtime environment)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// * `func_name` - Parameter `name` for the function
/// returns the result of the function `format_hello_world`
fn wrapper_wasm_rust_format_hello_world(
    engine: &Engine,
    module: &Module,
    func_name: String,
) -> anyhow::Result<String> {
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
    let func_validated = func_def.typed::<(u32, u32), u32>(&store)?;

    // prepare handing over Rust as input
    // instantiate memory
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
    let param_name_str: String = func_name.to_string();
    let param_name_string_as_bytes: &[u8] = param_name_str.as_bytes();
    // allocate some memory within the WASM module
    let offset: u32 = wrapper_wasm_allocate(
        instance,
        &mut store,
        param_name_string_as_bytes.len() as u32,
    )
    .unwrap() as u32;
    let length: u32 = param_name_str.len() as u32;
    memory
        .write(
            &mut store,
            offset.try_into().unwrap(),
            param_name_string_as_bytes,
        )
        .unwrap();
    // call function answer
    let result_offset = func_validated.call(&mut store, (offset, length))?;
    if result_offset == 0 {
        anyhow::bail!("Error: No valid answer received from function")
    } else {
        let mut result_offset_position = result_offset;
        // read answer from memory: these are two values: offset and length of the return string
        // read metadata (offset and length of the sring)
        // note: WebAssembly is by default 32 bit
        let mut ptr_buffer = [0u8; (u32::BITS / 8) as usize];
        let mut len_buffer = [0u8; (u32::BITS / 8) as usize];
        memory.read(
            &store,
            result_offset_position.try_into().unwrap(),
            &mut ptr_buffer,
        )?;
        result_offset_position += (u32::BITS / 8) as u32;
        memory.read(
            &store,
            result_offset_position.try_into().unwrap(),
            &mut len_buffer,
        )?;
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
        let dealloc_param_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, offset as *const u8).unwrap();
        if dealloc_param_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for parameter");
        }
        let dealloc_return_meta_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, result_offset as *const u8).unwrap();
        if dealloc_return_meta_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for return metadata");
        }
        let dealloc_return_data_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, result_ptr as *const u8).unwrap();
        if dealloc_return_data_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for return data");
        }
        let result_str: String = String::from_utf8_lossy(&result_str_buffer).into_owned();
        Ok(result_str.to_string())
    }
}

/// Wrapper around the function process_data_arrow (Use Arrow for cross-programming language data serialization) of the WASM Module.
/// # Arguments (note the function `process_data_arrow` of the WASM module itself expects to have the Arrow data exchanged in the module memory. The Arrow data is generated in this application through the functions create_arrow_example_meta_data (instructing the function what to do with the data) and create_arrow_example_data (containing the data to be processed)
/// * `engine` - wasmtime engine to use for the store
/// * `module` - module containing the WASM function
/// returns the result of the function `format_hello_world`
fn wrapper_wasm_process_data_arrow(engine: &Engine, module: &Module) -> anyhow::Result<String> {
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
        .get_func(&mut store, "wasm_memory_process_data_arrow")
        .expect("`wasm_memory_process_data_arrow` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<(u32, u32, u32, u32), u32>(&store)?;

    // prepare handing Arrow data
    let serialized_meta_data = create_arrow_example_meta_data();
    let serialized_meta_data_size = serialized_meta_data.len();
    let serialized_data = create_arrow_example_data();
    let serialized_data_size = serialized_data.len();

    // instantiate memory
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(anyhow::format_err!("failed to find `memory` export"))?;

    // allocate some memory within the WASM module for metadata
    let offset_meta_data: u32 =
        wrapper_wasm_allocate(instance, &mut store, serialized_meta_data_size as u32).unwrap()
            as u32;
    memory
        .write(
            &mut store,
            offset_meta_data.try_into().unwrap(),
            serialized_meta_data.as_slice(),
        )
        .unwrap();
    // allocate some memory within the WASM module for data
    let offset_data: u32 =
        wrapper_wasm_allocate(instance, &mut store, serialized_data_size as u32).unwrap() as u32;
    memory
        .write(
            &mut store,
            offset_data.try_into().unwrap(),
            serialized_data.as_slice(),
        )
        .unwrap();
    // call function answer
    let result_offset = func_validated.call(
        &mut store,
        (
            offset_meta_data,
            serialized_meta_data_size as u32,
            offset_data,
            serialized_data_size as u32,
        ),
    )?;
    if result_offset == 0 {
        anyhow::bail!("Error: No valid answer received from function")
    } else {
        let mut result_offset_position = result_offset;
        // read answer from memory: these are two values: offset of the processed data and size of the processed data in Arrow IPC format
        // read metadata (offset and size of the Arrow IPC data)
        // note: WebAssembly is by default 32 bit
        let mut ptr_buffer = [0u8; (u32::BITS / 8) as usize];
        let mut len_buffer = [0u8; (u32::BITS / 8) as usize];
        memory.read(
            &store,
            result_offset_position.try_into().unwrap(),
            &mut ptr_buffer,
        )?;
        result_offset_position += (u32::BITS / 8) as u32;
        memory.read(
            &store,
            result_offset_position.try_into().unwrap(),
            &mut len_buffer,
        )?;
        let result_ptr = u32::from_le_bytes(ptr_buffer);
        let result_len = u32::from_le_bytes(len_buffer);
        // read the Arrow IPC data
        let mut result_arrow_ipc: Vec<u8> = vec![0; result_len as usize];
        let mut result_arrow_ipc_buffer = result_arrow_ipc.as_mut_slice();
        memory.read(
            &store,
            result_ptr.try_into().unwrap(),
            &mut result_arrow_ipc_buffer,
        )?;
        // deallocate shared WASM Module memory
        let dealloc_meta_data_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, offset_meta_data as *const u8).unwrap();
        if dealloc_meta_data_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for meta data");
        }
        let dealloc_data_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, offset_data as *const u8).unwrap();
        if dealloc_data_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for data");
        }
        let dealloc_return_meta_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, result_offset as *const u8).unwrap();
        if dealloc_return_meta_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for return metadata");
        }
        let dealloc_return_data_code: i32 =
            wrapper_wasm_deallocate(instance, &mut store, result_ptr as *const u8).unwrap();
        if dealloc_return_data_code != 0 {
            println!("Error: Could not deallocate shared WASM module memory for return data");
        }
        // check correctness of returned Arrow IPC data
        println!("Displaying Arrow answer from Module");
        let stream_reader = StreamReader::try_new(result_arrow_ipc.as_slice(), None).unwrap();

        for item in stream_reader {
            print_batches(&[item.unwrap()]).unwrap();
        }
        Ok("".to_string())
    }
}

/// Wrapper around the allocate function of the WASM module to allocate shared WASM memory. Allocate some memory for the application to write data for the module
/// Note: It is up to the application (and not the WASM module) to provide enough pages, so the module does not run out of memory
/// # Arguments
/// * `size` - size of memory to allocaten
/// returns a pointer to the allocated memory area
fn wrapper_wasm_allocate(
    instance: Instance,
    mut store: impl AsContextMut<Data = MyState>,
    size: u32,
) -> anyhow::Result<*const u8> {
    // Load function an instantiate it

    // get the function
    let func_def = instance
        .get_func(&mut store, "wasm_allocate")
        .expect("`wasm_allocate` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<u32, u32>(&store)?;
    // call function
    let result = func_validated.call(&mut store, size)?;
    Ok(result as *const u8)
}

///  Wrapper around the deallocate function of the WASM module to deallocate shared WASM memory. Deallocates existing memory for the purpose of the application
/// # Arguments
/// * `ptr` - mutuable pointer to the memory to deallocate
/// returns a code if it was successful or not
fn wrapper_wasm_deallocate(
    instance: Instance,
    mut store: impl AsContextMut<Data = MyState>,
    ptr: *const u8,
) -> anyhow::Result<i32> {
    // get the function
    let func_def = instance
        .get_func(&mut store, "wasm_deallocate")
        .expect("`wasm_deallocate` was not an exported function");
    // validate that it corresponds to the parameters and return types we need
    let func_validated = func_def.typed::<u32, i32>(&store)?;
    // call function
    let result = func_validated.call(&mut store, ptr as u32)?;
    Ok(result)
}

/// Create example data
/// {id: 1, content: "this is a test", title: "test",date:"2022-01-01T12:00:00Z", score: 1.77}
/// returns a binary representation of the data in Arrow IPC format
fn create_arrow_example_data() -> Vec<u8> {
    // define schema
    let schema = Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("title", DataType::Utf8, false),
        Field::new(
            "date",
            DataType::Timestamp(TimeUnit::Second, Some("+00:00".to_string())),
            false,
        ),
        Field::new("score", DataType::Float64, false),
    ]);
    let ids = UInt64Array::from(vec![1]);
    let contents = StringArray::from(vec!["this is a test"]);
    let titles = StringArray::from(vec!["test"]);
    let dates = TimestampSecondArray::from(vec![datetime!(2022-01-01 12:00:00 UTC).unix_timestamp()]).with_timezone("+00:00".to_string());

    let scores = Float64Array::from(vec![1.123456f64]);

    // build a record batch
    let batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![
            Arc::new(ids),
            Arc::new(contents),
            Arc::new(titles),
            Arc::new(dates),
            Arc::new(scores),
        ],
    )
    .unwrap();
    // serialize it
    let buffer: Vec<u8> = Vec::new();

    let mut stream_writer = StreamWriter::try_new(buffer, &schema).unwrap();
    stream_writer.write(&batch).unwrap();

    let serialized_batch = stream_writer.into_inner().unwrap();
    return serialized_batch;
}

/// Create example meta-data, ie commands for the module on what to do with the data
/// A simple commmand structure {command: "test", config: {filename: "test.txt"}}
/// returns a binary representation of the data in Arrow IPC format
fn create_arrow_example_meta_data() -> Vec<u8> {
    // define schema
    let schema = Schema::new(vec![
        Field::new("command", DataType::Utf8, false),
        Field::new(
            "config",
            DataType::Struct(vec![Field::new("filename", DataType::Utf8, false)]),
            false,
        ),
    ]);
    // define one data item
    let command = StringArray::from(vec!["test"]);

    let config = StructArray::from(vec![(
        Field::new("filename", DataType::Utf8, false),
        Arc::new(StringArray::from(vec!["test.txt"])) as Arc<dyn Array>,
    )]);
    // build a record batch
    let batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![Arc::new(command), Arc::new(config)],
    )
    .unwrap();
    // serialize it
    let buffer: Vec<u8> = Vec::new();

    let mut stream_writer = StreamWriter::try_new(buffer, &schema).unwrap();
    stream_writer.write(&batch).unwrap();

    let serialized_batch = stream_writer.into_inner().unwrap();
    return serialized_batch;
}
