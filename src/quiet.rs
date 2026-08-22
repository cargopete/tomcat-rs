//! Quiet hours.
//!
//! The only interesting case is a window that wraps past midnight, which the
//! default 22:00 to 07:00 does, and which is the classic off-by-one trap.

/// True if `hour` falls inside the half-open window `[start, end)`.
///
/// `start == end` means the window is empty, which is how quiet hours are
/// disabled. Note that is the opposite reading to "start == end covers the
/// whole day", and it is deliberate: an accidentally equal pair should leave
/// the deterrent working, not silence it for good.
pub fn in_quiet_hours(hour: u32, start: u32, end: u32) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        (start..end).contains(&hour)
    } else {
        hour >= start || hour < end
    }
}

#[cfg(test)]
mod tests {
    use super::in_quiet_hours;

    // The real default: 22:00 -> 07:00, wrapping past midnight.
    #[test]
    fn wrapping_window_is_quiet_late_evening() {
        assert!(in_quiet_hours(22, 22, 7));
        assert!(in_quiet_hours(23, 22, 7));
    }

    #[test]
    fn wrapping_window_is_quiet_after_midnight() {
        assert!(in_quiet_hours(0, 22, 7));
        assert!(in_quiet_hours(3, 22, 7));
        assert!(in_quiet_hours(6, 22, 7));
    }

    #[test]
    fn wrapping_window_ends_exclusively_at_seven() {
        assert!(!in_quiet_hours(7, 22, 7), "07:00 is when it wakes up");
        assert!(!in_quiet_hours(8, 22, 7));
    }

    #[test]
    fn wrapping_window_is_loud_through_the_day() {
        for h in 7..22 {
            assert!(!in_quiet_hours(h, 22, 7), "{h}:00 should not be quiet");
        }
    }

    #[test]
    fn non_wrapping_window() {
        assert!(!in_quiet_hours(8, 9, 17));
        assert!(in_quiet_hours(9, 9, 17), "start is inclusive");
        assert!(in_quiet_hours(16, 9, 17));
        assert!(!in_quiet_hours(17, 9, 17), "end is exclusive");
    }

    #[test]
    fn equal_bounds_disable_quiet_hours() {
        for h in 0..24 {
            assert!(!in_quiet_hours(h, 0, 0));
            assert!(!in_quiet_hours(h, 13, 13));
        }
    }

    #[test]
    fn every_hour_is_covered_exactly_once_across_a_wrap() {
        let quiet = (0..24).filter(|&h| in_quiet_hours(h, 22, 7)).count();
        assert_eq!(quiet, 9, "22,23,0,1,2,3,4,5,6");
    }
}
