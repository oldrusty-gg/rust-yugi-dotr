//! Ordinary tests with synthetic flag tables. The numbers come from the
//! interpreter runs of `claude-end/run2.py` and `run3.py` on saved state
//! 767 (player vs Weevil, RNG state 0x11054A74C1E17A22 at the duel's end,
//! no LCG call in the result states); the private test at the end reads
//! the real flag bytes from the executable.

use super::*;

/// The LCG state of saved state 767 when mode 0xF starts [emu].
const RNG_767: u64 = 0x1105_4A74_C1E1_7A22;
/// Weevil's graveyard at 767 in removal order, and his deck leader.
const GRAVE_767: [u16; 4] = [817, 406, 757, 809];
const LEADER_767: u16 = 397;
const STRIPS_767: [[u16; 8]; 3] = [
    [817, 406, 757, 809, 397, 820, 820, 820],
    [406, 397, 820, 809, 817, 820, 820, 757],
    [406, 820, 757, 809, 817, 397, 820, 820],
];

fn flags(ineligible: &[u16], pool: &[u16]) -> RewardCardTable {
    RewardCardTable {
        reel_eligible: (0..CARD_COUNT).map(|c| !ineligible.contains(&c)).collect(),
        special_pool: (0..CARD_COUNT).map(|c| pool.contains(&c)).collect(),
    }
}

fn input_767() -> RewardListInput {
    RewardListInput {
        loser_graveyard: GRAVE_767.to_vec(),
        loser_deck_leader: LEADER_767,
        // Airknight Parshath 392: type 8, stored rank 1.
        winner_leader_kind: 8,
        winner_leader_rank: Some(1),
        winner_special_draw_announced: false,
    }
}

fn after_calls(mut state: u64, calls: usize) -> u64 {
    for _ in 0..calls {
        next_rand31(&mut state);
    }
    state
}

/// A grant that draws the LCG once per card, like 0x2157D0's instance key.
#[derive(Default)]
struct KeyGrant(Vec<(Prize, u32)>);
impl PrizeGrant for KeyGrant {
    fn grant(&mut self, prize: Prize, rng: &mut u64) {
        let key = next_rand31(rng);
        self.0.push((prize, key));
    }
}

/// Runs the machine with stop presses at the given reel-mode frames and a
/// Cross press on every third frame while a popup is up (as run2.py does).
fn replay(
    machine: &mut SlotMachine,
    rng: &mut u64,
    presses: [Option<u16>; 3],
    grant: &mut dyn PrizeGrant,
) -> Vec<(u16, SlotEvent)> {
    let mut log = Vec::new();
    for f in 0..2000u16 {
        let mut pressed = 0;
        for (r, p) in presses.iter().enumerate() {
            if *p == Some(f) {
                pressed |= buttons::STOP[r];
            }
        }
        if machine.screen == Screen::Prizes
            && machine.prizes.phase == PrizePhase::Popup
            && f % 3 == 0
        {
            pressed |= buttons::CROSS;
        }
        for e in machine.step(pressed, rng, grant) {
            log.push((f, e));
        }
        if machine.finished {
            break;
        }
    }
    log
}

#[test]
fn reel_list_appends_the_leader_filters_and_pads() {
    let t = flags(&[409], &[]);
    let data = build_reel_data(&input_767(), &t).unwrap();
    assert_eq!(data.strip, STRIPS_767[0]);
    assert!(data.random_slip);
    // Type 8 has the ability; rank 1 pays line 0 only.
    assert_eq!(data.enabled_lines, [true, false, false, false, false]);

    // An ineligible card is dropped from the middle, keeping the order.
    let mut input = input_767();
    input.loser_graveyard = vec![817, 409, 406];
    let data = build_reel_data(&input, &t).unwrap();
    assert_eq!(data.strip, [817, 406, 397, 820, 820, 820, 820, 820]);

    // Nothing to spin for: no reels.
    input.loser_graveyard = vec![409];
    input.loser_deck_leader = REJECTED_CARD;
    assert_eq!(build_reel_data(&input, &t), None);
    input.loser_graveyard = Vec::new();
    assert_eq!(build_reel_data(&input, &t), None);

    // A full graveyard refuses the leader; 8..=15 cards stay as they are;
    // more than 16 are cut to the first 16.
    input.loser_graveyard = (1..=GRAVEYARD_CAPACITY as u16).collect();
    input.loser_deck_leader = LEADER_767;
    let data = build_reel_data(&input, &t).unwrap();
    assert_eq!(data.strip, (1..=16).collect::<Vec<u16>>());
    input.loser_graveyard = (1..=10).collect();
    input.loser_deck_leader = REJECTED_CARD;
    assert_eq!(build_reel_data(&input, &t).unwrap().strip.len(), 10);
    input.loser_deck_leader = 500;
    assert_eq!(build_reel_data(&input, &t).unwrap().strip.len(), 11);
}

#[test]
fn lines_follow_the_winner_leader_type_and_rank() {
    let t = flags(&[], &[]);
    let lines = |kind: u8, rank: Option<u8>, announced: bool| {
        let mut input = input_767();
        input.winner_leader_kind = kind;
        input.winner_leader_rank = rank;
        input.winner_special_draw_announced = announced;
        let d = build_reel_data(&input, &t).unwrap();
        (d.enabled_lines, d.random_slip)
    };
    // Weevil's Basic Insect (type 9): no ability, whatever the rank.
    assert_eq!(
        lines(9, Some(12), false).0,
        [true, false, false, false, false]
    );
    assert_eq!(
        lines(0x29, Some(12), false).0,
        [true, false, false, false, false]
    );
    assert_eq!(
        lines(8, Some(4), false).0,
        [true, true, false, false, false]
    );
    assert_eq!(lines(8, Some(7), false).0, [true, true, true, false, false]);
    assert_eq!(lines(8, Some(10), false).0, [true, true, true, true, false]);
    assert_eq!(lines(8, Some(11), false).0, [true; 5]);
    // Rank 0 / no rank turn line 0 off in the buffer (0x284E60 forces it).
    assert_eq!(lines(8, Some(0), false).0, [false; 5]);
    assert_eq!(lines(8, None, false).0, [false; 5]);
    assert!(!lines(8, Some(1), true).1);
    let d = ReelData {
        strip: vec![1; 8],
        random_slip: false,
        enabled_lines: [false; 5],
    };
    let (m, _) = SlotMachine::new(&d, Vec::new(), &mut 1).unwrap();
    assert_eq!(m.enabled_lines, 1);
}

#[test]
fn init_reproduces_the_emu_767_strips() {
    let t = flags(&[409], &[]);
    let data = build_reel_data(&input_767(), &t).unwrap();
    let mut rng = RNG_767;
    let (m, events) = SlotMachine::new(&data, Vec::new(), &mut rng).unwrap();
    assert_eq!(events, [SlotEvent::SpinStarted]);
    for (reel, strip) in m.reels.iter().zip(STRIPS_767) {
        assert_eq!(reel.strip, strip);
        assert_eq!(reel.state, ReelState::Accelerating);
    }
    assert_eq!(rng, after_calls(RNG_767, 64));
    assert_eq!(
        (m.screen, m.count, m.cycle, m.live_lines),
        (Screen::Spinning, 8, 1024, 1)
    );
    assert!(matches!(
        SlotMachine::new(
            &ReelData {
                strip: Vec::new(),
                ..data.clone()
            },
            Vec::new(),
            &mut rng
        ),
        Err(SlotMachineError::InvalidStrip(0))
    ));
    assert!(matches!(
        SlotMachine::new_test_mode(),
        Err(SlotMachineError::Unsupported(_))
    ));
}

#[test]
fn reels_accelerate_to_full_speed_in_32_frames() {
    let data = build_reel_data(&input_767(), &flags(&[409], &[])).unwrap();
    let mut rng = RNG_767;
    let (mut m, _) = SlotMachine::new(&data, Vec::new(), &mut rng).unwrap();
    let mut sink = Vec::new();
    for f in 0..33 {
        m.step(0, &mut rng, &mut sink);
        if f == 0 {
            assert_eq!((m.reels[0].pos, m.reels[0].speed), (1, 1));
        }
    }
    // run2.py f32: state 2, pos 560, speed 32; no LCG use while spinning.
    for reel in &m.reels {
        assert_eq!(
            (reel.state, reel.pos, reel.speed),
            (ReelState::Spinning, 560, 32)
        );
    }
    assert_eq!(m.frame, 33);
    assert_eq!(rng, after_calls(RNG_767, 64));
    // A stop press is only taken while spinning: none during acceleration.
    let (mut m2, _) = SlotMachine::new(&data, Vec::new(), &mut rng.clone()).unwrap();
    m2.step(buttons::SQUARE, &mut rng.clone(), &mut sink);
    assert_eq!(m2.reels[0].state, ReelState::Accelerating);
}

#[test]
fn run2_presses_at_60_80_100_give_slips_and_prizes_820_809_406() {
    let data = build_reel_data(&input_767(), &flags(&[409], &[])).unwrap();
    let mut rng = RNG_767;
    let (mut m, _) = SlotMachine::new(&data, Vec::new(), &mut rng).unwrap();
    let mut grant = KeyGrant::default();
    let log = replay(
        &mut m,
        &mut rng,
        [Some(60), Some(80), Some(100)],
        &mut grant,
    );
    let stopping: Vec<_> = log
        .iter()
        .filter_map(|(f, e)| match e {
            SlotEvent::ReelStopping {
                reel,
                counter,
                slip,
            } => Some((*f, *reel, *counter, *slip)),
            _ => None,
        })
        .collect();
    assert_eq!(stopping, [(60, 0, 12, 1), (80, 1, 12, 1), (100, 2, 12, 1)]);
    let stopped: Vec<_> = log
        .iter()
        .filter_map(|(f, e)| match e {
            SlotEvent::ReelStopped {
                reel,
                rows,
                live_lines,
            } => Some((*f, *reel, *rows, *live_lines)),
            _ => None,
        })
        .collect();
    assert_eq!(
        stopped,
        [
            (71, 0, [820, 820, 820], 0),
            (91, 1, [820, 809, 817], 0),
            (111, 2, [820, 406, 820], 0),
        ]
    );
    assert_eq!(
        m.reels.iter().map(|r| (r.pos, r.card)).collect::<Vec<_>>(),
        [(640, 820), (256, 809), (896, 406)]
    );
    let rest: Vec<_> = log
        .iter()
        .filter(|(_, e)| {
            !matches!(
                e,
                SlotEvent::ReelStopping { .. } | SlotEvent::ReelStopped { .. }
            )
        })
        .cloned()
        .collect();
    let prize = |kind, card, order| SlotEvent::Prize(Prize { kind, card, order });
    assert_eq!(
        rest,
        [
            (112, SlotEvent::AllStopped { paying_lines: 0 }),
            (113, SlotEvent::PrizeScreen),
            (130, prize(PrizeKind::GetCard, 820, 0)),
            (132, prize(PrizeKind::GetCard, 809, 1)),
            (135, prize(PrizeKind::GetCard, 406, 2)),
            (138, SlotEvent::PopupClosed),
            (148, SlotEvent::Finished),
        ]
    );
    // 64 shuffle + 3 slip + 3 grant calls; the grant keys are the emu's
    // values at 0x215928.
    assert_eq!(rng, after_calls(RNG_767, 70));
    assert_eq!(
        grant
            .0
            .iter()
            .map(|(p, key)| (p.card, *key))
            .collect::<Vec<_>>(),
        [(820, 642670066), (809, 604314330), (406, 979881194)]
    );
    assert!(m.step(0, &mut rng, &mut grant).is_empty());
}

#[test]
fn forced_middle_line_pays_the_reel_card_and_a_special_card() {
    let pool: Vec<u16> = (100..122).collect();
    let mut data = build_reel_data(&input_767(), &flags(&[409], &[])).unwrap();
    data.random_slip = false;
    // Without the slip every reel follows the same path; find the frame at
    // which one press stops it with 809 (strip index 3) in the middle row.
    let mut rng = RNG_767;
    let (m0, _) = SlotMachine::new(&data, pool.clone(), &mut rng).unwrap();
    let rng_after_init = rng;
    let mut sink = Vec::new();
    let press_at = (32..200u16)
        .find(|&f| {
            let mut m = m0.clone();
            let mut r = rng_after_init;
            for frame in 0..f + 40 {
                let pressed = if frame == f { buttons::SQUARE } else { 0 };
                m.step(pressed, &mut r, &mut sink);
            }
            m.reels[0].state == ReelState::Stopped && m.reels[0].pos == 256
        })
        .expect("a press frame landing on 809");
    let mut m = m0;
    let mut grant = KeyGrant::default();
    let log = replay(&mut m, &mut rng, [Some(press_at); 3], &mut grant);
    let stopped_frame = log
        .iter()
        .find_map(|(f, e)| matches!(e, SlotEvent::ReelStopped { reel: 2, .. }).then_some(*f))
        .unwrap();
    // No slip: counter 5..=9, so the stop lands that many frames after the
    // press (the press frame's draw already counts one down).
    let counter = log
        .iter()
        .find_map(|(_, e)| match e {
            SlotEvent::ReelStopping {
                reel: 2,
                counter,
                slip,
            } => Some((*counter, *slip)),
            _ => None,
        })
        .unwrap();
    assert_eq!(counter.1, 0);
    assert!((5..=9).contains(&counter.0));
    assert_eq!(stopped_frame, press_at + u16::from(counter.0) - 1);
    for reel in &m.reels {
        assert_eq!((reel.pos, reel.card, reel.rows()[1]), (256, 809, 809));
    }
    assert_eq!(m.reels[0].rows(), [757, 809, 397]);
    let special = pool[(1230758800u32 % pool.len() as u32) as usize];
    let prize = |kind, card, order| SlotEvent::Prize(Prize { kind, card, order });
    let f0 = stopped_frame + 1;
    let rest: Vec<_> = log
        .iter()
        .filter(|(_, e)| {
            !matches!(
                e,
                SlotEvent::ReelStopping { .. } | SlotEvent::ReelStopped { .. }
            )
        })
        .cloned()
        .collect();
    let popup1 = f0 + 1 + 15 + 17;
    let popup2 = popup1 + 1 + (3 - (popup1 + 1) % 3) % 3;
    let closed = popup2 + 3;
    assert_eq!(
        rest,
        [
            (f0, SlotEvent::AllStopped { paying_lines: 1 }),
            (f0 + 1, SlotEvent::LineFlash { line: 0 }),
            (f0 + 1 + 15, SlotEvent::PrizeScreen),
            (popup1, prize(PrizeKind::GetCard, 809, 0)),
            (popup2, prize(PrizeKind::GetSpecialCard, special, 3)),
            (closed, SlotEvent::PopupClosed),
            (closed + 10, SlotEvent::Finished),
        ]
    );
    assert_eq!(m.flash.lines, [1, 0, 0, 0, 0]);
    assert_eq!(m.flash.cells, [[0, 1, 0]; 3]);
    // 64 shuffle + grant 809 + the line's pool draw (rand #66 =
    // 1230758800 in the emu) + grant of the special card.
    assert_eq!(rng, after_calls(RNG_767, 67));
    assert_eq!(grant.0.len(), 2);
    assert_eq!(grant.0[0].1, 2028204319);
}

#[test]
fn a_filler_or_a_mismatch_kills_a_line_as_reels_stop() {
    let data = ReelData {
        strip: vec![1, 2, 3, 4, 5, 6, 7, 8],
        random_slip: false,
        enabled_lines: [true; 5],
    };
    let mut rng = 5;
    let (mut m, _) = SlotMachine::new(&data, Vec::new(), &mut rng).unwrap();
    // Stop the reels by hand, one after the other, every reel at top index 0.
    for reel in &mut m.reels {
        reel.strip = vec![1, 2, 3, 4, 5, 6, 7, 8];
        reel.pos = 0;
    }
    m.reels[2].strip[1] = FILLER_CARD; // middle of reel 2 is the filler
    m.reels[1].strip[2] = 9; // bottom of reel 1 differs
    m.reels[0].state = ReelState::Stopped;
    m.evaluate_lines(0);
    assert_eq!(m.live_lines, 0x1F);
    m.reels[1].state = ReelState::Stopped;
    m.evaluate_lines(1);
    // Line 1 (bottom row) dies: 9 ≠ 3. Line 3 (reel 1 row 1 = 2 vs reel 0
    // row 2 = 3) and line 4 (2 vs 1) die too; line 0 (2 = 2) and line 2
    // (1 = 1) live.
    assert_eq!(m.live_lines, 0b00101);
    m.reels[2].state = ReelState::Stopped;
    m.evaluate_lines(2);
    // Line 0 dies on the filler; line 2 lives (1 = 1 = 1).
    assert_eq!(m.live_lines, 0b00100);
    assert_eq!(m.reels[2].card, FILLER_CARD);
}

#[test]
fn special_prizes_map_fixed_pairs_or_draw_from_the_pool() {
    let mut rng = 7;
    for (from, to) in SPECIAL_MAP {
        assert_eq!(special_prize(from, &[1, 2, 3], &mut rng), to);
    }
    assert_eq!(rng, 7);
    assert_eq!(special_prize(809, &[], &mut rng), NO_PRIZE);
    assert_eq!(rng, 7);
    let pool: Vec<u16> = (100..122).collect();
    let mut expect = 7;
    let v = next_rand31(&mut expect);
    assert_eq!(special_prize(809, &pool, &mut rng), pool[(v % 22) as usize]);
    assert_eq!(rng, expect);

    let t = flags(&[], &[0, 7, 83, 87, 733, REJECTED_CARD, 819, 853]);
    assert_eq!(special_pool(&t), [0, 7, 819, 853]);
}

#[test]
#[ignore = "requires the private PAL executable; set ROSES_TEST_ELF_PATH"]
fn private_special_pool_from_the_executable_has_22_cards() {
    // Test-only shortcut: byte 0xB of the 20-byte records at file offset
    // 0x1C4D80 (EE 0x2BE400); the authenticated parser is
    // roses_assets::parse_pal_reward_flags.
    let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").expect("ROSES_TEST_ELF_PATH"))
        .unwrap();
    let table = &elf[0x1C4D80..0x1C4D80 + 854 * 20];
    let t = RewardCardTable::from_record_flag_bytes(table.chunks_exact(20).map(|r| r[0xB]));
    assert!(!t.reel_eligible(409));
    let pool = special_pool(&t);
    assert_eq!(pool.len(), 22);
    assert_eq!(pool[18], 791);
    // run3.py: 0x252E10(809) with rand 1230758800 → 791.
    let mut rng = after_calls(RNG_767, 65);
    assert_eq!(special_prize(809, &pool, &mut rng), 791);
    assert_eq!(rng, after_calls(RNG_767, 66));
}
