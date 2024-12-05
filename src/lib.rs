use std::os::fd::AsRawFd;

pub mod utils {
    pub enum TimeoutMillis {
        Unlimited,
        Time(ClosedBoundedI16<1, 1000>),
    }

    #[repr(transparent)]
    #[derive(Debug, Clone, Copy)]
    pub struct OpenBoundedU8<const LOWER: u8, const UPPER: u8>(u8);

    impl<const LOWER: u8, const UPPER: u8> OpenBoundedU8<LOWER, UPPER> {
        pub const fn new(num: u8) -> Option<Self> {
            if LOWER > num || UPPER < num {
                None
            } else {
                Some(Self(num))
            }
        }

        pub const fn get(&self) -> u8 {
            self.0
        }
    }

    #[repr(transparent)]
    #[derive(Debug, Clone, Copy)]
    pub struct ClosedBoundedI16<const LOWER: i16, const UPPER: i16>(i16);

    impl<const LOWER: i16, const UPPER: i16> ClosedBoundedI16<LOWER, UPPER> {
        pub const fn new(num: i16) -> Option<Self> {
            if LOWER >= num || UPPER <= num {
                None
            } else {
                Some(Self(num))
            }
        }

        pub const fn get(&self) -> i16 {
            self.0
        }
    }
}

#[derive(Debug, num_enum::TryFromPrimitive)]
#[repr(i32)]
pub enum Status {
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
    buffer_length: i32,
    /// address
    devadr: u8,
    /// endpoint
    epadr: u8,
    packet_count: i32,
}

pub struct UrbInt {
    status: Status,
    handle: u64,
    buffer_length: i32,
    buffer_actual: i32,
    devadr: u8,
    epadr: u8,
    interval: i32,
}

pub struct UrbControl {
    status: Status,
    handle: u64,
    buffer_length: i32,
    buffer_actual: i32,
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
    buffer_length: i32,
    buffer_actual: i32,
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
    pub const fn requires_fetch_work(&self) -> bool {
        match self {
            Urb::Iso(urb_iso) => urb_iso.packet_count != 0,
            Urb::Int(urb_int) => urb_int.buffer_actual != 0,
            Urb::Ctrl(urb_control) => urb_control.buffer_actual != 0,
            Urb::Bulk(urb_bulk) => urb_bulk.buffer_actual != 0,
        }
    }
}

#[derive(num_enum::TryFromPrimitive)]
#[repr(u16)]
pub enum PortStatus {
    Connection = 0x0001,
    Enable = 0x0002,
    Suspend = 0x0004,
    Overcurrent = 0x0008,
    Reset = 0x0010,
    Power = 0x0100,
    LowSpeed = 0x0200,
    HighSpeed = 0x0400,
}

#[derive(num_enum::TryFromPrimitive)]
#[repr(u16)]
pub enum PortChange {
    Connection = 0x0001,
    Enable = 0x0002,
    Suspend = 0x0004,
    Overcurrent = 0x0008,
    Reset = 0x0010,
}

pub struct PortStat {
    status: PortStatus,
    change: PortChange,
    index: u8,
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

pub struct Device {
    dev: std::fs::File,
    controller_id: i32,
    usb_busnum: i32,
    bus_id: Box<str>,
}

static USB_VHCI_DEVICE_FILE: &str = "/dev/usb-vhci";

impl Device {
    pub fn open(num_ports: utils::OpenBoundedU8<0, 32>) -> std::io::Result<Self> {
        let device = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(USB_VHCI_DEVICE_FILE)?;

        let mut register = ioctl::IocRegister::default();
        register.port_count = num_ports.get();

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe {
            ioctl::usb_vhci_register(device.as_raw_fd(), &raw mut register)
                .map_err(|nix| std::io::Error::from(nix))?
        };

        Ok(Self {
            dev: device,
            controller_id: register.id,
            usb_busnum: register.usb_busnum,
            bus_id: std::str::from_utf8(&register.bus_id)
                .unwrap()
                .try_into()
                .unwrap(),
        })
    }

    pub fn fetch_work_timeout(&self, timeout: utils::TimeoutMillis) -> std::io::Result<Work> {
        let mut work = ioctl::IocWork::default();
        work.timeout = match timeout {
            utils::TimeoutMillis::Unlimited => -1,
            utils::TimeoutMillis::Time(time) => time.get(),
        };

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe {
            ioctl::usb_vhci_fetchwork(self.dev.as_raw_fd(), &raw mut work)
                .map_err(|nix| std::io::Error::from(nix))?
        };

        work.try_into().map_err(|nix| std::io::Error::from(nix))
    }
}

impl From<ioctl::IocPortStat> for PortStat {
    fn from(port_stat: ioctl::IocPortStat) -> Self {
        Self {
            status: port_stat.status.try_into().unwrap(),
            change: port_stat.change.try_into().unwrap(),
            index: port_stat.index,
            flags: port_stat.flags,
        }
    }
}

const fn is_out(epadr: u8) -> u8 {
    !((epadr) & 0x80)
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
                        buffer_length: iocurb.buffer_length,
                        devadr: iocurb.address,
                        epadr: iocurb.endpoint,
                        packet_count: iocurb.packet_count,
                    }),
                    ioctl::USB_VHCI_URB_TYPE_INT => Urb::Int(UrbInt {
                        status: Status::Pending,
                        handle: work.handle,
                        buffer_length: iocurb.buffer_length,
                        buffer_actual: if is_out(iocurb.endpoint) != 0 {
                            iocurb.buffer_length
                        } else {
                            0
                        },
                        devadr: iocurb.address,
                        epadr: iocurb.endpoint,
                        interval: iocurb.interval,
                    }),
                    ioctl::USB_VHCI_URB_TYPE_CONTROL => Urb::Ctrl(UrbControl {
                        status: Status::Pending,
                        handle: work.handle,
                        buffer_length: iocurb.buffer_length,
                        buffer_actual: if is_out(iocurb.endpoint) != 0 {
                            iocurb.buffer_length
                        } else {
                            0
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
                        buffer_length: iocurb.buffer_length,
                        buffer_actual: if is_out(iocurb.endpoint) != 0 {
                            iocurb.buffer_length
                        } else {
                            0
                        },
                        devadr: iocurb.address,
                        epadr: iocurb.endpoint,
                        flags: iocurb.flags
                            & (ioctl::USB_VHCI_URB_FLAGS_SHORT_NOT_OK
                                | ioctl::USB_VHCI_URB_FLAGS_ZERO_PACKET),
                    }),
                    _ => Err(nix::Error::EBADMSG.into())?,
                };

                Ok(Work::Urb(urb))
            }
            ioctl::USB_VHCI_WORK_TYPE_CANCEL_URB => Ok(Work::Handle(work.handle)),
            _ => Err(nix::Error::EBADMSG.into()),
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

    #[derive(Clone, Copy)]
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

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct IocIsoPacketGiveback {
        pub packet_actual: u32,
        pub status: i32,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct IocGiveback {
        handle: u64,
        buffer: *mut c_void,
        iso_packets: *mut IocIsoPacketGiveback,
        status: i32,
        buffer_actual: i32,
        packet_count: i32,
        error_count: i32,
    }

    ioctl_write_ptr!(
        usb_vhci_giveback,
        USB_VHCI_HCD_IOC_MAGIC,
        USB_VHCI_HCD_IOCGIVEBACK,
        IocGiveback
    );
}
