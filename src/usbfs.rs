use crate::ioctl::{
    URB_RQ_CLEAR_FEATURE, URB_RQ_GET_CONFIGURATION, URB_RQ_GET_DESCRIPTOR, URB_RQ_GET_INTERFACE,
    URB_RQ_SET_ADDRESS, URB_RQ_SET_CONFIGURATION, URB_RQ_SET_DESCRIPTOR, URB_RQ_SET_FEATURE,
    URB_RQ_SET_INTERFACE, URB_RQ_SYNCH_FRAME,
};

#[cfg(feature = "zerocopy")]
use zerocopy_derive::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Request {
    pub bm_request_type: u8,
    pub b_request: u8,
}

impl Request {
    pub const STANDARD_DEVICE_GET_STATUS: Self = Self {
        bm_request_type: 0x80,
        b_request: 0x00,
    };

    pub const STANDARD_DEVICE_CLEAR_FEATURE: Self = Self {
        bm_request_type: 0x00,
        b_request: 0x01,
    };

    pub const STANDARD_DEVICE_SET_FEATURE: Self = Self {
        bm_request_type: 0x00,
        b_request: 0x03,
    };

    pub const STANDARD_DEVICE_SET_ADDRESS: Self = Self {
        bm_request_type: 0x00,
        b_request: 0x05,
    };

    pub const STANDARD_DEVICE_GET_DESCRIPTOR: Self = Self {
        bm_request_type: 0x80,
        b_request: 0x06,
    };

    pub const STANDARD_DEVICE_SET_DESCRIPTOR: Self = Self {
        bm_request_type: 0x00,
        b_request: 0x07,
    };

    pub const STANDARD_DEVICE_GET_CONFIGURATION: Self = Self {
        bm_request_type: 0x80,
        b_request: 0x08,
    };

    pub const STANDARD_DEVICE_SET_CONFIGURATION: Self = Self {
        bm_request_type: 0x00,
        b_request: 0x09,
    };

    pub const STANDARD_INTERFACE_GET_STATUS: Self = Self {
        bm_request_type: 0x81,
        b_request: 0x00,
    };

    pub const STANDARD_INTERFACE_CLEAR_FEATURE: Self = Self {
        bm_request_type: 0x01,
        b_request: 0x01,
    };

    pub const STANDARD_INTERFACE_SET_FEATURE: Self = Self {
        bm_request_type: 0x01,
        b_request: 0x03,
    };

    pub const STANDARD_INTERFACE_GET_INTERFACE: Self = Self {
        bm_request_type: 0x81,
        b_request: 0x0A,
    };

    pub const STANDARD_INTERFACE_SET_INTERFACE: Self = Self {
        bm_request_type: 0x01,
        b_request: 0x11,
    };

    pub const STANDARD_ENDPOINT_GET_STATUS: Self = Self {
        bm_request_type: 0x82,
        b_request: 0x00,
    };

    pub const STANDARD_ENDPOINT_CLEAR_FEATURE: Self = Self {
        bm_request_type: 0x02,
        b_request: 0x01,
    };

    pub const STANDARD_ENDPOINT_SET_FEATURE: Self = Self {
        bm_request_type: 0x02,
        b_request: 0x03,
    };

    pub const STANDARD_ENDPOINT_SYNCH_FRAME: Self = Self {
        bm_request_type: 0x82,
        b_request: 0x12,
    };

    pub const fn kind(&self) -> (Dir, CtrlType, Recipient) {
        (self.dir(), self.ctrl_type(), self.recipient())
    }

    pub const fn ctrl_type(&self) -> CtrlType {
        CtrlType::from_u8((self.bm_request_type & 0x60) >> 5).unwrap()
    }

    pub const fn dir(&self) -> Dir {
        Dir::from_u8((self.bm_request_type & 0x80) >> 7).unwrap()
    }

    pub const fn recipient(&self) -> Recipient {
        Recipient::from_u8(self.bm_request_type & 0x1F).unwrap()
    }

    pub const fn req(&self) -> Req {
        match self.kind() {
            (_, CtrlType::Standard, _) => Req::standard_from_u8(self.b_request),
            (dir, CtrlType::Class, Recipient::Interface) => Req::class_from_u8(dir, self.b_request),
            _ => Req::Other(self.b_request),
        }
    }
}

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?} | {:?} | {:?}] {:?}",
            self.dir(),
            self.ctrl_type(),
            self.recipient(),
            self.req()
        )
    }
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

#[cfg_attr(
    feature = "zerocopy",
    derive(KnownLayout, Immutable, IntoBytes, TryFromBytes, Unaligned)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DescriptorType {
    Device = 0x1,
    Configuration = 0x2,
    String = 0x3,
    Interface = 0x4,
    Endpoint = 0x5,
}

impl DescriptorType {
    pub const fn from_u8(num: u8) -> Option<Self> {
        match num {
            1 => Some(DescriptorType::Device),
            2 => Some(DescriptorType::Configuration),
            3 => Some(DescriptorType::String),
            4 => Some(DescriptorType::Interface),
            5 => Some(DescriptorType::Endpoint),
            _ => None,
        }
    }
}

/// Transfer direction.
#[cfg_attr(
    feature = "zerocopy",
    derive(KnownLayout, Immutable, IntoBytes, FromZeros, Unaligned)
)]
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
#[cfg_attr(
    feature = "zerocopy",
    derive(KnownLayout, Immutable, IntoBytes, FromZeros, Unaligned)
)]
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
#[cfg_attr(
    feature = "zerocopy",
    derive(KnownLayout, Immutable, IntoBytes, FromZeros, Unaligned)
)]
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

#[derive(Default, Debug, Clone, Copy)]
pub enum Req {
    #[default]
    GetStatus,
    ClearFeature,
    SetFeature,
    SetAddress,
    GetDescriptor,
    SetDescriptor,
    GetConfiguration,
    SetConfiguration,
    GetInterface,
    SetInterface,
    SynchFrame,
    GetRequests,
    PutRequests,
    BulkOnlyMassStorageReset,
    GetMaxLun,
    UacSetCur,
    UacGetCur,
    UacSetMin,
    UacGetMin,
    UacSetMax,
    UacGetMax,
    UacSetRes,
    UacGetRes,
    UacSetIdle,
    GetReport,
    SetReport,
    Other(u8),
}

impl Req {
    pub const fn standard_from_u8(b_request: u8) -> Self {
        match b_request {
            0 => Self::GetStatus,
            URB_RQ_CLEAR_FEATURE => Self::ClearFeature,
            URB_RQ_SET_FEATURE => Self::SetFeature,
            URB_RQ_SET_ADDRESS => Self::SetAddress,
            URB_RQ_GET_DESCRIPTOR => Self::GetDescriptor,
            URB_RQ_SET_DESCRIPTOR => Self::SetDescriptor,
            URB_RQ_GET_CONFIGURATION => Self::GetConfiguration,
            URB_RQ_SET_CONFIGURATION => Self::SetConfiguration,
            URB_RQ_GET_INTERFACE => Self::GetInterface,
            URB_RQ_SET_INTERFACE => Self::SetInterface,
            URB_RQ_SYNCH_FRAME => Self::SynchFrame,
            _ => Self::Other(b_request),
        }
    }

    pub const fn class_from_u8(dir: Dir, b_request: u8) -> Req {
        match (dir, b_request) {
            (Dir::Out, 0x01) => Self::UacSetCur,
            (Dir::In, 0x01) => Self::GetReport,
            (_, 0x02) => Self::UacSetMin,
            (_, 0x03) => Self::UacSetMax,
            (_, 0x04) => Self::UacSetRes,
            (_, 0x09) => Self::SetReport,
            (_, 0x0A) => Self::UacSetIdle,
            (_, 0x81) => Self::UacGetCur,
            (_, 0x82) => Self::UacGetMin,
            (_, 0x83) => Self::UacGetMax,
            (_, 0x84) => Self::UacGetRes,
            (_, 0xFC) => Self::GetRequests,
            (_, 0xFD) => Self::PutRequests,
            (_, 0xFF) => Self::BulkOnlyMassStorageReset,
            (_, 0xFE) => Self::GetMaxLun,
            _ => Self::standard_from_u8(b_request),
        }
    }
}
