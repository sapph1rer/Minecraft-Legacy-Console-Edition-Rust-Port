//! GPU side of `LevelRenderer::renderSky` / `renderClouds`.

use crate::mesh::Vertex;
use crate::render::{Camera, DEPTH_FORMAT};
use crate::sky::{self, Env, Quads};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyU {
    vp: [[f32; 4]; 4],
    fog: [f32; 4],
    fog_sky: [f32; 4],
    fog_world: [f32; 4],
    tint: [f32; 4],
}

pub struct Rgba {
    pub px: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

pub struct SkyAssets {
    pub sun: Rgba,
    pub moon: Rgba,
    pub clouds: Rgba,
    pub tunnel: Rgba,
}

struct Mesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    n: u32,
}

fn upload(device: &wgpu::Device, q: &Quads) -> Option<Mesh> {
    if q.1.is_empty() {
        return None;
    }
    Some(Mesh {
        vb: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&q.0), usage: wgpu::BufferUsages::VERTEX }),
        ib: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&q.1), usage: wgpu::BufferUsages::INDEX }),
        n: q.1.len() as u32,
    })
}

pub struct SkyRenderer {
    ubufs: [wgpu::Buffer; 4],
    /// white texture × sky colour / × 1 / × star brightness / × black
    bg_sky: wgpu::BindGroup,
    bg_one: wgpu::BindGroup,
    bg_star: wgpu::BindGroup,
    bg_black: wgpu::BindGroup,
    bg_sun: wgpu::BindGroup,
    bg_moon: wgpu::BindGroup,
    bg_tunnel: wgpu::BindGroup,
    bg_cloud: wgpu::BindGroup,
    p_dome: wgpu::RenderPipeline,
    p_alpha: wgpu::RenderPipeline,
    p_add: wgpu::RenderPipeline,
    p_cloud_depth: wgpu::RenderPipeline,
    p_cloud: wgpu::RenderPipeline,
    stars: Option<Mesh>,
    plane_up: Option<Mesh>,
    plane_down: Option<Mesh>,
    end_box: Option<Mesh>,
    cloud_alpha: Vec<u8>,
}

fn texture(device: &wgpu::Device, queue: &wgpu::Queue, t: &Rgba) -> wgpu::TextureView {
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: t.w, height: t.h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &t.px,
        )
        .create_view(&Default::default())
}

impl SkyRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat, assets: &SkyAssets) -> SkyRenderer {
        let mkbuf = || device.create_buffer(&wgpu::BufferDescriptor { label: Some("sky-u"), size: std::mem::size_of::<SkyU>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let ubufs = [mkbuf(), mkbuf(), mkbuf(), mkbuf()];
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let clamp = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Nearest, min_filter: wgpu::FilterMode::Nearest, address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge, ..Default::default() });
        let repeat = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, address_mode_u: wgpu::AddressMode::Repeat, address_mode_v: wgpu::AddressMode::Repeat, ..Default::default() });
        let mk_bg = |ub: usize, view: &wgpu::TextureView, s: &wgpu::Sampler| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubufs[ub].as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) }, wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(s) }],
            })
        };
        let white = texture(device, queue, &Rgba { px: vec![255; 4], w: 1, h: 1 });
        let bg_sky = mk_bg(0, &white, &clamp);
        let bg_one = mk_bg(1, &white, &clamp);
        let bg_star = mk_bg(2, &white, &clamp);
        let bg_black = mk_bg(3, &white, &clamp);
        let bg_sun = mk_bg(1, &texture(device, queue, &assets.sun), &clamp);
        let bg_moon = mk_bg(1, &texture(device, queue, &assets.moon), &clamp);
        let bg_tunnel = mk_bg(1, &texture(device, queue, &assets.tunnel), &repeat);
        let bg_cloud = mk_bg(1, &white, &clamp);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("sky"), source: wgpu::ShaderSource::Wgsl(include_str!("sky.wgsl").into()) });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4, 3 => Float32x2];
        let vbl = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs };
        let add = wgpu::BlendState { color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add } };
        let mk = |entry: &str, blend: Option<wgpu::BlendState>, depth: Option<(bool, wgpu::CompareFunction)>, mask: wgpu::ColorWrites| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: "vs", compilation_options: Default::default(), buffers: &[vbl.clone()] },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: depth.map_or(false, |d| d.0), depth_compare: depth.map_or(wgpu::CompareFunction::Always, |d| d.1), stencil: Default::default(), bias: Default::default() }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: entry, compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: mask })] }),
                multiview: None,
                cache: None,
            })
        };
        let p_dome = mk("fs_dome", None, None, wgpu::ColorWrites::ALL);
        let p_alpha = mk("fs_plain", Some(wgpu::BlendState::ALPHA_BLENDING), None, wgpu::ColorWrites::ALL);
        let p_add = mk("fs_plain", Some(add), None, wgpu::ColorWrites::ALL);
        let p_cloud_depth = mk("fs_depth_only", None, Some((true, wgpu::CompareFunction::Less)), wgpu::ColorWrites::empty());
        let p_cloud = mk("fs_cloud", Some(wgpu::BlendState::ALPHA_BLENDING), Some((false, wgpu::CompareFunction::LessEqual)), wgpu::ColorWrites::ALL);
        let cloud_alpha: Vec<u8> = assets.clouds.px.chunks(4).map(|p| p[3]).collect();
        SkyRenderer {
            ubufs,
            bg_sky,
            bg_one,
            bg_star,
            bg_black,
            bg_sun,
            bg_moon,
            bg_tunnel,
            bg_cloud,
            p_dome,
            p_alpha,
            p_add,
            p_cloud_depth,
            p_cloud,
            stars: upload(device, &sky::star_mesh()),
            plane_up: upload(device, &sky::plane_mesh(16.0)),
            plane_down: upload(device, &sky::plane_mesh(-4.0)),
            end_box: upload(device, &sky::end_sky_box()),
            cloud_alpha,
        }
    }

    fn write_u(&self, queue: &wgpu::Queue, cam: &Camera, aspect: f32, env: &Env) {
        let proj = Mat4::perspective_rh(cam.fov_y, aspect, 0.1, 2000.0);
        let view = cam.view_at(Vec3::ZERO);
        let vp = (proj * view).to_cols_array_2d();
        let tints = [[env.sky[0], env.sky[1], env.sky[2], 1.0], [1.0; 4], [env.star_br * env.rain_brightness; 4], [0.0, 0.0, 0.0, 1.0]];
        for (i, t) in tints.iter().enumerate() {
            let u = SkyU { vp, fog: [env.clear[0], env.clear[1], env.clear[2], 1.0], fog_sky: [env.fog_sky[0], env.fog_sky[1], 0.0, 0.0], fog_world: env.fog_world, tint: *t };
            queue.write_buffer(&self.ubufs[i], 0, bytemuck::bytes_of(&u));
        }
    }

    fn draw<'a>(pass: &mut wgpu::RenderPass<'a>, m: &'a Mesh) {
        pass.set_vertex_buffer(0, m.vb.slice(..));
        pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..m.n, 0, 0..1);
    }

    /// Sky, sun, moon, stars (before the terrain). Colour/depth were cleared by an earlier pass; this one loads them.
    pub fn render_sky(&self, device: &wgpu::Device, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, color: &wgpu::TextureView, depth: &wgpu::TextureView, cam: &Camera, aspect: f32, env: &Env) {
        self.write_u(queue, cam, aspect, env);
        let mut fan = None;
        let (mut sun, mut moon) = (None, None);
        if env.natural {
            let (s, m) = sky::sun_moon(env.td, env.moon_phase, env.rain_brightness);
            sun = upload(device, &s);
            moon = upload(device, &m);
            if let Some(c) = env.sunrise {
                fan = upload(device, &sky::sunrise_fan(c, env.sun_angle));
            }
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sky"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if env.dim == crate::world::Dim::End {
            pass.set_pipeline(&self.p_alpha);
            pass.set_bind_group(0, &self.bg_tunnel, &[]);
            if let Some(m) = &self.end_box {
                Self::draw(&mut pass, m);
            }
            return;
        }
        if !env.natural {
            return;
        }
        pass.set_pipeline(&self.p_dome);
        pass.set_bind_group(0, &self.bg_sky, &[]);
        if let Some(m) = &self.plane_up {
            Self::draw(&mut pass, m);
        }
        if let Some(m) = &fan {
            pass.set_pipeline(&self.p_alpha);
            pass.set_bind_group(0, &self.bg_one, &[]);
            Self::draw(&mut pass, m);
        }
        pass.set_pipeline(&self.p_add);
        if let Some(m) = &sun {
            pass.set_bind_group(0, &self.bg_sun, &[]);
            Self::draw(&mut pass, m);
        }
        if let Some(m) = &moon {
            pass.set_bind_group(0, &self.bg_moon, &[]);
            Self::draw(&mut pass, m);
        }
        if env.star_br * env.rain_brightness > 0.0 {
            pass.set_bind_group(0, &self.bg_star, &[]);
            if let Some(m) = &self.stars {
                Self::draw(&mut pass, m);
            }
        }
        if env.below_horizon {
            pass.set_pipeline(&self.p_dome);
            pass.set_bind_group(0, &self.bg_black, &[]);
            if let Some(m) = &self.plane_down {
                Self::draw(&mut pass, m);
            }
        }
    }

    /// Fancy clouds, drawn after the terrain with a depth pre-pass so only the nearest layer blends.
    pub fn render_clouds(&self, device: &wgpu::Device, enc: &mut wgpu::CommandEncoder, color: &wgpu::TextureView, depth: &wgpu::TextureView, cam_pos: [f64; 3], env: &Env) {
        if !env.natural {
            return;
        }
        let q = sky::cloud_mesh(&self.cloud_alpha, env.cloud_rgb, cam_pos[0], cam_pos[1] as f32, cam_pos[2], env.cloud_ticks, env.cloud_height);
        let Some(m) = upload(device, &q) else { return };
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clouds"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: color, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.bg_cloud, &[]);
        pass.set_pipeline(&self.p_cloud_depth);
        Self::draw(&mut pass, &m);
        pass.set_pipeline(&self.p_cloud);
        Self::draw(&mut pass, &m);
    }
}
