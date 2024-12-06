# `usb-vhci`, my latest venture into the USB/IP Project

GOAL: Develop a Rust crate that can interface with `usb-vhci-hcd`
and `usb-vhci-iocifc`, kernel modules that allow userspace to
define and use fake USB devices.

---

Last time I messed around with the USB/IP project, I 
wrote a [Rust crate](https://github.com/three014/usbip-core) 
that went unfinished due to changes in
interest and a lack of time (school). The cool thing I almost
accomplished in that project was that I combined the userspace parts
of the [Linux](https://github.com/torvalds/linux/tree/master/tools/usb/usbip)
and [Windows](https://github.com/vadimgrn/usbip-win2) 
USB/IP libraries into one library and
wrote a [neat helper library](https://github.com/three014/win-deviceioctl) 
for interacting with the Windows
DeviceIoControl system calls. It also taught me a lot about how to
interface with SysFs in Linux, and so I found a lot of success in
that project, despite never finishing it.

However, the reason for doing this whole thing was because I 
wanted better USB support in the Sunshine/Moonlight cloud gaming
stack, and last time I checked not much has changed there. Not
that there was much that actually needed to be changed; I admit
that wanting to use a Guitar Hero controller on a remote
desktop is a somewhat niche reason for wanting to learn about USB/IP.

I'm currently writing a new program that's essentially the 
USB/IP project but with the QUIC transport protocol instead
of TCP. I believe the USB/IP project could benefit from 1. QUIC's
control flow methods and 2. Moving a lot of its code into userspace,
[something that was mentioned](https://github.com/torvalds/linux/blob/master/drivers/usb/usbip/vhci_hcd.c#L24C1-L24C43) 
in the kernel module for USB/IP.
