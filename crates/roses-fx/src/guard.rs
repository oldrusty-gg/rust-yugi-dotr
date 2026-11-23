//! Logged guards for task steps a ported record does not run (issue 15).
//!
//! Some records' entries test steps that nothing reaches: no spawn site in
//! live code, never a data word (`DISPATCH-AUDIT.md`, 4 Nov 2026, sections
//! A and B), so the ports leave them out. If one ever turns up in a task
//! slot, the record still skips it exactly as before (nothing is drawn or
//! stepped for it), but it now says so once per record and step on stderr
//! (the game's `game.log`), so a wrong port shows up instead of a silently
//! missing piece of an effect. A debug build also stops there, as the
//! records' old `debug_assert!`s did. Nothing here changes what is drawn,
//! and the checks only read the step word.

use std::collections::BTreeSet;
use std::sync::Mutex;

/// (record, step) pairs already reported.
static SEEN: Mutex<BTreeSet<(&'static str, u32)>> = Mutex::new(BTreeSet::new());

/// At most this many different reports (bounded memory and log).
const MAX_REPORTS: usize = 256;

/// Report `step` of `record` once: true the first time (it was logged).
pub fn note(record: &'static str, step: u32) -> bool {
    let first = match SEEN.lock() {
        Ok(mut seen) => seen.len() < MAX_REPORTS && seen.insert((record, step)),
        Err(_) => false,
    };
    if first {
        eprintln!("roses-fx: {record}: task step {step:#x} is not ported and was skipped (please report this line)");
    }
    first
}

/// A step `record` does not port came up: logged once; a debug build stops.
pub fn unported_step(record: &'static str, step: u32) {
    note(record, step);
    debug_assert!(false, "{record}: unported task step {step:#x}");
}

/// [`unported_step`] if `step` is one of the record's `unported` steps
/// (the audit's list); any other value is left to the record's dispatch.
#[inline]
pub fn check_step(record: &'static str, step: u32, unported: &[u32]) {
    if unported.contains(&step) {
        unported_step(record, step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skipped_step_is_logged_once_per_record_and_step() {
        assert!(note("guard-test", 0x01E0_1000));
        assert!(!note("guard-test", 0x01E0_1000));
        assert!(note("guard-test", 0x01E0_1010));
        assert!(note("guard-test-2", 0x01E0_1000));
    }

    #[test]
    fn a_step_not_on_the_list_is_not_reported() {
        check_step("guard-test-3", 0x01E0_2000, &[0x01E0_2010, 0x01E0_2020]);
        // Not reported above, so this is its first report.
        assert!(note("guard-test-3", 0x01E0_2000));
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn a_release_build_logs_and_goes_on() {
        check_step("guard-test-4", 0x01E0_3000, &[0x01E0_3000]);
        unported_step("guard-test-4", 0x01E0_3000);
        assert!(!note("guard-test-4", 0x01E0_3000));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "unported task step")]
    fn a_debug_build_stops_at_a_skipped_step() {
        check_step("guard-test-5", 0x01E0_4000, &[0x01E0_4000]);
    }
}
