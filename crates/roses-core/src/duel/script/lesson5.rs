//! Lesson 5 (controller 36, driver 0x17B340, jump table 0x2BB1E0, 69
//! steps, decks 0x19 / 0x1A, messages 4047–4086): the duel's functions —
//! the hand view (Square on the leader after the summon, 0x53), the
//! graveyard view (R1, 0x2D without the pick flag) with its details, the
//! settings panel (L1, 0x29: the two rows Monster Battle and Sound; the
//! card-name messages 4062–4065 are dead code, step 35 jumps over them),
//! the L2 display toggle and the camera sticks (the script writes pad 0's
//! analog bytes, 0x2035B0; presentation only).
//!
//! Every arm follows `steps-36.txt` in execution order; the shared
//! handlers are 0x17B74C (wait 8, key), 0x17B6C4 (wait 14, key), 0x17B444
//! (message), 0x17B8AC (FADEOUT 16), 0x17B91C (wait 32), 0x17B944 (FADEIN
//! 16), 0x17B9B8 (blink) and the stick steps 0x17BA48–0x17BBD0, which count
//! S+0x162 instead of the step counter [F]. The board and sub-states were
//! checked against a run of the original from the map's tutorial menu
//! (`modes/tutorial45`, [emu]).
//!
//! The duel starts with both graveyards filled by 0x280610 (0x1E00F0:
//! side 0 = 0, 3, 6, 700, side 1 = 0, 3, 6, 699) [F]. Step 68's message
//! closes into step 69 = the count: the next poll returns −1.

use super::{pad, Ctx, Res, BLINK_POINTER};
use crate::duel::CardData;

/// 0x280610: the graveyards the lesson starts with [F].
pub(super) const GRAVEYARDS: [[u16; 4]; 2] = [[0, 3, 6, 700], [0, 3, 6, 699]];

const KEY_WAIT: u32 = 14;
const SHORT_WAIT: u32 = 8;
const SETUP_WAIT: u32 = 32;
const BLINK_POLLS: u32 = 62;

/// Pad-0 analog byte indices of 0x2035B0(0, i, v) (+0x206 + i): 0/1 the
/// right stick X/Y, 2/3 the left stick X/Y [H: from the messages: the left
/// stick rotates (bytes 2/3), the right stick zooms (byte 1)].
const RIGHT_Y: usize = 1;
const LEFT_X: usize = 2;
const LEFT_Y: usize = 3;

pub(super) fn step<C: CardData>(ctx: &mut Ctx<'_, C>) -> Res {
    match ctx.step() {
        0 => ctx.goto(2),
        // Dead code in the original (never reached).
        1 => {
            ctx.model_ready(0)?;
            ctx.model_ready(0x100)?;
            ctx.goto(53);
        }
        2 => ctx.msg(4047, 4048)?,
        // A summon at (3,5): the Celtic Guardian (hand index 2) [emu].
        3..=9 => short(
            ctx,
            [
                pad::SQUARE,
                pad::UP,
                pad::CROSS,
                pad::RIGHT,
                pad::RIGHT,
                pad::CROSS,
                pad::CROSS,
            ][usize::from(ctx.step() - 3)],
        )?,
        10 => ctx.msg(4049, 4050)?,
        // Down to the leader, Square: the hand view (0x51 → 0x53).
        11 => short(ctx, pad::DOWN)?,
        12 => short(ctx, pad::SQUARE)?,
        13 => {
            ctx.msg(4051, 4051)?;
            ctx.flag906(false);
            ctx.key(pad::CIRCLE);
        }
        14 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 1);
        }
        15 | 30 | 44 | 52 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        16 | 31 | 45 | 53 => {
            ctx.fade_in(16)?;
            ctx.flag906(true);
            ctx.next();
        }
        17 => ctx.msg(4052, 4053)?,
        // R1: the graveyard view; Right/Left switch the shown side.
        18 => short(ctx, pad::R1)?,
        19 => ctx.msg(4054, 4055)?,
        20 | 22 => key(ctx, pad::RIGHT)?,
        21 | 23 => key(ctx, pad::LEFT)?,
        24 => {
            ctx.wait(KEY_WAIT)?;
            ctx.next();
        }
        25 => ctx.msg(4056, 4056)?,
        26 => short(ctx, pad::TRIANGLE)?,
        27 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
            ctx.key(pad::CIRCLE);
        }
        28 | 42 => {
            let (first, last) = if ctx.step() == 28 {
                (4057, 4058)
            } else {
                (4068, 4070)
            };
            ctx.msg(first, last)?;
            ctx.flag906(false);
            ctx.key(pad::CIRCLE);
        }
        29 | 43 | 51 | 67 => {
            ctx.fade_out(16)?;
            ctx.next();
        }
        32 => ctx.msg(4059, 4060)?,
        // L1: the settings panel.
        33 => key(ctx, pad::L1)?,
        34 => ctx.msg(4061, 4061)?,
        35 => ctx.goto(38),
        // Dead code (the card-name row of another version).
        36 => ctx.msg(4062, 4065)?,
        37 | 40 => short(ctx, pad::DOWN)?,
        38 => {
            ctx.blink_for(BLINK_POINTER, -0x30, 0x10, BLINK_POLLS)?;
            ctx.next();
        }
        39 => ctx.msg(4066, 4067)?,
        41 => {
            ctx.blink_for(BLINK_POINTER, -0x78, 0x30, BLINK_POLLS)?;
            ctx.next();
        }
        46 => ctx.msg(4071, 4073)?,
        // L2 twice: the info windows off and on (FLAG906).
        47 | 49 => short(ctx, pad::L2)?,
        48 => ctx.msg(4074, 4074)?,
        50 => {
            ctx.msg(4075, 4075)?;
            ctx.flag906(false);
        }
        54 => ctx.msg(4076, 4078)?,
        // The sticks: one direction for 8 polls, then centred for 12 and
        // the other direction until the 19th (0x17BA48, 0x17BA84).
        55 => ctx.stick_hold(LEFT_X, 0x00)?,
        56 => ctx.stick_return(LEFT_X, 0xFF)?,
        57 => ctx.msg(4079, 4079)?,
        58 => ctx.stick_hold(LEFT_Y, 0xFF)?,
        59 => ctx.stick_return(LEFT_Y, 0x00)?,
        60 => ctx.msg(4080, 4081)?,
        61 => ctx.stick_hold(RIGHT_Y, 0x00)?,
        62 => ctx.msg(4082, 4082)?,
        63 => ctx.stick_return(RIGHT_Y, 0xFF)?,
        64 => ctx.msg(4083, 4083)?,
        // L3 + R3: the camera reset (0x18 reads them for the camera only).
        65 => short(ctx, pad::L3 | pad::R3)?,
        66 => {
            ctx.msg(4084, 4085)?;
            ctx.flag906(false);
        }
        68 => ctx.msg(4086, 4086)?,
        _ => ctx.end_lesson(),
    }
    Ok(())
}

/// `wait 14 polls ; next ; KEY k` (0x17B6C4).
fn key<C: CardData>(ctx: &mut Ctx<'_, C>, k: u16) -> Res {
    ctx.wait(KEY_WAIT)?;
    ctx.next();
    ctx.key(k);
    Ok(())
}

/// `wait 8 polls ; next ; KEY k` (0x17B74C).
fn short<C: CardData>(ctx: &mut Ctx<'_, C>, k: u16) -> Res {
    ctx.wait(SHORT_WAIT)?;
    ctx.next();
    ctx.key(k);
    Ok(())
}
