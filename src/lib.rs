pub mod context;

pub struct PopContextGuard;

impl Drop for PopContextGuard {
    fn drop(&mut self) {
        unsafe { context::pop_handling_context() }
    }
}

pub struct Result<T> {
    value: Option<T>,
}

impl<T> Result<T> {
    pub fn is_ok(&self) -> bool {
        self.value.is_some()
    }

    pub fn is_error(&self) -> bool {
        self.value.is_none()
    }

    pub fn unwrap(self) -> T {
        self.value.unwrap()
    }

    pub fn report_error<E: 'static>(err: E) -> Self {
        context::push_error(err);
        Self { value: None }
    }
}

impl<T> From<T> for Result<T> {
    fn from(v: T) -> Self {
        Self { value: Some(v) }
    }
}

impl<T, E: 'static> From<std::result::Result<T, E>> for Result<T> {
    fn from(val: std::result::Result<T, E>) -> Self {
        match val {
            Ok(v) => Self { value: Some(v) },
            Err(e) => Result::report_error(e),
        }
    }
}

pub fn try_and_handle<F, FErr, T, HE: 'static>(func: F, err_handler: FErr) -> Result<T>
where
    F: FnOnce() -> Result<T>,
    FErr: FnOnce(HE) -> Result<T>,
{
    let mut error_storage: Option<HE> = None;
    unsafe { context::push_handling_context(&mut error_storage) };
    let guard = PopContextGuard; // Use a guard in case func() panics
    let res = func();
    drop(guard);
    if let Some(err) = error_storage.take() {
        context::error_handled();
        return err_handler(err);
    }
    return res;
}

#[cfg(test)]
mod tests {
    use crate::context::error_count;

    #[test]
    fn try_and_handle() {
        crate::context::reset();
        enum Err1 {
            Err,
        }

        fn err_func() -> crate::Result<i32> {
            crate::Result::report_error(Err1::Err)
        }

        fn handler(_e: Err1) -> crate::Result<i32> {
            1.into()
        }

        let res = crate::try_and_handle(err_func, handler);

        assert_eq!(res.unwrap(), 1);
        assert_eq!(error_count(), 0);
    }

    #[test]
    fn try_and_handle_nested() {
        crate::context::reset();
        enum Err1 {
            Err,
        }

        fn err_func1() -> crate::Result<i32> {
            crate::Result::report_error(Err1::Err)
        }

        fn err_func2() -> crate::Result<i32> {
            err_func1()
        }

        fn handler(_: Err1) -> crate::Result<i32> {
            1.into()
        }

        let res = crate::try_and_handle(err_func2, handler);

        assert_eq!(res.unwrap(), 1);
        assert_eq!(error_count(), 0);
    }

    #[test]
    fn try_and_handle_nested2() {
        crate::context::reset();
        enum Err1 {
            Err,
        }

        enum Err2 {
            Err,
        }

        fn err_func1() -> crate::Result<i32> {
            crate::Result::report_error(Err1::Err)
        }

        fn err_func2() -> crate::Result<i32> {
            fn my_handler(_: Err2) -> crate::Result<i32> {
                2.into()
            }
            crate::try_and_handle(err_func1, my_handler)
        }

        fn handler(_: Err1) -> crate::Result<i32> {
            1.into()
        }

        let res = crate::try_and_handle(err_func2, handler);

        assert_eq!(res.unwrap(), 1);
        assert_eq!(error_count(), 0);
    }

    #[test]
    fn try_and_handle_nested3() {
        crate::context::reset();
        enum Err1 {
            Err(i32),
        }

        enum Err2 {
            Err(i32),
        }

        fn err_func1() -> crate::Result<i32> {
            crate::Result::report_error(Err1::Err(2))
        }

        fn err_func2() -> crate::Result<i32> {
            fn my_handler(x: Err1) -> crate::Result<i32> {
                match x {
                    Err1::Err(x) => crate::Result::report_error(Err2::Err(x)),
                }
            }
            crate::try_and_handle(err_func1, my_handler)
        }

        let mut handler2_called = false;

        let handler = |e: Err2| {
            handler2_called = true;
            match e {
                Err2::Err(x) => x.into(),
            }
        };
        let res = crate::try_and_handle(err_func2, handler);

        assert_eq!(res.unwrap(), 2);
        assert_eq!(error_count(), 0);
    }

    #[test]
    #[should_panic]
    fn unhandled_error() {
        crate::try_and_handle(
            || -> crate::Result<i32> { crate::Result::report_error(10) },
            |_: ()| 0.into(),
        );
    }

    #[test]
    fn drop_scope_on_panic() {
        let x = std::panic::catch_unwind(|| {
            crate::try_and_handle(
                || -> crate::Result<i32> { crate::Result::report_error(10) },
                |_: ()| 0.into(),
            );
        });

        assert!(x.is_err());
        assert_eq!(crate::context::scopes_count(), 0);
    }
}
