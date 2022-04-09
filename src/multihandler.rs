use crate::context::{ErrorHandlingContext, ReportedError, TrySetErrorResult};
use crate::SingleErrorStorage;

pub trait TryHandle
{
    type Value;
    fn try_handle(self, error_id: u32) -> Option<crate::Result<Self::Value>>;
}

#[derive(Copy, Clone)]
pub struct BoundHandler<E, H> {
    storage: SingleErrorStorage<E>,
    handler: H,
}

impl<E, H> BoundHandler<E, H> {
    pub fn new(handler: H) -> Self {
        Self {
            storage: SingleErrorStorage::default(),
            handler,
        }
    }
}

impl<E, H, V> TryHandle for BoundHandler<E, H>
where
    H: FnOnce(E) -> crate::Result<V> {
    type Value = V;
    fn try_handle(self, error_id: u32) -> Option<crate::Result<V>> {
        match self.storage.into_inner() {
            Some((id, err)) if id == error_id => Some((self.handler)(err)),
            _ => None,
        }
    }
}

impl<E, H> ErrorHandlingContext for BoundHandler<E, H>
where
    E: crate::Error,
{
    unsafe fn try_set_error(&mut self, error: &ReportedError) -> TrySetErrorResult {
        self.storage.try_set_error(error)
    }
}

#[derive(Copy, Clone)]
pub struct Sequence<Left, Right> {
    left: Left,
    right: Right,
}

impl<Left, Right> ErrorHandlingContext for Sequence<Left, Right>
where
    Left: ErrorHandlingContext,
    Right: ErrorHandlingContext,
{
    unsafe fn try_set_error(&mut self, error: &ReportedError) -> TrySetErrorResult {
        match self.left.try_set_error(error) {
            TrySetErrorResult::NotHandled => self.right.try_set_error(error),
            x => x,
        }
    }
}

impl<Left, Right> TryHandle for Sequence<Left, Right>
where
    Left: TryHandle,
    Right: TryHandle<Value = Left::Value>,
{
    type Value = Left::Value;
    fn try_handle(self, error_id: u32) -> Option<crate::Result<Self::Value>> {
        match self.left.try_handle(error_id) {
            None => self.right.try_handle(error_id),
            x => x
        }
    }
}

#[derive(Copy, Clone)]
pub struct Builder<T>(T);

impl<T> Builder<T>
where
    T: TryHandle + ErrorHandlingContext
{
    /// Add a new error handler to the builder.
    ///
    /// # Arguments
    ///
    /// * `handler`: The error handler to add
    ///
    /// returns: [`Builder<Sequence<T, BoundHandler<E, H>>>`]
    ///
    /// # Examples
    ///
    /// ```
    /// let _handlers = xcept::builder(|_err: std::io::Error| -1.into())
    ///     .handle(|_err: std::str::Utf8Error| -2.into())
    ///     .build(); // A handler that can handle both std::io::Error and std::str::Utf8Error
    /// ```
    pub fn handle<H, E>(self, handler: H) -> Builder<Sequence<T, BoundHandler<E, H>>>
    where
        H: FnOnce(E) -> crate::Result<T::Value>
    {
        Builder(Sequence {
            left: self.0,
            right: BoundHandler::<E, H>::new(handler),
        })
    }

    /// Convert the builder to a handling context.
    ///
    /// The handling context is suitable for usage by [`try_or_handle`].
    ///
    /// See [`try_or_handle`] for more information.
    ///
    pub fn build(self) -> T {
        self.0
    }
}

/// Create a builder that to build a handler for use with [`try_or_handle`]
///
/// [`try_or_handle`]: crate::try_or_handle
///
/// # Arguments
///
/// * `handler`: The first handler to add to the builder
///
/// returns: [`Builder<BoundHandler<E, T>>`]
///
/// # Examples
///
/// ```
/// let _handlers = xcept::builder(|_err: std::io::Error| -1.into())
///     .handle(|_err: std::str::Utf8Error| -2.into())
///     .build(); // A handler that can handle both std::io::Error and std::str::Utf8Error
/// ```
pub fn builder<T, E, V>(handler: T) -> Builder<BoundHandler<E, T>>
where
    T: FnOnce(E) -> crate::Result<V>
{
    Builder(BoundHandler::<E, T>::new(handler))
}

/// Try to execute a function, and try to handle any error that happens.
///
/// Unlike [`try_or_handle_one`] this function can handle multiple different error types, but
/// the error handler must be built using a [builder].
///
/// [`try_or_handle_one`]: crate::try_or_handle_one
/// [builder]: builder
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
pub fn try_or_handle<F, H, T>(func: F, mut handlers: H) -> crate::Result<T>
    where
        F: FnOnce() -> crate::Result<T>,
        H: TryHandle<Value = T> + crate::context::ErrorHandlingContext,
{
    let mut scope = crate::context::ScopeNode::new(&mut handlers);
    let guard = unsafe { crate::context::push_handling_scope(&mut scope) };
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
