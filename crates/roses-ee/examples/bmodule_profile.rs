//! Research: where a B module's instructions go. Runs `updates` updates from
//! a private footage state and prints the instruction counts per function
//! (function = the nearest jal target at or below the pc, from the
//! executable's and the module's own jal instructions).
//! `bmodule_profile <state.bin> <BMODULE.MRG> <record> <card> <self hex> <target hex> <updates>`
use roses_ee::{Cpu, Hook};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let ram = std::fs::read(&a[1]).unwrap();
    let modules = std::fs::read(&a[2]).unwrap();
    let record: usize = a[3].parse().unwrap();
    let card: u16 = a[4].parse().unwrap();
    let (own, target) = (hex(&a[5]), hex(&a[6]));
    let updates: usize = a[7].parse().unwrap();
    let mut cpu = Cpu::new();
    cpu.load(0, &ram[..ram.len().min(roses_ee::MAIN_BYTES)]);
    cpu.load(0x1E0_0000, &modules[record * 0x8000..(record + 1) * 0x8000]);
    let (_, entry) = roses_ee::bmodule::entry_of(&mut cpu, card).unwrap();
    // Function starts: jal targets in the code ranges.
    let mut starts = std::collections::BTreeSet::new();
    for range in [(0x10_0000u32, 0x22_0000u32), (0x1E0_0000, 0x1E0_8000)] {
        for at in (range.0..range.1).step_by(4) {
            let w = cpu.read_u32(at);
            if w >> 26 == 3 {
                starts.insert((w & 0x3FF_FFFF) << 2);
            }
        }
    }
    let mut h = |_: &mut Cpu, pc: u32| match pc {
        0x206F30 | 0x206F90 => Hook::Return(Some(0)),
        0x1F8270 | 0x1F8280 | 0x1F82B0 => Hook::Return(Some(1)),
        0x2078A0 => Hook::Return(Some(0x1F9_0000)),
        0x207910 | 0x19A410 | 0x1A9080 => Hook::Return(Some(0)),
        _ => Hook::Run,
    };
    cpu.call(0x1F6580, &[u64::from(target)], 0x1F0_0000, 10_000, &mut h).unwrap();
    cpu.call(0x1F65A0, &[u64::from(own)], 0x1F0_0000, 10_000, &mut h).unwrap();
    cpu.histogram = Some(vec![0; 1 << 23]);
    let t = std::time::Instant::now();
    let mut total = 0;
    for _ in 0..updates {
        cpu.call(0x1F6580, &[u64::from(target)], 0x1F0_0000, 10_000, &mut h).unwrap();
        cpu.call(0x1F65A0, &[u64::from(own)], 0x1F0_0000, 10_000, &mut h).unwrap();
        cpu.call(entry, &[0x1ED_0000, -1i64 as u64], 0x1F0_0000, 300_000_000, &mut h).unwrap();
        total += cpu.steps;
    }
    let el = t.elapsed();
    let hist = cpu.histogram.take().unwrap();
    let mut per: std::collections::BTreeMap<u32, u64> = Default::default();
    for (i, n) in hist.iter().enumerate() {
        if *n > 0 {
            let pc = (i as u32) << 2;
            let f = starts.range(..=pc).next_back().copied().unwrap_or(0);
            *per.entry(f).or_insert(0) += u64::from(*n);
        }
    }
    let mut v: Vec<_> = per.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    let sum: u64 = v.iter().map(|x| x.1).sum();
    println!("{total} steps in {el:?} ({:.0} MIPS)", total as f64 / el.as_secs_f64() / 1e6);
    for (f, n) in v.iter().take(30) {
        println!("{f:08x} {n:>10} {:5.1}%", 100.0 * *n as f64 / sum as f64);
    }
}
