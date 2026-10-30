//! Spatial hit-testing areas porting PocketMC's `TouchAreaModel.h`.

/// Rectangle area for button hit-testing.
#[derive(Debug, Clone, Copy)]
pub struct RectangleArea {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl RectangleArea {
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    #[inline]
    pub fn is_inside(&self, x: f32, y: f32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }

    #[inline]
    pub fn center_x(&self) -> f32 {
        self.x0 + (self.x1 - self.x0) * 0.5
    }

    #[inline]
    pub fn center_y(&self) -> f32 {
        self.y0 + (self.y1 - self.y0) * 0.5
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }
}

/// A registered touch area with an associated Area ID.
#[derive(Debug, Clone, Copy)]
pub struct AreaEntry {
    pub area_id: i32,
    pub bounds: RectangleArea,
}

/// Collection of touch areas porting PocketMC's `TouchAreaModel`.
#[derive(Debug, Default, Clone)]
pub struct TouchAreaModel {
    areas: Vec<AreaEntry>,
}

impl TouchAreaModel {
    pub fn new() -> Self {
        Self { areas: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.areas.clear();
    }

    pub fn add_area(&mut self, area_id: i32, bounds: RectangleArea) {
        self.areas.push(AreaEntry { area_id, bounds });
    }

    /// Finds the area containing the coordinates (x, y).
    /// Returns `Some(area_id)` if inside a registered area, or `None` if in the camera/gesture area.
    pub fn get_area_id(&self, x: f32, y: f32) -> Option<i32> {
        for entry in &self.areas {
            if entry.bounds.is_inside(x, y) {
                return Some(entry.area_id);
            }
        }
        None
    }

    pub fn areas(&self) -> &[AreaEntry] {
        &self.areas
    }

    pub fn get_area_rect(&self, area_id: i32) -> Option<&RectangleArea> {
        self.areas.iter().find(|e| e.area_id == area_id).map(|e| &e.bounds)
    }
}

