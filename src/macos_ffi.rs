//! Shared unsafe-FFI loading helpers for the macOS private-API modules
//! (`macos_blur`, `macos_space`).
//!
//! Extracted so the dlsym null-check defensive path is one named function
//! both modules share, and QA-011 tests can exercise it directly: a missing
//! symbol must yield `None`, never a transmuted null function pointer.

/// Resolve `name` in an already-dlopened `handle`, returning `None` when the
/// symbol is absent.
///
/// # Safety
/// `handle` must be a valid non-null `libc::dlopen` handle, and `T` must be
/// a function pointer type whose ABI matches the symbol being resolved.
pub(crate) unsafe fn dlsym_checked<T>(
    handle: *mut libc::c_void,
    name: &std::ffi::CStr,
) -> Option<T> {
    let sym = unsafe { libc::dlsym(handle, name.as_ptr()) };
    if sym.is_null() {
        None
    } else {
        // transmute_copy is the generic form: size-checked at each
        // instantiation, where T is a pointer-sized function pointer type.
        Some(unsafe { std::mem::transmute_copy::<*mut libc::c_void, T>(&sym) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type VoidFn = unsafe extern "C" fn();

    #[test]
    fn absent_symbol_yields_none_not_a_null_fn_pointer() {
        // The defensive path QA-011 wanted pinned: an absent symbol must
        // produce None. Transmuting the null instead would hand the caller
        // a callable null pointer — UB the moment it is invoked.
        unsafe {
            let handle = libc::dlopen(
                c"/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation".as_ptr(),
                libc::RTLD_LAZY,
            );
            assert!(!handle.is_null(), "CoreFoundation must dlopen");
            let missing: Option<VoidFn> =
                dlsym_checked(handle, c"ParTermDefinitelyNotASymbol_qa011");
            assert!(
                missing.is_none(),
                "a bogus symbol name must resolve to None"
            );
            let present: Option<VoidFn> = dlsym_checked(handle, c"CFAbsoluteTimeGetCurrent");
            assert!(present.is_some(), "a real symbol must resolve");
        }
    }
}
