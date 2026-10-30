//! Touchscreen input system porting PocketMC's `TouchscreenInput.cpp`.

use super::area::{RectangleArea, TouchAreaModel};

// PocketMC Button Area IDs
pub const AREA_DPAD_N: i32 = 100;
pub const AREA_DPAD_S: i32 = 101;
pub const AREA_DPAD_W: i32 = 102;
pub const AREA_DPAD_E: i32 = 103;
pub const AREA_DPAD_C: i32 = 104; // Jump / Fly Up
pub const AREA_PAUSE: i32 = 105;
pub const AREA_CHAT: i32 = 106;
pub const AREA_THIRD: i32 = 107;
pub const AREA_SNEAK: i32 = 108; // Center of D-pad
pub const AREA_FLY_DOWN: i32 = 109;
pub const AREA_FLIGHT_TOGGLE: i32 = 110;

/// Configuration options matching PocketMC options.
#[derive(Debug, Clone)]
pub struct TouchOptions {
    pub is_left_handed: bool,
    pub auto_jump: bool,
    pub allow_sprint: bool,
    pub touch_movement_area: bool,
    pub sensitivity: f32,
}

impl Default for TouchOptions {
    fn default() -> Self {
        Self {
            is_left_handed: false,
            auto_jump: true,
            allow_sprint: true,
            touch_movement_area: false,
            sensitivity: 0.15,
        }
    }
}

/// Dynamic touch input layout and state tracker.
#[derive(Debug, Clone)]
pub struct TouchscreenInput {
    pub options: TouchOptions,
    pub model: TouchAreaModel,
    pub bounding_dpad: RectangleArea,
    pub width: f32,
    pub height: f32,

    // Button states
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub fly_up: bool,
    pub fly_down: bool,
    pub is_flying: bool,

    // Single-shot action toggles
    pub toggle_pause: bool,
    pub toggle_chat: bool,
    pub toggle_perspective: bool,
    pub toggle_flight: bool,

    // Track which area each pointer is pressing
    pointer_areas: Vec<(i32, i32)>, // (pointer_id, area_id)
}

impl TouchscreenInput {
    pub fn new(width: f32, height: f32, options: TouchOptions) -> Self {
        let mut input = Self {
            options,
            model: TouchAreaModel::new(),
            bounding_dpad: RectangleArea::new(0.0, 0.0, 1.0, 1.0),
            width,
            height,
            forward: false,
            backward: false,
            left: false,
            right: false,
            jump: false,
            sneak: false,
            fly_up: false,
            fly_down: false,
            is_flying: false,
            toggle_pause: false,
            toggle_chat: false,
            toggle_perspective: false,
            toggle_flight: false,
            pointer_areas: Vec::new(),
        };
        input.rebuild_layout(width, height);
        input
    }

    /// Rebuilds button geometries according to PocketMC proportional sizing.
    pub fn rebuild_layout(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        self.model.clear();

        let w = width;
        let h = height;

        // Button dimensions: PocketMC proportional sizing
        let bw = (w * 0.08).clamp(64.0, 120.0);
        let bh = bw;

        let margin_bottom = 24.0;
        let margin_side = 24.0;

        let base_y = h - 3.0 * bh - margin_bottom;
        let base_x = if self.options.is_left_handed {
            w - 3.0 * bw - margin_side
        } else {
            margin_side
        };

        // Bounding rectangle for the 3x3 D-pad
        self.bounding_dpad = RectangleArea::new(base_x, base_y, base_x + 3.0 * bw, base_y + 3.0 * bh);

        // 1. D-pad North (Forward)
        let xx = base_x + bw;
        let yy = base_y;
        self.model.add_area(AREA_DPAD_N, RectangleArea::new(xx, yy, xx + bw, yy + bh));

        // 2. D-pad Center (Sneak / Shift)
        let xx = base_x + bw;
        let yy = base_y + bh;
        self.model.add_area(AREA_SNEAK, RectangleArea::new(xx, yy, xx + bw, yy + bh));

        // 3. D-pad South (Backward)
        let xx = base_x + bw;
        let yy = base_y + 2.0 * bh;
        self.model.add_area(AREA_DPAD_S, RectangleArea::new(xx, yy, xx + bw, yy + bh));

        // 4. D-pad West (Left)
        let xx = base_x;
        let yy = base_y + bh;
        self.model.add_area(AREA_DPAD_W, RectangleArea::new(xx, yy, xx + bw, yy + bh));

        // 5. D-pad East (Right)
        let xx = base_x + 2.0 * bw;
        let yy = base_y + bh;
        self.model.add_area(AREA_DPAD_E, RectangleArea::new(xx, yy, xx + bw, yy + bh));

        // Right stack: Jump, Fly Down, Flight Toggle
        let base_x_right = if self.options.is_left_handed {
            margin_side + bw
        } else {
            w - margin_side - 2.0 * bw
        };
        let base_y_right = base_y;

        // Jump / Fly Up (middle height)
        let xx = base_x_right;
        let yy = base_y_right + bh;
        self.model.add_area(AREA_DPAD_C, RectangleArea::new(xx, yy, xx + bw, yy + bh));

        // Flight Toggle (only active when flying or in creative)
        self.model.add_area(AREA_FLIGHT_TOGGLE, RectangleArea::new(xx, yy, xx + bw, yy + bh));

        // Fly Down (bottom)
        let yy_down = base_y_right + 2.0 * bh;
        self.model.add_area(AREA_FLY_DOWN, RectangleArea::new(xx, yy_down, xx + bw, yy_down + bh));

        // Top bar buttons: Chat, Third-Person, Pause
        let btn_size = (h * 0.08).clamp(44.0, 72.0);
        let spacing = 12.0;
        let total_top_w = btn_size * 3.0 + spacing * 2.0;
        let top_left = (w - total_top_w) * 0.5;
        let top_y = 12.0;

        self.model.add_area(
            AREA_CHAT,
            RectangleArea::new(top_left, top_y, top_left + btn_size, top_y + btn_size),
        );
        let top_third = top_left + btn_size + spacing;
        self.model.add_area(
            AREA_THIRD,
            RectangleArea::new(top_third, top_y, top_third + btn_size, top_y + btn_size),
        );
        let top_pause = top_third + btn_size + spacing;
        self.model.add_area(
            AREA_PAUSE,
            RectangleArea::new(top_pause, top_y, top_pause + btn_size, top_y + btn_size),
        );
    }

    /// Update with touch event.
    /// Returns `Some(area_id)` if consumed by a UI button, or `None` if in the camera/gesture area.
    pub fn on_touch(&mut self, pointer_id: i32, action: i32, x: f32, y: f32) -> Option<i32> {
        match action {
            0 | 5 => {
                // Down or Pointer Down
                if let Some(area_id) = self.model.get_area_id(x, y) {
                    self.pointer_areas.retain(|(p, _)| *p != pointer_id);
                    self.pointer_areas.push((pointer_id, area_id));
                    self.handle_button_press(area_id);
                    Some(area_id)
                } else {
                    None
                }
            }
            2 => {
                // Move: if pointer was in a button, check if it moved outside or into another button
                let current_area = self.model.get_area_id(x, y);
                let prev_area = self.pointer_areas.iter().find(|(p, _)| *p == pointer_id).map(|(_, a)| *a);

                if prev_area != current_area {
                    if let Some(old_a) = prev_area {
                        self.handle_button_release(old_a);
                    }
                    self.pointer_areas.retain(|(p, _)| *p != pointer_id);
                    if let Some(new_a) = current_area {
                        self.pointer_areas.push((pointer_id, new_a));
                        self.handle_button_press(new_a);
                        Some(new_a)
                    } else {
                        None
                    }
                } else {
                    current_area
                }
            }
            1 | 6 | 3 => {
                // Up, Pointer Up, or Cancel
                if let Some(pos) = self.pointer_areas.iter().position(|(p, _)| *p == pointer_id) {
                    let (_, area_id) = self.pointer_areas.remove(pos);
                    self.handle_button_release(area_id);
                    Some(area_id)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn handle_button_press(&mut self, area_id: i32) {
        match area_id {
            AREA_DPAD_N => self.forward = true,
            AREA_DPAD_S => self.backward = true,
            AREA_DPAD_W => self.left = true,
            AREA_DPAD_E => self.right = true,
            AREA_DPAD_C => {
                self.jump = true;
                if self.is_flying {
                    self.fly_up = true;
                }
            }
            AREA_SNEAK => {
                self.sneak = !self.sneak; // toggle sneak like PocketMC
            }
            AREA_FLIGHT_TOGGLE => {
                self.toggle_flight = true;
            }
            AREA_FLY_DOWN => {
                if self.is_flying {
                    self.fly_down = true;
                }
            }
            AREA_PAUSE => self.toggle_pause = true,
            AREA_CHAT => self.toggle_chat = true,
            AREA_THIRD => self.toggle_perspective = true,
            _ => {}
        }
    }

    fn handle_button_release(&mut self, area_id: i32) {
        match area_id {
            AREA_DPAD_N => self.forward = false,
            AREA_DPAD_S => self.backward = false,
            AREA_DPAD_W => self.left = false,
            AREA_DPAD_E => self.right = false,
            AREA_DPAD_C => {
                self.jump = false;
                self.fly_up = false;
            }
            AREA_FLY_DOWN => self.fly_down = false,
            _ => {}
        }
    }

    /// Reset all button states.
    pub fn release_all(&mut self) {
        self.forward = false;
        self.backward = false;
        self.left = false;
        self.right = false;
        self.jump = false;
        self.fly_up = false;
        self.fly_down = false;
        self.pointer_areas.clear();
    }
}

