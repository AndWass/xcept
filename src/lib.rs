use std::hint::unreachable_unchecked;
use std::marker::PhantomData;

pub mod context;

pub trait Error: 'static {}

impl<T: 'static> Error for T {}

enum InnerResult<T> {
    Value(T),
    Error(u32),
}

pub struct Result<T> {
    value: InnerResult<T>,
    _not_send: PhantomData<*mut ()>,
}

impl<T> Result<T> {
    pub fn is_ok(&self) -> bool {
        matches!(&self.value, InnerResult::Value(_))
    }

    pub fn is_error(&self) -> bool {
        !self.is_ok()
    }

    pub fn unwrap(self) -> T {
        match self.value {
            InnerResult::Value(v) => v,
            InnerResult::Error(_) => panic!("Unwrapping an error")
        }
    }

    pub fn error_id(&self) -> Option<u32> {
        match &self.value {
            InnerResult::Value(_) => None,
            InnerResult::Error(id) => Some(*id),
        }
    }

    pub unsafe fn unchecked_error_id(&self) -> u32 {
        match &self.value {
            InnerResult::Error(x) => *x,
            _ => unreachable_unchecked(),
        }
    }

    pub fn report_error<E: Error>(err: E) -> Self {
        let id = context::push_error(err);
        Self { value:  InnerResult::Error(id), _not_send: PhantomData }
    }
}

impl<T> From<T> for Result<T> {
    fn from(v: T) -> Self {
        Self { value: InnerResult::Value(v), _not_send: PhantomData }
    }
}

impl<T, E: Error> From<std::result::Result<T, E>> for Result<T> {
    fn from(val: std::result::Result<T, E>) -> Self {
        match val {
            Ok(v) => Self { value: InnerResult::Value(v), _not_send: PhantomData },
            Err(e) => Self::report_error(e),
        }
    }
}

fn try_or_handle_one<F, H, T, E>(func: F, handler: H) -> Result<T>
where
    F: FnOnce() -> Result<T>,
    H: FnOnce(E) -> Result<T>,
    E: Error,
{
    let mut error_storage: Option<(u32, E)> = None;
    let mut scope = context::ScopeNode::new(&mut error_storage);
    // Safety: scope is kept alive, guard is dropped before `scope` is used again
    let guard = unsafe { context::push_handling_scope(&mut scope) };
    let res = func();
    drop(guard);
    if let Some(error_id) = res.error_id() {
        match error_storage {
            Some((stored_error_id, err)) if stored_error_id == error_id => handler(err),
            _ => res
        }
    }
    else {
        res
    }
}

#[cfg(test)]
mod tests
{
    #[test]
    fn try_or_handle_one() {
        fn func() -> crate::Result<i32> {
            crate::Result::report_error(true)
        }

        let x = crate::try_or_handle_one(func, |_e: bool| {
            1.into()
        }).unwrap();
        assert_eq!(x, 1);

        fn two_errors() -> crate::Result<i32> {
            crate::Result::<i32>::report_error(true);
            crate::Result::report_error(10)
        }

        let mut called = false;
        let x = crate::try_or_handle_one(two_errors, |_: bool| {
            called = true;
            2.into()
        });

        assert!(!called);
        assert!(x.is_error());
    }

    #[test]
    fn error_without_scopes() {
        let res: crate::Result<i32> = crate::Result::report_error(true);
        assert!(res.is_error());
    }
}