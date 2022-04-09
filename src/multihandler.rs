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
    pub fn handle<H, E>(self, handler: H) -> Builder<Sequence<T, BoundHandler<E, H>>>
    where
        H: FnOnce(E) -> crate::Result<T::Value>
    {
        Builder(Sequence {
            left: self.0,
            right: BoundHandler::<E, H>::new(handler),
        })
    }

    pub fn build(self) -> T {
        self.0
    }
}

pub fn builder<T, E, V>(handler: T) -> Builder<BoundHandler<E, T>>
where
    T: FnOnce(E) -> crate::Result<V>
{
    Builder(BoundHandler::<E, T>::new(handler))
}
