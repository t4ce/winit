use core::num::NonZeroU32;
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event_loop::EventLoopProxyProvider;
use winit_core::icon::Icon;
use winit_core::monitor::{Fullscreen, MonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel, WindowType,
};

use crate::abi;
use crate::event_loop::{ActiveEventLoop, EventLoopProxy, Ui4Connection};
#[derive(Debug)]
pub(crate) struct WindowInner {
    pub id: u32,
    pub connection: Arc<Ui4Connection>,
    pub size: Mutex<PhysicalSize<u32>>,
    pub pending_resize: Mutex<Option<crate::abi::ResizeEvent>>,
    pub position: Mutex<PhysicalPosition<i32>>,
    pub focused: Mutex<Option<bool>>,
    pub closed: AtomicBool,
    pub redraws: Arc<Mutex<VecDeque<WindowId>>>,
    pub pending_window_events: Arc<Mutex<VecDeque<(WindowId, winit_core::event::WindowEvent)>>>,
    pub proxy: Arc<EventLoopProxy>,
    pub input: Mutex<crate::input_ext::InputQueue>,
    pub keyboard: Mutex<crate::input::KeyboardState>,
}
#[derive(Debug)]
pub struct Window {
    pub(crate) inner: Arc<WindowInner>,
}
impl Window {
    pub(crate) fn new(el: &ActiveEventLoop, a: WindowAttributes) -> Result<Self, RequestError> {
        Self::new_with_layers(el, a, None)
    }
    pub(crate) fn new_with_layers(
        el: &ActiveEventLoop,
        a: WindowAttributes,
        background_hz: Option<u32>,
    ) -> Result<Self, RequestError> {
        if background_hz.is_some_and(|hz| hz == 0 || hz > 60) {
            return Err(NotSupportedError::new("UI4 background cadence must be 1..=60 Hz").into());
        }
        if a.window_type() != WindowType::Window {
            return Err(NotSupportedError::new("TRUEOS UI4 supports top-level windows only").into());
        };
        let s =
            a.surface_size.unwrap_or(Size::Physical(PhysicalSize::new(1024, 768))).to_physical(1.);
        let p =
            a.position.unwrap_or(Position::Physical(PhysicalPosition::new(0, 0))).to_physical(1.);
        if s.width == 0 || s.height == 0 {
            return Err(NotSupportedError::new("zero-sized UI4 frame").into());
        };
        let id = unsafe {
            match background_hz {
                Some(hz) => abi::trueos_cabi_ui4_scene_frame_open_layered_v1(
                    p.x, p.y, s.width, s.height, hz,
                ),
                None => {
                    abi::trueos_cabi_ui4_scene_frame_open_visual(p.x, p.y, s.width, s.height, 60)
                },
            }
        };
        if id == 0 {
            return Err(NotSupportedError::new("UI4 refused the visual frame").into());
        };
        if unsafe { abi::trueos_cabi_ui4_display_validate_window_v1(el.connection.raw().get(), id) }
            != 0
        {
            unsafe { abi::trueos_cabi_ui4_solara_frame_close(id) };
            return Err(NotSupportedError::new("UI4 rejected the visual frame connection").into());
        }
        let initialized = unsafe {
            abi::trueos_cabi_ui4_scene_frame_set_escape_key_action(id, 1) == 0
                && abi::trueos_cabi_ui4_scene_window_title_set_v1(
                    id,
                    a.title.as_ptr(),
                    a.title.len(),
                ) == 0
        };
        if !initialized {
            unsafe { abi::trueos_cabi_ui4_solara_frame_close(id) };
            return Err(NotSupportedError::new("UI4 could not initialize the visual frame").into());
        }
        if !a.visible {
            let mut state = abi::WindowStateV1::default();
            let hidden = unsafe {
                abi::trueos_cabi_ui4_scene_window_state_get_v1(id, &mut state) == 0
                    && state.version == abi::WINDOW_STATE_V1_VERSION
                    && {
                        state.visible = 0;
                        abi::trueos_cabi_ui4_scene_window_state_set_v1(id, &state) == 0
                    }
            };
            if !hidden {
                unsafe { abi::trueos_cabi_ui4_solara_frame_close(id) };
                return Err(NotSupportedError::new("UI4 could not hide the visual frame").into());
            }
        }
        let inner = Arc::new(WindowInner {
            id,
            connection: el.connection.clone(),
            size: Mutex::new(s),
            pending_resize: Mutex::new(None),
            position: Mutex::new(p),
            focused: Mutex::new(None),
            closed: AtomicBool::new(false),
            redraws: el.redraws.clone(),
            pending_window_events: el.pending_window_events.clone(),
            proxy: el.proxy.clone(),
            input: Mutex::new(crate::input_ext::InputQueue::default()),
            keyboard: Mutex::new(crate::input::KeyboardState::default()),
        });
        el.windows.lock().unwrap().push(Arc::downgrade(&inner));
        inner.redraws.lock().unwrap().push_back(WindowId::from_raw(id as usize));
        inner.proxy.wake_up();
        Ok(Self { inner })
    }
    pub fn trueos_window_id(&self) -> u32 {
        self.inner.id
    }
    fn unsupported() -> RequestError {
        NotSupportedError::new("unsupported by TRUEOS UI4").into()
    }
    fn state(&self) -> Option<abi::WindowStateV1> {
        let mut state = abi::WindowStateV1::default();
        let status =
            unsafe { abi::trueos_cabi_ui4_scene_window_state_get_v1(self.inner.id, &mut state) };
        (status == 0 && state.version == abi::WINDOW_STATE_V1_VERSION).then_some(state)
    }
    fn update_state(&self, update: impl FnOnce(&mut abi::WindowStateV1)) {
        let Some(mut state) = self.state() else { return };
        update(&mut state);
        let _ = unsafe { abi::trueos_cabi_ui4_scene_window_state_set_v1(self.inner.id, &state) };
    }
}
impl CoreWindow for Window {
    fn window_type(&self) -> WindowType {
        WindowType::Window
    }
    fn id(&self) -> WindowId {
        WindowId::from_raw(self.inner.id as usize)
    }
    fn scale_factor(&self) -> f64 {
        1.
    }
    fn request_redraw(&self) {
        if self.inner.closed.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }
        let id = self.id();
        let mut q = self.inner.redraws.lock().unwrap();
        if !q.contains(&id) {
            q.push_back(id);
            self.inner.proxy.wake_up();
        }
    }
    fn pre_present_notify(&self) {}
    fn reset_dead_keys(&self) {}
    fn surface_position(&self) -> PhysicalPosition<i32> {
        PhysicalPosition::new(0, 0)
    }
    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        let mut xy = [0i32; 2];
        if unsafe { abi::trueos_cabi_ui4_scene_frame_get_position(self.inner.id, xy.as_mut_ptr()) }
            == 0
        {
            let position = PhysicalPosition::new(xy[0], xy[1]);
            *self.inner.position.lock().unwrap() = position;
            Ok(position)
        } else {
            Err(Self::unsupported())
        }
    }
    fn set_outer_position(&self, p: Position) {
        let p = p.to_physical(1.);
        if unsafe { abi::trueos_cabi_ui4_scene_frame_set_position(self.inner.id, p.x, p.y) } == 0 {
            *self.inner.position.lock().unwrap() = p
        }
    }
    fn surface_size(&self) -> PhysicalSize<u32> {
        *self.inner.size.lock().unwrap()
    }
    fn request_surface_size(&self, s: Size) -> Option<PhysicalSize<u32>> {
        let s = s.to_physical(1.);
        if unsafe { abi::trueos_cabi_ui4_scene_frame_resize(self.inner.id, s.width, s.height) } == 0
        {
            *self.inner.size.lock().unwrap() = s;
            Some(s)
        } else {
            None
        }
    }
    fn outer_size(&self) -> PhysicalSize<u32> {
        self.surface_size()
    }
    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }
    fn set_min_surface_size(&self, _: Option<Size>) {}
    fn set_max_surface_size(&self, _: Option<Size>) {}
    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }
    fn set_surface_resize_increments(&self, _: Option<Size>) {}
    fn set_title(&self, title: &str) {
        if title.len() <= abi::MAX_WINDOW_TITLE_BYTES {
            let _ = unsafe {
                abi::trueos_cabi_ui4_scene_window_title_set_v1(
                    self.inner.id,
                    title.as_ptr(),
                    title.len(),
                )
            };
        }
    }
    fn title(&self) -> String {
        let mut bytes = [0u8; abi::MAX_WINDOW_TITLE_BYTES];
        let len = unsafe {
            abi::trueos_cabi_ui4_scene_window_title_get_v1(
                self.inner.id,
                bytes.as_mut_ptr(),
                bytes.len(),
            )
        };
        usize::try_from(len)
            .ok()
            .and_then(|len| core::str::from_utf8(&bytes[..len]).ok())
            .unwrap_or_default()
            .into()
    }
    fn set_transparent(&self, _: bool) {}
    fn set_blur(&self, _: bool) {}
    fn set_visible(&self, visible: bool) {
        self.update_state(|state| state.visible = visible as u32);
    }
    fn is_visible(&self) -> Option<bool> {
        self.state().map(|state| state.visible != 0)
    }
    fn set_resizable(&self, _: bool) {}
    fn is_resizable(&self) -> bool {
        true
    }
    fn set_enabled_buttons(&self, _: WindowButtons) {}
    fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }
    fn set_minimized(&self, _: bool) {}
    fn is_minimized(&self) -> Option<bool> {
        None
    }
    fn set_maximized(&self, _: bool) {}
    fn is_maximized(&self) -> bool {
        false
    }
    fn set_fullscreen(&self, _: Option<Fullscreen>) {}
    fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }
    fn set_decorations(&self, _: bool) {}
    fn is_decorated(&self) -> bool {
        false
    }
    fn set_window_level(&self, _: WindowLevel) {}
    fn set_window_icon(&self, _: Option<Icon>) {}
    fn request_ime_update(&self, _: ImeRequest) -> Result<(), ImeRequestError> {
        Err(ImeRequestError::NotSupported)
    }
    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        None
    }
    fn focus_window(&self) {}
    fn has_focus(&self) -> bool {
        false
    }
    fn request_user_attention(&self, _: Option<UserAttentionType>) {}
    fn set_theme(&self, _: Option<Theme>) {}
    fn theme(&self) -> Option<Theme> {
        None
    }
    fn set_content_protected(&self, _: bool) {}
    fn set_cursor(&self, _: Cursor) {}
    fn set_cursor_position(&self, _: Position) -> Result<(), RequestError> {
        Err(Self::unsupported())
    }
    fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), RequestError> {
        Err(Self::unsupported())
    }
    fn set_cursor_visible(&self, _: bool) {}
    fn drag_window(&self) -> Result<(), RequestError> {
        Err(Self::unsupported())
    }
    fn drag_resize_window(&self, _: ResizeDirection) -> Result<(), RequestError> {
        Err(Self::unsupported())
    }
    fn show_window_menu(&self, _: Position) {}
    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        let Some(mut state) = self.state() else { return Err(Self::unsupported()) };
        state.hit_testable = hittest as u32;
        if unsafe { abi::trueos_cabi_ui4_scene_window_state_set_v1(self.inner.id, &state) } == 0 {
            Ok(())
        } else {
            Err(Self::unsupported())
        }
    }
    fn current_monitor(&self) -> Option<MonitorHandle> {
        None
    }
    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        Box::new(std::iter::empty())
    }
    fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }
    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}
impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let Some(window) = NonZeroU32::new(self.inner.id) else {
            return Err(rwh_06::HandleError::Unavailable);
        };
        let raw = rwh_06::RawWindowHandle::Trueos(rwh_06::TrueosWindowHandle::new(window));
        // SAFETY: `raw` contains only the live UI4 frame identifier.
        Ok(unsafe { rwh_06::WindowHandle::borrow_raw(raw) })
    }
}
impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        self.inner.connection.display_handle()
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        self.inner.closed.store(true, std::sync::atomic::Ordering::Release);
        unsafe {
            abi::trueos_cabi_ui4_solara_frame_close(self.inner.id);
        }
        self.inner
            .pending_window_events
            .lock()
            .unwrap()
            .push_back((self.id(), winit_core::event::WindowEvent::Destroyed));
        self.inner.proxy.wake_up();
    }
}
