# MacBook Consumer Demo Spec

## Purpose

Create a separate project that consumes the `yasts-audio` Rust library and tests
the sound-reactive analysis flow using a MacBook microphone.

This project should prove that the library can process real ambient audio before
the LED controller hardware is chosen.

## Relationship To This Repository

This repository owns the reusable library:

```text
yasts-sys
  crates/yasts-audio
```

The new project should be a consumer app, not another library implementation.
It should import `yasts-audio`, capture microphone input, convert the input into
normalized `f32` frames, call `Analyzer::process_frame`, and display the
resulting features.

## Recommended Consumer Project Shape

Suggested project name:

```text
yasts-mac-vibe
```

Suggested app type for the first pass:

```text
Rust CLI terminal visualizer
```

The first consumer should stay simple:

- Capture audio from the default MacBook microphone.
- Feed audio frames to `yasts_audio::Analyzer`.
- Print or render `AudioFeatures` in the terminal.
- Avoid LED hardware, web UI, or controller board logic.

## High-Level Flow

```text
MacBook microphone
  -> audio capture crate
  -> sample conversion to f32 -1.0..=1.0
  -> frame buffer
  -> yasts_audio::Analyzer
  -> terminal output / visualization
```

## Library Consumption Options

### Option 1: Local Path Dependency

Use this while developing both projects on the same machine.

```toml
[dependencies]
yasts-audio = { path = "../yasts-sys/crates/yasts-audio" }
```

Pros:

- Fastest setup.
- No publish step required.
- Changes to the library are immediately testable.

Cons:

- The consumer project depends on a local folder layout.

### Option 2: Git Dependency

Use this after the library is pushed to GitHub.

```toml
[dependencies]
yasts-audio = { git = "https://github.com/jdmejiav/yasts-sys", package = "yasts-audio" }
```

Pros:

- Works outside the local machine.
- No crates.io publishing required.

Cons:

- Versioning is less clean than crates.io.
- Consumers depend on repository availability and selected branches or commits.

### Option 3: crates.io Published Dependency

Use this when the library API is stable enough for public reuse.

```toml
[dependencies]
yasts-audio = "0.1"
```

Pros:

- Cleanest dependency experience.
- Standard Rust package workflow.

Cons:

- Requires package metadata, ownership, and publishing.
- Published versions are permanent.

## Recommended Path For This Stage

Start with a local path dependency.

The library does not need to be published before the MacBook demo exists. The
best order is:

1. Compile and test `yasts-audio`.
2. Create the MacBook consumer app with a path dependency.
3. Validate microphone capture and feature output locally.
4. Push the library repository to GitHub.
5. Switch the consumer to a Git dependency if desired.
6. Publish to crates.io only after the API feels stable.

## MacBook Audio Capture

Use a cross-platform Rust audio input crate in the consumer project.

Recommended first choice:

```toml
cpal = "0.15"
```

The consumer app should:

- Open the default input device.
- Read the default input config.
- Support `f32`, `i16`, and `u16` input sample formats.
- Convert all samples to normalized `f32` values in `-1.0..=1.0`.
- Mix down multiple channels to mono.
- Accumulate samples into fixed-size frames.
- Call `Analyzer::process_frame` for each full frame.

Suggested analyzer defaults:

```rust
let config = AnalyzerConfig {
    sample_rate_hz,
    frame_size: 1024,
    smoothing: 0.65,
    beat_sensitivity: 0.5,
};
```

## macOS Permissions

The first time the app opens the microphone, macOS may request microphone
permission for the terminal app or IDE running the binary.

If capture fails:

- Check `System Settings -> Privacy & Security -> Microphone`.
- Enable access for the terminal or IDE.
- Restart the terminal after permission changes.

## Terminal Output

Minimum output:

```text
loudness=0.42 bass=0.36 mids=0.18 treble=0.09 beat=false beat_strength=0.00
```

Preferred first visualization:

```text
LOUD  ########..
BASS  ######....
MIDS  ###.......
HIGH  ##........
BEAT  *
```

The visualization should update in-place or at a steady interval so it feels
live.

## Consumer App Non-Goals

- Do not reimplement audio analysis in the consumer.
- Do not add LED hardware control yet.
- Do not choose Raspberry Pi, Arduino, or ESP32 yet.
- Do not create a web UI in the first pass.
- Do not publish the library just to test locally.

## Acceptance Criteria

The MacBook consumer project is successful when:

- `cargo run` starts microphone capture.
- The app requests or uses microphone permission on macOS.
- Ambient sound changes visible feature values.
- Bass-heavy music raises `bass`.
- Speech or vocals tend to raise `mids`.
- Sharp high-frequency sounds tend to raise `treble`.
- Loud pulses sometimes set `beat=true`.
- The consumer imports `yasts-audio` instead of duplicating analyzer logic.

## Future Direction

After the MacBook demo works, the next projects can be:

- A Raspberry Pi prototype that captures microphone input and controls LEDs.
- A terminal or desktop visualizer for tuning analyzer behavior.
- A lighting mapper crate that converts `AudioFeatures` into colors and motion.
- A hardware adapter crate for a specific LED protocol.
