/// Transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Dir {
    /// Host to device.
    Out = 0,

    /// Device to host.
    In = 1,
}

impl Dir {
    pub const fn from_u8(num: u8) -> Option<Self> {
        match num {
            0 => Some(Dir::Out),
            1 => Some(Dir::In),
            _ => None,
        }
    }
}

/// Specification defining the request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CtrlType {
    /// Request defined by the USB standard.
    Standard = 0,

    /// Request defined by the standard USB class specification.
    Class = 1,

    /// Non-standard request.
    Vendor = 2,
}

impl CtrlType {
    pub const fn from_u8(num: u8) -> Option<Self> {
        match num {
            0 => Some(Self::Standard),
            1 => Some(Self::Class),
            2 => Some(Self::Vendor),
            _ => None,
        }
    }
}

/// Entity targeted by the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Recipient {
    /// Request made to device as a whole.
    Device = 0,

    /// Request made to specific interface.
    Interface = 1,

    /// Request made to specific endpoint.
    Endpoint = 2,

    /// Other request.
    Other = 3,
}

impl Recipient {
    pub const fn from_u8(num: u8) -> Option<Self> {
        match num {
            0 => Some(Self::Device),
            1 => Some(Self::Interface),
            2 => Some(Self::Endpoint),
            3 => Some(Self::Other),
            _ => None,
        }
    }
}
