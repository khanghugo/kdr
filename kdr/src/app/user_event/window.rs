use common::KDR_CANVAS_ID;
use tracing::warn;

use crate::app::{App, AppEvent};

impl App {
    pub(super) fn request_resize(&mut self) {
        let Some(window_state) = self.state.window_state.as_ref() else {
            return;
        };

        let width = window_state.width;
        let height = window_state.height;

        let size = winit::dpi::PhysicalSize { width, height };

        // do not use this
        // this will lock the window min size
        // window.set_min_inner_size(size.into());

        if window_state.window().request_inner_size(size).is_none() {
            warn!("Request resize failed");
        }

        self.resize(size.clone());
    }

    pub(super) fn request_enter_fullscreen(&mut self) {
        let Some(window_state) = self.state.window_state.as_mut() else {
            return;
        };

        let window = window_state.window();

        // for some magical reasons, i don't even need to set the width and height???
        // and when exiting fullscreen, the old resolution is restored
        // thank you winit
        if let Some(monitor) = window.current_monitor() {
            window.set_fullscreen(winit::window::Fullscreen::Borderless(monitor.into()).into());
        }

        // on top of monitor fullscreen, also need canvas fullscreen for the web
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let canvas = document.get_element_by_id(KDR_CANVAS_ID).unwrap();

            if canvas.request_fullscreen().is_err() {
                warn!("Failed to request fullscreen");
            }
        }
    }

    pub(super) fn request_exit_fullscreen(&mut self) {
        let Some(window_state) = self.state.window_state.as_mut() else {
            return;
        };

        window_state.window().set_fullscreen(None);

        // doesnt need web specific fullscreen exit
        #[cfg(target_arch = "wasm32")]
        {}
    }

    pub(super) fn request_toggle_fullscreen(&mut self) {
        self.state.window_state.as_ref().map(|window_state| {
            if window_state.is_fullscreen {
                let _ = self
                    .event_loop_proxy
                    .send_event(AppEvent::RequestEnterFullScreen);
            } else {
                let _ = self
                    .event_loop_proxy
                    .send_event(AppEvent::RequestExitFullScreen);
            }
        });
    }
}
