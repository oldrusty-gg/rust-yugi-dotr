//! Power-up (equip) tables from the PAL executable.
//!
//! 0x249C90 decides whether power-up `pid` fits monster `id`: never a leader;
//! pid 0x304 needs a Zombie; otherwise bit ((pid − 0x2F0) & 7) of byte
//! 0x29C970 + id·7 + ((pid − 0x2F0) >> 3). The power-up's category is byte
//! 0x29C5E0 + pid and its value the u16 at 0x29C322 + pid·2 (category 0 adds
//! the value to ATK and DEF; category 4 transforms the monster). Raw data
//! only; behaviour lives in `roses-core`.

use super::{require_pal_executable, Error, Result, CARD_COUNT, PROFILE_ID};
use serde::Serialize;

/// First and last power-up card ids (kind 64) in the PAL table.
pub const FIRST_POWER_UP_ID: u16 = 0x2F0;
pub const LAST_POWER_UP_ID: u16 = 0x320;
/// Monster ids that have a compatibility row (0..0x2AB).
pub const MONSTER_ROWS: usize = 0x2AB;
const EE_TO_FILE: usize = 0xF_9680;
const COMPATIBILITY_EE: usize = 0x29_C970;
const CATEGORY_EE: usize = 0x29_C5E0;
const VALUE_EE: usize = 0x29_C322;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PowerUpData {
    pub id: u16,
    pub category: u8,
    pub value: u16,
}

#[derive(Clone, Debug, Serialize)]
pub struct EquipTable {
    pub schema_version: u32,
    pub profile_id: String,
    pub executable_sha256: String,
    /// Seven compatibility bytes per monster id (index = id).
    pub compatibility: Vec<[u8; 7]>,
    pub power_ups: Vec<PowerUpData>,
    pub gameplay_validated: bool,
}

pub fn parse_pal_equips(executable: &[u8]) -> Result<EquipTable> {
    let executable_sha256 = require_pal_executable(executable)?;
    let bytes = |ee: usize, len: usize| {
        executable
            .get(ee - EE_TO_FILE..ee - EE_TO_FILE + len)
            .ok_or_else(|| Error::Invalid("equip table lies outside executable".into()))
    };
    let rows = bytes(COMPATIBILITY_EE, MONSTER_ROWS * 7)?;
    let compatibility = rows
        .chunks_exact(7)
        .map(|r| r.try_into().unwrap())
        .collect();
    let mut power_ups = Vec::new();
    for id in FIRST_POWER_UP_ID..=LAST_POWER_UP_ID {
        debug_assert!(usize::from(id) < CARD_COUNT);
        let category = bytes(CATEGORY_EE + usize::from(id), 1)?[0];
        let v = bytes(VALUE_EE + usize::from(id) * 2, 2)?;
        power_ups.push(PowerUpData {
            id,
            category,
            value: u16::from_le_bytes([v[0], v[1]]),
        });
    }
    Ok(EquipTable {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        executable_sha256,
        compatibility,
        power_ups,
        gameplay_validated: false,
    })
}
