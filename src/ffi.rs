use std::ffi::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

pub(crate) const RUST_EXT_COLUMN_GENERATED: u32 = 1 << 0;
pub(crate) const RUST_EXT_COLUMN_SYSTEM: u32 = 1 << 1;
pub(crate) const RUST_EXT_COLUMN_EDITABLE: u32 = 1 << 2;
pub(crate) const RUST_EXT_COLUMN_FILTER_EQUALITY: u32 = 1 << 3;
pub(crate) const RUST_EXT_COLUMN_SORT_ASC: u32 = 1 << 4;
pub(crate) const RUST_EXT_COLUMN_ARRAY: u32 = 1 << 5;

pub(crate) const RUST_EXT_TABLE_VIEW: u32 = 1 << 0;
pub(crate) const RUST_EXT_TABLE_INSERT: u32 = 1 << 1;
pub(crate) const RUST_EXT_TABLE_UPDATE: u32 = 1 << 2;
pub(crate) const RUST_EXT_TABLE_DELETE: u32 = 1 << 3;
pub(crate) const RUST_EXT_TABLE_ROW_ID: u32 = 1 << 4;

pub(crate) const RUST_EXT_LOGICAL_VARCHAR: i32 = 0;
pub(crate) const RUST_EXT_LOGICAL_BOOLEAN: i32 = 1;
pub(crate) const RUST_EXT_LOGICAL_DECIMAL: i32 = 2;
pub(crate) const RUST_EXT_LOGICAL_TIMESTAMP_TZ: i32 = 3;
pub(crate) const RUST_EXT_LOGICAL_DATE: i32 = 4;
pub(crate) const RUST_EXT_LOGICAL_TIME: i32 = 5;
pub(crate) const RUST_EXT_LOGICAL_INTERVAL: i32 = 6;
pub(crate) const RUST_EXT_LOGICAL_CURRENCY: i32 = 7;
pub(crate) const RUST_EXT_LOGICAL_IMAGE: i32 = 8;
pub(crate) const RUST_EXT_LOGICAL_PERSON: i32 = 9;
pub(crate) const RUST_EXT_LOGICAL_HYPERLINK: i32 = 10;
pub(crate) const RUST_EXT_LOGICAL_LOOKUP: i32 = 11;
pub(crate) const RUST_EXT_LOGICAL_JSON: i32 = 12;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtString {
    pub(crate) ptr: *mut c_char,
    pub(crate) len: usize,
}

impl Default for RustExtString {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtError {
    pub(crate) message: RustExtString,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtInputValue {
    pub(crate) value_type: u8,
    pub(crate) bool_value: bool,
    pub(crate) int_value: i64,
    pub(crate) uint_value: u64,
    pub(crate) double_value: f64,
    pub(crate) string_value: RustExtString,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtWriteColumn {
    pub(crate) id: RustExtString,
    pub(crate) capabilities: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtTable {
    pub(crate) id: RustExtString,
    pub(crate) name: RustExtString,
    pub(crate) capabilities: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtTableList {
    pub(crate) items: *mut RustExtTable,
    pub(crate) count: usize,
    pub(crate) next_page_token: RustExtString,
}

impl Default for RustExtTableList {
    fn default() -> Self {
        Self {
            items: ptr::null_mut(),
            count: 0,
            next_page_token: RustExtString::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtColumn {
    pub(crate) id: RustExtString,
    pub(crate) name: RustExtString,
    pub(crate) type_name: RustExtString,
    pub(crate) capabilities: u32,
    pub(crate) logical_type: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtColumnList {
    pub(crate) items: *mut RustExtColumn,
    pub(crate) count: usize,
    pub(crate) next_page_token: RustExtString,
}

impl Default for RustExtColumnList {
    fn default() -> Self {
        Self {
            items: ptr::null_mut(),
            count: 0,
            next_page_token: RustExtString::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtCell {
    pub(crate) column_id: RustExtString,
    pub(crate) value_type: u8,
    pub(crate) value: RustExtString,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtRow {
    pub(crate) id: RustExtString,
    pub(crate) created_at: RustExtString,
    pub(crate) updated_at: RustExtString,
    pub(crate) deleted: bool,
    pub(crate) cells: *mut RustExtCell,
    pub(crate) cell_count: usize,
}

impl Default for RustExtRow {
    fn default() -> Self {
        Self {
            id: RustExtString::default(),
            created_at: RustExtString::default(),
            updated_at: RustExtString::default(),
            deleted: false,
            cells: ptr::null_mut(),
            cell_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtScanBatch {
    pub(crate) rows: *mut RustExtRow,
    pub(crate) row_count: usize,
    pub(crate) finished: bool,
}

impl Default for RustExtScanBatch {
    fn default() -> Self {
        Self {
            rows: ptr::null_mut(),
            row_count: 0,
            finished: false,
        }
    }
}

pub(crate) struct CodaRowsResponse {
    pub(crate) rows: *mut RustExtRow,
    pub(crate) row_count: usize,
    pub(crate) next_page_token: RustExtString,
    pub(crate) next_sync_token: RustExtString,
}

impl Default for CodaRowsResponse {
    fn default() -> Self {
        Self {
            rows: ptr::null_mut(),
            row_count: 0,
            next_page_token: RustExtString::default(),
            next_sync_token: RustExtString::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtArrayValue {
    pub(crate) is_null: bool,
    pub(crate) value: RustExtString,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtScanValue {
    pub(crate) is_null: bool,
    pub(crate) value_type: u8,
    pub(crate) bool_value: bool,
    pub(crate) value_owned: bool,
    pub(crate) value: RustExtString,
    pub(crate) array_values: *mut RustExtArrayValue,
    pub(crate) array_count: usize,
}

impl Default for RustExtScanValue {
    fn default() -> Self {
        Self {
            is_null: true,
            value_type: 0,
            bool_value: false,
            value_owned: false,
            value: RustExtString::default(),
            array_values: ptr::null_mut(),
            array_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtCatalogTable {
    pub(crate) id: RustExtString,
    pub(crate) name: RustExtString,
    pub(crate) capabilities: u32,
    pub(crate) columns: *mut RustExtColumn,
    pub(crate) column_count: usize,
}

impl Default for RustExtCatalogTable {
    fn default() -> Self {
        Self {
            id: RustExtString::default(),
            name: RustExtString::default(),
            capabilities: 0,
            columns: ptr::null_mut(),
            column_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtCatalog {
    pub(crate) tables: *mut RustExtCatalogTable,
    pub(crate) table_count: usize,
}

impl Default for RustExtCatalog {
    fn default() -> Self {
        Self {
            tables: ptr::null_mut(),
            table_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtClientConfig {
    pub(crate) resource: RustExtString,
    pub(crate) credential: RustExtString,
    pub(crate) endpoint: RustExtString,
    pub(crate) include_system_columns: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RustExtAttachConfig {
    pub(crate) resource: RustExtString,
    pub(crate) credential: RustExtString,
    pub(crate) endpoint: RustExtString,
    pub(crate) primary_secret_scope: RustExtString,
    pub(crate) fallback_secret_scope: RustExtString,
    pub(crate) include_system_columns: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustExtScanRequest {
    pub(crate) filter: RustExtString,
    pub(crate) order: RustExtString,
    pub(crate) limit: u64,
}

impl Default for RustExtScanRequest {
    fn default() -> Self {
        Self {
            filter: RustExtString::default(),
            order: RustExtString::default(),
            limit: 500,
        }
    }
}

pub(crate) struct CodaRowsRequest {
    pub(crate) page_token: String,
    pub(crate) query: String,
    pub(crate) sort_by: String,
    pub(crate) sync_token: String,
    pub(crate) limit: u64,
}

#[repr(C)]
pub struct RustExtDuckDbHost {
    pub(crate) set_description:
        unsafe extern "C" fn(*mut c_void, *const c_char, *mut RustExtError) -> bool,
    pub(crate) register_config_secret: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        *mut RustExtError,
    ) -> bool,
    pub(crate) register_storage_extension:
        unsafe extern "C" fn(*mut c_void, *const c_char, *mut RustExtError) -> bool,
}

#[repr(C)]
pub struct RustExtAttachHost {
    pub(crate) get_option: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *mut RustExtString,
        *mut RustExtError,
    ) -> bool,
    pub(crate) lookup_secret: unsafe extern "C" fn(
        *mut c_void,
        RustExtString,
        *const c_char,
        *const c_char,
        *mut RustExtString,
        *mut RustExtError,
    ) -> bool,
}

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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register_config_secret(
        &self,
        loader: *mut c_void,
        secret_type: *const c_char,
        provider: *const c_char,
        extension: *const c_char,
        default_scope: *const c_char,
        secret_key: *const c_char,
        secret_env_key: *const c_char,
        err: *mut RustExtError,
    ) -> bool {
        unsafe {
            (self.register_config_secret)(
                loader,
                secret_type,
                provider,
                extension,
                default_scope,
                secret_key,
                secret_env_key,
                err,
            )
        }
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

pub(crate) fn c_static(value: &'static [u8]) -> *const c_char {
    value.as_ptr().cast()
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
