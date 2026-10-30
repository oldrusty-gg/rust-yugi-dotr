//! Ritual requirements from the PAL executable.
//!
//! A ritual card (class 5, card byte 0 = 0xA0) has one effect entry whose
//! word 1 is the monster it creates. 0x2565B0(out, result) is a leaf switch
//! on that result id storing three requirement words (filter = low 16 bits,
//! parameter = high 16 bits); any other id stores −1. The switch is run here
//! by a small interpreter that knows only the instructions it uses (addiu,
//! lui, ori, sw, beq, beql, jr, nop), so no table is copied out of the game.
//! Raw data only; the selection rules live in `roses-core`.

use super::{
    parse_pal_card_effects, require_pal_executable, Error, Result, CARD_COUNT, CARD_RECORD_STRIDE,
    CARD_TABLE_FILE_OFFSET, PROFILE_ID,
};
use serde::Serialize;

const EE_TO_FILE: u32 = 0xF_9680;
/// 0x2565B0 in EE memory.
pub const RITUAL_REQUIREMENTS_EE: u32 = 0x25_65B0;
const RITUAL_CLASS: u8 = 5;
/// Entry-0 type of every ritual card (the builder 0x2560B8).
const RITUAL_TYPE: u32 = 0x32;
const STEP_LIMIT: usize = 4096;
/// Filters the requirement words may use: kinds 0x00–0x13, card id 0x1C,
/// ATK ≤ p 0x25, DEF ≥ p 0x39.
fn known_filter(word: u32) -> bool {
    matches!(word & 0xFFFF, 0..=0x13 | 0x1C | 0x25 | 0x39)
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RitualData {
    /// The ritual card.
    pub ritual: u16,
    /// The monster it creates (entry word 1).
    pub result: u16,
    /// The three requirement words of 0x2565B0.
    pub words: [u32; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct RitualTable {
    pub schema_version: u32,
    pub profile_id: String,
    pub executable_sha256: String,
    pub rituals: Vec<RitualData>,
    pub gameplay_validated: bool,
}

pub fn parse_pal_rituals(executable: &[u8]) -> Result<RitualTable> {
    let executable_sha256 = require_pal_executable(executable)?;
    let effects = parse_pal_card_effects(executable)?;
    let mut rituals = Vec::new();
    for id in 0..CARD_COUNT {
        let class = executable[CARD_TABLE_FILE_OFFSET + id * CARD_RECORD_STRIDE] >> 5;
        if class != RITUAL_CLASS {
            continue;
        }
        let entry = effects.cards[id].entries[0]
            .ok_or_else(|| invalid(format!("ritual card {id} has no effect entry")))?;
        if entry.word0 >> 8 != RITUAL_TYPE {
            return Err(invalid(format!(
                "ritual card {id} has entry type {:#x}",
                entry.word0 >> 8
            )));
        }
        let result = u16::try_from(entry.word1)
            .ok()
            .filter(|&r| usize::from(r) < CARD_COUNT)
            .ok_or_else(|| invalid(format!("ritual card {id} names no card")))?;
        let words = requirements(executable, result)?;
        if words.iter().any(|&w| w == u32::MAX || !known_filter(w)) {
            return Err(invalid(format!(
                "ritual result {result} has unknown requirements"
            )));
        }
        rituals.push(RitualData {
            ritual: id as u16,
            result,
            words,
        });
    }
    Ok(RitualTable {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        executable_sha256,
        rituals,
        gameplay_validated: false,
    })
}

fn invalid(message: String) -> Error {
    Error::Invalid(format!("PAL rituals: {message}"))
}

/// Runs 0x2565B0 with a1 = `result` and returns the three stored words.
pub fn requirements(executable: &[u8], result: u16) -> Result<[u32; 3]> {
    const A0: usize = 4;
    const A1: usize = 5;
    const RA: usize = 31;
    let fetch = |pc: u32| -> Result<u32> {
        let at = pc
            .checked_sub(EE_TO_FILE)
            .map(|a| a as usize)
            .ok_or_else(|| invalid(format!("pc {pc:#x} is outside the executable")))?;
        executable
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| invalid(format!("pc {pc:#x} is outside the executable")))
    };
    let mut reg = [0u32; 32];
    reg[A1] = u32::from(result);
    let mut out = [0u32; 3];
    let (mut pc, mut npc) = (RITUAL_REQUIREMENTS_EE, RITUAL_REQUIREMENTS_EE + 4);
    // Set by jr: the next instruction is its delay slot, then the call ends.
    let mut in_delay_of_return = false;
    for _ in 0..STEP_LIMIT {
        let insn = fetch(pc)?;
        let (rs, rt) = ((insn >> 21 & 31) as usize, (insn >> 16 & 31) as usize);
        let imm = insn & 0xFFFF;
        let simm = imm as u16 as i16 as i32 as u32;
        let mut target = npc.wrapping_add(4);
        let mut annul = false;
        let mut returns = false;
        match insn >> 26 {
            0 if insn == 0 => (),
            0 if insn & 0x3F == 0x08 && rs == RA && !in_delay_of_return => returns = true,
            0x09 => reg[rt] = reg[rs].wrapping_add(simm),
            0x0D => reg[rt] = reg[rs] | imm,
            0x0F => reg[rt] = imm << 16,
            0x2B => {
                if rs != A0 || !matches!(simm, 0 | 4 | 8) {
                    return Err(invalid(format!("store at {pc:#x} outside the result")));
                }
                out[simm as usize / 4] = reg[rt];
            }
            op @ (0x04 | 0x14) if !in_delay_of_return => {
                if reg[rs] == reg[rt] {
                    target = npc.wrapping_add(simm << 2);
                } else if op == 0x14 {
                    // beql: the delay slot runs only when the branch is taken.
                    annul = true;
                }
            }
            _ => {
                return Err(invalid(format!(
                    "unexpected instruction {insn:#010x} at {pc:#x}"
                )))
            }
        }
        reg[0] = 0;
        if in_delay_of_return {
            return Ok(out);
        }
        in_delay_of_return = returns;
        if annul {
            pc = npc.wrapping_add(4);
            npc = pc.wrapping_add(4);
        } else {
            pc = npc;
            npc = target;
        }
    }
    Err(invalid(format!(
        "0x2565B0 did not return for result {result}"
    )))
}
