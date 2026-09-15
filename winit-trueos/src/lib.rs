//! TRUEOS UI4 backend.
//!
//! A TRUEOS window is a UI4 visual frame. UI4 owns presentation, so it does
//! not expose a raw native graphics surface.
mod abi;
mod event_loop;
mod input;
mod window;
pub use event_loop::{
    ActiveEventLoop, EventLoop, EventLoopProxy, PlatformSpecificEventLoopAttributes,
};
pub use input::{physicalkey_to_scancode, scancode_to_physicalkey};
pub use window::Window;
use winit_core::window::Window as CoreWindow;
pub trait WindowExtTrueOS {
    fn trueos_window_id(&self) -> u32;
}
impl WindowExtTrueOS for dyn CoreWindow + '_ {
    fn trueos_window_id(&self) -> u32 {
        self.cast_ref::<Window>().expect("non-TRUEOS window on TRUEOS").trueos_window_id()
    }
}
