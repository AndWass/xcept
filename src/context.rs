use std::any::TypeId;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::thread_local;

pub struct ReportedError
{
    pub id: u32,
    pub type_id: TypeId,
    pub value: *mut (),
}

/// The result of `ErrorHandlingContext.try_set_error`
///
/// The handling of the value will differ based on the returned value.
pub enum TrySetErrorResult
{
    /// The error was unhandled
    NotHandled,
    /// The error was handled, the caller must `forget` the actual error
    NeedForget,
    /// The error was handled, the caller must `drop` the actual error
    NeedDrop,
}

pub trait ErrorHandlingContext: 'static
{
    /// Try to store the error into a handling context
    ///
    /// If this error handling context can handle the type described by `type_id` then it should
    /// read the data from the pointer and store it internally, and return true.
    ///
    /// ## Safety
    ///
    ///   * The `TypeId` of the actual type of the pointer that `error.value` points to must match
    ///     `error.type_id`.
    ///   * If this function returns true, the caller must ensure to `forget` the original value
    ///     since it is effectively moved to some other location.
    unsafe fn try_set_error(&mut self, error: &ReportedError) -> TrySetErrorResult;
}

impl<T: crate::Error> ErrorHandlingContext for Option<(u32, T)>
{
    unsafe fn try_set_error(&mut self, error: &ReportedError) -> TrySetErrorResult {
        if TypeId::of::<T>() == error.type_id {
            *self = Some((error.id, (error.value as *mut T).read()));
            TrySetErrorResult::NeedForget
        }
        else {
            TrySetErrorResult::NotHandled
        }
    }
}

struct CatchAllContext
{
    pub inner: Option<(u32, TypeId)>
}

impl ErrorHandlingContext for CatchAllContext {
    unsafe fn try_set_error(&mut self, error: &ReportedError) -> TrySetErrorResult {
        self.inner = Some((error.id, error.type_id));
        TrySetErrorResult::NeedDrop
    }
}

struct HandlingScopes
{
    error_id: u32,
    scopes: VecDeque<HandlingScope>
}

impl HandlingScopes {
    fn new() -> Self {
        Self {
            error_id: 0,
            scopes: VecDeque::new()
        }
    }
}

struct HandlingScope
{
    context: *mut dyn ErrorHandlingContext
}

impl HandlingScope {
    fn new(context: &mut dyn ErrorHandlingContext) -> Self {
        Self {
            context
        }
    }

    unsafe fn context(&mut self) -> &mut dyn ErrorHandlingContext {
        &mut *self.context
    }
}

thread_local! {
    static CONTEXTS: RefCell<HandlingScopes> = RefCell::new(HandlingScopes::new());
}

pub unsafe fn push_handling_context(handling_context: &mut dyn ErrorHandlingContext) {
    CONTEXTS.with(|contexts| {
        let mut ctx = contexts.borrow_mut();
        ctx.scopes.push_front(HandlingScope::new(handling_context));
    });
}

pub unsafe fn pop_handling_context() {
    CONTEXTS.with(|contexts| {
        let mut ctx = contexts.borrow_mut();
        ctx.scopes.pop_front();
    })
}

pub struct PopHandlingContextGuard;

impl Drop for PopHandlingContextGuard {
    fn drop(&mut self) {
        unsafe { pop_handling_context() };
    }
}

pub fn push_error<E: crate::Error>(mut err: E) -> u32 {
    CONTEXTS.with(move |contexts| {
        let mut ctx = contexts.borrow_mut();
        ctx.error_id = ctx.error_id.wrapping_add(1);
        let reported_error = ReportedError {
            id: ctx.error_id,
            type_id: TypeId::of::<E>(),
            value: &mut err as *mut _ as *mut (),
        };

        for x in ctx.scopes.iter_mut() {
            let context = unsafe { x.context() };
            match unsafe { context.try_set_error(&reported_error) } {
                // SAFETY: We must ensure to forget err if we end up here!
                TrySetErrorResult::NeedForget => {
                    std::mem::forget(err);
                    return reported_error.id;
                },
                TrySetErrorResult::NeedDrop => {
                    drop(err);
                    return reported_error.id
                },
                _ => {}
            }
        }
        reported_error.id
    })
}
