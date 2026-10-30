use super::*;
use crate::campaign_map::CampaignSave;
use roses_core::memory_card::{SaveFile, BLOCK_BYTES, FILE_BYTES};

/// A card for the tests: status, the data file, what was written.
#[derive(Clone, Debug)]
struct TestCard {
    status: CardStatus,
    file: Option<Vec<u8>>,
    writes: usize,
    formats: usize,
}

impl TestCard {
    fn formatted(file: Option<Vec<u8>>) -> Self {
        Self { status: CardStatus::Formatted { free_kb: 8000 }, file, writes: 0, formats: 0 }
    }
}

impl CardIo for TestCard {
    fn status(&self) -> CardStatus {
        self.status
    }
    fn has_save(&self) -> bool {
        self.file.is_some()
    }
    fn read(&mut self) -> Result<Vec<u8>, String> {
        self.file.clone().ok_or_else(|| "no file".into())
    }
    fn write(&mut self, file: &[u8]) -> Result<(), String> {
        self.file = Some(file.to_vec());
        self.writes += 1;
        Ok(())
    }
    fn format(&mut self) -> Result<(), String> {
        self.status = CardStatus::Formatted { free_kb: 8000 };
        self.file = None;
        self.formats += 1;
        Ok(())
    }
}

fn sample_file() -> Vec<u8> {
    let mut map = CampaignSave::after_landing();
    map.wins = 2;
    let save = save_record(None, &map, &[0x2005; 12]);
    SaveFile::new(vec![7; BLOCK_BYTES], save).unwrap().to_bytes().unwrap()
}

const CROSS: CardPad = CardPad { left: false, right: false, cross: true, circle: false, square: false, triangle: false };
const LEFT: CardPad = CardPad { left: true, right: false, cross: false, circle: false, square: false, triangle: false };
const CIRCLE: CardPad = CardPad { left: false, right: false, cross: false, circle: true, square: false, triangle: false };
const NONE: CardPad = CardPad { left: false, right: false, cross: false, circle: false, square: false, triangle: false };

/// Run `pads` (one per update) and return each update's box and outcome.
fn run(d: &mut CardDialog, card: &mut TestCard, pads: &[CardPad]) -> Vec<(Option<CardBox>, CardOutcome, Vec<u16>)> {
    pads.iter()
        .map(|&p| {
            let o = d.update(p, card);
            (d.overlay(), o, d.sfx.clone())
        })
        .collect()
}

#[test]
fn a_first_save_checks_saves_then_waits_for_a_button() {
    let mut card = TestCard::formatted(None);
    let file = sample_file();
    let mut d = CardDialog::save(file.clone());
    let mut pads = vec![NONE; 30];
    pads.push(CROSS);
    let out = run(&mut d, &mut card, &pads);
    // u1 nothing, u2..u3 Checking, u4..u23 Saving, u24 busy "Save
    // completed", u25.. the acknowledge box [emu save-fresh f5..f29].
    assert_eq!(out[0].0, None);
    assert_eq!(out[1].0, Some(CardBox::Busy(msg::CHECKING)));
    assert_eq!(out[2].0, Some(CardBox::Busy(msg::CHECKING)));
    for u in 4..=23 {
        assert_eq!(out[u - 1].0, Some(CardBox::Busy(msg::SAVING)), "u{u}");
    }
    assert_eq!(out[23].0, Some(CardBox::Busy(msg::SAVE_COMPLETE)));
    assert_eq!(out[24].0, Some(CardBox::Acknowledge(msg::SAVE_COMPLETE)));
    assert_eq!(card.writes, 1);
    assert_eq!(card.file.as_deref(), Some(file.as_slice()));
    let last = out.last().unwrap();
    assert_eq!((last.0, &last.1, &last.2), (None, &CardOutcome::Closed, &vec![SFX_CONFIRM]));
    assert!(d.finished());
}

#[test]
fn an_existing_save_asks_to_overwrite_with_no_preselected() {
    let mut card = TestCard::formatted(Some(sample_file()));
    let mut d = CardDialog::save(sample_file());
    let mut pads = vec![NONE; 8];
    pads.extend([LEFT, NONE, CROSS]);
    pads.extend(vec![NONE; 6]);
    let out = run(&mut d, &mut card, &pads);
    for u in 2..=7 {
        assert_eq!(out[u - 1].0, Some(CardBox::Busy(msg::CHECKING)), "u{u}");
    }
    assert_eq!(out[7].0, Some(CardBox::YesNo { text: msg::OVERWRITE_ASK, yes: false }));
    assert_eq!(out[8].0, Some(CardBox::YesNo { text: msg::OVERWRITE_ASK, yes: true }));
    assert_eq!(out[8].2, vec![SFX_CURSOR]);
    // The YES frame keeps the box; Saving for 4 updates, then completed.
    assert_eq!(out[10].0, Some(CardBox::YesNo { text: msg::OVERWRITE_ASK, yes: true }));
    assert_eq!(out[10].2, vec![SFX_CONFIRM]);
    for k in 11..=14 {
        assert_eq!(out[k].0, Some(CardBox::Busy(msg::SAVING)));
    }
    assert_eq!(out[15].0, Some(CardBox::Busy(msg::SAVE_COMPLETE)));
    assert_eq!(out[16].0, Some(CardBox::Acknowledge(msg::SAVE_COMPLETE)));
    assert_eq!(card.writes, 1);
}

#[test]
fn no_or_circle_closes_without_writing() {
    for (pad, sfx) in [(CROSS, SFX_CONFIRM), (CIRCLE, SFX_CANCEL)] {
        let mut card = TestCard::formatted(Some(sample_file()));
        let mut d = CardDialog::save(sample_file());
        let mut pads = vec![NONE; 8];
        pads.push(pad);
        let out = run(&mut d, &mut card, &pads);
        let last = out.last().unwrap();
        assert_eq!((last.0, &last.1, &last.2), (None, &CardOutcome::Closed, &vec![sfx]));
        assert_eq!(card.writes, 0);
    }
}

#[test]
fn failure_boxes_wait_for_any_face_button() {
    let cases = [
        (CardStatus::Missing, 1, msg::NO_CARD),
        (CardStatus::Formatted { free_kb: 249 }, 3, msg::NO_SPACE),
    ];
    for (status, at, text) in cases {
        let mut card = TestCard { status, file: None, writes: 0, formats: 0 };
        let mut d = CardDialog::save(Vec::new());
        let pads = [NONE, NONE, NONE, NONE, CardPad { triangle: true, ..NONE }];
        let out = run(&mut d, &mut card, &pads);
        assert_eq!(out[at].0, Some(CardBox::Acknowledge(text)));
        assert_eq!(out[4].1, CardOutcome::Closed);
        assert_eq!(card.writes, 0);
    }
    // 250 KB is enough [emu].
    let mut card = TestCard { status: CardStatus::Formatted { free_kb: 250 }, file: None, writes: 0, formats: 0 };
    let mut d = CardDialog::save(sample_file());
    let out = run(&mut d, &mut card, &[NONE; 4]);
    assert_eq!(out[3].0, Some(CardBox::Busy(msg::SAVING)));
}

#[test]
fn an_unformatted_card_is_formatted_on_yes_then_saved() {
    let mut card = TestCard { status: CardStatus::Unformatted, file: None, writes: 0, formats: 0 };
    let mut d = CardDialog::save(sample_file());
    let mut pads = vec![NONE; 4];
    pads.extend([LEFT, CROSS, NONE, NONE, CROSS]);
    pads.extend(vec![NONE; 8]);
    let out = run(&mut d, &mut card, &pads);
    assert_eq!(out[3].0, Some(CardBox::YesNo { text: msg::FORMAT_ASK, yes: false }));
    assert_eq!(out[6].0, Some(CardBox::Busy(msg::FORMATTING)));
    assert_eq!(out[7].0, Some(CardBox::Acknowledge(msg::FORMAT_DONE)));
    // The acknowledge's frame keeps the box, then four Checking updates,
    // then Saving [emu save-unformatted f59..f64].
    assert_eq!(out[8].0, Some(CardBox::Acknowledge(msg::FORMAT_DONE)));
    for k in 9..=12 {
        assert_eq!(out[k].0, Some(CardBox::Busy(msg::CHECKING)), "k{k}");
    }
    assert_eq!(out[13].0, Some(CardBox::Busy(msg::SAVING)));
    assert_eq!(card.formats, 1);
}

#[test]
fn continue_loads_a_good_file_and_refuses_a_bad_one() {
    let file = sample_file();
    let mut card = TestCard::formatted(Some(file.clone()));
    let mut d = CardDialog::load();
    let mut pads = vec![NONE; 8];
    pads.push(CROSS);
    let out = run(&mut d, &mut card, &pads);
    assert_eq!(out[0].0, Some(CardBox::Busy(msg::CHECKING)));
    assert_eq!(out[1].0, Some(CardBox::Busy(msg::CHECKING)));
    for k in 2..=5 {
        assert_eq!(out[k].0, Some(CardBox::Busy(msg::LOADING)));
    }
    assert_eq!(out[6].0, Some(CardBox::Busy(msg::LOAD_COMPLETE)));
    assert_eq!(out[7].0, Some(CardBox::Acknowledge(msg::LOAD_COMPLETE)));
    assert_eq!(out[8].1, CardOutcome::Loaded(file.clone()));

    let mut bad = file.clone();
    bad[0x10] ^= 0xFF;
    let mut card = TestCard::formatted(Some(bad));
    let mut d = CardDialog::load();
    let out = run(&mut d, &mut card, &[NONE, NONE, NONE, NONE, NONE, NONE, NONE, CROSS]);
    assert_eq!(out[6].0, Some(CardBox::Acknowledge(msg::LOAD_FAILED)));
    assert_eq!(out[7].1, CardOutcome::LoadFailed);

    let mut card = TestCard::formatted(None);
    let mut d = CardDialog::load();
    let out = run(&mut d, &mut card, &[NONE, NONE, CROSS]);
    assert_eq!(out[1].0, Some(CardBox::Acknowledge(msg::NO_SAVE_DATA)));
    assert_eq!(out[2].1, CardOutcome::LoadFailed);
}

#[test]
fn the_save_record_carries_the_port_fields_and_keeps_the_rest() {
    let mut map = CampaignSave::after_landing();
    map.event = 303;
    map.wins = 4;
    map.losses = 1;
    map.wins_by_opponent[2] = 4;
    map.losses_by_opponent[5] = 1;
    map.location = 2;
    map.hovered = 2;
    map.destination = 2;
    map.duel_count = 3;
    map.c9 = 1;
    map.opponent = 2;
    let mut base = [0u8; SAVE_BYTES];
    base[2] = 1; // white route cleared (not modelled)
    base[0x100] = 0xAB; // an unmodelled byte
    let mut name = vec![0x2010; 12];
    name[11] |= 0x8000;
    let s = save_record(Some(&base), &map, &name);
    assert_eq!(s[0x100], 0xAB);
    assert_eq!(u16::from_le_bytes([s[0xC2], s[0xC3]]), 303);
    assert_eq!(u16::from_le_bytes([s[0x3C], s[0x3D]]), 1);
    assert_eq!(u16::from_le_bytes([s[0x42 + 4], s[0x43 + 4]]), 4);
    assert_eq!(s[3 + 1], map.node_states[1]);
    let back = load_record(&s);
    // SAVE[2] comes back in the map's save too.
    assert_eq!(back.map, CampaignSave { white_route_cleared: true, ..map });
    assert_eq!(back.name_codes, name);
    assert!(back.white_route_cleared);
    assert!(back.map_entered);
}

/// The dialogs against the original's own code run in the EE interpreter
/// with a virtual card (`work/research/story/claude-save/captures/
/// *.json`, private): per frame the overlay state, text and YES/NO cursor
/// and the SE requests. Set ROSES_SAVE_CAPTURES to the captures folder.
#[test]
fn private_dialogs_match_the_emulated_original() {
    let Some(dir) = std::env::var_os("ROSES_SAVE_CAPTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let fresh_file = sample_file();
    let mut checked = 0;
    for (name, load, card) in [
        ("save-fresh", false, TestCard::formatted(None)),
        ("save-overwrite-yes", false, TestCard::formatted(Some(fresh_file.clone()))),
        ("save-overwrite-no", false, TestCard::formatted(Some(fresh_file.clone()))),
        ("save-overwrite-circle", false, TestCard::formatted(Some(fresh_file.clone()))),
        ("save-overwrite-leftleft", false, TestCard::formatted(Some(fresh_file.clone()))),
        ("save-nocard", false, TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 }),
        ("save-nocard-circle", false, TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 }),
        ("save-unformatted", false, TestCard { status: CardStatus::Unformatted, file: None, writes: 0, formats: 0 }),
        ("save-unformatted-no", false, TestCard { status: CardStatus::Unformatted, file: None, writes: 0, formats: 0 }),
        ("save-full", false, TestCard { status: CardStatus::Formatted { free_kb: 100 }, file: None, writes: 0, formats: 0 }),
        ("load-ok", true, TestCard::formatted(Some(fresh_file.clone()))),
        ("load-corrupt", true, TestCard::formatted(Some({ let mut b = fresh_file.clone(); b[0x10] ^= 0xFF; b }))),
        ("load-nosave", true, TestCard::formatted(None)),
        ("load-nocard", true, TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 }),
        ("load-unformatted", true, TestCard { status: CardStatus::Unformatted, file: None, writes: 0, formats: 0 }),
    ] {
        let path = dir.join(format!("{name}.json"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let frames = v["frames"].as_array().unwrap();
        let sfx_at = |f: i64| -> Vec<u16> {
            v["emu_log"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .filter(|e| e[0].as_i64() == Some(f) && e[1] == "sfx")
                        .map(|e| e[4].as_u64().unwrap() as u16)
                        .collect()
                })
                .unwrap_or_default()
        };
        let pad_of = |p: u64| CardPad {
            left: p & 0x8000 != 0,
            right: p & 0x2000 != 0,
            cross: p & 0x40 != 0,
            circle: p & 0x20 != 0,
            square: p & 0x80 != 0,
            triangle: p & 0x10 != 0,
        };
        // The save starts at the frame of the map's Cross (W+543E 0 → 1).
        let start = if load {
            0
        } else {
            frames.iter().position(|f| f["w543e"] == 1).unwrap() + 1
        };
        let (mut d, mut card) = (if load { CardDialog::load() } else { CardDialog::save(fresh_file.clone()) }, card);
        for f in &frames[start..] {
            let frame = f["frame"].as_i64().unwrap();
            let outcome = d.update(pad_of(f["pad"].as_u64().unwrap()), &mut card);
            let b = f["box"].as_array().unwrap();
            let (state, text, d35) = (b[0].as_u64().unwrap(), b[3].as_u64().unwrap() as u16, b[4].as_u64().unwrap());
            let want = match state {
                0x14 => Some(CardBox::Busy(text)),
                0x15 => Some(CardBox::Acknowledge(text)),
                0x16 => Some(CardBox::YesNo { text, yes: d35 != 0 }),
                _ => None,
            };
            assert_eq!(d.overlay(), want, "{name} frame {frame}");
            if !load {
                assert_eq!(d.sfx, sfx_at(frame), "{name} frame {frame} SE");
            }
            if outcome != CardOutcome::Running {
                if load {
                    let ret = f["ret"].as_i64().unwrap();
                    let want = if matches!(outcome, CardOutcome::Loaded(_)) { 1 } else { 2 };
                    assert_eq!(ret, want, "{name} frame {frame} return");
                }
                break;
            }
        }
        checked += 1;
    }
    assert!(checked > 0, "no capture found in {}", dir.display());
    let _ = FILE_BYTES;
}

/// The file the original wrote (ROSES_SAVE_ORACLE, private): its card block
/// decodes and re-encodes byte for byte and its SAVE survives the port's
/// record round trip.
#[test]
fn private_the_original_file_round_trips_through_the_port_state() {
    let Some(path) = std::env::var_os("ROSES_SAVE_ORACLE") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let parsed = SaveFile::parse(&bytes).unwrap();
    assert!(parsed.checksum_ok);
    let block = roses_core::card_block::CardBlock::from_bytes(&parsed.file.block).unwrap();
    assert_eq!(block.to_bytes().unwrap(), parsed.file.block);
    let record = load_record(&parsed.file.save);
    let again = save_record(Some(&parsed.file.save), &record.map, &record.name_codes);
    assert_eq!(again, parsed.file.save);
    let rebuilt = SaveFile::new(block.to_bytes().unwrap(), again).unwrap().to_bytes().unwrap();
    assert_eq!(rebuilt, bytes);
}

// ------------------------------------------------------------- op 2 (ending)
const RIGHT: CardPad = CardPad { left: false, right: true, cross: false, circle: false, square: false, triangle: false };

/// The ending's save on a card without a save [emu op2-fresh]: "Save?" with
/// YES from the first update; YES at Y → Checking Y+1..Y+2, Saving
/// Y+3..Y+22, busy "Save completed" Y+23, acknowledge from Y+24; Cross →
/// return 1 (no box), then −1.
#[test]
fn the_ending_save_asks_first_then_saves() {
    let mut card = TestCard::formatted(None);
    let file = sample_file();
    let mut d = CardDialog::ending_save(file.clone());
    assert_eq!(d.op(), CardOp::EndingSave);
    let mut pads = vec![NONE; 30];
    pads.push(CROSS); // u31 = Y
    pads.extend(vec![NONE; 30]);
    pads.push(CROSS); // u62
    pads.push(NONE);
    let out = run(&mut d, &mut card, &pads);
    for u in 1..=30 {
        assert_eq!(out[u - 1].0, Some(CardBox::YesNo { text: msg::SAVE_ASK, yes: true }), "u{u}");
    }
    assert_eq!(out[30].0, Some(CardBox::YesNo { text: msg::SAVE_ASK, yes: true }));
    assert_eq!(out[30].2, vec![SFX_CONFIRM]);
    assert_eq!(out[31].0, Some(CardBox::Busy(msg::CHECKING)));
    assert_eq!(out[32].0, Some(CardBox::Busy(msg::CHECKING)));
    for u in 34..=53 {
        assert_eq!(out[u - 1].0, Some(CardBox::Busy(msg::SAVING)), "u{u}");
    }
    assert_eq!(out[53].0, Some(CardBox::Busy(msg::SAVE_COMPLETE)));
    assert_eq!(out[54].0, Some(CardBox::Acknowledge(msg::SAVE_COMPLETE)));
    assert_eq!(card.file.as_deref(), Some(&file[..]));
    assert_eq!((out[61].0, &out[61].1, &out[61].2), (None, &CardOutcome::Saved, &vec![SFX_CONFIRM]));
    assert_eq!(out[62].1, CardOutcome::Closed);
}

/// NO (Right, Cross) or Circle on "Save?" gives up (return 2, nothing
/// written); refusals and NO on later questions come back to "Save?".
#[test]
fn the_ending_save_only_gives_up_on_the_first_question() {
    let mut card = TestCard::formatted(None);
    let mut d = CardDialog::ending_save(sample_file());
    let out = run(&mut d, &mut card, &[NONE, RIGHT, CROSS, NONE]);
    assert_eq!(out[1].0, Some(CardBox::YesNo { text: msg::SAVE_ASK, yes: false }));
    assert_eq!(out[1].2, vec![SFX_CURSOR]);
    assert_eq!((out[2].0, &out[2].1), (None, &CardOutcome::NotSaved));
    assert_eq!(out[3].1, CardOutcome::Closed);
    assert_eq!(card.writes, 0);
    let mut d = CardDialog::ending_save(sample_file());
    let out = run(&mut d, &mut card, &[NONE, CIRCLE]);
    assert_eq!((&out[1].1, &out[1].2), (&CardOutcome::NotSaved, &vec![SFX_CANCEL]));
    // No card: the acknowledge goes back to "Save?" (box kept on the press).
    let mut card = TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 };
    let mut d = CardDialog::ending_save(sample_file());
    // (The first update only shows "Save?"; its pad is read from the next.)
    let out = run(&mut d, &mut card, &[NONE, CROSS, NONE, CROSS, NONE]);
    assert_eq!(out[2].0, Some(CardBox::Acknowledge(msg::NO_CARD)));
    assert_eq!((out[3].0, &out[3].2), (Some(CardBox::Acknowledge(msg::NO_CARD)), &vec![SFX_CONFIRM]));
    assert_eq!(out[4].0, Some(CardBox::YesNo { text: msg::SAVE_ASK, yes: true }));
    // A save on the card: the overwrite question defaults to NO; NO shows
    // YES on the press frame and returns to "Save?".
    let mut card = TestCard::formatted(Some(sample_file()));
    let mut d = CardDialog::ending_save(sample_file());
    let mut pads = vec![NONE, CROSS];
    pads.extend(vec![NONE; 7]);
    pads.push(CROSS);
    pads.push(NONE);
    let out = run(&mut d, &mut card, &pads);
    assert_eq!(out[7].0, Some(CardBox::Busy(msg::CHECKING)));
    assert_eq!(out[8].0, Some(CardBox::YesNo { text: msg::OVERWRITE_ASK, yes: false }));
    assert_eq!(out[9].0, Some(CardBox::YesNo { text: msg::OVERWRITE_ASK, yes: true }));
    assert_eq!(out[10].0, Some(CardBox::YesNo { text: msg::SAVE_ASK, yes: true }));
    assert_eq!(card.writes, 0);
}

/// Replays the emulated op 2 runs (ROSES_ENDING_CAPTURES = the folder of
/// `op2-*.json`, private): box, SE and 0x1ABB20's return every update.
#[test]
fn private_the_ending_save_matches_the_emulated_original() {
    let Some(dir) = std::env::var_os("ROSES_ENDING_CAPTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let existing = sample_file();
    let blank = |status| TestCard { status, file: None, writes: 0, formats: 0 };
    let mut checked = 0;
    for (name, card) in [
        ("op2-fresh", TestCard::formatted(None)),
        ("op2-no", TestCard::formatted(None)),
        ("op2-circle", TestCard::formatted(None)),
        ("op2-save-square", TestCard::formatted(None)),
        ("op2-nocard", blank(CardStatus::Missing)),
        ("op2-nocard-circle", blank(CardStatus::Missing)),
        ("op2-unformatted", blank(CardStatus::Unformatted)),
        ("op2-unformatted-no", blank(CardStatus::Unformatted)),
        ("op2-unformatted-yes", blank(CardStatus::Unformatted)),
        ("op2-full", blank(CardStatus::Formatted { free_kb: 249 })),
        ("op2-exists", TestCard::formatted(Some(existing.clone()))),
        ("op2-exists-no", TestCard::formatted(Some(existing.clone()))),
        ("op2-exists-circle", TestCard::formatted(Some(existing.clone()))),
        ("op2-exists-yes", TestCard::formatted(Some(existing.clone()))),
    ] {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.json"))) else {
            continue;
        };
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut card = card;
        let mut d = CardDialog::ending_save(sample_file());
        for f in v["frames"].as_array().unwrap() {
            let u = f["u"].as_i64().unwrap();
            let p = f["pad"].as_u64().unwrap();
            let pad = CardPad {
                left: p & 0x8000 != 0,
                right: p & 0x2000 != 0,
                cross: p & 0x40 != 0,
                circle: p & 0x20 != 0,
                square: p & 0x80 != 0,
                triangle: p & 0x10 != 0,
            };
            let outcome = d.update(pad, &mut card);
            let b = f["box"].as_array().unwrap();
            let (state, text, d35) = (b[0].as_u64().unwrap(), b[3].as_u64().unwrap() as u16, b[4].as_u64().unwrap());
            let want = match state {
                0x14 => Some(CardBox::Busy(text)),
                0x15 => Some(CardBox::Acknowledge(text)),
                0x16 => Some(CardBox::YesNo { text, yes: d35 != 0 }),
                _ => None,
            };
            assert_eq!(d.overlay(), want, "{name} u{u}");
            let se: Vec<u16> = f["se"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u16).collect();
            assert_eq!(d.sfx, se, "{name} u{u} SE");
            let ret = match outcome {
                CardOutcome::Running => 0,
                CardOutcome::Saved => 1,
                CardOutcome::NotSaved => 2,
                CardOutcome::Closed => -1,
                other => panic!("{name} u{u}: {other:?}"),
            };
            assert_eq!(f["ret"].as_i64().unwrap(), ret, "{name} u{u} return");
        }
        checked += 1;
    }
    assert!(checked > 0, "no op2 capture found in {}", dir.display());
}

/// Op 3 (the title's Custom Duel) with each slot-2 state: the boxes per
/// update and the result word [emu op3-nocard2 / op3-empty /
/// op3-unformatted / op3-save].
#[test]
fn custom_duel_op3_checks_slot_two_after_loading_slot_one() {
    let busy = |t| Some(CardBox::Busy(t));
    let slot1 = sample_file();
    for (slot2, checking, loading, result) in [
        (TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 }, 1, 0, op3::NO_CARD),
        (TestCard::formatted(None), 2, 0, op3::NO_SAVE),
        (TestCard { status: CardStatus::Unformatted, file: None, writes: 0, formats: 0 }, 3, 0, op3::NO_SAVE),
        (TestCard::formatted(Some(sample_file())), 3, 4, op3::SECOND_SAVE),
        (TestCard::formatted(Some({ let mut b = sample_file(); b[0x10] ^= 0xFF; b })), 3, 4, op3::NO_SAVE),
    ] {
        let mut card = TestCard::formatted(Some(slot1.clone()));
        let mut slot2 = slot2;
        let mut d = CardDialog::custom_duel();
        let mut boxes = Vec::new();
        let outcome = loop {
            let pad = if matches!(d.overlay(), Some(CardBox::Acknowledge(_))) { CROSS } else { NONE };
            let o = d.update_slots(pad, &mut card, &mut slot2);
            if o != CardOutcome::Running {
                break o;
            }
            boxes.push(d.overlay());
            assert!(boxes.len() < 40);
        };
        let mut want = vec![busy(msg::CHECKING); 2];
        want.extend(vec![busy(msg::LOADING); 4]);
        want.extend(vec![busy(msg::CHECKING_SLOT2); checking]);
        want.extend(vec![busy(msg::LOADING_SLOT2); loading]);
        want.push(busy(msg::LOAD_COMPLETE));
        want.push(Some(CardBox::Acknowledge(msg::LOAD_COMPLETE)));
        assert_eq!(boxes, want, "result {result:#x}");
        match outcome {
            CardOutcome::CustomDuel { slot1: s1, result: r, slot2: s2 } => {
                assert_eq!(s1, slot1);
                assert_eq!(r, result);
                assert_eq!(s2.is_some(), result == op3::SECOND_SAVE);
            }
            other => panic!("{other:?}"),
        }
    }
}

/// Op 3 against the interpreter runs of the original (`ROSES_OP3_CAPTURES` =
/// `work/research/modes/other-modes/captures`): box state and text
/// per frame, the result word.
#[test]
fn private_op3_matches_the_emulated_original() {
    let Some(dir) = std::env::var_os("ROSES_OP3_CAPTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let corrupt = { let mut b = sample_file(); b[0x10] ^= 0xFF; b };
    for (name, slot2) in [
        ("op3-nocard2", TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 }),
        ("op3-empty", TestCard::formatted(None)),
        ("op3-unformatted", TestCard { status: CardStatus::Unformatted, file: None, writes: 0, formats: 0 }),
        ("op3-save", TestCard::formatted(Some(sample_file()))),
        ("op3-corrupt", TestCard::formatted(Some(corrupt.clone()))),
    ] {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        let mut card = TestCard::formatted(Some(sample_file()));
        let mut slot2 = slot2;
        let mut d = CardDialog::custom_duel();
        for f in v["frames"].as_array().unwrap() {
            let p = f["pad"].as_u64().unwrap();
            let pad = CardPad { cross: p & 0x40 != 0, circle: p & 0x20 != 0, square: p & 0x80 != 0, triangle: p & 0x10 != 0, left: p & 0x8000 != 0, right: p & 0x2000 != 0 };
            let o = d.update_slots(pad, &mut card, &mut slot2);
            let b = f["box"].as_array().unwrap();
            let frame = f["frame"].as_i64().unwrap();
            if o != CardOutcome::Running {
                assert_eq!(f["ret"].as_i64(), Some(1), "{name} f{frame}");
                let CardOutcome::CustomDuel { result, .. } = o else { panic!("{name}: {o:?}") };
                assert_eq!(u64::from(result), f["res"].as_u64().unwrap(), "{name} result");
                break;
            }
            let (state, text) = (b[0].as_u64().unwrap(), b[3].as_u64().unwrap() as u16);
            let want = match state {
                0x14 => Some(CardBox::Busy(text)),
                0x15 => Some(CardBox::Acknowledge(text)),
                _ => None,
            };
            assert_eq!(d.overlay(), want, "{name} f{frame}");
        }
    }
}

/// Ops 4 / 5 (Trade) against the interpreter runs of the game's card module
/// (`ROSES_TRADE_CARD_CAPTURES` = `modes/two-player/captures`: `op4-*` with
/// slot 2 none / empty / unformatted / save / corrupt and slot 1 none /
/// empty, `op45-save` / `op45-pulled` = op 4 then op 5): every update's box
/// and the return values.
#[test]
fn private_op4_op5_match_the_emulated_original() {
    let Some(dir) = std::env::var_os("ROSES_TRADE_CARD_CAPTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let corrupt = { let mut b = sample_file(); b[0x10] ^= 0xFF; b };
    let none = || TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 };
    let unformatted = || TestCard { status: CardStatus::Unformatted, file: None, writes: 0, formats: 0 };
    let save = || TestCard::formatted(Some(sample_file()));
    let pad_of = |p: u64| CardPad { cross: p & 0x40 != 0, circle: p & 0x20 != 0, square: p & 0x80 != 0, triangle: p & 0x10 != 0, left: p & 0x8000 != 0, right: p & 0x2000 != 0 };
    let boxed = |b: &serde_json::Value| {
        let (state, text) = (b[0].as_u64().unwrap(), b[3].as_u64().unwrap() as u16);
        match state {
            0x14 => Some(CardBox::Busy(text)),
            0x15 => Some(CardBox::Acknowledge(text)),
            _ => None,
        }
    };
    let mut checked = 0;
    for (name, slot1, slot2) in [
        ("op4-none", save(), none()),
        ("op4-empty", save(), TestCard::formatted(None)),
        ("op4-unformatted", save(), unformatted()),
        ("op4-save", save(), save()),
        ("op4-corrupt", save(), TestCard::formatted(Some(corrupt.clone()))),
        ("op4-slot1none", none(), save()),
        ("op4-slot1empty", TestCard::formatted(None), save()),
    ] {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        let (mut card, mut slot2) = (slot1, slot2);
        let mut d = CardDialog::trade_load();
        for f in v["frames"].as_array().unwrap() {
            let o = d.update_slots(pad_of(f["pad"].as_u64().unwrap()), &mut card, &mut slot2);
            let frame = f["frame"].as_i64().unwrap();
            if o != CardOutcome::Running {
                match f["ret"].as_i64() {
                    Some(1) => assert!(matches!(o, CardOutcome::Trade { .. }), "{name}: {o:?}"),
                    Some(2) => assert_eq!(o, CardOutcome::LoadFailed, "{name}"),
                    r => panic!("{name} f{frame}: ret {r:?}"),
                }
                checked += 1;
                break;
            }
            assert_eq!(d.overlay(), boxed(&f["box"]), "{name} f{frame}");
        }
    }
    for (name, removed) in [("op45-save", false), ("op45-pulled", true)] {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        let (mut card, mut slot2) = (save(), save());
        let mut d = CardDialog::trade_load();
        let mut op = 4;
        for f in v["frames"].as_array().unwrap() {
            if f["op"].as_u64() == Some(5) && op == 4 {
                op = 5;
                if removed {
                    slot2 = none();
                }
                d = CardDialog::trade_save(vec![1; 4], vec![2; 4]);
            }
            if op == 4 && d.finished() {
                continue;
            }
            let o = d.update_slots(pad_of(f["pad"].as_u64().unwrap()), &mut card, &mut slot2);
            let frame = f["frame"].as_i64().unwrap();
            if o != CardOutcome::Running {
                assert_eq!(f["ret"].as_i64(), Some(1), "{name} f{frame}");
                if op == 5 {
                    assert_eq!(o, CardOutcome::Saved, "{name}");
                    assert_eq!((card.writes, slot2.writes), if removed { (0, 0) } else { (1, 1) }, "{name}");
                    if !removed {
                        assert_eq!((card.file.clone(), slot2.file.clone()), (Some(vec![1; 4]), Some(vec![2; 4])));
                    }
                    checked += 1;
                    break;
                }
                continue;
            }
            assert_eq!(d.overlay(), boxed(&f["box"]), "{name} op {op} f{frame}");
        }
    }
    assert_eq!(checked, 9);
}

#[test]
fn trade_load_refuses_without_slot_2_and_saves_both_slots() {
    let mut card = TestCard::formatted(Some(sample_file()));
    let mut slot2 = TestCard { status: CardStatus::Missing, file: None, writes: 0, formats: 0 };
    let mut d = CardDialog::trade_load();
    let mut seen = Vec::new();
    for u in 0..12 {
        let o = d.update_slots(if u == 10 { CROSS } else { NONE }, &mut card, &mut slot2);
        seen.push(d.overlay());
        if o != CardOutcome::Running {
            assert_eq!(o, CardOutcome::LoadFailed);
            break;
        }
    }
    assert_eq!(seen[2], Some(CardBox::Busy(msg::LOADING_SLOTS)));
    assert_eq!(seen[6], Some(CardBox::Busy(msg::CHECKING_SLOT2)));
    assert_eq!(seen[7], Some(CardBox::Acknowledge(msg::NO_CARD_SLOT2)));
    let mut slot2 = TestCard::formatted(Some(sample_file()));
    let mut d = CardDialog::trade_save(vec![5; 3], vec![6; 3]);
    let mut outcome = CardOutcome::Running;
    for u in 0..12 {
        outcome = d.update_slots(if u == 10 { CROSS } else { NONE }, &mut card, &mut slot2);
        if u < 8 {
            assert_eq!(d.overlay(), Some(CardBox::Busy(msg::SAVING_SLOTS)), "u{u}");
        }
        if outcome != CardOutcome::Running {
            break;
        }
    }
    assert_eq!(outcome, CardOutcome::Saved);
    assert_eq!((card.file, slot2.file), (Some(vec![5; 3]), Some(vec![6; 3])));
}
