//! Touch controls ported from PocketMC for Horizon on Android.

pub mod area;
pub mod gesture;
pub mod input;
pub mod turn;

pub use area::{RectangleArea, TouchAreaModel};
pub use gesture::{GestureOutput, GestureRecognizer};
pub use input::{TouchOptions, TouchscreenInput};
pub use turn::{TouchTurnInput, TurnDelta};

/// High-level player action state produced by the touch subsystem.
#[derive(Debug, Default, Clone, Copy)]
pub struct TouchActionState {
    /// Normalized direction [strafe, forward] where 1.0 is forward/right, -1.0 is backward/left.
    pub move_direction: [f32; 2],
    pub jump: bool,
    pub sneak: bool,
    pub fly_up: bool,
    pub fly_down: bool,
    pub attack: bool,
    pub use_item: bool,
    pub look_delta: (f32, f32),
    pub toggle_flight: bool,
    pub toggle_pause: bool,
    pub toggle_chat: bool,
    pub toggle_perspective: bool,
}

/// Central touch manager bridging Android multitouch events to game actions.
#[derive(Debug)]
pub struct TouchManager {
    pub input: TouchscreenInput,
    pub gestures: GestureRecognizer,
    pub turn: TouchTurnInput,
    pub state: TouchActionState,
}

impl TouchManager {
    pub fn new(width: f32, height: f32, options: TouchOptions) -> Self {
        Self {
            input: TouchscreenInput::new(width, height, options),
            gestures: GestureRecognizer::new(),
            turn: TouchTurnInput::new(),
            state: TouchActionState::default(),
        }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.input.rebuild_layout(width, height);
    }

    /// Process a raw Android MotionEvent: (pointer_id, action, x, y).
    /// action: 0=DOWN, 1=UP, 2=MOVE, 3=CANCEL, 5=POINTER_DOWN, 6=POINTER_UP.
    pub fn on_touch(&mut self, pointer_id: i32, action: i32, x: f32, y: f32) {
        // 1. Check if the touch hits a UI button (D-pad, jump, pause, etc.)
        if let Some(_area_id) = self.input.on_touch(pointer_id, action, x, y) {
            // Button touched: cancel any pending camera/mining gesture on this pointer
            self.gestures.on_pointer_up(pointer_id);
            self.turn.on_pointer_up(pointer_id);
            return;
        }

        // 2. Otherwise, touch is in the camera / gesture area
        match action {
            0 | 5 => {
                // Down
                self.gestures.on_pointer_down(pointer_id, x, y);
                self.turn.on_pointer_down(pointer_id, x, y);
            }
            2 => {
                // Move
                let out = self.gestures.on_pointer_move(
                    pointer_id,
                    x,
                    y,
                    self.input.options.sensitivity,
                );
                self.handle_gesture_output(out);

                let delta = self.turn.on_pointer_move(pointer_id, x, y);
                if delta.dx != 0.0 || delta.dy != 0.0 {
                    self.state.look_delta.0 += delta.dx;
                    self.state.look_delta.1 += delta.dy;
                }
            }
            1 | 6 | 3 => {
                // Up or Cancel
                let out = self.gestures.on_pointer_up(pointer_id);
                self.handle_gesture_output(out);
                self.turn.on_pointer_up(pointer_id);
            }
            _ => {}
        }
    }

    fn handle_gesture_output(&mut self, output: GestureOutput) {
        match output {
            GestureOutput::None => {}
            GestureOutput::Tap { .. } => {
                // Place block / interact
                self.state.use_item = true;
            }
            GestureOutput::StartMining { .. } => {
                // Long press: hold attack / break block
                self.state.attack = true;
            }
            GestureOutput::StopMining => {
                self.state.attack = false;
            }
            GestureOutput::CameraDelta { dx, dy } => {
                self.state.look_delta.0 += dx;
                self.state.look_delta.1 += dy;
            }
        }
    }

    /// Update frame: evaluate long-press timers and assemble current ActionState.
    pub fn update(&mut self) -> TouchActionState {
        // Check long-press triggers for stationary pointers
        let outputs = self.gestures.tick();
        for out in outputs {
            self.handle_gesture_output(out);
        }

        // Calculate directional movement
        let forward = self.input.forward;
        let backward = self.input.backward;
        let left = self.input.left;
        let right = self.input.right;

        let impulse_x: f32 = if left == right {
            0.0
        } else if right {
            1.0
        } else {
            -1.0
        };

        let impulse_y: f32 = if forward == backward {
            0.0
        } else if forward {
            1.0
        } else {
            -1.0
        };

        let mag = (impulse_x * impulse_x + impulse_y * impulse_y).sqrt();
        let move_direction = if mag > 1.0e-4 {
            [impulse_x / mag, impulse_y / mag]
        } else {
            [0.0, 0.0]
        };

        self.state.move_direction = move_direction;
        self.state.jump = self.input.jump;
        self.state.sneak = self.input.sneak;
        self.state.fly_up = self.input.fly_up;
        self.state.fly_down = self.input.fly_down;

        // One-shot flags
        self.state.toggle_flight = self.input.toggle_flight;
        self.input.toggle_flight = false;

        self.state.toggle_pause = self.input.toggle_pause;
        self.input.toggle_pause = false;

        self.state.toggle_chat = self.input.toggle_chat;
        self.input.toggle_chat = false;

        self.state.toggle_perspective = self.input.toggle_perspective;
        self.input.toggle_perspective = false;

        // Return a copy and reset per-frame accumulators (e.g. look_delta and use_item)
        let current_state = self.state;
        self.state.look_delta = (0.0, 0.0);
        self.state.use_item = false;

        current_state
    }

    pub fn reset(&mut self) {
        self.input.release_all();
        self.gestures.reset();
        self.turn.reset();
        self.state = TouchActionState::default();
    }
}
