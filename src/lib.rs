use std::{ops::{Add, Sub}, os::fd::AsRawFd};

use bit_vec::BitVec;

pub mod utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Port(utils::OpenBoundedU8<0, 32>);

impl Port {
    pub const fn new(port: u8) -> Option<Self> {
        if let Some(num) = utils::OpenBoundedU8::new(port) {
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
            Status::Error => {
                if is_iso {
                    -EXDEV
                } else {
                    -EPROTO
                }
            }
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
            Status::AllIsoPacketsFailed => {
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
            EINVAL => {
                if is_iso {
                    Status::AllIsoPacketsFailed
                } else {
                    Status::Error
                }
            }
            _ => Status::Error,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct IsoPacket {
    offset: u32,
    packet_length: i32,
    packet_actual: i32,
    status: Status,
}

pub struct UrbIso {
    status: Status,
    handle: u64,
    /// buffer length is the actual length for iso urbs
    buffer: Box<[u8]>,
    iso_packets: Box<[IsoPacket]>,
    error_count: i32,
    /// address
    devadr: u8,
    /// endpoint
    epadr: u8,
    interval: i32,
}

pub struct UrbInt {
    status: Status,
    handle: u64,
    buffer: Vec<u8>,
    devadr: u8,
    epadr: u8,
    interval: i32,
}

pub struct UrbControl {
    status: Status,
    handle: u64,
    buffer: Vec<u8>,
    devadr: u8,
    epadr: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    bm_request_type: u8,
    b_request: u8,
}

pub struct UrbBulk {
    status: Status,
    handle: u64,
    buffer: Vec<u8>,
    devadr: u8,
    epadr: u8,
    flags: u16,
}

pub enum Urb {
    Iso(UrbIso),
    Int(UrbInt),
    Ctrl(UrbControl),
    Bulk(UrbBulk),
}

impl Urb {
    pub const fn handle(&self) -> u64 {
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

    pub const fn epadr(&self) -> u8 {
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
    pub struct PortStatus: u16 {
        const CONNECTION = 0x0001;
        const ENABLE = 0x0002;
        const SUSPEND = 0x0004;
        const OVERCURRENT = 0x0008;
        const RESET = 0x0010;
        const POWER = 0x0100;
        const LOW_SPEED = 0x0200;
        const HIGH_SPEED = 0x0400;
    }

    pub struct PortChange: u16 {
        const CONNECTION = 0x0001;
        const ENABLE = 0x0002;
        const SUSPEND = 0x0004;
        const OVERCURRENT = 0x0008;
        const RESET = 0x0010;
    }
}

pub struct PortStat {
    status: PortStatus,
    change: PortChange,
    index: Port,
    flags: u8,
}

pub enum DataRate {
    Full = 0,
    Low = 1,
    High = 2,
}

pub enum Work {
    Handle(u64),
    Urb(Urb),
    PortStat(PortStat),
}

pub struct Vhci {
    dev: std::fs::File,
    open_ports: BitVec,
    controller_id: i32,
    usb_busnum: i32,
    bus_id: Box<str>,
}

static USB_VHCI_DEVICE_FILE: &str = "/dev/usb-vhci";

impl Vhci {
    pub fn open(num_ports: utils::OpenBoundedU8<0, 32>) -> std::io::Result<Self> {
        let device = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(USB_VHCI_DEVICE_FILE)?;

        let mut register = ioctl::IocRegister {
            port_count: num_ports.get(),
            ..Default::default()
        };

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe {
            ioctl::usb_vhci_register(device.as_raw_fd(), &raw mut register)
                .map_err(std::io::Error::from)?
        };

        Ok(Self {
            dev: device,
            open_ports: BitVec::from_elem(num_ports.get() as usize, false),
            controller_id: register.id,
            usb_busnum: register.usb_busnum,
            bus_id: std::str::from_utf8(&register.bus_id).unwrap().into(),
        })
    }

    pub fn fetch_work(&self) -> std::io::Result<Work> {
        self.fetch_work_timeout(utils::TimeoutMillis::Time(
            utils::ClosedBoundedI16::new(100).unwrap(),
        ))
    }

    pub fn fetch_work_timeout(&self, timeout: utils::TimeoutMillis) -> std::io::Result<Work> {
        let mut work = ioctl::IocWork {
            timeout: match timeout {
                utils::TimeoutMillis::Unlimited => ioctl::USB_VHCI_TIMEOUT_INFINITE,
                utils::TimeoutMillis::Time(time) => time.get(),
            },
            ..Default::default()
        };

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe {
            ioctl::usb_vhci_fetchwork(self.dev.as_raw_fd(), &raw mut work)
                .map_err(std::io::Error::from)?
        };

        work.try_into().map_err(std::io::Error::from)
    }

    pub fn fetch_data(&self, urb: &mut Urb) -> std::io::Result<()> {
        let mut urb_data = ioctl::IocUrbData {
            handle: urb.handle(),
            buffer_length: urb.buffer_length() as i32,
            packet_count: urb.packet_count() as i32,
            buffer: urb.buffer_mut().as_mut_ptr().cast(),
            ..Default::default()
        };
        let mut iso_packets = Vec::with_capacity(urb.packet_count());
        if urb.packet_count() > 0 {
            urb_data.iso_packets = iso_packets.as_mut_ptr();
        }

        // SAFETY: TODO: We allocate our own buffer for the iso packets,
        //         and that shouuuuuld last throughout this call?
        //         After the ioctl call, `iso_packets` should have the
        //         same len as the buffer in the urb??
        unsafe {
            ioctl::usb_vhci_fetchdata(self.dev.as_raw_fd(), &raw mut urb_data)
                .map_err(std::io::Error::from)?;
            urb_data.iso_packets = std::ptr::null_mut();
            iso_packets.set_len(urb.packet_count());
        };

        if let Urb::Iso(urb_iso) = urb {
            for (iso_packet, ioc_iso_packet) in urb_iso.iso_packets.iter_mut().zip(iso_packets) {
                iso_packet.offset = ioc_iso_packet.offset;
                iso_packet.packet_length = ioc_iso_packet.packet_length as i32;
                iso_packet.packet_actual = 0;
                iso_packet.status = Status::Pending;
            }
        }

        Ok(())
    }

    pub fn giveback(&self, urb: &mut Urb) -> std::io::Result<()> {
        let mut giveback = ioctl::IocGiveback {
            handle: urb.handle(),
            status: urb.status_to_errno_raw(),
            buffer_actual: urb.buffer_actual() as i32,
            ..Default::default()
        };

        let mut iso_packets: Vec<ioctl::IocIsoPacketGiveback> =
            Vec::with_capacity(urb.packet_count());

        if is_in(urb.epadr()) != 0 && giveback.buffer_actual > 0 {
            giveback.buffer = urb.buffer_mut().as_mut_ptr().cast();
        }
        if let Urb::Iso(ref urb_iso) = urb {
            for (iso_packet, ioc_iso_packet_giveback) in
                urb_iso.iso_packets.iter().zip(iso_packets.iter_mut())
            {
                ioc_iso_packet_giveback.status = iso_packet.status.to_errno_raw(true);
                ioc_iso_packet_giveback.packet_actual = iso_packet.packet_actual as u32;
            }
            giveback.iso_packets = iso_packets.as_mut_ptr();
            giveback.packet_count = urb.packet_count() as i32;
            giveback.error_count = urb.error_count();
        }

        // SAFETY: TODO: We allocate our own buffer for the iso packets,
        //         and that shouuuuuld last throughout this call?
        unsafe {
            match ioctl::usb_vhci_giveback(self.dev.as_raw_fd(), &raw mut giveback) {
                Err(nix::Error::ECANCELED) | Ok(_) => Ok(()),
                Err(nix) => Err(std::io::Error::from(nix)),
            }
        }
    }

    pub fn port_connect_any(&mut self, data_rate: DataRate) -> std::io::Result<Port> {
        let port =
            Port::new(self.open_ports.iter().position(|in_use| !in_use).unwrap().add(1) as u8).unwrap();
        dbg!(port);
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
        let mut port_stat = ioctl::IocPortStat {
            status: status.bits(),
            change: PortChange::CONNECTION.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut port_stat)
                .map_err(std::io::Error::from)?
        };

        self.open_ports.set(port.get().sub(1) as usize, true);

        Ok(())
    }

    pub fn port_disconnect(&self, port: Port) -> std::io::Result<()> {
        let mut port_stat = ioctl::IocPortStat {
            change: PortChange::CONNECTION.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }

    pub fn port_disable(&self, port: Port) -> std::io::Result<()> {
        let mut port_stat = ioctl::IocPortStat {
            change: PortChange::ENABLE.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }

    pub fn port_resumed(&self, port: Port) -> std::io::Result<()> {
        let mut port_stat = ioctl::IocPortStat {
            change: PortChange::SUSPEND.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }

    pub fn port_overcurrent(&self, port: Port, set: bool) -> std::io::Result<()> {
        let mut port_stat = ioctl::IocPortStat {
            change: PortChange::OVERCURRENT.bits(),
            index: port.get(),
            ..Default::default()
        };
        if set {
            port_stat.status = PortStatus::OVERCURRENT.bits();
        }

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }

    pub fn port_reset_done(&self, port: Port, enable: bool) -> std::io::Result<()> {
        let mut port_stat = ioctl::IocPortStat {
            index: port.get(),
            change: PortChange::RESET.bits(),
            ..Default::default()
        };
        if enable {
            port_stat.status = PortStatus::ENABLE.bits();
        } else {
            port_stat.change |= PortChange::ENABLE.bits();
        }

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut port_stat)
                .map_err(std::io::Error::from)?
        };
        Ok(())
    }
}

impl From<ioctl::IocPortStat> for PortStat {
    fn from(port_stat: ioctl::IocPortStat) -> Self {
        Self {
            status: PortStatus::from_bits(port_stat.status).unwrap(),
            change: PortChange::from_bits(port_stat.change).unwrap(),
            index: Port::new(port_stat.index).unwrap(),
            flags: port_stat.flags,
        }
    }
}

const fn is_out(epadr: u8) -> u8 {
    !((epadr) & 0x80)
}

const fn is_in(epadr: u8) -> u8 {
    !is_out(epadr)
}

impl TryFrom<ioctl::IocWork> for Work {
    type Error = nix::Error;

    fn try_from(work: ioctl::IocWork) -> Result<Self, Self::Error> {
        match work.tp {
            ioctl::USB_VHCI_WORK_TYPE_PORT_STAT => {
                // SAFETY: The ioctl always tells us what type was written
                //         through the "tp" parameter, so we can safely
                //         use this variant.
                Ok(Work::PortStat(unsafe { work.work.port.into() }))
            }
            ioctl::USB_VHCI_WORK_TYPE_PROCESS_URB => {
                // SAFETY: The ioctl always tells us what type was written
                //         through the "tp" parameter, so we can safely
                //         use this variant.
                let iocurb = unsafe { work.work.urb };
                let urb = match iocurb.tp {
                    ioctl::USB_VHCI_URB_TYPE_ISO => Urb::Iso(UrbIso {
                        status: Status::Pending,
                        handle: work.handle,
                        buffer: vec![0; iocurb.buffer_length.try_into().unwrap()]
                            .into_boxed_slice(),
                        error_count: 0,
                        devadr: iocurb.address,
                        epadr: iocurb.endpoint,
                        iso_packets: vec![
                            IsoPacket::default();
                            iocurb.packet_count.try_into().unwrap()
                        ]
                        .into_boxed_slice(),
                        interval: iocurb.interval,
                    }),
                    ioctl::USB_VHCI_URB_TYPE_INT => Urb::Int(UrbInt {
                        status: Status::Pending,
                        handle: work.handle,
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(iocurb.buffer_length.try_into().unwrap());
                            let actual_len = if is_out(iocurb.endpoint) != 0 {
                                iocurb.buffer_length
                            } else {
                                0
                            };
                            buf.resize(actual_len.try_into().unwrap(), 0);
                            buf
                        },
                        devadr: iocurb.address,
                        epadr: iocurb.endpoint,
                        interval: iocurb.interval,
                    }),
                    ioctl::USB_VHCI_URB_TYPE_CONTROL => Urb::Ctrl(UrbControl {
                        status: Status::Pending,
                        handle: work.handle,
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(iocurb.buffer_length.try_into().unwrap());
                            let actual_len = if is_out(iocurb.endpoint) != 0 {
                                iocurb.buffer_length
                            } else {
                                0
                            };
                            buf.resize(actual_len.try_into().unwrap(), 0);
                            buf
                        },
                        devadr: iocurb.address,
                        epadr: iocurb.endpoint,
                        w_value: iocurb.setup_packet.w_value,
                        w_index: iocurb.setup_packet.w_index,
                        w_length: iocurb.setup_packet.w_length,
                        bm_request_type: iocurb.setup_packet.bm_request_type,
                        b_request: iocurb.setup_packet.b_request,
                    }),
                    ioctl::USB_VHCI_URB_TYPE_BULK => Urb::Bulk(UrbBulk {
                        status: Status::Pending,
                        handle: work.handle,
                        buffer: {
                            let mut buf = Vec::new();
                            buf.reserve_exact(iocurb.buffer_length.try_into().unwrap());
                            let actual_len = if is_out(iocurb.endpoint) != 0 {
                                iocurb.buffer_length
                            } else {
                                0
                            };
                            buf.resize(actual_len.try_into().unwrap(), 0);
                            buf
                        },
                        devadr: iocurb.address,
                        epadr: iocurb.endpoint,
                        flags: iocurb.flags
                            & (ioctl::USB_VHCI_URB_FLAGS_SHORT_NOT_OK
                                | ioctl::USB_VHCI_URB_FLAGS_ZERO_PACKET),
                    }),
                    _ => Err(nix::Error::EBADMSG)?,
                };

                Ok(Work::Urb(urb))
            }
            ioctl::USB_VHCI_WORK_TYPE_CANCEL_URB => Ok(Work::Handle(work.handle)),
            _ => Err(nix::Error::EBADMSG),
        }
    }
}

mod ioctl {
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
        pub tp: u8,
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
        pub tp: u8,
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

    #[derive(Clone, Copy, Default)]
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
    use super::*;

    const NUM_PORTS: utils::OpenBoundedU8<0, 32> = utils::OpenBoundedU8::new(1).unwrap();

    #[test]
    fn can_create_vhci() {
        let _vhci = Vhci::open(NUM_PORTS).unwrap();
    }

    #[test]
    fn can_connect_disconnect_port() {
        let mut vhci = Vhci::open(NUM_PORTS).unwrap();
        let port = vhci.port_connect_any(DataRate::High).unwrap();
        vhci.port_disconnect(port).unwrap();
    }
}
