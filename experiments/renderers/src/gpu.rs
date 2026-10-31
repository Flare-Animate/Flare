//! The vello_gpu candidate. The device and the renderer are made once and kept, because
//! creating them (adapter, pipelines) costs far more than a frame; `first` in the output is that
//! cold cost. Any failure is an `Err` so the caller can fall back to the CPU rasterizer.

use std::cell::RefCell;

use vello_gpu::kurbo::{BezPath, Cap, Join, Rect, Stroke};
use vello_gpu::peniko::color::{AlphaColor, Srgb};
use vello_gpu::{
    ClearSettings, RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TargetInit,
    TextureBindings,
};

use crate::scene::{Item, Seg};

struct State {
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: (u16, u16),
    texture: wgpu::Texture,
    renderer: Renderer,
    resources: Resources,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn open(w: u16, h: u16) -> Result<State, String> {
    // Lets the fallback be tried without taking the GPU away: FLARE_NO_GPU=1.
    if std::env::var_os("FLARE_NO_GPU").is_some() {
        return Err("disabled by FLARE_NO_GPU".into());
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU adapter: {e}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("bench"),
        required_features: wgpu::Features::empty(),
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU device: {e}"))?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: w.into(),
            height: h.into(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let (renderer, resources) = Renderer::new(
        &device,
        &RenderTargetConfig {
            format: texture.format(),
            width: w,
            height: h,
        },
    );
    Ok(State {
        device,
        queue,
        size: (w, h),
        texture,
        renderer,
        resources,
    })
}

pub fn render(items: &[Item], w: u32, h: u32, readback: bool) -> Result<Vec<u8>, String> {
    let (w16, h16) = (
        u16::try_from(w).map_err(|_| "too wide")?,
        u16::try_from(h).map_err(|_| "too tall")?,
    );
    STATE.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.as_ref().is_none_or(|s| s.size != (w16, h16)) {
            *slot = Some(open(w16, h16)?);
        }
        let state = slot.as_mut().ok_or("no state")?;
        draw(state, items, w16, h16, readback)
    })
}

fn draw(state: &mut State, items: &[Item], w: u16, h: u16, readback: bool) -> Result<Vec<u8>, String> {
    let mut scene = Scene::new(w, h);
    scene.set_paint(AlphaColor::<Srgb>::from_rgba8(255, 255, 255, 255));
    scene.fill_rect(&Rect::new(0.0, 0.0, f64::from(w), f64::from(h)));
    for item in items {
        let mut path = BezPath::new();
        for seg in &item.segs {
            match *seg {
                Seg::Move(x, y) => path.move_to((f64::from(x), f64::from(y))),
                Seg::Line(x, y) => path.line_to((f64::from(x), f64::from(y))),
                Seg::Cubic(a, b, c, d, e, f) => path.curve_to(
                    (f64::from(a), f64::from(b)),
                    (f64::from(c), f64::from(d)),
                    (f64::from(e), f64::from(f)),
                ),
                Seg::Close => path.close_path(),
            }
        }
        let [r, g, b, a] = item.rgba;
        scene.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
        match item.stroke {
            None => scene.fill_path(&path),
            Some(width) => {
                scene.set_stroke(
                    Stroke::new(f64::from(width))
                        .with_caps(Cap::Round)
                        .with_join(Join::Round),
                );
                scene.stroke_path(&path);
            }
        }
    }
    let view = state
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let size = RenderSize {
        width: w,
        height: h,
    };
    let depth = Renderer::create_depth_texture_view(&state.device, &size);
    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
    state
        .renderer
        .render(
            &scene,
            &mut state.resources,
            &state.device,
            &state.queue,
            &mut encoder,
            &size,
            &view,
            Some(&depth),
            &TextureBindings::new(),
            TargetInit::Clear(ClearSettings::default()),
        )
        .map_err(|e| format!("render: {e:?}"))?;
    if !readback {
        state.queue.submit([encoder.finish()]);
        state
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("poll: {e:?}"))?;
        return Ok(Vec::new());
    }
    let row = (u32::from(w) * 4).next_multiple_of(256);
    let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &state.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: w.into(),
            height: h.into(),
            depth_or_array_layers: 1,
        },
    );
    state.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    state
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| format!("poll: {e:?}"))?;
    let mut out = Vec::with_capacity(usize::from(w) * usize::from(h) * 4);
    {
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| format!("map: {e:?}"))?;
        for line in mapped.chunks_exact(row as usize) {
            out.extend_from_slice(&line[..usize::from(w) * 4]);
        }
    }
    buffer.unmap();
    Ok(out)
}
