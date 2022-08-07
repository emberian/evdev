# Changelog

## evdev next
[c0bd8dd...HEAD](https://github.com/emberian/evdev/compare/c0bd8dd...HEAD)

### Added

- Document `FFEffect`.
- Publicly export `FFEffect` from root.
- Add `FFEffect::id()` as an accessor for the effect ID.
- Add missing `EventStream::device_mut()` in `sync_stream.rs`.

### Changed

### Fixed

- Make sure that the `DevNodesBlocking` iterator is not blocking indefinitely when all entries in the directory have been exhausted.

## evdev 0.11.6 (2022-08-03)
[372d000...c0bd8dd](https://github.com/emberian/evdev/compare/372d000...c0bd8dd)

### Added

- Add a `CHANGELOG.md` with a changelog for each new release.
- Force feedback support [#74](https://github.com/emberian/evdev/pull/74).
- Implement serde support for `evdev_enum!` types and `InputEventKind` [#76](https://github.com/emberian/evdev/pull/76).
- Implement `VirtualDevice::get_sys_path()` as well as an iterator over the device node paths for virtual devices [#72](https://github.com/emberian/evdev/pull/72).
- Implement an `Error` type [#75](https://github.com/emberian/evdev/pull/75).
- Add `EventStream::device_mut()` to get a mutable reference to `RawDevice` [#73](https://github.com/emberian/evdev/pull/73).
- Add support for absolute axes for virtual devices [#71](https://github.com/emberian/evdev/pull/71).

### Changed

### Fixed

- Documentation and code tidying [#67](https://github.com/emberian/evdev/pull/67).

## evdev 0.11.5 (2022-03-05)
[099b6e9...372d000](https://github.com/emberian/evdev/compare/099b6e9...372d000)

### Added

- Introduce `RawDevice::sys_path` and `Device::sys_path` [#62](https://github.com/emberian/evdev/pull/62).
- Implement `FromIterator` for `AttributeSet`.

### Changed

### Fixed

## evdev 0.11.4 (2022-01-12)
[1d020f1...099b6e9](https://github.com/emberian/evdev/compare/1d020f1...099b6e9)

### Added

### Changed

- Update bitvec to 1.0.

### Fixed

## evdev 0.11.3 (2021-12-07)
[898bb5c...1d020f1](https://github.com/emberian/evdev/compare/898bb5c...1d020f1)

### Added

- Introduce `RawDevice::send_event` and `Device::send_event` to toggle LEDs, play sounds and play force feedback effects [#60](https://github.com/emberian/evdev/pull/60).

### Changed

### Fixed

- Fix a bug in `compensate_events` where it returned the same event when invoking `next()` multiple times [#61](https://github.com/emberian/evdev/pull/61).

## evdev 0.11.2 (2021-12-03)
[763ef01...898bb5c](https://github.com/emberian/evdev/compare/763ef01...898bb5c)

### Added

### Changed

- Update bitvec to 1.0.0-rc1.

### Fixed

## evdev 0.11.1 (2021-10-08)
[1898f49...763ef01](https://github.com/emberian/evdev/compare/1898f49...763ef01)

### Added

- Implement `Device::grab` and `Device::ungrab`
- Implement `VirtualDeviceBuilder::with_switches`.
- Support autorepeats and getting keymap entries.

### Changed

- Update nix to 0.23.

### Fixed

## evdev 0.11.0 (2021-04-01)
[79b6c2b...1898f49](https://github.com/emberian/evdev/compare/79b6c2b...1898f49)
