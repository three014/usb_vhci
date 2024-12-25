use num_enum::TryFromPrimitive;

/// Transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum Direction {
    /// Host to device.
    Out = 0,

    /// Device to host.
    In = 1,
}

/// Specification defining the request
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum ControlType {
    /// Request defined by the USB standard.
    Standard = 0,

    /// Request defined by the standard USB class specification.
    Class = 1,

    /// Non-standard request.
    Vendor = 2,
}

/// Entity targeted by the request.
#[derive(Debug,Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum Recipient {
    /// Request made to device as a whole.
    Device = 0,

    /// Request made to specific interface.
    Interface = 1,

    /// Request made to specific endpoint.
    Endpoint = 2,

    /// Other request.
    Other = 3,
}
