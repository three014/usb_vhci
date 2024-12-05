use std::{os::fd::AsRawFd, path::Path, time::Duration};

pub mod utils {
    use std::time::Duration;

    pub struct MillisUnderOneSec(Duration);

    impl MillisUnderOneSec {
        pub const fn new(millis: u64) -> Option<Self> {
            if millis < 1000 {
                Some(MillisUnderOneSec(Duration::from_millis(millis)))
            } else {
                None
            }
        }

        pub const fn as_i16(&self) -> i16 {
            self.0.as_millis() as i16
        }
    }
}

pub enum Error {
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
    status: Result<(), Error>,
}

pub struct Urb {
    handle: u64,
    buffer: Vec<u8>,
    // buffer: *mut u8,
    // iso_packets: *mut IsoPacket,
    // buffer_length: i32,
    // buffer_actual: i32,
    // packet_count: i32,
    iso_packets: Box<[IsoPacket]>,
    error_count: i32,
    status: i32,
    interval: i32,
    flags: u16,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    bm_request_type: u8,
    b_request: u8,
    devadr: u8,
    epadr: u8,
    tp: u8,
}

pub enum PortStatus {
    Connection = 0x0001,
    Enable = 0x0002,
    Suspend = 0x0004,
    Overcurrent = 0x0008,
    Reset = 0x0010,
    Power = 0x0100,
    LowSpeed = 0x0200,
    HighSpeed = 0x400,
}

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
    pub fn open(num_ports: u8) -> std::io::Result<Self> {
        let device = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(USB_VHCI_DEVICE_FILE)?;

        let mut register = ioctl::IocRegister::new(num_ports);
        unsafe {
            ioctl::usb_vhci_register(device.as_raw_fd(), &raw mut register)
                .map_err(|nix| std::io::Error::from(nix))?
        };

        Ok(Self {
            dev: device,
            controller_id: register.id(),
            usb_busnum: register.usb_busnum(),
            bus_id: std::str::from_utf8(register.bus_id())
                .unwrap()
                .try_into()
                .unwrap(),
        })
    }

    pub fn fetch_work_timeout(&self, timeout: utils::MillisUnderOneSec) -> std::io::Result<Work> {
        let mut work = ioctl::IocWork::default();
        work.timeout = timeout.as_i16();
        unsafe {
            ioctl::usb_vhci_fetchwork(self.dev.as_raw_fd(), &raw mut work)
                .map_err(|nix| std::io::Error::from(nix))?
        };


        todo!()
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

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct IocRegister {
        id: i32,
        usb_busnum: i32,
        bus_id: [u8; 20],
        port_count: u8,
    }

    impl IocRegister {
        pub fn new(port_count: u8) -> Self {
            Self {
                id: 0,
                usb_busnum: 0,
                bus_id: [0; 20],
                port_count,
            }
        }

        pub fn id(&self) -> i32 {
            self.id
        }
        pub fn usb_busnum(&self) -> i32 {
            self.usb_busnum
        }

        pub fn bus_id(&self) -> &[u8] {
            &self.bus_id
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
        status: u16,
        change: u16,
        index: u8,
        flags: u8,
        reserved1: u8,
        reserved2: u8,
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
        bm_request_type: u8,
        b_request: u8,
        w_value: u16,
        w_index: u16,
        w_length: u16,
    }

    #[derive(Clone, Copy, Default)]
    #[repr(C)]
    pub struct IocUrb {
        setup_packet: IocSetupPacket,
        buffer_length: i32,
        interval: i32,
        packet_count: i32,
        flags: u16,
        address: u8,
        endpoint: u8,
        tp: u8,
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
        offset: u32,
        packet_length: u32,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct IocUrbData {
        handle: u64,
        buffer: *mut c_void,
        iso_packets: *mut IocIsoPacketData,
        buffer_length: i32,
        packet_count: i32,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct IocIsoPacketGiveback {
        packet_actual: u32,
        status: i32,
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
