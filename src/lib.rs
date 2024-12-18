use bit_vec::BitVec;
use bitflags::bitflags;
use std::{
    ops::{Add, Sub},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
};

pub mod utils;
pub use nix::libc;

pub const MAX_ISO_PACKETS: usize = 64;
static USB_VHCI_DEVICE_FILE: &str = "/dev/usb-vhci";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Endpoint(u8);

impl Endpoint {
    pub const fn direction(&self) -> Dir {
        match !(self.get() & 0x80) != 0 {
            true => Dir::Out,
            false => Dir::In,
        }
    }

    pub const fn is_out(&self) -> bool {
        matches!(self.direction(), Dir::Out)
    }

    pub const fn is_in(&self) -> bool {
        matches!(self.direction(), Dir::In)
    }

    pub const fn get(&self) -> u8 {
        self.0
    }

    pub const fn new(epadr: u8) -> Endpoint {
        Endpoint(epadr)
    }
}

impl From<u8> for Endpoint {
    fn from(epadr: u8) -> Self {
        Endpoint::new(epadr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    In = 0,
    Out = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Port(utils::BoundedU8<1, 32>);

impl nohash_hasher::IsEnabled for Port {}

impl Port {
    pub const fn new(port: u8) -> Option<Self> {
        if let Some(num) = utils::BoundedU8::new(port) {
            Some(Self(num))
        } else {
            None
        }
    }

    pub const fn get(&self) -> u8 {
        self.0.get()
    }
}

#[derive(Debug, num_enum::TryFromPrimitive, Default, Clone, Copy)]
#[repr(i32)]
pub enum IsoStatus {
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

impl IsoStatus {
    pub const fn to_errno_raw(&self, is_iso: bool) -> i32 {
        use nix::libc::*;
        match self {
            IsoStatus::Success => 0,
            IsoStatus::Pending => -EINPROGRESS,
            IsoStatus::ShortPacket => -EREMOTEIO,
            IsoStatus::Error => {
                if is_iso {
                    -EXDEV
                } else {
                    -EPROTO
                }
            }
            IsoStatus::Canceled => -ECONNRESET,
            IsoStatus::TimedOut => -ETIMEDOUT,
            IsoStatus::DeviceDisabled => -ESHUTDOWN,
            IsoStatus::DeviceDisconnected => -ENODEV,
            IsoStatus::BitStuff => -EPROTO,
            IsoStatus::Crc => -EILSEQ,
            IsoStatus::NoResponse => -ETIME,
            IsoStatus::Babble => -EOVERFLOW,
            IsoStatus::Stall => -EPIPE,
            IsoStatus::BufferOverrun => -ECOMM,
            IsoStatus::BufferUnderrun => -ENOSR,
            IsoStatus::AllIsoPacketsFailed => {
                if is_iso {
                    -EINVAL
                } else {
                    -EPROTO
                }
            }
        }
    }

    pub const fn from_errno_raw(errno: i32, is_iso: bool) -> Self {
        use nix::libc::*;
        match -errno {
            0 => IsoStatus::Success,
            EINPROGRESS => IsoStatus::Pending,
            EREMOTEIO => IsoStatus::ShortPacket,
            ENOENT | ECONNRESET => IsoStatus::Canceled,
            ETIMEDOUT => IsoStatus::TimedOut,
            ESHUTDOWN => IsoStatus::DeviceDisabled,
            ENODEV => IsoStatus::DeviceDisconnected,
            EPROTO => IsoStatus::BitStuff,
            EILSEQ => IsoStatus::Crc,
            ETIME => IsoStatus::NoResponse,
            EOVERFLOW => IsoStatus::Babble,
            EPIPE => IsoStatus::Stall,
            ECOMM => IsoStatus::BufferOverrun,
            ENOSR => IsoStatus::BufferUnderrun,
            EINVAL => {
                if is_iso {
                    IsoStatus::AllIsoPacketsFailed
                } else {
                    IsoStatus::Error
                }
            }
            _ => IsoStatus::Error,
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
pub struct UrbHandle(u64);

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
    status: IsoStatus,
}

#[derive(Debug, Clone)]
pub struct UrbIso {
    status: IsoStatus,
    handle: UrbHandle,
    /// buffer length is the actual length for iso urbs
    buffer: Box<[u8]>,
    iso_packets: Box<[IsoPacket]>,
    error_count: i32,
    /// address
    devadr: u8,
    /// endpoint
    epadr: Endpoint,
    asap: bool,
    interval: i32,
}

#[derive(Debug, Clone)]
pub struct UrbInt {
    status: IsoStatus,
    handle: UrbHandle,
    buffer: Vec<u8>,
    devadr: u8,
    epadr: Endpoint,
    short_not_ok: bool,
    interval: i32,
}

#[derive(Debug, Clone)]
pub struct UrbControl {
    status: IsoStatus,
    handle: UrbHandle,
    buffer: Vec<u8>,
    pub devadr: u8,
    pub epadr: Endpoint,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
    pub bm_request_type: u8,
    pub b_request: u8,
}

#[derive(Debug, Clone)]
pub struct UrbBulk {
    status: IsoStatus,
    handle: UrbHandle,
    buffer: Vec<u8>,
    devadr: u8,
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
    pub const fn direction(&self) -> Dir {
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

    pub const fn devadr(&self) -> u8 {
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

#[derive(Debug)]
pub struct WorkReceiver {
    dev: std::os::unix::io::RawFd,
}

impl WorkReceiver {
    pub fn fetch_work(&self) -> std::io::Result<Work> {
        self.fetch_work_timeout(utils::TimeoutMillis::Time(
            utils::BoundedI16::new(100).unwrap(),
        ))
    }

    pub fn fetch_work_timeout(&self, timeout: utils::TimeoutMillis) -> std::io::Result<Work> {
        let mut ioc_work = ioctl::IocWork {
            timeout: match timeout {
                // utils::TimeoutMillis::Unlimited => ioctl::USB_VHCI_TIMEOUT_INFINITE,
                utils::TimeoutMillis::Time(time) => time.get(),
            },
            ..Default::default()
        };

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe {
            ioctl::usb_vhci_fetchwork(self.dev, &raw mut ioc_work).map_err(std::io::Error::from)?
        };

        ioc_work.try_into().map_err(std::io::Error::from)
    }
}

#[derive(Debug, Clone)]
pub struct Remote {
    dev: std::os::unix::io::RawFd,
}

impl Remote {
    pub fn fetch_data(&self, urb: &mut Urb) -> std::io::Result<()> {
        let mut ioc_urb_data = ioctl::IocUrbData {
            handle: urb.handle().as_raw_handle(),
            buffer_length: urb.buffer_length() as i32,
            packet_count: urb.packet_count() as i32,
            buffer: urb.buffer_mut().as_mut_ptr().cast(),
            ..Default::default()
        };
        let mut ioc_iso_packets = heapless::Vec::<ioctl::IocIsoPacketData, MAX_ISO_PACKETS>::new();
        assert!(urb.packet_count() <= MAX_ISO_PACKETS);
        if urb.packet_count() > 0 {
            ioc_urb_data.iso_packets = ioc_iso_packets.as_mut_ptr();
        }

        // SAFETY: TODO: We allocate our own buffer for the iso packets,
        //         and that shouuuuuld last throughout this call?
        //         After the ioctl call, `iso_packets` should have the
        //         same len as the buffer in the urb??
        unsafe {
            ioctl::usb_vhci_fetchdata(self.dev, &raw mut ioc_urb_data)
                .map_err(std::io::Error::from)?;
            // Can't forget about the aliasing rule
            ioc_urb_data.iso_packets = std::ptr::null_mut();
            ioc_iso_packets.set_len(urb.packet_count());
        };

        if let Urb::Iso(urb_iso) = urb {
            for (iso_packet, ioc_iso_packet) in urb_iso.iso_packets.iter_mut().zip(ioc_iso_packets)
            {
                iso_packet.offset = ioc_iso_packet.offset;
                iso_packet.packet_length = ioc_iso_packet.packet_length as i32;
                iso_packet.packet_actual = 0;
                iso_packet.status = IsoStatus::Pending;
            }
        }

        Ok(())
    }

    pub fn giveback(&self, urb: Urb) -> std::io::Result<()> {
        let mut ioc_giveback = ioctl::IocGiveback {
            handle: urb.handle().as_raw_handle(),
            status: urb.status_to_errno_raw(),
            buffer_actual: urb.buffer_actual() as i32,
            ..Default::default()
        };

        let mut ioc_iso_packets =
            heapless::Vec::<ioctl::IocIsoPacketGiveback, MAX_ISO_PACKETS>::new();
        assert!(urb.packet_count() <= MAX_ISO_PACKETS);
        if urb.epadr().is_in() && ioc_giveback.buffer_actual > 0 {
            ioc_giveback.buffer = urb.buffer().as_ptr().cast_mut().cast();
        }
        if let Urb::Iso(ref urb_iso) = urb {
            for iso_packet in urb_iso.iso_packets.iter() {
                ioc_iso_packets
                    .push(ioctl::IocIsoPacketGiveback {
                        packet_actual: iso_packet.packet_actual as u32,
                        status: iso_packet.status.to_errno_raw(true),
                    })
                    .expect("URB should not have more than 64 ISO packets");
            }
            ioc_giveback.iso_packets = ioc_iso_packets.as_mut_ptr();
            ioc_giveback.packet_count = urb.packet_count() as i32;
            ioc_giveback.error_count = urb.error_count();
        }

        // SAFETY: TODO: We allocate our own buffer for the iso packets,
        //         and that shouuuuuld last throughout this call?
        unsafe {
            match ioctl::usb_vhci_giveback(self.dev, &raw mut ioc_giveback) {
                Err(nix::Error::ECANCELED) | Ok(_) => Ok(()),
                Err(nix) => Err(std::io::Error::from(nix)),
            }
        }
    }

    pub fn port_disable(&self, port: Port) -> std::io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            change: PortChange::ENABLE.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }

    pub fn port_resumed(&self, port: Port) -> std::io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            change: PortChange::SUSPEND.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }

    pub fn port_overcurrent(&self, port: Port, set: bool) -> std::io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            change: PortChange::OVERCURRENT.bits(),
            index: port.get(),
            ..Default::default()
        };
        if set {
            ioc_port_stat.status = PortStatus::OVERCURRENT.bits();
        }

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }

    pub fn port_reset_done(&self, port: Port, enable: bool) -> std::io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            index: port.get(),
            change: PortChange::RESET.bits(),
            ..Default::default()
        };
        if enable {
            ioc_port_stat.status = PortStatus::ENABLE.bits();
        } else {
            ioc_port_stat.change |= PortChange::ENABLE.bits();
        }

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }
}

#[derive(Debug)]
pub struct Controller {
    dev: std::fs::File,
    open_ports: BitVec,
    controller_id: i32,
    usb_busnum: i32,
    bus_id: Box<str>,
    work_recv_split: bool,
}

impl Controller {
    pub fn open(num_ports: utils::BoundedU8<1, 32>) -> std::io::Result<Self> {
        let device = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_NONBLOCK)
            .open(USB_VHCI_DEVICE_FILE)?;

        let mut ioc_register = ioctl::IocRegister {
            port_count: num_ports.get(),
            ..Default::default()
        };

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe {
            ioctl::usb_vhci_register(device.as_raw_fd(), &raw mut ioc_register)
                .map_err(std::io::Error::from)?
        };

        Ok(Self {
            dev: device,
            open_ports: BitVec::from_elem(num_ports.get() as usize, false),
            controller_id: ioc_register.id,
            usb_busnum: ioc_register.usb_busnum,
            bus_id: std::str::from_utf8(&ioc_register.bus_id)
                .unwrap()
                .trim_end_matches('\0')
                .into(),
            work_recv_split: false,
        })
    }

    pub fn free_ports(&self) -> u64 {
        self.open_ports.count_zeros()
    }

    pub fn is_active(&self) -> bool {
        !self.open_ports.none()
    }

    /// Clones the underlying file descriptor into
    /// an object with less capabilities than the
    /// main controller.
    ///
    ///
    pub fn remote(&self) -> Remote {
        Remote {
            dev: self.dev.as_raw_fd(),
        }
    }

    pub fn work_receiver(&mut self) -> Option<WorkReceiver> {
        if self.work_recv_split {
            None
        } else {
            self.work_recv_split = true;
            Some(WorkReceiver {
                dev: self.dev.as_raw_fd(),
            })
        }
    }

    pub fn fetch_work(&self) -> std::io::Result<Work> {
        self.fetch_work_timeout(utils::TimeoutMillis::Time(
            utils::BoundedI16::new(100).unwrap(),
        ))
    }

    pub fn fetch_work_timeout(&self, timeout: utils::TimeoutMillis) -> std::io::Result<Work> {
        if self.work_recv_split {
            Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists))?
        } else {
            WorkReceiver {
                dev: self.dev.as_raw_fd(),
            }
            .fetch_work_timeout(timeout)
        }
    }

    pub fn fetch_data(&self, urb: &mut Urb) -> std::io::Result<()> {
        Remote {
            dev: self.dev.as_raw_fd(),
        }
        .fetch_data(urb)
    }

    pub fn giveback(&self, urb: Urb) -> std::io::Result<()> {
        Remote {
            dev: self.dev.as_raw_fd(),
        }
        .giveback(urb)
    }

    pub fn port_connect_any(&mut self, data_rate: DataRate) -> std::io::Result<Port> {
        let port = Port::new(
            self.open_ports
                .iter()
                .position(|in_use| !in_use)
                .unwrap()
                .add(1) as u8,
        )
        .unwrap();
        self.port_connect(port, data_rate)?;
        Ok(port)
    }

    pub fn port_connect(&mut self, port: Port, data_rate: DataRate) -> std::io::Result<()> {
        let mut status = PortStatus::CONNECTION;
        match data_rate {
            DataRate::Full => (),
            DataRate::Low => status |= PortStatus::LOW_SPEED,
            DataRate::High => status |= PortStatus::HIGH_SPEED,
        }
        let mut ioc_port_stat = ioctl::IocPortStat {
            status: status.bits(),
            change: PortChange::CONNECTION.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut ioc_port_stat)
                .map_err(std::io::Error::from)?
        };

        self.open_ports.set(port.get().sub(1) as usize, true);

        Ok(())
    }

    pub fn port_disconnect(&mut self, port: Port) -> std::io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            change: PortChange::CONNECTION.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut ioc_port_stat)
                .map_err(std::io::Error::from)?
        };

        self.open_ports.set(port.get().sub(1) as usize, false);
        Ok(())
    }

    pub fn port_disable(&self, port: Port) -> std::io::Result<()> {
        Remote {
            dev: self.dev.as_raw_fd(),
        }
        .port_disable(port)
    }

    pub fn port_resumed(&self, port: Port) -> std::io::Result<()> {
        Remote {
            dev: self.dev.as_raw_fd(),
        }
        .port_resumed(port)
    }

    pub fn port_overcurrent(&self, port: Port, set: bool) -> std::io::Result<()> {
        Remote {
            dev: self.dev.as_raw_fd(),
        }
        .port_overcurrent(port, set)
    }

    pub fn port_reset_done(&self, port: Port, enable: bool) -> std::io::Result<()> {
        Remote {
            dev: self.dev.as_raw_fd(),
        }
        .port_reset_done(port, enable)
    }
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

impl TryFrom<ioctl::IocWork> for Work {
    type Error = nix::Error;

    fn try_from(ioc_work: ioctl::IocWork) -> Result<Self, Self::Error> {
        match ioc_work.typ {
            ioctl::USB_VHCI_WORK_TYPE_PORT_STAT => {
                // SAFETY: The ioctl always tells us what type was written
                //         through the "typ" parameter, so we can safely
                //         use this variant.
                Ok(Work::PortStat(unsafe { ioc_work.work.port.into() }))
            }
            ioctl::USB_VHCI_WORK_TYPE_PROCESS_URB => {
                // SAFETY: The ioctl always tells us what type was written
                //         through the "typ" parameter, so we can safely
                //         use this variant.
                let ioc_urb = unsafe { ioc_work.work.urb };
                let urb = match ioc_urb.typ {
                    ioctl::USB_VHCI_URB_TYPE_ISO => Urb::Iso(UrbIso {
                        status: IsoStatus::Pending,
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
                    ioctl::USB_VHCI_URB_TYPE_INT => Urb::Int(UrbInt {
                        status: IsoStatus::Pending,
                        handle: UrbHandle(ioc_work.handle),
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(ioc_urb.buffer_length.try_into().unwrap());
                            let actual_len = if Endpoint::new(ioc_urb.endpoint).is_out() {
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
                    ioctl::USB_VHCI_URB_TYPE_CONTROL => Urb::Ctrl(UrbControl {
                        status: IsoStatus::Pending,
                        handle: UrbHandle(ioc_work.handle),
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(ioc_urb.buffer_length.try_into().unwrap());
                            let actual_len = if Endpoint::new(ioc_urb.endpoint).is_out() {
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
                    ioctl::USB_VHCI_URB_TYPE_BULK => Urb::Bulk(UrbBulk {
                        status: IsoStatus::Pending,
                        handle: UrbHandle(ioc_work.handle),
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(ioc_urb.buffer_length.try_into().unwrap());
                            let actual_len = if Endpoint::new(ioc_urb.endpoint).is_out() {
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
                    _ => Err(nix::Error::EBADMSG)?,
                };

                Ok(Work::ProcessUrb(urb))
            }
            ioctl::USB_VHCI_WORK_TYPE_CANCEL_URB => Ok(Work::CancelUrb(UrbHandle(ioc_work.handle))),
            _ => Err(nix::Error::EBADMSG),
        }
    }
}

pub mod ioctl {
    use std::ffi::c_void;

    use nix::{ioctl_readwrite, ioctl_write_ptr};

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

    #[derive(Clone, Copy, Default)]
    #[repr(C)]
    pub struct IocRegister {
        pub id: i32,
        pub usb_busnum: i32,
        pub bus_id: [u8; 20],
        pub port_count: u8,
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
        pub reserved1: u8,
        pub reserved2: u8,
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
        pub typ: u8,
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

    #[derive(Clone, Copy, Default)]
    #[repr(C)]
    pub struct IocWork {
        pub handle: u64,
        pub work: IocWorkUnion,
        pub timeout: i16,
        pub typ: u8,
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
}

#[cfg(test)]
mod tests {
    use utils::{BoundedI16, BoundedU8, TimeoutMillis};

    use super::*;

    const NUM_PORTS: BoundedU8<1, 32> = BoundedU8::new(1).unwrap();

    #[test]
    fn invalid_fd_fails() {
        let mut vhci = Controller::open(NUM_PORTS).unwrap();
        let remote = vhci.remote();
        let port = vhci.port_connect_any(DataRate::Full).unwrap();
        drop(vhci);
        dbg!(remote.port_reset_done(port, true).unwrap_err());
    }

    #[test]
    fn can_create_vhci() {
        let _vhci = Controller::open(NUM_PORTS).unwrap();
    }

    #[test]
    fn can_connect_disconnect_port() {
        let mut vhci = Controller::open(NUM_PORTS).unwrap();
        let port = vhci.port_connect_any(DataRate::High).unwrap();
        vhci.port_disconnect(port).unwrap();
    }

    #[test]
    fn can_fetch_work() {
        let num_ports = BoundedU8::new(2).unwrap();
        let mut vhci = Controller::open(num_ports).unwrap();
        let mut prev = PortStat {
            status: PortStatus::empty(),
            change: PortChange::empty(),
            index: Port::new(1).unwrap(),
            flags: PortFlag::empty(),
        };
        let _urb = loop {
            let timeout = TimeoutMillis::Time(BoundedI16::new(500).unwrap());
            let work = vhci.fetch_work_timeout(timeout).unwrap();
            eprintln!("{work:?}");
            match work {
                Work::CancelUrb(_) => unreachable!(),
                Work::ProcessUrb(urb) => break urb,
                Work::PortStat(next) => {
                    if (!prev.status).contains(PortStatus::POWER)
                        && next.status.contains(PortStatus::POWER)
                    {
                        vhci.port_connect(next.index, DataRate::Full).unwrap();
                    } else if (!prev.status).contains(PortStatus::RESET)
                        && next
                            .status
                            .contains(PortStatus::RESET | PortStatus::CONNECTION)
                    {
                        vhci.port_reset_done(next.index, true).unwrap();
                    } else if (!prev.flags).contains(PortFlag::RESUMING)
                        && next.flags.contains(PortFlag::RESUMING)
                        && next.status.contains(PortStatus::CONNECTION)
                    {
                        vhci.port_resumed(next.index).unwrap();
                    }
                    prev = next;
                }
            }
        };

        vhci.port_disconnect(Port::new(1).unwrap()).unwrap();
        vhci.port_disconnect(Port::new(2).unwrap()).unwrap();
    }
}
