//! Lesson 4 (controller 35, driver 0x178E60, jump table 0x2BACD0, 322
//! steps, decks 0x17 / 0x18, messages 3866–4046): duelling technique —
//! hand fusions and combo tags, the combo's star cost, failed fusions,
//! three-card fusions, power-ups (compatible, incompatible, stacked, after
//! a fusion), field fusions, a summon combined with a field monster, the
//! leader destroying an own card, the details view of field, hand and
//! graveyard cards, limited and full range traps (Spellbinding Circle,
//! Mirror Force), a face-up trap and a ritual (Ultimate Dragon).
//!
//! Every arm follows `steps-35.txt` (the project notes, decoded from the PAL
//! executable) in execution order; the shared handlers are 0x17984C (wait
//! 14, key), 0x179C94 (wait 8, key), 0x178F7C (message), 0x17A1E4 (blink),
//! 0x17A69C / 0x17A294 (FADEOUT 16 / 8 and the setup), 0x17AB58 (wait 32),
//! 0x17AF44 / 0x17AEB0 (FADEIN 16 / 8) and the cursor walks 0x17B020,
//! 0x17B084, 0x17B0E8, 0x17B150, 0x17B22C, 0x17B2A0 (one key every poll
//! until the cursor is on the cell; no wait) [F]. The forced hands of the
//! setups are 0x178BF0(step) (clear the hand 0x1E02F0, add five cards
//! 0x1E03B0) [F]. The board after every step was checked against a run of
//! the original from the map's tutorial menu (`modes/tutorial45`, [emu]).
//!
//! The lesson starts with side 0 at 8 stars (0x280440 sets both sides with
//! 0x1DF860(s, 4, 8); side 1 shows 4 at the first poll [emu]) and step 0
//! sets `NO_LP0_END`. Step 321's message closes into step 322 = the count:
//! the next poll returns −1.

use super::{pad, unit_flag as f, Ctx, Res, BLINK_POINTER};
use crate::duel::CardData;

// Cards (ids of the card table; names from the messages).
const BLUE_EYES_WHITE_DRAGON: u16 = 0;
const KAISER_DRAGON: u16 = 3;
const KOUMORI_DRAGON: u16 = 5;
const RED_EYES_B_DRAGON: u16 = 7;
const SUMMONED_SKULL: u16 = 294;
const MABARREL: u16 = 423;
const CELTIC_GUARDIAN: u16 = 156;
const MONSTROUS_BIRD: u16 = 285;
const CARD_321: u16 = 321;
const CARD_12: u16 = 12;
const SPELLBINDING_CIRCLE: u16 = 801;
const MIRROR_FORCE: u16 = 827;
const ULTIMATE_DRAGON_RITUAL: u16 = 837;

const KEY_WAIT: u32 = 14;
const SHORT_WAIT: u32 = 8;
const SETUP_WAIT: u32 = 32;
const BLINK_POLLS: u32 = 62;
/// 0x280760(S, 0x1E, c, r): the trap-area highlight kind.
const AREA_TRAP: u8 = 0x1E;

/// 0x178BF0(step): the hand side 0 is given at a setup [F].
pub(super) fn forced_hand(step: u16) -> Option<[u16; 5]> {
    Some(match step {
        0x25 | 0x3B | 0xD0 => [321, 7, 156, 249, 294],
        0x48 => [15, 5, 156, 249, 294],
        0x5D | 0xAC => [321, 7, 156, 249, 766],
        0x71 => [321, 7, 156, 249, 752],
        0x87 => [321, 7, 156, 766, 766],
        0x98 => [15, 5, 294, 766, 766],
        _ => return None,
    })
}

pub(super) fn step<C: CardData>(ctx: &mut Ctx<'_, C>) -> Res {
    match ctx.step() {
        0 => {
            ctx.no_lp0_end();
            ctx.goto(2);
        }
        // Dead code in the original (never reached).
        1 => {
            ctx.model_ready(0)?;
            ctx.model_ready(0x100)?;
            ctx.goto(208);
        }
        2 => ctx.msg(3866, 3872)?,
        // A summon at (3,5): hand 321, 7, 156, 249, 294 [emu].
        3 => key(ctx, pad::SQUARE)?,
        4 => key(ctx, pad::UP)?,
        5 => key(ctx, pad::CROSS)?,
        6 => key(ctx, pad::RIGHT)?,
        7 => ctx.msg(3873, 3873)?,
        8..=10 => short(ctx, pad::RIGHT)?,
        11 => ctx.msg(3874, 3874)?,
        12..=14 => short(ctx, pad::LEFT)?,
        // Combo tag 1 on the Red-Eyes B. Dragon.
        15 => key(ctx, pad::UP)?,
        16 => blink(ctx, -0x78, -0x50)?,
        17 => ctx.msg(3875, 3878)?,
        18 => key(ctx, pad::DOWN)?,
        19 => ctx.msg(3879, 3879)?,
        20..=22 => short(ctx, pad::RIGHT)?,
        // Tag 1 on the Summoned Skull.
        23 => key(ctx, pad::UP)?,
        24 => ctx.msg(3880, 3882)?,
        25 => short(ctx, pad::DOWN)?,
        26..=28 => short(ctx, pad::LEFT)?,
        29 => short(ctx, pad::UP)?,
        30..=32 => short(ctx, pad::RIGHT)?,
        33 => short(ctx, pad::UP)?,
        // Red-Eyes (tag 1) + Summoned Skull (tag 2) → B. Skull Dragon, 7
        // stars of 8 [emu].
        34 | 35 => key(ctx, pad::CROSS)?,
        36 => {
            ctx.msg(3883, 3883)?;
            ctx.flag906(false);
        }
        37 | 59 | 72 | 113 | 152 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 1);
            hand_setup(ctx)?;
        }
        38 | 60 | 73 | 94 | 114 | 136 | 153 => setup_wait(ctx)?,
        39 | 61 | 74 | 115 | 137 | 154 | 182 | 258 | 275 | 286 | 302 => fade_in(ctx, 16)?,
        40..=42 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 40)])?,
        43 | 45..=47 => short(ctx, pad::RIGHT)?,
        44 | 48 => short(ctx, pad::UP)?,
        49 => ctx.msg(3884, 3884)?,
        50 | 57 => blink(ctx, -0xB4, -0xB4)?,
        51 => ctx.msg(3885, 3887)?,
        52 | 88 => blink(ctx, 0x50, -0x78)?,
        53 => ctx.msg(3888, 3892)?,
        54 | 55 => key(ctx, pad::CROSS)?,
        56 => ctx.next(),
        58 => {
            ctx.msg(3893, 3894)?;
            ctx.flag906(false);
        }
        62..=64 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 62)])?,
        65 => ctx.msg(3895, 3896)?,
        // The incompatible pair: the lower tag is destroyed [emu: the
        // Red-Eyes B. Dragon alone at (3,5)].
        66 | 68 => key(ctx, pad::UP)?,
        67 => key(ctx, pad::RIGHT)?,
        69 | 70 => key(ctx, pad::CROSS)?,
        71 => {
            ctx.msg(3897, 3898)?;
            ctx.flag906(false);
        }
        75..=77 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 75)])?,
        78 => ctx.msg(3899, 3902)?,
        79 | 81 | 86 => key(ctx, pad::UP)?,
        80 => key(ctx, pad::RIGHT)?,
        82 => ctx.msg(3903, 3903)?,
        83..=85 => short(ctx, pad::RIGHT)?,
        87 => ctx.msg(3904, 3904)?,
        89 => ctx.msg(3905, 3907)?,
        // Koumori + Tyhone #2 → Red-Eyes, + Summoned Skull → B. Skull
        // Dragon for 6 stars [emu].
        90 | 91 => key(ctx, pad::CROSS)?,
        92 => {
            ctx.msg(3908, 3911)?;
            ctx.flag906(false);
        }
        93 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(0, 1);
            hand_setup(ctx)?;
        }
        95 | 174 | 202 | 210 | 244 | 309 => fade_in(ctx, 8)?,
        96 => ctx.msg(3912, 3912)?,
        97..=99 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 97)])?,
        100..=103 => short(ctx, pad::RIGHT)?,
        104 => ctx.msg(3913, 3915)?,
        105 | 109 => key(ctx, pad::UP)?,
        106..=108 => key(ctx, pad::LEFT)?,
        // Red-Eyes + Dragon Treasure: +500 (2900 / 2500, message 3916).
        110 | 111 => key(ctx, pad::CROSS)?,
        112 => {
            ctx.msg(3916, 3916)?;
            ctx.flag906(false);
        }
        116..=118 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 116)])?,
        119 => ctx.msg(3917, 3918)?,
        120 => key(ctx, pad::RIGHT)?,
        // The details of the Red-Eyes from the hand menu (0x43 → 0x20).
        121 => key(ctx, pad::TRIANGLE)?,
        122 => ctx.msg(3919, 3919)?,
        123 | 124 => key(ctx, pad::DOWN)?,
        125 => ctx.msg(3920, 3921)?,
        126 => short(ctx, pad::CIRCLE)?,
        127 | 131 => short(ctx, pad::UP)?,
        128..=130 => short(ctx, pad::RIGHT)?,
        // Red-Eyes + Legendary Sword (incompatible): the monster is
        // destroyed and the sword stays on the field for 7 stars [emu].
        132 | 133 => short(ctx, pad::CROSS)?,
        134 => {
            ctx.msg(3922, 3925)?;
            ctx.flag906(false);
        }
        135 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.remove(0, 6);
            hand_setup(ctx)?;
        }
        138..=140 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 138)])?,
        141 => ctx.msg(3926, 3928)?,
        142 | 144 | 145 | 147 => short(ctx, pad::RIGHT)?,
        143 | 146 | 148 => key(ctx, pad::UP)?,
        // Red-Eyes + two Dragon Treasures: 3400 / 3000 (message 3929).
        149 | 150 => key(ctx, pad::CROSS)?,
        151 => {
            ctx.msg(3929, 3929)?;
            ctx.flag906(false);
        }
        155..=157 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 155)])?,
        158 => ctx.msg(3930, 3932)?,
        159 | 161 | 163 | 165 | 167 => short(ctx, pad::UP)?,
        160 | 162 | 164 | 166 => short(ctx, pad::RIGHT)?,
        168 => ctx.msg(3933, 3933)?,
        // Koumori + Tyhone #2 + Summoned Skull + two Dragon Treasures:
        // B. Skull Dragon 4200 / 3500 (message 3934).
        169 | 170 => key(ctx, pad::CROSS)?,
        171 => {
            ctx.msg(3934, 3937)?;
            ctx.flag906(false);
        }
        172 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.stars(0, 8);
            ctx.place(0, RED_EYES_B_DRAGON, 3, 4)?;
            ctx.place(0, SUMMONED_SKULL, 4, 4)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(0, 2, f::COMMITTED_FACE_UP, true)?;
            ctx.set_hand(0, &forced_hand(0xAC).expect("table"));
            ctx.summon_ok(0, true);
        }
        173 => {
            ctx.cursor_to(4, 4);
            ctx.model_ready(1)?;
            ctx.model_ready(2)?;
            setup_wait(ctx)?;
        }
        175 => ctx.msg(3938, 3943)?,
        // The field fusion: the Summoned Skull moved onto the Red-Eyes
        // → B. Skull Dragon at (3,4) [emu].
        176 | 178 => key(ctx, pad::CROSS)?,
        177 => key(ctx, pad::LEFT)?,
        179 => {
            ctx.msg(3944, 3944)?;
            ctx.flag906(false);
        }
        180 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.remove(0, 2);
            ctx.place(0, SUMMONED_SKULL, 3, 5)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
        }
        181 => {
            ctx.cursor_to(3, 6);
            ctx.model_ready(1)?;
            setup_wait(ctx)?;
        }
        183 => ctx.msg(3945, 3947)?,
        // A summon onto the Summoned Skull's cell with Red-Eyes (tag 1) and
        // Dragon Treasure (tag 2).
        184 => key(ctx, pad::SQUARE)?,
        185 | 190 | 195 => key(ctx, pad::UP)?,
        186 => key(ctx, pad::CROSS)?,
        187 => ctx.msg(3948, 3950)?,
        188 => {
            ctx.wait(KEY_WAIT)?;
            ctx.next();
        }
        189 | 192..=194 => key(ctx, pad::RIGHT)?,
        191 => ctx.msg(3951, 3951)?,
        196 => ctx.msg(3952, 3952)?,
        197 | 198 => key(ctx, pad::CROSS)?,
        199 => {
            ctx.msg(3953, 3957)?;
            ctx.flag906(false);
        }
        200 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.stars(0, 8);
            ctx.place(0, MONSTROUS_BIRD, 2, 5)?;
            ctx.place(0, KOUMORI_DRAGON, 4, 5)?;
            ctx.place(0, MABARREL, 2, 6)?;
            ctx.place(0, CARD_321, 4, 6)?;
            for slot in 1..=5 {
                ctx.unit_flag(0, slot, f::COMMITTED_FACE_UP, true)?;
            }
        }
        201 => {
            ctx.cursor_to(3, 6);
            for unit in 2..=5 {
                ctx.model_ready(unit)?;
            }
            setup_wait(ctx)?;
        }
        203 => ctx.msg(3958, 3964)?,
        // The leader steps onto its own card at (4,6), which is destroyed.
        204 | 206 => key(ctx, pad::CROSS)?,
        205 => key(ctx, pad::RIGHT)?,
        207 => {
            ctx.msg(3965, 3966)?;
            ctx.flag906(false);
        }
        208 => {
            ctx.fade_out(8)?;
            ctx.next();
            for slot in 1..=5 {
                ctx.remove(0, slot);
            }
            ctx.place(0, CELTIC_GUARDIAN, 3, 3)?;
            ctx.leader_rank(0, CELTIC_GUARDIAN, 12)?;
            ctx.unit_flag(0, 1, f::COMMITTED_FACE_UP, true)?;
            ctx.set_hand(0, &forced_hand(0xD0).expect("table"));
        }
        209 => {
            ctx.cursor_to(3, 3);
            ctx.model_ready(1)?;
            setup_wait(ctx)?;
        }
        211 => ctx.msg(3967, 3971)?,
        // The details of the Celtic Guardian, its four pages.
        212 => key(ctx, pad::TRIANGLE)?,
        213 => ctx.msg(3972, 3975)?,
        214 | 216 | 218 => key(ctx, pad::DOWN)?,
        215 => ctx.msg(3976, 3977)?,
        217 => ctx.msg(3978, 3981)?,
        219 => ctx.msg(3982, 3985)?,
        220 | 221 => short(ctx, pad::CIRCLE)?,
        222 => ctx.msg(3986, 3987)?,
        // To the leader at (4,6), then the hand view of the summon menu.
        223 => walk(ctx, (4, 6), SIDE0_WALK)?,
        224..=226 => short(ctx, [pad::SQUARE, pad::UP, pad::CROSS][usize::from(ctx.step() - 224)])?,
        227 => ctx.msg(3988, 3989)?,
        228 => key(ctx, pad::RIGHT)?,
        229 => key(ctx, pad::TRIANGLE)?,
        230 => ctx.msg(3990, 3990)?,
        231..=233 => short(ctx, pad::CIRCLE)?,
        234 => ctx.msg(3991, 3991)?,
        // R1: the graveyard view, then the details of its entry.
        235 => key(ctx, pad::R1)?,
        236 => ctx.msg(3992, 3993)?,
        237 => key(ctx, pad::TRIANGLE)?,
        238 => ctx.msg(3994, 3994)?,
        239 | 240 => short(ctx, pad::CIRCLE)?,
        241 => {
            ctx.msg(3995, 3996)?;
            ctx.flag906(false);
        }
        242 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(0, 1);
            ctx.place(0, SPELLBINDING_CIRCLE, 2, 5)?;
            ctx.place(0, MIRROR_FORCE, 4, 5)?;
            ctx.unit_flag(0, 6, f::COMMITTED_FACE_UP, true)?;
            ctx.unit_flag(0, 7, f::COMMITTED_FACE_UP, true)?;
            ctx.teleport(0, 0, 3, 6)?;
        }
        243 => {
            ctx.cursor_to(3, 6);
            setup_wait(ctx)?;
        }
        245 => ctx.msg(3997, 4000)?,
        246 => short(ctx, pad::LEFT)?,
        247 => short(ctx, pad::UP)?,
        248 | 253 => short(ctx, pad::TRIANGLE)?,
        249 => ctx.msg(4001, 4001)?,
        250 => short(ctx, pad::CIRCLE)?,
        251 | 252 => short(ctx, pad::RIGHT)?,
        254 => {
            ctx.msg(4002, 4002)?;
            ctx.flag906(false);
        }
        255 => short(ctx, pad::CIRCLE)?,
        256 => {
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(0, 6);
            ctx.remove(0, 7);
            ctx.place(0, SPELLBINDING_CIRCLE, 3, 5)?;
            ctx.place(1, KAISER_DRAGON, 3, 3)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
        }
        257 => {
            ctx.cursor_to(3, 3);
            ctx.model_ready(0x101)?;
            setup_wait(ctx)?;
        }
        259 => ctx.msg(4003, 4003)?,
        260 => walk(ctx, (3, 5), SIDE0_WALK)?,
        261 => key(ctx, pad::TRIANGLE)?,
        262 => key(ctx, pad::DOWN)?,
        263 => ctx.msg(4004, 4005)?,
        264 => short(ctx, pad::CIRCLE)?,
        265 => {
            ctx.area(AREA_TRAP, 3, 5);
            ctx.wait(BLINK_POLLS)?;
            ctx.next();
        }
        266 => {
            ctx.area(AREA_TRAP, 3, 5);
            ctx.msg(4006, 4009)?;
        }
        267 | 304 | 305 => key(ctx, pad::START)?,
        // The enemy's turn: the Kaiser Dragon steps into the trap's area
        // and is spellbound for 3 turns [emu].
        268 => walk(ctx, (3, 3), SIDE1_WALK)?,
        269 | 280 | 282 | 296 | 298 => short(ctx, pad::CROSS)?,
        270 | 281 | 297 => short(ctx, pad::UP)?,
        271 => key(ctx, pad::CROSS)?,
        272 => {
            ctx.msg(4010, 4011)?;
            ctx.key(pad::START);
        }
        273 => {
            ctx.flag906(false);
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(1, 1);
            ctx.place(1, KAISER_DRAGON, 3, 5)?;
            ctx.place(0, SPELLBINDING_CIRCLE, 2, 6)?;
            ctx.unit_flag(1, 1, f::COMMITTED_FACE_UP, true)?;
        }
        274 => {
            ctx.cursor_to(3, 6);
            ctx.model_ready(0x101)?;
            setup_wait(ctx)?;
        }
        276 => {
            ctx.msg(4012, 4012)?;
            ctx.key(pad::LEFT);
        }
        277 => {
            ctx.area(AREA_TRAP, 2, 6);
            ctx.wait(BLINK_POLLS)?;
            ctx.next();
        }
        278 => {
            ctx.area(AREA_TRAP, 2, 6);
            ctx.msg(4013, 4013)?;
            ctx.key(pad::START);
        }
        // The Kaiser Dragon attacks the leader next to the trap.
        279 | 295 => walk(ctx, (3, 5), SIDE1_WALK)?,
        283 => {
            ctx.msg(4014, 4017)?;
            ctx.key(pad::START);
        }
        284 => {
            ctx.flag906(false);
            ctx.fade_out(16)?;
            ctx.next();
            ctx.remove(1, 1);
            ctx.place(0, MIRROR_FORCE, 0, 6)?;
            ctx.place(1, KAISER_DRAGON, 3, 5)?;
            ctx.place(1, KOUMORI_DRAGON, 2, 4)?;
            ctx.place(1, CARD_12, 2, 5)?;
            for slot in 1..=3 {
                ctx.unit_flag(1, slot, f::COMMITTED_FACE_UP, true)?;
            }
        }
        285 => {
            ctx.cursor_to(3, 6);
            for unit in 0x101..=0x103 {
                ctx.model_ready(unit)?;
            }
            setup_wait(ctx)?;
        }
        287 => ctx.msg(4018, 4018)?,
        // 0x17B22C: to (0,6) (column first), then CURSOR(0,6) and the
        // details of the Mirror Force on the same poll.
        288 => {
            if walk_key(ctx.cursor(), (0, 6), SIDE0_WALK).is_none() {
                ctx.cursor_to(0, 6);
                ctx.goto(289);
                ctx.key(pad::TRIANGLE);
            } else {
                walk(ctx, (0, 6), SIDE0_WALK)?;
            }
        }
        289 => ctx.msg(4019, 4019)?,
        290 => key(ctx, pad::DOWN)?,
        291 => ctx.msg(4020, 4020)?,
        292 => short(ctx, pad::CIRCLE)?,
        293 => walk(ctx, (3, 6), SIDE0_WALK)?,
        294 => {
            ctx.msg(4021, 4022)?;
            ctx.key(pad::START);
        }
        // The attack on the leader: Mirror Force destroys every attacker
        // in attack position [emu].
        299 => {
            ctx.msg(4023, 4028)?;
            ctx.key(pad::START);
        }
        300 => {
            ctx.flag906(false);
            ctx.fade_out(16)?;
            ctx.next();
            ctx.place(0, SPELLBINDING_CIRCLE, 3, 3)?;
            ctx.unit_flag(0, 6, f::COMMITTED_FACE_UP, true)?;
        }
        301 => {
            ctx.cursor_to(3, 3);
            setup_wait(ctx)?;
        }
        303 => ctx.msg(4029, 4033)?,
        306 => {
            ctx.msg(4034, 4035)?;
            ctx.flag906(false);
        }
        307 => {
            ctx.fade_out(8)?;
            ctx.next();
            ctx.remove(0, 6);
            ctx.place(0, BLUE_EYES_WHITE_DRAGON, 2, 3)?;
            ctx.place(0, BLUE_EYES_WHITE_DRAGON, 3, 3)?;
            ctx.place(0, BLUE_EYES_WHITE_DRAGON, 4, 3)?;
            ctx.place(0, ULTIMATE_DRAGON_RITUAL, 3, 4)?;
            for slot in 1..=3 {
                ctx.unit_flag(0, slot, f::COMMITTED_FACE_UP, true)?;
            }
        }
        308 => {
            ctx.cursor_to(3, 4);
            for unit in 1..=3 {
                ctx.model_ready(unit)?;
            }
            setup_wait(ctx)?;
        }
        310 => ctx.msg(4036, 4041)?,
        311 | 318 => key(ctx, pad::TRIANGLE)?,
        312 => key(ctx, pad::DOWN)?,
        313 => ctx.msg(4042, 4042)?,
        314 => short(ctx, pad::CIRCLE)?,
        // The ritual: selected, turned face-up (R2) and activated in place
        // → Blue-Eyes Ultimate Dragon (2) at (3,4) [emu].
        315 | 317 => key(ctx, pad::CROSS)?,
        316 => key(ctx, pad::R2)?,
        319 => {
            ctx.msg(4043, 4045)?;
            ctx.flag906(false);
            ctx.key(pad::CIRCLE);
        }
        320 => {
            ctx.fade_out(8)?;
            ctx.next();
        }
        321 => {
            ctx.msg(4046, 4046)?;
            ctx.flag906(false);
        }
        _ => ctx.end_lesson(),
    }
    Ok(())
}

/// The setups 0x17A69C / 0x17A294 of the hand demos: `STARS0 op4 8 ;
/// CALL 178bf0(step) ; SUMMONOK0=1 ; CURSOR(3,6)` (after `REMOVE`s). The
/// step is the one the arm ran as (already advanced by `next`).
fn hand_setup<C: CardData>(ctx: &mut Ctx<'_, C>) -> Res {
    let step = ctx.step() - 1;
    ctx.stars(0, 8);
    if let Some(hand) = forced_hand(step) {
        ctx.set_hand(0, &hand);
    }
    ctx.summon_ok(0, true);
    ctx.cursor_to(3, 6);
    Ok(())
}

/// `wait 14 polls ; next ; KEY k` (0x17984C).
fn key<C: CardData>(ctx: &mut Ctx<'_, C>, k: u16) -> Res {
    ctx.wait(KEY_WAIT)?;
    ctx.next();
    ctx.key(k);
    Ok(())
}

/// `wait 8 polls ; next ; KEY k` (0x179C94).
fn short<C: CardData>(ctx: &mut Ctx<'_, C>, k: u16) -> Res {
    ctx.wait(SHORT_WAIT)?;
    ctx.next();
    ctx.key(k);
    Ok(())
}

/// `BLINK(0x800@x,y) ; for 62 polls ; BLINKOFF ; next` (0x17A1E4).
fn blink<C: CardData>(ctx: &mut Ctx<'_, C>, x: i16, y: i16) -> Res {
    ctx.blink_for(BLINK_POINTER, x, y, BLINK_POLLS)?;
    ctx.next();
    Ok(())
}

/// `wait 32 polls ; next` (0x17AB58).
fn setup_wait<C: CardData>(ctx: &mut Ctx<'_, C>) -> Res {
    ctx.wait(SETUP_WAIT)?;
    ctx.next();
    Ok(())
}

/// `FADEIN(v) ; FLAG906=1 ; next` (0x17AF44 / 0x17AEB0).
fn fade_in<C: CardData>(ctx: &mut Ctx<'_, C>, v: u8) -> Res {
    ctx.fade_in(v)?;
    ctx.flag906(true);
    ctx.next();
    Ok(())
}

/// The keys of a cursor walk: (column too high, column too low, row too
/// high, row too low). Side 0's are absolute; side 1's are screen keys,
/// which `DuelUi::press` mirrors (0x17B150, 0x17B0E8).
type WalkKeys = (u16, u16, u16, u16);
const SIDE0_WALK: WalkKeys = (pad::LEFT, pad::RIGHT, pad::UP, pad::DOWN);
const SIDE1_WALK: WalkKeys = (pad::RIGHT, pad::LEFT, pad::DOWN, pad::UP);

/// The key of one poll of a walk to `target`: the column first, then the
/// row; `None` on the cell [F].
fn walk_key(at: crate::GridPos, target: (u8, u8), keys: WalkKeys) -> Option<u16> {
    let (c, r) = (at.column, at.row);
    if c > target.0 {
        Some(keys.0)
    } else if c < target.0 {
        Some(keys.1)
    } else if r > target.1 {
        Some(keys.2)
    } else if r < target.1 {
        Some(keys.3)
    } else {
        None
    }
}

/// A cursor walk: one key every poll until the active side's cursor is on
/// `target`, then (on that poll, no key) the next step.
fn walk<C: CardData>(ctx: &mut Ctx<'_, C>, target: (u8, u8), keys: WalkKeys) -> Res {
    match walk_key(ctx.cursor(), target, keys) {
        Some(k) => ctx.key(k),
        None => ctx.next(),
    }
    Ok(())
}
