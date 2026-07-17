//! One texture, many named regions.

use std::collections::HashMap;

use super::Gfx;

/// A named sub-rectangle of the atlas, in UV space plus pixel size.
#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub size: [f32; 2], // pixels
}

pub struct Atlas {
    pub(crate) bind_group: wgpu::BindGroup,
    regions: HashMap<String, Region>,
}

impl Atlas {
    /// Upload an RGBA image and name rectangular regions of it:
    /// `(name, [x, y, w, h])` in pixels.
    pub fn new(gfx: &Gfx, img: &image::RgbaImage, regions: &[(&str, [u32; 4])]) -> Atlas {
        let (w, h) = img.dimensions();
        let texture = gfx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gfx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = gfx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = gfx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas"),
            layout: &gfx.sprites.atlas_layout,
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
        let regions = regions
            .iter()
            .map(|(name, [x, y, rw, rh])| {
                let r = Region {
                    uv_min: [*x as f32 / w as f32, *y as f32 / h as f32],
                    uv_max: [(*x + *rw) as f32 / w as f32, (*y + *rh) as f32 / h as f32],
                    size: [*rw as f32, *rh as f32],
                };
                (name.to_string(), r)
            })
            .collect();
        Atlas { bind_group, regions }
    }

    /// Panics on unknown name — regions are dev-time constants.
    pub fn region(&self, name: &str) -> Region {
        *self.regions.get(name).unwrap_or_else(|| panic!("no atlas region '{name}'"))
    }
}

/// Procedural placeholder atlas so examples need no asset files: flat-color
/// diamond tiles, a unit billboard, and a selection ring.
// ponytail: dev art, not a content pipeline — the game repo loads real PNGs
// (e.g. Kenney iso packs, 128x64 tiles) through Atlas::new the same way.
pub fn dev_atlas(gfx: &Gfx) -> Atlas {
    let mut img = image::RgbaImage::new(256, 256);
    let diamond = |img: &mut image::RgbaImage, x0: u32, y0: u32, rgba: [u8; 4]| {
        for dy in 0..64u32 {
            for dx in 0..128u32 {
                let fx = (dx as f32 + 0.5) / 128.0 - 0.5;
                let fy = (dy as f32 + 0.5) / 64.0 - 0.5;
                if fx.abs() + fy.abs() <= 0.5 {
                    img.put_pixel(x0 + dx, y0 + dy, image::Rgba(rgba));
                }
            }
        }
    };
    diamond(&mut img, 0, 0, [92, 158, 82, 255]); // "grass"
    diamond(&mut img, 128, 0, [196, 178, 122, 255]); // "sand"
    diamond(&mut img, 0, 64, [72, 74, 78, 255]); // "block"
    // unit: 24x36 solid body at (128, 64)
    for dy in 0..36u32 {
        for dx in 0..24u32 {
            img.put_pixel(128 + dx, 64 + dy, image::Rgba([214, 64, 48, 255]));
        }
    }
    // ring: 48x24 ellipse outline at (160, 64)
    for dy in 0..24u32 {
        for dx in 0..48u32 {
            let fx = (dx as f32 + 0.5) / 48.0 - 0.5;
            let fy = (dy as f32 + 0.5) / 24.0 - 0.5;
            let d = (fx * fx * 4.0 + fy * fy * 4.0).sqrt();
            if (0.8..=1.0).contains(&d) {
                img.put_pixel(160 + dx, 64 + dy, image::Rgba([255, 255, 255, 255]));
            }
        }
    }
    Atlas::new(
        gfx,
        &img,
        &[
            ("grass", [0, 0, 128, 64]),
            ("sand", [128, 0, 128, 64]),
            ("block", [0, 64, 128, 64]),
            ("unit", [128, 64, 24, 36]),
            ("ring", [160, 64, 48, 24]),
        ],
    )
}
