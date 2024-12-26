use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutMillis {
    // Unlimited, TODO: Find out why this times out immediately?
    Time(BoundedI16<0, 1000>),
}

impl TimeoutMillis {
    pub const fn from_duration(dur: Duration) -> Option<TimeoutMillis> {
        let millis = dur.as_millis();
        if 1000 <= millis {
            None
        } else {
            Some(Self::Time(BoundedI16::new(millis as i16).unwrap()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BoundedU8<const LOWER_INC: u8, const UPPER_EX: u8>(u8);

impl<const LOWER_INC: u8, const UPPER_EX: u8> BoundedU8<LOWER_INC, UPPER_EX> {
    pub const fn new(num: u8) -> Option<Self> {
        if LOWER_INC > num || UPPER_EX <= num {
            None
        } else {
            Some(Self(num))
        }
    }

    pub const fn get(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BoundedU16<const LOWER_INC: u16, const UPPER_EX: u16>(u16);

impl<const LOWER_INC: u16, const UPPER_EX: u16> BoundedU16<LOWER_INC, UPPER_EX> {
    pub const fn new(num: u16) -> Option<Self> {
        if LOWER_INC > num || UPPER_EX <= num {
            None
        } else {
            Some(Self(num))
        }
    }

    pub const fn get(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BoundedI16<const LOWER_INC: i16, const UPPER_EX: i16>(i16);

impl<const LOWER_INC: i16, const UPPER_EX: i16> BoundedI16<LOWER_INC, UPPER_EX> {
    pub const fn new(num: i16) -> Option<Self> {
        if LOWER_INC > num || UPPER_EX <= num {
            None
        } else {
            Some(Self(num))
        }
    }

    pub const fn get(&self) -> i16 {
        self.0
    }
}
