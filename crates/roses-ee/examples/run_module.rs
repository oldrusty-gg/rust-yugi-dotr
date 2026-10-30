//! Research runner: run a B module from a saved EE memory image (private
//! footage state) and print each call's return and primitive count, for
//! comparison with the Python interpreter's runs.
//! `run_module <state.bin> <BMODULE.MRG> <record> <entry hex> <target hex> <self hex> <param> [calls]`
use roses_ee::{Cpu, Hook};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let state = std::fs::read(&a[1]).unwrap();
    let modules = std::fs::read(&a[2]).unwrap();
    let record: usize = a[3].parse().unwrap();
    let (entry, target, own) = (hex(&a[4]), hex(&a[5]), hex(&a[6]));
    let param: i64 = a[7].parse().unwrap();
    let calls: usize = a.get(8).map_or(2000, |s| s.parse().unwrap());
    let mut cpu = Cpu::new();
    if state.get(..4) == Some(b"ELF") {
        cpu.load_elf(&state).unwrap();
    } else {
        cpu.load(0, &state[..state.len().min(roses_ee::MAIN_BYTES)]);
    }
    cpu.load(0x1E0_0000, &modules[record * 0x8000..(record + 1) * 0x8000]);
    let run = |cpu: &mut Cpu, pc: u32, args: &[u64], prims: &mut usize| {
        cpu.call(pc, args, 0x1F0_0000, 300_000_000, &mut |c, pc| match pc {
            0x206F30 => {
                *prims += 1;
                Hook::Return(Some(0))
            }
            0x1F8270 | 0x1F8280 | 0x1F82B0 => Hook::Return(Some(1)),
            0x2078A0 => Hook::Return(Some(0x1F9_0000)),
            0x207910 | 0x19A410 => Hook::Return(Some(0)),
            0x1A9080 => Hook::Return(Some(0)),
            _ => {
                let _ = c;
                Hook::Run
            }
        })
    };
    let mut n = 0;
    run(&mut cpu, 0x1F6580, &[u64::from(target)], &mut n).unwrap();
    run(&mut cpu, 0x1F65A0, &[u64::from(own)], &mut n).unwrap();
    let start = std::time::Instant::now();
    let mut total_steps = 0u64;
    for k in 0..calls {
        let arg = if k == 0 { param } else { -1 };
        let mut prims = 0;
        let r = match run(&mut cpu, entry, &[0x1ED_0000, arg as u64], &mut prims) {
            Ok(r) => r as u32 as i32,
            Err(e) => {
                println!("STOP {k} {e}");
                break;
            }
        };
        total_steps += cpu.steps;
        println!("{k} {r} {prims} {}", cpu.steps);
        if r == 8 {
            break;
        }
    }
    eprintln!("{total_steps} instructions in {:?}", start.elapsed());
}
