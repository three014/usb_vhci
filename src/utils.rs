pub enum TimeoutMillis {
    Unlimited,
    Time(ClosedBoundedI16<1, 1000>),
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct OpenBoundedU8<const LOWER: u8, const UPPER: u8>(u8);

impl<const LOWER: u8, const UPPER: u8> OpenBoundedU8<LOWER, UPPER> {
    pub const fn new(num: u8) -> Option<Self> {
        if LOWER > num || UPPER < num {
            None
        } else {
            Some(Self(num))
        }
    }

    pub const fn get(&self) -> u8 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct ClosedBoundedI16<const LOWER: i16, const UPPER: i16>(i16);

impl<const LOWER: i16, const UPPER: i16> ClosedBoundedI16<LOWER, UPPER> {
    pub const fn new(num: i16) -> Option<Self> {
        if LOWER >= num || UPPER <= num {
            None
        } else {
            Some(Self(num))
        }
    }

    pub const fn get(&self) -> i16 {
        self.0
    }
}
