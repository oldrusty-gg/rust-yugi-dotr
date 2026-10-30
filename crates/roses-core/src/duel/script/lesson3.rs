//! Lesson 3 (controller 34, driver 0x1768B0, jump table 0x2BA830, 288
//! steps, decks 0x15 Dark Magician / 0x16 Baby Dragon, messages
//! 3710–3865): limits, stars, spells, terrain, spellbind, the Destiny Draw,
//! ranks. Every arm follows `steps-34.txt` in execution order; the poll
//! counts and keys were checked against the interpreter run from saved
//! state 189 (`emu-34.txt`) and the board/LP/round/flags after every setup
//! and battle against saved states 189–414 (`states-34.txt`; tests in
//! `lesson3/tests.rs`) [F, emu, O].
//!
//! Keys are on side 0's turn with 14-poll waits (62 for the R2/L2 toggles
//! of steps 136–140, the details views of 265–277 and their Circle/Start);
//! every setup is FADEOUT → edits → wait 32 → FADEIN. Step 0 sets
//! `NO_LP0_END`. Jumps [F]: 0 → 2; 145 → 155 (stored before its fade, so
//! case 155 finishes the fade) → 156; 208 → 146 (blink) → 147 → 209; 254 →
//! 148 (Circle) → 149 (Circle) → 255; 150–154 are unused fillers.
//!
//! What the port cannot do yet (each refused explicitly, never guessed):
//! step 284's move flips the Celtic Guardian through the
//! Baby Dragon's rank-12 ability #13 "Open Opponent's Card" [O 408], which
//! no engine rule models, so that key is refused here. The face-up Dragon
//! Capture Jars of steps 49–103 are run by the engine (a face-up magic card
//! activates when its controller acts with it). The Destiny Draw of
//! step 248 is drawn by the session ([`super::destiny`]).

use super::{pad, unit_flag as f, CameraOp, Ctx, Halt, Res, BLINK_POINTER};
use crate::duel::{terrain, CardData, DuelError};

// Cards (ids of the card table; text from the disc at runtime).
/// 0x3C, the leader of deck 0x15.
const DARK_MAGICIAN: u16 = 60;
/// 0x15, the leader of deck 0x16.
const BABY_DRAGON: u16 = 21;
/// 0x9C.
const CELTIC_GUARDIAN: u16 = 156;
const KOUMORI_DRAGON: u16 = 5;
const ZANKI: u16 = 160;
const MABARREL: u16 = 321;
const STEEL_OGRE_GROTTO_1: u16 = 522;
const DRAGON_CAPTURE_JAR: u16 = 683;
const KAISER_DRAGON: u16 = 3;
/// 0x170.
const GYAKUTENNO_MEGAMI: u16 = 368;
const FLAME_SWORDSMAN: u16 = 151;
const WALL_SHADOW: u16 = 144;
/// 0x1D.
const PARROT_DRAGON: u16 = 29;

/// The rank both leaders are written with at step 259 (0xC, Field Marshal).
const LEADER_RANK: u8 = 12;
/// `S0+0x344 = 2`: the Destiny Draw armed with situation 2.
const DESTINY_SITUATION: u8 = 2;

const KEY_WAIT: u32 = 14;
const SETUP_WAIT: u32 = 32;
/// The R2/L2 toggles, details views, captions and blinks.
const LONG_WAIT: u32 = 62;

/// The 3×3 block the terrain lessons set, in the script's order.
const BLOCK: [(u8, u8); 9] = [
    (2, 2),
    (2, 3),
    (2, 4),
    (3, 2),
    (3, 3),
    (3, 4),
    (4, 2),
    (4, 3),
    (4, 4),
];

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
            ctx.msg(3710, 3711)?;
            ctx.flag906(false);
        }
        // Five monsters and five Jars around the leader [O 191].
        3 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.place(0, CELTIC_GUARDIAN, 3, 4)?;
            ctx.place(0, KOUMORI_DRAGON, 2, 4)?;
            ctx.place(0, ZANKI, 4, 4)?;
            ctx.place(0, MABARREL, 2, 5)?;
            ctx.place(0, STEEL_OGRE_GROTTO_1, 4, 5)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 2, 6)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 4, 6)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 1, 6)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 5, 6)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 1, 5)?;
            for slot in 1..=5 {
                ctx.unit_flag(0, slot, f::COMMITTED_FACE_UP, true)?;
            }
        }
        4 => {
            for slot in 1..=5 {
                ctx.model_ready(slot)?;
            }
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        5 | 19 | 47 | 69 | 90 | 106 | 155 | 171 | 175 | 184 | 188 | 204 | 238 | 257 | 261 => {
            fade_in_flag(ctx, 8)?
        }
        6 => ctx.msg(3712, 3712)?,
        7 => blink(ctx, -0xB4, -0xA0)?,
        8 => ctx.msg(3713, 3715)?,
        // The sixth monster (hand index 2, the Celtic Guardian) is reduced
        // to dust [O 197–199].
        9 | 23 => key(ctx, pad::SQUARE)?,
        10 | 24 => key(ctx, pad::UP)?,
        11 | 25 => key(ctx, pad::CROSS)?,
        12 | 13 | 26 | 27 => key(ctx, pad::RIGHT)?,
        14 | 15 | 28 | 29 => key(ctx, pad::CROSS)?,
        16 => {
            ctx.msg(3716, 3718)?;
            ctx.flag906(false);
        }
        // Cleared board, the five-card decks, 4 stars each [O 202].
        17 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.clear_units();
            ctx.deck_reset();
            ctx.stars(0, 4);
            ctx.stars(1, 4);
            ctx.summon_ok(0, true);
        }
        18 | 46 | 241 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.cursor_to(3, 6);
            ctx.next();
        }
        20 => ctx.msg(3719, 3721)?,
        21 | 30 | 34 | 39 | 43 => blink(ctx, -0xB4, -0xB4)?,
        22 => ctx.msg(3722, 3722)?,
        // The Celtic Guardian (level 4) uses the 4 stars [O 208]; two turn
        // ends bring the income of round 2 [O 214].
        31 => ctx.msg(3723, 3725)?,
        32 | 33 | 41 | 42 => key(ctx, pad::START)?,
        35 => {
            ctx.msg(3726, 3726)?;
            ctx.flag906(false);
        }
        36 => {
            ctx.fade_out(16)?;
            ctx.next();
        }
        // Stars 12/12: the cap [O 215].
        37 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.stars(0, 12);
            ctx.stars(1, 12);
            ctx.next();
        }
        38 | 110 | 120 | 132 | 164 | 217 | 228 | 242 | 280 => fade_in_flag(ctx, 16)?,
        40 => ctx.msg(3727, 3728)?,
        44 => {
            ctx.msg(3729, 3729)?;
            ctx.flag906(false);
        }
        // Spells: cleared board, decks reset, round 0 [O 221].
        45 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.clear_units();
            ctx.deck_reset();
            ctx.stars(0, 4);
            ctx.stars(1, 4);
            ctx.round(0);
        }
        48 => {
            ctx.msg(3730, 3732)?;
            ctx.flag906(false);
        }
        // The Celtic Guardian face-up at (3,4), an enemy Jar face-up at
        // (3,3) [O 224]: the monster destroys the spell (no damage).
        49 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.place(0, CELTIC_GUARDIAN, 3, 4)?;
            ctx.place(1, DRAGON_CAPTURE_JAR, 3, 3)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 6, f::COMMITTED_FACE_UP, true)?;
        }
        50 | 109 | 119 | 144 | 163 => {
            ctx.model_ready(1)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        51 | 61 => {
            ctx.camera(CameraOp::CloseUp {
                card: CELTIC_GUARDIAN,
            });
            fade_in_flag(ctx, 8)?;
        }
        52 => ctx.msg(3733, 3734)?,
        53 | 54 | 56 | 64 | 73 | 79 | 85 | 94 | 101 | 115 | 166 | 179 | 192 | 199 | 231 | 247
        | 283 => key(ctx, pad::UP)?,
        55 | 57 | 63 | 65 | 72 | 74 | 78 | 80 | 84 | 86 | 93 | 95 | 100 | 102 | 114 | 116 | 124
        | 128 | 134 | 165 | 167 | 178 | 180 | 191 | 193 | 198 | 200 | 230 | 234 | 248 | 282 => {
            key(ctx, pad::CROSS)?
        }
        58 => {
            ctx.msg(3735, 3735)?;
            ctx.flag906(false);
        }
        // A face-down Celtic Guardian against a fresh face-up Jar [O 229]:
        // the spell dies, the monster stays face-down [O 232].
        59 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.place(0, CELTIC_GUARDIAN, 3, 4)?;
            ctx.place(1, DRAGON_CAPTURE_JAR, 3, 3)?;
            ctx.unit_flag(1, 6, f::COMMITTED_FACE_UP, true)?;
        }
        60 | 89 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.cursor_to(3, 4);
            ctx.next();
        }
        62 => ctx.msg(3736, 3737)?,
        66 => {
            ctx.msg(3738, 3739)?;
            ctx.flag906(false);
        }
        // Three own Jars face-up against three Kaiser Dragons: face-up in
        // attack, face-up in committed defence, face-down [O 234]; each Jar
        // attacking is destroyed [O 237, 241, 244].
        67 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.place(0, DRAGON_CAPTURE_JAR, 2, 4)?;
            ctx.place(1, KAISER_DRAGON, 2, 3)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 3, 4)?;
            ctx.place(1, KAISER_DRAGON, 3, 3)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 4, 4)?;
            ctx.place(1, KAISER_DRAGON, 4, 3)?;
            for slot in 6..=8 {
                ctx.unit_flag(0, slot, f::COMMITTED_FACE_UP, true)?;
            }
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 2, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 2, f::DEFENCE, true)?;
            ctx.unit_flag(1, 2, f::COMMITTED_DEFENCE, true)?;
        }
        68 => {
            for slot in 1..=3 {
                ctx.model_ready(0x100 | slot)?;
            }
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
            ctx.cursor_to(3, 4);
        }
        70 => ctx.msg(3740, 3740)?,
        71 | 92 | 190 => key(ctx, pad::LEFT)?,
        75 => ctx.msg(3741, 3742)?,
        76 | 82 | 98 | 99 | 196 | 197 | 250..=253 => key(ctx, pad::RIGHT)?,
        77 | 83 | 97 | 195 => key(ctx, pad::DOWN)?,
        81 => ctx.msg(3743, 3744)?,
        87 => {
            ctx.msg(3745, 3747)?;
            ctx.flag906(false);
        }
        // Spell against spell: a face-down Jar (2,4) and a face-up one (4,4)
        // against an enemy Jar face-up at (2,3) and one in committed
        // defence at (4,3); the Kaiser Dragon stays at (3,3) [O 247]. Left
        // is pressed on the poll the fade completes.
        88 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(1, 2);
            ctx.remove(1, 3);
            ctx.teleport(1, 1, 3, 3)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 2, 4)?;
            ctx.place(0, DRAGON_CAPTURE_JAR, 4, 4)?;
            ctx.place(1, DRAGON_CAPTURE_JAR, 2, 3)?;
            ctx.place(1, DRAGON_CAPTURE_JAR, 4, 3)?;
            ctx.unit_flag(0, 6, f::COMMITTED_FACE_UP, false)?;
            ctx.unit_flag(0, 7, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 1, f::ACTED, false)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 6, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 7, f::DEFENCE, true)?;
            ctx.unit_flag(1, 7, f::COMMITTED_DEFENCE, true)?;
            ctx.key(pad::LEFT);
        }
        91 => ctx.msg(3748, 3750)?,
        // The face-down Jar penetrates to (2,3) [O 252]; the face-up one
        // activates on completing its move: the Kaiser Dragon is
        // spellbound for ever and the Jar leaves the field [O 257–259].
        96 => ctx.msg(3751, 3754)?,
        103 => {
            ctx.msg(3755, 3759)?;
            ctx.flag906(false);
        }
        // Terrain: a Meadow block [O 265].
        104 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(1, 1);
            ctx.remove(0, 6);
            ctx.remove(0, 7);
            block(ctx, terrain::MEADOW)?;
        }
        105 | 170 | 183 | 203 | 207 | 216 => {
            ctx.cursor_to(3, 3);
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        107 => {
            ctx.msg(3760, 3763)?;
            ctx.flag906(false);
        }
        // The Celtic Guardian at (3,5) [O 269] moves onto the Meadow
        // [O 273]: +500 for a Warrior.
        108 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.place(0, CELTIC_GUARDIAN, 3, 5)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.cursor_to(3, 5);
        }
        111 => ctx.msg(3764, 3764)?,
        112 => blink(ctx, 0x5A, 0xDC)?,
        113 => ctx.msg(3765, 3766)?,
        117 => {
            ctx.msg(3767, 3769)?;
            ctx.flag906(false);
        }
        // Its action restored [O 275]: two steps from favoured terrain
        // (Up, Up to (3,2)) [O 281].
        118 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.unit_flag(0, 1, f::ACTED, false)?;
        }
        121 => ctx.msg(3770, 3772)?,
        122 => blink(ctx, 0x96, 0xD2)?,
        123 => ctx.msg(3773, 3773)?,
        125 => ctx.msg(3774, 3774)?,
        126 | 127 => key(ctx, pad::UP)?,
        129 => {
            ctx.msg(3775, 3775)?;
            ctx.flag906(false);
        }
        // Face-down at (3,3): no bonus [O 282] until R2 turns it up in the
        // move state (R2 L2 R2 L2 R2, 62 polls each [O 285–289]).
        130 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.place(0, CELTIC_GUARDIAN, 3, 3)?;
        }
        131 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.cursor_to(3, 3);
            ctx.next();
        }
        133 => ctx.msg(3776, 3776)?,
        135 => ctx.msg(3777, 3778)?,
        136 | 138 | 140 => long_key(ctx, pad::R2)?,
        137 | 139 => long_key(ctx, pad::L2)?,
        141 => ctx.next(),
        142 => {
            ctx.msg(3779, 3780)?;
            ctx.flag906(false);
        }
        // Dark terrain, Gyakutenno Megami (a Fairy) at (3,5) [O 293]; Circle
        // leaves the move state whose unit the edit replaced.
        143 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.place(0, GYAKUTENNO_MEGAMI, 3, 5)?;
            block(ctx, terrain::DARK)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.key(pad::CIRCLE);
        }
        // STEP := 155 before the fade, so case 155 finishes it and goes on
        // to 156 [F, emu].
        145 => {
            ctx.goto(155);
            ctx.fade_in(8)?;
            ctx.flag906(true);
            ctx.goto(156);
        }
        // Subroutine slot entered from 208, back through 147 to 209.
        146 => blink(ctx, 0x96, 0xF0)?,
        147 => ctx.goto(209),
        // Subroutine slot entered from 254, back through 149 to 255: two
        // Circles close the hand menu and the summon cell.
        148 => {
            ctx.next();
            ctx.key(pad::CIRCLE);
        }
        149 => {
            ctx.goto(255);
            ctx.key(pad::CIRCLE);
        }
        // Unused table fillers.
        150..=154 => ctx.end_lesson(),
        156 => {
            ctx.msg(3781, 3781)?;
            ctx.flag906(false);
        }
        157 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.camera(CameraOp::Save);
        }
        162 | 226 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.camera(CameraOp::Restore);
        }
        158 => {
            ctx.cursor_to(3, 5);
            ctx.wait(SETUP_WAIT)?;
            ctx.camera(CameraOp::Save);
            ctx.next();
        }
        159 => {
            ctx.camera(CameraOp::CloseUp {
                card: GYAKUTENNO_MEGAMI,
            });
            ctx.fade_in(16)?;
            ctx.next();
        }
        160 => {
            ctx.caption(GYAKUTENNO_MEGAMI);
            ctx.wait(LONG_WAIT)?;
            ctx.flag906(true);
            ctx.next();
        }
        161 => {
            // The caption is cleared when the next message opens [H].
            ctx.caption_off();
            ctx.msg(3782, 3784)?;
            ctx.flag906(false);
        }
        // Onto the Dark block: −500 [O 300].
        168 => {
            ctx.msg(3785, 3787)?;
            ctx.flag906(false);
        }
        // A Labyrinth block [O 303]; the Flame Swordsman at (3,5) attacks
        // the Wall Shadow inside it [O 309]: destroyed, LP1 3800, no
        // advance [O 315].
        169 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(0, 1);
            block(ctx, terrain::LABYRINTH)?;
        }
        172 => {
            ctx.msg(3788, 3793)?;
            ctx.flag906(false);
        }
        173 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.place(0, FLAME_SWORDSMAN, 3, 5)?;
            ctx.place(1, WALL_SHADOW, 3, 4)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.cursor_to(3, 4);
        }
        174 | 222 | 227 => {
            ctx.model_ready(1)?;
            ctx.model_ready(0x101)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.camera(CameraOp::Save);
            ctx.next();
        }
        176 => {
            ctx.msg(3794, 3794)?;
            ctx.key(pad::DOWN);
        }
        177 => ctx.msg(3795, 3795)?,
        181 => {
            ctx.msg(3796, 3797)?;
            ctx.flag906(false);
        }
        // A Crush block [O 317]: the Celtic Guardian (1400) survives it
        // [O 324], the Flame Swordsman (1800) is destroyed on entry [O 328].
        182 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(0, 1);
            block(ctx, terrain::CRUSH)?;
        }
        185 => {
            ctx.msg(3798, 3801)?;
            ctx.flag906(false);
        }
        186 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.place(0, CELTIC_GUARDIAN, 2, 5)?;
            ctx.place(0, FLAME_SWORDSMAN, 4, 5)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(0, 2, f::COMMITTED_FACE_UP, true)?;
        }
        187 => {
            ctx.cursor_to(3, 5);
            ctx.model_ready(1)?;
            ctx.model_ready(2)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.next();
        }
        189 => ctx.msg(3802, 3802)?,
        194 => ctx.msg(3803, 3803)?,
        201 => {
            ctx.msg(3804, 3808)?;
            ctx.flag906(false);
        }
        // Spellbinding: the Celtic Guardian back at (3,3) on normal terrain
        // [O 333], bound for one turn [O 337]; four turn ends free it
        // [O 341–344]; then bound for ever [O 346].
        202 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.teleport(0, 1, 3, 3)?;
            ctx.unit_flag(0, 1, f::ACTED, false)?;
            block(ctx, terrain::NORMAL)?;
        }
        205 => {
            ctx.msg(3809, 3812)?;
            ctx.flag906(false);
        }
        206 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.spellbind(0, 1, 1)?;
        }
        215 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.spellbind(0, 1, 0xFF)?;
        }
        // STEP := 209 then := 146 after the fade: the blink subroutine.
        208 => {
            ctx.fade_in(16)?;
            ctx.flag906(true);
            ctx.goto(146);
        }
        209 => ctx.msg(3813, 3815)?,
        210..=213 => key(ctx, pad::START)?,
        214 => {
            ctx.msg(3816, 3817)?;
            ctx.flag906(false);
        }
        218 => ctx.msg(3818, 3820)?,
        219 => blink(ctx, 0x96, 0xF0)?,
        220 => {
            ctx.msg(3821, 3823)?;
            ctx.flag906(false);
        }
        // Attribute binding: the freed Celtic Guardian at (3,4) against a
        // Parrot Dragon (WIND) at (3,3) [O 352]; EARTH beats WIND, so the
        // destroyed attacker leaves the winner spellbound for a turn
        // [O 361].
        221 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.teleport(0, 1, 3, 4)?;
            ctx.set_bind(0, 1, 0)?;
            ctx.place(1, PARROT_DRAGON, 3, 3)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.cursor_to(3, 4);
        }
        223 => {
            ctx.camera(CameraOp::CloseUp {
                card: PARROT_DRAGON,
            });
            ctx.fade_in(16)?;
            ctx.next();
        }
        224 => {
            ctx.caption(PARROT_DRAGON);
            ctx.wait(LONG_WAIT)?;
            ctx.next();
        }
        225 => {
            ctx.caption_off();
            ctx.msg(3824, 3824)?;
            ctx.flag906(false);
        }
        229 => ctx.msg(3825, 3825)?,
        232 => blink(ctx, -0x1C, 0xF0)?,
        233 => ctx.msg(3826, 3827)?,
        235 => {
            ctx.msg(3828, 3831)?;
            ctx.flag906(false);
        }
        // The Destiny Draw: a Kaiser Dragon at (2,5), LP0 400 [O 365]; the
        // byte armed with situation 2 [O 370]; the refill's last card is the
        // special one [O 377].
        236 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(1, 1);
            ctx.place(1, KAISER_DRAGON, 2, 5)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.lp(0, 400);
        }
        237 => {
            ctx.cursor_to(3, 6);
            ctx.model_ready(0x101)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.camera(CameraOp::Save);
            ctx.next();
        }
        239 => {
            ctx.msg(3832, 3835)?;
            ctx.flag906(false);
        }
        240 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.summon_ok(0, true);
            ctx.destiny_byte(0, DESTINY_SITUATION);
        }
        243 => ctx.msg(3836, 3838)?,
        244 => blink(ctx, 0x78, 0xDC)?,
        245 => ctx.msg(3839, 3840)?,
        246 => key(ctx, pad::SQUARE)?,
        249 => ctx.msg(3841, 3841)?,
        254 => {
            ctx.msg(3842, 3842)?;
            ctx.goto(148);
        }
        255 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(1, 1);
            ctx.key(pad::CIRCLE);
        }
        256 => {
            ctx.wait(SETUP_WAIT)?;
            ctx.camera(CameraOp::Save);
            ctx.next();
            ctx.key(pad::CIRCLE);
        }
        258 => {
            ctx.msg(3843, 3848)?;
            ctx.flag906(false);
        }
        // Ranks: both leaders written as Field Marshals (rank 12).
        259 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.leader_rank(0, DARK_MAGICIAN, LEADER_RANK)?;
            ctx.leader_rank(1, BABY_DRAGON, LEADER_RANK)?;
        }
        260 => {
            ctx.cursor_to(3, 6);
            ctx.wait(SETUP_WAIT)?;
            ctx.camera(CameraOp::Save);
            ctx.next();
        }
        262 => ctx.msg(3849, 3850)?,
        263 => blink(ctx, 0x5A, 0xD2)?,
        264 => ctx.msg(3851, 3852)?,
        // The leader's details (0x21 [O 390]) paged three times, closed;
        // the enemy's on its own turn [O 396–402].
        265 | 272 => long_key(ctx, pad::TRIANGLE)?,
        266..=268 | 273..=275 => long_key(ctx, pad::DOWN)?,
        269 => ctx.msg(3853, 3854)?,
        270 => long_key(ctx, pad::CIRCLE)?,
        271 => key(ctx, pad::START)?,
        276 => {
            ctx.msg(3855, 3858)?;
            ctx.flag906(false);
            ctx.key(pad::CIRCLE);
        }
        277 => long_key(ctx, pad::START)?,
        // A face-down Celtic Guardian at (3,2) before the enemy leader
        // [O 405]; moving next to it turns it face-up through the Baby
        // Dragon's "Open Opponent's Card" [O 409].
        278 => {
            ctx.flag906(false);
            ctx.fade_out(16)?;
            ctx.next();
            ctx.place(0, CELTIC_GUARDIAN, 3, 2)?;
        }
        279 => {
            ctx.cursor_to(3, 2);
            ctx.model_ready(1)?;
            ctx.wait(SETUP_WAIT)?;
            ctx.camera(CameraOp::Save);
            ctx.next();
        }
        281 => ctx.msg(3859, 3860)?,
        284 => {
            ctx.wait(KEY_WAIT)?;
            return Err(Halt::Error(DuelError::Unsupported(
                "lesson 3 step 284: the move next to the enemy leader flips the Celtic Guardian through the Baby Dragon's rank-12 leader ability #13 (Open Opponent's Card), which the engine does not model".into(),
            )));
        }
        285 => {
            ctx.msg(3861, 3864)?;
            ctx.flag906(false);
        }
        286 => {
            ctx.fade_out(8)?;
            ctx.next();
        }
        // Afterwards step 288 = count: the next poll returns −1 (the driver
        // checks the count before dispatching, so no arm runs past 287).
        287 => ctx.msg(3865, 3865)?,
        other => {
            return Err(Halt::Error(DuelError::Unsupported(format!(
                "lesson 3 step {other} has no arm"
            ))))
        }
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

/// `wait 62 polls ; next ; KEY k` (0x1789E4).
fn long_key<C: CardData>(ctx: &mut Ctx<'_, C>, k: u16) -> Res {
    ctx.wait(LONG_WAIT)?;
    ctx.next();
    ctx.key(k);
    Ok(())
}

/// `FADEIN(v) ; FLAG906=1 ; next` (0x178178 / 0x1782AC).
fn fade_in_flag<C: CardData>(ctx: &mut Ctx<'_, C>, v: u8) -> Res {
    ctx.fade_in(v)?;
    ctx.flag906(true);
    ctx.next();
    Ok(())
}

/// `BLINK(0x800@x,y) ; for 62 polls ; BLINKOFF ; next` (0x1783C8).
fn blink<C: CardData>(ctx: &mut Ctx<'_, C>, x: i16, y: i16) -> Res {
    ctx.blink_for(BLINK_POINTER, x, y, LONG_WAIT)?;
    ctx.next();
    Ok(())
}

/// `TERR(2,2)=t … TERR(4,4)=t`: the 3×3 block.
fn block<C: CardData>(ctx: &mut Ctx<'_, C>, code: u8) -> Res {
    for (c, r) in BLOCK {
        ctx.terrain(c, r, code)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
