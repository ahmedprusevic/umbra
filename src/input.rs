use std::collections::HashSet;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Polled keyboard/mouse state plus one-frame edge events.
///
/// The winit loop feeds this via the `pub(crate)` `on_*` setters; game code
/// reads it inside `EngineApp::frame`. `end_frame` (called by the app loop
/// after your frame callback) clears the per-frame edges.
#[derive(Default)]
pub struct Input {
    keys_down: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    keys_released: HashSet<KeyCode>,
    mouse_pos: (f32, f32),
    buttons_down: HashSet<MouseButton>,
    buttons_pressed: HashSet<MouseButton>,
    buttons_released: HashSet<MouseButton>,
    scroll: f32,
}

impl Input {
    pub fn is_down(&self, key: KeyCode) -> bool { self.keys_down.contains(&key) }
    pub fn pressed(&self, key: KeyCode) -> bool { self.keys_pressed.contains(&key) }
    pub fn released(&self, key: KeyCode) -> bool { self.keys_released.contains(&key) }
    pub fn mouse(&self) -> (f32, f32) { self.mouse_pos }
    pub fn mouse_down(&self, b: MouseButton) -> bool { self.buttons_down.contains(&b) }
    pub fn mouse_pressed(&self, b: MouseButton) -> bool { self.buttons_pressed.contains(&b) }
    pub fn mouse_released(&self, b: MouseButton) -> bool { self.buttons_released.contains(&b) }
    pub fn scroll(&self) -> f32 { self.scroll }

    pub(crate) fn on_key(&mut self, key: KeyCode, down: bool) {
        if down {
            if self.keys_down.insert(key) {
                self.keys_pressed.insert(key); // insert() is false on key-repeat
            }
        } else {
            self.keys_down.remove(&key);
            self.keys_released.insert(key);
        }
    }

    pub(crate) fn on_mouse_button(&mut self, b: MouseButton, down: bool) {
        if down {
            if self.buttons_down.insert(b) {
                self.buttons_pressed.insert(b);
            }
        } else {
            self.buttons_down.remove(&b);
            self.buttons_released.insert(b);
        }
    }

    pub(crate) fn on_cursor(&mut self, x: f32, y: f32) { self.mouse_pos = (x, y); }
    pub(crate) fn on_scroll(&mut self, delta: f32) { self.scroll += delta; }

    pub(crate) fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.scroll = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_down_and_edge_lifecycle() {
        let mut input = Input::default();
        input.on_key(KeyCode::KeyW, true);
        assert!(input.is_down(KeyCode::KeyW));
        assert!(input.pressed(KeyCode::KeyW));
        input.end_frame();
        assert!(input.is_down(KeyCode::KeyW), "held key stays down");
        assert!(!input.pressed(KeyCode::KeyW), "edge cleared after frame");
        input.on_key(KeyCode::KeyW, false);
        assert!(!input.is_down(KeyCode::KeyW));
        assert!(input.released(KeyCode::KeyW));
        input.end_frame();
        assert!(!input.released(KeyCode::KeyW));
    }

    #[test]
    fn key_repeat_does_not_retrigger_pressed() {
        let mut input = Input::default();
        input.on_key(KeyCode::Space, true);
        input.end_frame();
        input.on_key(KeyCode::Space, true); // OS key-repeat
        assert!(!input.pressed(KeyCode::Space));
        assert!(input.is_down(KeyCode::Space));
    }

    #[test]
    fn mouse_buttons_and_position() {
        let mut input = Input::default();
        input.on_cursor(100.0, 50.0);
        assert_eq!(input.mouse(), (100.0, 50.0));
        input.on_mouse_button(MouseButton::Left, true);
        assert!(input.mouse_down(MouseButton::Left));
        assert!(input.mouse_pressed(MouseButton::Left));
        assert!(!input.mouse_down(MouseButton::Right));
        input.end_frame();
        input.on_mouse_button(MouseButton::Left, false);
        assert!(input.mouse_released(MouseButton::Left));
        assert!(!input.mouse_down(MouseButton::Left));
    }

    #[test]
    fn scroll_accumulates_within_frame_then_resets() {
        let mut input = Input::default();
        input.on_scroll(1.0);
        input.on_scroll(0.5);
        assert_eq!(input.scroll(), 1.5);
        input.end_frame();
        assert_eq!(input.scroll(), 0.0);
    }
}
