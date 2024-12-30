use bitflags::bitflags;
use ioctl::{Address, Endpoint};
use usbfs::Direction;
use utils::BoundedU8;

#[cfg(feature = "zerocopy")]
use zerocopy_derive::*;

#[cfg(feature = "controller")]
pub use controller::{Controller, Remote, WorkReceiver};
pub use nix::libc;

#[cfg(feature = "controller")]
mod controller;
pub mod ioctl;
pub mod usbfs;
pub mod utils;

pub const MAX_ISO_PACKETS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Port(BoundedU8<1, 32>);

impl nohash_hasher::IsEnabled for Port {}

impl Port {
    pub const fn new(port: u8) -> Option<Self> {
        if let Some(num) = BoundedU8::new(port) {
            Some(Self(num))
        } else {
            None
        }
    }

    pub const fn get(&self) -> u8 {
        self.0.get()
    }
}

#[cfg_attr(
    feature = "zerocopy",
    derive(KnownLayout, Immutable, IntoBytes, TryFromBytes)
)]
#[derive(Debug, num_enum::TryFromPrimitive, Default, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Status {
    #[default]
    Success = 0x00000000,
    Pending = 0x10000001,
    ShortPacket = 0x10000002,
    Error = 0x7ff00000,
    Canceled = 0x30000001,
    TimedOut = 0x30000002,
    DeviceDisabled = 0x71000001,
    DeviceDisconnected = 0x71000002,
    BitStuff = 0x72000001,
    Crc = 0x72000002,
    NoResponse = 0x72000003,
    Babble = 0x72000004,
    Stall = 0x74000001,
    BufferOverrun = 0x72100001,
    BufferUnderrun = 0x72100002,
    AllIsoPacketsFailed = 0x78000001,
}

impl Status {
    pub const fn to_errno_raw(&self, is_iso: bool) -> i32 {
        use nix::libc::*;
        match self {
            Status::Success => 0,
            Status::Pending => -EINPROGRESS,
            Status::ShortPacket => -EREMOTEIO,
            Status::Error if is_iso => -EXDEV,
            Status::Canceled => -ECONNRESET,
            Status::TimedOut => -ETIMEDOUT,
            Status::DeviceDisabled => -ESHUTDOWN,
            Status::DeviceDisconnected => -ENODEV,
            Status::BitStuff => -EPROTO,
            Status::Crc => -EILSEQ,
            Status::NoResponse => -ETIME,
            Status::Babble => -EOVERFLOW,
            Status::Stall => -EPIPE,
            Status::BufferOverrun => -ECOMM,
            Status::BufferUnderrun => -ENOSR,
            Status::AllIsoPacketsFailed if is_iso => -EINVAL,
            Status::AllIsoPacketsFailed | Status::Error => -EPROTO,
        }
    }

    pub const fn from_errno_raw(errno: i32, is_iso: bool) -> Self {
        use nix::libc::*;
        match -errno {
            0 => Status::Success,
            EINPROGRESS => Status::Pending,
            EREMOTEIO => Status::ShortPacket,
            ENOENT | ECONNRESET => Status::Canceled,
            ETIMEDOUT => Status::TimedOut,
            ESHUTDOWN => Status::DeviceDisabled,
            ENODEV => Status::DeviceDisconnected,
            EPROTO => Status::BitStuff,
            EILSEQ => Status::Crc,
            ETIME => Status::NoResponse,
            EOVERFLOW => Status::Babble,
            EPIPE => Status::Stall,
            ECOMM => Status::BufferOverrun,
            ENOSR => Status::BufferUnderrun,
            EINVAL if is_iso => Status::AllIsoPacketsFailed,
            _ => Status::Error,
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct UrbFlags: u16 {
        const SHORT_NOT_OK = 0x0001;
        const ISO_ASAP = 0x0002;
        const ZERO_PACKET = 0x0040;

        const _ = !0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct UrbHandle(pub u64);

impl nohash_hasher::IsEnabled for UrbHandle {}

impl UrbHandle {
    pub(crate) fn as_raw_handle(&self) -> u64 {
        self.0
    }
}

#[derive(Default, Debug, Clone)]
pub struct IsoPacket {
    offset: u32,
    packet_length: i32,
    packet_actual: i32,
    status: Status,
}

#[derive(Debug, Clone)]
pub struct UrbIso {
    status: Status,
    handle: UrbHandle,
    /// buffer length is the actual length for iso urbs
    buffer: Box<[u8]>,
    iso_packets: Box<[IsoPacket]>,
    error_count: i32,
    /// address
    devadr: Address,
    /// endpoint
    epadr: Endpoint,
    asap: bool,
    interval: i32,
}

#[derive(Debug, Clone)]
pub struct UrbInt {
    status: Status,
    handle: UrbHandle,
    buffer: Vec<u8>,
    devadr: Address,
    epadr: Endpoint,
    short_not_ok: bool,
    interval: i32,
}

#[derive(Debug, Clone)]
pub struct UrbControl {
    status: Status,
    handle: UrbHandle,
    buffer: Vec<u8>,
    pub devadr: Address,
    pub epadr: Endpoint,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
    pub bm_request_type: u8,
    pub b_request: ioctl::UrbRequest,
}

#[derive(Debug, Clone)]
pub struct UrbBulk {
    status: Status,
    handle: UrbHandle,
    buffer: Vec<u8>,
    devadr: Address,
    epadr: Endpoint,
    send_zero_packet: bool,
}

#[derive(Debug)]
pub enum Urb {
    Iso(UrbIso),
    Int(UrbInt),
    Ctrl(UrbControl),
    Bulk(UrbBulk),
}

impl Urb {
    pub const fn direction(&self) -> Direction {
        match self {
            Urb::Iso(urb_iso) => urb_iso.epadr.direction(),
            Urb::Int(urb_int) => urb_int.epadr.direction(),
            Urb::Ctrl(urb_control) => urb_control.epadr.direction(),
            Urb::Bulk(urb_bulk) => urb_bulk.epadr.direction(),
        }
    }

    pub const fn handle(&self) -> UrbHandle {
        match self {
            Urb::Iso(urb_iso) => urb_iso.handle,
            Urb::Int(urb_int) => urb_int.handle,
            Urb::Ctrl(urb_control) => urb_control.handle,
            Urb::Bulk(urb_bulk) => urb_bulk.handle,
        }
    }

    /// The buffer's capacity (I know)
    ///
    /// The actual length of the buffer is found
    /// with [`Urb::buffer_actual`]
    pub fn buffer_length(&self) -> usize {
        match self {
            Urb::Iso(urb_iso) => urb_iso.buffer.len(),
            Urb::Int(urb_int) => urb_int.buffer.capacity(),
            Urb::Ctrl(urb_control) => urb_control.buffer.capacity(),
            Urb::Bulk(urb_bulk) => urb_bulk.buffer.capacity(),
        }
    }

    pub fn buffer_actual(&self) -> usize {
        match self {
            Urb::Iso(urb_iso) => urb_iso.buffer.len(),
            Urb::Int(urb_int) => urb_int.buffer.len(),
            Urb::Ctrl(urb_control) => urb_control.buffer.len(),
            Urb::Bulk(urb_bulk) => urb_bulk.buffer.len(),
        }
    }

    pub fn packet_count(&self) -> usize {
        match self {
            Urb::Iso(urb_iso) => urb_iso.iso_packets.len(),
            _ => 0,
        }
    }

    pub fn requires_fetch_data(&self) -> bool {
        match self {
            Urb::Iso(urb_iso) => !urb_iso.iso_packets.is_empty(),
            Urb::Int(urb_int) => !urb_int.buffer.is_empty(),
            Urb::Ctrl(urb_control) => !urb_control.buffer.is_empty(),
            Urb::Bulk(urb_bulk) => !urb_bulk.buffer.is_empty(),
        }
    }

    pub fn buffer(&self) -> &[u8] {
        match self {
            Urb::Iso(urb_iso) => &urb_iso.buffer,
            Urb::Int(urb_int) => &urb_int.buffer,
            Urb::Ctrl(urb_control) => &urb_control.buffer,
            Urb::Bulk(urb_bulk) => &urb_bulk.buffer,
        }
    }

    pub fn buffer_mut(&mut self) -> &mut [u8] {
        match self {
            Urb::Iso(urb_iso) => &mut urb_iso.buffer,
            Urb::Int(urb_int) => &mut urb_int.buffer,
            Urb::Ctrl(urb_control) => &mut urb_control.buffer,
            Urb::Bulk(urb_bulk) => &mut urb_bulk.buffer,
        }
    }

    pub const fn devadr(&self) -> Address {
        match self {
            Urb::Iso(urb_iso) => urb_iso.devadr,
            Urb::Int(urb_int) => urb_int.devadr,
            Urb::Ctrl(urb_control) => urb_control.devadr,
            Urb::Bulk(urb_bulk) => urb_bulk.devadr,
        }
    }

    pub const fn status_to_errno_raw(&self) -> i32 {
        let (status, is_iso) = match self {
            Urb::Iso(urb_iso) => (urb_iso.status, true),
            Urb::Int(urb_int) => (urb_int.status, false),
            Urb::Ctrl(urb_control) => (urb_control.status, false),
            Urb::Bulk(urb_bulk) => (urb_bulk.status, false),
        };
        status.to_errno_raw(is_iso)
    }

    pub const fn epadr(&self) -> Endpoint {
        match self {
            Urb::Iso(urb_iso) => urb_iso.epadr,
            Urb::Int(urb_int) => urb_int.epadr,
            Urb::Ctrl(urb_control) => urb_control.epadr,
            Urb::Bulk(urb_bulk) => urb_bulk.epadr,
        }
    }

    pub const fn error_count(&self) -> i32 {
        match self {
            Urb::Iso(urb_iso) => urb_iso.error_count,
            _ => 0,
        }
    }
}

pub struct UrbExtended {
    urb: ioctl::IocUrb,
    status: Status,
    num_errors: i32,
    iso_packets: Vec<IsoPacket>,
    transfer: Vec<u8>,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PortStatus: u16 {
        const CONNECTION = 0x0001;
        const ENABLE = 0x0002;
        const SUSPEND = 0x0004;
        const OVERCURRENT = 0x0008;
        const RESET = 0x0010;
        const POWER = 0x0100;
        const LOW_SPEED = 0x0200;
        const HIGH_SPEED = 0x0400;

        const _ = !0;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct PortChange: u16 {
        const CONNECTION = 0x0001;
        const ENABLE = 0x0002;
        const SUSPEND = 0x0004;
        const OVERCURRENT = 0x0008;
        const RESET = 0x0010;

        const _ = !0;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct PortFlag: u8 {
        const RESUMING = 0x01;

        const _ = !0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PortStat {
    pub status: PortStatus,
    pub change: PortChange,
    pub index: Port,
    pub flags: PortFlag,
}

#[derive(Debug, Clone, Copy)]
pub enum DataRate {
    Full = 0,
    Low = 1,
    High = 2,
}

#[derive(Debug)]
pub enum Work {
    /// URB was cancelled and
    /// given back to its creator.
    CancelUrb(UrbHandle),

    /// Creator of URB wants data.
    /// When finished, giveback modified/new URB.
    ProcessUrb(Urb),

    /// Information about a port.
    PortStat(PortStat),
}

impl From<ioctl::IocPortStat> for PortStat {
    fn from(port_stat: ioctl::IocPortStat) -> Self {
        Self {
            status: PortStatus::from_bits(port_stat.status).unwrap(),
            change: PortChange::from_bits(port_stat.change).unwrap(),
            index: Port::new(port_stat.index).unwrap(),
            flags: PortFlag::from_bits_retain(port_stat.flags),
        }
    }
}

impl From<ioctl::IocWork> for Work {
    fn from(ioc_work: ioctl::IocWork) -> Self {
        match ioc_work.typ {
            ioctl::WorkType::PortStat => {
                // SAFETY: The ioctl always tells us what type was written
                //         through the "typ" parameter, so we can safely
                //         use this variant.
                Work::PortStat(unsafe { ioc_work.work.port.into() })
            }
            ioctl::WorkType::ProcessUrb => {
                // SAFETY: The ioctl always tells us what type was written
                //         through the "typ" parameter, so we can safely
                //         use this variant.
                let ioc_urb = unsafe { ioc_work.work.urb };
                let urb = match ioc_urb.typ {
                    ioctl::UrbType::Iso => Urb::Iso(UrbIso {
                        status: Status::Pending,
                        handle: UrbHandle(ioc_work.handle),
                        buffer: vec![0; ioc_urb.buffer_length.try_into().unwrap()]
                            .into_boxed_slice(),
                        error_count: 0,
                        devadr: ioc_urb.address,
                        epadr: ioc_urb.endpoint.into(),
                        iso_packets: vec![
                            IsoPacket::default();
                            ioc_urb.packet_count.try_into().unwrap()
                        ]
                        .into_boxed_slice(),
                        asap: UrbFlags::from_bits_retain(ioc_urb.flags)
                            .contains(UrbFlags::ISO_ASAP),
                        interval: ioc_urb.interval,
                    }),
                    ioctl::UrbType::Int => Urb::Int(UrbInt {
                        status: Status::Pending,
                        handle: UrbHandle(ioc_work.handle),
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(ioc_urb.buffer_length.try_into().unwrap());
                            let actual_len =
                                if matches!(ioc_urb.endpoint.direction(), Direction::Out) {
                                    ioc_urb.buffer_length
                                } else {
                                    0
                                };
                            buf.resize(actual_len.try_into().unwrap(), 0);
                            buf
                        },
                        devadr: ioc_urb.address,
                        epadr: ioc_urb.endpoint.into(),
                        short_not_ok: UrbFlags::from_bits_retain(ioc_urb.flags)
                            .contains(UrbFlags::SHORT_NOT_OK),
                        interval: ioc_urb.interval,
                    }),
                    ioctl::UrbType::Ctrl => Urb::Ctrl(UrbControl {
                        status: Status::Pending,
                        handle: UrbHandle(ioc_work.handle),
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(ioc_urb.buffer_length.try_into().unwrap());
                            let actual_len =
                                if matches!(ioc_urb.endpoint.direction(), Direction::Out) {
                                    ioc_urb.buffer_length
                                } else {
                                    0
                                };
                            buf.resize(actual_len.try_into().unwrap(), 0);
                            buf
                        },
                        devadr: ioc_urb.address,
                        epadr: ioc_urb.endpoint.into(),
                        w_value: ioc_urb.setup_packet.w_value,
                        w_index: ioc_urb.setup_packet.w_index,
                        w_length: ioc_urb.setup_packet.w_length,
                        bm_request_type: ioc_urb.setup_packet.bm_request_type,
                        b_request: ioc_urb.setup_packet.b_request,
                    }),
                    ioctl::UrbType::Bulk => Urb::Bulk(UrbBulk {
                        status: Status::Pending,
                        handle: UrbHandle(ioc_work.handle),
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(ioc_urb.buffer_length.try_into().unwrap());
                            let actual_len =
                                if matches!(ioc_urb.endpoint.direction(), Direction::Out) {
                                    ioc_urb.buffer_length
                                } else {
                                    0
                                };
                            buf.resize(actual_len.try_into().unwrap(), 0);
                            buf
                        },
                        devadr: ioc_urb.address,
                        epadr: ioc_urb.endpoint.into(),
                        send_zero_packet: UrbFlags::from_bits_retain(ioc_urb.flags)
                            .contains(UrbFlags::ZERO_PACKET),
                    }),
                };

                Work::ProcessUrb(urb)
            }
            ioctl::WorkType::CancelUrb => Work::CancelUrb(UrbHandle(ioc_work.handle)),
        }
    }
}
