fn hello_world(x: i32) -> xcept::Result<&'static str> {
    if x % 2 == 0 {
        "Even".into()
    } else {
        xcept::Result::new_error(x)
    }
}

fn error_handler(x: i32) -> xcept::Result<&'static str> {
    println!("x: {x}");
    "Odd".into()
}

fn main() {
    let y = xcept::try_or_handle_one(|| hello_world(0), error_handler);
    println!("{}", y.unwrap());

    let y = xcept::try_or_handle_one(|| hello_world(1), error_handler);
    println!("{}", y.unwrap());
}
