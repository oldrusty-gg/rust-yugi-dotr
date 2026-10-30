//! The battle's attack effects (BMODULE.MRG) run headless: the return code
//! of every call, which drives the battle (contact, hit and hold, the
//! target's reaction, the module-end camera, the end) [F/emu
//! `claude-units/BMODULE-SCOPE.md`].
//!
//! - BMODULE record = card id; the module is linked at 0x1E00000 and keeps
//!   its state in the data area 0x1ED0000.
//! - Its entry comes from the executable's type-1 table 0x2B94C0, indexed by
//!   the monster model index (card = u16 0x2CE024[model], model < 854,
//!   card < 683).
//! - Launch: `entry(data, param)`; each later update `entry(data, -1)`;
//!   return 8 ends it. B220 and B125 return the footage's sequences from
//!   the plain executable image (no game state) [emu].
//!
//! Draw output (0x206F30) and the other boundary calls are answered by
//! hooks here (no draws are produced).
use crate::{Cpu, Hook, Stop};

pub const LINK_ADDRESS: u32 = 0x01E0_0000;
pub const DATA_AREA: u32 = 0x01ED_0000;
pub const RECORD_BYTES: usize = 0x8000;
const ENTRY_TABLE: u32 = 0x002B_94C0;
const MODEL_CARD_TABLE: u32 = 0x002C_E024;
/// Model indices: the type-1 entry table and u16 0x2CE024[model] have 854
/// (0x356) entries (the table ends where 0x2CE6D0 begins); records (cards)
/// are below 683 [F].
const MODEL_COUNT: u16 = 854;
const SET_TARGET: u32 = 0x001F_6580;
const SET_SELF: u32 = 0x001F_65A0;
const STACK: u32 = 0x01F0_0000;
/// Calls before a module is given up (the longest observed is 341).
pub const MAX_CALLS: usize = 4000;
const STEPS_PER_CALL: u64 = 300_000_000;

/// Headless boundary: draws and unit-model draws are dropped.
fn headless(_: &mut Cpu, pc: u32) -> Hook {
    match pc {
        0x0020_6F30 | 0x0020_6F90 => Hook::Return(Some(0)),
        0x001F_8270 | 0x001F_8280 | 0x001F_82B0 => Hook::Return(Some(1)),
        0x0020_78A0 => Hook::Return(Some(0x01F9_0000)),
        0x0020_7910 | 0x0019_A410 | 0x001A_9080 => Hook::Return(Some(0)),
        _ => Hook::Run,
    }
}

/// The monster model index of `card` (inverse of u16 0x2CE024[model]).
pub fn model_index(cpu: &mut Cpu, card: u16) -> Option<u16> {
    if card >= 683 {
        return None;
    }
    (0..MODEL_COUNT).find(|&m| cpu.read_u16(MODEL_CARD_TABLE + 2 * u32::from(m)) == card)
}

/// Every return of `card`'s attack effect from launch (index 0) to its
/// return 8, with `target` / `own` unit handles (side << 8 | slot);
/// `None` when the card has no model index or no module record.
pub fn attack_effect_returns(
    executable: &[u8],
    bmodule: &[u8],
    card: u16,
    target: u16,
    own: u16,
) -> Result<Option<Vec<i32>>, Stop> {
    let mut cpu = Cpu::new();
    cpu.load_elf(executable)?;
    let Some(model) = model_index(&mut cpu, card) else {
        return Ok(None);
    };
    let start = usize::from(card) * RECORD_BYTES;
    let Some(record) = bmodule.get(start..start + RECORD_BYTES) else {
        return Ok(None);
    };
    if record.get(..4) != Some(b"MWo3") {
        return Ok(None);
    }
    let entry = cpu.read_u32(ENTRY_TABLE + 4 * u32::from(model));
    if !(LINK_ADDRESS..LINK_ADDRESS + RECORD_BYTES as u32).contains(&entry) {
        return Ok(None);
    }
    cpu.load(LINK_ADDRESS, record);
    cpu.call(SET_TARGET, &[u64::from(target)], STACK, 10_000, &mut headless)?;
    cpu.call(SET_SELF, &[u64::from(own)], STACK, 10_000, &mut headless)?;
    let mut returns = Vec::new();
    for k in 0..MAX_CALLS {
        let arg: i64 = if k == 0 { 0 } else { -1 };
        let r = cpu.call(entry, &[u64::from(DATA_AREA), arg as u64], STACK, STEPS_PER_CALL, &mut headless)? as u32 as i32;
        returns.push(r);
        if r == 8 {
            return Ok(Some(returns));
        }
    }
    Err(Stop(format!("card {card}'s attack effect did not end in {MAX_CALLS} calls")))
}
