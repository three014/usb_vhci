use crate::ioctl::{
    URB_RQ_CLEAR_FEATURE, URB_RQ_GET_CONFIGURATION, URB_RQ_GET_DESCRIPTOR, URB_RQ_GET_INTERFACE,
    URB_RQ_SET_ADDRESS, URB_RQ_SET_CONFIGURATION, URB_RQ_SET_DESCRIPTOR, URB_RQ_SET_FEATURE,
    URB_RQ_SET_INTERFACE, URB_RQ_SYNCH_FRAME,
};

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

#[cfg_attr(
    feature = "zerocopy",
    derive(IntoBytes, FromZeros, Unaligned, Immutable, KnownLayout,)
)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Req {
    /// Needs to use a literal '0' for zerocopy. Equal to [`URB_RQ_GET_STATUS`]
    #[default]
    GetStatus = 0,
    ClearFeature = URB_RQ_CLEAR_FEATURE,
    SetFeature = URB_RQ_SET_FEATURE,
    SetAddress = URB_RQ_SET_ADDRESS,
    GetDescriptor = URB_RQ_GET_DESCRIPTOR,
    SetDescriptor = URB_RQ_SET_DESCRIPTOR,
    GetConfiguration = URB_RQ_GET_CONFIGURATION,
    SetConfiguration = URB_RQ_SET_CONFIGURATION,
    GetInterface = URB_RQ_GET_INTERFACE,
    SetInterface = URB_RQ_SET_INTERFACE,
    SynchFrame = URB_RQ_SYNCH_FRAME,
}

pub type UsbReq = ((Dir, CtrlType, Recipient), Req);

pub const STANDARD_DEVICE_GET_STATUS: UsbReq = (
    (Dir::In, CtrlType::Standard, Recipient::Device),
    Req::GetStatus,
);

pub const STANDARD_DEVICE_CLEAR_FEATURE: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Device),
    Req::ClearFeature,
);

pub const STANDARD_DEVICE_SET_FEATURE: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Device),
    Req::SetFeature,
);

pub const STANDARD_DEVICE_SET_ADDRESS: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Device),
    Req::SetAddress,
);

pub const STANDARD_DEVICE_GET_DESCRIPTOR: UsbReq = (
    (Dir::In, CtrlType::Standard, Recipient::Device),
    Req::GetDescriptor,
);

pub const STANDARD_DEVICE_SET_DESCRIPTOR: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Device),
    Req::SetDescriptor,
);

pub const STANDARD_DEVICE_GET_CONFIGURATION: UsbReq = (
    (Dir::In, CtrlType::Standard, Recipient::Device),
    Req::GetConfiguration,
);

pub const STANDARD_DEVICE_SET_CONFIGURATION: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Device),
    Req::SetConfiguration,
);

pub const STANDARD_INTERFACE_GET_STATUS: UsbReq = (
    (Dir::In, CtrlType::Standard, Recipient::Interface),
    Req::GetStatus,
);

pub const STANDARD_INTERFACE_CLEAR_FEATURE: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Interface),
    Req::ClearFeature,
);

pub const STANDARD_INTERFACE_SET_FEATURE: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Interface),
    Req::SetFeature,
);

pub const STANDARD_INTERFACE_GET_INTERFACE: UsbReq = (
    (Dir::In, CtrlType::Standard, Recipient::Interface),
    Req::GetInterface,
);

pub const STANDARD_INTERFACE_SET_INTERFACE: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Interface),
    Req::SetInterface,
);

pub const STANDARD_ENDPOINT_GET_STATUS: UsbReq = (
    (Dir::In, CtrlType::Standard, Recipient::Endpoint),
    Req::GetStatus,
);

pub const STANDARD_ENDPOINT_CLEAR_FEATURE: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Endpoint),
    Req::ClearFeature,
);

pub const STANDARD_ENDPOINT_SET_FEATURE: UsbReq = (
    (Dir::Out, CtrlType::Standard, Recipient::Endpoint),
    Req::SetFeature,
);

pub const STANDARD_ENDPOINT_SYNCH_FRAME: UsbReq = (
    (Dir::In, CtrlType::Standard, Recipient::Endpoint),
    Req::SynchFrame,
);
