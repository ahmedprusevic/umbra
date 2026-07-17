//! CPU-sorted sprite batch and its one render pipeline.

use bytemuck::{Pod, Zeroable};

use super::atlas::{Atlas, Region};
use super::camera::Camera;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

#[derive(Clone, Copy)]
pub struct Sprite {
    pub region: Region,
    /// Bottom-center anchor, world pixels. `.1` is the draw-order key.
    pub pos: (f32, f32),
    pub layer: i32,
}

#[derive(Default)]
pub struct SpriteBatch {
    sprites: Vec<Sprite>,
}

impl SpriteBatch {
    pub fn clear(&mut self) {
        self.sprites.clear();
    }

    pub fn push(&mut self, region: Region, pos: (f32, f32), layer: i32) {
        self.sprites.push(Sprite { region, pos, layer });
    }

    /// Back-to-front: (layer, screen-Y). Stable sort keeps equal keys
    /// deterministic in push order.
    pub(crate) fn sorted(&self) -> Vec<Sprite> {
        let mut s = self.sprites.clone();
        s.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.pos.1.total_cmp(&b.pos.1)));
        s
    }
}

pub(crate) struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    camera_buf: wgpu::Buffer,
    camera_bg: wgpu::BindGroup,
    pub(crate) atlas_layout: wgpu::BindGroupLayout,
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    capacity_quads: u64,
}

const START_QUADS: u64 = 4096;

impl SpritePipeline {
    pub(crate) fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite"),
            bind_group_layouts: &[Some(&camera_layout), Some(&atlas_layout)], // wgpu 29+: Option-wrapped
            immediate_size: 0,
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout)], // wgpu 30: Option-wrapped
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera_buf.as_entire_binding() }],
        });
        let (vbuf, ibuf) = Self::make_buffers(device, START_QUADS);
        SpritePipeline {
            pipeline,
            camera_buf,
            camera_bg,
            atlas_layout,
            vbuf,
            ibuf,
            capacity_quads: START_QUADS,
        }
    }

    fn make_buffers(device: &wgpu::Device, quads: u64) -> (wgpu::Buffer, wgpu::Buffer) {
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite vertices"),
            size: quads * 4 * std::mem::size_of::<Vertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices: Vec<u32> = (0..quads as u32)
            .flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 1, 4 * q + 3])
            .collect();
        let ibuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite indices"),
            size: (indices.len() * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        (vbuf, ibuf)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        batch: &SpriteBatch,
        atlas: &Atlas,
        camera: &Camera,
        viewport: (u32, u32),
    ) {
        let sprites = batch.sorted();
        if sprites.is_empty() {
            return;
        }
        if sprites.len() as u64 > self.capacity_quads {
            self.capacity_quads = (sprites.len() as u64).next_power_of_two();
            let (v, i) = Self::make_buffers(device, self.capacity_quads);
            self.vbuf = v;
            self.ibuf = i;
        }
        // vertices and indices are rewritten every frame — one memcpy each,
        // and freshly-grown buffers are always populated before drawing:
        let mut verts = Vec::with_capacity(sprites.len() * 4);
        for s in &sprites {
            let [w, h] = s.region.size;
            let (x, y) = s.pos; // bottom-center anchor
            let (x0, x1) = (x - w / 2.0, x + w / 2.0);
            let (y0, y1) = (y - h, y);
            let [u0, v0] = s.region.uv_min;
            let [u1, v1] = s.region.uv_max;
            verts.push(Vertex { pos: [x0, y0], uv: [u0, v0] });
            verts.push(Vertex { pos: [x1, y0], uv: [u1, v0] });
            verts.push(Vertex { pos: [x0, y1], uv: [u0, v1] });
            verts.push(Vertex { pos: [x1, y1], uv: [u1, v1] });
        }
        queue.write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&verts));
        queue.write_buffer(&self.camera_buf, 0, bytemuck::cast_slice(&camera.view_proj(viewport)));
        let indices: Vec<u32> = (0..sprites.len() as u32)
            .flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 1, 4 * q + 3])
            .collect();
        queue.write_buffer(&self.ibuf, 0, bytemuck::cast_slice(&indices));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprites"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // clear() already ran
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bg, &[]);
        pass.set_bind_group(1, &atlas.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.set_index_buffer(self.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..sprites.len() as u32 * 6, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::atlas::Region;

    fn r() -> Region {
        Region { uv_min: [0.0, 0.0], uv_max: [1.0, 1.0], size: [10.0, 10.0] }
    }

    #[test]
    fn sorts_by_layer_then_screen_y() {
        let mut batch = SpriteBatch::default();
        batch.push(r(), (0.0, 5.0), 1); // top layer, low y
        batch.push(r(), (0.0, 9.0), 0); // ground, y=9
        batch.push(r(), (0.0, 2.0), 0); // ground, y=2 (drawn first)
        let order: Vec<(i32, f32)> = batch.sorted().iter().map(|s| (s.layer, s.pos.1)).collect();
        assert_eq!(order, vec![(0, 2.0), (0, 9.0), (1, 5.0)]);
    }
}
