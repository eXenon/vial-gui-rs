# vial-gui-rs

A Rust port of [Vial](https://get.vial.today/)'s `vial-gui` — the open-source,
cross-platform GUI for configuring QMK/Vial keyboards in real time.

The original `vial-gui` is a Python/PyQt5 application. This project rewrites it
in Rust so that it can be built as a small, self-contained native binary —
including for Windows.

## Frameworks

| Concern      | Choice                                              |
| ------------ | --------------------------------------------------- |
| GUI          | [`eframe`/`egui`](https://github.com/emilk/egui) (immediate-mode, pure Rust) |
| HID          | [`hidapi`](https://crates.io/crates/hidapi) (same C library as the original) |
| JSON         | [`serde`](https://serde.rs/) + `serde_json`        |
| Definition decompression | [`xz2`](https://crates.io/crates/xz2) (liblzma, matches Python's `lzma`) |
| HTTP         | [`ureq`](https://crates.io/crates/ureq) (downloads VIA definitions) |

All dependencies are cross-platform and compile natively on Windows, macOS, and
Linux.

## Building

The project pins its Rust toolchain with [mise](https://mise.jdx.dev):

```sh
mise install          # installs Rust 1.99.1 (and zig for cross-compilation)
```

### Native (Linux / macOS / Windows)

```sh
cargo build --release
# binary at target/release/vial-gui-rs (vial-gui-rs.exe on Windows)
```

On Windows, this uses the MSVC toolchain by default and produces
`target\release\vial-gui-rs.exe` with no extra configuration.

### Cross-compile from Linux/macOS to Windows

Two supported options:

**Option A — MinGW-w64** (requires `x86_64-w64-mingw32-gcc`):

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

**Option B — cargo-zigbuild** (self-contained, no system packages):

```sh
mise install zig            # or ensure `zig` is on PATH
cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

The linker for `x86_64-pc-windows-gnu` is pre-configured in `.cargo/config.toml`.

### GitHub Actions

A workflow (`.github/workflows/windows.yml`) builds Windows `.exe` artifacts on
every push/tag, so you don't need a Windows machine to produce release binaries.

## Status

This is an active work-in-progress. Implemented so far:

- [x] Full Vial/VIA raw-HID protocol (`protocol/`) — definition retrieval
      (LZMA/XZ decompressed), keymap read/write, encoders, layer count,
      macro/dynamic-entry counts, feature detection, lock/unlock/reset.
- [x] Complete keycode model (`keycodes/`) — protocol v5 and v6 code tables,
      serialization/deserialization, layer/macro/tap-dance/user/MIDI keycodes,
      and a keycode-expression evaluator (e.g. `LCTL(KC_A)`, `LT(1, KC_A)`).
- [x] KLE layout parser (`kle.rs`).
- [x] Device discovery (`device.rs`) — Vial serial magic, VIBL bootloader,
      VIA stack detection.
- [x] VIA definition stack download/parse (`via.rs`).
- [x] Keymap editor UI (`app.rs`, `ui/`) — device selection, keyboard
      rendering with hit-testing, layer switching, zoom, keycode tray, and
      free-form "Any keycode" entry.
- [x] RGB lighting management (`ui/rgb.rs`) — VialRGB (with per-layer
      settings on protocol v2), QMK rgblight (underglow), and QMK backlight.
- [x] Import/export of full configuration as `.vil2` JSON — keymap, encoders,
      layout options, and RGB (File → Export/Import layout, via native dialogs).

Not yet implemented (planned):

- [ ] Layout editor (per-layout option selection).
- [ ] Macros, Tap Dance, Combos, Key Overrides, Alt Repeat Key editors.
- [ ] QMK Settings and Matrix tester.
- [ ] Firmware flasher (bootloader).
- [ ] Secure unlock flow.
- [ ] Keyboard-layout (language) overrides and themes.

## The `.vil2` layout format

`File → Export layout` writes a pretty-printed JSON file with the `.vil2`
extension containing everything configured on the keyboard:

- `layout` — the full keymap (layers × rows × cols, integer keycodes)
- `encoder_layout` — per-encoder clockwise/counter-clockwise keycodes
- `layout_options` — the selected layout options
- `rgb` — the RGB config: VialRGB (per-layer configs on protocol v2, or the
  global config on v1), QMK rgblight underglow, and QMK backlight
- `uid`, `vial_protocol`, `via_protocol` — for cross-keyboard safety checks

`File → Import layout` reads a `.vil2` file, warns if it was saved from a
different keyboard (by `uid`), and applies it — writing the keymap, encoders,
layout options, and RGB config back to the device (and persisting RGB to EEPROM).

## License

GPL-2.0-or-later, matching the original `vial-gui`.
