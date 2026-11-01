use super::*;

#[test]
fn a_stall_adds_at_most_one_step_of_movie_time() {
    assert_eq!(step(Duration::from_millis(16)), Duration::from_millis(16));
    assert_eq!(step(Duration::from_secs(30)), MAX_STEP);
    assert_eq!(step(Duration::ZERO), Duration::ZERO);
}

#[test]
fn a_picture_that_fits_is_shown_as_it_is_and_one_that_does_not_is_shrunk_to_fit() {
    let room = egui::vec2(1000.0, 800.0);
    assert_eq!(
        fit(egui::vec2(400.0, 300.0), room),
        egui::vec2(400.0, 300.0)
    );
    // Too wide: shrunk until it fits inside nine tenths of the room, keeping its shape.
    let shrunk = fit(egui::vec2(1800.0, 900.0), room);
    assert!(
        (shrunk.x - 900.0).abs() < 0.01 && (shrunk.y - 450.0).abs() < 0.01,
        "{shrunk:?}"
    );
    // Too tall.
    let shrunk = fit(egui::vec2(500.0, 1440.0), room);
    assert!(
        (shrunk.y - 720.0).abs() < 0.01 && (shrunk.x - 250.0).abs() < 0.01,
        "{shrunk:?}"
    );
}

#[test]
fn a_picture_with_no_size_or_a_room_with_none_shows_nothing_and_does_not_divide_by_zero() {
    assert_eq!(
        fit(egui::Vec2::ZERO, egui::vec2(100.0, 100.0)),
        egui::Vec2::ZERO
    );
    assert_eq!(
        fit(egui::vec2(10.0, 10.0), egui::Vec2::ZERO),
        egui::Vec2::ZERO
    );
}

/// The Test Movie window as the editor draws it: egui paints the window into an off-screen
/// texture on a device of its own, and the picture of the movie is looked for in what came out.
/// Needs a GPU adapter, so it is ignored in the ordinary run (`scripts/gpu_tests.sh`).
#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
#[allow(clippy::expect_used)]
fn the_window_shows_the_picture_of_the_movie() {
    use egui_wgpu::{
        RenderState, RendererOptions, ScreenDescriptor, WgpuConfiguration, WgpuSetup, wgpu,
    };
    use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
    use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Timeline};

    // A movie of one red frame, 120 x 90.
    let shape = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        120.0,
        90.0,
        Rgba8::opaque(220, 30, 30),
    )
    .expect("rect");
    let span = FrameSpan::new(0, std::num::NonZeroU32::MIN, Content::Shape(shape));
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(120, 90, Rgba8::WHITE).expect("stage"));
    let swf = flare_publish::publish_swf(&document, &flare_publish::PublishOptions::default())
        .expect("publish")
        .bytes;

    let config = WgpuConfiguration {
        wgpu_setup: WgpuSetup::without_display_handle(),
        ..WgpuConfiguration::default()
    };
    let instance = pollster::block_on(config.wgpu_setup.new_instance());
    let state = pollster::block_on(RenderState::create(
        &config,
        &instance,
        None,
        RendererOptions::default(),
    ))
    .expect("a GPU adapter");
    let mut window = MovieWindow::open(swf, &state, 1.0).expect("the movie starts");

    let (width, height) = (640_u32, 480_u32);
    let target = state.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: state.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut pixels = Vec::new();
    let mut texts = Vec::new();
    // Frames, as the window has them: each one ticks the movie a little and paints.
    for frame in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(16));
        let raw = egui::RawInput {
            time: Some(f64::from(frame) * 0.016),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width as f32, height as f32),
            )),
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(raw, |ui| window.show(ui.ctx()));
        texts = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some((
                    t.galley.text().to_owned(),
                    t.visual_bounding_rect().center(),
                )),
                _ => None,
            })
            .collect();
        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        let screen = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: 1.0,
        };
        let mut encoder = state.device.create_command_encoder(&Default::default());
        let mut renderer = state.renderer.write();
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                renderer.update_texture(&state.device, &state.queue, *id, delta);
            }
        }
        let extra = renderer.update_buffers(
            &state.device,
            &state.queue,
            &mut encoder,
            &primitives,
            &screen,
        );
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 1.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            renderer.render(&mut pass, &primitives, &screen);
        }
        let row = width * 4;
        let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        state
            .queue
            .submit(extra.into_iter().chain([encoder.finish()]));
        drop(renderer);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| result.expect("map"));
        state
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
        pixels = buffer
            .slice(..)
            .get_mapped_range()
            .expect("mapped")
            .to_vec();
        for id in &output.textures_delta.free {
            state.renderer.write().free_texture(id);
        }
        output.textures_delta.clear();
    }
    // The controls: a row in the middle, Restart, Pause and Mute in that order, and Close under
    // them at the right.
    let at = |name: &str| {
        texts
            .iter()
            .find(|(text, _)| text == name)
            .unwrap_or_else(|| panic!("{name} is in the window: {texts:?}"))
            .1
    };
    let (restart, pause, mute, close) = (at("Restart"), at("Pause"), at("Mute"), at("Close"));
    assert!(restart.x < pause.x && pause.x < mute.x, "in order");
    assert!(
        (restart.y - pause.y).abs() < 1.0 && (pause.y - mute.y).abs() < 1.0,
        "one row"
    );
    assert!(
        ((restart.x + mute.x) / 2.0 - pause.x).abs() < 2.0,
        "the row is centered on its middle control"
    );
    assert!(
        close.y > pause.y && close.x > mute.x,
        "Close is below, at the right"
    );
    // The movie's red (in either channel order) is somewhere in the window.
    let red = pixels.as_chunks::<4>().0.iter().filter(|p| {
        let near = |a: u8, b: u8| a.abs_diff(b) < 12;
        (near(p[0], 220) && near(p[1], 30) && near(p[2], 30))
            || (near(p[2], 220) && near(p[1], 30) && near(p[0], 30))
    });
    // Every pixel of the 120 x 90 picture, and nothing else of that color: a texture that was
    // never drawn into (as when it was taken before the player resized its target) shows none.
    assert_eq!(
        red.count(),
        120 * 90,
        "the window did not show the movie's picture"
    );
}
