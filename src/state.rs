use eframe::epaint::mutex::Mutex;

/// Track what ui elements are shown or not rn
pub struct UiState {
    side_menu: Mutex<bool>,
}

impl UiState {
    pub fn side_menu_opened(&self) -> bool {
        *self.side_menu.lock()
    }

    pub fn open_side_menu(&self) {
        *self.side_menu.lock() = true;
    }

    pub fn close_side_menu(&self) {
        *self.side_menu.lock() = false;
    }

    pub fn toggle_side_menu(&self) {
        let mut side_menu = self.side_menu.lock();
        *side_menu = !*side_menu;
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            side_menu: Mutex::new(false),
        }
    }
}
