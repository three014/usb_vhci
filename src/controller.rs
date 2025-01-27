use std::{
    io,
    ops::{Add, Sub},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
};

use bit_vec::BitVec;

use crate::{
    ioctl::{self, UrbType}, usbfs::Dir, utils::{BoundedI16, BoundedU8, TimeoutMillis}, DataRate, IsoPacketDataMut, IsoPacketGivebackMut, Port, PortChange, PortStatus, TransferMut, Urb, MAX_ISO_PACKETS
};

static USB_VHCI_DEVICE_FILE: &str = "/dev/usb-vhci";

#[derive(Debug)]
pub struct WorkReceiver {
    dev: std::os::unix::io::RawFd,
}

impl WorkReceiver {
    const fn new(dev: std::os::unix::io::RawFd) -> Self {
        Self { dev }
    }

    pub fn fetch_work(&self) -> io::Result<ioctl::IocWork> {
        self.fetch_work_timeout(TimeoutMillis::Time(BoundedI16::new(100).unwrap()))
    }

    pub fn fetch_work_timeout(&self, timeout: TimeoutMillis) -> io::Result<ioctl::IocWork> {
        let mut ioc_work = ioctl::IocWork {
            timeout: match timeout {
                // utils::TimeoutMillis::Unlimited => ioctl::USB_VHCI_TIMEOUT_INFINITE,
                TimeoutMillis::Time(time) => time.get(),
            },
            ..Default::default()
        };

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe { ioctl::usb_vhci_fetchwork(self.dev, &raw mut ioc_work).map_err(io::Error::from)? };

        Ok(ioc_work)
    }
}

#[derive(Debug, Clone)]
pub struct Remote {
    dev: std::os::unix::io::RawFd,
}

impl Remote {
    const fn new(dev: std::os::unix::io::RawFd) -> Self {
        Self { dev }
    }

    pub fn fetch_data(&self, mut urb: impl Urb + IsoPacketDataMut + TransferMut) -> io::Result<()> {
        let buffer_length = urb.transfer_mut().len().try_into().unwrap();
        let buffer = urb.transfer_mut().as_mut_ptr().cast();
        let packet_count = urb.iso_packet_data_mut().len();
        assert!(packet_count <= MAX_ISO_PACKETS);

        let mut ioc_urb_data = ioctl::IocUrbData {
            handle: urb.handle().get(),
            buffer,
            buffer_length,
            ..Default::default()
        };

        if 0 < packet_count {
            ioc_urb_data.iso_packets = urb.iso_packet_data_mut().as_mut_ptr();
            ioc_urb_data.packet_count = packet_count.try_into().unwrap();
        }

        // SAFETY:
        // - `ioc_iso_packets` is valid and initialized for the ioctl call
        // - transfer buffer is initialized and its length does not change
        unsafe {
            ioctl::usb_vhci_fetchdata(self.dev, &raw mut ioc_urb_data).map_err(io::Error::from)?
        };

        Ok(())
    }

    pub fn giveback(&self, mut urb: impl Urb + IsoPacketGivebackMut + TransferMut) -> io::Result<()> {
        let packet_count = urb.iso_packet_giveback_mut().len();
        let buffer_len = urb.bytes_transferred();
        assert!(packet_count <= MAX_ISO_PACKETS);

        let mut ioc_giveback = ioctl::IocGiveback {
            handle: urb.handle().get(),
            status: urb.status().to_errno_raw(ioctl::UrbType::Iso == urb.kind()),
            buffer_actual: buffer_len.try_into().unwrap(),
            ..Default::default()
        };

        if Dir::In == urb.dir() && 0 < buffer_len {
            assert_eq!(buffer_len as usize, urb.transfer_mut().len());
            ioc_giveback.buffer = urb.transfer_mut().as_mut_ptr().cast();
        }

        if ioctl::UrbType::Iso == urb.kind() {
            ioc_giveback.iso_packets = urb.iso_packet_giveback_mut().as_mut_ptr();
            ioc_giveback.packet_count = packet_count.try_into().unwrap();
            ioc_giveback.error_count = urb.error_count().into();
        }

        // SAFETY: All buffers are valid for the ioctl call
        unsafe {
            match ioctl::usb_vhci_giveback(self.dev, &raw mut ioc_giveback) {
                Err(nix::Error::ECANCELED) | Ok(_) => Ok(()),
                Err(nix) => Err(io::Error::from(nix)),
            }
        }
    }

    pub fn port_disable(&self, port: Port) -> io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            change: PortChange::ENABLE.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat).map_err(io::Error::from)?
        };
        Ok(())
    }

    pub fn port_resumed(&self, port: Port) -> io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            change: PortChange::SUSPEND.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat).map_err(io::Error::from)?
        };
        Ok(())
    }

    pub fn port_overcurrent(&self, port: Port, set: bool) -> io::Result<()> {
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
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat).map_err(io::Error::from)?
        };
        Ok(())
    }

    pub fn port_reset_done(&self, port: Port, enable: bool) -> io::Result<()> {
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
            ioctl::usb_vhci_portstat(self.dev, &raw mut ioc_port_stat).map_err(io::Error::from)?
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
    pub fn open(num_ports: BoundedU8<1, 32>) -> io::Result<Self> {
        let device = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_NONBLOCK)
            .open(USB_VHCI_DEVICE_FILE)?;

        let mut ioc_register = ioctl::IocRegister::new(num_ports.get());

        // SAFETY: We are using a valid file descriptor that we
        //         are sure will last for the entire duration of this
        //         ioctl. We also pass in a valid pointer for this
        //         ioctl's return type.
        unsafe {
            ioctl::usb_vhci_register(device.as_raw_fd(), &raw mut ioc_register)
                .map_err(io::Error::from)?
        };

        Ok(Self {
            dev: device,
            open_ports: BitVec::from_elem(num_ports.get() as usize, false),
            controller_id: ioc_register.id,
            usb_busnum: ioc_register.usb_busnum,
            bus_id: ioc_register
                .bus_id()
                .to_str()
                .map(|s| s.trim_end_matches('\0'))
                .map(Box::from)
                .unwrap(),
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
    pub fn remote(&self) -> Remote {
        Remote::new(self.dev.as_raw_fd())
    }

    pub fn work_receiver(&mut self) -> Option<WorkReceiver> {
        if self.work_recv_split {
            None
        } else {
            self.work_recv_split = true;
            Some(WorkReceiver::new(self.dev.as_raw_fd()))
        }
    }

    pub fn return_work_receiver(&mut self, _recv: WorkReceiver) {
        self.work_recv_split = false;
    }

    pub fn fetch_work(&self) -> io::Result<ioctl::IocWork> {
        const DEFAULT_TIMEOUT: TimeoutMillis = TimeoutMillis::Time(BoundedI16::new(100).unwrap());
        self.fetch_work_timeout(DEFAULT_TIMEOUT)
    }

    pub fn fetch_work_timeout(&self, timeout: TimeoutMillis) -> io::Result<ioctl::IocWork> {
        if self.work_recv_split {
            Err(io::Error::from(io::ErrorKind::AlreadyExists))?
        } else {
            WorkReceiver::new(self.dev.as_raw_fd()).fetch_work_timeout(timeout)
        }
    }

    pub fn fetch_data(&self, urb: impl Urb + TransferMut + IsoPacketDataMut) -> io::Result<()> {
        Remote::new(self.dev.as_raw_fd()).fetch_data(urb)
    }

    pub fn giveback(&self, urb: impl Urb + TransferMut + IsoPacketGivebackMut) -> io::Result<()> {
        Remote::new(self.dev.as_raw_fd()).giveback(urb)
    }

    pub fn port_connect_any(&mut self, data_rate: DataRate) -> io::Result<Port> {
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

    pub fn port_connect(&mut self, port: Port, data_rate: DataRate) -> io::Result<()> {
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
                .map_err(io::Error::from)?
        };

        self.open_ports.set(port.get().sub(1) as usize, true);

        Ok(())
    }

    pub fn port_disconnect(&mut self, port: Port) -> io::Result<()> {
        let mut ioc_port_stat = ioctl::IocPortStat {
            change: PortChange::CONNECTION.bits(),
            index: port.get(),
            ..Default::default()
        };

        // SAFETY: Both the file descriptor and raw mut pointer
        //         are valid for the duration of this ioctl call.
        unsafe {
            ioctl::usb_vhci_portstat(self.dev.as_raw_fd(), &raw mut ioc_port_stat)
                .map_err(io::Error::from)?
        };

        self.open_ports.set(port.get().sub(1) as usize, false);
        Ok(())
    }

    pub fn port_disable(&self, port: Port) -> io::Result<()> {
        Remote::new(self.dev.as_raw_fd()).port_disable(port)
    }

    pub fn port_resumed(&self, port: Port) -> io::Result<()> {
        Remote::new(self.dev.as_raw_fd()).port_resumed(port)
    }

    pub fn port_overcurrent(&self, port: Port, set: bool) -> io::Result<()> {
        Remote::new(self.dev.as_raw_fd()).port_overcurrent(port, set)
    }

    pub fn port_reset_done(&self, port: Port, enable: bool) -> io::Result<()> {
        Remote::new(self.dev.as_raw_fd()).port_reset_done(port, enable)
    }
}

#[cfg(test)]
mod tests {
    use utils::{BoundedI16, BoundedU8, TimeoutMillis};

    use crate::{utils, PortFlag};

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
        let mut prev = ioctl::IocPortStat::default();

        let _urb = loop {
            let timeout = TimeoutMillis::Time(BoundedI16::new(500).unwrap());
            let work = vhci.fetch_work_timeout(timeout).unwrap();
            // SAFETY: We don't alter the `typ` field, which
            //         satisfies the safety constraints
            match unsafe { work.into_inner() } {
                ioctl::Work::ProcessUrb((urb, _handle)) => break urb,
                ioctl::Work::CancelUrb(_handle) => unreachable!(),
                ioctl::Work::PortStat(next) => {
                    if (!prev.status()).contains(PortStatus::POWER)
                        && next.status().contains(PortStatus::POWER)
                    {
                        vhci.port_connect(next.index(), DataRate::Full).unwrap();
                    } else if (!prev.status()).contains(PortStatus::RESET)
                        && next
                            .status()
                            .contains(PortStatus::RESET | PortStatus::CONNECTION)
                    {
                        vhci.port_reset_done(next.index(), true).unwrap();
                    } else if (!prev.flags()).contains(PortFlag::RESUMING)
                        && next.flags().contains(PortFlag::RESUMING)
                        && next.status().contains(PortStatus::CONNECTION)
                    {
                        vhci.port_resumed(next.index()).unwrap();
                    }
                    prev = next;
                }
            }
        };
    }
}
