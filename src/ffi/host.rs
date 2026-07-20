use std::ffi::{c_char, c_void};

use super::{
    ref_from_raw, RustExtAttachHost, RustExtDuckDbHost, RustExtError, RustExtSecretRegistration,
    RustExtString,
};

impl RustExtDuckDbHost {
    pub(crate) fn from_ptr<'a>(
        ptr: *const RustExtDuckDbHost,
    ) -> Result<&'a RustExtDuckDbHost, String> {
        ref_from_raw(ptr, "DuckDB host")
    }

    pub(crate) fn set_description(
        &self,
        loader: *mut c_void,
        description: *const c_char,
        err: *mut RustExtError,
    ) -> bool {
        unsafe { (self.set_description)(loader, description, err) }
    }

    pub(crate) fn register_secret(
        &self,
        loader: *mut c_void,
        registration: RustExtSecretRegistration,
        err: *mut RustExtError,
    ) -> bool {
        unsafe { (self.register_secret)(loader, registration, err) }
    }

    pub(crate) fn register_storage_extension(
        &self,
        loader: *mut c_void,
        extension_name: *const c_char,
        err: *mut RustExtError,
    ) -> bool {
        unsafe { (self.register_storage_extension)(loader, extension_name, err) }
    }
}

impl RustExtAttachHost {
    pub(crate) fn from_ptr<'a>(
        ptr: *const RustExtAttachHost,
    ) -> Result<&'a RustExtAttachHost, String> {
        ref_from_raw(ptr, "attach host")
    }

    pub(crate) fn get_option(
        &self,
        userdata: *mut c_void,
        name: *const c_char,
        out: *mut RustExtString,
        err: *mut RustExtError,
    ) -> bool {
        unsafe { (self.get_option)(userdata, name, out, err) }
    }

    pub(crate) fn lookup_secret(
        &self,
        userdata: *mut c_void,
        scope: RustExtString,
        secret_type: *const c_char,
        secret_key: *const c_char,
        out: *mut RustExtString,
        err: *mut RustExtError,
    ) -> bool {
        unsafe { (self.lookup_secret)(userdata, scope, secret_type, secret_key, out, err) }
    }
}
