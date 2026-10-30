//! `roses import-campaign-map`: the campaign map (Lancastrian route) from the
//! user's ISO into a new private directory: the node table (EE 0x297F80) and
//! path table (0x298400) read from the executable with the runtime dot
//! positions recomputed by the traced spline rule, the per-node speaker/place
//! and text-base constants decoded from the executable's switch code
//! (0x2D2630, 0x2D2530, 0x2D2500, 0x2D24D0, rose table 0x2D25A0), the map
//! models of MODEL.MRG bank block 35 (slots 34..=65) with GS-byte PNG
//! textures, the needed message pages and a hashed manifest. Follows
//! `import_duel_board.rs`; see the private research
//! `story/claude-campaign-map/MAP.md`.
use crate::import_duel_board::gs_rgba;
use crate::import_story::message_image;
use crate::{emit_json, open_pal, write_png, Result};
use roses_assets::{
    decode_pal_campaign_map, decode_pal_campaign_map_texture, decode_pal_message_font,
    parse_pal_messages, CampaignMapGeometry, MessageToken, PalMessage, PAL_CAMPAIGN_MAP_BLOCK,
};
use roses_client::campaign_map::{
    self as map, MapData, MapNode, MapPath, MessageInfo, NodeText, EE_TO_FILE, NODE_COUNT,
    NODE_RECORD_BYTES, NODE_TABLE_EE, PATHS_PER_SIDE, PATH_RECORD_BYTES, PATH_SIDE_BYTES,
    PATH_TABLE_EE, SIDES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const SCHEMA: &str = map::SCHEMA;
const MODEL_ISO_PATH: &str = "DATA/MODEL.MRG";
const MESSAGE_ISO_PATH: &str = "MSG/MSG_E.DAT";
/// Bank table (file offset, u32 per block) the story init indexes with 35.
pub const BANK_TABLE_EE: u32 = 0x0029_1CD0;
/// Jump tables of the hover switch (16 entries) and the encounter, loss and
/// win text switches (12 entries each), plus the rose-card node switch.
pub const HOVER_SWITCH_EE: u32 = 0x002D_2630;
pub const ENCOUNTER_SWITCH_EE: u32 = 0x002D_2530;
pub const LOSS_SWITCH_EE: u32 = 0x002D_2500;
pub const WIN_SWITCH_EE: u32 = 0x002D_24D0;
pub const ROSE_SWITCH_EE: u32 = 0x002D_25A0;
const HOVER_ENTRIES: usize = 16;
const TEXT_ENTRIES: usize = 12;
const ROSE_ENTRIES: usize = 12;
/// `slti $v0, $v0, 3` at target + 8: the rose count branch (0x2344F0).
const ROSE_COUNT_SIGNATURE: u32 = 0x2842_0003;
const MAX_SCAN: usize = 40;
/// `parse_pal_messages` takes at most 256 ids per call; the map's pages are
/// read in chunks and merged.
const MESSAGE_CHUNK: usize = 200;

#[derive(Serialize)]
struct FileRef {
    path: String,
    sha256: String,
    bytes: u64,
}
#[derive(Serialize)]
struct TextureRef {
    index: usize,
    path: String,
    sha256: String,
    width: u32,
    height: u32,
}
#[derive(Serialize)]
struct ModelRef {
    slot: u16,
    role: &'static str,
    ordinal: u16,
    file_offset: u64,
    record_sha256: String,
    node_count: u8,
    draws: usize,
    strips: usize,
    vertices: usize,
    textures: Vec<TextureRef>,
}

/// Immediates found by a bounded straight-line scan of one switch target.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SwitchImmediates {
    pub target_ee: u32,
    /// `addiu $v1, $zero, n` followed by `dsll32 $s4, $v1, 16`.
    pub speaker: Option<u16>,
    /// `addiu $v1, $zero, n` followed by `dsll32 $s5, $v1, 16`.
    pub place: Option<u16>,
    /// First `addiu r, r, n` on $v1 or $a3 (message base added to the page counter).
    pub base: Option<u16>,
    /// The value stored by `sh r, 0x537e($a0)` (page groups).
    pub groups: Option<u16>,
    /// A conditional branch preceded the constants: only the first branch is kept.
    pub conditional: bool,
}

/// 0x22D0A0's caption base instruction (C2 = 100, side 0).
const VOYAGE_CAPTION_EE: u32 = 0x0022_D0C0;

fn file_offset(ee: u32) -> Result<usize> {
    ee.checked_sub(EE_TO_FILE)
        .map(|o| o as usize)
        .ok_or_else(|| "EE address below the executable image".into())
}

fn bytes_at(executable: &[u8], ee: u32, len: usize) -> Result<&[u8]> {
    let at = file_offset(ee)?;
    executable
        .get(at..at + len)
        .ok_or_else(|| format!("executable is too short for EE {ee:#x}").into())
}

fn u32_at(executable: &[u8], ee: u32) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes_at(executable, ee, 4)?.try_into().unwrap(),
    ))
}

/// Decode the constants of a switch target (MIPS little-endian). Only
/// `addiu`, `dsll32`, `sh` and branch shapes are interpreted; everything else
/// is skipped. The scan stops after an unconditional branch and its delay slot.
pub fn decode_switch_target(executable: &[u8], target_ee: u32) -> Result<SwitchImmediates> {
    let mut out = SwitchImmediates {
        target_ee,
        ..Default::default()
    };
    let mut li = [None::<u16>; 32];
    let mut stop_after = None::<usize>;
    // A message base is `addiu r,r,imm` on $v1/$a3 whose result is stored to
    // the message id `sh r,0x5378($a0)` within the next two words (directly or
    // in the delay slot of the following `b`). The shared no-text continuation
    // 0x22E630 (loss/win entries of nodes 0 and 1) has `addiu $v1,$v1,1` after
    // its `bne`, but stores it to the page counter 0x5380: not a base.
    let mut pending_base = None::<(usize, usize, u16)>;
    for index in 0..MAX_SCAN {
        let word = u32_at(executable, target_ee + 4 * index as u32)?;
        let op = word >> 26;
        let rs = ((word >> 21) & 31) as usize;
        let rt = ((word >> 16) & 31) as usize;
        let rd = ((word >> 11) & 31) as usize;
        let sa = (word >> 6) & 31;
        let imm = (word & 0xFFFF) as u16;
        let funct = word & 63;
        match op {
            9 => {
                if rs == 0 {
                    li[rt] = Some(imm);
                } else if rs == rt && (rs == 3 || rs == 7) && out.base.is_none() {
                    pending_base = Some((rt, index, imm));
                }
            }
            0x29 if rs == 4 && imm == 0x5378 => {
                if let Some((reg, at, base)) = pending_base {
                    if reg == rt && index <= at + 2 && out.base.is_none() {
                        out.base = Some(base);
                    }
                }
            }
            0 if funct == 0x3C && rt == 3 && sa == 16 => match rd {
                20 => out.speaker = li[3],
                21 => out.place = li[3],
                _ => {}
            },
            0x29 if rs == 4 && imm == 0x537E && out.groups.is_none() => out.groups = li[rt],
            _ => {}
        }
        let unconditional = (op == 4 && rs == 0 && rt == 0)
            || op == 2
            || op == 3
            || (op == 0 && (funct == 8 || funct == 9));
        let conditional =
            !unconditional && matches!(op, 1 | 4 | 5 | 6 | 7 | 0x14 | 0x15 | 0x16 | 0x17);
        if conditional && out.speaker.is_none() && out.base.is_none() {
            out.conditional = true;
        }
        if let Some(last) = stop_after {
            if index >= last {
                break;
            }
        } else if unconditional {
            stop_after = Some(index + 1);
        }
    }
    Ok(out)
}

/// The save/work bytes a switch case reads (RED-ROUTE.md §3–4): the side,
/// Stonehenge's state SAVE+0xE, the event C2, the hovered node C6, the C9
/// mirror W+53C4 and the free-duel flag (W+18C, the hover's first test).
#[derive(Clone, Copy, Debug, Default)]
pub struct CaseEnv {
    /// The hover switch (its `$a0` is the free-duel flag) rather than a
    /// text-start switch (`$a0` = W).
    pub hover: bool,
    pub side: u8,
    pub stonehenge_state: u8,
    pub event: i16,
    pub node: u8,
    pub w53c4: u8,
    pub free_duel: u8,
}

/// What a switch case left behind: the hover's speaker/place (registers
/// s4/s5), the text start's message base (W+5378 with the page counter 0)
/// and groups (W+537E).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CaseValues {
    pub speaker: Option<u16>,
    pub place: Option<u16>,
    pub base: Option<u16>,
    pub groups: Option<u16>,
    /// W+537C (the text style) and W+53BE (the style-2 speaker) as the
    /// text start left them.
    pub style: Option<u8>,
    pub text_speaker: Option<u16>,
}

/// The text start 0x22CF00 up to its page code 0x22E630: one function
/// whose branches on SAVE[0], C2 and C6 pick the base (the White Rose
/// side's are compare chains, not jump tables) [F].
pub const TEXT_START_EE: u32 = 0x0022_CF00;
pub const TEXT_PAGES_EE: u32 = 0x0022_E630;
const TEXT_START_MAX_STEPS: usize = 1200;
/// The hover switch's 22 entries (nodes 0..=21; the red import reads the
/// first 16 with the pattern decoder).
const HOVER_ALL_ENTRIES: usize = 22;
/// The White Rose map's nodes with texts: Brest, Stonehenge and 12..=21.
const WHITE_TEXT_NODES: [u8; 12] = [0, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21];
/// The map events whose texts the White Rose text start sets by C2:
/// 105 (Boulogne, C6 = 16) and 111 (Brest, C6 = 0) [F 0x22D218..0x22D27C].
const WHITE_CAPTION_EVENTS: [(i16, u8); 2] = [(105, 16), (111, 0)];

const SAVE_EE: u32 = 0x0029_91F0;
const WORK_EE: u32 = 0x01ED_0000;
/// A case never runs past the next case (`window` = the distance to the
/// next-higher jump-table target) nor this many bytes.
const CASE_WINDOW: u32 = 0x200;
const CASE_MAX_STEPS: usize = 400;

/// Run one switch case of the hover panel (0x234CC0) or the text start
/// (0x22CF00 and its loss/win chains) with the straight-line MIPS
/// interpreter below until it branches out of the case, under `env`. The
/// executable's own branches decide which constants are reached, so the
/// Stonehenge branches (state/W+53C4/C2) come from the code, not from a
/// table in this program.
pub fn evaluate_case(
    executable: &[u8],
    target_ee: u32,
    window: u32,
    env: CaseEnv,
) -> Result<CaseValues> {
    run_case(
        executable,
        target_ee,
        window.clamp(8, CASE_WINDOW),
        CASE_MAX_STEPS,
        env,
    )
}

/// The whole text start 0x22CF00 under `env` (a text-start environment:
/// `hover` false), up to the page code.
pub fn evaluate_text_start(executable: &[u8], env: CaseEnv) -> Result<CaseValues> {
    run_case(
        executable,
        TEXT_START_EE,
        TEXT_PAGES_EE - TEXT_START_EE,
        TEXT_START_MAX_STEPS,
        CaseEnv { hover: false, ..env },
    )
}

fn run_case(
    executable: &[u8],
    target_ee: u32,
    window: u32,
    max_steps: usize,
    env: CaseEnv,
) -> Result<CaseValues> {
    let mut r = [0i64; 32];
    // Hover: `beqz $a0` on the free-duel flag, $s0 = SAVE. Text starts:
    // $a0 = W, $a1 = SAVE.
    r[4] = if env.hover {
        i64::from(env.free_duel)
    } else {
        i64::from(WORK_EE)
    };
    r[5] = i64::from(SAVE_EE);
    r[16] = i64::from(SAVE_EE);
    let mut out = CaseValues::default();
    let load = |addr: u32, size: usize, signed: bool| -> i64 {
        let raw: i64 = if (SAVE_EE..SAVE_EE + 0x100).contains(&addr) {
            match addr - SAVE_EE {
                0 => i64::from(env.side),
                0xE => i64::from(env.stonehenge_state),
                0xC2 => i64::from(env.event),
                0xC6 => i64::from(env.node),
                _ => 0,
            }
        } else if (WORK_EE..WORK_EE + 0x6000).contains(&addr) {
            match addr - WORK_EE {
                0x53C4 => i64::from(env.w53c4),
                0x18C => i64::from(env.free_duel),
                _ => 0,
            }
        } else {
            0
        };
        let bits = size * 8;
        let mask = (1i64 << bits) - 1;
        let v = raw & mask;
        if signed && v & (1 << (bits - 1)) != 0 {
            v - (1 << bits)
        } else {
            v
        }
    };
    let mut pc = target_ee;
    let mut steps = 0usize;
    loop {
        if steps > max_steps {
            return Err(format!("switch case at {target_ee:#x} does not end").into());
        }
        steps += 1;
        let word = u32_at(executable, pc)?;
        let op = word >> 26;
        let rs = ((word >> 21) & 31) as usize;
        let rt = ((word >> 16) & 31) as usize;
        let rd = ((word >> 11) & 31) as usize;
        let sa = (word >> 6) & 31;
        let funct = word & 63;
        let imm = i64::from(word as u16 as i16);
        let uimm = i64::from(word as u16);
        let branch_target = |pc: u32| pc.wrapping_add(4).wrapping_add((imm as i32 as u32) << 2);
        let mut next = pc + 4;
        let mut jump = None::<u32>;
        let mut likely_skip = false;
        let addr = |r: &[i64; 32]| (r[rs] + imm) as u32;
        match op {
            0 => match funct {
                0 => r[rd] = i64::from((r[rt] as u32) << sa as i32),
                2 => r[rd] = i64::from(((r[rt] as u32) >> sa) as i32),
                3 => r[rd] = i64::from((r[rt] as i32) >> sa),
                8 => jump = Some(r[rs] as u32),
                0x21 => r[rd] = i64::from((r[rs] as i32).wrapping_add(r[rt] as i32)),
                0x23 => r[rd] = i64::from((r[rs] as i32).wrapping_sub(r[rt] as i32)),
                0x24 => r[rd] = r[rs] & r[rt],
                0x25 => r[rd] = r[rs] | r[rt],
                0x2A => r[rd] = i64::from(r[rs] < r[rt]),
                0x2B => r[rd] = i64::from((r[rs] as u64) < (r[rt] as u64)),
                0x38 => r[rd] = r[rt] << sa,
                0x3C => r[rd] = r[rt] << (sa + 32),
                0x3E => r[rd] = ((r[rt] as u64) >> (sa + 32)) as i64,
                0x3F => r[rd] = r[rt] >> (sa + 32),
                _ => {
                    return Err(format!(
                        "switch case {target_ee:#x}: unsupported special {word:#010x} at {pc:#x}"
                    )
                    .into())
                }
            },
            // MMI padduw rd, zero, zero clears a register.
            0x1C if rs == 0 && rt == 0 => r[rd] = 0,
            1 => {
                let taken = match rt {
                    0 | 0x10 => r[rs] < 0,
                    1 | 0x11 => r[rs] >= 0,
                    _ => {
                        return Err(format!(
                            "switch case {target_ee:#x}: unsupported regimm {word:#010x}"
                        )
                        .into());
                    }
                };
                if taken {
                    jump = Some(branch_target(pc));
                }
            }
            2 | 3 => jump = Some((pc.wrapping_add(4) & 0xF000_0000) | ((word & 0x03FF_FFFF) << 2)),
            4 | 5 | 0x14 | 0x15 => {
                let eq = r[rs] == r[rt];
                let taken = if op & 1 == 0 { eq } else { !eq };
                if taken {
                    jump = Some(branch_target(pc));
                } else if op >= 0x14 {
                    likely_skip = true;
                }
            }
            6 | 7 => {
                let taken = if op == 6 { r[rs] <= 0 } else { r[rs] > 0 };
                if taken {
                    jump = Some(branch_target(pc));
                }
            }
            8 | 9 => r[rt] = i64::from((r[rs] as i32).wrapping_add(imm as i32)),
            0xA => r[rt] = i64::from(r[rs] < imm),
            0xB => r[rt] = i64::from((r[rs] as u64) < (imm as u64)),
            0xC => r[rt] = r[rs] & uimm,
            0xD => r[rt] = r[rs] | uimm,
            0xF => r[rt] = i64::from((uimm as i32) << 16),
            0x20 => r[rt] = load(addr(&r), 1, true),
            0x21 => r[rt] = load(addr(&r), 2, true),
            0x23 => r[rt] = load(addr(&r), 4, true),
            0x24 => r[rt] = load(addr(&r), 1, false),
            0x25 => r[rt] = load(addr(&r), 2, false),
            0x28 | 0x29 | 0x2B => {
                let a = addr(&r);
                if a >= WORK_EE {
                    match a - WORK_EE {
                        0x5378 => out.base = Some(r[rt] as u16),
                        0x537E => out.groups = Some(r[rt] as u16),
                        0x537C => out.style = Some(r[rt] as u8),
                        0x53BE => out.text_speaker = Some(r[rt] as u16),
                        _ => {}
                    }
                }
            }
            _ => {
                return Err(format!(
                    "switch case {target_ee:#x}: unsupported opcode {word:#010x} at {pc:#x}"
                )
                .into());
            }
        }
        r[0] = 0;
        if let Some(target) = jump {
            // Delay slot, then the jump; leaving the case window ends it.
            let slot = u32_at(executable, pc + 4)?;
            let slot_op = slot >> 26;
            let slot_rs = ((slot >> 21) & 31) as usize;
            let slot_rt = ((slot >> 16) & 31) as usize;
            let slot_rd = ((slot >> 11) & 31) as usize;
            let slot_sa = (slot >> 6) & 31;
            let slot_imm = i64::from(slot as u16 as i16);
            match slot_op {
                0 if slot & 63 == 0x3C => r[slot_rd] = r[slot_rt] << (slot_sa + 32),
                0 if slot & 63 == 0x3F => r[slot_rd] = r[slot_rt] >> (slot_sa + 32),
                0 if slot == 0 => {}
                0x1C if slot_rs == 0 && slot_rt == 0 => r[slot_rd] = 0,
                9 => r[slot_rt] = i64::from((r[slot_rs] as i32).wrapping_add(slot_imm as i32)),
                0xA => r[slot_rt] = i64::from(r[slot_rs] < slot_imm),
                0xF => r[slot_rt] = i64::from(((slot as u16) as i32) << 16),
                0x20 => r[slot_rt] = load((r[slot_rs] + slot_imm) as u32, 1, true),
                0x21 => r[slot_rt] = load((r[slot_rs] + slot_imm) as u32, 2, true),
                0x28 | 0x29 => {
                    let a = (r[slot_rs] + slot_imm) as u32;
                    if a >= WORK_EE {
                        match a - WORK_EE {
                            0x5378 => out.base = Some(r[slot_rt] as u16),
                            0x537E => out.groups = Some(r[slot_rt] as u16),
                            0x537C => out.style = Some(r[slot_rt] as u8),
                            0x53BE => out.text_speaker = Some(r[slot_rt] as u16),
                            _ => {}
                        }
                    }
                }
                _ => {
                    return Err(format!(
                        "switch case {target_ee:#x}: unsupported delay slot {slot:#010x} at {:#x}",
                        pc + 4
                    )
                    .into());
                }
            }
            r[0] = 0;
            if !(target_ee..target_ee + window).contains(&target) {
                break;
            }
            next = target;
        } else if likely_skip {
            next = pc + 8;
        }
        pc = next;
        if !(target_ee..target_ee + window).contains(&pc) {
            break;
        }
    }
    if r[20] != 0 {
        out.speaker = u16::try_from(r[20]).ok();
    }
    if r[21] != 0 {
        out.place = u16::try_from(r[21]).ok();
    }
    Ok(out)
}

fn jump_table(executable: &[u8], table_ee: u32, entries: usize) -> Result<Vec<u32>> {
    (0..entries)
        .map(|i| u32_at(executable, table_ee + 4 * i as u32))
        .collect()
}

fn read_nodes(executable: &[u8]) -> Result<Vec<MapNode>> {
    let mut nodes = Vec::new();
    for index in 0..NODE_COUNT {
        let r = bytes_at(
            executable,
            NODE_TABLE_EE + (index * NODE_RECORD_BYTES) as u32,
            NODE_RECORD_BYTES,
        )?;
        let f32_at = |at: usize| f32::from_le_bytes(r[at..at + 4].try_into().unwrap());
        let i16_at = |at: usize| i16::from_le_bytes(r[at..at + 2].try_into().unwrap());
        let u16s = |at: usize| -> [u16; 4] {
            std::array::from_fn(|i| {
                u16::from_le_bytes(r[at + 2 * i..at + 2 * i + 2].try_into().unwrap())
            })
        };
        nodes.push(MapNode {
            index: index as u8,
            name: format!("node-{index}"),
            position: [f32_at(0), f32_at(4), f32_at(8)],
            facing_raw: f32_at(0xC),
            base_model: i16_at(0x10),
            castle_model: i16_at(0x12),
            red_paths: u16s(0x14),
            white_paths: u16s(0x1C),
            path_count: u16::from_le_bytes(r[0x24..0x26].try_into().unwrap()),
            path_dirs: u16s(0x26),
            win_event: i16_at(0x2E),
            loss_event: i16_at(0x30),
        });
    }
    Ok(nodes)
}

fn read_paths(executable: &[u8]) -> Result<Vec<MapPath>> {
    let mut paths = Vec::new();
    for side in 0..SIDES {
        for index in 0..PATHS_PER_SIDE {
            let r = bytes_at(
                executable,
                PATH_TABLE_EE + (side * PATH_SIDE_BYTES + index * PATH_RECORD_BYTES) as u32,
                PATH_RECORD_BYTES,
            )?;
            let i16_at = |at: usize| i16::from_le_bytes(r[at..at + 2].try_into().unwrap());
            let u16_at = |at: usize| u16::from_le_bytes(r[at..at + 2].try_into().unwrap());
            let count = usize::from(u16_at(0x40));
            if count > map::MAX_WAYPOINTS || !matches!(u16_at(0x42), 0 | 1) {
                return Err(format!(
                    "path {side}/{index}: waypoint count or curve flag out of range"
                )
                .into());
            }
            let waypoints: Vec<[i16; 2]> = (0..count)
                .map(|i| [i16_at(4 * i), i16_at(4 * i + 2)])
                .collect();
            let sea_route = u16_at(0x42) == 1;
            paths.push(MapPath {
                side: side as u8,
                index: index as u8,
                dots: map::spline_dots(&waypoints, sea_route),
                waypoints,
                sea_route,
                ends: [u16_at(0x44), u16_at(0x46)],
                alternate_ends: [u16_at(0x48), u16_at(0x4A)],
            });
        }
    }
    Ok(paths)
}

struct SwitchTables {
    nodes: Vec<MapNode>,
    hover: Vec<SwitchImmediates>,
    encounter: Vec<SwitchImmediates>,
    loss: Vec<SwitchImmediates>,
    win: Vec<SwitchImmediates>,
    rose_targets: Vec<u32>,
    rose_nodes: Vec<u8>,
}

fn read_switches(executable: &[u8]) -> Result<SwitchTables> {
    let decode = |table: u32, entries: usize| -> Result<Vec<SwitchImmediates>> {
        jump_table(executable, table, entries)?
            .into_iter()
            .map(|target| decode_switch_target(executable, target))
            .collect()
    };
    let rose_targets = jump_table(executable, ROSE_SWITCH_EE, ROSE_ENTRIES)?;
    let mut rose_nodes = Vec::new();
    for (node, target) in rose_targets.iter().enumerate() {
        if u32_at(executable, target + 8)? == ROSE_COUNT_SIGNATURE {
            rose_nodes.push(node as u8);
        }
    }
    if rose_nodes.is_empty() || rose_nodes.len() == ROSE_ENTRIES {
        return Err("rose-card switch does not separate counted nodes".into());
    }
    Ok(SwitchTables {
        nodes: read_nodes(executable)?,
        hover: decode(HOVER_SWITCH_EE, HOVER_ENTRIES)?,
        encounter: decode(ENCOUNTER_SWITCH_EE, TEXT_ENTRIES)?,
        loss: decode(LOSS_SWITCH_EE, TEXT_ENTRIES)?,
        win: decode(WIN_SWITCH_EE, TEXT_ENTRIES)?,
        rose_targets,
        rose_nodes,
    })
}

fn node_texts(switches: &SwitchTables) -> Vec<NodeText> {
    (0..HOVER_ENTRIES)
        .map(|n| {
            let hover = &switches.hover[n];
            let text = |table: &[SwitchImmediates]| table.get(n).cloned().unwrap_or_default();
            let (enc, loss, win) = (
                text(&switches.encounter),
                text(&switches.loss),
                text(&switches.win),
            );
            NodeText {
                node: n as u8,
                speaker_message: hover.speaker,
                place_message: hover.place,
                opponent: hover
                    .speaker
                    .and_then(|s| s.checked_sub(197))
                    .and_then(|o| u8::try_from(o).ok()),
                encounter_base: enc.base,
                encounter_groups: enc.base.and(enc.groups),
                loss_base: loss.base,
                win_base: win.base,
                loss_groups: loss.base.and(loss.groups),
                win_groups: win.base.and(win.groups),
                conditional: hover.conditional
                    || enc.conditional
                    || loss.conditional
                    || win.conditional,
                second_visit: None,
            }
        })
        .collect()
}

/// Stonehenge's two visits from the executable's switch code (RED-ROUTE.md
/// §3–4): the first visit is the hover/text start with state 1 and W+53C4 = 0
/// and the loss/win texts of events 212/312; the second visit (Manawyddan)
/// is state 3 with W+53C4 = 1 and events 213/313. The free-duel branch of
/// the hover (W+18C) is not evaluated.
fn stonehenge_texts(
    executable: &[u8],
    switches: &SwitchTables,
    texts: &mut [NodeText],
) -> Result<()> {
    let node = usize::from(map::STONEHENGE_NODE);
    let window = |table: &[u32]| -> u32 {
        let target = table[node];
        table
            .iter()
            .filter(|t| **t > target)
            .map(|t| t - target)
            .min()
            .unwrap_or(CASE_WINDOW)
    };
    let hover_targets: Vec<u32> = switches.hover.iter().map(|s| s.target_ee).collect();
    let text_targets =
        |table: &[SwitchImmediates]| -> Vec<u32> { table.iter().map(|s| s.target_ee).collect() };
    let (enc_t, loss_t, win_t) = (
        text_targets(&switches.encounter),
        text_targets(&switches.loss),
        text_targets(&switches.win),
    );
    let visit =
        |state: u8, w53c4: u8, loss_event: i16, win_event: i16| -> Result<map::NodeTextVariant> {
            let env = |hover: bool, event: i16| CaseEnv {
                hover,
                side: 0,
                stonehenge_state: state,
                event,
                node: map::STONEHENGE_NODE,
                w53c4,
                free_duel: 0,
            };
            let hover = evaluate_case(
                executable,
                hover_targets[node],
                window(&hover_targets),
                env(true, 0),
            )?;
            let enc = evaluate_case(executable, enc_t[node], window(&enc_t), env(false, 0))?;
            let loss = evaluate_case(
                executable,
                loss_t[node],
                window(&loss_t),
                env(false, loss_event),
            )?;
            let win = evaluate_case(
                executable,
                win_t[node],
                window(&win_t),
                env(false, win_event),
            )?;
            Ok(map::NodeTextVariant {
                speaker_message: hover.speaker,
                place_message: hover.place,
                opponent: hover
                    .speaker
                    .and_then(|s| s.checked_sub(197))
                    .and_then(|o| u8::try_from(o).ok()),
                encounter_base: enc.base,
                encounter_groups: enc.base.and(enc.groups),
                loss_base: loss.base,
                loss_groups: loss.base.and(loss.groups),
                win_base: win.base,
                win_groups: win.base.and(win.groups),
            })
        };
    let entry = texts
        .get_mut(node)
        .ok_or("the hover switch has no Stonehenge entry")?;
    let first = visit(
        map::STATE_OPEN,
        0,
        entry_event(&switches.nodes, node, false),
        entry_event(&switches.nodes, node, true),
    )?;
    let second = visit(
        map::STATE_BEATEN,
        1,
        map::EVENT_MANAWYDDAN_LOST,
        map::EVENT_MANAWYDDAN_WON,
    )?;
    if first.speaker_message.is_none()
        || first.encounter_base.is_none()
        || second.speaker_message.is_none()
    {
        return Err("Stonehenge switch cases did not yield both visits".into());
    }
    entry.speaker_message = first.speaker_message;
    entry.place_message = first.place_message;
    entry.opponent = first.opponent;
    entry.encounter_base = first.encounter_base;
    entry.encounter_groups = first.encounter_groups;
    entry.loss_base = first.loss_base;
    entry.loss_groups = first.loss_groups;
    entry.win_base = first.win_base;
    entry.win_groups = first.win_groups;
    entry.second_visit = Some(second);
    Ok(())
}

/// The White Rose side's node texts and event captions (SAVE[0] = 1): the
/// hover switch case of each node and the text start 0x22CF00 run with
/// side 1 (WHITE-ROUTE-AUDIT.md §1). Nodes without a speaker (Dover,
/// Boulogne) keep their place only.
fn white_texts(
    executable: &[u8],
    nodes: &[MapNode],
) -> Result<(Vec<NodeText>, Vec<map::EventCaption>)> {
    let hover_targets = jump_table(executable, HOVER_SWITCH_EE, HOVER_ALL_ENTRIES)?;
    let window = |target: u32| -> u32 {
        hover_targets
            .iter()
            .filter(|t| **t > target)
            .map(|t| t - target)
            .min()
            .unwrap_or(CASE_WINDOW)
    };
    let env = |node: u8, event: i16| CaseEnv {
        hover: false,
        side: 1,
        stonehenge_state: map::STATE_OPEN,
        event,
        node,
        w53c4: 0,
        free_duel: 0,
    };
    let mut texts = Vec::new();
    for node in WHITE_TEXT_NODES {
        let target = hover_targets[usize::from(node)];
        let hover = evaluate_case(
            executable,
            target,
            window(target),
            CaseEnv {
                hover: true,
                ..env(node, 0)
            },
        )?;
        let entry = &nodes[usize::from(node)];
        let enc = evaluate_text_start(executable, env(node, 0))?;
        let (loss, win) = if entry.win_event > 0 && hover.speaker.is_some() {
            (
                evaluate_text_start(executable, env(node, entry.loss_event))?,
                evaluate_text_start(executable, env(node, entry.win_event))?,
            )
        } else {
            (CaseValues::default(), CaseValues::default())
        };
        texts.push(NodeText {
            node,
            speaker_message: hover.speaker,
            place_message: hover.place,
            opponent: hover
                .speaker
                .and_then(|s| s.checked_sub(197))
                .and_then(|o| u8::try_from(o).ok()),
            encounter_base: enc.base,
            encounter_groups: enc.base.and(enc.groups),
            loss_base: loss.base,
            win_base: win.base,
            loss_groups: loss.base.and(loss.groups),
            win_groups: win.base.and(win.groups),
            conditional: false,
            second_visit: None,
        });
    }
    let mut captions = Vec::new();
    for (event, node) in WHITE_CAPTION_EVENTS {
        let c = evaluate_text_start(executable, env(node, event))?;
        let (Some(base), Some(groups)) = (c.base, c.groups) else {
            return Err(format!("the White Rose text start sets no caption for event {event}").into());
        };
        captions.push(map::EventCaption {
            event,
            base,
            groups,
            style: c.style.unwrap_or(0),
            speaker: c.text_speaker.filter(|_| c.style == Some(2)),
        });
    }
    // Every white duel node has its three texts and a speaker.
    for t in &texts {
        let duel = nodes[usize::from(t.node)].win_event > 0
            && t.node != map::WHITE_DOVER_NODE
            && t.node != map::WHITE_BOULOGNE_NODE;
        if duel && (t.speaker_message.is_none() || t.encounter_base.is_none() || t.win_base.is_none() || t.loss_base.is_none()) {
            return Err(format!("the White Rose texts of node {} were not decoded", t.node).into());
        }
    }
    Ok((texts, captions))
}

fn entry_event(nodes: &[MapNode], node: usize, win: bool) -> i16 {
    nodes
        .get(node)
        .map(|n| if win { n.win_event } else { n.loss_event })
        .unwrap_or(0)
}

fn transcribe(message: &PalMessage) -> String {
    message
        .lines
        .iter()
        .map(|line| {
            line.tokens
                .iter()
                .map(|t| match t {
                    MessageToken::Glyph {
                        character: Some(c), ..
                    } => *c,
                    _ => '?',
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every message the map shows: the caption/rose/over-DC texts, the story
/// windows of 11120/11124/11149 (and 11149's page counter 2804), each
/// node's speaker and place, and the dialogue pages base+1..=4·groups of
/// every text base (both Stonehenge visits).
fn message_ids(texts: &[NodeText]) -> Vec<u16> {
    let mut ids = BTreeSet::from([
        map::CAPTION_MESSAGE,
        map::ROSE_CARD_MESSAGE,
        map::OVER_DC_MESSAGE,
    ]);
    ids.extend(map::map_story_message_ids());
    let add_pages = |ids: &mut BTreeSet<u16>, base: Option<u16>, groups: Option<u16>| {
        let Some(base) = base else {
            return;
        };
        let pages = groups.unwrap_or(1).clamp(1, 16) * map::PAGES_PER_GROUP;
        for page in 1..=pages {
            if let Some(id) = base.checked_add(page) {
                if usize::from(id) < roses_assets::MESSAGE_COUNT {
                    ids.insert(id);
                }
            }
        }
    };
    for text in texts {
        let mut variants = vec![text.first_visit()];
        variants.extend(text.second_visit.clone());
        for v in variants {
            ids.extend(v.speaker_message);
            ids.extend(v.place_message);
            add_pages(&mut ids, v.encounter_base, v.encounter_groups);
            add_pages(&mut ids, v.loss_base, v.loss_groups);
            add_pages(&mut ids, v.win_base, v.win_groups);
        }
    }
    ids.into_iter().collect()
}

/// GS view of a texel: 16-bit palettes as `gs_rgba`; 32-bit palettes keep
/// their RGB bytes and map GS alpha 128 to PNG 255 (the same convention).
pub fn map_rgba(texture: &roses_assets::OpeningIndexedTexture) -> Result<Vec<u8>> {
    match texture.palette_format {
        roses_assets::OpeningPaletteFormat::Rgb5551 => gs_rgba(texture),
        roses_assets::OpeningPaletteFormat::Rgba8888Alpha128 => {
            let mut pixels = Vec::with_capacity(texture.palette_indices.len() * 4);
            for index in &texture.palette_indices {
                let value = texture.palette_words_raw[usize::from(*index)];
                pixels.extend([
                    value as u8,
                    (value >> 8) as u8,
                    (value >> 16) as u8,
                    ((value >> 24) * 255 / 128).min(255) as u8,
                ]);
            }
            Ok(pixels)
        }
    }
}

fn file_ref(output: &Path, path: &str) -> Result<FileRef> {
    let bytes = fs::read(output.join(path))?;
    Ok(FileRef {
        path: path.into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    })
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "campaign map output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    // parse_pal_rules authenticates the executable before any table is read.
    let rules = roses_assets::parse_pal_rules(&executable)?;
    let nodes = read_nodes(&executable)?;
    let paths = read_paths(&executable)?;
    let switches = read_switches(&executable)?;
    let mut texts = node_texts(&switches);
    stonehenge_texts(&executable, &switches, &mut texts)?;
    let (white_texts, white_captions) = white_texts(&executable, &nodes)?;
    let block_offset = u32_at(
        &executable,
        BANK_TABLE_EE + 4 * PAL_CAMPAIGN_MAP_BLOCK as u32,
    )?;
    let block_end = u32_at(
        &executable,
        BANK_TABLE_EE + 4 * (PAL_CAMPAIGN_MAP_BLOCK as u32 + 1),
    )?;
    let archive = disc.read_file(MODEL_ISO_PATH, 32 * 1024 * 1024)?;
    let geometry: CampaignMapGeometry =
        decode_pal_campaign_map(&archive, block_offset as usize, block_end as usize)?;
    let message_file = disc.read_file(MESSAGE_ISO_PATH, 300_000)?;
    // The voyage caption (event 100): `addiu $v1, $v1, 0x987` at 0x22D0C0.
    let caption_word = u32_at(&executable, VOYAGE_CAPTION_EE)?;
    if caption_word != 0x2463_0000 | u32::from(map::VOYAGE_CAPTION_BASE) {
        return Err(format!(
            "the voyage caption base at {VOYAGE_CAPTION_EE:#x} is {caption_word:#010x}, not the traced PAL instruction"
        )
        .into());
    }
    let mut ids = message_ids(&texts);
    ids.extend(message_ids(&white_texts));
    for c in &white_captions {
        for page in 1..=c.groups.clamp(1, 16) * map::PAGES_PER_GROUP {
            ids.push(c.base + page);
        }
    }
    ids.retain(|id| usize::from(*id) < roses_assets::MESSAGE_COUNT);
    ids.push(map::VOYAGE_CAPTION_BASE + 1);
    ids.sort_unstable();
    ids.dedup();
    let mut messages = parse_pal_messages(&message_file, &ids[..ids.len().min(MESSAGE_CHUNK)])?;
    for chunk in ids.chunks(MESSAGE_CHUNK).skip(1) {
        messages
            .messages
            .extend(parse_pal_messages(&message_file, chunk)?.messages);
    }
    let font = decode_pal_message_font(&message_file)?;
    let mut infos = BTreeMap::new();
    for message in &messages.messages {
        infos.insert(
            message.message_id,
            MessageInfo {
                blank: message
                    .expanded_codes
                    .first()
                    .is_some_and(|c| c & 0x3FFF == map::BLANK_GLYPH),
                glyphs: message
                    .expanded_codes
                    .iter()
                    .filter(|c| **c != 0x2000)
                    .count()
                    .min(u16::MAX as usize) as u16,
                lines: message.lines.len().min(255) as u8,
                text: transcribe(message),
            },
        );
    }
    let mut nodes = nodes;
    for node in &mut nodes {
        if let Some(name) = texts
            .iter()
            .find(|t| t.node == node.index)
            .and_then(|t| t.place_message)
            .and_then(|id| infos.get(&id))
            .map(|m| m.text.replace('\n', " "))
            .filter(|n| !n.trim().is_empty() && n.len() <= 32)
        {
            node.name = name;
        }
    }
    let data = MapData {
        nodes,
        paths,
        texts,
        rose_card_nodes: switches.rose_nodes.clone(),
        messages: infos,
        white_texts,
        white_captions,
    };
    data.validate()?;

    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    fs::create_dir(output.join("textures"))?;
    fs::create_dir(output.join("text"))?;
    let mut refs = Vec::new();
    for model in &geometry.models {
        let mut textures = Vec::new();
        for index in 0..model.textures.len() {
            let texture = decode_pal_campaign_map_texture(&archive, model, index)?;
            let path = format!("textures/slot-{}-t{index}.png", model.slot);
            write_png(
                &output.join(&path),
                texture.width,
                texture.height,
                &map_rgba(&texture)?,
            )?;
            let file = file_ref(output, &path)?;
            textures.push(TextureRef {
                index,
                path,
                sha256: file.sha256,
                width: texture.width,
                height: texture.height,
            });
        }
        refs.push(ModelRef {
            slot: model.slot,
            role: map::slot_role(model.slot),
            ordinal: model.ordinal,
            file_offset: model.file_offset,
            record_sha256: model.source_sha256.clone(),
            node_count: model.node_count,
            draws: model.draws.len(),
            strips: model.strips.len(),
            vertices: model.strips.iter().map(|s| s.vertices.len()).sum(),
            textures,
        });
    }
    // The talking faces of the hover panel (0x247310 slots draw TEXANM
    // record = speaker); the archive is kept verbatim and decoded at run time.
    let texanm = disc.read_file("DATA/TEXANM.MRG", roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64)?;
    std::fs::write(output.join("TEXANM.MRG"), &texanm)?;
    drop(texanm);
    let mut text_images = serde_json::Map::new();
    for message in &messages.messages {
        let id = message.message_id;
        if data.messages[&id].blank {
            continue;
        }
        match message_image(&font, message) {
            Ok(image) => {
                let path = format!("text/message-{id}.png");
                write_png(
                    &output.join(&path),
                    image.width,
                    image.height,
                    &image.pixels,
                )?;
                let file = file_ref(output, &path)?;
                text_images.insert(
                    id.to_string(),
                    serde_json::json!({"path": path, "sha256": file.sha256, "width": image.width, "height": image.height}),
                );
            }
            Err(error) => {
                text_images.insert(
                    id.to_string(),
                    serde_json::json!({"not_rendered": error.to_string()}),
                );
            }
        }
    }
    emit_json(&data, Some(&output.join("map-data.json")))?;
    emit_json(&geometry, Some(&output.join("geometry.json")))?;
    emit_json(&messages, Some(&output.join("messages.json")))?;
    emit_json(&report, Some(&output.join("disc.json")))?;
    let switch_json = |table: &[SwitchImmediates]| serde_json::to_value(table).unwrap();
    emit_json(
        &serde_json::json!({
            "schema": SCHEMA,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "source": {
                "iso_sha256": report.iso_sha256,
                "executable_sha256": rules.executable_sha256,
                "model_archive": {
                    "iso_path": MODEL_ISO_PATH,
                    "sha256": geometry.archive_sha256,
                    "bytes": archive.len(),
                },
                "message_file": {
                    "iso_path": MESSAGE_ISO_PATH,
                    "sha256": messages.message_sha256,
                },
            },
            "map_data": file_ref(output, "map-data.json")?,
            "texanm": file_ref(output, "TEXANM.MRG")?,
            "geometry": file_ref(output, "geometry.json")?,
            "messages": file_ref(output, "messages.json")?,
            "text_images": text_images,
            "text_image_encoding": "message font cells (left 12 of 22 px per glyph, 22 px rows), RGBA; pages with more than three lines or thirty columns are not rendered",
            "tables": {
                "ee_to_file_offset": EE_TO_FILE,
                "node_table": {"ee": NODE_TABLE_EE, "count": NODE_COUNT, "stride": NODE_RECORD_BYTES},
                "path_table": {"ee": PATH_TABLE_EE, "sides": SIDES, "count": PATHS_PER_SIDE, "stride": PATH_RECORD_BYTES},
                "dots": "natural cubic spline through the waypoints; floor(chord/4) samples per segment at t = i/n, sea routes 9 samples at t = i/10 (0x21B490 at story init)",
                "bank_table": {"ee": BANK_TABLE_EE, "block": PAL_CAMPAIGN_MAP_BLOCK, "block_offset": block_offset, "block_end": block_end},
                "hover_switch": {"ee": HOVER_SWITCH_EE, "entries": switch_json(&switches.hover)},
                "encounter_switch": {"ee": ENCOUNTER_SWITCH_EE, "entries": switch_json(&switches.encounter)},
                "loss_switch": {"ee": LOSS_SWITCH_EE, "entries": switch_json(&switches.loss)},
                "win_switch": {"ee": WIN_SWITCH_EE, "entries": switch_json(&switches.win)},
                "rose_switch": {"ee": ROSE_SWITCH_EE, "targets": switches.rose_targets, "counted_nodes": switches.rose_nodes},
            },
            "camera": {
                "eye_offset": map::CAMERA_EYE_OFFSET,
                "projection": [map::PROJECTION_X, map::PROJECTION_Y],
                "screen_centre": map::SCREEN_CENTRE,
                "anchor_height": map::ANCHOR_HEIGHT,
                "hover_units": map::HOVER_UNITS,
            },
            "models": refs,
            "unsupported": geometry.unsupported,
            "texture_encoding": "16-bit CLUT: PNG RGB = GS expansion (5-bit value << 3), PNG alpha 255 = CLUT alpha bit (GS alpha 128 via TEXA TA1); 32-bit CLUT: RGB bytes as stored, PNG alpha = GS alpha * 255 / 128 (128 = 255)",
            "not_included": [
                "leader figure models (MONSTER data at slots 30/31)",
                "HUD, hover panel, caption window and cursor art (the game draws them from the duel import's TEXSYS.MRG)",
                "opponent decks and deck costs (the game reads them from the duel import for the DC check)",
                "the conditional Stonehenge texts beyond their two visits (the White Rose texts are white_texts / white_captions of map-data.json)",
                "the White Rose story events 12040, 12041, 12130..=12156 (the story scene's pages)",
                "sound and music data (cue numbers only)"
            ],
            "complete_game_import": false,
            "rendering_validated": false,
            "gameplay_validated": false,
        }),
        Some(&output.join("manifest.json")),
    )?;
    eprintln!(
        "Imported {} map models ({} unsupported), {} messages and the node/path tables into {}",
        geometry.models.len(),
        geometry.unsupported.len(),
        messages.messages.len(),
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(op: u32, rs: u32, rt: u32, imm: u32) -> u32 {
        (op << 26) | (rs << 21) | (rt << 16) | (imm & 0xFFFF)
    }
    fn exe(words: &[u32]) -> Vec<u8> {
        let mut bytes = vec![0u8; 64];
        for w in words {
            bytes.extend(w.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn switch_targets_yield_speaker_place_base_and_groups() {
        // addiu v1,zero,199; dsll32 s4,v1,16; addiu v1,zero,221; dsll32 s5,v1,16; b; nop
        let hover = exe(&[
            word(9, 0, 3, 199),
            (3 << 16) | (20 << 11) | (16 << 6) | 0x3C,
            word(9, 0, 3, 221),
            (3 << 16) | (21 << 11) | (16 << 6) | 0x3C,
            word(4, 0, 0, 5),
            0,
            word(9, 0, 3, 999),
        ]);
        let d = decode_switch_target(&hover, EE_TO_FILE + 64).unwrap();
        assert_eq!(
            (d.speaker, d.place, d.conditional),
            (Some(199), Some(221), false)
        );
        // lh a3; addiu v1,zero,1; addiu a3,a3,2523; sh a3,0x5378(a0); b; sh v1,0x537e(a0)
        let text = exe(&[
            word(0x21, 4, 7, 0x5380),
            word(9, 0, 3, 1),
            word(9, 7, 7, 2523),
            word(0x29, 4, 7, 0x5378),
            word(4, 0, 0, 5),
            word(0x29, 4, 3, 0x537E),
            word(9, 0, 3, 77),
        ]);
        let d = decode_switch_target(&text, EE_TO_FILE + 64).unwrap();
        assert_eq!(
            (d.base, d.groups, d.conditional),
            (Some(2523), Some(1), false)
        );
        // beqz a0 first: conditional, then the first branch's constants.
        let cond = exe(&[
            word(4, 4, 0, 3),
            0,
            word(9, 0, 3, 218),
            (3 << 16) | (20 << 11) | (16 << 6) | 0x3C,
            word(9, 0, 3, 240),
            (3 << 16) | (21 << 11) | (16 << 6) | 0x3C,
            word(4, 0, 0, 1),
            0,
        ]);
        let d = decode_switch_target(&cond, EE_TO_FILE + 64).unwrap();
        assert_eq!(
            (d.speaker, d.place, d.conditional),
            (Some(218), Some(240), true)
        );
        // No constants at all (shared "no text" continuation).
        let none = exe(&[
            word(0x21, 5, 7, 0xC2),
            word(9, 0, 3, 100),
            word(4, 0, 0, 1),
            0,
        ]);
        let d = decode_switch_target(&none, EE_TO_FILE + 64).unwrap();
        assert_eq!((d.base, d.speaker), (None, None));
        // 0x22E630 shape: ori v1,a001; bne a3,v1; nop; lh v1; addiu v1,v1,1 (page
        // counter, not a base); sh v1; b; nop.
        let shared = exe(&[
            word(0xD, 0, 3, 0xA001),
            word(5, 7, 3, 4),
            0,
            word(0x21, 4, 3, 0x5380),
            word(9, 3, 3, 1),
            word(0x29, 4, 3, 0x5380),
            word(4, 0, 0, 1),
            0,
        ]);
        let d = decode_switch_target(&shared, EE_TO_FILE + 64).unwrap();
        assert_eq!((d.base, d.groups, d.conditional), (None, None, true));
        assert!(decode_switch_target(&none, EE_TO_FILE - 4).is_err());
    }

    #[test]
    fn map_texels_keep_32_bit_palettes() {
        let texture = roses_assets::OpeningIndexedTexture {
            width: 2,
            height: 1,
            palette_format: roses_assets::OpeningPaletteFormat::Rgba8888Alpha128,
            palette_words_raw: (0..256)
                .map(|i| if i == 1 { 0x80ff_8001 } else { 0x4000_0010 })
                .collect(),
            palette_indices: vec![0, 1],
        };
        assert_eq!(
            map_rgba(&texture).unwrap(),
            [16, 0, 0, 127, 1, 128, 255, 255]
        );
    }

    #[test]
    fn message_selection_is_bounded_and_sorted() {
        let texts = vec![
            NodeText {
                node: 2,
                speaker_message: Some(199),
                place_message: Some(221),
                opponent: Some(2),
                encounter_base: Some(2523),
                encounter_groups: Some(1),
                loss_base: Some(2527),
                win_base: Some(2531),
                loss_groups: None,
                win_groups: None,
                conditional: false,
                second_visit: None,
            },
            NodeText {
                node: 11,
                speaker_message: Some(218),
                place_message: Some(240),
                opponent: Some(21),
                encounter_base: Some(2831),
                encounter_groups: Some(6),
                loss_base: None,
                win_base: None,
                loss_groups: None,
                win_groups: None,
                conditional: true,
                second_visit: None,
            },
        ];
        let ids = message_ids(&texts);
        assert!(ids.contains(&map::CAPTION_MESSAGE) && ids.contains(&2524) && ids.contains(&2535));
        assert!(ids.contains(&240) && ids.contains(&2832) && ids.contains(&2855));
        // Chester's win text (groups absent → one group): 2532..=2535 only.
        assert!(!ids.contains(&2856) && ids.contains(&2532) && !ids.contains(&2536));
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    #[ignore = "Requires the user's supported ISO (ROSES_TEST_ISO_PATH)"]
    fn private_tables_match_the_campaign_map_research() {
        let iso = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_ISO_PATH").unwrap());
        let (mut disc, _) = open_pal(&iso).unwrap();
        let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024).unwrap();
        let nodes = read_nodes(&executable).unwrap();
        // The White Rose texts (WHITE-ROUTE-AUDIT.md §1): taunt, loss and
        // win bases with groups per node, the 105/111 captions.
        let (white, captions) = white_texts(&executable, &nodes).unwrap();
        let row = |n: u8| {
            let t = white.iter().find(|t| t.node == n).unwrap();
            (
                t.speaker_message,
                t.place_message,
                t.encounter_base.zip(t.encounter_groups),
                t.loss_base.zip(t.loss_groups),
                t.win_base.zip(t.win_groups),
            )
        };
        assert_eq!(row(12), (Some(208), Some(230), Some((3103, 1)), Some((3107, 1)), Some((3111, 1))));
        assert_eq!(row(16), (Some(211), Some(233), Some((3163, 1)), Some((3167, 1)), Some((3171, 2))));
        assert_eq!(row(18), (Some(212), Some(234), Some((3183, 3)), Some((3195, 1)), Some((3199, 1))));
        assert_eq!(row(21), (Some(215), Some(237), Some((3239, 4)), Some((3255, 3)), Some((3267, 2))));
        assert_eq!(row(0), (Some(216), Some(238), Some((3275, 2)), Some((3283, 1)), Some((3287, 3))));
        assert_eq!(row(11), (Some(218), Some(240), Some((3359, 1)), Some((3363, 2)), Some((3371, 3))));
        assert_eq!(row(15), (None, Some(242), None, None, None));
        assert_eq!(row(17), (None, Some(243), None, None, None));
        assert_eq!(
            captions,
            [
                map::EventCaption { event: 105, base: 3179, groups: 1, style: 1, speaker: None },
                map::EventCaption { event: 111, base: 3299, groups: 1, style: 2, speaker: Some(198) },
            ]
        );
        assert_eq!(nodes[2].position, [-30.0, 0.0, 40.0]);
        assert_eq!((nodes[2].base_model, nodes[2].castle_model), (9, 13));
        assert_eq!((nodes[2].win_event, nodes[2].loss_event), (303, 203));
        assert_eq!(nodes[2].red_paths, [3, 4, 12, 0]);
        assert_eq!(nodes[15].facing_raw, 6.0);
        let paths = read_paths(&executable).unwrap();
        assert_eq!(paths[1].waypoints, [[-75, 65], [-50, 60], [-30, 40]]);
        assert_eq!(paths[1].dots.len(), 13);
        assert!(paths[0].sea_route && paths[0].dots.len() == 45);
        assert_eq!(paths[16].ends, [11, 12]);
        let switches = read_switches(&executable).unwrap();
        let texts = node_texts(&switches);
        assert_eq!(
            (
                texts[2].speaker_message,
                texts[2].place_message,
                texts[2].opponent
            ),
            (Some(199), Some(221), Some(2))
        );
        assert_eq!(
            (
                texts[2].encounter_base,
                texts[2].loss_base,
                texts[2].win_base
            ),
            (Some(2523), Some(2527), Some(2531))
        );
        assert_eq!(
            (texts[3].encounter_base, texts[3].encounter_groups),
            (Some(2607), Some(5))
        );
        assert_eq!(texts[7].win_base, Some(2683));
        assert!(texts[0].encounter_base.is_none() && texts[1].speaker_message.is_none());
        // Loss/win entries of nodes 0 and 1 jump to the shared close check 0x22E630.
        for t in &texts[..2] {
            assert_eq!((t.loss_base, t.win_base), (None, None));
        }
        assert!(texts[11].conditional);
        let mut texts = texts;
        stonehenge_texts(&executable, &switches, &mut texts).unwrap();
        // RED-ROUTE.md §3: Seto 198 / place 220, taunt 2831 × 6, loss 2855,
        // win 2859; Manawyddan 217 / 239, taunt 2939 × 1, loss 2943 × 2, win 2951.
        let seto = texts[11].first_visit();
        assert_eq!(
            (seto.speaker_message, seto.place_message, seto.opponent),
            (Some(198), Some(220), Some(1))
        );
        assert_eq!(
            (
                seto.encounter_base,
                seto.encounter_groups,
                seto.loss_base,
                seto.win_base
            ),
            (Some(2831), Some(6), Some(2855), Some(2859))
        );
        let m = texts[11].second_visit.clone().unwrap();
        assert_eq!(
            (m.speaker_message, m.place_message, m.opponent),
            (Some(217), Some(239), Some(20))
        );
        assert_eq!(
            (
                m.encounter_base,
                m.encounter_groups,
                m.loss_base,
                m.loss_groups,
                m.win_base,
                m.win_groups
            ),
            (
                Some(2939),
                Some(1),
                Some(2943),
                Some(2),
                Some(2951),
                Some(1)
            )
        );
        // Pegasus: win text of four groups (logs/lancashire-win: 2632..2644).
        assert_eq!(
            (texts[3].win_groups, texts[3].loss_groups),
            (Some(4), Some(1))
        );
        let ids = message_ids(&texts);
        assert!(
            ids.contains(&2644) && ids.contains(&2940) && ids.contains(&2952) && ids.contains(&239)
        );
        assert_eq!(switches.rose_nodes, [2, 4, 5, 6, 8, 9, 10, 11]);
        let block = u32_at(&executable, BANK_TABLE_EE + 4 * 35).unwrap();
        assert_eq!(block, 0x136_D800);
    }
}
