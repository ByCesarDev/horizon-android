//! Bridge connecting TouchActionState to Horizon game actions.

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use super::TouchActionState;

/// Concrete game events produced by the touch subsystem each frame/tick.
#[derive(Debug, Default, Clone)]
pub struct TouchGameEvents {
    /// Normalized direction [strafe, forward] where 1.0 is forward/right, -1.0 is backward/left.
    pub move_direction: [f32; 2],
    /// Camera look delta (degrees or pixels: yaw, pitch).
    pub look_delta: (f32, f32),
    /// Continuous key state changes: (key, pressed).
    pub key_changes: Vec<(KeyCode, bool)>,
    /// Single-shot key presses (Escape, T, F5).
    pub key_presses: Vec<KeyCode>,
    /// Mouse button down/up transitions (e.g. Left button for attack/mining).
    pub mouse_changes: Vec<(MouseButton, bool)>,
    /// Single-shot right-click tap (place block / interact).
    pub use_click: bool,
}

/// State bridge tracking previous action state to emit clean edge-triggered events.
#[derive(Debug, Default, Clone)]
pub struct TouchBridge {
    pub previous_state: TouchActionState,
}

impl TouchBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates the current `TouchActionState` against previous state and produces
    /// concrete game events (movement, key transitions, mouse clicks, look deltas).
    pub fn process_state(&mut self, current: TouchActionState) -> TouchGameEvents {
        let prev = self.previous_state;
        let mut events = TouchGameEvents {
            move_direction: current.move_direction,
            look_delta: current.look_delta,
            ..Default::default()
        };

        // 1. D-pad movement keys (W, A, S, D)
        let forward = current.move_direction[1] > 0.3;
        let backward = current.move_direction[1] < -0.3;
        let left = current.move_direction[0] < -0.3;
        let right = current.move_direction[0] > 0.3;

        let prev_forward = prev.move_direction[1] > 0.3;
        let prev_backward = prev.move_direction[1] < -0.3;
        let prev_left = prev.move_direction[0] < -0.3;
        let prev_right = prev.move_direction[0] > 0.3;

        if forward != prev_forward {
            events.key_changes.push((KeyCode::KeyW, forward));
        }
        if backward != prev_backward {
            events.key_changes.push((KeyCode::KeyS, backward));
        }
        if left != prev_left {
            events.key_changes.push((KeyCode::KeyA, left));
        }
        if right != prev_right {
            events.key_changes.push((KeyCode::KeyD, right));
        }

        // 2. Jump (Space)
        if current.jump != prev.jump {
            events.key_changes.push((KeyCode::Space, current.jump));
        }

        // 3. Sneak (ShiftLeft)
        if current.sneak != prev.sneak {
            events.key_changes.push((KeyCode::ShiftLeft, current.sneak));
        }

        // 4. Attack / Mine (MouseButton::Left)
        if current.attack != prev.attack {
            events.mouse_changes.push((MouseButton::Left, current.attack));
        }

        // 5. Use / Tap (MouseButton::Right)
        if current.use_item {
            events.use_click = true;
        }

        // 6. UI single-shot toggles
        if current.toggle_pause {
            events.key_presses.push(KeyCode::Escape);
        }
        if current.toggle_chat {
            events.key_presses.push(KeyCode::KeyT);
        }
        if current.toggle_perspective {
            events.key_presses.push(KeyCode::F5);
        }

        self.previous_state = current;
        events
    }
}

