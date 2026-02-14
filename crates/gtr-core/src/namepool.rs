use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const MAD_MAX_NAMES: &[&str] = &[
    "nux", "slit", "rictus", "furiosa", "capable", "toast",
    "cheedo", "dag", "angharad", "dementus", "scrotus",
    "morsov", "ace", "valkyrie", "keeper", "glory",
    "corpus", "praetorian", "buzzard", "rock-rider",
];

/// Pick the next available name from the pool.
/// Cycles through names; appends a suffix if pool is exhausted.
pub fn next_name() -> String {
    let idx = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = MAD_MAX_NAMES[idx % MAD_MAX_NAMES.len()];
    if idx < MAD_MAX_NAMES.len() {
        base.to_string()
    } else {
        format!("{base}-{}", idx / MAD_MAX_NAMES.len())
    }
}

/// Reset the counter (useful for tests).
pub fn reset() {
    COUNTER.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cycle_with_suffix() {
        reset();
        let first = next_name();
        assert_eq!(first, "nux");
        // Exhaust pool
        for _ in 1..MAD_MAX_NAMES.len() {
            next_name();
        }
        let overflow = next_name();
        assert_eq!(overflow, "nux-1");
    }
}
