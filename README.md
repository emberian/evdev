`evdev`
=======

[![Travis](https://img.shields.io/travis/cmr/evdev.svg?style=flat-square)](https://travis-ci.org/cmr/ioctl)
[![Crates.io](https://img.shields.io/crates/v/evdev.svg?style=flat-square)](https://crates.io/crates/ioctl)

[Documentation](https://cmr.github.io/evdev)

Nice(r) access to `evdev`. Works on Rust >= 1.2.0.

What is `evdev`?
===================

`evdev` is the Linux kernel's generic input interface.

What does this library support?
===============================

This library exposes raw evdev events, but uses the Rust `Iterator` trait to
do so, and will handle `SYN_DROPPED` events properly for the client. I try to
match [libevdev](http://www.freedesktop.org/software/libevdev/doc/latest/)
closely, where possible.

Writing to devices is not yet supported (eg, turning LEDs on).

Example
=======

See <examples/evtest.rs> for an example of using this library (which roughly
corresponds to the userspace [evtest](http://cgit.freedesktop.org/evtest/)
tool.
