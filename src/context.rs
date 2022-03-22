use std::any::TypeId;
use std::cell::RefCell;
use std::ops::DerefMut;
use std::thread_local;

pub struct ReportedError
{
    pub id: u32,
    pub type_id: TypeId,
    pub value: *mut (),
}

impl ReportedError {
    fn new<E: crate::Error>(id: u32, err: &mut E) -> Self {
        Self {
            id,
            type_id: TypeId::of::<E>(),
            value: err as *const _ as *mut (),
        }
    }
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

pub struct CatchAllContext
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
    scopes: *mut ScopeNode
}

impl HandlingScopes {
    fn new() -> Self {
        Self {
            error_id: 0,
            scopes: core::ptr::null_mut(),
        }
    }
}

pub struct ScopeNode
{
    context: *mut dyn ErrorHandlingContext,
    next: *mut ScopeNode,
}

impl ScopeNode {
    pub fn new(context: &mut dyn ErrorHandlingContext) -> Self {
        Self {
            context,
            next: core::ptr::null_mut(),
        }
    }

    unsafe fn context(&mut self) -> &mut dyn ErrorHandlingContext {
        &mut *self.context
    }
}

thread_local! {
    static CONTEXTS: RefCell<HandlingScopes> = RefCell::new(HandlingScopes::new());
}

/// Push a new error handling scope to the list of scopes
///
/// # Safety
///
/// The following requirements must be met:
///
///   * No references to `scope` must be created after this function returns.
///     * When the returned guard is dropped it is safe to reference `scope` again.
///   * `scope` must be kept alive until the guard is dropped
///   * The context that `scope` refers to must be kept alive until the guard is dropped
///   * The returned guard must be dropped, it must not be forgotten.
///
pub unsafe fn push_handling_scope(scope: &mut ScopeNode) -> PopScopeGuard {
    CONTEXTS.with(move |contexts| {
        let mut ctx = contexts.borrow_mut();
        scope.next = ctx.scopes;
        ctx.scopes = scope;
        PopScopeGuard(scope)
    })
}

/// Pop a scope from the list of error handling scopes
///
/// # Safety
///
/// Scope must previously have been pushed, and never been popped before.
///
unsafe fn pop_handling_scope(scope: *mut ScopeNode) {
    CONTEXTS.with(move |contexts| {
        let mut ctx = contexts.borrow_mut();
        ctx.scopes = (*scope).next;
    })
}

/// Scope guard to automatically pop a scope when it is destroyed.
///
/// This is created by pushing scopes and then manually dropping the guard.
pub struct PopScopeGuard(*mut ScopeNode);

impl Drop for PopScopeGuard {
    fn drop(&mut self) {
        // Safety: the guard is only created by `push_handling_scope`
        // and the safety guarantees required by that function extends to the guard
        unsafe { pop_handling_scope(self.0) }
    }
}

unsafe fn try_scope(scope: *mut ScopeNode, err: &ReportedError) -> TrySetErrorResult {
    (*scope).context().try_set_error(err)
}

pub fn push_error<E: crate::Error>(mut err: E) -> u32 {
    CONTEXTS.with(move |contexts| {
        let mut ctx = contexts.borrow_mut();
        let ctx = ctx.deref_mut();

        ctx.error_id = ctx.error_id.wrapping_add(1);
        let reported_error = ReportedError::new(ctx.error_id, &mut err);

        // Safety: All scopes must be kept alive by the contract of push and pop scope
        let mut iter = ctx.scopes;
        while !iter.is_null() {
            match unsafe { try_scope(iter, &reported_error) } {
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
            iter = unsafe { (*iter).next }
        }
        reported_error.id
    })
}
