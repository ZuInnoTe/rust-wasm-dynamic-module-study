use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::Arc;

use arrow::array::{StringArray, UInt64Array};
use arrow::datatypes::{
    DataType, Field, Float64Type, Schema, TimeUnit, TimestampSecondType, UInt64Type,
};
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;

use time::macros::datetime;

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

/// A simple example function that processes data in Arrow IPC format from the WASM module memory
/// # Arguments
/// * `meta_data_offset` - position of the start of the meta data ("command") in Arrow IPC format
/// * `meta_data_size` - size of the meta data in Arrow IPC format
/// * `data_offset` - position of the start of the data ("data") in Arrow IPC format
/// * `data_size` - size of the data in Arrow IPC format
/// Returns an offset in the WASM module memory where an offset and size of the result data in Arrow IPC format are stored
#[unsafe(no_mangle)]
pub extern "C" fn wasm_memory_process_data_arrow(
    meta_data_offset: *mut u32,
    meta_data_size: u32,
    data_offset: *mut u32,
    data_size: u32,
) -> u32 {
    // validate meta data pointer
    let expected_size_meta_data: usize = validate_pointer(meta_data_offset as *const u8);
    if (expected_size_meta_data == 0) | (expected_size_meta_data != meta_data_size as usize) {
        return 0;
    }; // return if no valid allocated memory was provided
       // validate data pointer
    let expected_size_data: usize = validate_pointer(data_offset as *const u8);
    if (expected_size_data == 0) | (expected_size_data != data_size as usize) {
        return 0;
    }; // return if no valid allocated memory was provided
       // fetch from WASM module memory - meta data
    let mut input_vec_meta_data: Vec<u8> = Vec::new();
    unsafe {
        Vec::extend_from_slice(
            &mut input_vec_meta_data,
            std::slice::from_raw_parts(meta_data_offset as *mut u8, meta_data_size as usize),
        )
    };
    // fetch from WASM module memory - / data
    let mut input_vec_data: Vec<u8> = Vec::new();
    unsafe {
        Vec::extend_from_slice(
            &mut input_vec_data,
            std::slice::from_raw_parts(data_offset as *mut u8, data_size as usize),
        )
    };
    // check the meta data and data
    // deserialize the meta data
    let stream_reader_meta_data =
        StreamReader::try_new(input_vec_meta_data.as_slice(), None).unwrap();
    // check if the meta data content is as expected (ie hardcoded in app)
    for item in stream_reader_meta_data {
        let arrow_record_batch = item.unwrap();
        // validate schema
        assert_eq!(arrow_record_batch.schema().field(0).name(), "command");
        assert_eq!(
            arrow_record_batch.schema().field(0).data_type(),
            &DataType::Utf8
        );
        assert_eq!(arrow_record_batch.schema().field(1).name(), "config");
        assert_eq!(
            arrow_record_batch.schema().field(1).data_type(),
            &DataType::Struct(arrow::datatypes::Fields::from(vec![Field::new("filename", DataType::Utf8, false)]))
        );

        // validate meta_data
        assert_eq!(arrow_record_batch.num_rows(), 1);
        let first_row_command =
            arrow::array::as_string_array(arrow_record_batch.column(0)).value(0);
        assert_eq!(first_row_command, "test");
        let first_row_config =
            arrow::array::as_struct_array(arrow_record_batch.column(1)).column(0);
        let first_row_config_filename = arrow::array::as_string_array(first_row_config).value(0);
        assert_eq!(first_row_config_filename, "test.txt");
    }

    // deserialize the  data
    let stream_reader_data = StreamReader::try_new(input_vec_data.as_slice(), None).unwrap();
    // check if the  data content is as expected (ie hardcoded in app)
    for item in stream_reader_data {
        let arrow_record_batch = item.unwrap();
        // validate schema
        assert_eq!(arrow_record_batch.schema().field(0).name(), "id");
        assert_eq!(
            arrow_record_batch.schema().field(0).data_type(),
            &DataType::UInt64
        );
        assert_eq!(arrow_record_batch.schema().field(1).name(), "content");
        assert_eq!(
            arrow_record_batch.schema().field(1).data_type(),
            &DataType::Utf8
        );
        assert_eq!(arrow_record_batch.schema().field(2).name(), "title");
        assert_eq!(
            arrow_record_batch.schema().field(2).data_type(),
            &DataType::Utf8
        );
        assert_eq!(arrow_record_batch.schema().field(3).name(), "date");
        assert_eq!(
            arrow_record_batch.schema().field(3).data_type(),
            &DataType::Timestamp(TimeUnit::Second, Some("+00:00".to_string().into()))
        );
        assert_eq!(arrow_record_batch.schema().field(4).name(), "score");
        assert_eq!(
            arrow_record_batch.schema().field(4).data_type(),
            &DataType::Float64
        );
        // validate data
        assert_eq!(arrow_record_batch.num_rows(), 1);
        let first_row_id =
            arrow::array::as_primitive_array::<UInt64Type>(arrow_record_batch.column(0)).value(0);
        assert_eq!(first_row_id, 1);
        let first_row_content =
            arrow::array::as_string_array(arrow_record_batch.column(1)).value(0);
        assert_eq!(first_row_content, "this is a test");
        let first_row_title = arrow::array::as_string_array(arrow_record_batch.column(2)).value(0);
        assert_eq!(first_row_title, "test");
        let first_row_date =
            arrow::array::as_primitive_array::<TimestampSecondType>(arrow_record_batch.column(3))
                .value(0);
        assert_eq!(
            first_row_date,
            datetime!(2022-01-01 12:00:00 UTC).unix_timestamp()
        );
        let first_row_score =
            arrow::array::as_primitive_array::<Float64Type>(arrow_record_batch.column(4)).value(0);
        assert_eq!(first_row_score, 1.123456f64);
    }
    // lets generate a return answer to the processing request modifying the field content of document with id 1
    // define schema
    let schema = Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("content", DataType::Utf8, false),
    ]);
    let ids = UInt64Array::from(vec![1]);
    let contents = StringArray::from(vec!["this is a test2"]);

    // build a record batch
    let result_batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![Arc::new(ids), Arc::new(contents)],
    )
    .unwrap();
    // serialize it
    let buffer: Vec<u8> = Vec::new();

    let mut stream_writer = StreamWriter::try_new(buffer, &schema).unwrap();
    stream_writer.write(&result_batch).unwrap();

    let serialized_result_batch: Vec<u8> = stream_writer.into_inner().unwrap();
    // allocate memory for the answer
    let serialized_result_batch_alloc: ManuallyDrop<Box<[u8]>> =
        ManuallyDrop::new(serialized_result_batch.into_boxed_slice());
    let serialized_result_batch_alloc_len: usize = serialized_result_batch_alloc.len();

    let serialized_result_batch_ptr = allocate(
        serialized_result_batch_alloc_len,
        serialized_result_batch_alloc,
    );
    // return position of WASM memory where we can find a offset, length pair
    let mut vec_meta: Vec<u8> = Vec::new();
    let serialized_result_batch_ptr_array: [u8; (usize::BITS / 8) as usize] =
        (serialized_result_batch_ptr as usize).to_le_bytes();
    let serialized_result_batch_alloc_len: [u8; (usize::BITS / 8) as usize] =
        serialized_result_batch_alloc_len.to_le_bytes();
    for byte in serialized_result_batch_ptr_array {
        vec_meta.push(byte);
    }
    for byte in serialized_result_batch_alloc_len {
        vec_meta.push(byte);
    }
    let serialized_result_batch_meta: Box<[u8]> = vec_meta.into_boxed_slice();
    let serialized_result_batch_meta_len: usize = serialized_result_batch_meta.len();
    let serialized_result_batch_meta_ptr = allocate(
        serialized_result_batch_meta_len,
        ManuallyDrop::new(serialized_result_batch_meta),
    );

    return serialized_result_batch_meta_ptr as u32;
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
