# except

A very early Rust proof of concept inspired by [`boost.LEAF`].

  [`boost.LEAF`]: https://www.boost.org/doc/libs/1_78_0/libs/leaf/doc/html/index.html

## Hello world

```rust
#[derive(Debug)]
struct CustomError(i32);

fn hello_world(x: i32) -> xcept::Result<&'static str> {
    // This function reports two completely different error types, apart from the value type
    if x % 2 == 0 {
        "Even".into()
    } else if x == 1 {
        xcept::Result::new_error(CustomError(x))
    }
    else {
        xcept::Result::new_error(x)
    }
}

fn custom_error_handler(err: CustomError) -> xcept::Result<&'static str> {
    println!("{err:?}");
    "One".into()
}
fn error_handler(x: i32) -> xcept::Result<&'static str> {
    println!("x: {x}");
    "Odd".into()
}

fn main() {
    let y = xcept::try_or_handle_one(|| hello_world(0), error_handler);
    println!("{}", y.unwrap());

    let y = xcept::try_or_handle_one(|| hello_world(1), custom_error_handler);
    println!("{}", y.unwrap());

    let y = xcept::try_or_handle_one(|| hello_world(3), error_handler);
    println!("{}", y.unwrap());
}
```
