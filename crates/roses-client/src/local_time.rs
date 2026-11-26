//! The player's local time for a Unix time (a port addition, for the
//! online tournament times): the operating system's time zone rules for
//! that date (Windows: `SystemTimeToTzSpecificLocalTime`, so summer time
//! is right for a date in the other half of the year). Elsewhere UTC.

/// A calendar time: year, month (1-12), day, weekday (0 = Monday), hour,
/// minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub year: i64,
    pub month: u8,
    pub day: u8,
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
}

/// The UTC calendar time of `unix` seconds.
pub fn utc(unix: i64) -> Clock {
    let days = unix.div_euclid(86_400);
    let rem = unix.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Clock {
        year,
        month: month as u8,
        day: day as u8,
        // 1970-01-01 was a Thursday.
        weekday: (days + 3).rem_euclid(7) as u8,
        hour: (rem / 3600) as u8,
        minute: (rem % 3600 / 60) as u8,
    }
}

/// The local calendar time of `unix` seconds (UTC where the system cannot
/// say).
pub fn local(unix: i64) -> Clock {
    #[cfg(windows)]
    {
        if let Some(c) = windows_local(unix) {
            return c;
        }
    }
    utc(unix)
}

#[cfg(windows)]
fn windows_local(unix: i64) -> Option<Clock> {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::Time::SystemTimeToTzSpecificLocalTime;
    let u = utc(unix);
    let st = SYSTEMTIME {
        wYear: u16::try_from(u.year).ok()?,
        wMonth: u16::from(u.month),
        wDayOfWeek: u16::from((u.weekday + 1) % 7),
        wDay: u16::from(u.day),
        wHour: u16::from(u.hour),
        wMinute: u16::from(u.minute),
        wSecond: 0,
        wMilliseconds: 0,
    };
    let mut out = st;
    // SAFETY: both pointers are to valid SYSTEMTIMEs; a null time zone means
    // the current one.
    let ok = unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &st, &mut out) };
    if ok == 0 {
        return None;
    }
    Some(Clock {
        year: i64::from(out.wYear),
        month: out.wMonth as u8,
        day: out.wDay as u8,
        weekday: ((out.wDayOfWeek + 6) % 7) as u8,
        hour: out.wHour as u8,
        minute: out.wMinute as u8,
    })
}

pub const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

impl Clock {
    /// "Fri 4 Dec 19:00".
    pub fn short(&self) -> String {
        format!(
            "{} {} {} {:02}:{:02}",
            WEEKDAYS[usize::from(self.weekday % 7)],
            self.day,
            MONTHS[usize::from(self.month.clamp(1, 12) - 1)],
            self.hour,
            self.minute
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_calendar_and_weekday() {
        let c = utc(1_796_410_800);
        assert_eq!((c.year, c.month, c.day, c.weekday, c.hour, c.minute), (2026, 12, 4, 4, 19, 0));
        assert_eq!(c.short(), "Fri 4 Dec 19:00");
        assert_eq!(utc(0).short(), "Thu 1 Jan 00:00");
        // The local time is a whole calendar time (any zone).
        let l = local(1_796_410_800);
        assert!((1..=12).contains(&l.month) && l.weekday < 7 && l.hour < 24);
    }
}
