//! Gesture recognizer distinguishing Taps (use/place), Long Press (attack/mine),
//! and Camera Swipes (turn look) in open touchscreen areas.

use std::time::{Duration, Instant};

const SLOP_PIXELS: f32 = 14.0;
const LONG_PRESS_DURATION: Duration = Duration::from_millis(380);
const TAP_MAX_DURATION: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureType {
    /// Initial state, waiting to determine gesture.
    Pending,
    /// Dragging / swiping to turn the camera.
    CameraDrag,
    /// Holding stationary to mine / break blocks.
    MiningHold,
}

#[derive(Debug, Clone)]
pub struct PointerGesture {
    pub pointer_id: i32,
    pub start_pos: (f32, f32),
    pub current_pos: (f32, f32),
    pub start_time: Instant,
    pub state: GestureType,
}

/// Output event from gesture processing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureOutput {
    None,
    /// Single tap: interact / place item / use.
    Tap { x: f32, y: f32 },
    /// Started holding: start attacking / mining block.
    StartMining { x: f32, y: f32 },
    /// Stopped holding: stop attacking / mining.
    StopMining,
    /// Moving camera: delta (yaw, pitch).
    CameraDelta { dx: f32, dy: f32 },
}

#[derive(Debug, Default)]
pub struct GestureRecognizer {
    active_gestures: Vec<PointerGesture>,
}

impl GestureRecognizer {
    pub fn new() -> Self {
        Self {
            active_gestures: Vec::new(),
        }
    }

    pub fn on_pointer_down(&mut self, pointer_id: i32, x: f32, y: f32) {
        // Remove old entry if same pointer_id existed
        self.active_gestures.retain(|g| g.pointer_id != pointer_id);
        self.active_gestures.push(PointerGesture {
            pointer_id,
            start_pos: (x, y),
            current_pos: (x, y),
            start_time: Instant::now(),
            state: GestureType::Pending,
        });
    }

    pub fn on_pointer_move(
        &mut self,
        pointer_id: i32,
        x: f32,
        y: f32,
        sensitivity: f32,
    ) -> GestureOutput {
        let Some(gesture) = self.active_gestures.iter_mut().find(|g| g.pointer_id == pointer_id) else {
            return GestureOutput::None;
        };

        let last_x = gesture.current_pos.0;
        let last_y = gesture.current_pos.1;
        gesture.current_pos = (x, y);

        let dx_from_start = x - gesture.start_pos.0;
        let dy_from_start = y - gesture.start_pos.1;
        let dist_from_start = (dx_from_start * dx_from_start + dy_from_start * dy_from_start).sqrt();

        match gesture.state {
            GestureType::Pending => {
                if dist_from_start > SLOP_PIXELS {
                    gesture.state = GestureType::CameraDrag;
                    let dx = (x - last_x) * sensitivity;
                    let dy = (y - last_y) * sensitivity;
                    GestureOutput::CameraDelta { dx, dy }
                } else if gesture.start_time.elapsed() >= LONG_PRESS_DURATION {
                    gesture.state = GestureType::MiningHold;
                    GestureOutput::StartMining { x, y }
                } else {
                    GestureOutput::None
                }
            }
            GestureType::CameraDrag => {
                let dx = (x - last_x) * sensitivity;
                let dy = (y - last_y) * sensitivity;
                GestureOutput::CameraDelta { dx, dy }
            }
            GestureType::MiningHold => {
                // If moved too far while mining, cancel mining and switch to camera
                if dist_from_start > SLOP_PIXELS * 2.5 {
                    gesture.state = GestureType::CameraDrag;
                    GestureOutput::StopMining
                } else {
                    GestureOutput::None
                }
            }
        }
    }

    pub fn on_pointer_up(&mut self, pointer_id: i32) -> GestureOutput {
        let Some(idx) = self.active_gestures.iter().position(|g| g.pointer_id == pointer_id) else {
            return GestureOutput::None;
        };
        let gesture = self.active_gestures.remove(idx);

        match gesture.state {
            GestureType::MiningHold => GestureOutput::StopMining,
            GestureType::CameraDrag => GestureOutput::None,
            GestureType::Pending => {
                let dx = gesture.current_pos.0 - gesture.start_pos.0;
                let dy = gesture.current_pos.1 - gesture.start_pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                if gesture.start_time.elapsed() <= TAP_MAX_DURATION && dist <= SLOP_PIXELS {
                    GestureOutput::Tap {
                        x: gesture.current_pos.0,
                        y: gesture.current_pos.1,
                    }
                } else {
                    GestureOutput::None
                }
            }
        }
    }

    /// Check stationary pointers in Pending state that have reached the long-press threshold.
    pub fn tick(&mut self) -> Vec<GestureOutput> {
        let mut outputs = Vec::new();
        for gesture in &mut self.active_gestures {
            if gesture.state == GestureType::Pending
                && gesture.start_time.elapsed() >= LONG_PRESS_DURATION
            {
                let dx = gesture.current_pos.0 - gesture.start_pos.0;
                let dy = gesture.current_pos.1 - gesture.start_pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= SLOP_PIXELS {
                    gesture.state = GestureType::MiningHold;
                    outputs.push(GestureOutput::StartMining {
                        x: gesture.current_pos.0,
                        y: gesture.current_pos.1,
                    });
                }
            }
        }
        outputs
    }

    pub fn reset(&mut self) {
        self.active_gestures.clear();
    }
}

