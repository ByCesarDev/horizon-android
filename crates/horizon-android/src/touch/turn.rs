//! Touch turn (camera look) input porting PocketMC's `TouchTurnInput.h`.

/// Represents a delta turn for the camera (Yaw and Pitch).
#[derive(Debug, Default, Clone, Copy)]
pub struct TurnDelta {
    pub dx: f32,
    pub dy: f32,
}

/// State tracking for camera rotation swipes.
#[derive(Debug, Clone)]
pub struct TouchTurnInput {
    pub sensitivity: f32,
    pub dead_zone: f32,
    last_x: f32,
    last_y: f32,
    active_pointer_id: Option<i32>,
}

impl Default for TouchTurnInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchTurnInput {
    pub fn new() -> Self {
        Self {
            sensitivity: 0.15,
            dead_zone: 0.5,
            last_x: 0.0,
            last_y: 0.0,
            active_pointer_id: None,
        }
    }

    /// Reset any active turning pointer.
    pub fn reset(&mut self) {
        self.active_pointer_id = None;
    }

    /// Notify that a pointer in the camera region started.
    pub fn on_pointer_down(&mut self, pointer_id: i32, x: f32, y: f32) {
        if self.active_pointer_id.is_none() {
            self.active_pointer_id = Some(pointer_id);
            self.last_x = x;
            self.last_y = y;
        }
    }

    /// Update coordinates when a pointer moves.
    /// Returns the accumulated TurnDelta if this pointer is driving the camera.
    pub fn on_pointer_move(&mut self, pointer_id: i32, x: f32, y: f32) -> TurnDelta {
        if self.active_pointer_id == Some(pointer_id) {
            let raw_dx = x - self.last_x;
            let raw_dy = y - self.last_y;
            self.last_x = x;
            self.last_y = y;

            let dx = if raw_dx.abs() > self.dead_zone {
                raw_dx * self.sensitivity
            } else {
                0.0
            };

            let dy = if raw_dy.abs() > self.dead_zone {
                raw_dy * self.sensitivity
            } else {
                0.0
            };

            TurnDelta { dx, dy }
        } else {
            TurnDelta::default()
        }
    }

    /// Notify that a pointer was released or cancelled.
    pub fn on_pointer_up(&mut self, pointer_id: i32) {
        if self.active_pointer_id == Some(pointer_id) {
            self.active_pointer_id = None;
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_pointer_id.is_some()
    }
}

