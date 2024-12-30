use log::trace;

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

static STR1_DESC: &[u8] = b"\x1a\x03H\0e\0l\0l\0o\0 \0W\0o\0r\0l\0d\0!";

fn main() {
    println!("Hello world!")
}
