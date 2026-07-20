use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use super::{RustExtError, RustExtString};

pub(crate) fn c_static(value: &'static [u8]) -> *const c_char {
    value.as_ptr().cast()
}

pub(crate) fn c_static_string(value: &'static [u8]) -> RustExtString {
    let len = value
        .len()
        .saturating_sub(usize::from(value.last() == Some(&0)));
    RustExtString {
        ptr: value.as_ptr().cast_mut().cast(),
        len,
    }
}

pub(crate) fn alloc_string(value: &str) -> RustExtString {
    if value.is_empty() {
        return RustExtString::default();
    }
    let mut bytes = value.as_bytes().to_vec();
    let result = RustExtString {
        ptr: bytes.as_mut_ptr().cast(),
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    result
}

pub(crate) fn borrow_string(value: &str) -> RustExtString {
    RustExtString {
        ptr: value.as_ptr().cast_mut().cast(),
        len: value.len(),
    }
}

impl RustExtString {
    pub(crate) fn as_str(&self) -> &str {
        std::str::from_utf8(slice_from_raw_parts(self.ptr.cast::<u8>(), self.len)).unwrap_or("")
    }

    pub(crate) fn free(self) {
        drop(vec_from_raw_parts(self.ptr.cast::<u8>(), self.len));
    }
}

pub(crate) fn str_from_raw<'a>(ptr: *const c_char, len: usize) -> &'a str {
    std::str::from_utf8(slice_from_raw_parts(ptr.cast::<u8>(), len)).unwrap_or("")
}

pub(crate) fn vec_from_raw_parts<T>(ptr: *mut T, len: usize) -> Vec<T> {
    if ptr.is_null() {
        Vec::new()
    } else {
        unsafe { Vec::from_raw_parts(ptr, len, len) }
    }
}

pub(crate) fn vec_into_raw_parts<T>(items: Vec<T>) -> (*mut T, usize) {
    let len = items.len();
    if len == 0 {
        return (ptr::null_mut(), 0);
    }
    let mut boxed = items.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    (ptr, len)
}

pub(crate) fn slice_from_raw_parts<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

pub(crate) fn mut_from_raw<'a, T>(ptr: *mut T, name: &str) -> Result<&'a mut T, String> {
    if ptr.is_null() {
        Err(format!("missing {name} pointer"))
    } else {
        Ok(unsafe { &mut *ptr })
    }
}

pub(crate) fn ref_from_raw<'a, T>(ptr: *const T, name: &str) -> Result<&'a T, String> {
    if ptr.is_null() {
        Err(format!("missing {name} pointer"))
    } else {
        Ok(unsafe { &*ptr })
    }
}

pub(crate) fn write_out<T>(out: *mut T, value: T) -> Result<(), String> {
    let out = mut_from_raw(out, "output")?;
    *out = value;
    Ok(())
}

pub(crate) fn set_error(err: *mut RustExtError, message: impl AsRef<str>) {
    if !err.is_null() {
        unsafe {
            (*err).message = alloc_string(message.as_ref());
        }
    }
}

pub(crate) fn ffi_bool(
    err: *mut RustExtError,
    context: &str,
    f: impl FnOnce() -> Result<(), String>,
) -> bool {
    let result = catch_unwind(AssertUnwindSafe(f));
    match result {
        Ok(Ok(())) => true,
        Ok(Err(message)) => {
            set_error(err, format!("{context}: {message}"));
            false
        }
        Err(_) => {
            set_error(err, format!("{context}: panic"));
            false
        }
    }
}
