//! The map editor's 2D layer (mode 0xD draw 0x1CDE40's HUD part, built by
//! `roses_client::map_editor::hud_calls`) rendered from the user's
//! TEXSYS.MRG atlases, message font and the board import's terrain models
//! (the terrain picture 0x1B4370). Bevy-free; the scene is
//! `map_editor_render.rs` in the binary. Notes:
//! the project notes.
use crate::duel_hud_picture::TerrainPicture;
use roses_client::duel_hud::{self, HudTables, HudTextures, MenuCall, YesNoTables};
use roses_client::map_editor::{EditorTables, MapEditor};

/// The layer's side (the GS frame the HUD layer covers).
pub const LAYER: usize = 512;

/// The editor's 2D layer: its calls as primitives (premultiplied RGBA,
/// 512 × 512), the terrain model rendered into the render target first;
/// returns the layer and what could not be drawn.
#[allow(clippy::too_many_arguments)]
pub fn render_layer(
    editor: &MapEditor,
    data: &EditorTables,
    tables: &HudTables,
    yes_no: &YesNoTables,
    textures: &mut HudTextures,
    picture: Option<&mut TerrainPicture>,
    forward: [f64; 3],
) -> (Vec<u8>, Vec<String>) {
    let calls = roses_client::map_editor::hud_calls(editor, data);
    let frame = duel_hud::menu_frame(tables, yes_no, &calls);
    let mut notes = Vec::new();
    match picture {
        Some(p) => {
            textures.render_target = p.render(editor.terrain(), forward, tables.dimx);
            p.advance();
        }
        None => notes.push("terrain picture (no board import attached)".to_string()),
    }
    for u in &frame.unsupported {
        if !matches!(u.kind, duel_hud::UnsupportedKind::TerrainTileRender { .. }) {
            notes.push(format!("unsupported {:?}", u.kind));
        }
    }
    let (layer, skipped) = duel_hud::render_layer(&frame, tables, textures, LAYER, LAYER);
    for s in skipped {
        notes.push(format!("primitive {} skipped: {}", s.index, s.reason));
    }
    (layer, notes)
}

/// The calls as both the port and the interpreter log name them.
pub fn call_key(c: &MenuCall, tables: &HudTables) -> Vec<(String, Vec<i64>)> {
    let v = |xs: &[i32]| xs.iter().map(|&x| i64::from(x)).collect::<Vec<_>>();
    match c {
        MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, .. } => vec![("panel".into(), v(&[*grad, *deco, *ptype as i32, *x, *y, *wu, *hu]))],
        MenuCall::Sprite { desc, x, y } => vec![("sprite".into(), v(&[*desc as i32, *x, *y, -1]))],
        MenuCall::SpriteOverlay { desc, x, y, mode, .. } => vec![("sprite".into(), v(&[*desc as i32, *x, *y, i32::from(*mode)]))],
        MenuCall::Number { x, y, size, flags, digits, value, .. } => {
            vec![("number".into(), v(&[*x, *y, *size as i32, *flags as i32, *digits, *value]))]
        }
        MenuCall::Quad { corners } => vec![("f206f90".into(), vec![corners[0].0 as i64, corners[0].1 as i64, corners[3].0 as i64, corners[3].1 as i64])],
        MenuCall::TerrainPicture { colour, terrain, x, y } => vec![("terrainpic".into(), v(&[*colour as i32, *terrain, *x, *y]))],
        MenuCall::Legend { side, x, y, .. } => vec![("legend".into(), v(&[i32::from(*side), *x, *y]))],
        // 0x1B4220's calls (the editor inlines them, 0x1CDFE4..0x1CE0C4).
        MenuCall::TerrainBox { terrain, x, y } => {
            let w = tables.message((*terrain + 72) as u16).map_or(0, |m| duel_hud::TextCanvas::measure(m).1 as i32);
            let sc = if w >= 85 { 84.0 / w as f32 } else { 1.0 };
            vec![
                ("panel".into(), v(&[-1, 2, 3, *x, *y, 9, 18])),
                ("panel".into(), v(&[5, -1, 3, *x, *y + 64, 9, 2])),
                ("text".into(), v(&[*x + 48 - ((w >> 1) as f32 * sc) as i32, *y + 69])),
            ]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_client::map_editor::{CellOverlay, EditorPad, Pulse};

    fn logged(c: &serde_json::Value) -> Option<(String, Vec<i64>)> {
        let ra = i64::from_str_radix(c["ra"].as_str()?.trim_start_matches("0x"), 16).ok()?;
        if !(0x1C_DE10..0x1C_E7F0).contains(&ra) {
            return None;
        }
        let a: Vec<i64> = c["args"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let st: Vec<i64> = c["stack"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let f = c["fn"].as_str()?;
        Some(match f {
            "panel" => (f.into(), a[1..8].to_vec()),
            "sprite" => (f.into(), vec![(a[1] - 0x28_F300) / 10, a[2] - 0x700, a[3] - 0x700, st[8]]),
            "number" => (f.into(), a[1..7].to_vec()),
            "text" => (f.into(), a[2..4].to_vec()),
            "terrainpic" => (f.into(), a[1..5].to_vec()),
            "legend" => (f.into(), a[1..4].to_vec()),
            "f206f90" => {
                let hex = c["packet"].as_str()?;
                let b: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect();
                let h = |o: usize| i64::from(u16::from_le_bytes([b[o], b[o + 1]])) / 16 - 0x700;
                (f.into(), vec![h(0x18), h(0x1A), h(0x48), h(0x4A)])
            }
            _ => return None,
        })
    }

    /// The editor's 2D calls against the interpreter's draw logs
    /// (`ROSES_MAP_EDITOR_CAPTURES` = modes/map-editor/captures, `m*.json`;
    /// `ROSES_DUEL_IMPORT` = a duel import): the terrain picture, the
    /// inlined terrain box, the legend and every call of the counts panel
    /// 0x1CE130 (icons with the cursor terrain's overlay mode, gauge quads,
    /// counts), per update, in order.
    #[test]
    fn private_hud_matches_the_emulated_draw() {
        let (Some(dir), Some(duel)) = (std::env::var_os("ROSES_MAP_EDITOR_CAPTURES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
            return;
        };
        let data = crate::duel_play::load(std::path::Path::new(&duel)).unwrap();
        let mut codes = std::collections::BTreeMap::new();
        for chunk in HudTables::required_messages().chunks(256) {
            for m in roses_assets::parse_pal_messages(&data.messages, chunk).unwrap().messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        let tables = HudTables::from_executable(&data.executable, codes).unwrap();
        let ed_tables = EditorTables::from_executable(&data.executable).unwrap();
        let mut checked = 0;
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('m') && n.ends_with(".json")))
            .collect();
        files.sort();
        for path in files {
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let frames = v["frames"].as_array().unwrap();
            let init = frames.iter().position(|f| f["ret"] == "init").unwrap();
            let f0 = &frames[init];
            let byte = |f: &serde_json::Value, k: &str, i: usize| f[k][i].as_u64().unwrap() as u8;
            let cell = |f: &serde_json::Value, i: usize, k: usize| f["cells"][i][k].as_u64().unwrap() as u8;
            let cells: [u8; 49] = std::array::from_fn(|i| cell(f0, i, 16));
            let d64 = f0["d64"].as_u64().unwrap() as u8;
            let pulse = |k: &str, bits: u8| Pulse { value: f0[k].as_u64().unwrap() as u16, dir: bits & 3 };
            let mut ed = MapEditor::new(cells, byte(f0, "a9", 0x8C), byte(f0, "a9", 0x8D), 0, pulse("p1", d64), pulse("p2", d64 >> 2));
            for (i, o) in ed.overlay.iter_mut().enumerate() {
                let b: Vec<u8> = (0..19).map(|k| cell(f0, i, k)).collect();
                *o = CellOverlay {
                    target: [b[0], b[1], b[2], b[3]],
                    fill: [b[4], b[5], b[6], b[7]],
                    outline_target: [b[8], b[9], b[10], b[11]],
                    outline: [b[12], b[13], b[14], b[15]],
                    mode: b[17],
                    latched: b[18],
                };
            }
            let calls = v["calls"].as_array().unwrap();
            let mut prev = 0u16;
            for f in frames.iter().skip(init + 1) {
                let pad = f["pad"].as_u64().unwrap() as u16;
                ed.update(EditorPad { pressed: pad & !prev, repeat: pad & !prev, held: pad });
                prev = pad;
                ed.draw_overlay(ed_tables.cross);
                let frame_no = f["frame"].as_i64().unwrap() - 1;
                let theirs: Vec<_> = calls.iter().filter(|c| c["frame"].as_i64() == Some(frame_no)).filter_map(logged).collect();
                let ours: Vec<_> = roses_client::map_editor::hud_calls(&ed, &ed_tables).iter().flat_map(|c| call_key(c, &tables)).collect();
                for (i, (a, b)) in ours.iter().zip(theirs.iter()).enumerate() {
                    assert_eq!(a, b, "{} frame {frame_no} call {i}", path.display());
                }
                assert_eq!(ours.len(), theirs.len(), "{} frame {frame_no} call count", path.display());
                checked += 1;
            }
        }
        eprintln!("{checked} editor frames match");
        assert!(checked > 20);
    }
}
