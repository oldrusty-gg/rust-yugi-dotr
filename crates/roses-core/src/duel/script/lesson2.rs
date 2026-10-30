//! Lesson 2 (controller 33, driver 0x1750A0, jump table 0x2BA730, 64
//! steps, decks 0x13 Exodia head / 0x14 Blue-Eyes, messages 3670–3709):
//! how a duel is decided. Every arm follows `steps-33.txt` in execution
//! order; the poll counts were checked against the interpreter run from
//! saved state 133 (`emu-33.txt`) and the board/LP after every setup
//! against saved states 137–186 (private tests in `tests.rs`) [F, emu, O].
//!
//! Keys are on side 0's turn with 14-poll waits; every setup is FADEOUT →
//! edits → wait 32 → FADEIN. Step 0 sets `NO_LP0_END` so LP 0 does not end
//! the duel (steps 11 and 25). Step 63 returns −1 on the poll its message
//! closes (0x17642C), like lesson 1's step 157.

use super::{pad, unit_flag as f, CameraOp, Ctx, Res, BLINK_POINTER};
use crate::duel::CardData;

// Cards (ids of the card table; text from the disc at runtime).
const CELTIC_GUARDIAN: u16 = 156;
const NEEDLE_BALL: u16 = 346;
const KAISER_DRAGON: u16 = 3;
const MONSTROUS_BIRD: u16 = 285;
const STEEL_OGRE_GROTTO_1: u16 = 522;
const ROCK_OGRE_GROTTO_2: u16 = 631;
const BABY_DRAGON: u16 = 21;
const DRAGON_CAPTURE_JAR: u16 = 683;
/// 0x3A, the leader of deck 0x13.
const EXODIA: u16 = 58;
/// The four limbs, in the order step 54 places them (57, 56, 55, 54).
const LIMBS: [u16; 4] = [57, 56, 55, 54];

const LABYRINTH: u8 = 8;
const NORMAL: u8 = 7;
/// The seven cells around (3,5) that steps 43/49 set (not (3,4)).
const RING_35: [(u8, u8); 7] = [(3, 6), (2, 6), (4, 6), (2, 5), (4, 5), (2, 4), (4, 4)];

const KEY_WAIT: u32 = 14;
const SETUP_WAIT: u32 = 32;
const BLINK_POLLS: u32 = 62;

pub(super) fn step<C: CardData>(ctx: &mut Ctx<'_, C>) -> Res {
    match ctx.step() {
        0 => {
            ctx.no_lp0_end();
            ctx.flag906(true);
            ctx.goto(2);
        }
        // Dead code in the original (never reached).
        1 => {
            ctx.model_ready(0)?;
            ctx.model_ready(0x100)?;
            ctx.next();
        }
        2 => {
            ctx.msg(3670, 3672)?;
            ctx.flag906(false);
        }
        3 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            ctx.lp(0, 4000);
            ctx.lp(1, 1000);
            ctx.place(0, CELTIC_GUARDIAN, 3, 1)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.next();
        }
        4 => {
            ctx.cursor_to(3, 1);
            ctx.model_ready(1)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        5 => {
            ctx.camera(CameraOp::CloseUp {
                card: CELTIC_GUARDIAN,
            });
            ctx.flag906(true);
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.next();
        }
        6 => ctx.msg(3673, 3673)?,
        7 => {
            ctx.blink_for(BLINK_POINTER, 0xF0, -0xC8, BLINK_POLLS)?;
            ctx.next();
        }
        8 => ctx.msg(3674, 3674)?,
        // Celtic Guardian's direct attack on the leader: LP1 1000 → 0, no
        // duel end [O 142].
        9 => key(ctx, pad::CROSS)?,
        10 => key(ctx, pad::UP)?,
        11 => key(ctx, pad::CROSS)?,
        12 => {
            ctx.msg(3675, 3676)?;
            ctx.flag906(false);
        }
        13 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            ctx.remove(0, 1);
            ctx.lp(0, 500);
            ctx.lp(1, 500);
            ctx.place(0, CELTIC_GUARDIAN, 3, 4)?;
            ctx.place(1, NEEDLE_BALL, 3, 3)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.next();
        }
        14 => {
            ctx.cursor_to(3, 4);
            ctx.model_ready(1)?;
            ctx.model_ready(0x101)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        15 => {
            ctx.camera(CameraOp::CloseUp {
                card: CELTIC_GUARDIAN,
            });
            ctx.flag906(true);
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.next();
        }
        16 => ctx.msg(3677, 3679)?,
        // Details view of the Needle Ball (Tri → 0x20/0x21; Down = page
        // [H]; Circle closes it), then the attack: both LP 0 [O 154].
        17 => key(ctx, pad::UP)?,
        18 => key(ctx, pad::TRIANGLE)?,
        19 => key(ctx, pad::DOWN)?,
        20 => ctx.msg(3680, 3682)?,
        21 => key(ctx, pad::CIRCLE)?,
        22 => key(ctx, pad::DOWN)?,
        23 => key(ctx, pad::CROSS)?,
        24 => key(ctx, pad::UP)?,
        25 => key(ctx, pad::CROSS)?,
        26 => {
            ctx.msg(3683, 3685)?;
            ctx.flag906(false);
        }
        27 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            ctx.remove(0, 1);
            ctx.remove(1, 1);
            ctx.teleport(0, 0, 3, 4)?;
            ctx.lp(0, 4000);
            ctx.lp(1, 4000);
            ctx.next();
        }
        28 => {
            ctx.cursor_to(3, 4);
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        29 => {
            ctx.flag906(true);
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.next();
        }
        30 => ctx.msg(3686, 3686)?,
        // Square at the leader shows the summon areas (state 0x3E).
        31 => key(ctx, pad::SQUARE)?,
        32 => ctx.msg(3687, 3687)?,
        // All eight neighbours of the leader (3,4) enemy-held; Circle
        // leaves the summon-cell state while black [O 159].
        33 => {
            ctx.fade_out(16)?;
            ctx.camera(CameraOp::Save);
            ctx.place(1, KAISER_DRAGON, 3, 3)?;
            ctx.place(1, MONSTROUS_BIRD, 2, 3)?;
            ctx.place(1, STEEL_OGRE_GROTTO_1, 4, 3)?;
            ctx.place(1, ROCK_OGRE_GROTTO_2, 2, 4)?;
            ctx.place(1, BABY_DRAGON, 4, 4)?;
            ctx.place(1, DRAGON_CAPTURE_JAR, 2, 5)?;
            ctx.place(1, DRAGON_CAPTURE_JAR, 3, 5)?;
            ctx.place(1, DRAGON_CAPTURE_JAR, 4, 5)?;
            for slot in 1..=5 {
                ctx.unit_flag(1, slot, f::COMMITTED_FACE_UP, true)?;
            }
            ctx.next();
            ctx.key(pad::CIRCLE);
        }
        34 => {
            for slot in 1..=5 {
                ctx.model_ready(0x100 | slot)?;
            }
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
            ctx.camera(CameraOp::Preset { value: 0 });
        }
        35 => {
            ctx.fade_in(16)?;
            ctx.next();
        }
        36 => {
            ctx.msg(3688, 3690)?;
            ctx.flag906(false);
        }
        // Leader to the corner (0,6), the monsters parked at column 6
        // [O 163].
        37 => {
            ctx.fade_out(8)?;
            ctx.remove(1, 6);
            ctx.remove(1, 7);
            ctx.remove(1, 8);
            for slot in 1..=5 {
                ctx.camera(CameraOp::UnitFx2 {
                    side: 1,
                    slot,
                    value: -1,
                });
                ctx.camera(CameraOp::UnitFx {
                    side: 1,
                    slot,
                    value: 0,
                });
            }
            for slot in 1..=5 {
                ctx.teleport(1, slot, 6, slot - 1)?;
            }
            ctx.teleport(0, 0, 0, 6)?;
            ctx.next();
        }
        38 => {
            ctx.cursor_to(0, 6);
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
            ctx.camera(CameraOp::Zoom);
            ctx.key(pad::SQUARE);
        }
        39 => {
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.next();
        }
        40 => ctx.msg(3691, 3691)?,
        // The three areas of the corner occupied [O 164].
        41 => {
            ctx.fade_out(16)?;
            for slot in 1..=3 {
                ctx.camera(CameraOp::UnitFx {
                    side: 1,
                    slot,
                    value: 0x80,
                });
                ctx.camera(CameraOp::UnitFx2 {
                    side: 1,
                    slot,
                    value: 1,
                });
            }
            ctx.teleport(1, 1, 0, 5)?;
            ctx.teleport(1, 2, 1, 5)?;
            ctx.teleport(1, 3, 1, 6)?;
            ctx.next();
        }
        42 => {
            ctx.fade_in(16)?;
            ctx.msg(3692, 3692)?;
            ctx.flag906(false);
        }
        // Leader at (3,5) inside a Labyrinth ring; Circle then Square show
        // the reduced areas [O 165].
        43 => {
            ctx.fade_out(16)?;
            ctx.teleport(1, 1, 6, 0)?;
            ctx.teleport(1, 2, 6, 1)?;
            ctx.teleport(1, 3, 6, 2)?;
            ctx.teleport(0, 0, 3, 5)?;
            for (c, r) in RING_35 {
                ctx.terrain(c, r, LABYRINTH)?;
            }
            ctx.next();
            ctx.key(pad::CIRCLE);
        }
        44 => {
            ctx.cursor_to(3, 5);
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
            ctx.key(pad::SQUARE);
        }
        45 => {
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.next();
        }
        46 => ctx.msg(3693, 3693)?,
        47 => {
            ctx.fade_out(16)?;
            ctx.teleport(1, 1, 3, 4)?;
            ctx.next();
        }
        48 => {
            ctx.fade_in(16)?;
            ctx.msg(3694, 3694)?;
            ctx.flag906(false);
        }
        // Cleared board, leader home, round 99 for the turn-limit lesson
        // [O 167].
        49 => {
            ctx.fade_out(8)?;
            for slot in 1..=5 {
                ctx.remove(1, slot);
            }
            ctx.teleport(0, 0, 3, 6)?;
            for (c, r) in RING_35 {
                ctx.terrain(c, r, NORMAL)?;
            }
            ctx.round(99);
            ctx.next();
            ctx.key(pad::CIRCLE);
        }
        50 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
            ctx.cursor_to(3, 6);
        }
        51 => {
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.msg(3695, 3695)?;
        }
        52 => {
            ctx.blink_for(BLINK_POINTER, 0x10, -0xD2, BLINK_POLLS)?;
            ctx.next();
        }
        53 => {
            ctx.msg(3696, 3699)?;
            ctx.flag906(false);
        }
        // Exodia: the limbs at the far corners, face-up [O 172].
        54 => {
            ctx.fade_out(8)?;
            ctx.place(0, LIMBS[0], 6, 0)?;
            ctx.place(0, LIMBS[1], 5, 0)?;
            ctx.place(0, LIMBS[2], 0, 0)?;
            ctx.place(0, LIMBS[3], 1, 0)?;
            ctx.round(0);
            for slot in 1..=4 {
                ctx.unit_flag(0, slot, f::COMMITTED_FACE_UP, true)?;
            }
            ctx.next();
        }
        55 => {
            for slot in 1..=4 {
                ctx.model_ready(slot)?;
            }
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        56 => {
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.msg(3700, 3704)?;
            ctx.flag906(false);
        }
        57 => {
            ctx.fade_out(16)?;
            ctx.camera(CameraOp::Save);
            ctx.camera(CameraOp::CloseUp { card: EXODIA });
            ctx.next();
        }
        58 => {
            ctx.fade_in(16)?;
            ctx.caption(EXODIA);
            ctx.wait(BLINK_POLLS)?;
            ctx.next();
        }
        59 => {
            ctx.caption_off();
            ctx.msg(3705, 3705)?;
        }
        // The limbs around the leader [O 179]; Start ends side 0's turn on
        // the poll the message closes, step 62 ends side 1's turn at its
        // first poll; side 0's turn start finds Exodia complete, which is
        // an event only for a scripted side [F].
        60 => {
            ctx.fade_out(16)?;
            ctx.teleport(0, 1, 2, 6)?;
            ctx.teleport(0, 2, 4, 6)?;
            ctx.teleport(0, 3, 2, 5)?;
            ctx.teleport(0, 4, 4, 5)?;
            ctx.camera(CameraOp::Restore);
            ctx.next();
        }
        61 => {
            ctx.fade_in(16)?;
            ctx.msg(3706, 3708)?;
            ctx.flag906(false);
            ctx.key(pad::START);
        }
        62 => {
            ctx.next();
            ctx.key(pad::START);
        }
        // −1 on the poll the message closes (0x17642C: `0x280C80(3709,
        // 3709, S) == 1` → return −1), as lesson 1's step 157 [F].
        63 => {
            ctx.msg(3709, 3709)?;
            ctx.end_lesson();
        }
        _ => ctx.end_lesson(),
    }
    Ok(())
}

/// `wait 14 polls ; next ; KEY k`.
fn key<C: CardData>(ctx: &mut Ctx<'_, C>, k: u16) -> Res {
    ctx.wait(KEY_WAIT)?;
    ctx.next();
    ctx.key(k);
    Ok(())
}
