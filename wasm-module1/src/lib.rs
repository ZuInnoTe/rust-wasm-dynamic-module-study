#[no_mangle]
pub extern "C" fn answer() -> i32 {
    return 42;
}


#[no_mangle]
pub extern "C" fn format_hello_world(name: &str) -> &str{
    return "Hello World!";  
}

