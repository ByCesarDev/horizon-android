//! Screen state and navigation for Horizon Android.

/// The active screen/mode in Horizon Android.
#[derive(Debug, Clone, PartialEq)]
pub enum ScreenState {
    /// Main Title Screen ("HORIZON" logo, Singleplayer, Options, Quit).
    MainMenu,
    /// World selection screen (list of worlds in `saves/`).
    SelectWorld { selected: Option<usize> },
    /// Create new world dialog.
    CreateWorld {
        name: String,
        creative: bool,
        difficulty: u8, // 0=Peaceful, 1=Easy, 2=Normal, 3=Hard
    },
    /// In-game active gameplay with 3D view and touch HUD.
    Playing { world_name: String },
    /// In-game pause menu with dimmed backdrop.
    Paused { world_name: String },
    /// Options / settings screen.
    Options { return_to: Box<ScreenState> },
}

impl ScreenState {
    /// Returns true if currently in active gameplay (not in a menu).
    pub fn is_playing(&self) -> bool {
        matches!(self, ScreenState::Playing { .. })
    }

    /// Returns true if currently in a menu overlaying or replacing gameplay.
    pub fn is_menu(&self) -> bool {
        !self.is_playing()
    }
}

/// An interactive screen button.
#[derive(Debug, Clone)]
pub struct ScreenButton {
    pub id: u32,
    pub label: String,
    pub x: f32, // Screen pixels
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub pressed: bool,
    pub enabled: bool,
}

impl ScreenButton {
    pub fn new(id: u32, label: impl Into<String>, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            id,
            label: label.into(),
            x,
            y,
            w,
            h,
            pressed: false,
            enabled: true,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn is_inside(&self, px: f32, py: f32) -> bool {
        self.enabled && px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}
