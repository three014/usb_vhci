use std::{
    ffi::{c_void, OsStr},
    os::unix::ffi::OsStrExt,
};

use nix::{ioctl_readwrite, ioctl_write_ptr};

#[cfg(feature = "zerocopy")]
use zerocopy_derive::*;

use crate::{
    usbfs::{CtrlType, Dir, Recipient, Req},
    utils::BoundedU8,
    Port, PortChange, PortFlag, PortStatus,
};

pub const USB_VHCI_HCD_IOC_MAGIC: u8 = 138;
pub const USB_VHCI_HCD_IOCREGISTER: u8 = 0;
pub const USB_VHCI_HCD_IOCPORTSTAT: u8 = 1;
pub const USB_VHCI_HCD_IOCFETCHWORK_RO: u8 = 2;
pub const USB_VHCI_HCD_IOCFETCHWORK: u8 = 2;
pub const USB_VHCI_HCD_IOCGIVEBACK: u8 = 3;
pub const USB_VHCI_HCD_IOCGIVEBACK32: u8 = 3;
pub const USB_VHCI_HCD_IOCFETCHDATA: u8 = 4;
pub const USB_VHCI_HCD_IOCFETCHDATA32: u8 = 4;

pub const USB_VHCI_PORT_STAT_FLAG_RESUMING: u8 = 0x01;

pub const USB_VHCI_URB_FLAGS_SHORT_NOT_OK: u16 = 0x0001;
pub const USB_VHCI_URB_FLAGS_ISO_ASAP: u16 = 0x0002;
pub const USB_VHCI_URB_FLAGS_ZERO_PACKET: u16 = 0x0040;

pub const USB_VHCI_URB_TYPE_ISO: u8 = 0;
pub const USB_VHCI_URB_TYPE_INT: u8 = 1;
pub const USB_VHCI_URB_TYPE_CONTROL: u8 = 2;
pub const USB_VHCI_URB_TYPE_BULK: u8 = 3;

pub const USB_VHCI_TIMEOUT_INFINITE: i16 = -1;

pub const USB_VHCI_WORK_TYPE_PORT_STAT: u8 = 0;
pub const USB_VHCI_WORK_TYPE_PROCESS_URB: u8 = 1;
pub const USB_VHCI_WORK_TYPE_CANCEL_URB: u8 = 2;

pub const URB_RQ_GET_STATUS: u8 = 0;
pub const URB_RQ_CLEAR_FEATURE: u8 = 1;
pub const URB_RQ_SET_FEATURE: u8 = 3;
pub const URB_RQ_SET_ADDRESS: u8 = 5;
pub const URB_RQ_GET_DESCRIPTOR: u8 = 6;
pub const URB_RQ_SET_DESCRIPTOR: u8 = 7;
pub const URB_RQ_GET_CONFIGURATION: u8 = 8;
pub const URB_RQ_SET_CONFIGURATION: u8 = 9;
pub const URB_RQ_GET_INTERFACE: u8 = 10;
pub const URB_RQ_SET_INTERFACE: u8 = 11;
pub const URB_RQ_SYNCH_FRAME: u8 = 12;

#[cfg_attr(feature = "zerocopy", derive(Immutable, KnownLayout, TryFromBytes))]
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct IocRegister {
    pub id: i32,
    pub usb_busnum: i32,
    pub bus_id: [u8; 20],
    pub port_count: u8,
}

impl IocRegister {
    pub const fn new(num_ports: u8) -> Self {
        Self {
            id: 0,
            usb_busnum: 0,
            bus_id: [0; 20],
            port_count: num_ports,
        }
    }

    pub fn bus_id(&self) -> &OsStr {
        OsStr::from_bytes(&self.bus_id)
    }
}

ioctl_readwrite!(
    usb_vhci_register,
    USB_VHCI_HCD_IOC_MAGIC,
    USB_VHCI_HCD_IOCREGISTER,
    IocRegister
);

#[cfg_attr(
    feature = "zerocopy",
    derive(IntoBytes, TryFromBytes, Immutable, KnownLayout)
)]
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct IocPortStat {
    pub status: u16,
    pub change: u16,
    pub index: u8,
    pub flags: u8,
    pub _reserved1: u8,
    pub _reserved2: u8,
}

impl IocPortStat {
    pub const fn status(&self) -> PortStatus {
        PortStatus::from_bits(self.status).unwrap()
    }

    pub const fn change(&self) -> PortChange {
        PortChange::from_bits(self.change).unwrap()
    }

    pub const fn index(&self) -> Port {
        Port::new(self.index).unwrap()
    }

    pub const fn flags(&self) -> PortFlag {
        PortFlag::from_bits_retain(self.flags)
    }
}

ioctl_write_ptr!(
    usb_vhci_portstat,
    USB_VHCI_HCD_IOC_MAGIC,
    USB_VHCI_HCD_IOCPORTSTAT,
    IocPortStat
);

#[cfg_attr(
    feature = "zerocopy",
    derive(IntoBytes, TryFromBytes, Immutable, KnownLayout)
)]
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct IocSetupPacket {
    pub bm_request_type: u8,
    pub b_request: Req,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

impl IocSetupPacket {
    pub const fn request_type(&self) -> (Dir, CtrlType, Recipient) {
        (self.direction(), self.control_type(), self.recipient())
    }
    #[inline(always)]
    pub const fn request(&self) -> Req {
        self.b_request
    }

    #[inline(always)]
    pub const fn control_type(&self) -> CtrlType {
        CtrlType::from_u8((self.bm_request_type & 0x60) >> 5).unwrap()
    }

    #[inline(always)]
    pub const fn direction(&self) -> Dir {
        Dir::from_u8((self.bm_request_type & 0x80) >> 7).unwrap()
    }

    #[inline(always)]
    pub const fn recipient(&self) -> Recipient {
        Recipient::from_u8(self.bm_request_type & 0x1F).unwrap()
    }

    #[inline(always)]
    pub const fn value(&self) -> u16 {
        self.w_value
    }

    #[inline(always)]
    pub const fn index(&self) -> u16 {
        self.w_index
    }

    pub const fn length(&self) -> u16 {
        self.w_length
    }
}

#[cfg_attr(
    feature = "zerocopy",
    derive(IntoBytes, FromZeros, Immutable, KnownLayout, Unaligned)
)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UrbType {
    /// Needs to use a literal '0' for zerocopy. Equal to [`USB_VHCI_URB_TYPE_ISO`]
    #[default]
    Iso = 0,
    Int = USB_VHCI_URB_TYPE_INT,
    Ctrl = USB_VHCI_URB_TYPE_CONTROL,
    Bulk = USB_VHCI_URB_TYPE_BULK,
}

#[cfg_attr(
    feature = "zerocopy",
    derive(FromBytes, IntoBytes, Immutable, KnownLayout)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Address(u8);

impl Address {
    /// Returns whether the address is meant for
    /// any USB device that does not already have
    /// an assigned address.
    pub const fn is_anycast(&self) -> bool {
        (self.0 & 0x7F) == 0
    }

    pub const fn new(addr: u8) -> Option<Self> {
        if let Some(val) = BoundedU8::<0, 128>::new(addr) {
            Some(Self(val.get()))
        } else {
            None
        }
    }

    pub const fn get(&self) -> u8 {
        self.0
    }
}

impl Default for Address {
    fn default() -> Self {
        Self(0)
    }
}

#[cfg_attr(
    feature = "zerocopy",
    derive(IntoBytes, FromBytes, Immutable, KnownLayout, Unaligned)
)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Endpoint(pub u8);

impl Endpoint {
    pub const fn direction(&self) -> Dir {
        Dir::from_u8((self.0 & 0x80) >> 7).unwrap()
    }

    /// Returns whether the endpoint should be
    /// sent to all devices.
    pub const fn is_broadcast(&self) -> bool {
        self.0 & 0x7f == 0
    }
}

#[cfg_attr(
    feature = "zerocopy",
    derive(IntoBytes, TryFromBytes, Immutable, KnownLayout)
)]
#[derive(Clone, Default, Copy)]
#[repr(C)]
pub struct IocUrb {
    pub setup_packet: IocSetupPacket,
    pub buffer_length: i32,
    pub interval: i32,
    pub packet_count: i32,
    pub flags: u16,
    pub address: Address,
    pub endpoint: Endpoint,
    pub typ: UrbType,
    pub _reserved: [u8; 3],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union IocWorkUnion {
    pub urb: IocUrb,
    pub port: IocPortStat,
}

impl Default for IocWorkUnion {
    fn default() -> Self {
        Self {
            port: Default::default(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UrbHandle(pub u64);

impl UrbHandle {
    pub const fn as_raw(&self) -> u64 {
        self.0
    }
}

impl nohash_hasher::IsEnabled for UrbHandle {}

pub enum WorkRef<'a> {
    PortStat(IocPortStat),
    ProcessUrb((&'a IocUrb, UrbHandle)),
    CancelUrb(UrbHandle),
}

#[derive(Clone)]
pub enum Work {
    PortStat(IocPortStat),
    ProcessUrb((IocUrb, UrbHandle)),
    CancelUrb(UrbHandle),
}

#[cfg_attr(
    feature = "zerocopy",
    derive(IntoBytes, FromZeros, Immutable, KnownLayout, Unaligned)
)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WorkType {
    #[default]
    PortStat = 0,
    ProcessUrb = USB_VHCI_WORK_TYPE_PROCESS_URB,
    CancelUrb = USB_VHCI_WORK_TYPE_CANCEL_URB,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct IocWork {
    pub handle: u64,
    pub work: IocWorkUnion,
    pub timeout: i16,
    pub typ: WorkType,
    pub _padding: [u8; 1],
}

impl IocWork {
    /// # Safety
    ///
    /// The caller must make sure that `IocWork::work` is the
    /// same type as what's specified in `IocWork::typ`.
    ///
    /// If this work item was returned from an ioctl call, then
    /// the above will always be true.
    pub const unsafe fn into_inner(self) -> Work {
        // SAFETY: Caller upholds safety contract in function description.
        match self.typ {
            WorkType::PortStat => Work::PortStat(unsafe { self.work.port }),
            WorkType::ProcessUrb => {
                Work::ProcessUrb((unsafe { self.work.urb }, UrbHandle(self.handle)))
            }
            WorkType::CancelUrb => Work::CancelUrb(UrbHandle(self.handle)),
        }
    }
    /// # Safety
    ///
    /// The caller must make sure that `IocWork::work` is the
    /// same type as what's specified in `IocWork::typ`.
    ///
    /// If this work item was returned from an ioctl call, then
    /// the above will always be true.
    pub const fn get(&self) -> WorkRef {
        // SAFETY: Caller upholds safety contract in function description.
        match self.typ {
            WorkType::PortStat => WorkRef::PortStat(unsafe { self.work.port }),
            WorkType::ProcessUrb => {
                WorkRef::ProcessUrb((unsafe { &self.work.urb }, UrbHandle(self.handle)))
            }
            WorkType::CancelUrb => WorkRef::CancelUrb(UrbHandle(self.handle)),
        }
    }
}

ioctl_readwrite!(
    usb_vhci_fetchwork,
    USB_VHCI_HCD_IOC_MAGIC,
    USB_VHCI_HCD_IOCFETCHWORK,
    IocWork
);

#[cfg_attr(
    feature = "zerocopy",
    derive(FromBytes, IntoBytes, KnownLayout, Immutable)
)]
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct IocIsoPacketData {
    pub offset: u32,
    pub packet_length: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct IocUrbData {
    pub handle: u64,
    pub buffer: *mut c_void,
    pub iso_packets: *mut IocIsoPacketData,
    pub buffer_length: i32,
    pub packet_count: i32,
}

impl Default for IocUrbData {
    fn default() -> Self {
        Self {
            handle: 0,
            buffer: std::ptr::null_mut(),
            iso_packets: std::ptr::null_mut(),
            buffer_length: 0,
            packet_count: 0,
        }
    }
}

ioctl_write_ptr!(
    usb_vhci_fetchdata,
    USB_VHCI_HCD_IOC_MAGIC,
    USB_VHCI_HCD_IOCFETCHDATA,
    IocUrbData
);

#[cfg_attr(
    feature = "zerocopy",
    derive(FromBytes, IntoBytes, KnownLayout, Immutable)
)]
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct IocIsoPacketGiveback {
    pub packet_actual: u32,
    pub status: i32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct IocGiveback {
    pub handle: u64,
    pub buffer: *mut c_void,
    pub iso_packets: *mut IocIsoPacketGiveback,
    pub status: i32,
    pub buffer_actual: i32,
    pub packet_count: i32,
    pub error_count: i32,
}

impl Default for IocGiveback {
    fn default() -> Self {
        Self {
            handle: 0,
            buffer: std::ptr::null_mut(),
            iso_packets: std::ptr::null_mut(),
            status: 0,
            buffer_actual: 0,
            packet_count: 0,
            error_count: 0,
        }
    }
}

ioctl_write_ptr!(
    usb_vhci_giveback,
    USB_VHCI_HCD_IOC_MAGIC,
    USB_VHCI_HCD_IOCGIVEBACK,
    IocGiveback
);
