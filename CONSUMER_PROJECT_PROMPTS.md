# Consumer Project Prompt Plan

Use this file as a step-by-step prompt guide for building a separate project
that consumes `yasts-audio`.

The goal is to stay focused: first prove microphone-to-feature processing on a
MacBook, then decide whether the library should be consumed locally, from Git,
or from crates.io.

## Current Library Status

The library lives here:

```text
../yasts-sys/crates/yasts-audio
```

The current crate name is:

```text
yasts-audio
```

The Rust import name is:

```rust
use yasts_audio::{Analyzer, AnalyzerConfig};
```

Before building the consumer, verify the library:

```sh
cd ../yasts-sys
cargo test
cargo clippy --all-targets -- -D warnings
```

## Step 1: Create The Consumer Project

Prompt:

```text
Create a new Rust CLI project named yasts-mac-vibe outside the yasts-sys repo.
It should consume the yasts-audio crate using a local path dependency:
../yasts-sys/crates/yasts-audio.
Keep the project small and focused on MacBook microphone input and terminal
feature output.
```

Expected result:

```text
yasts-mac-vibe/
  Cargo.toml
  src/main.rs
```

Expected dependency:

```toml
[dependencies]
yasts-audio = { path = "../yasts-sys/crates/yasts-audio" }
```

## Step 2: Add Microphone Capture

Prompt:

```text
Add microphone capture to yasts-mac-vibe using the cpal crate. Use the default
input device and default input config. Support f32, i16, and u16 sample formats.
Convert all input samples into normalized f32 values in -1.0..=1.0.
Do not perform audio analysis in this project yet; only prove that microphone
samples are being received.
```

Expected dependency:

```toml
cpal = "0.15"
```

Expected behavior:

```text
cargo run
```

The app should print that an input device was found and show sample activity.

## Step 3: Buffer Audio Frames

Prompt:

```text
Update yasts-mac-vibe to collect microphone samples into fixed-size mono frames.
Use a frame size of 1024 samples. If the microphone has multiple channels, mix
them down to mono by averaging the channels. Keep audio capture and frame
buffering separate from analyzer usage.
```

Expected behavior:

- The app builds complete frames of normalized mono `f32` samples.
- The app does not panic when callback chunks are smaller or larger than one
  frame.

## Step 4: Consume yasts-audio

Prompt:

```text
Wire yasts_audio::Analyzer into yasts-mac-vibe. Create an AnalyzerConfig using
the microphone sample rate, frame_size 1024, smoothing 0.65, and
beat_sensitivity 0.5. For every full frame, call process_frame and print the
AudioFeatures values.
```

Expected output:

```text
loudness=0.12 bass=0.05 mids=0.18 treble=0.02 beat=false beat_strength=0.00
```

## Step 5: Make The Output Feel Live

Prompt:

```text
Replace raw repeated println output with a compact terminal visualization.
Show loudness, bass, mids, treble, and beat strength as bars. Show a beat marker
when beat is true. Keep the app terminal-only.
```

Expected output:

```text
LOUD  #####.....
BASS  ###.......
MIDS  ######....
HIGH  ##........
BEAT
```

## Step 6: Handle macOS Permission Problems

Prompt:

```text
Add clear startup and error messages for macOS microphone permission issues.
If no input device is available or stream creation fails, explain that the user
may need to enable microphone access for Terminal or the IDE in System Settings
under Privacy & Security -> Microphone.
```

Expected behavior:

- Errors are readable.
- The app does not fail silently.

## Step 7: Add A Generated-Audio Fallback

Prompt:

```text
Add an optional generated-audio mode for yasts-mac-vibe. When run with
--generated, do not open the microphone. Instead generate synthetic sine waves
and pulses, feed them to yasts_audio::Analyzer, and render the same terminal
visualization. This mode should help test the consumer even when microphone
permission is unavailable.
```

Expected behavior:

```sh
cargo run -- --generated
```

The app should show changing features without using the microphone.

## Step 8: Decide How To Consume The Library

Prompt:

```text
Review the current yasts-mac-vibe dependency on yasts-audio. Explain whether we
should keep using a local path dependency, switch to a Git dependency, or publish
to crates.io. Recommend the next step based on whether the library API feels
stable.
```

Decision guide:

- Local path dependency: best while both projects are changing quickly.
- Git dependency: best after `yasts-sys` is pushed and the consumer should work
  on another machine.
- crates.io: best only after the API is stable enough to version publicly.

## Step 9: Prepare The Library For Publishing

Do this in `yasts-sys`, not the consumer project.

Prompt:

```text
Prepare yasts-audio for a possible crates.io publish. Check Cargo.toml metadata,
crate docs, public API docs, version, license, README strategy, and package
contents. Do not publish yet. Run cargo test, cargo clippy, and cargo package.
Report anything that should be fixed before publishing.
```

Commands:

```sh
cd ../yasts-sys
cargo test
cargo clippy --all-targets -- -D warnings
cargo package -p yasts-audio
```

Notes:

- `cargo package` builds the package that would be uploaded.
- It does not publish the crate.
- Publishing requires a crates.io account and API token.

## Step 10: Publish Only When Ready

Prompt:

```text
Publish yasts-audio to crates.io only after confirming the package metadata,
API, docs, and version are ready. Before publishing, explain that published
crate versions are permanent and ask for confirmation.
```

Command:

```sh
cargo publish -p yasts-audio
```

Important:

- Do not publish casually.
- Published crate versions cannot be overwritten.
- If a mistake is published, the fix is a new version.

## Step 11: Switch The Consumer To The Published Crate

Prompt:

```text
Update yasts-mac-vibe to consume yasts-audio from crates.io instead of a local
path. Use the published version. Run cargo update, cargo test, and cargo run to
confirm the consumer still works.
```

Expected dependency:

```toml
[dependencies]
yasts-audio = "0.1"
```

## Practical Recommendation

For now, do not publish.

Use this order:

1. Keep `yasts-audio` as a local path dependency.
2. Build the MacBook microphone demo.
3. Tune analyzer behavior using real music and room audio.
4. Push `yasts-sys` to GitHub.
5. Optionally switch the consumer to a Git dependency.
6. Publish to crates.io once the API has survived a real consumer project.
