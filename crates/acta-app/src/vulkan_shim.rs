//! The Vulkan loader, found at run time (ADR 0014, amendment "Vulkan in
//! Acta.exe"). Windows and Linux only.
//!
//! ggml-vulkan is linked statically into this executable. It reaches Vulkan
//! through Vulkan-Hpp's dynamic dispatcher, except for three functions it
//! calls directly; this module defines those three, so the linker resolves
//! them here and the executable never imports `vulkan-1.dll` or
//! `libvulkan.so.1`. Acta therefore starts, and dictates on the processor, on
//! a machine without a Vulkan loader.
//!
//! The loader is opened only once a worker allowed it ([`allow`]): the probe,
//! and a worker asked to use the graphics card. Otherwise, and when it is
//! missing, `vkEnumerateInstanceVersion` reports Vulkan 1.0; ggml then throws
//! "Vulkan 1.2 required" and catches it itself, its Vulkan backend is not
//! registered, and no GPU driver is loaded into the process. That is how a
//! worker on the processor stays on the processor: ggml registers every
//! backend it was built with and offloads large operations to a registered
//! GPU whatever `use_gpu` says.
//!
//! The GUI process never calls into ggml, so it never opens the loader.

use std::ffi::{CStr, c_char, c_void};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

type Pfn = unsafe extern "system" fn();
type GetInstanceProcAddr = unsafe extern "system" fn(*mut c_void, *const c_char) -> Option<Pfn>;
type CmdCopyBuffer = unsafe extern "system" fn(*mut c_void, u64, u64, u32, *const c_void);
type GetPhysicalDeviceFeatures2 = unsafe extern "system" fn(*mut c_void, *mut c_void);

/// The real loader's entry points.
#[derive(Clone, Copy)]
struct Loader {
    get_instance_proc_addr: GetInstanceProcAddr,
    cmd_copy_buffer: CmdCopyBuffer,
    get_physical_device_features2: GetPhysicalDeviceFeatures2,
}

/// Whether the loader may be opened; false until a worker says otherwise.
static ALLOWED: AtomicBool = AtomicBool::new(false);
/// The loader, opened (or not) at its first use, then for the process's life.
static LOADER: OnceLock<Option<Loader>> = OnceLock::new();

/// Lets the loader be opened (`true`: the probe, a worker on the graphics
/// card). Call it before whisper.cpp first runs: the choice is fixed at the
/// first Vulkan call ggml makes.
pub fn allow(yes: bool) {
    ALLOWED.store(yes, Ordering::SeqCst);
}

/// The system's loader file.
#[cfg(windows)]
const LOADER_FILE: &CStr = c"vulkan-1.dll";
#[cfg(not(windows))]
const LOADER_FILE: &CStr = c"libvulkan.so.1";

fn loader() -> Option<&'static Loader> {
    LOADER
        .get_or_init(|| {
            if ALLOWED.load(Ordering::SeqCst) {
                open(LOADER_FILE)
            } else {
                None
            }
        })
        .as_ref()
}

/// Opens a loader by file name: System32 only on Windows (never the
/// install folder, where a planted DLL could sit), the system's library
/// path on Linux. Never closed.
fn open(file: &CStr) -> Option<Loader> {
    let get = |name: &CStr| -> Option<*const c_void> { symbol(file, name) };
    let gipa = get(c"vkGetInstanceProcAddr")?;
    let copy = get(c"vkCmdCopyBuffer")?;
    let features = get(c"vkGetPhysicalDeviceFeatures2")?;
    // SAFETY: the loader exports these with the Vulkan signatures above.
    unsafe {
        Some(Loader {
            get_instance_proc_addr: std::mem::transmute::<*const c_void, GetInstanceProcAddr>(gipa),
            cmd_copy_buffer: std::mem::transmute::<*const c_void, CmdCopyBuffer>(copy),
            get_physical_device_features2: std::mem::transmute::<
                *const c_void,
                GetPhysicalDeviceFeatures2,
            >(features),
        })
    }
}

#[cfg(windows)]
fn symbol(file: &CStr, name: &CStr) -> Option<*const c_void> {
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
    };
    let wide: Vec<u16> = file
        .to_str()
        .ok()?
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: NUL-terminated strings; a module already loaded is only
    // reference-counted again, and never freed.
    unsafe {
        let module = LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if module.is_null() {
            return None;
        }
        GetProcAddress(module, name.as_ptr().cast()).map(|f| f as *const c_void)
    }
}

#[cfg(not(windows))]
fn symbol(file: &CStr, name: &CStr) -> Option<*const c_void> {
    // SAFETY: NUL-terminated strings; the handle is never closed.
    unsafe {
        let module = libc::dlopen(file.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if module.is_null() {
            return None;
        }
        let f = libc::dlsym(module, name.as_ptr());
        (!f.is_null()).then_some(f as *const c_void)
    }
}

/// What `vkEnumerateInstanceVersion` says without a loader: 1.0, which
/// ggml refuses.
unsafe extern "system" fn version_1_0(out: *mut u32) -> i32 {
    if !out.is_null() {
        // SAFETY: Vulkan-Hpp passes a valid pointer to a u32.
        unsafe { *out = 1 << 22 };
    }
    0
}

/// `vkGetInstanceProcAddr` over `loader`, or without one.
///
/// # Safety
/// `name` is a NUL-terminated string; `instance` as Vulkan expects.
unsafe fn resolve(
    loader: Option<&Loader>,
    instance: *mut c_void,
    name: *const c_char,
) -> Option<Pfn> {
    match loader {
        // SAFETY: the caller's arguments, to the real loader.
        Some(l) => unsafe { (l.get_instance_proc_addr)(instance, name) },
        None => {
            if name.is_null() {
                return None;
            }
            // SAFETY: a NUL-terminated string from the caller.
            let name = unsafe { CStr::from_ptr(name) };
            (name == c"vkEnumerateInstanceVersion").then(|| {
                // SAFETY: Vulkan-Hpp calls it back with its real signature.
                unsafe {
                    std::mem::transmute::<unsafe extern "system" fn(*mut u32) -> i32, Pfn>(
                        version_1_0,
                    )
                }
            })
        }
    }
}

/// # Safety
/// The Vulkan loader's contract; called by ggml-vulkan.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetInstanceProcAddr(
    instance: *mut c_void,
    name: *const c_char,
) -> Option<Pfn> {
    // SAFETY: the caller's arguments.
    unsafe { resolve(loader(), instance, name) }
}

/// # Safety
/// The Vulkan loader's contract. Reached only after an instance was made,
/// which needs the loader.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkCmdCopyBuffer(
    command_buffer: *mut c_void,
    src: u64,
    dst: u64,
    count: u32,
    regions: *const c_void,
) {
    let Some(l) = loader() else {
        std::process::abort();
    };
    // SAFETY: the caller's arguments, to the real loader.
    unsafe { (l.cmd_copy_buffer)(command_buffer, src, dst, count, regions) }
}

/// # Safety
/// The Vulkan loader's contract. Reached only after an instance was made,
/// which needs the loader.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetPhysicalDeviceFeatures2(
    device: *mut c_void,
    features: *mut c_void,
) {
    let Some(l) = loader() else {
        std::process::abort();
    };
    // SAFETY: the caller's arguments, to the real loader.
    unsafe { (l.get_physical_device_features2)(device, features) }
}

#[cfg(test)]
mod tests {
    use super::*;

    type EnumerateVersion = unsafe extern "system" fn(*mut u32) -> i32;

    fn version_without_loader() -> u32 {
        // SAFETY: a valid name; no instance.
        let f = unsafe {
            resolve(
                None,
                std::ptr::null_mut(),
                c"vkEnumerateInstanceVersion".as_ptr(),
            )
        }
        .expect("a version function");
        // SAFETY: that is its signature.
        let f = unsafe { std::mem::transmute::<Pfn, EnumerateVersion>(f) };
        let mut version = 0;
        // SAFETY: a valid pointer.
        assert_eq!(unsafe { f(&mut version) }, 0);
        version
    }

    #[test]
    fn without_a_loader_vulkan_is_1_0_and_nothing_else_resolves() {
        assert_eq!(version_without_loader(), 1 << 22);
        for name in [
            c"vkCreateInstance",
            c"vkEnumerateInstanceExtensionProperties",
            c"vkEnumerateInstanceLayerProperties",
            c"vkEnumeratePhysicalDevices",
        ] {
            // SAFETY: valid names; no instance.
            assert!(unsafe { resolve(None, std::ptr::null_mut(), name.as_ptr()) }.is_none());
        }
        // SAFETY: a null name is answered, not read.
        assert!(unsafe { resolve(None, std::ptr::null_mut(), std::ptr::null()) }.is_none());
    }

    #[test]
    fn a_missing_loader_file_opens_nothing() {
        assert!(open(c"acta-no-such-vulkan-loader").is_none());
    }

    /// The processor's path: nothing allowed the loader in this test
    /// process (no test calls `allow(true)`), so the exported entry point
    /// answers without opening it, and never will.
    #[test]
    fn the_exported_entry_point_does_not_open_the_loader_unless_allowed() {
        assert!(!ALLOWED.load(Ordering::SeqCst));
        // SAFETY: a valid name; no instance.
        let f =
            unsafe { vkGetInstanceProcAddr(std::ptr::null_mut(), c"vkCreateInstance".as_ptr()) };
        assert!(f.is_none());
        assert!(matches!(LOADER.get(), Some(None)));
    }
}
