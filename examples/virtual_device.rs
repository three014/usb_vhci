use std::{
    io,
    time::{Duration, Instant},
};

use log::{debug, trace};
use usb_vhci::{
    ioctl::{self, Req},
    usbfs::{self, CtrlType, Dir, Recipient},
    utils::{BoundedU8, TimeoutMillis},
    Controller, DataRate, PortChange, PortFlag, PortStatus, Status, UrbWithData,
};

static DEV_DESC: &[u8] = &[
    18,   // descriptor length,
    1,    // type: device descriptor,
    0x00, // bcd usb release number
    0x02, //  "
    0,    // device class: per interface
    0,    // device subclass
    0,    // device protocol
    64,   // max packet size
    0xad, // vendor id
    0xde, //  "
    0xef, // product id,
    0xbe, //  "
    0x38, // bcd device release number
    0x11, //  "
    0,    // manufacturer string
    1,    // product string,
    0,    // serial number string,
    1,    // number of configurations
];

static CONF_DESC: &[u8] = &[
    9,    // descriptor length
    2,    // type: configuration descriptor
    18,   // total descriptor length (configuration+interface)
    0,    //  "
    1,    // number of interfaces
    1,    // configuration index
    0,    // configuration string
    0x80, // attributes: none
    0,    // max power
    9,    // descriptor length
    4,    // type: interface
    0,    // interface number
    0,    // alternate setting
    0,    // number of endpoints
    0,    // interface class
    0,    // interface sub class
    0,    // interface protocol
    0,    // interface string
];

static STR0_DESC: &[u8] = &[
    4,    // descriptor length
    3,    // type: string
    0x09, // lang id: english (us)
    0x04, //  "
];

static STR1_DESC: &[u8] = b"\x1a\x03H\0e\0l\0l\0o\0 \0W\0o\0r\0l\0d\0!\0";

fn process_urb(urb: &mut UrbWithData) {
    if ioctl::UrbType::Ctrl != urb.kind() {
        trace!("not CONTROL");
        return;
    }
    if !urb.endpoint().is_broadcast() {
        trace!("not endpoint 0");
        urb.set_status(Status::Stall);
        return;
    }

    let control_packet = urb.control_packet();
    let request_type = control_packet.request_type();
    let request = control_packet.request();

    match (request_type, request) {
        ((Dir::Out, CtrlType::Standard, Recipient::Device), Req::SetConfiguration) => {
            trace!("SET_CONFIGURATION");
            urb.set_status(Status::Success);
        }
        ((Dir::Out, CtrlType::Standard, Recipient::Device), Req::SetInterface) => {
            trace!("SET_INTERFACE");
            urb.set_status(Status::Success);
        }
        ((Dir::In, CtrlType::Standard, Recipient::Device), Req::GetDescriptor) => {
            trace!("GET_DESCRIPTOR");
            match control_packet.value() >> 8 {
                1 => {
                    trace!("DEVICE_DESCRIPTOR");
                    let length =
                        std::cmp::min(DEV_DESC[0] as usize, control_packet.length() as usize);
                    let new_len = urb
                        .available_transfer_mut()
                        .iter_mut()
                        .zip(&DEV_DESC[..length])
                        .fold(0, |acc, (left, &right)| {
                            left.write(right);
                            acc + 1
                        });
                    // SAFETY: Wrote less than the number of bytes remaining
                    //         in the buffer.
                    unsafe { urb.update_transfer_len(new_len) };
                    urb.set_status(Status::Success);
                }
                2 => {
                    trace!("CONFIGURATION_DESCRIPTOR");
                    let length = dbg!(std::cmp::min(
                        CONF_DESC[0] as usize + CONF_DESC[9] as usize,
                        control_packet.length() as usize
                    ));
                    let new_len = urb
                        .available_transfer_mut()
                        .iter_mut()
                        .zip(&CONF_DESC[..length])
                        .fold(0, |acc, (left, &right)| {
                            left.write(right);
                            acc + 1
                        });
                    // SAFETY: Wrote less than the number of bytes remaining
                    //         in the buffer.
                    unsafe { urb.update_transfer_len(new_len) };
                    urb.set_status(Status::Success);
                }
                3 => {
                    trace!("STRING_DESCRIPTOR");
                    match control_packet.value() & 0xff {
                        0 => {
                            let length = std::cmp::min(
                                STR0_DESC[0] as usize,
                                control_packet.length() as usize,
                            );
                            let new_len = urb
                                .available_transfer_mut()
                                .iter_mut()
                                .zip(&STR0_DESC[..length])
                                .fold(0, |acc, (left, &right)| {
                                    left.write(right);
                                    acc + 1
                                });
                            // SAFETY: Wrote less than the number of bytes remaining
                            //         in the buffer.
                            unsafe { urb.update_transfer_len(new_len) };
                            urb.set_status(Status::Success);
                        }
                        1 => {
                            let length = std::cmp::min(
                                STR1_DESC[0] as usize,
                                control_packet.length() as usize,
                            );
                            let new_len = urb
                                .available_transfer_mut()
                                .iter_mut()
                                .zip(&STR1_DESC[..length])
                                .fold(0, |acc, (left, &right)| {
                                    left.write(right);
                                    acc + 1
                                });
                            // SAFETY: Wrote less than the number of bytes remaining
                            //         in the buffer.
                            unsafe { urb.update_transfer_len(new_len) };
                            urb.set_status(Status::Success);
                        }
                        _ => urb.set_status(Status::Stall),
                    }
                }
                _ => urb.set_status(Status::Stall),
            }
        }
        _ => urb.set_status(Status::Stall),
    }
}

fn main() {
    env_logger::init();
    let num_ports = BoundedU8::new(1).unwrap();
    let mut vhci = Controller::open(num_ports).unwrap();
    let mut prev = ioctl::IocPortStat::default();
    let mut addr = 0xff;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        let dur = Duration::from_millis(500);
        let timeout = TimeoutMillis::from_duration(dur).unwrap();
        let work = match vhci.fetch_work_timeout(timeout) {
            Ok(work) => work,
            Err(err) if io::ErrorKind::TimedOut == err.kind() => continue,
            Err(err) => Err(err).unwrap(),
        };
        debug!("==============================================");

        // SAFETY: We don't alter the `typ field, which
        //         satisfies the safety constraints.
        match unsafe { work.into_inner() } {
            ioctl::Work::PortStat(next) => {
                debug!("got port stat");
                debug!("status: {:?}", next.status());
                debug!("change: {:?}", next.change());
                debug!("flags: {:?}", next.flags());
                if next.change().contains(PortChange::CONNECTION) {
                    trace!("CONNECTION state changed -> invalidating address");
                    addr = 0xff;
                }
                if next.change().contains(PortChange::RESET)
                    && (!next.status()).contains(PortStatus::RESET)
                    && next.status().contains(PortStatus::ENABLE)
                {
                    trace!("RESET successful -> use default address");
                    addr = 0;
                }
                if prev.status().contains(PortStatus::POWER)
                    && (!next.status()).contains(PortStatus::POWER)
                {
                    trace!("port is powered off");
                }
                if (!prev.status()).contains(PortStatus::POWER)
                    && next.status().contains(PortStatus::POWER)
                {
                    trace!("port is powered on -> connecting device");
                    vhci.port_connect(next.index(), DataRate::Full).unwrap();
                }
                if (!prev.status()).contains(PortStatus::RESET)
                    && next
                        .status()
                        .contains(PortStatus::RESET | PortStatus::CONNECTION)
                {
                    trace!("port is resetting -> completing reset");
                    vhci.port_reset_done(next.index(), true).unwrap();
                }
                if (!prev.flags()).contains(PortFlag::RESUMING)
                    && next.flags().contains(PortFlag::RESUMING)
                    && next.status().contains(PortStatus::CONNECTION)
                {
                    trace!("port is resuming -> completing resume");
                    vhci.port_resumed(next.index()).unwrap();
                }
                prev = next;
            }
            ioctl::Work::ProcessUrb((urb, handle)) => {
                debug!("got process urb");
                if urb.address.0 != addr {
                    trace!("not for {addr}, skipping");
                    continue;
                }

                let mut urb = UrbWithData::from_ioctl(urb, handle);
                if urb.needs_data_fetch() {
                    match vhci.fetch_data(&mut urb) {
                        Ok(_) => {}
                        Err(err)
                            if err
                                .raw_os_error()
                                .is_some_and(|errno| nix::libc::ECANCELED == errno) => {}
                        Err(err) => Err(err).unwrap(),
                    }
                }
                if ioctl::UrbType::Ctrl == urb.kind()
                    && urb.endpoint().is_broadcast()
                    && usbfs::CtrlType::Standard == urb.control_packet().control_type()
                    && usbfs::Dir::Out == urb.control_packet().direction()
                    && usbfs::Recipient::Device == urb.control_packet().recipient()
                    && ioctl::Req::SetAddress == urb.control_packet().b_request
                {
                    if 0x7f < urb.control_packet().w_value {
                        urb.set_status(Status::Stall);
                    } else {
                        urb.set_status(Status::Success);
                        addr = urb.control_packet().w_value.try_into().unwrap();
                        trace!("SET_ADDRESS (addr={:#x})", addr);
                    }
                } else {
                    process_urb(&mut urb);
                }

                vhci.giveback(urb).unwrap();
            }
            ioctl::Work::CancelUrb(handle) => {
                debug!("got cancel urb {handle:?}");
            }
        }
    }
}
