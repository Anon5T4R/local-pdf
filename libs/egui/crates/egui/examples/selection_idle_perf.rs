//! Paired CPU control for selection-response changes: 1,000 idle widgets and one focused field.
//! The runner builds this identical source with the original and repaired response policy.
//! Output is JSON on stdout. No window, GPU, or platform performance is measured.

use core::{hint::black_box, time::Duration};
use std::{io::Write as _, time::Instant};

use egui::{Context, Id, RawInput, Rect, TextEdit, pos2, vec2};

fn frame(
    ctx: &Context,
    labels: &[String],
    text: &mut String,
    frame_nr: u64,
) -> Result<(u128, u64, usize, usize), String> {
    let start = Instant::now();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, 1500.0))),
            time: Some(frame_nr as f64 / 60.0),
            ..Default::default()
        },
        |ui| {
            ui.add(TextEdit::singleline(text).id(Id::unique("focused-field")));
            egui::Grid::new("idle-widgets")
                .num_columns(20)
                .show(ui, |ui| {
                    for (i, label) in labels.iter().enumerate() {
                        let _ = black_box(ui.button(black_box(label)));
                        if i % 20 == 19 {
                            ui.end_row();
                        }
                    }
                });
        },
    );
    let primitives = ctx.tessellate(core::mem::take(&mut output.shapes), output.pixels_per_point);
    let elapsed = start.elapsed().as_nanos();
    // Fingerprinting and validation are outside the measured UI+tessellation interval.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }
    };
    feed(&(primitives.len() as u64).to_le_bytes());
    for primitive in &primitives {
        for v in [
            primitive.clip_rect.min.x,
            primitive.clip_rect.min.y,
            primitive.clip_rect.max.x,
            primitive.clip_rect.max.y,
        ] {
            feed(&v.to_bits().to_le_bytes());
        }
        match &primitive.primitive {
            egui::epaint::Primitive::Mesh(mesh) => {
                let (kind, id) = match mesh.texture_id {
                    egui::TextureId::Managed(id) => (0_u8, id),
                    egui::TextureId::User(id) => (1, id),
                };
                feed(&[kind]);
                feed(&id.to_le_bytes());
                feed(&(mesh.vertices.len() as u64).to_le_bytes());
                feed(&(mesh.indices.len() as u64).to_le_bytes());
                for v in &mesh.vertices {
                    for n in [v.pos.x, v.pos.y, v.uv.x, v.uv.y] {
                        feed(&n.to_bits().to_le_bytes());
                    }
                    feed(&v.color.to_array());
                }
                for i in &mesh.indices {
                    feed(&i.to_le_bytes());
                }
            }
            egui::epaint::Primitive::Callback(_) => {
                return Err("unexpected renderer callback".into());
            }
        }
    }
    output.textures_delta.clear();
    if !ctx.memory(|m| m.has_focus(Id::unique("focused-field"))) {
        return Err("fixture lost focus".into());
    }
    Ok((
        elapsed,
        hash,
        primitives.len(),
        output.platform_output.num_completed_passes,
    ))
}

fn main() -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("use --release".into());
    }
    let ctx = Context::default();
    ctx.all_styles_mut(|style| {
        style.animation_time = 0.0;
        style.visuals.text_cursor.blink = false;
    });
    let labels: Vec<_> = (0..1000).map(|i| format!("Widget {i}")).collect();
    let mut text = "selection performance fixture".to_owned();
    ctx.memory_mut(|m| m.request_focus(Id::unique("focused-field")));
    let warmup_start = Instant::now();
    let mut frame_nr = 0;
    while frame_nr < 50 || warmup_start.elapsed() < Duration::from_secs(3) {
        frame(&ctx, &labels, &mut text, frame_nr)?;
        frame_nr += 1;
    }
    let warmup_ns = warmup_start.elapsed().as_nanos();
    let warmups = frame_nr;
    let mut samples = Vec::with_capacity(1000);
    let mut expected = None;
    for _ in 0..1000 {
        let (ns, hash, primitives, passes) = frame(&ctx, &labels, &mut text, frame_nr)?;
        frame_nr += 1;
        if passes != 1 {
            return Err("measured fixture must render exactly one pass".into());
        }
        if let Some(identity) = expected {
            if identity != (hash, primitives) {
                return Err("measured frame geometry changed".into());
            }
        } else {
            expected = Some((hash, primitives));
        }
        samples.push(ns);
    }
    let (hash, primitives) = expected.ok_or("no measured frames")?;
    writeln!(
        std::io::stdout().lock(),
        "{{\"schema\":1,\"egui\":\"{}\",\"widgets\":1001,\"warmups\":{warmups},\"warmup_ns\":{warmup_ns},\"samples_ns\":{samples:?},\"frame_mesh_fnv64\":\"{hash:016x}\",\"primitives\":{primitives},\"passes_per_frame\":1}}",
        env!("CARGO_PKG_VERSION")
    ).map_err(|error| error.to_string())?;
    Ok(())
}
