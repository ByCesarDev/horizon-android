//! Screen manager, state transitions, and interactive UI for Horizon Android.

pub mod font;
pub mod render;
pub mod state;

use std::fs;

use crate::storage::StorageManager;
use render::ScreenRenderer;
pub use state::{ScreenButton, ScreenState};

// Button IDs
const BTN_MAIN_SINGLEPLAYER: u32 = 1;
const BTN_MAIN_MULTIPLAYER: u32 = 2;
const BTN_MAIN_OPTIONS: u32 = 3;
const BTN_MAIN_QUIT: u32 = 4;

const BTN_SELECT_PLAY: u32 = 10;
const BTN_SELECT_CREATE: u32 = 11;
const BTN_SELECT_BACK: u32 = 12;
const BTN_WORLD_BASE: u32 = 100;

const BTN_CREATE_NAME: u32 = 20;
const BTN_CREATE_MODE: u32 = 21;
const BTN_CREATE_DIFF: u32 = 22;
const BTN_CREATE_CONFIRM: u32 = 23;
const BTN_CREATE_CANCEL: u32 = 24;

const BTN_PAUSE_RESUME: u32 = 30;
const BTN_PAUSE_OPTIONS: u32 = 31;
const BTN_PAUSE_QUIT: u32 = 32;

const BTN_OPT_RENDER_DIST: u32 = 40;
const BTN_OPT_GUI_SCALE: u32 = 41;
const BTN_OPT_AUTO_JUMP: u32 = 42;
const BTN_OPT_SENSITIVITY: u32 = 43;
const BTN_OPT_DONE: u32 = 44;

pub struct ScreenManager {
    state: ScreenState,
    buttons: Vec<ScreenButton>,
    screen_w: f32,
    screen_h: f32,
    // Options
    pub render_distance: u32,
    pub gui_scale: u32,
    pub auto_jump: bool,
    pub sensitivity: u32,
    // In-game look state
    pub player_yaw: f32,
    pub player_pitch: f32,
}

impl ScreenManager {
    pub fn new(screen_w: f32, screen_h: f32, storage: &StorageManager) -> Self {
        let mut mgr = Self {
            state: ScreenState::MainMenu,
            buttons: Vec::new(),
            screen_w: screen_w.max(1.0),
            screen_h: screen_h.max(1.0),
            render_distance: 8,
            gui_scale: 3,
            auto_jump: true,
            sensitivity: 100,
            player_yaw: 0.0,
            player_pitch: 0.0,
        };
        mgr.rebuild_layout(storage);
        mgr
    }

    pub fn current_state(&self) -> &ScreenState {
        &self.state
    }

    pub fn resize(&mut self, w: f32, h: f32, storage: &StorageManager) {
        self.screen_w = w.max(1.0);
        self.screen_h = h.max(1.0);
        self.rebuild_layout(storage);
    }

    pub fn pause(&mut self, storage: &StorageManager) {
        if let ScreenState::Playing { world_name } = &self.state {
            let name = world_name.clone();
            self.state = ScreenState::Paused { world_name: name };
            self.rebuild_layout(storage);
        }
    }

    pub fn resume(&mut self, storage: &StorageManager) {
        if let ScreenState::Paused { world_name } = &self.state {
            let name = world_name.clone();
            self.state = ScreenState::Playing { world_name: name };
            self.rebuild_layout(storage);
        }
    }

    /// Rebuilds button layout for the current screen state and resolution.
    pub fn rebuild_layout(&mut self, storage: &StorageManager) {
        self.buttons.clear();
        let cx = self.screen_w * 0.5;
        let _cy = self.screen_h * 0.5;

        match &self.state {
            ScreenState::MainMenu => {
                let btn_w = 440.0;
                let btn_h = 54.0;
                let spacing = 16.0;
                let start_y = (self.screen_h * 0.40).max(180.0);

                self.buttons.push(ScreenButton::new(
                    BTN_MAIN_SINGLEPLAYER,
                    "Un jugador",
                    cx - btn_w * 0.5,
                    start_y,
                    btn_w,
                    btn_h,
                ));
                self.buttons.push(
                    ScreenButton::new(
                        BTN_MAIN_MULTIPLAYER,
                        "Multijugador (Próximamente)",
                        cx - btn_w * 0.5,
                        start_y + (btn_h + spacing),
                        btn_w,
                        btn_h,
                    )
                    .with_enabled(false),
                );
                self.buttons.push(ScreenButton::new(
                    BTN_MAIN_OPTIONS,
                    "Opciones...",
                    cx - btn_w * 0.5,
                    start_y + 2.0 * (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_MAIN_QUIT,
                    "Salir del juego",
                    cx - btn_w * 0.5,
                    start_y + 3.0 * (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));
            }
            ScreenState::SelectWorld { selected } => {
                let worlds = storage.list_worlds();
                let list_y = 120.0;
                let item_h = 52.0;

                for (idx, world_name) in worlds.iter().enumerate().take(5) {
                    let is_sel = *selected == Some(idx);
                    let label = if is_sel {
                        format!("> {} (Seleccionado)", world_name)
                    } else {
                        world_name.clone()
                    };
                    self.buttons.push(ScreenButton::new(
                        BTN_WORLD_BASE + idx as u32,
                        label,
                        cx - 300.0,
                        list_y + idx as f32 * (item_h + 8.0),
                        600.0,
                        item_h,
                    ));
                }

                // Bottom buttons
                let has_selected = selected.is_some() || !worlds.is_empty();
                self.buttons.push(
                    ScreenButton::new(
                        BTN_SELECT_PLAY,
                        "Jugar al mundo seleccionado",
                        cx - 320.0,
                        self.screen_h - 130.0,
                        310.0,
                        52.0,
                    )
                    .with_enabled(has_selected),
                );
                self.buttons.push(ScreenButton::new(
                    BTN_SELECT_CREATE,
                    "Crear un nuevo mundo",
                    cx + 10.0,
                    self.screen_h - 130.0,
                    310.0,
                    52.0,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_SELECT_BACK,
                    "Volver al menú principal",
                    cx - 150.0,
                    self.screen_h - 68.0,
                    300.0,
                    48.0,
                ));
            }
            ScreenState::CreateWorld {
                name,
                creative,
                difficulty,
            } => {
                let btn_w = 480.0;
                let btn_h = 52.0;
                let spacing = 16.0;
                let start_y = 130.0;

                self.buttons.push(ScreenButton::new(
                    BTN_CREATE_NAME,
                    format!("Nombre: {}", name),
                    cx - btn_w * 0.5,
                    start_y,
                    btn_w,
                    btn_h,
                ));

                let mode_str = if *creative {
                    "Modo de juego: Creativo"
                } else {
                    "Modo de juego: Supervivencia"
                };
                self.buttons.push(ScreenButton::new(
                    BTN_CREATE_MODE,
                    mode_str,
                    cx - btn_w * 0.5,
                    start_y + (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));

                let diff_str = match difficulty {
                    0 => "Dificultad: Pacífico",
                    1 => "Dificultad: Fácil",
                    2 => "Dificultad: Normal",
                    _ => "Dificultad: Difícil",
                };
                self.buttons.push(ScreenButton::new(
                    BTN_CREATE_DIFF,
                    diff_str,
                    cx - btn_w * 0.5,
                    start_y + 2.0 * (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));

                // Bottom actions
                self.buttons.push(ScreenButton::new(
                    BTN_CREATE_CONFIRM,
                    "Crear nuevo mundo",
                    cx - 260.0,
                    self.screen_h - 110.0,
                    250.0,
                    54.0,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_CREATE_CANCEL,
                    "Cancelar",
                    cx + 10.0,
                    self.screen_h - 110.0,
                    250.0,
                    54.0,
                ));
            }
            ScreenState::Playing { .. } => {
                // Gameplay screen has no blocking menu buttons
            }
            ScreenState::Paused { .. } => {
                let btn_w = 460.0;
                let btn_h = 54.0;
                let spacing = 18.0;
                let start_y = (self.screen_h * 0.36).max(160.0);

                self.buttons.push(ScreenButton::new(
                    BTN_PAUSE_RESUME,
                    "Volver al juego",
                    cx - btn_w * 0.5,
                    start_y,
                    btn_w,
                    btn_h,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_PAUSE_OPTIONS,
                    "Opciones...",
                    cx - btn_w * 0.5,
                    start_y + (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_PAUSE_QUIT,
                    "Guardar y volver al menú principal",
                    cx - btn_w * 0.5,
                    start_y + 2.0 * (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));
            }
            ScreenState::Options { .. } => {
                let btn_w = 460.0;
                let btn_h = 52.0;
                let spacing = 14.0;
                let start_y = 110.0;

                self.buttons.push(ScreenButton::new(
                    BTN_OPT_RENDER_DIST,
                    format!("Distancia de renderizado: {} chunks", self.render_distance),
                    cx - btn_w * 0.5,
                    start_y,
                    btn_w,
                    btn_h,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_OPT_GUI_SCALE,
                    format!("Escala de interfaz: {}x", self.gui_scale),
                    cx - btn_w * 0.5,
                    start_y + (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_OPT_AUTO_JUMP,
                    format!("Salto automático: {}", if self.auto_jump { "SÍ" } else { "NO" }),
                    cx - btn_w * 0.5,
                    start_y + 2.0 * (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));
                self.buttons.push(ScreenButton::new(
                    BTN_OPT_SENSITIVITY,
                    format!("Sensibilidad de cámara: {}%", self.sensitivity),
                    cx - btn_w * 0.5,
                    start_y + 3.0 * (btn_h + spacing),
                    btn_w,
                    btn_h,
                ));

                self.buttons.push(ScreenButton::new(
                    BTN_OPT_DONE,
                    "Listo",
                    cx - 150.0,
                    self.screen_h - 90.0,
                    300.0,
                    52.0,
                ));
            }
        }
    }

    /// Handles touch input when in a menu screen.
    /// `action`: 0=DOWN, 1=UP, 2=MOVE, 3=CANCEL.
    pub fn handle_touch(&mut self, action: i32, x: f32, y: f32, storage: &mut StorageManager) {
        match action {
            0 | 5 => {
                // Touch Down
                for btn in &mut self.buttons {
                    if btn.is_inside(x, y) {
                        btn.pressed = true;
                    }
                }
            }
            2 => {
                // Touch Move
                for btn in &mut self.buttons {
                    btn.pressed = btn.is_inside(x, y);
                }
            }
            1 | 6 => {
                // Touch Up: trigger action if released on pressed button
                let mut triggered_id = None;
                for btn in &mut self.buttons {
                    if btn.pressed && btn.is_inside(x, y) {
                        triggered_id = Some(btn.id);
                    }
                    btn.pressed = false;
                }

                if let Some(id) = triggered_id {
                    self.trigger_button(id, storage);
                }
            }
            3 => {
                // Cancel
                for btn in &mut self.buttons {
                    btn.pressed = false;
                }
            }
            _ => {}
        }
    }

    fn trigger_button(&mut self, id: u32, storage: &mut StorageManager) {
        log::info!("ScreenManager: button triggered id={id}");

        match id {
            // Main Menu
            BTN_MAIN_SINGLEPLAYER => {
                let worlds = storage.list_worlds();
                let selected = if !worlds.is_empty() { Some(0) } else { None };
                self.state = ScreenState::SelectWorld { selected };
                self.rebuild_layout(storage);
            }
            BTN_MAIN_OPTIONS => {
                self.state = ScreenState::Options {
                    return_to: Box::new(ScreenState::MainMenu),
                };
                self.rebuild_layout(storage);
            }
            BTN_MAIN_QUIT => {
                log::info!("Player requested exit from Main Menu");
                // In Android, exiting simply pauses or signals exit
            }

            // Select World
            BTN_SELECT_PLAY => {
                let worlds = storage.list_worlds();
                let world_name = match &self.state {
                    ScreenState::SelectWorld { selected: Some(idx) } if *idx < worlds.len() => {
                        worlds[*idx].clone()
                    }
                    _ if !worlds.is_empty() => worlds[0].clone(),
                    _ => "Mundo 1".to_string(),
                };
                log::info!("Launching world: {world_name}");
                self.state = ScreenState::Playing { world_name };
                self.rebuild_layout(storage);
            }
            BTN_SELECT_CREATE => {
                self.state = ScreenState::CreateWorld {
                    name: "Nuevo Mundo".to_string(),
                    creative: false,
                    difficulty: 2, // Normal
                };
                self.rebuild_layout(storage);
            }
            BTN_SELECT_BACK => {
                self.state = ScreenState::MainMenu;
                self.rebuild_layout(storage);
            }

            // World slot selection
            id if id >= BTN_WORLD_BASE && id < BTN_WORLD_BASE + 20 => {
                let idx = (id - BTN_WORLD_BASE) as usize;
                self.state = ScreenState::SelectWorld {
                    selected: Some(idx),
                };
                self.rebuild_layout(storage);
            }

            // Create World
            BTN_CREATE_NAME => {
                if let ScreenState::CreateWorld { name, .. } = &mut self.state {
                    *name = match name.as_str() {
                        "Nuevo Mundo" => "Mi Supervivencia".to_string(),
                        "Mi Supervivencia" => "Aventura Horizon".to_string(),
                        "Aventura Horizon" => "Mundo Extremo".to_string(),
                        _ => "Nuevo Mundo".to_string(),
                    };
                }
                self.rebuild_layout(storage);
            }
            BTN_CREATE_MODE => {
                if let ScreenState::CreateWorld { creative, .. } = &mut self.state {
                    *creative = !*creative;
                }
                self.rebuild_layout(storage);
            }
            BTN_CREATE_DIFF => {
                if let ScreenState::CreateWorld { difficulty, .. } = &mut self.state {
                    *difficulty = (*difficulty + 1) % 4;
                }
                self.rebuild_layout(storage);
            }
            BTN_CREATE_CONFIRM => {
                let (name, _creative) = match &self.state {
                    ScreenState::CreateWorld { name, creative, .. } => (name.clone(), *creative),
                    _ => ("Nuevo Mundo".to_string(), false),
                };
                // Ensure directory created in saves/
                let world_dir = storage.saves_dir.join(&name);
                let _ = fs::create_dir_all(&world_dir);
                log::info!("Created world at {}", world_dir.display());

                self.state = ScreenState::Playing { world_name: name };
                self.rebuild_layout(storage);
            }
            BTN_CREATE_CANCEL => {
                let worlds = storage.list_worlds();
                let selected = if !worlds.is_empty() { Some(0) } else { None };
                self.state = ScreenState::SelectWorld { selected };
                self.rebuild_layout(storage);
            }

            // Pause
            BTN_PAUSE_RESUME => {
                self.resume(storage);
            }
            BTN_PAUSE_OPTIONS => {
                let world_name = match &self.state {
                    ScreenState::Paused { world_name } => world_name.clone(),
                    _ => "Mundo".to_string(),
                };
                self.state = ScreenState::Options {
                    return_to: Box::new(ScreenState::Paused { world_name }),
                };
                self.rebuild_layout(storage);
            }
            BTN_PAUSE_QUIT => {
                log::info!("Player saved and quit to Main Menu");
                self.state = ScreenState::MainMenu;
                self.rebuild_layout(storage);
            }

            // Options
            BTN_OPT_RENDER_DIST => {
                self.render_distance = match self.render_distance {
                    4 => 6,
                    6 => 8,
                    8 => 12,
                    _ => 4,
                };
                self.rebuild_layout(storage);
            }
            BTN_OPT_GUI_SCALE => {
                self.gui_scale = match self.gui_scale {
                    2 => 3,
                    3 => 4,
                    _ => 2,
                };
                self.rebuild_layout(storage);
            }
            BTN_OPT_AUTO_JUMP => {
                self.auto_jump = !self.auto_jump;
                self.rebuild_layout(storage);
            }
            BTN_OPT_SENSITIVITY => {
                self.sensitivity = match self.sensitivity {
                    50 => 75,
                    75 => 100,
                    100 => 125,
                    _ => 50,
                };
                self.rebuild_layout(storage);
            }
            BTN_OPT_DONE => {
                let ret = match &self.state {
                    ScreenState::Options { return_to } => *return_to.clone(),
                    _ => ScreenState::MainMenu,
                };
                self.state = ret;
                self.rebuild_layout(storage);
            }

            _ => {}
        }
    }

    /// Draws the current screen into the `ScreenRenderer` vertex batch.
    pub fn draw(&self, renderer: &mut ScreenRenderer, storage: &StorageManager, time_ticks: u64) {
        let cx = self.screen_w * 0.5;

        match &self.state {
            ScreenState::MainMenu => {
                // Minecraft tiled dirt background tinted dark
                renderer.draw_tiled_dirt(
                    0.0,
                    0.0,
                    self.screen_w,
                    self.screen_h,
                    [0.22, 0.22, 0.22, 1.0],
                );

                // Title banner
                let banner_y = (self.screen_h * 0.12).max(40.0);
                renderer.draw_minecraft_banner(cx, banner_y);

                // Buttons
                for btn in &self.buttons {
                    renderer.draw_minecraft_button(btn, 1.8);
                }

                // Watermark footer
                renderer.draw_text(
                    "Horizon v26.3 • Rust + wgpu (Vulkan) • Port Android",
                    16.0,
                    self.screen_h - 28.0,
                    1.4,
                    [0.70, 0.70, 0.70, 0.85],
                    true,
                );
            }

            ScreenState::SelectWorld { selected: _ } => {
                renderer.draw_tiled_dirt(
                    0.0,
                    0.0,
                    self.screen_w,
                    self.screen_h,
                    [0.22, 0.22, 0.22, 1.0],
                );

                // Header
                renderer.draw_centered_text(
                    "Seleccionar mundo",
                    cx,
                    36.0,
                    2.4,
                    [1.0, 1.0, 1.0, 1.0],
                    true,
                );

                let worlds = storage.list_worlds();
                if worlds.is_empty() {
                    renderer.draw_centered_text(
                        "No hay mundos guardados. ¡Crea uno nuevo!",
                        cx,
                        180.0,
                        1.8,
                        [0.85, 0.85, 0.85, 0.9],
                        true,
                    );
                }

                for btn in &self.buttons {
                    renderer.draw_minecraft_button(btn, 1.6);
                }
            }

            ScreenState::CreateWorld { .. } => {
                renderer.draw_tiled_dirt(
                    0.0,
                    0.0,
                    self.screen_w,
                    self.screen_h,
                    [0.22, 0.22, 0.22, 1.0],
                );

                // Header
                renderer.draw_centered_text(
                    "Crear un nuevo mundo",
                    cx,
                    36.0,
                    2.4,
                    [1.0, 1.0, 1.0, 1.0],
                    true,
                );

                for btn in &self.buttons {
                    renderer.draw_minecraft_button(btn, 1.6);
                }
            }

            ScreenState::Playing { world_name } => {
                // In-game 3D sky/ground view
                renderer.draw_game_world(
                    self.screen_w,
                    self.screen_h,
                    self.player_yaw,
                    self.player_pitch,
                    time_ticks,
                );

                // Subtle in-game info text (top right)
                let info = format!("Horizon 26.3 | {}", world_name);
                let w = renderer.measure_text(&info, 1.4);
                renderer.draw_text(
                    &info,
                    self.screen_w - w - 16.0,
                    16.0,
                    1.4,
                    [1.0, 1.0, 1.0, 0.75],
                    true,
                );
            }

            ScreenState::Paused { world_name } => {
                // In-game world in background
                renderer.draw_game_world(
                    self.screen_w,
                    self.screen_h,
                    self.player_yaw,
                    self.player_pitch,
                    time_ticks,
                );

                // Translucent dark dimming overlay
                renderer.fill_rect(0.0, 0.0, self.screen_w, self.screen_h, [0.0, 0.0, 0.0, 0.70]);

                // Header
                let title_y = (self.screen_h * 0.16).max(50.0);
                renderer.draw_centered_text(
                    "Juego pausado",
                    cx,
                    title_y,
                    2.6,
                    [1.0, 1.0, 1.0, 1.0],
                    true,
                );

                let sub = format!("Mundo: {}", world_name);
                renderer.draw_centered_text(
                    &sub,
                    cx,
                    title_y + 36.0,
                    1.5,
                    [0.80, 0.80, 0.80, 0.85],
                    true,
                );

                // Buttons
                for btn in &self.buttons {
                    renderer.draw_minecraft_button(btn, 1.7);
                }
            }

            ScreenState::Options { .. } => {
                renderer.draw_tiled_dirt(
                    0.0,
                    0.0,
                    self.screen_w,
                    self.screen_h,
                    [0.22, 0.22, 0.22, 1.0],
                );

                // Header
                renderer.draw_centered_text(
                    "Opciones de Horizon",
                    cx,
                    36.0,
                    2.4,
                    [1.0, 1.0, 1.0, 1.0],
                    true,
                );

                for btn in &self.buttons {
                    renderer.draw_minecraft_button(btn, 1.6);
                }
            }
        }
    }
}
