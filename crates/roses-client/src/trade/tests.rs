use super::*;
use roses_core::card_block::{CardBlock, CardInstance};

/// A static card table good enough for the rules (no executable needed):
/// the comparators fall back to `None` records.
fn cards() -> StaticCards {
    StaticCards::from_executable(&vec![0u8; 0x1D_4000], 0).expect("a zero image is long enough")
}

fn block(key: u32, copies: &[(u16, &[u32])]) -> CardBlock {
    let mut b = CardBlock::empty(key);
    for r in &mut b.records {
        for i in &mut r.instances {
            i.id = 0xC000_0000;
        }
    }
    for &(card, ids) in copies {
        for (k, &id) in ids.iter().enumerate() {
            b.records[usize::from(card)].instances[k] = CardInstance { owner: key, id: 0xC000_0000 | id };
        }
    }
    b
}

fn press(t: &mut Trade, side: usize, bits: u16, blocks: &mut [CardBlock; 2], cards: &StaticCards) -> Vec<TradeEvent> {
    let mut pads = [TradePad::default(); 2];
    pads[side] = TradePad::press(bits);
    t.update(pads, blocks, cards, true)
}

#[test]
fn lists_hold_trunk_copies_and_exports_count_up_to_them() {
    let cards = cards();
    let mut blocks = [block(0x11, &[(5, &[1, 2]), (9, &[3])]), block(0x22, &[(9, &[4])])];
    let mut t = Trade::new(&blocks, &cards);
    assert_eq!((t.sides[0].count, t.sides[1].count), (2, 1));
    assert_eq!(&t.sides[0].list[..3], &[5, 9, EMPTY]);
    // Side 0: Cross adds, Square removes; the maximum is the trunk count.
    for _ in 0..3 {
        press(&mut t, 0, pad::CROSS, &mut blocks, &cards);
        press(&mut t, 0, 0, &mut blocks, &cards);
    }
    assert_eq!(t.sides[0].exports[5], 2);
    press(&mut t, 0, pad::SQUARE, &mut blocks, &cards);
    assert_eq!(t.sides[0].exports[5], 1);
    // Side 1: Square adds.
    let ev = press(&mut t, 1, pad::SQUARE, &mut blocks, &cards);
    assert_eq!(ev, vec![TradeEvent::Sfx(sfx::CURSOR)]);
    assert_eq!(t.sides[1].exports[9], 1);
    // R3: the export view lists the exported cards.
    press(&mut t, 0, R3, &mut blocks, &cards);
    assert_eq!((t.sides[0].view, t.sides[0].count, t.sides[0].list[0]), (1, 1, 5));
}

#[test]
fn both_ready_then_yes_moves_the_copies_and_saves() {
    let cards = cards();
    let mut blocks = [block(0x11, &[(5, &[1, 2])]), block(0x22, &[(9, &[4])])];
    let mut t = Trade::new(&blocks, &cards);
    press(&mut t, 0, pad::CROSS, &mut blocks, &cards);
    press(&mut t, 0, pad::START, &mut blocks, &cards);
    assert_eq!(t.sides[0].state, state::READY);
    press(&mut t, 1, pad::START, &mut blocks, &cards);
    assert_eq!([t.sides[0].state, t.sides[1].state], [state::TRADE_ASK; 2]);
    press(&mut t, 1, pad::LEFT, &mut blocks, &cards);
    let ev = press(&mut t, 1, pad::CROSS, &mut blocks, &cards);
    assert_eq!(ev, vec![TradeEvent::Sfx(sfx::CONFIRM), TradeEvent::Save]);
    assert_eq!(trunk_copies(&blocks[0], 5), 1);
    assert_eq!(trunk_copies(&blocks[1], 5), 1);
    let moved = blocks[1].records[5].instances[0];
    assert_eq!((moved.owner, moved.random_id()), (0x11, 1), "the copy keeps its owner key and id");
    assert_eq!(blocks[1].recent[0], 5);
    // State 7 pops once the card module is idle (both sides' updates).
    let ev = t.update([TradePad::default(); 2], &mut blocks, &cards, true);
    assert_eq!(ev, vec![TradeEvent::Title, TradeEvent::Title]);
}

#[test]
fn a_duplicate_id_or_a_full_destination_refuses_the_trade() {
    let cards = cards();
    let nine: Vec<u32> = (100..109).collect();
    let mut blocks = [block(0x11, &[(5, &[1]), (9, &[7])]), block(0x22, &[(5, &nine), (9, &[7])])];
    let mut t = Trade::new(&blocks, &cards);
    press(&mut t, 0, pad::CROSS, &mut blocks, &cards);
    press(&mut t, 0, pad::DOWN, &mut blocks, &cards);
    press(&mut t, 0, pad::CROSS, &mut blocks, &cards);
    assert_eq!(t.sides[0].exported(), 2);
    press(&mut t, 0, pad::START, &mut blocks, &cards);
    press(&mut t, 1, pad::START, &mut blocks, &cards);
    press(&mut t, 0, pad::LEFT, &mut blocks, &cards);
    let ev = press(&mut t, 0, pad::CROSS, &mut blocks, &cards);
    assert!(!ev.contains(&TradeEvent::Save));
    assert_eq!([t.sides[0].state, t.sides[1].state], [state::REFUSED; 2]);
    let ev = press(&mut t, 1, pad::CIRCLE, &mut blocks, &cards);
    assert_eq!(ev, vec![TradeEvent::Sfx(sfx::CONFIRM), TradeEvent::Title]);
}

/// The model against the interpreter runs of 0x1C3050 / 0x1C3470
/// (`ROSES_TRADE_CAPTURES` = `modes/two-player/captures`, `ROSES_DUEL_IMPORT`
/// = a duel import folder for the executable's card table): every update's
/// per-side state, window, lists, export counts, SE ids and mode pops, and
/// the card records the trade changed.
#[test]
fn private_trade_matches_the_emulated_original() {
    let (Some(dir), Some(duel)) = (std::env::var_os("ROSES_TRADE_CAPTURES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
        return;
    };
    let exe = std::fs::read(std::path::Path::new(&duel).join("SLES_515.79")).unwrap();
    let cards = StaticCards::from_executable(&exe, 0).unwrap();
    let mut checked = 0;
    for name in ["t0-probe", "t1-trade", "t2-refused", "t3-no-yes"] {
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(std::path::Path::new(&dir).join(format!("{name}.json"))).unwrap()).unwrap();
        let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
        let starts: Vec<Vec<u8>> = v["start_blocks"].as_array().unwrap().iter().map(|b| hex(b.as_str().unwrap())).collect();
        let mut blocks = [CardBlock::from_bytes(&starts[0]).unwrap(), CardBlock::from_bytes(&starts[1]).unwrap()];
        let mut t = Trade::new(&blocks, &cards);
        let log = v["log"].as_array().unwrap();
        let frames = v["frames"].as_array().unwrap();
        let mut prev = [0u16; 2];
        let mut stopped = false;
        for (k, f) in frames.iter().enumerate() {
            if k > 0 {
                let word = [0, 1].map(|p| f["pad"][p].as_u64().unwrap() as u16);
                let pads = [0, 1].map(|p| TradePad {
                    pressed: word[p] & !prev[p],
                    held: word[p],
                    repeat: word[p] & !prev[p],
                    analog: None,
                });
                prev = word;
                let ev = t.update(pads, &mut blocks, &cards, true);
                let frame = (k - 1) as i64;
                let theirs: Vec<String> = log
                    .iter()
                    .filter(|e| e[0].as_i64() == Some(frame))
                    .filter_map(|e| match e[1].as_str()? {
                        "sfx" => Some(format!("sfx {}", e[4].as_u64()?)),
                        "mode_pop" => Some("title".into()),
                        "cardop" => Some("save".into()),
                        _ => None,
                    })
                    .collect();
                let ours: Vec<String> = ev
                    .iter()
                    .map(|e| match e {
                        TradeEvent::Sfx(id) => format!("sfx {id}"),
                        TradeEvent::Title => "title".into(),
                        TradeEvent::Save => "save".into(),
                    })
                    .collect();
                assert_eq!(ours, theirs, "{name} frame {frame} events");
                // The interpreter keeps calling the stubbed pop; the port's
                // frontend leaves at the first one.
                stopped = ours.contains(&"title".to_string());
            }
            let ctx = format!("{name} frame {}", k as i64 - 1);
            let u8s = |key: &str| [0, 1].map(|s| f[key][s].as_u64().unwrap() as u16);
            let s = &t.sides;
            assert_eq!(u8s("state"), [0, 1].map(|i| u16::from(s[i].state)), "{ctx} state");
            assert_eq!(u8s("view"), [0, 1].map(|i| u16::from(s[i].view)), "{ctx} view");
            assert_eq!(u8s("cursor"), [0, 1].map(|i| u16::from(s[i].cursor)), "{ctx} cursor");
            assert_eq!(u8s("keyidx"), [0, 1].map(|i| u16::from(s[i].page)), "{ctx} page");
            assert_eq!(u8s("top"), [0, 1].map(|i| s[i].top), "{ctx} top");
            assert_eq!(u8s("count"), [0, 1].map(|i| s[i].count), "{ctx} count");
            assert_eq!(u8s("slide"), [0, 1].map(|i| s[i].slide as u16), "{ctx} slide");
            assert_eq!(u8s("idle"), [0, 1].map(|i| s[i].idle), "{ctx} idle");
            assert_eq!(f["yes"].as_u64().unwrap() != 0, t.yes, "{ctx} yes");
            for i in 0..2 {
                let keys: Vec<u8> = f["keys"][i].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u8).collect();
                assert_eq!(keys, s[i].keys.to_vec(), "{ctx} keys {i}");
                let list: Vec<u16> = f["lists"][i].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u16).collect();
                assert_eq!(list, s[i].list, "{ctx} list {i}");
                let gives = f["gives"][i].as_object().unwrap();
                let ours: Vec<(usize, u8)> = s[i].exports.iter().enumerate().filter(|e| *e.1 != 0).map(|(c, &n)| (c, n)).collect();
                let theirs: Vec<(usize, u8)> = gives.iter().map(|(c, n)| (c.parse().unwrap(), n.as_u64().unwrap() as u8)).collect();
                assert_eq!(ours, theirs, "{ctx} exports {i}");
            }
            checked += 1;
            if stopped {
                break;
            }
        }
        for c in v["changed"].as_array().unwrap() {
            let side = c["side"].as_u64().unwrap() as usize;
            let card = c["card"].as_u64().unwrap() as usize;
            let after = hex(c["after"].as_str().unwrap());
            let ours = blocks[side].to_bytes().unwrap();
            assert_eq!(&ours[card * 0x4C..card * 0x4C + 0x4C], &after[..], "{name}: side {side} card {card}");
        }
        // Nothing else changed.
        for side in 0..2 {
            let ours = blocks[side].to_bytes().unwrap();
            let changed: Vec<usize> = (0..CARDS).filter(|&c| ours[c * 0x4C..c * 0x4C + 0x4C] != starts[side][c * 0x4C..c * 0x4C + 0x4C]).collect();
            let theirs: Vec<usize> = v["changed"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["side"].as_u64() == Some(side as u64))
                .map(|c| c["card"].as_u64().unwrap() as usize)
                .collect();
            assert_eq!(changed, theirs, "{name}: changed records of side {side}");
        }
    }
    eprintln!("{checked} trade updates match");
    assert!(checked > 100);
}
