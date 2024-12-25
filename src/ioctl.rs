use std::{
    ffi::{c_void, OsStr},
    os::unix::ffi::OsStrExt,
};

use nix::{ioctl_readwrite, ioctl_write_ptr};
use zerocopy_derive::*;

use crate::{
    usbfs::{ControlType, Direction, Recipient},
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoBytes, FromZeros, Unaligned, Immutable, KnownLayout,
)]
#[repr(u8)]
pub enum UrbRequest {
    /// Needs to use a literal '0' for zerocopy. Equal to [`URB_RQ_GET_STATUS`]
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

#[derive(Clone, Copy, Default)]
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

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct IocSetupPacket {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

impl IocSetupPacket {
    pub const fn request(&self) -> u8 {
        self.b_request
    }

    pub fn control_type(&self) -> ControlType {
        ControlType::try_from(self.bm_request_type & 0x70).unwrap()
    }

    pub fn direction(&self) -> Direction {
        Direction::try_from(self.bm_request_type & 0x80).unwrap()
    }

    pub fn recipient(&self) -> Recipient {
        Recipient::try_from(self.bm_request_type & 0xF).unwrap()
    }

    pub const fn value(&self) -> u16 {
        self.w_value
    }

    pub const fn index(&self) -> u16 {
        self.w_index
    }

    pub const fn length(&self) -> u16 {
        self.w_length
    }
}

#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    FromZeros,
    IntoBytes,
    Immutable,
    Unaligned,
    KnownLayout,
)]
#[repr(u8)]
pub enum UrbType {
    /// Needs to use a literal '0' for zerocopy. Equal to [`USB_VHCI_URB_TYPE_ISO`]
    #[default]
    Iso = 0,
    Int = USB_VHCI_URB_TYPE_INT,
    Ctrl = USB_VHCI_URB_TYPE_CONTROL,
    Bulk = USB_VHCI_URB_TYPE_BULK,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct IocUrb {
    pub setup_packet: IocSetupPacket,
    pub buffer_length: i32,
    pub interval: i32,
    pub packet_count: i32,
    pub flags: u16,
    pub address: u8,
    pub endpoint: u8,
    pub typ: UrbType,
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

#[derive(Clone)]
pub struct PortStat<'a>(&'a IocWork);

impl PortStat<'_> {
    /// # Safety
    ///
    /// The caller must not modify the `IocWork::typ` field
    /// if this work item came from the vhci controller, or else
    /// undefined behavior follows.
    pub unsafe fn get(&self) -> &IocPortStat {
        assert!(matches!(self.0.typ, WorkType::PortStat));

        // SAFETY: We ensure that the discriminator matches
        //         the cancel urb value, and the caller has
        //         not modified the `IocWork::typ` field.
        unsafe { &self.0.work.port }
    }
}

#[derive(Clone)]
pub struct ProcessUrb<'a>(&'a IocWork);

impl ProcessUrb<'_> {
    /// # Safety
    ///
    /// The caller must not modify the `IocWork::typ` field
    /// if this work item came from the vhci controller, or else
    /// undefined behavior follows.
    pub unsafe fn get(&self) -> &IocUrb {
        assert!(matches!(self.0.typ, WorkType::ProcessUrb));

        // SAFETY: We ensure that the discriminator matches
        //         the cancel urb value, and the caller has
        //         not modified the `IocWork::typ` field.
        unsafe { &self.0.work.urb }
    }
}

#[derive(Clone)]
pub struct CancelUrb<'a>(&'a IocWork);

impl CancelUrb<'_> {
    /// # Safety
    ///
    /// The caller must not modify the `IocWork::typ` field
    /// if this work item came from the vhci controller, or else
    /// undefined behavior follows.
    pub unsafe fn get(&self) -> &IocUrb {
        assert!(matches!(self.0.typ, WorkType::CancelUrb));

        // SAFETY: We ensure that the discriminator matches
        //         the cancel urb value, and the caller has
        //         not modified the `IocWork::typ` field.
        unsafe { &self.0.work.urb }
    }
}

#[derive(Clone)]
pub enum Work<'a> {
    PortStat(PortStat<'a>),
    ProcessUrb(ProcessUrb<'a>),
    CancelUrb(CancelUrb<'a>),
}

#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    KnownLayout,
    Unaligned,
    Immutable,
    FromZeros,
    IntoBytes,
)]
#[repr(u8)]
pub enum WorkType {
    #[default]
    PortStat = 0,
    ProcessUrb = USB_VHCI_WORK_TYPE_PROCESS_URB,
    CancelUrb = USB_VHCI_WORK_TYPE_CANCEL_URB,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct IocWork {
    pub handle: u64,
    pub work: IocWorkUnion,
    pub timeout: i16,
    pub typ: WorkType,
}

impl IocWork {
    pub const fn get(&self) -> Work {
        match self.typ {
            WorkType::PortStat => Work::PortStat(PortStat(self)),
            WorkType::ProcessUrb => Work::ProcessUrb(ProcessUrb(self)),
            WorkType::CancelUrb => Work::CancelUrb(CancelUrb(self)),
        }
    }
}

ioctl_readwrite!(
    usb_vhci_fetchwork,
    USB_VHCI_HCD_IOC_MAGIC,
    USB_VHCI_HCD_IOCFETCHWORK,
    IocWork
);

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
