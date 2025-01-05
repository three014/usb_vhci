use std::mem::MaybeUninit;

use bitflags::bitflags;
use usbfs::Dir;
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

pub trait Urb {
    fn kind(&self) -> ioctl::UrbType;
    fn handle(&self) -> ioctl::UrbHandle;
    fn transfer(&self) -> &[u8];
    fn transfer_mut(&mut self) -> &mut [u8];
    fn iso_packets_rx(&self) -> &[ioctl::IocIsoPacketGiveback];
    fn iso_packets_rx_mut(&mut self) -> &mut [ioctl::IocIsoPacketGiveback];
    fn iso_packets_tx(&self) -> &[ioctl::IocIsoPacketData];
    fn iso_packets_tx_mut(&mut self) -> &mut [ioctl::IocIsoPacketData];
    fn status(&self) -> Status;
    fn error_count(&self) -> u16;
    fn endpoint(&self) -> ioctl::Endpoint;
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

    fn transfer(&self) -> &[u8] {
        T::transfer(self)
    }

    fn transfer_mut(&mut self) -> &mut [u8] {
        T::transfer_mut(self)
    }

    fn iso_packets_tx(&self) -> &[ioctl::IocIsoPacketData] {
        T::iso_packets_tx(self)
    }

    fn iso_packets_tx_mut(&mut self) -> &mut [ioctl::IocIsoPacketData] {
        T::iso_packets_tx_mut(self)
    }

    fn iso_packets_rx(&self) -> &[ioctl::IocIsoPacketGiveback] {
        T::iso_packets_rx(self)
    }

    fn iso_packets_rx_mut(&mut self) -> &mut [ioctl::IocIsoPacketGiveback] {
        T::iso_packets_rx_mut(self)
    }

    fn status(&self) -> Status {
        T::status(self)
    }

    fn error_count(&self) -> u16 {
        T::error_count(self)
    }

    fn endpoint(&self) -> ioctl::Endpoint {
        T::endpoint(self)
    }
}

pub struct UrbWithData {
    transfer: Vec<u8>,
    urb: ioctl::IocUrb,
    iso_packets_rx: Box<[ioctl::IocIsoPacketGiveback]>,
    iso_packets_tx: Box<[ioctl::IocIsoPacketData]>,
    handle: ioctl::UrbHandle,
    status: Status,
    num_errors: u16,
}

impl UrbWithData {
    pub const fn kind(&self) -> ioctl::UrbType {
        self.urb.typ
    }

    pub const fn handle(&self) -> ioctl::UrbHandle {
        self.handle
    }

    pub fn transfer_mut(&mut self) -> &mut [u8] {
        &mut self.transfer[..]
    }

    pub fn transfer(&self) -> &[u8] {
        &self.transfer[..]
    }

    pub fn iso_packets_tx(&self) -> &[ioctl::IocIsoPacketData] {
        &self.iso_packets_tx[..]
    }

    pub fn iso_packets_tx_mut(&mut self) -> &mut [ioctl::IocIsoPacketData] {
        &mut self.iso_packets_tx[..]
    }

    pub fn iso_packets_rx(&self) -> &[ioctl::IocIsoPacketGiveback] {
        &self.iso_packets_rx[..]
    }

    pub fn iso_packets_rx_mut(&mut self) -> &mut [ioctl::IocIsoPacketGiveback] {
        &mut self.iso_packets_rx[..]
    }

    pub const fn status_to_errno_raw(&self) -> i32 {
        let is_iso = matches!(self.urb.typ, ioctl::UrbType::Iso);
        self.status.to_errno_raw(is_iso)
    }

    pub fn available_transfer_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        self.transfer.spare_capacity_mut()
    }

    pub const fn endpoint(&self) -> ioctl::Endpoint {
        self.urb.endpoint
    }

    pub const fn error_count(&self) -> u16 {
        self.num_errors
    }

    pub fn needs_data_fetch(&self) -> bool {
        self.transfer().len() > 0 || self.iso_packets_tx().len() > 0
    }

    pub const fn control_packet(&self) -> &ioctl::IocSetupPacket {
        &self.urb.setup_packet
    }

    /// Updates the transfer buffer by setting its length
    /// to the current length plus the number of bytes written to
    /// the uninitialized portion.
    ///
    /// The uninitialized portion can be obtained using [`UrbWithData::available_transfer_mut`].
    ///
    /// # Safety
    ///
    /// The caller must make sure that `bytes_written_to_uninit` matches
    /// what was written to the uninitialized portion of the buffer.
    ///
    /// All of the other constraints from [`Vec::set_len`] apply as well.
    pub unsafe fn update_transfer_len(&mut self, bytes_written_to_uninit: usize) {
        self.transfer
            .set_len(self.transfer.len() + bytes_written_to_uninit);
    }

    pub fn from_ioctl(urb: ioctl::IocUrb, handle: ioctl::UrbHandle) -> Self {
        let iso_packets_tx =
            vec![ioctl::IocIsoPacketData::default(); urb.packet_count.try_into().unwrap()].into_boxed_slice();
        let iso_packets_rx =
            vec![ioctl::IocIsoPacketGiveback::default(); urb.packet_count.try_into().unwrap()].into_boxed_slice();
        let transfer = match urb.typ {
            ioctl::UrbType::Iso | _ if Dir::Out == urb.endpoint.direction() => {
                vec![0; urb.buffer_length.try_into().unwrap()]
            }
            _ => Vec::with_capacity(urb.buffer_length.try_into().unwrap()),
        };

        Self {
            iso_packets_tx,
            iso_packets_rx,
            transfer,
            urb,
            handle,
            status: Status::Pending,
            num_errors: 0,
        }
    }

    pub fn set_status(&mut self, new_status: Status) {
        self.status = new_status;
    }
}

impl Urb for UrbWithData {
    fn kind(&self) -> ioctl::UrbType {
        self.kind()
    }

    fn handle(&self) -> ioctl::UrbHandle {
        self.handle()
    }

    fn transfer(&self) -> &[u8] {
        self.transfer()
    }

    fn transfer_mut(&mut self) -> &mut [u8] {
        self.transfer_mut()
    }

    fn iso_packets_tx(&self) -> &[ioctl::IocIsoPacketData] {
        self.iso_packets_tx()
    }

    fn iso_packets_tx_mut(&mut self) -> &mut [ioctl::IocIsoPacketData] {
        self.iso_packets_tx_mut()
    }

    fn iso_packets_rx(&self) -> &[ioctl::IocIsoPacketGiveback] {
        self.iso_packets_rx()
    }

    fn iso_packets_rx_mut(&mut self) -> &mut [ioctl::IocIsoPacketGiveback] {
        self.iso_packets_rx_mut()
    }

    fn status(&self) -> Status {
        self.status
    }

    fn error_count(&self) -> u16 {
        self.error_count()
    }

    fn endpoint(&self) -> ioctl::Endpoint {
        self.endpoint()
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
