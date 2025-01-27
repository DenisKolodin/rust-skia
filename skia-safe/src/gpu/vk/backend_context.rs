use super::{Device, GetProc, GetProcOf, Instance, PhysicalDevice, Queue};
use crate::prelude::*;
use skia_bindings::{
    C_GrVkBackendContext_Delete, C_GrVkBackendContext_New, GrVkExtensionFlags, GrVkFeatureFlags,
};
use std::cell::RefCell;
use std::os::raw;
use std::{ffi, mem};

bitflags! {
    pub struct ExtensionFlags : u32 {
        const EXT_DEBUG_REPORT = skia_bindings::GrVkExtensionFlags_kEXT_debug_report_GrVkExtensionFlag as _;
        const NV_GLSL_SHADER = skia_bindings::GrVkExtensionFlags_kNV_glsl_shader_GrVkExtensionFlag as _;
        const KHR_SURFACE = skia_bindings::GrVkExtensionFlags_kKHR_surface_GrVkExtensionFlag as _;
        const KHR_SWAPCHAIN = skia_bindings::GrVkExtensionFlags_kKHR_swapchain_GrVkExtensionFlag as _;
        const KHR_WIN32_SURFACE = skia_bindings::GrVkExtensionFlags_kKHR_win32_surface_GrVkExtensionFlag as _;
        const KHR_ANDROID_SURFACE = skia_bindings::GrVkExtensionFlags_kKHR_android_surface_GrVkExtensionFlag as _;
        const KHR_XCB_SURFACE = skia_bindings::GrVkExtensionFlags_kKHR_xcb_surface_GrVkExtensionFlag as _;
    }
}

impl NativeTransmutable<GrVkExtensionFlags> for ExtensionFlags {}
#[test]
fn test_extension_flags_layout() {
    ExtensionFlags::test_layout();
}

bitflags! {
    pub struct FeatureFlags: u32 {
        const GEOMETRY_SHADER = skia_bindings::GrVkFeatureFlags_kGeometryShader_GrVkFeatureFlag as _;
        const DUAL_SRC_BLEND = skia_bindings::GrVkFeatureFlags_kDualSrcBlend_GrVkFeatureFlag as _;
        const SAMPLE_RATE_SHADING = skia_bindings::GrVkFeatureFlags_kSampleRateShading_GrVkFeatureFlag as _;
    }
}

impl NativeTransmutable<GrVkFeatureFlags> for FeatureFlags {}
#[test]
fn test_feature_flags_layout() {
    FeatureFlags::test_layout();
}

// Note: the GrBackendContext's layout generated by bindgen does not match in size,
// so we do need to use a pointer here for now.
pub struct BackendContext<'a> {
    pub(crate) native: *mut ffi::c_void,
    get_proc: &'a GetProc,
}

impl<'a> Drop for BackendContext<'a> {
    fn drop(&mut self) {
        unsafe { C_GrVkBackendContext_Delete(self.native) }
    }
}

// TODO: add some accessor functions to the public fields.
// TODO: may support Clone (note the original structure holds a smartpointer!)
// TODO: think about making this safe in respect to the lifetime of the handles
//       it refers to.
impl<'a> BackendContext<'a> {
    pub unsafe fn new(
        instance: Instance,
        physical_device: PhysicalDevice,
        device: Device,
        (queue, queue_index): (Queue, usize),
        get_proc: &impl GetProc,
    ) -> BackendContext {
        BackendContext {
            native: C_GrVkBackendContext_New(
                instance as _,
                physical_device as _,
                device as _,
                queue as _,
                queue_index.try_into().unwrap(),
                Some(global_get_proc),
            ),
            get_proc,
        }
    }

    // The idea here is to set up a thread local variable with the GetProc function trait
    // and reroute queries to global_get_proc to it as long the caller does not invoke the Drop
    // impl trait that is returned.
    // This is an attempt to support Rust Closures / Functions that resolve function pointers instead
    // of relying on a global extern "C" function.
    // TODO: This is a mess, highly unsafe, and needs to be simplified / rewritten
    //       by someone who understands Rust better.

    pub(crate) unsafe fn begin_resolving(&self) -> impl Drop {
        THREAD_LOCAL_GET_PROC.with(|get_proc| {
            let get_proc_trait_object: &GetProc = self.get_proc;
            *get_proc.borrow_mut() = Some(mem::transmute(get_proc_trait_object))
        });

        EndResolving {}
    }
}

struct EndResolving {}

impl Drop for EndResolving {
    fn drop(&mut self) {
        THREAD_LOCAL_GET_PROC.with(|get_proc| *get_proc.borrow_mut() = None)
    }
}

thread_local! {
    static THREAD_LOCAL_GET_PROC: RefCell<Option<TraitObject>> = RefCell::new(None);
}

// https://doc.rust-lang.org/1.19.0/std/raw/struct.TraitObject.html
#[repr(C)]
// Copy & Clone are required for the *get_proc.borrow() below.
#[derive(Copy, Clone)]
struct TraitObject {
    pub data: *mut (),
    pub vtable: *mut (),
}

// The global resolvement function passed to Skia.
unsafe extern "C" fn global_get_proc(
    name: *const raw::c_char,
    instance: Instance,
    device: Device,
) -> *const raw::c_void {
    THREAD_LOCAL_GET_PROC.with(|get_proc| {
        match *get_proc.borrow() {
            Some(get_proc) => {
                let get_proc_trait_object: &GetProc = mem::transmute(get_proc);
                if !device.is_null() {
                    get_proc_trait_object(GetProcOf::Device(device, name))
                } else {
                    // note: instance may be null here!
                    get_proc_trait_object(GetProcOf::Instance(instance, name))
                }
            }
            None => panic!("Vulkan GetProc called outside of a thread local resolvement context."),
        }
    })
}
