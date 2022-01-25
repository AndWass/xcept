use std::any::TypeId;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::thread_local;

pub trait ErrorHandlingContext: 'static
{
    /// Checks if the handling context holds an active error already.
    ///
    /// Each context should only hold 1 active error per context.
    fn holds_active_error(&self) -> bool;
    /// Try to store the error into a handling context
    ///
    /// If this error handling context can handle the type described by `type_id` then it should
    /// read the data from the pointer and store it internally, and return true.
    ///
    /// ## Safety
    ///
    ///   * The `TypeId` of the actual type of the pointer that `value` points to must match
    ///     `type_id`.
    ///   * If this function returns true, the caller must ensure to `forget` the original value
    ///     since it is effectively moved to some other location.
    unsafe fn try_set_error(&mut self, type_id: TypeId, value: *mut ()) -> bool;
}

impl<T: 'static> ErrorHandlingContext for Option<T>
{
    fn holds_active_error(&self) -> bool {
        self.is_some()
    }
    unsafe fn try_set_error(&mut self, type_id: TypeId, value: *mut ()) -> bool {
        if TypeId::of::<T>() == type_id {
            *self = Some((value as *mut T).read());
            true
        }
        else {
            false
        }
    }
}

struct HandlingScopes
{
    error_count: u32,
    scopes: VecDeque<HandlingScope>
}

impl HandlingScopes {
    fn new() -> Self {
        Self {
            error_count: 0,
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

pub(crate) fn reset() {
    CONTEXTS.with(|c| {
        *c.borrow_mut() = HandlingScopes::new();
    });
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

pub fn push_error<E: 'static>(mut err: E) {
    CONTEXTS.with(move |contexts| {
        let mut ctx = contexts.borrow_mut();
        ctx.error_count += 1;
        let err_type = TypeId::of::<E>();
        let err_ptr = &mut err as *mut _ as *mut ();
        for x in ctx.scopes.iter_mut() {
            let context = unsafe { x.context() };
            if unsafe { context.try_set_error(err_type, err_ptr) } {
                // SAFETY: We must ensure to forget err if we end up here!
                std::mem::forget(err);
                return;
            }
        }
    });
}

pub fn error_handled() {
    CONTEXTS.with(|contexts| {
        contexts.borrow_mut().error_count -= 1;
    });
}

pub fn error_count() -> u32 {
    CONTEXTS.with(|contexts| {
        contexts.borrow().error_count
    })
}

pub fn scopes_count() -> usize {
    CONTEXTS.with(|c| {
        c.borrow().scopes.len()
    })
}
