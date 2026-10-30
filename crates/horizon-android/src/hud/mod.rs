//! HUD renderer for Horizon Android.
//!
//! Renders the touch control overlay (D-pad buttons, jump, top bar, crosshair,
//! hotbar) using a simple wgpu 2D pipeline with alpha-blended textured quads.
//!
//! # Architecture
//!
//! ```text
//! HudRenderer
//!   ├── wgpu pipeline (hud.wgsl, BlendState::ALPHA_BLENDING)
//!   ├── static index buffer + persistent vertex buffer (up to 64 quads)
//!   ├── TextureAtlas — one wgpu::Texture per button image
//!   │     └── loaded from Android assets via AssetManager (ndk crate)
//!   └── draw(queue, view, encoder, input)
//!         ├── collect quads for all active buttons
//!         ├── upload all vertices in a single queue.write_buffer
//!         └── single render pass with multiple draw_indexed calls
//! ```
//!
//! All pixel coordinates come from [`TouchscreenInput::rebuild_layout`] so the
//! button positions and hit areas are always in sync with the input system.

use std::collections::HashMap;

use wgpu::util::DeviceExt;

use crate::touch::area::RectangleArea;
use crate::touch::input::{
    TouchscreenInput, AREA_CHAT, AREA_DPAD_C, AREA_DPAD_E, AREA_DPAD_N, AREA_DPAD_S, AREA_DPAD_W,
    AREA_FLY_DOWN, AREA_FLIGHT_TOGGLE, AREA_PAUSE, AREA_SNEAK, AREA_THIRD,
};

// ── Vertex format ──────────────────────────────────────────────────────────────

/// One vertex of a 2D textured quad.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2], // NDC [-1, 1]
    pub uv: [f32; 2],
    pub alpha: f32,
}

impl Vertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2,  // position
            1 => Float32x2,  // uv
            2 => Float32,    // alpha
        ],
    };
}

// ── HUD sprite keys ────────────────────────────────────────────────────────────

/// Identifies which image to use for a button (normal vs pressed variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpriteKey {
    MoveUp,
    MoveUpPress,
    MoveDown,
    MoveDownPress,
    MoveLeft,
    MoveLeftPress,
    MoveRight,
    MoveRightPress,
    MoveUpLeft,
    MoveUpLeftPress,
    MoveUpRight,
    MoveUpRightPress,
    Shift,
    ShiftPress,
    Jump,
    JumpPress,
    FlyUp,
    FlyUpPress,
    FlyDown,
    FlyDownPress,
    Flight,
    FlightPress,
    Pause,
    PausePress,
    Chat,
    ChatPress,
    F5,
    F5Press,
    Crosshair,
    HotbarAndroid,
}

impl SpriteKey {
    fn asset_name(self) -> &'static str {
        match self {
            Self::MoveUp => "touchgui/move_up_button.png",
            Self::MoveUpPress => "touchgui/move_up_button_press.png",
            Self::MoveDown => "touchgui/move_down_button.png",
            Self::MoveDownPress => "touchgui/move_down_button_press.png",
            Self::MoveLeft => "touchgui/move_left_button.png",
            Self::MoveLeftPress => "touchgui/move_left_button_press.png",
            Self::MoveRight => "touchgui/move_right_button.png",
            Self::MoveRightPress => "touchgui/move_right_button_press.png",
            Self::MoveUpLeft => "touchgui/move_up_left_button.png",
            Self::MoveUpLeftPress => "touchgui/move_up_left_press_button.png",
            Self::MoveUpRight => "touchgui/move_up_right_button.png",
            Self::MoveUpRightPress => "touchgui/move_up_right_press_button.png",
            Self::Shift => "touchgui/shift_button.png",
            Self::ShiftPress => "touchgui/shift_press_button.png",
            Self::Jump => "touchgui/jump_button.png",
            Self::JumpPress => "touchgui/jump_press_button.png",
            Self::FlyUp => "touchgui/fly_up_button.png",
            Self::FlyUpPress => "touchgui/fly_up_button_press.png",
            Self::FlyDown => "touchgui/fly_down_button.png",
            Self::FlyDownPress => "touchgui/fly_down_button_press.png",
            Self::Flight => "touchgui/flight_button.png",
            Self::FlightPress => "touchgui/flight_press_button.png",
            Self::Pause => "touchgui/pause_button.png",
            Self::PausePress => "touchgui/pause_button_press.png",
            Self::Chat => "touchgui/chat_button.png",
            Self::ChatPress => "touchgui/chat_button_press.png",
            Self::F5 => "touchgui/f5_button.png",
            Self::F5Press => "touchgui/f5_button_press.png",
            Self::Crosshair => "touchgui/crosshair.png",
            Self::HotbarAndroid => "touchgui/hotbar-android.png",
        }
    }
}

// ── GPU texture handle ─────────────────────────────────────────────────────────

struct GpuTexture {
    bind_group: wgpu::BindGroup,
}

// ── HudRenderer ────────────────────────────────────────────────────────────────

const MAX_QUADS: usize = 64;
const VERTICES_PER_QUAD: usize = 4;
const MAX_VERTICES: usize = MAX_QUADS * VERTICES_PER_QUAD;

/// wgpu-based HUD renderer for touch controls.
pub struct HudRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    textures: HashMap<SpriteKey, GpuTexture>,
    /// Asset manager pointer for loading files from APK assets.
    /// Only used on Android; remains null on host builds.
    asset_manager: *mut ndk_sys::AAssetManager,
    screen_w: f32,
    screen_h: f32,
}

// SAFETY: AAssetManager pointer is only used from the game thread.
unsafe impl Send for HudRenderer {}
unsafe impl Sync for HudRenderer {}

impl HudRenderer {
    /// Create the HUD renderer.
    ///
    /// `asset_manager` should be the `AAssetManager*` obtained from Java.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        screen_w: f32,
        screen_h: f32,
        asset_manager: *mut ndk_sys::AAssetManager,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HUD Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HUD Texture BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HUD Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HUD Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(Vertex::LAYOUT)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Pre-allocate dynamic vertex buffer
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD Dynamic VB"),
            size: (MAX_VERTICES * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Static index buffer for quad rendering (CCW)
        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HUD Static IB"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut renderer = Self {
            pipeline,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
            textures: HashMap::new(),
            asset_manager,
            screen_w,
            screen_h,
        };

        // Pre-load all HUD sprites
        let all_sprites = [
            SpriteKey::MoveUp,
            SpriteKey::MoveUpPress,
            SpriteKey::MoveDown,
            SpriteKey::MoveDownPress,
            SpriteKey::MoveLeft,
            SpriteKey::MoveLeftPress,
            SpriteKey::MoveRight,
            SpriteKey::MoveRightPress,
            SpriteKey::MoveUpLeft,
            SpriteKey::MoveUpLeftPress,
            SpriteKey::MoveUpRight,
            SpriteKey::MoveUpRightPress,
            SpriteKey::Shift,
            SpriteKey::ShiftPress,
            SpriteKey::Jump,
            SpriteKey::JumpPress,
            SpriteKey::FlyUp,
            SpriteKey::FlyUpPress,
            SpriteKey::FlyDown,
            SpriteKey::FlyDownPress,
            SpriteKey::Flight,
            SpriteKey::FlightPress,
            SpriteKey::Pause,
            SpriteKey::PausePress,
            SpriteKey::Chat,
            SpriteKey::ChatPress,
            SpriteKey::F5,
            SpriteKey::F5Press,
            SpriteKey::Crosshair,
            SpriteKey::HotbarAndroid,
        ];

        for key in all_sprites {
            match renderer.load_texture(device, queue, key) {
                Ok(gpu_tex) => {
                    renderer.textures.insert(key, gpu_tex);
                }
                Err(e) => {
                    log::warn!("HUD: failed to load sprite {:?}: {}", key, e);
                }
            }
        }

        log::info!(
            "HudRenderer: loaded {}/{} sprites",
            renderer.textures.len(),
            all_sprites.len()
        );
        renderer
    }

    /// Resize the viewport (called from engine when surface changes).
    pub fn resize(&mut self, w: f32, h: f32) {
        self.screen_w = w;
        self.screen_h = h;
    }

    /// Load a PNG texture from Android assets or disk.
    fn load_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: SpriteKey,
    ) -> Result<GpuTexture, String> {
        let png_data = self.read_asset(key.asset_name())?;
        let img = image::load_from_memory(&png_data)
            .map_err(|e| format!("image decode error: {e}"))?
            .into_rgba8();

        let (w, h) = img.dimensions();
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(key.asset_name()),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            texture.as_image_copy(),
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(key.asset_name()),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Ok(GpuTexture { bind_group })
    }

    /// Read a file from Android assets (on Android) or from cwd.
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, String> {
        #[cfg(target_os = "android")]
        {
            if self.asset_manager.is_null() {
                return Err("AssetManager not set".into());
            }
            unsafe { android_read_asset(self.asset_manager, path) }
        }

        #[cfg(not(target_os = "android"))]
        {
            std::fs::read(path).map_err(|e| format!("fs read {path}: {e}"))
        }
    }

    /// Draw the HUD overlay on top of whatever was rendered before.
    ///
    /// Single render pass: all vertices written at once, batch-drawn by quad.
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        input: &TouchscreenInput,
    ) {
        let sw = self.screen_w;
        let sh = self.screen_h;

        // Button definitions: (area_id, normal, pressed, is_pressed)
        let buttons: &[(i32, SpriteKey, SpriteKey, bool)] = &[
            (AREA_DPAD_N, SpriteKey::MoveUp, SpriteKey::MoveUpPress, input.forward),
            (AREA_DPAD_S, SpriteKey::MoveDown, SpriteKey::MoveDownPress, input.backward),
            (AREA_DPAD_W, SpriteKey::MoveLeft, SpriteKey::MoveLeftPress, input.left),
            (AREA_DPAD_E, SpriteKey::MoveRight, SpriteKey::MoveRightPress, input.right),
            (AREA_SNEAK, SpriteKey::Shift, SpriteKey::ShiftPress, input.sneak),
            (AREA_DPAD_C, SpriteKey::Jump, SpriteKey::JumpPress, input.jump),
            (AREA_FLY_DOWN, SpriteKey::FlyDown, SpriteKey::FlyDownPress, input.fly_down),
            (AREA_FLIGHT_TOGGLE, SpriteKey::Flight, SpriteKey::FlightPress, false),
            (AREA_PAUSE, SpriteKey::Pause, SpriteKey::PausePress, false),
            (AREA_CHAT, SpriteKey::Chat, SpriteKey::ChatPress, false),
            (AREA_THIRD, SpriteKey::F5, SpriteKey::F5Press, false),
        ];

        let mut all_vertices: Vec<Vertex> = Vec::with_capacity(32 * VERTICES_PER_QUAD);
        let mut draw_commands: Vec<(&wgpu::BindGroup, u32)> = Vec::with_capacity(32);

        // 1. D-pad & action buttons
        for &(area_id, normal_key, press_key, pressed) in buttons {
            let Some(rect) = input.model.get_area_rect(area_id) else {
                continue;
            };
            let sprite_key = if pressed { press_key } else { normal_key };
            let Some(gpu_tex) = self.textures.get(&sprite_key) else {
                continue;
            };

            let quad_idx = draw_commands.len() as u32;
            if (quad_idx as usize) >= MAX_QUADS {
                break;
            }

            let alpha = if pressed { 1.0_f32 } else { 0.85_f32 };
            let verts = quad_vertices(rect, sw, sh, alpha);
            all_vertices.extend_from_slice(&verts);
            draw_commands.push((&gpu_tex.bind_group, quad_idx));
        }

        // 2. Crosshair at screen center
        let crosshair_size = (sw.min(sh) * 0.04).clamp(24.0, 48.0);
        let cx = sw / 2.0;
        let cy = sh / 2.0;
        let crosshair_rect = RectangleArea::new(
            cx - crosshair_size / 2.0,
            cy - crosshair_size / 2.0,
            cx + crosshair_size / 2.0,
            cy + crosshair_size / 2.0,
        );
        if let Some(gpu_tex) = self.textures.get(&SpriteKey::Crosshair) {
            let quad_idx = draw_commands.len() as u32;
            if (quad_idx as usize) < MAX_QUADS {
                let verts = quad_vertices(&crosshair_rect, sw, sh, 0.9);
                all_vertices.extend_from_slice(&verts);
                draw_commands.push((&gpu_tex.bind_group, quad_idx));
            }
        }

        // 3. Hotbar centered at bottom
        let hotbar_h = (sh * 0.08).clamp(44.0, 72.0);
        let hotbar_aspect = 182.0_f32 / 22.0;
        let hotbar_w = hotbar_h * hotbar_aspect;
        let hotbar_rect = RectangleArea::new(
            (sw - hotbar_w) / 2.0,
            sh - hotbar_h - 12.0,
            (sw + hotbar_w) / 2.0,
            sh - 12.0,
        );
        if let Some(gpu_tex) = self.textures.get(&SpriteKey::HotbarAndroid) {
            let quad_idx = draw_commands.len() as u32;
            if (quad_idx as usize) < MAX_QUADS {
                let verts = quad_vertices(&hotbar_rect, sw, sh, 0.95);
                all_vertices.extend_from_slice(&verts);
                draw_commands.push((&gpu_tex.bind_group, quad_idx));
            }
        }

        if draw_commands.is_empty() {
            return;
        }

        // Upload all quad vertices in a single transfer
        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&all_vertices),
        );

        // Single render pass on top of existing framebuffer contents
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve background/clear
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            for (bind_group, quad_idx) in draw_commands {
                pass.set_bind_group(0, bind_group, &[]);
                let base_vertex = (quad_idx * VERTICES_PER_QUAD as u32) as i32;
                pass.draw_indexed(0..6, base_vertex, 0..1);
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Build 4 vertices for a textured quad at pixel-space `rect`, normalised to NDC.
fn quad_vertices(rect: &RectangleArea, sw: f32, sh: f32, alpha: f32) -> [Vertex; 4] {
    // Pixel → NDC: x: [0,sw] → [-1,1], y: [0,sh] → [1,-1]  (screen top = NDC +1)
    let ndc_x = |px: f32| px / sw * 2.0 - 1.0;
    let ndc_y = |py: f32| 1.0 - py / sh * 2.0;

    let x0 = ndc_x(rect.x0);
    let y0 = ndc_y(rect.y0); // top-left
    let x1 = ndc_x(rect.x1);
    let y1 = ndc_y(rect.y1); // bottom-right

    [
        Vertex { position: [x0, y0], uv: [0.0, 0.0], alpha }, // top-left
        Vertex { position: [x0, y1], uv: [0.0, 1.0], alpha }, // bottom-left
        Vertex { position: [x1, y1], uv: [1.0, 1.0], alpha }, // bottom-right
        Vertex { position: [x1, y0], uv: [1.0, 0.0], alpha }, // top-right
    ]
}

/// Read a file from the APK's `assets/` folder using the NDK AAssetManager.
#[cfg(target_os = "android")]
unsafe fn android_read_asset(
    mgr: *mut ndk_sys::AAssetManager,
    path: &str,
) -> Result<Vec<u8>, String> {
    use std::ffi::CString;
    use ndk_sys::{
        AAsset_close, AAsset_getLength, AAsset_read, AAssetManager_open, AASSET_MODE_BUFFER,
    };

    let c_path = CString::new(path).map_err(|e| format!("CString: {e}"))?;
    let asset = unsafe { AAssetManager_open(mgr, c_path.as_ptr(), AASSET_MODE_BUFFER as i32) };
    if asset.is_null() {
        return Err(format!("asset not found in APK: {path}"));
    }

    let len = unsafe { AAsset_getLength(asset) } as usize;
    let mut buf = vec![0u8; len];
    let read = unsafe { AAsset_read(asset, buf.as_mut_ptr() as *mut _, len) };
    unsafe { AAsset_close(asset) };

    if read < 0 {
        return Err(format!("AAsset_read failed for {path}"));
    }
    buf.truncate(read as usize);
    Ok(buf)
}
