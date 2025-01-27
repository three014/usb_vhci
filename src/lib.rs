use bitflags::bitflags;
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
    derive(KnownLayout, Immutable, IntoBytes, FromZeros)
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct UrbFlags: u16 {
        const SHORT_NOT_OK = 0x0001;
        const ISO_ASAP = 0x0002;
        const ZERO_PACKET = 0x0040;

        const _ = !0;
    }
}

pub trait Urb {
    fn kind(&self) -> ioctl::UrbType;
    fn handle(&self) -> ioctl::UrbHandle;
    fn status(&self) -> Status;
    fn dir(&self) -> usbfs::Dir;
    fn bytes_transferred(&self) -> u16;
}

pub trait Transfer {
    fn transfer(&self) -> &[u8];
}

pub trait TransferMut {
    fn transfer_mut(&mut self) -> &mut [u8];
}

pub trait IsoPacketData {
    fn iso_packet_data(&self) -> &[ioctl::IocIsoPacketData];
}

pub trait IsoPacketDataMut {
    fn iso_packet_data_mut(&mut self) -> &mut [ioctl::IocIsoPacketData];
}

pub trait IsoPacketGiveback {
    fn iso_packet_giveback(&self) -> &[ioctl::IocIsoPacketGiveback];
    fn error_count(&self) -> u16;
}

pub trait IsoPacketGivebackMut {
    fn iso_packet_giveback_mut(&mut self) -> &mut [ioctl::IocIsoPacketGiveback];
    fn error_count(&self) -> u16;
}

impl<T> Urb for &mut T
where
    T: Urb + ?Sized,
{
    fn kind(&self) -> ioctl::UrbType {
        T::kind(self)
    }

    fn handle(&self) -> ioctl::UrbHandle {
        T::handle(self)
    }

    fn status(&self) -> Status {
        T::status(self)
    }

    fn dir(&self) -> usbfs::Dir {
        T::dir(self)
    }

    fn bytes_transferred(&self) -> u16 {
        T::bytes_transferred(self)
    }
}

impl<T> Transfer for &T
where
    T: Transfer + ?Sized,
{
    fn transfer(&self) -> &[u8] {
        T::transfer(self)
    }
}

impl<T> TransferMut for &mut T
where
    T: TransferMut + ?Sized,
{
    fn transfer_mut(&mut self) -> &mut [u8] {
        T::transfer_mut(self)
    }
}

impl<T> IsoPacketData for &T
where
    T: IsoPacketData + ?Sized,
{
    fn iso_packet_data(&self) -> &[ioctl::IocIsoPacketData] {
        T::iso_packet_data(self)
    }
}

impl<T> IsoPacketDataMut for &mut T
where
    T: IsoPacketDataMut + ?Sized,
{
    fn iso_packet_data_mut(&mut self) -> &mut [ioctl::IocIsoPacketData] {
        T::iso_packet_data_mut(self)
    }
}

impl<T> IsoPacketGiveback for &T
where
    T: IsoPacketGiveback + ?Sized,
{
    fn iso_packet_giveback(&self) -> &[ioctl::IocIsoPacketGiveback] {
        T::iso_packet_giveback(self)
    }

    fn error_count(&self) -> u16 {
        T::error_count(self)
    }
}

impl<T> IsoPacketGivebackMut for &mut T
where
    T: IsoPacketGivebackMut + ?Sized,
{
    fn iso_packet_giveback_mut(&mut self) -> &mut [ioctl::IocIsoPacketGiveback] {
        T::iso_packet_giveback_mut(self)
    }

    fn error_count(&self) -> u16 {
        T::error_count(self)
    }
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
pub enum DataRate {
    Full = 0,
    Low = 1,
    High = 2,
}
