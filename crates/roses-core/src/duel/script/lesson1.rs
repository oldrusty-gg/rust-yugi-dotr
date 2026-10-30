//! Lesson 1 (controller 32, driver 0x173170, jump tables 0x2BA4B0 and
//! 0x2BA480, 158 steps, decks 0x11 Dark Magician / 0x12 Blue-Eyes,
//! messages 3562–3669): the basics — summon, move, battle, positions, a
//! magic card, surrender. Every arm follows `steps-32.txt` in execution
//! order; the poll counts and keys were checked against the interpreter
//! runs from the practice start (`emu-32.txt`, step 2) and from saved state
//! 000 (step 26), and the board/LP after the setups and battles against
//! saved states 000–131 (tests in `lesson1/tests.rs`) [F, emu, O].
//!
//! Keys are on side 0's turn with 14-poll waits except steps 38–49, the
//! enemy's turn (8-poll waits; the script emits screen-relative directions,
//! side 1's d-pad is mirrored by the input layer). Every setup is FADEOUT →
//! edits → wait 32 → FADEIN. The cursor loops of steps 138/145 press a
//! direction every 5th poll until the cursor row matches. Step 157 returns
//! −1 on the poll its message closes (0x174E88; the interpreter calls
//! 0x23B590 in that poll).

use super::{pad, unit_flag as f, CameraOp, Ctx, Res, BLINK_POINTER};
use crate::duel::CardData;

// Cards (ids of the card table; text from the disc at runtime).
/// 0x3C, the leader of deck 0x11.
const DARK_MAGICIAN: u16 = 60;
/// The leader of deck 0x12.
const BLUE_EYES_WHITE_DRAGON: u16 = 0;
/// 0x9C.
const CELTIC_GUARDIAN: u16 = 156;
const BABY_DRAGON: u16 = 21;
const KOUMORI_DRAGON: u16 = 5;
const WINGED_DRAGON_1: u16 = 22;
const HARD_ARMOR: u16 = 169;
const AQUA_MADOOR: u16 = 100;
const ROCK_OGRE_GROTTO_2: u16 = 631;
const GIANT_SOLDIER_OF_STONE: u16 = 626;
const KAISER_DRAGON: u16 = 3;
const DRAGON_CAPTURE_JAR: u16 = 683;
const ZANKI: u16 = 160;
const GREAT_MAMMOTH_OF_GOLDFINE: u16 = 130;
const KARBONALA_WARRIOR: u16 = 157;

const KEY_WAIT: u32 = 14;
/// Steps 38–49 (0x1738A0).
const ENEMY_KEY_WAIT: u32 = 8;
const SETUP_WAIT: u32 = 32;
/// Details views, the name captions and the blinks.
const LONG_WAIT: u32 = 62;
/// The cursor loops press every fifth poll.
const CURSOR_PERIOD: u32 = 5;

pub(super) fn step<C: CardData>(ctx: &mut Ctx<'_, C>) -> Res {
    match ctx.step() {
        0 => ctx.goto(2),
        // Dead code in the original (never reached).
        1 => {
            ctx.model_ready(0)?;
            ctx.model_ready(0x100)?;
            ctx.next();
        }
        2 => {
            ctx.msg(3562, 3565)?;
            ctx.flag906(false);
        }
        // Close-up on the Dark Magician with its name caption [O 383].
        3 => {
            ctx.fade_out(16)?;
            ctx.camera(CameraOp::Save);
            ctx.camera(CameraOp::CloseUp {
                card: DARK_MAGICIAN,
            });
            ctx.next();
        }
        4 => {
            ctx.fade_in(16)?;
            ctx.caption(DARK_MAGICIAN);
            ctx.wait(LONG_WAIT)?;
            ctx.next();
        }
        5 => {
            // The caption is cleared when the next message opens [H].
            ctx.caption_off();
            ctx.msg(3566, 3567)?;
        }
        6 => {
            ctx.fade_out(16)?;
            ctx.camera(CameraOp::Restore);
            ctx.next();
        }
        7 => {
            ctx.flag906(true);
            ctx.fade_in(16)?;
            ctx.msg(3568, 3568)?;
        }
        // Square on the leader opens the summon areas (0x3E); Up, Cross
        // opens the hand at (3,5), refilled with 321, 285, 156, 249, 505.
        8 => key(ctx, pad::SQUARE)?,
        9 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        10 => ctx.msg(3569, 3571)?,
        11 => key(ctx, pad::UP)?,
        12 => key(ctx, pad::CROSS)?,
        13 => ctx.msg(3572, 3574)?,
        14 => {
            ctx.blink_for(BLINK_POINTER, -0xB4, -0xB4, LONG_WAIT)?;
            ctx.next();
        }
        15 => ctx.msg(3575, 3578)?,
        16 => {
            ctx.blink_for(BLINK_POINTER, 0x20, 0x50, LONG_WAIT)?;
            ctx.next();
        }
        17 => ctx.msg(3579, 3582)?,
        // Hand index 2 = the Celtic Guardian; Cross, Cross summons it at
        // (3,5) [O 000].
        18..=20 => key(ctx, pad::RIGHT)?,
        21 => key(ctx, pad::LEFT)?,
        22 => ctx.msg(3583, 3583)?,
        23 => key(ctx, pad::CROSS)?,
        24 => ctx.msg(3584, 3585)?,
        25 => key(ctx, pad::CROSS)?,
        26 => {
            ctx.msg(3586, 3588)?;
            ctx.camera(CameraOp::Save);
            ctx.camera(CameraOp::Preset { value: 0x1E });
        }
        // Close-up on the Blue-Eyes with its caption [O 323]; message 3589
        // is shown as 4092 (text remap).
        27 => {
            ctx.wait(LONG_WAIT)?;
            ctx.flag906(false);
            ctx.fade_out(16)?;
            ctx.camera(CameraOp::CloseUp {
                card: BLUE_EYES_WHITE_DRAGON,
            });
            ctx.next();
        }
        28 => {
            ctx.fade_in(16)?;
            ctx.caption(BLUE_EYES_WHITE_DRAGON);
            ctx.wait(LONG_WAIT)?;
            ctx.next();
        }
        29 => {
            ctx.caption_off();
            ctx.msg(3589, 3589)?;
        }
        30 => {
            ctx.fade_out(16)?;
            ctx.camera(CameraOp::Restore);
            ctx.next();
        }
        31 => {
            ctx.flag906(true);
            ctx.fade_in(16)?;
            ctx.msg(3590, 3590)?;
        }
        // The Celtic Guardian moves to (3,4) [O 011]; the script ends the
        // turn with Start on the poll message 3598 closes [O 016].
        32 => key(ctx, pad::CROSS)?,
        33 => ctx.msg(3591, 3593)?,
        34 => key(ctx, pad::UP)?,
        35 => key(ctx, pad::CROSS)?,
        36 => {
            ctx.msg(3594, 3598)?;
            ctx.key(pad::START);
        }
        // The enemy's turn: leader (3,0) → (3,1), Baby Dragon summoned at
        // (3,2) and moved to (3,3), turn ended [O 020–022].
        37 => ctx.msg(3599, 3599)?,
        38 => enemy_key(ctx, pad::CROSS)?,
        39 => enemy_key(ctx, pad::UP)?,
        40 => enemy_key(ctx, pad::CROSS)?,
        41 => enemy_key(ctx, pad::SQUARE)?,
        42 => enemy_key(ctx, pad::UP)?,
        43..=46 => enemy_key(ctx, pad::CROSS)?,
        47 => enemy_key(ctx, pad::UP)?,
        48 => enemy_key(ctx, pad::CROSS)?,
        49 => enemy_key(ctx, pad::START)?,
        // The Celtic Guardian attacks the Baby Dragon: destroyed, LP1 3800
        // [O 029].
        50 => ctx.msg(3600, 3603)?,
        51 => key(ctx, pad::CROSS)?,
        52 => key(ctx, pad::UP)?,
        53 => ctx.msg(3604, 3604)?,
        54 => key(ctx, pad::CROSS)?,
        55 => {
            ctx.msg(3605, 3607)?;
            ctx.flag906(false);
        }
        // Battle demo 1: Baby Dragon in attack position [O 034 → 044].
        56 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            ctx.lp(0, 4000);
            ctx.lp(1, 4000);
            demo_place(ctx, BABY_DRAGON, false)?;
            ctx.next();
        }
        57 | 67 | 74 | 81 | 88 | 95 => demo_wait(ctx)?,
        58 => {
            ctx.camera(CameraOp::CloseUp {
                card: CELTIC_GUARDIAN,
            });
            ctx.flag906(true);
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.next();
        }
        59 => ctx.msg(3608, 3612)?,
        60 => key(ctx, pad::UP)?,
        61 => key(ctx, pad::DOWN)?,
        62 => {
            ctx.msg(3613, 3613)?;
            ctx.key(pad::CROSS);
        }
        63 | 70 | 77 | 84 | 91 | 98 | 105 => key(ctx, pad::UP)?,
        64 | 71 | 78 | 85 | 92 | 99 | 106 => key(ctx, pad::CROSS)?,
        65 => {
            ctx.msg(3614, 3615)?;
            ctx.flag906(false);
        }
        // Demo 2: Koumori Dragon (ATK 1500): the Celtic Guardian is
        // destroyed, LP0 −100; no LP reset before it [O 046].
        66 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            demo_place(ctx, KOUMORI_DRAGON, false)?;
            ctx.next();
        }
        68 => {
            ctx.camera(CameraOp::CloseUp {
                card: CELTIC_GUARDIAN,
            });
            ctx.flag906(true);
            ctx.fade_in(8)?;
            ctx.next();
        }
        69 => {
            ctx.msg(3616, 3617)?;
            ctx.key(pad::CROSS);
        }
        72 => {
            ctx.msg(3618, 3618)?;
            ctx.flag906(false);
        }
        // Demo 3: Winged Dragon #1 (ATK 1400): both destroyed [O 051 → 057].
        73 => demo_setup(ctx, WINGED_DRAGON_1, false)?,
        75 | 82 | 89 | 96 => {
            ctx.flag906(true);
            ctx.camera(CameraOp::CloseUp {
                card: CELTIC_GUARDIAN,
            });
            ctx.fade_in(8)?;
            ctx.next();
        }
        76 => {
            ctx.msg(3619, 3620)?;
            ctx.key(pad::CROSS);
        }
        79 => {
            ctx.msg(3621, 3622)?;
            ctx.flag906(false);
        }
        // Demo 4: Hard Armor in defence: destroyed without damage
        // [O 059 → 067].
        80 => demo_setup(ctx, HARD_ARMOR, true)?,
        83 => {
            ctx.msg(3623, 3626)?;
            ctx.key(pad::CROSS);
        }
        86 => {
            ctx.msg(3627, 3627)?;
            ctx.flag906(false);
        }
        // Demo 5: Aqua Madoor (DEF 2000): LP0 −600, no advance [O 069 → 075].
        87 => demo_setup(ctx, AQUA_MADOOR, true)?,
        90 => {
            ctx.msg(3628, 3629)?;
            ctx.key(pad::CROSS);
        }
        93 => {
            ctx.msg(3630, 3631)?;
            ctx.flag906(false);
        }
        // Demo 6: Rock Ogre Grotto #2 (DEF = ATK): nothing [O 077 → 082].
        94 => demo_setup(ctx, ROCK_OGRE_GROTTO_2, true)?,
        97 => {
            ctx.msg(3632, 3632)?;
            ctx.key(pad::CROSS);
        }
        100 => {
            ctx.msg(3633, 3635)?;
            ctx.flag906(false);
        }
        // Back to the duel: LP 4000/3800, the Celtic Guardian at (3,2) in
        // front of the enemy leader (3,1) [O 085]; the direct attack takes
        // LP1 to 2400 [O 094].
        101 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            ctx.lp(0, 4000);
            ctx.lp(1, 3800);
            ctx.remove(0, 1);
            ctx.remove(1, 1);
            ctx.place(0, CELTIC_GUARDIAN, 3, 2)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.next();
        }
        102 => {
            ctx.model_ready(1)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
            ctx.cursor_to(3, 2);
        }
        103 => {
            ctx.flag906(true);
            ctx.camera(CameraOp::CloseUp {
                card: CELTIC_GUARDIAN,
            });
            ctx.fade_in(8)?;
            ctx.next();
        }
        104 => {
            ctx.msg(3636, 3640)?;
            ctx.key(pad::CROSS);
        }
        107 => {
            ctx.msg(3641, 3642)?;
            ctx.flag906(false);
        }
        // Giant Soldier of Stone face-down at (3,5) [O 096]: position and
        // face toggles in the move state, then held face-up in defence
        // [O 109].
        108 => {
            ctx.fade_out(8)?;
            ctx.place(0, GIANT_SOLDIER_OF_STONE, 3, 5)?;
            ctx.next();
        }
        109 => {
            ctx.cursor_to(3, 5);
            ctx.model_ready(2)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        110 => {
            ctx.flag906(true);
            ctx.fade_in(8)?;
            ctx.next();
        }
        111 => {
            ctx.msg(3643, 3645)?;
            ctx.key(pad::CROSS);
        }
        112 => ctx.msg(3646, 3646)?,
        113 | 115 | 118 | 121 | 130 => key(ctx, pad::R1)?,
        114 | 116 => key(ctx, pad::L1)?,
        117 => ctx.msg(3647, 3647)?,
        119 => key(ctx, pad::UP)?,
        120 => key(ctx, pad::DOWN)?,
        122 => key(ctx, pad::RIGHT)?,
        123 => key(ctx, pad::LEFT)?,
        124 => ctx.msg(3648, 3651)?,
        125 | 127 | 131 => key(ctx, pad::R2)?,
        126 | 128 => key(ctx, pad::L2)?,
        129 => ctx.msg(3652, 3652)?,
        132 => key(ctx, pad::CROSS)?,
        133 => {
            ctx.msg(3653, 3654)?;
            ctx.flag906(false);
        }
        // Kaiser Dragon face-up at (2,2), the Dragon Capture Jar face-down
        // at (2,6) [O 111]; the cursor walks down to the Jar, its details
        // view opens (0x21 [O 116]), then it is flipped in place and its
        // magic activates ("Spellbound!", the Jar leaves the field [O 123]).
        134 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            ctx.place(1, KAISER_DRAGON, 2, 2)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 2, 6)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.next();
        }
        135 => {
            ctx.cursor_to(2, 2);
            ctx.model_ready(0x101)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        136 => {
            ctx.flag906(true);
            ctx.camera(CameraOp::CloseUp {
                card: KAISER_DRAGON,
            });
            ctx.fade_in(8)?;
            ctx.next();
        }
        137 => ctx.msg(3655, 3657)?,
        138 => {
            if ctx.cursor().row == 6 {
                ctx.next();
                ctx.key(pad::TRIANGLE);
            } else if ctx.every(CURSOR_PERIOD) {
                ctx.key(pad::DOWN);
            }
        }
        139 => {
            ctx.wait(LONG_WAIT)?;
            ctx.next();
            ctx.key(pad::DOWN);
        }
        140 => ctx.msg(3658, 3662)?,
        141 => {
            ctx.wait(LONG_WAIT)?;
            ctx.next();
            ctx.key(pad::CIRCLE);
        }
        142 | 144 => key(ctx, pad::CROSS)?,
        143 => key(ctx, pad::R2)?,
        145 => {
            if ctx.cursor().row == 2 {
                ctx.next();
            } else if ctx.every(CURSOR_PERIOD) {
                ctx.key(pad::UP);
            }
        }
        146 => {
            ctx.msg(3663, 3663)?;
            ctx.flag906(false);
        }
        // Cornered: LP0 800, Blue-Eyes (2,6), Zanki (3,5), Great Mammoth
        // (4,5) against a Karbonala Warrior (4,6), all face-up [O 124]; the
        // surrender prompt is opened, "yes" highlighted and cancelled
        // [O 128–129].
        147 => {
            ctx.fade_out(8)?;
            ctx.camera(CameraOp::Save);
            ctx.remove(0, 1);
            ctx.remove(0, 2);
            ctx.remove(1, 1);
            ctx.lp(0, 800);
            ctx.place(1, BLUE_EYES_WHITE_DRAGON, 2, 6)?;
            ctx.place(1, ZANKI, 3, 5)?;
            ctx.place(1, GREAT_MAMMOTH_OF_GOLDFINE, 4, 5)?;
            ctx.place(0, KARBONALA_WARRIOR, 4, 6)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            for slot in 1..=3 {
                ctx.unit_flag(1, slot, f::COMMITTED_FACE_UP, true)?;
            }
            ctx.next();
        }
        148 => {
            ctx.cursor_to(3, 6);
            ctx.next();
        }
        149 => {
            for slot in 1..=3 {
                ctx.model_ready(0x100 | slot)?;
            }
            ctx.model_ready(1)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        150 => {
            ctx.flag906(true);
            ctx.fade_in(8)?;
            ctx.next();
        }
        151 => ctx.msg(3664, 3666)?,
        152 => key(ctx, pad::SELECT)?,
        153 => key(ctx, pad::LEFT)?,
        154 => key(ctx, pad::CIRCLE)?,
        155 => ctx.next(),
        156 => {
            ctx.msg(3667, 3668)?;
            ctx.flag906(false);
        }
        // −1 on the poll the last message closes (0x174E88).
        157 => {
            ctx.fade_out(8)?;
            ctx.msg(3669, 3669)?;
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

/// `wait 8 polls ; next ; KEY k` (the enemy's turn, steps 38–49).
fn enemy_key<C: CardData>(ctx: &mut Ctx<'_, C>, k: u16) -> Res {
    ctx.wait(ENEMY_KEY_WAIT)?;
    ctx.next();
    ctx.key(k);
    Ok(())
}

/// The battle-demo board: both slot-1 units removed, the Celtic Guardian
/// face-up at (3,4) and `enemy` face-up at (3,3), in committed defence when
/// `defence` (UFLAG f3 and f5 after f4).
fn demo_place<C: CardData>(ctx: &mut Ctx<'_, C>, enemy: u16, defence: bool) -> Res {
    ctx.remove(0, 1);
    ctx.remove(1, 1);
    ctx.place(0, CELTIC_GUARDIAN, 3, 4)?;
    ctx.place(1, enemy, 3, 3)?;
    ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
    ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
    if defence {
        ctx.unit_flag(1, 1, f::DEFENCE, true)?;
        ctx.unit_flag(1, 1, f::COMMITTED_DEFENCE, true)?;
    }
    Ok(())
}

/// Steps 73, 80, 87, 94: `FADEOUT(8) ; CAMSAVE ; LP0=4000 ; LP1=4000 ;
/// <demo board> ; next`.
fn demo_setup<C: CardData>(ctx: &mut Ctx<'_, C>, enemy: u16, defence: bool) -> Res {
    ctx.fade_out(8)?;
    ctx.camera(CameraOp::Save);
    ctx.lp(0, 4000);
    ctx.lp(1, 4000);
    demo_place(ctx, enemy, defence)?;
    ctx.next();
    Ok(())
}

/// Steps 57, 67, 74, 81, 88, 95: `MODELREADY(1) ; MODELREADY(0x101) ;
/// wait 32 polls ; next ; CURSOR(3,4)`.
fn demo_wait<C: CardData>(ctx: &mut Ctx<'_, C>) -> Res {
    ctx.model_ready(1)?;
    ctx.model_ready(0x101)?;
    ctx.wait(SETUP_WAIT)?;
    ctx.next();
    ctx.cursor_to(3, 4);
    Ok(())
}

#[cfg(test)]
mod tests;
