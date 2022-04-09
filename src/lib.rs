use crate::context::SingleErrorStorage;
use std::hint::unreachable_unchecked;
use std::marker::PhantomData;

pub mod context;
pub mod multihandler;

/// Marker trait for error compatible types
///
/// This is blanket implemented for all types that satisfies it.
pub trait Error: 'static {}

impl<T: 'static> Error for T {}

/// The main result type
///
/// Unlike `std::result::Result` this `Result` can only hold a value, or an error flag. The error,
/// if one has occurred will be set directly at the handling scope.
///
pub struct Result<T> {
    value: core::result::Result<T, u32>,
    _not_send: PhantomData<*mut ()>,
}

impl<T> Result<T> {
    /// Create a new `Result` holding `value`
    ///
    /// # Arguments
    ///
    /// * `value`: The value held by the result.
    ///
    /// returns: Result<T>
    ///
    /// # Examples
    ///
    /// ```
    /// let x = xcept::Result::new(10);
    /// assert_eq!(x.unwrap(), 10);
    /// ```
    #[inline]
    pub fn new(value: T) -> Self {
        Self {
            value: Ok(value),
            _not_send: PhantomData,
        }
    }

    #[inline]
    pub fn new_with_error_id(id: u32) -> Self {
        Self {
            value: Err(id),
            _not_send: PhantomData,
        }
    }

    /// Create a new `Result` with an error indication.
    ///
    /// The error is not held within `Result`, but is directly assigned to the nearest handler,
    /// if one is found. If no handler is found the error is dropped.
    ///
    /// # Arguments
    ///
    /// * `err`: The error to report.
    ///
    /// returns: Result<T>
    ///
    /// # Examples
    ///
    /// ```
    /// let err: xcept::Result<i32> = xcept::Result::new_error("Error");
    /// assert!(!err.is_ok());
    /// assert!(err.is_error());
    /// ```
    #[inline]
    pub fn new_error<E: Error>(err: E) -> Self {
        let id = context::push_error(err);
        Self {
            value: Err(id),
            _not_send: PhantomData,
        }
    }

    /// Test if a `Result` contains a value.
    ///
    /// # Examples
    ///
    /// ```
    /// let err: xcept::Result<i32> = xcept::Result::new_error("Error");
    /// assert!(!err.is_ok());
    /// assert!(err.is_error());
    /// ```
    #[inline]
    pub fn is_ok(&self) -> bool {
        self.value.is_ok()
    }

    /// Convert the `Result` to an `Option<T>`
    #[inline]
    pub fn ok(self) -> Option<T> {
        self.value.ok()
    }

    /// Test if a `Result` contains an error.
    ///
    /// # Examples
    ///
    /// ```
    /// let err: xcept::Result<i32> = xcept::Result::new_error("Error");
    /// assert!(!err.is_ok());
    /// assert!(err.is_error());
    /// ```
    #[inline]
    pub fn is_error(&self) -> bool {
        self.value.is_err()
    }

    /// Unwrap the `Result` to a value, panicking if `Result` holds an error.
    ///
    /// # Panics
    ///
    /// If the `Result` doesn't contain a value we panic instead.
    #[inline]
    pub fn unwrap(self) -> T {
        self.value.unwrap()
    }

    /// Unchecked unwrap
    ///
    /// # Safety
    ///
    /// If `result.is_ok()` returns `false` this will result in *undefined behaviour*.
    #[inline]
    pub unsafe fn unwrap_unchecked(self) -> T {
        self.value.unwrap_unchecked()
    }

    /// Get the ID of the error that was set when `Result` was created.
    #[inline]
    pub fn error_id(&self) -> Option<u32> {
        match &self.value {
            Ok(_) => None,
            Err(x) => Some(*x),
        }
    }

    /// Unchecked getter of the ID of the error that was set when `Result` was created.
    ///
    /// # Safety
    ///
    /// If `result.is_error()` returns `false` this will result in *undefined behaviour*.
    #[inline]
    pub unsafe fn unchecked_error_id(&self) -> u32 {
        match &self.value {
            Err(x) => *x,
            _ => unreachable_unchecked(),
        }
    }
}

impl<T> From<T> for Result<T> {
    #[inline]
    fn from(v: T) -> Self {
        Self::new(v)
    }
}

impl<T, E: Error> From<std::result::Result<T, E>> for Result<T> {
    #[inline]
    fn from(val: std::result::Result<T, E>) -> Self {
        match val {
            Ok(v) => Self::new(v),
            Err(e) => Self::new_error(e),
        }
    }
}

/// Try to execute a function, and try to handle an error, if one occurs.
///
/// # Arguments
///
/// * `func`: The function to execute
/// * `handler`: Handle a single error
///
/// returns: Result<T>
///
/// # Examples
///
/// ```
/// fn to_int(string: &str) -> xcept::Result<i32> {
///     string.parse().into()
/// }
///
/// type ErrorT = <i32 as std::str::FromStr>::Err;
/// let res = xcept::try_or_handle_one(|| to_int("abc"), |_err: ErrorT| xcept::Result::new(-1));
/// assert_eq!(res.unwrap(), -1);
///
/// let res = xcept::try_or_handle_one(|| to_int("10"), |_err: ErrorT| xcept::Result::new(-1));
/// assert_eq!(res.unwrap(), 10);
/// ```
#[inline]
pub fn try_or_handle_one<F, H, T, E>(func: F, handler: H) -> Result<T>
where
    F: FnOnce() -> Result<T>,
    H: FnOnce(E) -> Result<T>,
    E: Error,
{
    let mut error_storage: crate::context::SingleErrorStorage<E> = SingleErrorStorage::default();
    let mut scope = context::ScopeNode::new(&mut error_storage);
    // Safety: scope is kept alive, guard is dropped before `scope` is used again
    let guard = unsafe { context::push_handling_scope(&mut scope) };
    let res = func();
    drop(guard);
    if res.is_error() {
        // Safety: res.is_error() is true
        unsafe { error_storage.unchecked_try_handle(res, handler) }
    } else {
        res
    }
}

/// Try to execute a function, and try to handle any error that happens.
///
/// Unlike `try_or_handle_one` this function can handle multiple different error types, but
/// the error handler must be built using a builder.
///
/// # Examples
///
/// ```
/// fn to_int(string: &str) -> xcept::Result<i32> {
///     if string.is_empty() {
///         xcept::Result::new_error("Empty")
///     }
///     else {
///         string.parse().into()
///     }
/// }
///
/// type ErrorT = <i32 as std::str::FromStr>::Err;
/// let handlers = xcept::multihandler::builder(|_: ErrorT| xcept::Result::new(-1))
///     .handle(|s: &str| {
///         println!("Error: {}", s);
///         xcept::Result::new(-2)
///     })
///     .build();
/// let res = xcept::try_or_handle(|| to_int("abc"), handlers.clone());
/// assert_eq!(res.unwrap(), -1);
///
/// let res = xcept::try_or_handle(|| to_int(""), handlers.clone());
/// assert_eq!(res.unwrap(), -2);
/// ```
#[inline]
pub fn try_or_handle<F, H, T>(func: F, mut handlers: H) -> Result<T>
where
    F: FnOnce() -> Result<T>,
    H: multihandler::TryHandle<Value = T> + context::ErrorHandlingContext,
{
    let mut scope = context::ScopeNode::new(&mut handlers);
    let guard = unsafe { context::push_handling_scope(&mut scope) };
    let res = func();
    drop(guard);
    if res.is_error() {
        match handlers.try_handle(unsafe { res.unchecked_error_id() }) {
            None => res,
            Some(x) => x,
        }
    } else {
        res
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    #[test]
    fn try_or_handle_one() {
        fn func() -> crate::Result<i32> {
            crate::Result::new_error(true)
        }

        let x = crate::try_or_handle_one(func, |_e: bool| 1.into()).unwrap();
        assert_eq!(x, 1);

        fn two_errors() -> crate::Result<i32> {
            crate::Result::<i32>::new_error(true);
            crate::Result::new_error(10)
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
        let res: crate::Result<i32> = crate::Result::new_error(true);
        assert!(res.is_error());
    }

    #[test]
    fn multi_handlers() {
        fn handler1(e: i32) -> crate::Result<i32> {
            (e * 2).into()
        }
        fn handler2(_e: &str) -> crate::Result<i32> {
            (-1).into()
        }
        fn handler3(_e: bool) -> crate::Result<i32> {
            crate::Result::new_error("Bool not supported!")
        }
        let handlers = crate::multihandler::builder(handler1)
            .handle(handler2)
            .handle(handler3)
            .build();

        let res = crate::try_or_handle(|| 10.into(), handlers.clone());

        assert_eq!(res.unwrap(), 10);

        let res = crate::try_or_handle(|| crate::Result::new_error(15), handlers.clone());

        assert_eq!(res.unwrap(), 30);

        let res = crate::try_or_handle(
            || crate::Result::new_error("should be -1"),
            handlers.clone(),
        );

        assert_eq!(res.unwrap(), -1);

        let res = crate::try_or_handle(|| crate::Result::new_error(false), handlers.clone());

        assert_eq!(res.is_error(), true);
    }

    #[test]
    fn multi_handlers_with_refs() {
        let which = RefCell::new(0);
        let handlers = crate::multihandler::builder(|_: i32| {
            *which.borrow_mut() = 1;
            crate::Result::new(1)
        })
        .handle(|_: &str| {
            *which.borrow_mut() = 2;
            crate::Result::new(2)
        })
        .build();

        let res = crate::try_or_handle(|| crate::Result::new_error(0), handlers.clone());
        assert_eq!(res.unwrap(), 1);
        assert_eq!(*which.borrow(), 1);

        let res = crate::try_or_handle(|| crate::Result::new_error("hello"), handlers.clone());
        assert_eq!(res.unwrap(), 2);
        assert_eq!(*which.borrow(), 2);
    }
}
