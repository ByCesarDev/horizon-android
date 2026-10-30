//! Screen rendering pipeline in wgpu for Minecraft-style UI and menus.

use wgpu::util::DeviceExt;

use super::font::{get_glyph, GLYPH_HEIGHT, GLYPH_WIDTH};
use super::state::ScreenButton;

/// Vertex format for screen quads.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScreenVertex {
    pub position: [f32; 2], // NDC [-1..1, -1..1]
    pub uv: [f32; 2],       // Atlas UV [0..1]
    pub color: [f32; 4],    // RGBA tint / multiplier
}

impl ScreenVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ScreenVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2, // position
            1 => Float32x2, // uv
            2 => Float32x4, // color
        ],
    };
}

const ATLAS_SIZE: u32 = 256;
const MAX_SCREEN_QUADS: usize = 4096;
const VERTICES_PER_QUAD: usize = 4;
const MAX_SCREEN_VERTICES: usize = MAX_SCREEN_QUADS * VERTICES_PER_QUAD;

/// Minecraft Dirt tile 16x16 pattern in RGBA8.
fn generate_dirt_tile() -> [u8; 16 * 16 * 4] {
    let mut data = [0u8; 16 * 16 * 4];
    // Base Minecraft dirt colors: #866043, #6c4d35, #976e4d, #573d28
    let c_base = [134, 96, 67, 255];
    let c_dark = [108, 77, 53, 255];
    let c_light = [151, 110, 77, 255];
    let c_shadow = [87, 61, 40, 255];

    let pattern: [u8; 16 * 16] = [
        0, 1, 0, 0, 2, 0, 1, 0, 0, 0, 1, 0, 0, 2, 0, 0,
        0, 0, 3, 0, 0, 1, 0, 0, 2, 0, 0, 1, 0, 0, 1, 0,
        1, 0, 0, 1, 0, 0, 0, 2, 0, 1, 0, 0, 3, 0, 0, 1,
        0, 2, 0, 0, 1, 0, 1, 0, 0, 0, 2, 0, 0, 1, 0, 0,
        0, 0, 1, 0, 0, 3, 0, 0, 1, 0, 0, 0, 1, 0, 2, 0,
        2, 0, 0, 2, 0, 0, 1, 0, 0, 3, 0, 1, 0, 0, 0, 1,
        0, 1, 0, 0, 1, 0, 0, 2, 0, 0, 1, 0, 0, 2, 0, 0,
        0, 0, 2, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0,
        1, 0, 0, 1, 0, 0, 3, 0, 0, 2, 0, 1, 0, 0, 0, 2,
        0, 2, 0, 0, 2, 0, 0, 1, 0, 0, 1, 0, 0, 3, 0, 0,
        0, 0, 1, 0, 0, 1, 0, 0, 2, 0, 0, 0, 1, 0, 1, 0,
        3, 0, 0, 3, 0, 0, 2, 0, 0, 1, 0, 2, 0, 0, 0, 1,
        0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 3, 0, 0, 1, 0, 0,
        0, 0, 2, 0, 0, 2, 0, 0, 1, 0, 0, 1, 0, 0, 2, 0,
        1, 0, 0, 1, 0, 0, 1, 0, 0, 2, 0, 0, 1, 0, 0, 1,
        0, 3, 0, 0, 0, 1, 0, 3, 0, 0, 1, 0, 0, 0, 3, 0,
    ];

    for y in 0..16 {
        for x in 0..16 {
            let idx = (y * 16 + x) * 4;
            let val = pattern[y * 16 + x];
            let col = match val {
                1 => c_dark,
                2 => c_light,
                3 => c_shadow,
                _ => c_base,
            };
            data[idx..idx + 4].copy_from_slice(&col);
        }
    }
    data
}

/// Builds the 256x256 UI texture atlas.
fn build_atlas_image() -> Vec<u8> {
    let mut pixels = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];

    // 1. Region (0..16, 0..16): Solid White block for tinted solid primitives
    for y in 0..16 {
        for x in 0..16 {
            let idx = ((y * ATLAS_SIZE + x) * 4) as usize;
            pixels[idx..idx + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    // 2. Region (16..32, 0..16): Minecraft Dirt Tile
    let dirt = generate_dirt_tile();
    for y in 0..16usize {
        for x in 0..16usize {
            let src_idx = (y * 16 + x) * 4;
            let dst_idx = ((y * ATLAS_SIZE as usize + (16 + x)) * 4) as usize;
            pixels[dst_idx..dst_idx + 4].copy_from_slice(&dirt[src_idx..src_idx + 4]);
        }
    }

    // 3. Region (0..256, 32..256): Font glyphs
    // 16 columns x 14 rows = 224 glyphs of 16x16 cells (8x8 glyph scaled 2x)
    for c_code in 32u32..256 {
        let char_opt = char::from_u32(c_code);
        let ch = match char_opt {
            Some(c) => c,
            None => '?',
        };
        let glyph = get_glyph(ch);
        let index = c_code - 32;
        let col = index % 16;
        let row = index / 16;
        let base_x = col * 16;
        let base_y = 32 + row * 16;

        if base_y + 16 > ATLAS_SIZE {
            break;
        }

        for gy in 0..GLYPH_HEIGHT {
            let row_byte = glyph[gy];
            for gx in 0..GLYPH_WIDTH {
                let bit_set = (row_byte & (0x80 >> gx)) != 0;
                let color = if bit_set {
                    [255, 255, 255, 255]
                } else {
                    [255, 255, 255, 0]
                };

                // Draw 2x2 pixels for crisp 8x8 -> 16x16 cell
                for dy in 0..2 {
                    for dx in 0..2 {
                        let px = base_x + gx as u32 * 2 + dx;
                        let py = base_y + gy as u32 * 2 + dy;
                        let idx = ((py * ATLAS_SIZE + px) * 4) as usize;
                        pixels[idx..idx + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }

    pixels
}

pub struct ScreenRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    pub vertices: Vec<ScreenVertex>,
    pub screen_w: f32,
    pub screen_h: f32,
}

impl ScreenRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        screen_w: f32,
        screen_h: f32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Screen WGSL Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("screen.wgsl").into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Screen Texture BGL"),
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
            label: Some("Screen Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Screen Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ScreenVertex::LAYOUT)],
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

        // Create Atlas texture
        let atlas_pixels = build_atlas_image();
        let texture_size = wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Screen Atlas Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            texture.as_image_copy(),
            &atlas_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * ATLAS_SIZE),
                rows_per_image: Some(ATLAS_SIZE),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Screen Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Screen Atlas BindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screen Dynamic VB"),
            size: (MAX_SCREEN_VERTICES * std::mem::size_of::<ScreenVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Pre-allocate index buffer for MAX_SCREEN_QUADS quads (6 indices each)
        let mut indices = Vec::with_capacity(MAX_SCREEN_QUADS * 6);
        for i in 0..MAX_SCREEN_QUADS as u16 {
            let base = i * 4;
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
        }

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Screen IB"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            bind_group,
            vertex_buffer,
            index_buffer,
            vertices: Vec::with_capacity(1024),
            screen_w,
            screen_h,
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        self.screen_w = w.max(1.0);
        self.screen_h = h.max(1.0);
    }

    pub fn clear_batch(&mut self) {
        self.vertices.clear();
    }

    /// Converts screen pixel coordinates (x, y) into NDC [-1..1, 1..-1].
    #[inline]
    fn to_ndc(&self, px: f32, py: f32) -> [f32; 2] {
        let ndc_x = (px / self.screen_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (py / self.screen_h) * 2.0;
        [ndc_x, ndc_y]
    }

    /// Appends a quad to the vertex batch.
    pub fn push_quad(&mut self, rect: [f32; 4], uv: [f32; 4], color: [f32; 4]) {
        if self.vertices.len() + 4 > MAX_SCREEN_VERTICES {
            return;
        }
        let [x0, y0, x1, y1] = rect;
        let [u0, v0, u1, v1] = uv;

        let ndc0 = self.to_ndc(x0, y0);
        let ndc1 = self.to_ndc(x1, y1);

        self.vertices.push(ScreenVertex {
            position: [ndc0[0], ndc0[1]],
            uv: [u0, v0],
            color,
        });
        self.vertices.push(ScreenVertex {
            position: [ndc0[0], ndc1[1]],
            uv: [u0, v1],
            color,
        });
        self.vertices.push(ScreenVertex {
            position: [ndc1[0], ndc1[1]],
            uv: [u1, v1],
            color,
        });
        self.vertices.push(ScreenVertex {
            position: [ndc1[0], ndc0[1]],
            uv: [u1, v0],
            color,
        });
    }

    /// Fills a solid rectangle with RGBA color.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        // Samples the 1x1 white texel at (8, 8) in the atlas
        let white_uv = [8.0 / 256.0, 8.0 / 256.0, 8.0 / 256.0, 8.0 / 256.0];
        self.push_quad([x, y, x + w, y + h], white_uv, color);
    }

    /// Draws repeating Minecraft Dirt tile across the specified area.
    pub fn draw_tiled_dirt(&mut self, x: f32, y: f32, w: f32, h: f32, tint: [f32; 4]) {
        let tile_size = 64.0; // Scaled dirt tile size in screen pixels
        let mut cy = y;
        let dirt_u0 = 16.0 / 256.0;
        let dirt_v0 = 0.0 / 256.0;
        let dirt_u1 = 32.0 / 256.0;
        let dirt_v1 = 16.0 / 256.0;

        while cy < y + h {
            let mut cx = x;
            let th = (y + h - cy).min(tile_size);
            while cx < x + w {
                let tw = (x + w - cx).min(tile_size);
                let u_frac = tw / tile_size;
                let v_frac = th / tile_size;
                let u1 = dirt_u0 + (dirt_u1 - dirt_u0) * u_frac;
                let v1 = dirt_v0 + (dirt_v1 - dirt_v0) * v_frac;

                self.push_quad(
                    [cx, cy, cx + tw, cy + th],
                    [dirt_u0, dirt_v0, u1, v1],
                    tint,
                );
                cx += tile_size;
            }
            cy += tile_size;
        }
    }

    /// Draws a single character glyph.
    pub fn draw_char(&mut self, c: char, x: f32, y: f32, scale: f32, color: [f32; 4]) {
        let c_code = c as u32;
        if c_code < 32 || c_code > 255 {
            return;
        }
        let index = c_code - 32;
        let col = index % 16;
        let row = index / 16;
        let base_x = col * 16;
        let base_y = 32 + row * 16;

        let u0 = base_x as f32 / 256.0;
        let v0 = base_y as f32 / 256.0;
        let u1 = (base_x + 16) as f32 / 256.0;
        let v1 = (base_y + 16) as f32 / 256.0;

        let gw = 8.0 * scale;
        let gh = 8.0 * scale;

        self.push_quad([x, y, x + gw, y + gh], [u0, v0, u1, v1], color);
    }

    /// Draws text with optional drop shadow.
    pub fn draw_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        scale: f32,
        color: [f32; 4],
        has_shadow: bool,
    ) {
        let step = 8.0 * scale;
        let shadow_offset = 2.0 * (scale * 0.5).max(1.0);

        if has_shadow {
            let shadow_color = [color[0] * 0.25, color[1] * 0.25, color[2] * 0.25, color[3]];
            let mut cx = x + shadow_offset;
            for c in text.chars() {
                self.draw_char(c, cx, y + shadow_offset, scale, shadow_color);
                cx += step;
            }
        }

        let mut cx = x;
        for c in text.chars() {
            self.draw_char(c, cx, y, scale, color);
            cx += step;
        }
    }

    /// Measures text width in pixels.
    pub fn measure_text(&self, text: &str, scale: f32) -> f32 {
        text.chars().count() as f32 * 8.0 * scale
    }

    /// Draws text centered at `cx`.
    pub fn draw_centered_text(
        &mut self,
        text: &str,
        cx: f32,
        y: f32,
        scale: f32,
        color: [f32; 4],
        has_shadow: bool,
    ) {
        let w = self.measure_text(text, scale);
        self.draw_text(text, cx - w * 0.5, y, scale, color, has_shadow);
    }

    /// Draws an authentic Minecraft button with bevel borders and centered text.
    pub fn draw_minecraft_button(&mut self, btn: &ScreenButton, scale: f32) {
        let b = 3.0; // border bevel thickness
        let (c_face, c_light, c_dark, c_text) = if !btn.enabled {
            (
                [0.18, 0.18, 0.18, 0.95],
                [0.25, 0.25, 0.25, 1.0],
                [0.10, 0.10, 0.10, 1.0],
                [0.60, 0.60, 0.60, 1.0],
            )
        } else if btn.pressed {
            (
                [0.45, 0.45, 0.45, 0.95],
                [0.75, 0.75, 0.75, 1.0],
                [0.25, 0.25, 0.25, 1.0],
                [1.0, 1.0, 0.65, 1.0], // Glowing yellow text when pressed
            )
        } else {
            (
                [0.32, 0.32, 0.32, 0.95],
                [0.55, 0.55, 0.55, 1.0],
                [0.18, 0.18, 0.18, 1.0],
                [1.0, 1.0, 1.0, 1.0], // White text
            )
        };

        // Outer black frame
        self.fill_rect(btn.x - 2.0, btn.y - 2.0, btn.w + 4.0, btn.h + 4.0, [0.0, 0.0, 0.0, 1.0]);

        // Main face
        self.fill_rect(btn.x, btn.y, btn.w, btn.h, c_face);

        // Top & Left highlight bevel
        self.fill_rect(btn.x, btn.y, btn.w, b, c_light);
        self.fill_rect(btn.x, btn.y, b, btn.h, c_light);

        // Bottom & Right shadow bevel
        self.fill_rect(btn.x, btn.y + btn.h - b, btn.w, b, c_dark);
        self.fill_rect(btn.x + btn.w - b, btn.y, b, btn.h, c_dark);

        // Centered button label
        let text_y = btn.y + (btn.h - 8.0 * scale) * 0.5;
        self.draw_centered_text(&btn.label, btn.x + btn.w * 0.5, text_y, scale, c_text, true);
    }

    /// Draws the Minecraft Title header banner.
    pub fn draw_minecraft_banner(&mut self, cx: f32, y: f32) {
        // Drop shadow header
        self.draw_centered_text(
            "HORIZON",
            cx,
            y,
            4.0,
            [0.95, 0.95, 0.95, 1.0],
            true,
        );

        // Subtitle banner in iconic gold/yellow
        self.draw_centered_text(
            "Minecraft: Java Edition 26.3 • Android ARM64",
            cx,
            y + 40.0,
            1.8,
            [1.0, 0.88, 0.20, 1.0],
            true,
        );
    }

    /// Draws the in-game world view (sky gradient, sun, horizon terrain responsive to look yaw/pitch).
    pub fn draw_game_world(
        &mut self,
        screen_w: f32,
        screen_h: f32,
        yaw: f32,
        pitch: f32,
        _time_ticks: u64,
    ) {
        let horizon_y = (screen_h * 0.5) + pitch * 2.5;

        // 1. Sky Gradient (top to horizon)
        let sky_top = [0.35, 0.58, 0.95, 1.0];
        let sky_horizon = [0.72, 0.84, 1.0, 1.0];

        // Draw sky slices
        let sky_h = horizon_y.max(0.0);
        let slices = 16;
        for i in 0..slices {
            let t0 = i as f32 / slices as f32;
            let t1 = (i + 1) as f32 / slices as f32;
            let y0 = t0 * sky_h;
            let y1 = t1 * sky_h;
            let c = [
                sky_top[0] * (1.0 - t0) + sky_horizon[0] * t0,
                sky_top[1] * (1.0 - t0) + sky_horizon[1] * t0,
                sky_top[2] * (1.0 - t0) + sky_horizon[2] * t0,
                1.0,
            ];
            self.fill_rect(0.0, y0, screen_w, y1 - y0 + 1.0, c);
        }

        // 2. Minecraft Sun
        let sun_x = (screen_w * 0.5 + yaw * 3.0) % (screen_w * 1.5);
        let sun_y = (horizon_y - 120.0).max(20.0);
        let sun_size = 54.0;
        self.fill_rect(sun_x - 4.0, sun_y - 4.0, sun_size + 8.0, sun_size + 8.0, [1.0, 1.0, 0.8, 0.4]);
        self.fill_rect(sun_x, sun_y, sun_size, sun_size, [1.0, 1.0, 0.95, 1.0]);

        // 3. Ground / Terrain (horizon down to screen bottom)
        let ground_y = horizon_y.clamp(0.0, screen_h);
        let ground_h = screen_h - ground_y;
        if ground_h > 0.0 {
            // Grass surface layer
            self.fill_rect(0.0, ground_y, screen_w, ground_h.min(48.0), [0.34, 0.58, 0.22, 1.0]);
            // Dirt layer below
            if ground_h > 48.0 {
                self.draw_tiled_dirt(0.0, ground_y + 48.0, screen_w, ground_h - 48.0, [0.45, 0.35, 0.25, 1.0]);
            }
        }

        // 4. Subtle distance grid lines giving 3D voxel perspective
        let grid_count = 12;
        for i in 0..grid_count {
            let offset = ((i as f32 * 60.0 + yaw * 2.0) % screen_w + screen_w) % screen_w;
            self.fill_rect(offset, ground_y, 2.0, ground_h, [0.28, 0.48, 0.18, 0.6]);
        }
    }

    /// Uploads all batch vertices to the GPU dynamic buffer and executes the render pass.
    pub fn flush(
        &mut self,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.vertices.is_empty() {
            return;
        }

        let vertex_bytes = bytemuck::cast_slice(&self.vertices);
        queue.write_buffer(&self.vertex_buffer, 0, vertex_bytes);

        let quad_count = self.vertices.len() / 4;
        let index_count = (quad_count * 6) as u32;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Screen Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Draw on top of background
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..vertex_bytes.len() as u64));
        pass.set_index_buffer(self.index_buffer.slice(..(index_count as usize * 2) as u64), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..index_count, 0, 0..1);
    }
}
