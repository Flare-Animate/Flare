//! Port of `common/tcore/texception.cpp`.
//!
//! `TException` is OpenToonz/Tahoma's base exception class. Its only out-of-line member is
//! the `std::string` constructor, whose body is a single statement:
//!
//! ```cpp
//! TException::TException(const std::string &msg) { m_msg = ::to_wstring(msg); }
//! ```
//!
//! `::to_wstring` converts a UTF-8 `std::string` to the platform's `std::wstring`
//! (`TString`). That width is platform-dependent -- 16 bits on Windows, 32 on Unix -- so
//! the Rust side keeps the same split and returns a NUL-terminated buffer of the native
//! width, which the header copies into `m_msg` and frees. The destructor and
//! `getMessage()` are inline in `texception.h` and stay in C++.
//!
//! Deviations: none of consequence. The C++ wrote `m_msg = ::to_wstring(msg)`, i.e. a
//! `std::wstring` copy; the forwarder does `m_msg = flare_to_wstring(msg.c_str())` and
//! `free()`s the returned buffer. `TString` owns its own copy, so the lifetime is fine.

#[cfg(target_os = "windows")]
type WChar = u16;
#[cfg(not(target_os = "windows"))]
type WChar = u32;

/// Convert a UTF-8 `std::string` (NUL-terminated) to a heap-allocated NUL-terminated
/// `std::wstring`. Returns NULL on a NULL input or on allocation failure. The caller
/// releases the buffer with `flare_to_wstring_free`.
#[no_mangle]
pub unsafe extern "C" fn flare_to_wstring(s: *const std::os::raw::c_char) -> *mut WChar {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = std::ffi::CStr::from_ptr(s).to_bytes();
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return std::ptr::null_mut(),
    };

    let mut out: Vec<WChar> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        out.extend(text.encode_utf16());
    }
    #[cfg(not(target_os = "windows"))]
    {
        out.extend(text.chars().map(|c| c as u32));
    }
    out.push(0);

    let ptr = libc_malloc(out.len() * std::mem::size_of::<WChar>()) as *mut WChar;
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    std::ptr::copy_nonoverlapping(out.as_ptr(), ptr, out.len());
    ptr
}

/// Counterpart to `flare_to_wstring`: releases a buffer it returned.
#[no_mangle]
pub unsafe extern "C" fn flare_to_wstring_free(p: *mut WChar) {
    if !p.is_null() {
        libc_free(p as *mut std::os::raw::c_void);
    }
}

// The C runtime's allocator, reached through the same symbol the C++ side links against,
// so the buffer belongs to one heap and can be freed from either language.
extern "C" {
    fn malloc(size: usize) -> *mut std::os::raw::c_void;
    fn free(ptr: *mut std::os::raw::c_void);
}
fn libc_malloc(size: usize) -> *mut std::os::raw::c_void {
    unsafe { malloc(size) }
}
fn libc_free(ptr: *mut std::os::raw::c_void) {
    unsafe { free(ptr) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(text: &str) -> Vec<WChar> {
        let p = std::ffi::CString::new(text).unwrap();
        let out = unsafe { flare_to_wstring(p.as_ptr()) };
        assert!(!out.is_null());
        let mut v = Vec::new();
        let mut i = 0;
        loop {
            let c = unsafe { *out.add(i) };
            if c == 0 {
                break;
            }
            v.push(c);
            i += 1;
        }
        unsafe { flare_to_wstring_free(out) };
        v
    }

    #[cfg(target_os = "windows")]
    fn expected(text: &str) -> Vec<WChar> {
        text.encode_utf16().collect()
    }
    #[cfg(not(target_os = "windows"))]
    fn expected(text: &str) -> Vec<WChar> {
        text.chars().map(|c| c as u32).collect()
    }

    #[test]
    fn ascii_round_trips() {
        assert_eq!(round_trip("Toonz Exception"), expected("Toonz Exception"));
    }

    #[test]
    fn unicode_round_trips() {
        assert_eq!(round_trip("caf\u{00E9} \u{1F600}"), expected("caf\u{00E9} \u{1F600}"));
    }

    #[test]
    fn null_and_invalid_input_return_null() {
        unsafe {
            assert!(flare_to_wstring(std::ptr::null()).is_null());
        }
    }

    #[test]
    fn free_is_idempotent() {
        unsafe { flare_to_wstring_free(std::ptr::null_mut()) };
    }
}