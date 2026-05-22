# Sound-Reactive LED Controller Spec

## Project Intent

Build a modular system that can control LEDs in response to ambient music.
The first milestone is a Rust library that converts audio samples into stable,
useful lighting control data. The controller hardware is intentionally undecided
at this stage.

The project should grow toward a bigger setup, but the first useful artifact is
an audio-to-light analysis library that can be consumed by a controller app or
firmware.

## Core Constraint

The sound analysis library must be written in Rust.

The library should avoid hard-coding assumptions about:

- Audio input source
- LED protocol
- Controller board
- Operating system
- Number of LED strips or pixels

Those choices belong in later adapter layers.

## Working Name

`yasts-sys`

Possible meaning: "Yet Another Sound-To-Strip System".

## Desired User Experience

A user should be able to put the LED setup into a music-reactive vibe mode.
When ambient music plays, the system should extract musical features such as
volume, bass energy, mid energy, treble energy, and beat-like pulses. Those
features should be translated into values a controller can use to update LED
colors, brightness, animation speed, or patterns.

## Suggested Architecture

```text
Audio source
  -> audio capture adapter
  -> Rust sound analysis library
  -> lighting state / animation data
  -> controller adapter
  -> LED driver adapter
  -> LED strip or matrix
```

The first implementation should focus only on this part:

```text
raw audio samples -> Rust sound analysis library -> lighting control data
```

## Library Scope

Create a Rust crate that accepts frames of audio samples and produces compact
analysis output suitable for LED control.

The library should provide:

- Audio frame ingestion
- RMS / loudness estimation
- Frequency-band energy extraction
- Basic beat or pulse detection
- Smoothing to avoid harsh flicker
- Normalized output values in the range `0.0..=1.0`
- A small, stable API that controller code can call repeatedly

The library should not provide, in its first version:

- Microphone setup
- USB audio setup
- LED strip protocol implementations
- Wi-Fi, Bluetooth, or HTTP control
- UI or mobile app behavior
- Hardware-specific pin control

## Controller Strategy

Do not choose the final controller in the first milestone. Design the library so
it can support multiple controller paths later.

Likely paths:

- Raspberry Pi or similar Linux SBC: easiest for microphone capture, Rust
  runtime, testing, and fast iteration.
- ESP32-class microcontroller: strong candidate for embedded LED control and
  wireless behavior, but audio capture and Rust support require more care.
- Arduino-class board: possible for LED control, but limited for real-time audio
  analysis unless paired with another processor or simplified analysis.

Initial recommendation for experimentation:

Use a desktop or Raspberry Pi-style Linux environment for the first proof of
concept, then port the analysis boundary to embedded hardware after the signal
processing behavior feels good.

## Rust Crate Requirements

The crate should be designed so it can eventually support embedded targets.

Implementation guidance:

- Prefer a small public API.
- Keep allocation low and predictable.
- Keep dependencies minimal.
- Separate signal processing from hardware IO.
- Consider a future `no_std` mode, but do not require it in the first milestone
  unless the selected controller demands it.
- Use `f32` audio samples normalized to `-1.0..=1.0` as the initial input type.
- Use explicit configuration structs instead of magic constants.

Potential crate name:

- `yasts_audio`

## Public API Sketch

```rust
pub struct Analyzer {
    // Internal state for smoothing, FFT windows, and beat detection.
}

pub struct AnalyzerConfig {
    pub sample_rate_hz: u32,
    pub frame_size: usize,
    pub smoothing: f32,
    pub beat_sensitivity: f32,
}

pub struct AudioFeatures {
    pub loudness: f32,
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub beat: bool,
    pub beat_strength: f32,
}

impl Analyzer {
    pub fn new(config: AnalyzerConfig) -> Self;

    pub fn process_frame(&mut self, samples: &[f32]) -> AudioFeatures;
}
```

All returned numeric values should be normalized to `0.0..=1.0` unless the API
explicitly documents otherwise.

## Lighting Data Model

The first library does not need to output final LED colors. It should output
features that animation code can consume.

Later, a separate crate or module can map features into lighting commands:

```rust
pub struct LightingFrame {
    pub brightness: f32,
    pub primary_hue: f32,
    pub secondary_hue: f32,
    pub pulse: f32,
    pub motion: f32,
}
```

Keep this separate from the audio analyzer so the same analysis data can drive
many visual styles.

## First Milestone

Create a Rust workspace with an audio analysis crate.

Minimum deliverables:

- `Cargo.toml` workspace
- `crates/yasts-audio/Cargo.toml`
- `crates/yasts-audio/src/lib.rs`
- `AnalyzerConfig`
- `Analyzer`
- `AudioFeatures`
- RMS loudness calculation
- Simple bass / mids / treble extraction
- Basic smoothing
- Unit tests for normalization and stable frame processing

Frequency extraction can start simple. If FFT support is added, prefer a
well-maintained Rust crate and keep it behind a clean internal abstraction.

## Suggested Implementation Phases

### Phase 1: Shape the Rust Library

- Create the workspace and audio crate.
- Define the public API.
- Implement loudness and smoothing.
- Add tests for empty frames, quiet frames, loud frames, and clipping behavior.

### Phase 2: Add Frequency Awareness

- Add FFT or filter-based band extraction.
- Produce bass, mids, and treble energy values.
- Add tests using generated sine waves.
- Document the expected frequency ranges.

Suggested initial bands:

- Bass: `20..250 Hz`
- Mids: `250..4000 Hz`
- Treble: `4000..12000 Hz`

### Phase 3: Add Beat-Like Pulse Detection

- Track recent energy history.
- Detect sudden energy increases.
- Expose `beat` and `beat_strength`.
- Add tests using synthetic pulse frames.

### Phase 4: Build a Local Demo

- Add a small CLI or example that reads generated audio or a file.
- Print `AudioFeatures` over time.
- Optionally render a terminal visualization.

### Phase 5: Pick a Controller Prototype

- If using Raspberry Pi, build a Rust binary that captures microphone input and
  sends output to an LED adapter.
- If using ESP32 or another embedded board, evaluate whether the analyzer needs
  `no_std`, fixed-size buffers, or a simplified algorithm.

### Phase 6: LED Adapter

- Add hardware-specific LED output outside the audio library.
- Keep LED protocol code isolated.
- Translate `AudioFeatures` or `LightingFrame` into LED strip updates.

## Acceptance Criteria For The First Agent Pass

An implementation agent should be able to finish the first pass when:

- The repository has a Rust workspace.
- The audio crate builds with `cargo test`.
- The public API matches the spirit of this spec.
- `Analyzer::process_frame` returns deterministic, normalized values.
- The library has tests for silence, loudness, smoothing, and invalid or unusual
  frame inputs.
- No hardware-specific code is required to run the tests.

## Non-Goals For The First Agent Pass

- Choosing the final controller board
- Building the physical LED wiring
- Implementing a full lighting animation engine
- Capturing microphone input in production
- Creating a mobile app or web UI
- Supporting every possible LED protocol

## Open Decisions

These should stay open until the library exists and can be tested:

- Controller board
- Microphone type
- LED strip type and protocol
- Power requirements
- Final physical layout
- Whether the analyzer must support `no_std`
- Whether the first demo should be terminal-based, web-based, or hardware-based

## Notes For Future Agents

- Keep the Rust audio analysis code independent from hardware.
- Do not introduce controller-specific dependencies into the audio crate.
- Prefer testable signal-processing behavior over impressive visuals in the
  first pass.
- Keep the public API boring, small, and stable.
- When in doubt, add a test with generated sample data.

