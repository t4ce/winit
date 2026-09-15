# TRUEOS UI4 backend

This crate supplies Winit's `target_os = "trueos"` platform backend. It maps
top-level Winit windows to broker-owned UI4 visual frames.

The backend uses the versioned UI4 keyboard stream and preserves physical USB
HID usages, press/release state, modifiers, and device-loss resets. Window
visibility, hit testing, opacity, and title metadata use the versioned window
state ABI. Escape is delivered to the Winit application rather than closing a
frame in the host.

UI4 owns presentation. Raw-window-handle exposes a host-issued UI4 graphics
connection and a generation-bearing visual-frame ID; neither is an X11,
Wayland, or output handle. OpenGL and Glutin surface creation remain
unavailable until a TRUEOS graphics backend consumes those handles. UI4 stores
title metadata, but does not render title bars.

Run the backend checks without inheriting a parent Blueprint Cargo config:

```sh
cd /tmp
cargo test --manifest-path /home/t4ce/Repos/TRUEOS-Blueprints/vendor/winit/Cargo.toml -p winit-trueos
cargo check --manifest-path /home/t4ce/Repos/TRUEOS-Blueprints/vendor/winit/Cargo.toml -p winit-trueos
```

The full facade is selected for the Blueprint application target. The archived
TRUEOS standard library builds successfully. Use the Blueprint builder's
vendored `libc` overlay for this target. The raw check below still reaches the
archived toolchain's `restricted_std` gate in ordinary `std` consumers; it is
not a Winit backend error:

```sh
cd /tmp
RUSTC=/home/t4ce/Repos/TRUEOS-Rust-Toolchain-nightly-2026-07-10/bin/rustc \
  /home/t4ce/Repos/TRUEOS-Rust-Toolchain-nightly-2026-07-10/bin/cargo \
  -Zbuild-std=std,panic_abort -Zjson-target-spec check \
  --config 'patch.crates-io.libc.path="/home/t4ce/Repos/TRUEOS-Blueprints/vendor/libc-0.2.186"' \
  --manifest-path /home/t4ce/Repos/TRUEOS-Blueprints/vendor/winit/Cargo.toml \
  -p winit --no-default-features --features serde \
  --target /home/t4ce/Repos/TRUEOS-Blueprints/apps/target.json
```

This backend targets Winit 0.31. It does not make applications built against
the distinct Winit 0.30 API, including the current Alacritty checkout, source
compatible by itself.
