//! Android storage management for Horizon.
//!
//! Manages game directories (`saves/`, `logs/`, `resourcepacks/`), default
//! mobile-optimized `options.txt`, and resource verification.

use std::fs;
use std::path::PathBuf;

/// Default mobile-tailored Minecraft/Horizon settings.
/// Uses 8 chunks render distance (optimal for mobile GPUs like Mali-G57),
/// GUI scale 3 for high-density phone displays, and touchscreen mode enabled.
const DEFAULT_MOBILE_OPTIONS: &str = "\
version:3465
renderDistance:8
simulationDistance:6
guiScale:3
autoJump:true
fov:0.0
maxFps:60
fullscreen:true
touchscreen:true
graphicsMode:\"fast\"
ao:\"min\"
particles:\"decreased\"
";

/// Storage manager ensuring standard Horizon directory layouts on Android.
#[derive(Debug, Clone)]
pub struct StorageManager {
    pub home_dir: PathBuf,
    pub resources_dir: PathBuf,
    pub saves_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub packs_dir: PathBuf,
    pub options_path: PathBuf,
}

impl StorageManager {
    /// Initialize storage layout under `home_dir` and `resources_dir`.
    pub fn new(home_dir: PathBuf, resources_dir: PathBuf) -> Self {
        let saves_dir = home_dir.join("saves");
        let logs_dir = home_dir.join("logs");
        let packs_dir = home_dir.join("resourcepacks");
        let options_path = home_dir.join("options.txt");

        let manager = Self {
            home_dir,
            resources_dir,
            saves_dir,
            logs_dir,
            packs_dir,
            options_path,
        };

        manager.ensure_directories();
        manager.ensure_default_options();
        manager
    }

    /// Creates all required game subdirectories if they do not exist.
    fn ensure_directories(&self) {
        for dir in &[&self.home_dir, &self.saves_dir, &self.logs_dir, &self.packs_dir] {
            if !dir.exists() {
                if let Err(e) = fs::create_dir_all(dir) {
                    log::warn!("Storage: failed to create {}: {e}", dir.display());
                } else {
                    log::info!("Storage: created directory {}", dir.display());
                }
            }
        }
    }

    /// Creates a mobile-optimized `options.txt` if none exists yet.
    fn ensure_default_options(&self) {
        if !self.options_path.exists() {
            if let Err(e) = fs::write(&self.options_path, DEFAULT_MOBILE_OPTIONS) {
                log::warn!("Storage: failed to write default options.txt: {e}");
            } else {
                log::info!(
                    "Storage: created default mobile options.txt at {}",
                    self.options_path.display()
                );
            }
        }
    }

    /// Lists existing world save folders in `saves/`.
    pub fn list_worlds(&self) -> Vec<String> {
        let mut worlds = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.saves_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Ok(name) = entry.file_name().into_string() {
                            worlds.push(name);
                        }
                    }
                }
            }
        }
        worlds
    }

    /// Checks whether resources (assets, datagen, jars) exist in the resources dir.
    pub fn check_resources(&self) -> bool {
        let manifest = self.resources_dir.join("horizon-resources.json");
        let exists = manifest.exists();
        log::info!(
            "Storage: checking resources at {}: exists={exists}",
            manifest.display()
        );
        exists
    }
}
