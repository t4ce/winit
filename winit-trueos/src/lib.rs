//! TRUEOS UI4 backend.
//!
//! A TRUEOS window is a UI4 visual frame. UI4 owns presentation, so it does
//! not expose a raw native graphics surface.
mod abi;
mod device_input;
mod event_loop;
mod input;
mod input_ext;
pub use abi::{KeyboardOutputEvent, PanEvent, PointerEvent};
pub use input_ext::{InputBatch, InputDevice, InputEvent, device_source};
/// Native HID discovery, independent sequence readers, combo bindings, and virtual devices.
#[cfg(target_os = "trueos")]
pub use v::vinput as hid;
mod window;
pub use event_loop::{
    ActiveEventLoop, EventLoop, EventLoopProxy, PlatformSpecificEventLoopAttributes,
};
pub use input::{physicalkey_to_scancode, scancode_to_physicalkey};
pub use window::Window;
use winit_core::window::Window as CoreWindow;
pub trait WindowExtTrueOS {
    fn trueos_window_id(&self) -> u32;
    /// Enable a bounded copy of routed UI4 input alongside standard Winit events.
    /// Disabled by default. Disabling clears the queue and loss counter.
    fn trueos_capture_input(&self, enabled: bool);
    /// Drain captured input. Call from `about_to_wait` or a window event callback.
    /// This never consumes input from another window or from standard Winit callbacks.
    fn trueos_take_input(&self) -> InputBatch;
}
impl WindowExtTrueOS for dyn CoreWindow + '_ {
    fn trueos_capture_input(&self, enabled: bool) {
        self.cast_ref::<Window>()
            .expect("non-TRUEOS window on TRUEOS")
            .trueos_capture_input(enabled);
    }
    fn trueos_take_input(&self) -> InputBatch {
        self.cast_ref::<Window>().expect("non-TRUEOS window on TRUEOS").trueos_take_input()
    }
    fn trueos_window_id(&self) -> u32 {
        self.cast_ref::<Window>().expect("non-TRUEOS window on TRUEOS").trueos_window_id()
    }
}

impl WindowExtTrueOS for Window {
    fn trueos_window_id(&self) -> u32 {
        self.trueos_window_id()
    }
    fn trueos_capture_input(&self, enabled: bool) {
        self.inner.input.lock().unwrap().enable(enabled);
    }
    fn trueos_take_input(&self) -> InputBatch {
        self.inner.input.lock().unwrap().take()
    }
}

/// Opt in to one input window with independently writable scene and UI layers.
/// Standard `create_window` continues to create a single visual frame.
pub trait ActiveEventLoopExtTrueOS {
    fn create_layered_window(
        &self,
        attributes: winit_core::window::WindowAttributes,
        background_hz: u32,
    ) -> Result<Box<dyn CoreWindow>, winit_core::error::RequestError>;
}
impl ActiveEventLoopExtTrueOS for dyn winit_core::event_loop::ActiveEventLoop + '_ {
    fn create_layered_window(
        &self,
        attributes: winit_core::window::WindowAttributes,
        background_hz: u32,
    ) -> Result<Box<dyn CoreWindow>, winit_core::error::RequestError> {
        let event_loop =
            self.cast_ref::<ActiveEventLoop>().expect("non-TRUEOS event loop on TRUEOS");
        Window::new_with_layers(event_loop, attributes, Some(background_hz))
            .map(|window| Box::new(window) as Box<dyn CoreWindow>)
    }
}
