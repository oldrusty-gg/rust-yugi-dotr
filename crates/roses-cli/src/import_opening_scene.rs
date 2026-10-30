//! `roses import-opening-scene`: derive the opening's Simon scene (event 10040)
//! from the user's ISO alone and write it in the `roses-private-captured-rh0-
//! scene-1` JSON schema that the Bevy prototype loads, plus the model textures
//! as PNG. `--verify-against <mesh-scene-simon.json>` compares every field
//! with an emulator capture and reports the differences.
use crate::{emit_json, open_pal, write_png, Result};
use roses_assets::opening_scene::{
    DrawMesh, Qword, SimonPose, SimonScene, WorldModel, MODEL_ARCHIVE_ISO_PATH, PROVENANCE_KIND,
    SCENE_PROFILE,
};
use roses_assets::RgbaImage;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Which world pack to derive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    /// The opening's Stonehenge (new-game load 0x1E91B0(3, 4) at 0x21C0BC).
    Opening,
    /// The Stonehenge story events' "scene 0" (0x1E91B0(3, 0) at 0x220888,
    /// event 11160; 11180 loads the same pack at 0x220FF0).
    StoryScene0,
}
impl World {
    fn call(self) -> (u32, u32) {
        match self {
            World::Opening => (0x21c0bc, 0x21c0c0),
            World::StoryScene0 => (0x220888, 0x22088c),
        }
    }
    fn name(self) -> &'static str {
        match self {
            World::Opening => "simon",
            World::StoryScene0 => "stonehenge-scene-0",
        }
    }
}

pub struct Options {
    pub world: World,
    pub iso: PathBuf,
    pub output: PathBuf,
    pub pose: SimonPose,
    pub verify_against: Option<PathBuf>,
    /// Root that the capture's texture paths are relative to (default: cwd).
    pub capture_root: PathBuf,
}

pub fn default_pose() -> SimonPose {
    SimonPose::default()
}

fn texture_name(slot: u16, index: usize) -> String {
    format!("textures/slot{slot}-tex{index}.png")
}

/// The exporter's `clip_from_source` for view-space draws: `P · c` in f64 with
/// left-to-right summation (`sum(v[i] * m[i][j] for i)`).
fn clip_columns(projection: &[Qword; 4], emitted: &[Qword; 4], view_space: bool) -> Vec<Vec<f64>> {
    emitted
        .iter()
        .map(|c| {
            if view_space {
                (0..4)
                    .map(|j| {
                        (0..4).fold(0.0f64, |acc, i| {
                            acc + f64::from(c[i]) * f64::from(projection[i][j])
                        })
                    })
                    .collect()
            } else {
                c.iter().map(|v| f64::from(*v)).collect()
            }
        })
        .collect()
}

fn mesh_json(
    scene: &SimonScene,
    model: &WorldModel,
    mesh: &DrawMesh,
    texture: (&str, &str),
) -> Value {
    let columns =
        |m: &[Qword; 4]| -> Value { json!(m.iter().map(|q| q.to_vec()).collect::<Vec<_>>()) };
    let mut value = json!({
        "object_slot": mesh.object_slot,
        "draw_index": mesh.draw_index,
        "source_record_offset": model.archive_offset,
        "source_record_sha256": model.record_sha256,
        "source_geometry_sha256": model.geometry_sha256,
        "source_geometry_equals_saved_ee": Value::Null,
        "emitted_matrix_address": Value::Null,
        "emitted_matrix_sha256": model.emitted_matrix_sha256,
        "emitted_matrix_space": if model.view_space { "view" } else { "clip_gs" },
        "emitted_matrix_columns": columns(&model.emitted_matrix),
        "clip_from_source_gs_columns": clip_columns(&scene.camera.projection, &model.emitted_matrix, model.view_space),
        "normal_from_source_columns": model.normal_rows.iter().map(|q| q.to_vec()).collect::<Vec<_>>(),
        "vu_program_entry": model.vu_program_entry,
        "fog_vif83": scene.lighting.draw_fog_vif83,
        "fog_packet_address": Value::Null,
        "texture_path": texture.0,
        "texture_sha256": texture.1,
        "gs_registers_raw": mesh.gs_registers_raw,
        "sampler": {
            "u": if mesh.sampler_u_clamp { "clamp" } else { "repeat" },
            "v": if mesh.sampler_v_clamp { "clamp" } else { "repeat" },
            "magnification": if mesh.magnification_linear { "linear" } else { "nearest" },
            "minification_code": mesh.minification_code,
        },
        "material_vectors_raw": mesh.material_vectors_raw,
        "material_vectors_emitted": mesh.material_vectors_emitted,
        "positions_raw": mesh.positions_raw,
        "normals_raw": mesh.normals_raw,
        "vertex_rgba_gs": mesh.vertex_rgba_gs,
        "uv": mesh.uv,
        "indices": mesh.indices,
    });
    // Only present on blended (second DMA chain) draws, so the opening's
    // scene.json stays byte-identical.
    if mesh.blend {
        value["alpha_blend"] = json!(true);
    }
    value
}

/// The animated models' draw matrices per animation frame (the opening's sky,
/// slot 37): `clip_from_source_gs_columns` of its draws at frames 1 ..=
/// frame_count − 1, which the renderer steps through once per story update
/// (`0x1EE6E0`: +1 per update from 1.0 at the object's creation, restart at
/// 1.0 after the last frame).
fn animations_json(scene: &SimonScene) -> Result<Value> {
    let animated = scene.animation_matrices()?;
    Ok(Value::Array(
        animated
            .iter()
            .map(|a| {
                json!({
                    "object_slot": a.slot,
                    "frame_count": a.frame_count,
                    "first_frame": 1,
                    "clip_from_source_gs_columns_by_frame": a
                        .frames
                        .iter()
                        .map(|(m, view)| clip_columns(&scene.camera.projection, m, *view))
                        .collect::<Vec<_>>(),
                })
            })
            .collect(),
    ))
}

fn scene_json(scene: &SimonScene, name: &str, source: Value, meshes: Vec<Value>) -> Value {
    let c = &scene.constants;
    let half = c.viewport_offset as u32;
    let centre = c.gs_center as u32;
    let viewport = c.viewport as u32;
    json!({
        "schema_version": 1,
        "profile": SCENE_PROFILE,
        "scene": name,
        "provenance": source,
        "matrix_layout": "column_major_columns",
        "raw_canvas": [viewport, viewport],
        "display_aspect": [4, 3],
        "gs_projection_columns": scene.camera.projection.iter().map(|q| q.to_vec()).collect::<Vec<_>>(),
        "gs_projection_center": [centre, centre],
        "gs_projection_half_extent": [half, half],
        "gs_xyoffset": [centre - half, centre - half],
        "gs_scissor_inclusive": [0, viewport - 1, 0, viewport - 1],
        "fog": {
            "near": scene.lighting.fog_near,
            "far": scene.lighting.fog_far,
            "color": scene.lighting.fog_colour,
            "vu_qword83": scene.lighting.fog_qword83,
        },
        "light_from_view_normal_columns": scene.lighting.light_rows.iter().map(|q| q.to_vec()).collect::<Vec<_>>(),
        "meshes": meshes,
        "native": {
            "pose": scene.pose,
            "pack": scene.pack,
            "camera": scene.camera,
            "lighting": scene.lighting,
            "models": scene.models,
        },
        "limitations": [
            "Static frame: the pan byte, camera level and the sky animation counter are import parameters (defaults are the settled dialogue view with sky frame 3.0, the captured state). The sky counter's per-update rule is traced (0x1EE6E0: +speed*step per story update, restart at 1.0 after frame 500; roses_assets::opening_scene::advance_animation_frame) and `animations` lists the sky's draw matrix for every frame, but the number of updates between the scene load and any given moment is not replayed, so the counter's phase at a moment is not derived here; the dialogue camera pan and the entry tilt are likewise parameters",
            "Vertex colours use the VU1 main-path model of the earlier capture research (ambient plus clamped diffuse, integer truncation), evaluated in f64; the VU1 microcode's own arithmetic is not emulated",
            "Float arithmetic follows PCSX2 v2.8.2 for this game (EE round-to-+inf with add/sub alignment, nearest division, VU0 macro in the EE mode); real hardware rounding differs and is not modelled",
            "Packet addresses of the emulator capture (emitted_matrix_address, fog_packet_address) have no disc equivalent and are null; source_geometry_equals_saved_ee is null because no saved memory image is involved",
            "Historical separate cloud effect not represented by these eight meshes",
        ],
    })
}

pub fn run(options: &Options) -> Result<i32> {
    if options.output.exists() {
        return Err(
            "opening scene output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    let (mut disc, report) = open_pal(&options.iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let archive = disc.read_file(MODEL_ARCHIVE_ISO_PATH, 32 * 1024 * 1024)?;
    eprintln!(
        "Deriving the {} scene from the executable and MODEL.MRG…",
        options.world.name()
    );
    let scene =
        SimonScene::derive_world(&executable, &archive, options.pose, options.world.call())?;
    if let Some(parent) = options
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&options.output)?;
    fs::create_dir(options.output.join("textures"))?;
    // Textures: one PNG per (slot, model-local texture index) used by a draw.
    let mut textures: BTreeMap<(u16, usize), (String, String, RgbaImage)> = BTreeMap::new();
    for mesh in &scene.meshes {
        let key = (mesh.object_slot, mesh.texture_index);
        if textures.contains_key(&key) {
            continue;
        }
        let (_, image) = scene.texture(&archive, key.0, key.1)?;
        let name = texture_name(key.0, key.1);
        write_png(
            &options.output.join(&name),
            image.width,
            image.height,
            &image.pixels,
        )?;
        let bytes = fs::read(options.output.join(&name))?;
        textures.insert(key, (name, format!("{:x}", Sha256::digest(&bytes)), image));
    }
    let meshes: Vec<Value> = scene
        .meshes
        .iter()
        .map(|mesh| {
            let model = scene
                .models
                .iter()
                .find(|m| m.slot == mesh.object_slot)
                .expect("model");
            let (name, sha, _) = &textures[&(mesh.object_slot, mesh.texture_index)];
            mesh_json(&scene, model, mesh, (name, sha))
        })
        .collect();
    let source = json!({
        "kind": PROVENANCE_KIND,
        "iso_sha256": report.iso_sha256,
        "executable_sha256": format!("{:x}", Sha256::digest(&executable)),
        "model_archive_sha256": format!("{:x}", Sha256::digest(&archive)),
        "model_archive_iso_path": MODEL_ARCHIVE_ISO_PATH,
        "importer_version": env!("CARGO_PKG_VERSION"),
        "texture_encoding": "RGBA PNG from the RH2 record's 256-entry palette (alpha 128 → 255), the same decoder as the opening texture importer",
    });
    let mut document = scene_json(&scene, options.world.name(), source, meshes);
    let animations = animations_json(&scene)?;
    if animations.as_array().is_some_and(|a| !a.is_empty()) {
        document["animations"] = animations;
    }
    emit_json(&document, Some(&options.output.join("scene.json")))?;
    emit_json(&report, Some(&options.output.join("disc.json")))?;
    eprintln!(
        "Derived {} draws of {} world models (pack {} at MODEL.MRG {:#x}) into {}",
        scene.meshes.len(),
        scene.models.len(),
        scene.pack.pack_index,
        scene.pack.archive_offset,
        options.output.display()
    );
    let Some(capture) = &options.verify_against else {
        return Ok(0);
    };
    let captured: Value = serde_json::from_slice(&fs::read(capture)?)?;
    let differences = verify(&document, &captured, &textures, &options.capture_root)?;
    Ok(if differences == 0 { 0 } else { 1 })
}

fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn same_number_bits(a: &Value, b: &Value) -> bool {
    match (as_f64(a), as_f64(b)) {
        (Some(x), Some(y)) => (x as f32).to_bits() == (y as f32).to_bits() && x == y,
        _ => a == b,
    }
}

/// Deep comparison where numbers must be bit-identical (both as f64 and after
/// conversion to f32) and everything else equal; returns the differing paths.
fn diff(path: &str, a: &Value, b: &Value, out: &mut Vec<String>, limit: usize) {
    match (a, b) {
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!("{path}: length {} vs {}", x.len(), y.len()));
                return;
            }
            let mut count = 0;
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                let before = out.len();
                diff(&format!("{path}[{i}]"), p, q, out, limit);
                if out.len() > before {
                    count += 1;
                    if count >= limit {
                        out.push(format!("{path}: further differences elided"));
                        return;
                    }
                }
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for (k, p) in x {
                match y.get(k) {
                    Some(q) => diff(&format!("{path}.{k}"), p, q, out, limit),
                    None => out.push(format!("{path}.{k}: missing in capture")),
                }
            }
        }
        _ => {
            if !same_number_bits(a, b) {
                out.push(format!("{path}: {a} vs {b}"));
            }
        }
    }
}

fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0u8; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buffer)?;
    let pixels = buffer[..info.buffer_size()].to_vec();
    let pixels = match info.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        other => return Err(format!("unsupported capture PNG colour type {other:?}").into()),
    };
    Ok((info.width, info.height, pixels))
}

fn verify(
    derived: &Value,
    captured: &Value,
    textures: &BTreeMap<(u16, usize), (String, String, RgbaImage)>,
    capture_root: &Path,
) -> Result<usize> {
    let mut report: Vec<String> = Vec::new();
    let mut exact: Vec<String> = Vec::new();
    fn check(name: &str, a: &Value, b: &Value, exact: &mut Vec<String>, report: &mut Vec<String>) {
        let mut out = Vec::new();
        diff(name, a, b, &mut out, 4);
        if out.is_empty() {
            exact.push(name.to_string());
        } else {
            report.extend(out);
        }
    }
    for key in [
        "schema_version",
        "profile",
        "scene",
        "matrix_layout",
        "raw_canvas",
        "display_aspect",
        "gs_projection_columns",
        "gs_projection_center",
        "gs_projection_half_extent",
        "gs_xyoffset",
        "gs_scissor_inclusive",
        "fog",
        "light_from_view_normal_columns",
    ] {
        check(key, &derived[key], &captured[key], &mut exact, &mut report);
    }
    let captured_meshes: Map<String, Value> = captured["meshes"]
        .as_array()
        .ok_or("capture has no meshes")?
        .iter()
        .map(|m| {
            (
                format!("{}/{}", m["object_slot"], m["draw_index"]),
                m.clone(),
            )
        })
        .collect();
    let derived_meshes = derived["meshes"]
        .as_array()
        .ok_or("derived scene has no meshes")?;
    if derived_meshes.len() != captured_meshes.len() {
        report.push(format!(
            "meshes: {} derived vs {} captured",
            derived_meshes.len(),
            captured_meshes.len()
        ));
    }
    let mut texture_checked: BTreeMap<String, bool> = BTreeMap::new();
    for mesh in derived_meshes {
        let key = format!("{}/{}", mesh["object_slot"], mesh["draw_index"]);
        let Some(cap) = captured_meshes.get(&key) else {
            report.push(format!("mesh {key}: not in capture"));
            continue;
        };
        for field in [
            "source_record_offset",
            "source_record_sha256",
            "source_geometry_sha256",
            "emitted_matrix_sha256",
            "emitted_matrix_space",
            "emitted_matrix_columns",
            "clip_from_source_gs_columns",
            "normal_from_source_columns",
            "vu_program_entry",
            "fog_vif83",
            "gs_registers_raw",
            "sampler",
            "material_vectors_raw",
            "material_vectors_emitted",
            "positions_raw",
            "normals_raw",
            "vertex_rgba_gs",
            "uv",
            "indices",
        ] {
            check(
                &format!("mesh {key}.{field}"),
                &mesh[field],
                &cap[field],
                &mut exact,
                &mut report,
            );
        }
        // Texture pixels: decode the capture's PNG and compare with the
        // freshly decoded RH2 record.
        let slot = mesh["object_slot"].as_u64().unwrap_or(0) as u16;
        let index = textures
            .iter()
            .find(|(_, (name, _, _))| Some(name.as_str()) == mesh["texture_path"].as_str())
            .map(|(k, _)| k.1)
            .unwrap_or(usize::MAX);
        let cap_path = cap["texture_path"].as_str().unwrap_or("");
        let tag = format!("{cap_path}<-{}", texture_name(slot, index));
        if !texture_checked.contains_key(&tag) {
            let ok = match (
                textures.get(&(slot, index)),
                read_png(&capture_root.join(cap_path)),
            ) {
                (Some((_, _, image)), Ok((w, h, pixels))) => {
                    w == image.width && h == image.height && pixels == image.pixels
                }
                (_, Err(e)) => {
                    report.push(format!("texture {cap_path}: {e}"));
                    false
                }
                _ => false,
            };
            texture_checked.insert(tag.clone(), ok);
            if ok {
                exact.push(format!("texture pixels {tag}"));
            } else {
                report.push(format!("texture pixels differ or unreadable: {tag}"));
            }
        }
    }
    println!(
        "verification against {}",
        captured["provenance"]["state_sha256"]
    );
    println!("  exact fields: {}", exact.len());
    for e in &exact {
        println!("    = {e}");
    }
    println!("  differences: {}", report.len());
    for d in &report {
        println!("    ! {d}");
    }
    Ok(report.len())
}
