//! Optional Linux EGL identity queries; no new display, context or library load.
use std::ffi::{CStr, c_char, c_void};

struct Library(*mut c_void);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: one successful dlopen reference belongs to this guard.
        unsafe { libc::dlclose(self.0) };
    }
}

pub(super) fn identity() -> (Option<String>, Option<String>) {
    query().unwrap_or_default()
}

fn query() -> Option<(Option<String>, Option<String>)> {
    // SAFETY: all symbols below use the EGL 1.0 / EGL_MESA_query_driver C ABI.
    // RTLD_NOLOAD only takes a reference to SDL's already loaded EGL library.
    // The guard keeps symbols alive. No borrowed EGL handle is destroyed.
    // We query only the calling thread's current display/context, which belongs
    // to the SDL canvas checked by current_gl. Returned C strings are copied
    // while the library and context remain alive; null pointers stay unknown.
    unsafe {
        let library = libc::dlopen(c"libEGL.so.1".as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD);
        if library.is_null() {
            return None;
        }
        let library = Library(library);
        let symbol = |name: &CStr| {
            let pointer = libc::dlsym(library.0, name.as_ptr());
            (!pointer.is_null()).then_some(pointer)
        };
        let current = std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> *mut c_void>(
            symbol(c"eglGetCurrentDisplay")?,
        );
        let context = std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> *mut c_void>(
            symbol(c"eglGetCurrentContext")?,
        );
        let query = std::mem::transmute::<
            *mut c_void,
            unsafe extern "C" fn(*mut c_void, i32) -> *const c_char,
        >(symbol(c"eglQueryString")?);
        let display = current();
        if display.is_null()
            || context().is_null()
            || context() != sdl2::sys::SDL_GL_GetCurrentContext()
        {
            return None;
        }
        let string = |pointer: *const c_char| {
            if pointer.is_null() {
                None
            } else {
                Some(CStr::from_ptr(pointer).to_string_lossy().into_owned())
            }
        };
        let version = string(query(display, 0x3054)); // EGL_VERSION
        let extensions = string(query(display, 0x3055)); // EGL_EXTENSIONS
        let driver = if extensions.as_deref().is_some_and(|s| {
            s.split_whitespace()
                .any(|ext| ext == "EGL_MESA_query_driver")
        }) {
            let proc = std::mem::transmute::<
                *mut c_void,
                unsafe extern "C" fn(*const c_char) -> *const (),
            >(symbol(c"eglGetProcAddress")?);
            let address = proc(c"eglGetDisplayDriverName".as_ptr());
            if address.is_null() {
                None
            } else {
                let name = std::mem::transmute::<
                    *const (),
                    unsafe extern "C" fn(*mut c_void) -> *const c_char,
                >(address);
                string(name(display))
            }
        } else {
            None
        };
        Some((version, driver))
    }
}
