//! Optional Linux EGL identity queries; no new display, context or library load.
use std::ffi::{CStr, c_char, c_void};

struct Library(*mut c_void);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: one successful dlopen reference belongs to this guard.
        if unsafe { libc::dlclose(self.0) } != 0_i32 {
            eprintln!("EGL library reference release failed");
        }
    }
}

pub(super) fn identity() -> (Option<String>, Option<String>) {
    query().unwrap_or_default()
}

fn query() -> Option<(Option<String>, Option<String>)> {
    // SAFETY: the literal is NUL-terminated; RTLD_NOLOAD only takes a reference
    // to SDL's already loaded library. The guard owns that reference once non-null.
    let handle =
        unsafe { libc::dlopen(c"libEGL.so.1".as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD) };
    if handle.is_null() {
        return None;
    }
    let library = Library(handle);
    let symbol = |name: &CStr| {
        // SAFETY: the guard holds a live library reference and name is a live
        // NUL-terminated string. dlsym does not retain this borrowed name.
        let pointer = unsafe { libc::dlsym(library.0, name.as_ptr()) };
        (!pointer.is_null()).then_some(pointer)
    };
    let current_symbol = symbol(c"eglGetCurrentDisplay")?;
    // SAFETY: this EGL 1.0 symbol has the declared C ABI; the library guard
    // outlives the function pointer and every invocation below.
    let current_display = unsafe {
        std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> *mut c_void>(current_symbol)
    };
    let context_symbol = symbol(c"eglGetCurrentContext")?;
    // SAFETY: EGL 1.0 defines this no-argument C function; its library stays alive.
    let current_context = unsafe {
        std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> *mut c_void>(context_symbol)
    };
    let query_symbol = symbol(c"eglQueryString")?;
    // SAFETY: EGL 1.0 defines this exact C ABI for a display and integer selector.
    let query_string = unsafe {
        std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void, i32) -> *const c_char>(
            query_symbol,
        )
    };
    // SAFETY: EGL's current-display query takes no arguments and changes no ownership.
    let display = unsafe { current_display() };
    // SAFETY: the thread's current context is borrowed for an identity comparison only.
    let context = unsafe { current_context() };
    // SAFETY: SDL video is live on this thread; the result is only compared, never freed.
    let sdl_context = unsafe { sdl2::sys::SDL_GL_GetCurrentContext() };
    if display.is_null() || context.is_null() || context != sdl_context {
        return None;
    }
    let string = |pointer: *const c_char| {
        if pointer.is_null() {
            None
        } else {
            // SAFETY: only EGL-owned NUL-terminated strings from the queries
            // below reach this closure. The library/context remain live while
            // bytes are copied; no borrowed pointer escapes this function.
            Some(
                unsafe { CStr::from_ptr(pointer) }
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    };
    // SAFETY: the validated current display is live; EGL_VERSION is a core selector.
    let version = string(unsafe { query_string(display, 0x3054) });
    // SAFETY: EGL_EXTENSIONS is a core selector for the same live display.
    let extensions = string(unsafe { query_string(display, 0x3055) });
    let driver = if extensions.as_deref().is_some_and(|s| {
        s.split_whitespace()
            .any(|ext| ext == "EGL_MESA_query_driver")
    }) {
        let proc_symbol = symbol(c"eglGetProcAddress")?;
        // SAFETY: the non-null EGL symbol has this core C ABI and its library is live.
        let get_proc = unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*const c_char) -> *const ()>(
                proc_symbol,
            )
        };
        // SAFETY: the procedure name is a live NUL-terminated literal.
        let address = unsafe { get_proc(c"eglGetDisplayDriverName".as_ptr()) };
        if address.is_null() {
            None
        } else {
            // SAFETY: EGL_MESA_query_driver was advertised above; the non-null
            // procedure implements its documented C ABI.
            let name = unsafe {
                std::mem::transmute::<*const (), unsafe extern "C" fn(*mut c_void) -> *const c_char>(
                    address,
                )
            };
            // SAFETY: the current display is live and the extension entry point
            // was validated. Its borrowed string is copied immediately.
            string(unsafe { name(display) })
        }
    } else {
        None
    };
    Some((version, driver))
}
