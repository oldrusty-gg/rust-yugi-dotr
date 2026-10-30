use super::*;

/// Card blocks as the menu sees them (current decks, playable decks, the
/// both-routes mark).
#[derive(Clone, Debug)]
struct Fake {
    current: [u8; 2],
    playable: [[bool; 3]; 2],
    maps: [bool; 2],
}

impl SetupBlocks for Fake {
    fn current_deck(&self, side: u8) -> u8 {
        self.current[usize::from(side)]
    }
    fn set_current_deck(&mut self, side: u8, deck: u8) {
        self.current[usize::from(side)] = deck;
    }
    fn deck_playable(&self, side: u8, deck: u8) -> bool {
        self.playable[usize::from(side)][usize::from(deck)]
    }
    fn has_own_maps(&self, side: u8) -> bool {
        self.maps[usize::from(side)]
    }
}

fn fake() -> Fake {
    Fake { current: [0, 0], playable: [[true, false, false]; 2], maps: [false; 2] }
}

fn press(m: &mut CustomDuel, b: &mut Fake, bits: u16) -> Vec<MenuEvent> {
    m.update([MenuPad::press(bits), MenuPad::default()], b, true)
}

fn sfx(ev: &[MenuEvent]) -> Vec<u16> {
    ev.iter()
        .filter_map(|e| match e {
            MenuEvent::Sfx(id) => Some(*id),
            _ => None,
        })
        .collect()
}

#[test]
fn op3_results_pick_the_opponent() {
    // [emu op3-*]: 0x100 save in slot 2, 0x280 no card, 0x380 empty /
    // unformatted / corrupt.
    assert_eq!(selector_from_result(0x100), Some(0));
    assert_eq!(selector_from_result(0x280), Some(1));
    assert_eq!(selector_from_result(0x380), Some(2));
    assert_eq!(Opponent::of(0), Some(Opponent::SecondPlayer));
    assert_eq!(Opponent::of(1).unwrap().control(), 22);
    assert_eq!(Opponent::of(2), Some(Opponent::Deck { control: 25, record: 0x32 }));
    assert_eq!(Opponent::of(3).unwrap().control(), 24);
    assert_eq!(Opponent::of(4), Some(Opponent::Deck { control: 23, record: 0x30 }));
    assert_eq!(side_one_block(1), Some(SideOneBlock::PlayerCopy));
    assert_eq!(side_one_block(2), Some(SideOneBlock::StaticDeck(0x32)));
}

#[test]
fn a_fresh_menu_against_the_computer() {
    let b = fake();
    let m = CustomDuel::new(1, &b);
    assert_eq!(m.sides[0], DuelSide { control: 100, lp: 4000, sp: 3 });
    assert_eq!(m.sides[1], DuelSide { control: 22, lp: 4000, sp: 3 });
    assert_eq!(m.maps.len(), 24);
    assert_eq!(m.map_top, 22);
    assert_eq!(m.chosen_entry(), 0x200);
    assert_eq!(entry_map(0x200), 0x16);
    assert_eq!(entry_map(0x217), 0x2D);
    assert_eq!(entry_map(0x003), 0x31);
    assert_eq!(entry_map(0x104), 0x37);
    let own = Fake { maps: [true, true], ..fake() };
    let m = CustomDuel::new(1, &own);
    assert_eq!(m.maps.len(), 34);
    assert_eq!((m.own_top, m.own_count, m.map_top), (3, 5, 32));
}

#[test]
fn the_grid_skips_missing_rows_and_edit_map_without_own_maps() {
    let mut b = fake();
    let mut m = CustomDuel::new(1, &b);
    // Right: Build Deck (column 1, row 0); Down: Save.
    assert_eq!(sfx(&press(&mut m, &mut b, pad::RIGHT)), [SFX_CURSOR]);
    assert_eq!((m.players[0].cursor, m.players[0].state), (5, row::BUILD_DECK));
    press(&mut m, &mut b, pad::DOWN);
    assert_eq!((m.players[0].cursor, m.players[0].state), (6, row::SAVE));
    // Down: Edit Map is skipped without own maps, rows 3 and 4 have none
    // → Build Deck again.
    press(&mut m, &mut b, pad::DOWN);
    assert_eq!((m.players[0].cursor, m.players[0].state), (5, row::BUILD_DECK));
    // Left → Select Deck; Up wraps over rows 4 and 3 to SP Recharge Rate.
    press(&mut m, &mut b, pad::LEFT);
    assert_eq!(m.players[0].state, row::SELECT_DECK);
    press(&mut m, &mut b, pad::UP);
    assert_eq!((m.players[0].cursor, m.players[0].state), (2, row::SP_RECHARGE));
    // Right from row 2: Edit Map is skipped → back to itself: no move, no SE.
    assert!(sfx(&press(&mut m, &mut b, pad::RIGHT)).is_empty());
    assert_eq!(m.players[0].state, row::SP_RECHARGE);
    // With own maps Edit Map is a row.
    let mut own = Fake { maps: [true, false], ..fake() };
    let mut m = CustomDuel::new(1, &own);
    press(&mut m, &mut own, pad::UP);
    press(&mut m, &mut own, pad::RIGHT);
    assert_eq!((m.players[0].cursor, m.players[0].state), (7, row::EDIT_MAP));
}

#[test]
fn lp_and_sp_rows_commit_with_the_computer_floor() {
    let mut b = fake();
    let mut m = CustomDuel::new(1, &b);
    press(&mut m, &mut b, pad::DOWN);
    press(&mut m, &mut b, pad::CROSS);
    assert_eq!(m.players[0].state, mode::EDIT | row::LP_SETTING);
    for _ in 0..5 {
        press(&mut m, &mut b, pad::LEFT);
    }
    assert_eq!(m.players[0].lp, 3500);
    assert_eq!(m.sides[0].lp, 4000, "not committed yet");
    assert_eq!(sfx(&press(&mut m, &mut b, pad::CROSS)), [SFX_CONFIRM]);
    assert_eq!((m.sides[0].lp, m.sides[1].lp), (3500, 4000));
    assert_eq!((m.players[0].lp_set, m.players[1].lp_set), (3500, 4000));
    // Cross again on the committed value: nothing.
    assert!(press(&mut m, &mut b, pad::CROSS).is_empty());
    for _ in 0..70 {
        press(&mut m, &mut b, pad::RIGHT);
    }
    assert_eq!(m.players[0].lp, 9999);
    assert_eq!(sfx(&press(&mut m, &mut b, pad::R1)), [SFX_BUZZER]);
    press(&mut m, &mut b, pad::CROSS);
    assert_eq!((m.sides[0].lp, m.sides[1].lp), (9999, 9999));
    // From 9999 Left gives 9900 (rounded hundreds).
    press(&mut m, &mut b, pad::LEFT);
    assert_eq!(m.players[0].lp, 9900);
    assert_eq!(sfx(&press(&mut m, &mut b, pad::CIRCLE)), [SFX_CANCEL]);
    assert_eq!(m.players[0].state, row::LP_SETTING);
    // SP: down to 1 (buzzer at the floor), commit; the computer gets 3.
    press(&mut m, &mut b, pad::DOWN);
    press(&mut m, &mut b, pad::CROSS);
    press(&mut m, &mut b, pad::LEFT);
    press(&mut m, &mut b, pad::LEFT);
    assert_eq!(sfx(&press(&mut m, &mut b, pad::LEFT)), [SFX_BUZZER]);
    press(&mut m, &mut b, pad::CROSS);
    assert_eq!((m.sides[0].sp, m.sides[1].sp), (1, 3));
    for _ in 0..20 {
        press(&mut m, &mut b, pad::RIGHT);
    }
    press(&mut m, &mut b, pad::CROSS);
    assert_eq!((m.sides[0].sp, m.sides[1].sp), (12, 12));
}

#[test]
fn select_deck_refuses_an_unplayable_deck_and_mirrors_the_choice() {
    let mut b = Fake { playable: [[true, true, false]; 2], ..fake() };
    let mut m = CustomDuel::new(1, &b);
    press(&mut m, &mut b, pad::CROSS);
    assert_eq!(m.players[0].state, mode::EDIT);
    press(&mut m, &mut b, pad::LEFT);
    assert_eq!(m.players[0].deck, 2);
    assert_eq!(sfx(&press(&mut m, &mut b, pad::CROSS)), [SFX_BUZZER]);
    press(&mut m, &mut b, pad::RIGHT);
    press(&mut m, &mut b, pad::RIGHT);
    assert_eq!(m.players[0].deck, 1);
    assert_eq!(sfx(&press(&mut m, &mut b, pad::CROSS)), [SFX_CONFIRM]);
    assert_eq!(b.current, [1, 1], "the mirror's block follows");
    assert!(m.reload[0]);
}

#[test]
fn start_opens_the_map_list_and_cross_starts_the_duel() {
    let mut b = fake();
    let mut m = CustomDuel::new(2, &b);
    assert_eq!(sfx(&press(&mut m, &mut b, pad::START)), [SFX_CONFIRM]);
    assert_eq!(m.players.map(|p| p.state & 0xF0), [mode::MAP; 2]);
    press(&mut m, &mut b, pad::DOWN);
    press(&mut m, &mut b, pad::R1);
    let ev = press(&mut m, &mut b, pad::CROSS);
    let start = ev
        .iter()
        .find_map(|e| match e {
            MenuEvent::Start(s) => Some(*s),
            _ => None,
        })
        .unwrap();
    // top 22 → 23 → 28 % 24 = 4; chosen = 6 → map 0x1C.
    assert_eq!(start.map, 0x16 + 6);
    assert_eq!(start.identities, [1100, 1025]);
    assert_eq!((start.leader_abilities, start.card_display), (3, 3));
    assert_eq!(start.controls(), [100, 25]);
    // After the duel: the list is still open.
    m.resume();
    assert_eq!(m.players[0].state & 0xF0, mode::MAP);
    press(&mut m, &mut b, pad::CIRCLE);
    assert_eq!(m.players[0].state, 0);
}

#[test]
fn circle_asks_before_returning_to_the_title() {
    let mut b = fake();
    let mut m = CustomDuel::new(1, &b);
    press(&mut m, &mut b, pad::CIRCLE);
    assert_eq!(m.players.map(|p| p.state), [mode::EXIT; 2]);
    assert!(!m.exit_yes);
    assert_eq!(sfx(&press(&mut m, &mut b, pad::LEFT)), [SFX_CURSOR]);
    assert!(press(&mut m, &mut b, pad::CROSS).contains(&MenuEvent::Title));
}

#[test]
fn mirror_block_clears_the_both_routes_mark() {
    let mut block = CardBlock::empty(7);
    crate::ending::mark_both_routes(&mut block);
    let copy = mirror_block(&block);
    assert!(crate::ending::both_routes_marked(&block));
    assert!(!crate::ending::both_routes_marked(&copy));
}

// ---------------------------------------------------------------- oracle

#[derive(serde::Deserialize)]
struct Env {
    current: [u8; 2],
    playable: [[u8; 3]; 2],
    maps: [u32; 2],
}

#[derive(serde::Deserialize)]
struct Frame {
    a9: Vec<u8>,
    lp: [u16; 2],
    ctl: [u8; 2],
    sp: [u8; 2],
    pad: [u16; 2],
    ret: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct Capture {
    selector: u8,
    env: Env,
    log: Vec<serde_json::Value>,
    frames: Vec<Frame>,
}

fn bytes_of(m: &CustomDuel) -> Vec<(usize, u8)> {
    let mut v = Vec::new();
    for (s, p) in m.players.iter().enumerate() {
        v.push((s, p.cursor));
        v.push((2 + s, p.state));
        v.push((4 + s, p.deck));
        v.push((6 + s, p.sp));
        v.push((8 + s, p.leader));
        v.push((0xA + s, p.leader_set));
        v.push((0xC + s, p.display));
        v.push((0xE + s, p.display_set));
        v.push((0x10 + 2 * s, p.lp as u8));
        v.push((0x11 + 2 * s, (p.lp >> 8) as u8));
        v.push((0x14 + 2 * s, p.lp_set as u8));
        v.push((0x15 + 2 * s, (p.lp_set >> 8) as u8));
    }
    v.push((0x89, m.map_top));
    v.push((0x8A, m.maps.len() as u8));
    v.push((0x8C, m.own_top));
    v.push((0x8D, m.own_count));
    v.push((0x8E, u8::from(m.exit_yes)));
    for (i, e) in m.maps.iter().enumerate() {
        v.push((0x18 + 2 * i, *e as u8));
        v.push((0x19 + 2 * i, (*e >> 8) as u8));
    }
    v
}

/// The interpreter runs of the game's own init/update (`tools/run_cd.py`,
/// `ROSES_CUSTOM_DUEL_CAPTURES` = the `captures` folder): every frame's
/// per-player block, duel words, SE, events and params match.
#[test]
fn private_menu_matches_the_emulated_original() {
    let Some(dir) = std::env::var_os("ROSES_CUSTOM_DUEL_CAPTURES") else {
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('s')) && p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no s*.json captures in {}", std::path::Path::new(&dir).display());
    for path in files {
        let c: Capture = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let mut b = Fake {
            current: c.env.current,
            playable: c.env.playable.map(|s| s.map(|d| d != 0)),
            maps: c.env.maps.map(|v| v != 0),
        };
        let mut m = CustomDuel::new(c.selector, &b);
        // 0xA9C08E is not written by init (residue until the first Circle).
        let mut exit_seen = false;
        let check = |m: &CustomDuel, f: &Frame, at: &str, exit_seen: bool| {
            for (o, v) in bytes_of(m) {
                if o == 0x8E && !exit_seen {
                    continue;
                }
                assert_eq!(v, f.a9[o], "{name} {at}: byte {o:#x}");
            }
            assert_eq!(m.sides.map(|s| s.lp), f.lp, "{name} {at}: LP");
            assert_eq!(m.sides.map(|s| s.control), f.ctl, "{name} {at}: control");
            assert_eq!(m.sides.map(|s| s.sp), f.sp, "{name} {at}: SP");
        };
        check(&m, &c.frames[0], "init", false);
        let mut frame = 0i64;
        // The interpreter's pad words: new = held & !previous; no repeat
        // counters run there, so the repeat word is the new presses.
        let mut prev = [0u16; 2];
        for (k, f) in c.frames.iter().enumerate().skip(1) {
            if f.ret == serde_json::json!("resume") {
                m.resume();
                check(&m, f, &format!("resume at {k}"), exit_seen);
                continue;
            }
            let pads = [0, 1].map(|p| MenuPad::press(f.pad[p] & !prev[p]));
            prev = f.pad;
            let ev = m.update(pads, &mut b, true);
            let at = format!("update {k} (frame {frame}, pads {:?})", f.pad);
            exit_seen |= m.players.iter().any(|p| p.state & 0xF0 == mode::EXIT);
            check(&m, f, &at, exit_seen);
            let logged: Vec<&Vec<serde_json::Value>> = c
                .log
                .iter()
                .filter_map(|e| e.as_array())
                .filter(|e| e[0].as_i64() == Some(frame))
                .collect();
            let want_sfx: Vec<u16> = logged
                .iter()
                .filter(|e| e[1] == "sfx")
                .map(|e| e[4].as_u64().unwrap() as u16)
                .collect();
            assert_eq!(sfx(&ev), want_sfx, "{name} {at}: SE");
            let pushed: Vec<u64> = logged.iter().filter(|e| e[1] == "mode_push").map(|e| e[3].as_u64().unwrap()).collect();
            let card_ops = logged.iter().filter(|e| e[1] == "cardop").count();
            let mut want_push = Vec::new();
            for e in &ev {
                match e {
                    MenuEvent::BuildDeck => want_push.push(6),
                    MenuEvent::EditMap { .. } => want_push.push(0xD),
                    MenuEvent::Start(s) => {
                        want_push.push(1);
                        let param = |i: i64| {
                            logged
                                .iter()
                                .find(|e| e[1] == "param" && e[2].as_i64() == Some(i))
                                .map(|e| e[3].as_i64().unwrap())
                        };
                        assert_eq!(param(1), Some(i64::from(s.map)), "{name} {at}: map");
                        assert_eq!(param(3), Some(i64::from(s.identities[0])), "{name} {at}: p3");
                        assert_eq!(param(4), Some(i64::from(s.identities[1])), "{name} {at}: p4");
                        assert_eq!(param(28), Some(i64::from(s.leader_abilities)), "{name} {at}: 0x1C");
                        assert_eq!(param(29), Some(i64::from(s.card_display)), "{name} {at}: 0x1D");
                    }
                    _ => {}
                }
            }
            assert_eq!(pushed, want_push, "{name} {at}: mode pushes");
            assert_eq!(card_ops, ev.iter().filter(|e| **e == MenuEvent::Save).count(), "{name} {at}: op 1");
            assert_eq!(
                logged.iter().filter(|e| e[1] == "mode_pop").count(),
                ev.iter().filter(|e| **e == MenuEvent::Title).count(),
                "{name} {at}: title"
            );
            frame += 1;
        }
        eprintln!("{name}: {} frames match", c.frames.len());
    }
}
