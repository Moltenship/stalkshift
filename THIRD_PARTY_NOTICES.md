# Third-party notices

The small ABI declarations in `crates/stalkshift-plugin/src/ffi.rs` and related SDK constants/callback usage are adapted from SCS SDK 1.14, Copyright (C) 2016 SCS Software. Its permission notice is preserved in [third-party/scs-sdk-LICENSE.txt](third-party/scs-sdk-LICENSE.txt) and must accompany distributed plugin binaries.

Official source: https://download.eurotrucksimulator2.com/scs_sdk_1_14.zip

Referenced files: `scssdk.h`, `scssdk_value.h`, `scssdk_input.h`, `scssdk_input_device.h`, `scssdk_input_event.h`, `scssdk_telemetry.h`, `scssdk_telemetry_event.h`, `scssdk_telemetry_channel.h` and the logical indicator channel declarations. The project uses its own minimal Rust ABI layer, not the community `scs-sdk-crates` dependency.

Cargo dependencies retain their respective licenses; exact versions are recorded in `Cargo.lock`. Release archives include the dependency inventory in `third-party/dependencies.json` and copyright and permission notices under `third-party/dependencies/`. StalkShift's original code is MIT licensed.
