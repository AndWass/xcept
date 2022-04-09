#[derive(Debug, Copy, Clone)]
struct CustomError(i32);

fn hello_world(x: i32) -> xcept::Result<&'static str> {
    // This function reports two completely different error types, apart from the value type
    if x % 2 == 0 {
        "Even".into()
    } else if x == 1 {
        xcept::Result::new_error(CustomError(x))
    }
    else if x == 11 {
        xcept::Result::new_error("Prime!")
    }
    else {
        xcept::Result::new_error(x)
    }
}

fn custom_error_handler(err: CustomError) -> xcept::Result<&'static str> {
    println!("err = {err:?}");
    "One".into()
}
fn error_handler(x: i32) -> xcept::Result<&'static str> {
    println!("x: {x}");
    "Odd".into()
}

fn main() {
    let handlers = xcept::multihandler::builder(error_handler)
        .handle(custom_error_handler)
        .build();

    println!("Result = {}", xcept::try_or_handle(|| hello_world(0), handlers.clone()).unwrap());
    println!("Result = {}", xcept::try_or_handle(|| hello_world(1), handlers.clone()).unwrap());
    println!("Result = {}", xcept::try_or_handle(|| hello_world(3), handlers.clone()).unwrap());

    let res = xcept::try_or_handle(|| hello_world(11), handlers.clone());

    println!("Error ID = {:?}", res.error_id())
}
