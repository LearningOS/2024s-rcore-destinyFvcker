//! RISC-V timer-related functionality

use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use riscv::register::time;
/// The number of ticks per second
const TICKS_PER_SEC: usize = 100;
/// The number of milliseconds per second
const MSEC_PER_SEC: usize = 1000;
/// The number of microseconds per second
const MICRO_PER_SEC: usize = 1_000_000;

/// The struct that record time
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeVal {
    /// second
    pub sec: usize,
    /// microsecond
    pub usec: usize,
}

impl TimeVal {
    /// The structure is updated with the current time information
    pub fn update(&mut self) {
        let us = get_time_us();

        self.sec = us / 1_000_000;
        self.usec = us % 1_000_000;
    }

    /// Turn the representation of time from TimeVal to the form of ms
    pub fn as_ms(&self) -> usize {
        self.sec * 1_000 + self.usec / 1_000
    }
}

/// Get the current time in ticks
pub fn get_time() -> usize {
    time::read()
}

/// get current time in milliseconds
pub fn get_time_ms() -> usize {
    time::read() * MSEC_PER_SEC / CLOCK_FREQ
}

/// get current time in microseconds
pub fn get_time_us() -> usize {
    time::read() * MICRO_PER_SEC / CLOCK_FREQ
}

/// Set the next timer interrupt
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}
