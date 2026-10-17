//! wgpu renderer for chunk meshes: opaque + blended pass, fog, nearest-filtered terrain atlas.

use crate::hud::HudVertex;
use crate::ui::{Batch, Tex, UiFrame, TEX_COUNT};
use crate::mesh::{ChunkMesh, MeshData, Vertex};
use crate::sky::Env;
use crate::skyrender::{SkyAssets, SkyRenderer};
use std::collections::HashMap;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

pub const HUD_CAPACITY: usize = 65536;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    fog: [f32; 4],
    cam: [f32; 4],
    /// start, end, density, mode (0 linear / 1 exp)
    fog_p: [f32; 4],
}

/// Textures of world-space entity geometry (all sampled nearest, drawn with the particle pipeline).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntTex {
    Terrain,
    Items,
    Char,
    Zombie,
    Pig,
    Cow,
    Sheep,
    SheepFur,
    Chicken,
    Creeper,
    Skeleton,
    Spider,
    CaveSpider,
    Slime,
    LavaSlime,
    /// additive glow layer (`glBlendFunc(GL_ONE, GL_ONE)`)
    SpiderEyes,
    Door,
    Sign,
    Rain,
    Snow,
    Arrow,
    /// the glyph sheet, for text in the world (signs)
    Font,
}

pub const ENT_TEX_COUNT: usize = 22;

impl EntTex {
    pub const ALL: [EntTex; ENT_TEX_COUNT] = [
        EntTex::Terrain, EntTex::Items, EntTex::Char, EntTex::Zombie, EntTex::Pig, EntTex::Cow, EntTex::Sheep, EntTex::SheepFur,
        EntTex::Chicken, EntTex::Creeper, EntTex::Skeleton, EntTex::Spider, EntTex::CaveSpider, EntTex::Slime, EntTex::LavaSlime, EntTex::SpiderEyes, EntTex::Door, EntTex::Sign, EntTex::Rain, EntTex::Snow, EntTex::Arrow, EntTex::Font,
    ];
    /// path under `Minecraft.Client/Common/res/` (None = supplied separately)
    pub fn file(self) -> Option<&'static str> {
        Some(match self {
            EntTex::Terrain => return None,
            EntTex::Items => "gui/items.png",
            EntTex::Char => "mob/char.png",
            EntTex::Zombie => "mob/zombie.png",
            EntTex::Pig => "mob/pig.png",
            EntTex::Cow => "mob/cow.png",
            EntTex::Sheep => "mob/sheep.png",
            EntTex::SheepFur => "mob/sheep_fur.png",
            EntTex::Chicken => "mob/chicken.png",
            EntTex::Creeper => "mob/creeper.png",
            EntTex::Skeleton => "mob/skeleton.png",
            EntTex::Spider => "mob/spider.png",
            EntTex::CaveSpider => "1_2_2/mob/cavespider.png",
            EntTex::Slime => "mob/slime.png",
            EntTex::LavaSlime => "1_2_2/mob/lava.png",
            EntTex::SpiderEyes => "mob/spider_eyes.png",
            EntTex::Door => "item/door.png",
            EntTex::Sign => "item/sign.png",
            EntTex::Rain => "environment/rain.png",
            EntTex::Snow => "environment/snow.png",
            EntTex::Arrow => "item/arrows.png",
            EntTex::Font => "font/default.png",
        })
    }
}

/// Everything drawn between the terrain and the HUD besides chunks: entities in world space and the first-person hand.
#[derive(Default)]
pub struct EntityFrame {
    pub world: Vec<(EntTex, MeshData)>,
    /// geometry in eye space (drawn after clearing depth)
    pub hand: Vec<(EntTex, MeshData)>,
    /// projection × bobbing for the hand pass
    pub hand_proj: Option<Mat4>,
    /// rain / snow streaks (alpha-blended, no depth write)
    pub weather: Vec<(EntTex, MeshData)>,
}

pub struct GpuMesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    n: u32,
}

pub struct GpuChunk {
    pub center: Vec3,
    opaque: Option<GpuMesh>,
    trans: Option<GpuMesh>,
}

/// Left, right, bottom, top, near, far planes `(a, b, c, d)` with `ax + by + cz + d >= 0` inside (Gribb–Hartmann).
pub fn frustum_planes(m: &Mat4) -> [[f32; 4]; 6] {
    let r = [m.row(0), m.row(1), m.row(2), m.row(3)];
    let p = [r[3] + r[0], r[3] - r[0], r[3] + r[1], r[3] - r[1], r[3] + r[2], r[3] - r[2]];
    p.map(|v| v.to_array())
}

/// False only when the box is completely outside one plane.
pub fn box_in_frustum(planes: &[[f32; 4]; 6], min: [f32; 3], max: [f32; 3]) -> bool {
    planes.iter().all(|p| {
        let x = if p[0] >= 0.0 { max[0] } else { min[0] };
        let y = if p[1] >= 0.0 { max[1] } else { min[1] };
        let z = if p[2] >= 0.0 { max[2] } else { min[2] };
        p[0] * x + p[1] * y + p[2] * z + p[3] >= 0.0
    })
}

fn is_glow(t: usize) -> bool {
    t == EntTex::SpiderEyes as usize
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
    /// view-space offset applied after the look-at view (view bobbing)
    pub eye_post: Mat4,
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }
    /// View matrix for an eye at `pos`: look-at with an explicit up vector (stays valid when looking straight up/down),
    /// followed by the bobbing offset.
    pub fn view_at(&self, pos: Vec3) -> Mat4 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let up = Vec3::new(sy * sp, cp, cy * sp);
        self.eye_post * Mat4::look_to_rh(pos, self.forward(), up)
    }
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, aspect, 0.1, 2000.0) * self.view_at(self.pos)
    }
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    opaque_pipe: wgpu::RenderPipeline,
    trans_pipe: wgpu::RenderPipeline,
    particle_pipe: wgpu::RenderPipeline,
    crack_pipe: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buf: wgpu::Buffer,
    pub chunks: HashMap<(i32, i32), GpuChunk>,
    overlay: Option<GpuMesh>,
    particles: Option<GpuMesh>,
    crack: Option<GpuMesh>,
    hud_pipe: wgpu::RenderPipeline,
    hud_buf: wgpu::Buffer,
    hud_count: u32,
    hud_batches: Vec<Batch>,
    ui_layout: wgpu::BindGroupLayout,
    ui_sampler: wgpu::Sampler,
    ui_tex: Vec<Option<wgpu::BindGroup>>,
    pub sky: [f32; 3],
    pub fog_end: f32,
    lightmap: wgpu::Texture,
    light_key: Option<(u32, u8)>,
    skyr: Option<SkyRenderer>,
    format: wgpu::TextureFormat,
    bgl: wgpu::BindGroupLayout,
    atlas_sampler: wgpu::Sampler,
    /// tiling textures (rain, snow)
    repeat_sampler: wgpu::Sampler,
    lm_view: wgpu::TextureView,
    lm_samp: wgpu::Sampler,
    hand_uniform_buf: wgpu::Buffer,
    ent_bg: Vec<Option<wgpu::BindGroup>>,
    ent_bg_hand: Vec<Option<wgpu::BindGroup>>,
    weather_pipe: wgpu::RenderPipeline,
    glow_pipe: wgpu::RenderPipeline,
}

pub fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn upload(device: &wgpu::Device, m: &MeshData) -> Option<GpuMesh> {
    if m.is_empty() {
        return None;
    }
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&m.vertices), usage: wgpu::BufferUsages::VERTEX });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&m.indices), usage: wgpu::BufferUsages::INDEX });
    Some(GpuMesh { vb, ib, n: m.indices.len() as u32 })
}

impl Renderer {
    /// `atlas` is RGBA8, `aw × ah`.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat, atlas: &[u8], aw: u32, ah: u32, sky: [f32; 3]) -> Renderer {
        let fixed = fix_grass_side(atlas, aw);
        let atlas: &[u8] = &fixed;
        let tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("terrain"),
                size: wgpu::Extent3d { width: aw, height: ah, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            atlas,
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let repeat_sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Nearest, min_filter: wgpu::FilterMode::Nearest, address_mode_u: wgpu::AddressMode::Repeat, address_mode_v: wgpu::AddressMode::Repeat, ..Default::default() });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("uniforms"), size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let hand_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("hand uniforms"), size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let lightmap = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lightmap"),
            size: wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let lm_view = lightmap.create_view(&Default::default());
        let lm_samp = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge, ..Default::default() });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&lm_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&lm_samp) },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("terrain"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4, 3 => Float32x2];
        let vbl = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs };
        let mk = |entry: &str, blend: Option<wgpu::BlendState>, depth_write: bool, bias: wgpu::DepthBiasState, cull: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: "vs", compilation_options: Default::default(), buffers: &[vbl.clone()] },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: cull, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: depth_write, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: entry, compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })] }),
                multiview: None,
                cache: None,
            })
        };
        let no_bias = wgpu::DepthBiasState::default();
        let opaque_pipe = mk("fs_opaque", None, true, no_bias, Some(wgpu::Face::Back));
        let trans_pipe = mk("fs_trans", Some(wgpu::BlendState::ALPHA_BLENDING), false, no_bias, Some(wgpu::Face::Back));
        // GL_ALPHA_TEST > 1/255 + SRC_ALPHA / ONE_MINUS_SRC_ALPHA, depth writes on (ParticleEngine::render, opaque list)
        let particle_pipe = mk("fs_particle", Some(wgpu::BlendState::ALPHA_BLENDING), true, no_bias, None);
        // LevelRenderer::renderDestroyAnimation: glBlendFunc(GL_DST_COLOR, GL_SRC_COLOR), glPolygonOffset(-3, -3)
        let crack_blend = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Dst, dst_factor: wgpu::BlendFactor::Src, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent::OVER,
        };
        // rain / snow: alpha blended, depth-tested, no depth writes
        let weather_pipe = mk("fs_particle", Some(wgpu::BlendState::ALPHA_BLENDING), false, no_bias, None);
        // armor / eye layers: GL_ONE + GL_ONE, depth func LEQUAL, no depth writes
        let glow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glow"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs", compilation_options: Default::default(), buffers: &[vbl.clone()] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::LessEqual, stencil: Default::default(), bias: no_bias }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_glow",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState { color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent::OVER }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let crack_pipe = mk("fs_crack", Some(crack_blend), false, wgpu::DepthBiasState { constant: -3, slope_scale: -3.0, clamp: 0.0 }, None);
        let ui_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let hud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("ui"), bind_group_layouts: &[&ui_layout], push_constant_ranges: &[] });
        let ui_sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Nearest, min_filter: wgpu::FilterMode::Nearest, address_mode_u: wgpu::AddressMode::Repeat, address_mode_v: wgpu::AddressMode::Repeat, ..Default::default() });
        let hud_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("hud"), source: wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into()) });
        let hud_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        let hud_vbl = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<HudVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &hud_attrs };
        let hud_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud"),
            layout: Some(&hud_layout),
            vertex: wgpu::VertexState { module: &hud_shader, entry_point: "vs", compilation_options: Default::default(), buffers: &[hud_vbl] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::Always, stencil: Default::default(), bias: Default::default() }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &hud_shader, entry_point: "fs", compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            multiview: None,
            cache: None,
        });
        let hud_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("hud"), size: (HUD_CAPACITY * std::mem::size_of::<HudVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let mut r = Renderer { device, queue, opaque_pipe, trans_pipe, particle_pipe, crack_pipe, bind_group, uniform_buf, chunks: HashMap::new(), overlay: None, particles: None, crack: None, hud_pipe, hud_buf, hud_count: 0, hud_batches: Vec::new(), ui_layout, ui_sampler, ui_tex: (0..TEX_COUNT).map(|_| None).collect(), sky, fog_end: 200.0, lightmap, light_key: None, skyr: None, format, bgl, atlas_sampler: sampler, repeat_sampler, lm_view, lm_samp, hand_uniform_buf, ent_bg: (0..ENT_TEX_COUNT).map(|_| None).collect(), ent_bg_hand: (0..ENT_TEX_COUNT).map(|_| None).collect(), weather_pipe, glow_pipe };
        r.set_entity_texture(EntTex::Terrain, atlas, aw, ah);
        r.set_ui_texture(Tex::Terrain, atlas, aw, ah);
        r
    }

    fn make_bg(&self, uniform: &wgpu::Buffer, view: &wgpu::TextureView, tiling: bool) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(if tiling { &self.repeat_sampler } else { &self.atlas_sampler }) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.lm_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.lm_samp) },
            ],
        })
    }

    /// Upload the texture of an entity layer (mob skins, item sheet, rain, ...).
    pub fn set_entity_texture(&mut self, t: EntTex, rgba: &[u8], w: u32, h: u32) {
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("entity"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let tiling = matches!(t, EntTex::Rain | EntTex::Snow);
        let a = self.make_bg(&self.uniform_buf, &view, tiling);
        let b = self.make_bg(&self.hand_uniform_buf, &view, tiling);
        self.ent_bg[t as usize] = Some(a);
        self.ent_bg_hand[t as usize] = Some(b);
    }

    /// Forget the mesh of a chunk that was unloaded.
    pub fn remove_chunk(&mut self, cx: i32, cz: i32) {
        self.chunks.remove(&(cx, cz));
    }

    /// Add or replace the mesh of chunk (`cx`, `cz`).
    pub fn set_chunk(&mut self, cx: i32, cz: i32, mesh: &ChunkMesh) {
        let center = Vec3::new(cx as f32 * 16.0 + 8.0, 64.0, cz as f32 * 16.0 + 8.0);
        self.chunks.insert((cx, cz), GpuChunk { center, opaque: upload(&self.device, &mesh.opaque), trans: upload(&self.device, &mesh.translucent) });
    }

    /// Geometry drawn on top of the world with the opaque pipeline (block selection outline).
    pub fn set_overlay(&mut self, m: Option<&MeshData>) {
        self.overlay = m.and_then(|m| upload(&self.device, m));
    }

    /// This frame's particle quads (terrain atlas).
    pub fn set_particles(&mut self, m: Option<&MeshData>) {
        self.particles = m.and_then(|m| upload(&self.device, m));
    }

    /// Block-breaking crack overlay (`LevelRenderer::renderDestroyAnimation`).
    pub fn set_crack(&mut self, m: Option<&MeshData>) {
        self.crack = m.and_then(|m| upload(&self.device, m));
    }

    pub fn triangle_count(&self) -> u64 {
        self.chunks.values().map(|c| c.opaque.as_ref().map_or(0, |m| m.n as u64) + c.trans.as_ref().map_or(0, |m| m.n as u64)).sum::<u64>() / 3
    }

    pub fn set_sky_assets(&mut self, assets: &SkyAssets) {
        self.skyr = Some(SkyRenderer::new(&self.device, &self.queue, self.format, assets));
    }

    fn update_lightmap(&mut self, env: &Env) {
        let key = (env.sky_darken.to_bits(), env.dim as u8);
        if self.light_key == Some(key) {
            return;
        }
        self.light_key = Some(key);
        let px = crate::sky::light_texture(env.dim, env.sky_darken, env.blr);
        self.queue.write_texture(
            wgpu::ImageCopyTexture { texture: &self.lightmap, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &px,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(64), rows_per_image: Some(16) },
            wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
        );
    }

    /// Upload one of the GUI sheets (RGBA8).
    pub fn set_ui_texture(&mut self, t: Tex, rgba: &[u8], w: u32, h: u32) {
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor { label: Some("ui"), size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING, view_formats: &[] },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui"),
            layout: &self.ui_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.ui_sampler) }],
        });
        self.ui_tex[t as usize] = Some(bg);
    }

    /// The GUI pass: batches by texture over whatever is already in `color`.
    fn ui_pass(&self, enc: &mut wgpu::CommandEncoder, color: &wgpu::TextureView, depth: &wgpu::TextureView) {
        if self.hud_count == 0 {
            return;
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hud"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.hud_pipe);
        pass.set_vertex_buffer(0, self.hud_buf.slice(..));
        for b in &self.hud_batches {
            let end = (b.start + b.count).min(self.hud_count);
            if b.start >= end {
                continue;
            }
            if let Some(bg) = &self.ui_tex[b.tex as usize] {
                pass.set_bind_group(0, bg, &[]);
                pass.draw(b.start..end, 0..1);
            }
        }
    }

    /// Menu screens without a world behind them: black clear, then the GUI.
    pub fn render_ui_only(&mut self, color: &wgpu::TextureView, depth: &wgpu::TextureView, ui: &UiFrame) {
        let n = ui.verts.len().min(HUD_CAPACITY);
        if n > 0 {
            self.queue.write_buffer(&self.hud_buf, 0, bytemuck::cast_slice(&ui.verts[..n]));
        }
        self.hud_count = n as u32;
        self.hud_batches = ui.batches.clone();
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        drop(enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        }));
        self.ui_pass(&mut enc, color, depth);
        self.queue.submit(Some(enc.finish()));
    }

    /// Drop every chunk mesh (leaving the world).
    pub fn clear_chunks(&mut self) {
        self.chunks.clear();
        self.overlay = None;
        self.particles = None;
        self.crack = None;
    }

    pub fn render(&mut self, color: &wgpu::TextureView, depth: &wgpu::TextureView, cam: &Camera, aspect: f32, ui: &UiFrame, env: &Env, ents: &EntityFrame) {
        let n = ui.verts.len().min(HUD_CAPACITY);
        if n > 0 {
            self.queue.write_buffer(&self.hud_buf, 0, bytemuck::cast_slice(&ui.verts[..n]));
        }
        self.hud_count = n as u32;
        self.hud_batches = ui.batches.clone();
        self.update_lightmap(env);
        let u = Uniforms {
            view_proj: cam.view_proj(aspect).to_cols_array_2d(),
            fog: [env.clear[0], env.clear[1], env.clear[2], 1.0],
            cam: [cam.pos.x, cam.pos.y, cam.pos.z, self.fog_end],
            fog_p: env.fog_world,
        };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&u));
        if let Some(hp) = ents.hand_proj {
            // no fog on the hand
            let hu = Uniforms { view_proj: hp.to_cols_array_2d(), fog: u.fog, cam: [0.0, 0.0, 0.0, 1.0e9], fog_p: [1.0e8, 2.0e8, 0.0, 0.0] };
            self.queue.write_buffer(&self.hand_uniform_buf, 0, bytemuck::bytes_of(&hu));
        }
        let up = |list: &[(EntTex, MeshData)], dev: &wgpu::Device| -> Vec<(usize, GpuMesh)> { list.iter().filter_map(|(t, m)| upload(dev, m).map(|g| (*t as usize, g))).collect() };
        let world_gpu = up(&ents.world, &self.device);
        let weather_gpu = up(&ents.weather, &self.device);
        let hand_gpu = if ents.hand_proj.is_some() { up(&ents.hand, &self.device) } else { Vec::new() };
        // frustum culling: chunks outside the view volume (6 planes of the clip matrix) are not drawn
        let planes = frustum_planes(&cam.view_proj(aspect));
        let mut order: Vec<(i32, i32)> = self.chunks.keys().copied().filter(|&(cx, cz)| box_in_frustum(&planes, [cx as f32 * 16.0, 0.0, cz as f32 * 16.0], [cx as f32 * 16.0 + 16.0, 256.0, cz as f32 * 16.0 + 16.0])).collect();
        order.sort_by(|a, b| {
            let da = (self.chunks[a].center - cam.pos).length_squared();
            let db = (self.chunks[b].center - cam.pos).length_squared();
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let c = env.clear;
        // 1. clear
        drop(enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        }));
        // 2. sky
        if let Some(sk) = &self.skyr {
            sk.render_sky(&self.device, &self.queue, &mut enc, color, depth, cam, aspect, env);
        }
        // 3. terrain
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.opaque_pipe);
            for i in order.iter().rev() {
                if let Some(m) = &self.chunks[i].opaque {
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.n, 0, 0..1);
                }
            }
            if let Some(m) = &self.overlay {
                pass.set_vertex_buffer(0, m.vb.slice(..));
                pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.n, 0, 0..1);
            }
            if let Some(m) = &self.crack {
                pass.set_pipeline(&self.crack_pipe);
                pass.set_vertex_buffer(0, m.vb.slice(..));
                pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.n, 0, 0..1);
            }
            if let Some(m) = &self.particles {
                pass.set_pipeline(&self.particle_pipe);
                pass.set_vertex_buffer(0, m.vb.slice(..));
                pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.n, 0, 0..1);
            }
            // entities (dropped items, mobs): alpha-tested, depth-writing
            pass.set_pipeline(&self.particle_pipe);
            for (t, m) in world_gpu.iter().filter(|(t, _)| !is_glow(*t)) {
                if let Some(bg) = &self.ent_bg[*t] {
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.n, 0, 0..1);
                }
            }
            pass.set_pipeline(&self.glow_pipe);
            for (t, m) in world_gpu.iter().filter(|(t, _)| is_glow(*t)) {
                if let Some(bg) = &self.ent_bg[*t] {
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.n, 0, 0..1);
                }
            }
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.trans_pipe);
            for i in &order {
                if let Some(m) = &self.chunks[i].trans {
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.n, 0, 0..1);
                }
            }
        }
        // 4. clouds
        if let Some(sk) = &self.skyr {
            sk.render_clouds(&self.device, &mut enc, color, depth, [cam.pos.x as f64, cam.pos.y as f64, cam.pos.z as f64], env);
        }
        // 4b. weather streaks
        if !weather_gpu.is_empty() {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weather"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.weather_pipe);
            for (t, m) in &weather_gpu {
                if let Some(bg) = &self.ent_bg[*t] {
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.n, 0, 0..1);
                }
            }
        }
        // 4c. first-person hand on a cleared depth buffer
        if !hand_gpu.is_empty() {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hand"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.particle_pipe);
            for (t, m) in &hand_gpu {
                if let Some(bg) = &self.ent_bg_hand[*t] {
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.n, 0, 0..1);
                }
            }
        }
        // 5. HUD / screens
        self.ui_pass(&mut enc, color, depth);
        self.queue.submit(Some(enc.finish()));
    }
}

/// The grass side icon has its green fringe baked in; the tinted `grass_side_overlay` is drawn over it, but the two
/// fringes do not line up, so bright green pixels peek out next to biome-tinted grass. Paint the baked fringe with
/// the dirt icon instead (the overlay supplies the grass).
pub fn fix_grass_side(atlas: &[u8], aw: u32) -> Vec<u8> {
    let mut out = atlas.to_vec();
    let cell = (aw / 16) as usize;
    let stride = aw as usize * 4;
    if cell == 0 || out.len() < stride * cell {
        return out;
    }
    for y in 0..cell {
        for x in 0..cell {
            let side = y * stride + (3 * cell + x) * 4;
            let dirt = y * stride + (2 * cell + x) * 4;
            let (r, g, b) = (out[side] as i32, out[side + 1] as i32, out[side + 2] as i32);
            if g > r + 14 && g > b + 14 {
                for k in 0..4 {
                    out[side + k] = out[dirt + k];
                }
            }
        }
    }
    out
}

/// Decode a PNG into RGBA8.
pub fn load_png_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut r = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("indexed png not expanded".into()),
    };
    Ok((rgba, info.width, info.height))
}
