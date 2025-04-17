use std::pin::Pin;

use futures::FutureExt;
use rfd::AsyncFileDialog;
use tracing::warn;

use crate::app::AppEvent;

use super::AppState;

/// This is just to simply tell the program what kind of thing is being loaded so it can reasonably resetting stuffs
pub enum SelectedFileType {
    Bsp,
    Replay,
    None,
}

pub struct FileState {
    pub selected_file: Option<String>,
    pub selected_file_bytes: Option<Vec<u8>>,
    pub selected_file_type: SelectedFileType,

    pub file_dialogue_future:
        Option<Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + 'static>>>,
    pub file_bytes_future: Option<Pin<Box<dyn Future<Output = Vec<u8>> + 'static>>>,
}

impl Default for FileState {
    fn default() -> Self {
        Self {
            selected_file: None,
            selected_file_bytes: None,
            file_dialogue_future: None,
            file_bytes_future: None,
            selected_file_type: SelectedFileType::None,
        }
    }
}

impl FileState {
    pub fn trigger_file_dialogue(&mut self) {
        let future = AsyncFileDialog::new()
            .add_filter("BSP/DEM", &["bsp", "dem"])
            .pick_file();

        self.file_dialogue_future = Some(Box::pin(future))
    }
}

impl AppState {
    pub fn file_state_poll(&mut self) {
        // only read the file name, yet to have the bytes
        if let Some(future) = &mut self.file_state.file_dialogue_future {
            if let Some(file_handle) = future.now_or_never() {
                self.file_state.selected_file = file_handle.map(|f| {
                    #[cfg(not(target_arch = "wasm32"))]
                    let result = f.path().display().to_string();

                    #[cfg(target_arch = "wasm32")]
                    let result = f.file_name();

                    self.file_state.file_bytes_future = Some(Box::pin(async move {
                        let bytes = f.read().await;
                        bytes
                    }));

                    return result;
                });

                self.file_state.file_dialogue_future = None;
            }
        }

        // now have the bytes
        if let Some(future) = &mut self.file_state.file_bytes_future {
            if let Some(file_bytes) = future.now_or_never() {
                self.file_state.selected_file_bytes = file_bytes.into();
                self.file_state.file_bytes_future = None;

                // only new file when we have the bytes
                self.event_loop_proxy
                    .send_event(AppEvent::NewFileSelected)
                    .unwrap_or_else(|_| warn!("Cannot send NewFileSelected"));
            }
        }
    }
}
