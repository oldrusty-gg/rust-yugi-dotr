//! Research: which game state a B module reads. Runs a module from a saved
//! EE memory image (private footage state) for some updates and prints every
//! address read or written outside the module's own image / data / stack,
//! with the first pc and a count, grouped by 16-byte line.
//! `trace_state <state.bin> <BMODULE.MRG> <record> <entry hex> <target hex> <self hex> <updates> [first arg]`
use roses_ee::{Cpu, Hook};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let state = std::fs::read(&a[1]).unwrap();
    let modules = std::fs::read(&a[2]).unwrap();
    let record: usize = a[3].parse().unwrap();
    let (entry, target, own) = (hex(&a[4]), hex(&a[5]), hex(&a[6]));
    let updates: usize = a[7].parse().unwrap();
    let first: i64 = a.get(8).map_or(-1, |s| s.parse().unwrap());
    let mut cpu = Cpu::new();
    cpu.load(0, &state[..state.len().min(roses_ee::MAIN_BYTES)]);
    cpu.load(0x1E0_0000, &modules[record * 0x8000..(record + 1) * 0x8000]);
    type Seen = BTreeMap<u32, (u32, u32, bool, bool)>;
    let seen: Arc<Mutex<Seen>> = Default::default();
    let s2 = seen.clone();
    let hooks = |_: &mut Cpu, pc: u32| match pc {
        0x206F30 | 0x206F90 => Hook::Return(Some(0)),
        0x1F8270 | 0x1F8280 | 0x1F82B0 => Hook::Return(Some(1)),
        0x2078A0 => Hook::Return(Some(0x1F9_0000)),
        0x207910 | 0x19A410 | 0x1A9080 => Hook::Return(Some(0)),
        _ => Hook::Run,
    };
    let mut h = hooks;
    cpu.call(0x1F6580, &[u64::from(target)], 0x1F0_0000, 10_000, &mut h).unwrap();
    cpu.call(0x1F65A0, &[u64::from(own)], 0x1F0_0000, 10_000, &mut h).unwrap();
    cpu.tracer = Some(Box::new(move |pc, addr, store| {
        let addr = addr & 0x1FF_FFFF | if addr >> 28 == 7 { 0x7000_0000 } else { 0 };
        let own_area = (0x1E0_0000..0x1F0_0000).contains(&addr) || addr >= 0x7000_0000 || (0x1EF_0000..0x1F0_0000).contains(&addr);
        if own_area {
            return;
        }
        let mut m = s2.lock().unwrap();
        let e = m.entry(addr).or_insert((pc, 0, false, false));
        e.1 += 1;
        if store {
            e.3 = true;
        } else {
            e.2 = true;
        }
    }));
    for k in 0..updates {
        let arg = if k == 0 { first } else { -1 };
        let r = cpu.call(entry, &[0x1ED_0000, arg as u64], 0x1F0_0000, 300_000_000, &mut h);
        eprintln!("update {k}: {r:?} {} steps", cpu.steps);
    }
    let m = seen.lock().unwrap();
    let mut lines: BTreeMap<u32, Vec<(u32, u32, u32, bool, bool)>> = BTreeMap::new();
    for (addr, (pc, n, r, w)) in m.iter() {
        lines.entry(addr & !15).or_default().push((*addr, *pc, *n, *r, *w));
    }
    for (line, v) in lines {
        let desc: Vec<String> = v
            .iter()
            .map(|(a, pc, n, r, w)| format!("+{:x}{}{} pc{:x} n{}", a & 15, if *r { "r" } else { "" }, if *w { "w" } else { "" }, pc, n))
            .collect();
        println!("{line:08x}: {}", desc.join("; "));
    }
}
