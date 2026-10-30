use super::*;

fn editor() -> MapEditor {
    let cells: [u8; 49] = std::array::from_fn(|i| ((7 * (i % 7) + 1) % 10) as u8);
    MapEditor::new(cells, 3, 5, 0, Pulse::new(0, true), Pulse::new(0, true))
}

fn held(ed: &mut MapEditor, bits: u16, n: usize) -> Vec<EditorEvent> {
    let mut all = Vec::new();
    let mut prev = 0;
    for _ in 0..n {
        let p = EditorPad { pressed: bits & !prev, repeat: bits & !prev, held: bits };
        prev = bits;
        all.extend(ed.update(p));
    }
    all
}

#[test]
fn init_puts_the_cursor_in_the_middle() {
    let ed = editor();
    assert_eq!((ed.cursor, ed.delay, ed.diagonal, ed.d63), ((3, 3), 0, false, 0));
    assert_eq!(ed.map(), 0);
    // A non-zero param resets the own-map list to (3, 5) [F 0x1CD864].
    let other = MapEditor::new([7; 49], 0, 1, 1, Pulse::new(0, true), Pulse::new(0, true));
    assert_eq!((other.own_top, other.own_count), (3, 5));
}

#[test]
fn a_held_direction_steps_every_eight_updates() {
    let mut ed = editor();
    let ev = held(&mut ed, pad::RIGHT, 9);
    // [emu m1-basic] moves on the first and the ninth update.
    assert_eq!(ed.cursor, (5, 3));
    assert_eq!(ev.iter().filter(|e| **e == EditorEvent::Sfx(SFX_CURSOR)).count(), 2);
    assert_eq!(ed.d63, HELD_MARK);
    held(&mut ed, pad::RIGHT, 20);
    assert_eq!(ed.cursor.0, 6);
}

#[test]
fn a_held_diagonal_alternates_the_axes() {
    let mut ed = editor();
    held(&mut ed, pad::LEFT | pad::DOWN, 1);
    assert_eq!((ed.cursor, ed.diagonal), ((2, 3), true));
    held(&mut ed, 0, 8);
    held(&mut ed, pad::LEFT | pad::DOWN, 1);
    assert_eq!((ed.cursor, ed.diagonal), ((2, 4), false));
}

#[test]
fn edges_block_their_axis_and_left_beats_right() {
    let mut ed = editor();
    ed.cursor = (0, 0);
    let ev = held(&mut ed, pad::LEFT | pad::UP, 1);
    assert!(ev.is_empty());
    assert_eq!(ed.d63, HELD_MARK, "a blocked direction still marks d63");
    held(&mut ed, 0, 8);
    held(&mut ed, pad::LEFT | pad::RIGHT, 1);
    assert_eq!(ed.cursor, (0, 0), "left at the edge drops right too");
    ed.cursor = (3, 3);
    ed.delay = 0;
    held(&mut ed, pad::LEFT | pad::RIGHT, 1);
    assert_eq!(ed.cursor, (2, 3));
}

#[test]
fn l1_r1_cycle_the_terrain_and_save_writes_the_chosen_map() {
    let mut ed = editor();
    let t = ed.terrain();
    held(&mut ed, pad::L1, 1);
    assert_eq!(ed.terrain(), (t + 9) % 10);
    held(&mut ed, 0, 1);
    held(&mut ed, pad::R1, 1);
    held(&mut ed, 0, 1);
    held(&mut ed, pad::R1, 1);
    assert_eq!(ed.terrain(), (t + 1) % 10);
    let ev = held(&mut ed, pad::CROSS, 1);
    match &ev[..] {
        [EditorEvent::Save { map: 0, cells }, EditorEvent::Sfx(SFX_CONFIRM)] => assert_eq!(*cells, ed.cells),
        other => panic!("{other:?}"),
    }
    let ev = held(&mut ed, pad::CIRCLE, 1);
    assert_eq!(ev, vec![EditorEvent::Sfx(SFX_CANCEL), EditorEvent::Cancel]);
}

#[test]
fn pulses_bounce_and_d63_holds_p1_at_the_top() {
    let mut p = Pulse::new(0x7F00, true);
    p.step(P1_STEP);
    assert_eq!((p.value, p.rising()), (0x8000, false));
    p.step(P1_STEP);
    assert_eq!((p.value, p.rising()), (0x7E00, false));
    let mut ed = editor();
    ed.p1 = Pulse::new(0x7E00, true);
    ed.d63 = HELD_MARK;
    for _ in 0..6 {
        ed.update(EditorPad::default());
        assert!(ed.p1.value >= 0x7E00);
    }
    assert_eq!(ed.p1.value, 0x8000);
}

#[test]
fn the_cross_eases_in() {
    let mut ed = editor();
    ed.draw_overlay([0, 0, 0x60, 0]);
    let i = 3 * 7 + 3;
    assert_eq!(ed.overlay[i].target, [0, 0, 0x60, 0x40]);
    assert_eq!(ed.overlay[i].fill, [0, 0, 0, 0], "a new mode waits one update");
    ed.draw_overlay([0, 0, 0x60, 0]);
    assert_eq!(ed.overlay[i].fill, [0, 0, 8, 8]);
    assert_eq!(ed.overlay[0].mode, 0);
}

/// One interpreter frame of `tools/run_me.py`.
#[derive(serde::Deserialize)]
struct Frame {
    pad: u16,
    ret: serde_json::Value,
    a9: Vec<u8>,
    p1: u16,
    p2: u16,
    d63: u8,
    d64: u8,
    cells: Vec<Vec<u8>>,
    own: Vec<u8>,
}

#[derive(serde::Deserialize)]
struct Capture {
    frames: Vec<Frame>,
    log: Vec<serde_json::Value>,
    cam: Vec<serde_json::Value>,
}

/// The model against the game's own code in the EE interpreter
/// (`ROSES_MAP_EDITOR_CAPTURES` = modes/map-editor/captures, files `m*.json`
/// written by `tools/run_me.py`): after every update and draw the cursor,
/// move delay, diagonal bit, both pulses and their directions, d63, the 49
/// terrains, every cell's overlay bytes, the SE ids, the pop and the saved
/// own maps.
#[test]
fn private_editor_matches_the_emulated_original() {
    let Some(dir) = std::env::var_os("ROSES_MAP_EDITOR_CAPTURES") else {
        return;
    };
    let mut checked = 0;
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('m') && n.ends_with(".json")))
        .collect();
    files.sort();
    for path in files {
        let name = path.display().to_string();
        let c: Capture = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let colour: Vec<u8> = c.cam[2].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect();
        let colour = [colour[0], colour[1], colour[2], colour[3]];
        let init = c.frames.iter().position(|f| f.ret == serde_json::json!("init")).unwrap();
        let f0 = &c.frames[init];
        let cells: [u8; 49] = std::array::from_fn(|i| f0.cells[i][16]);
        let pulse = |v: u16, bits: u8| Pulse { value: v, dir: bits & 3 };
        let mut ed = MapEditor::new(cells, f0.a9[0x8C], f0.a9[0x8D], 0, pulse(f0.p1, f0.d64), pulse(f0.p2, f0.d64 >> 2));
        for (k, cell) in ed.overlay.iter_mut().enumerate() {
            let b = &f0.cells[k];
            *cell = CellOverlay {
                target: [b[0], b[1], b[2], b[3]],
                fill: [b[4], b[5], b[6], b[7]],
                outline_target: [b[8], b[9], b[10], b[11]],
                outline: [b[12], b[13], b[14], b[15]],
                mode: b[17],
                latched: b[18],
            };
        }
        let mut prev = 0u16;
        let mut saved = f0.own.clone();
        for (n, f) in c.frames.iter().enumerate().skip(init + 1) {
            let p = EditorPad { pressed: f.pad & !prev, repeat: f.pad & !prev, held: f.pad };
            prev = f.pad;
            let ev = ed.update(p);
            for e in &ev {
                if let EditorEvent::Save { map, cells } = e {
                    saved[49 * map..49 * map + 49].copy_from_slice(cells);
                }
            }
            ed.draw_overlay(colour);
            let at = format!("{name} frame {n} pad {:#x}", f.pad);
            assert_eq!((ed.cursor.0, ed.cursor.1, ed.delay), (f.a9[0x8F], f.a9[0x90], f.a9[0x91]), "{at} cursor/delay");
            assert_eq!(u8::from(ed.diagonal), f.a9[0x92] & 1, "{at} diagonal");
            assert_eq!((ed.p1.value, ed.p2.value, ed.d63), (f.p1, f.p2, f.d63), "{at} pulses");
            assert_eq!(ed.p1.dir | (ed.p2.dir << 2), f.d64 & 0xF, "{at} directions");
            for i in 0..49 {
                let b = &f.cells[i];
                let o = &ed.overlay[i];
                assert_eq!(ed.cells[i], b[16], "{at} terrain {i}");
                assert_eq!(
                    (o.target, o.fill, o.outline_target, o.outline, o.mode, o.latched),
                    ([b[0], b[1], b[2], b[3]], [b[4], b[5], b[6], b[7]], [b[8], b[9], b[10], b[11]], [b[12], b[13], b[14], b[15]], b[17], b[18]),
                    "{at} overlay {i}"
                );
            }
            assert_eq!(saved, f.own, "{at} own maps");
            checked += 1;
        }
        // SE ids and the pop, in order (the log's editor part follows the
        // mode push).
        let push = c.log.iter().position(|e| e[1] == "mode_push").unwrap();
        let logged: Vec<String> = c.log[push + 1..]
            .iter()
            .filter(|e| e[1] == "sfx" || e[1] == "mode_pop")
            .map(|e| if e[1] == "sfx" { format!("se{}", e[4]) } else { "pop".into() })
            .collect();
        let mut ed2 = MapEditor::new(cells, f0.a9[0x8C], f0.a9[0x8D], 0, pulse(f0.p1, f0.d64), pulse(f0.p2, f0.d64 >> 2));
        let mut prev = 0u16;
        let mut mine = Vec::new();
        for f in c.frames.iter().skip(init + 1) {
            let p = EditorPad { pressed: f.pad & !prev, repeat: f.pad & !prev, held: f.pad };
            prev = f.pad;
            for e in ed2.update(p) {
                match e {
                    EditorEvent::Sfx(id) => mine.push(format!("se{id}")),
                    EditorEvent::Save { .. } => {}
                    EditorEvent::Cancel => mine.push("pop".into()),
                }
                if mine.last().is_some_and(|s| s == "se4") {
                    mine.push("pop".into());
                }
            }
        }
        assert_eq!(mine, logged, "{name} SE and pop");
    }
    eprintln!("{checked} frames match");
    assert!(checked > 20);
}
