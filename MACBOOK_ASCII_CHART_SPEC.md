# MacBook Consumer ASCII Chart Spec

## Purpose

Add an ASCII-only chart mode to the MacBook consumer app so microphone input can
be seen as a live, growing visual signal in the terminal.

The current raw analyzer values can be small, often around `0.00..0.04` for
`bass`, `mids`, and `treble`. The chart should make those small changes visible
without pretending the raw values are larger than they are.

## Core Idea

Keep two values for each feature:

- Raw value: the actual `AudioFeatures` value from `yasts-audio`.
- Display value: a visually amplified value used only for terminal rendering.

The app should always preserve and optionally show the raw values. The ASCII
chart can use display scaling, adaptive gain, and history to make quiet signals
readable.

## Non-Goal

Do not change the `yasts-audio` library for this feature.

This feature belongs in the consumer app. It is a visualization layer, not a
signal-processing change.

## Suggested Command

Add a chart mode flag:

```sh
cargo run -- --chart
```

Generated test mode should also support it:

```sh
cargo run -- --generated --chart
```

For smoke tests:

```sh
cargo run -- --generated --chart --frames 40
```

## Visual Design

Use ASCII characters only.

Suggested display:

```text
YASTS ASCII CHART

LOUD raw=0.03 display=0.61 |############--------|
BASS raw=0.01 display=0.35 |#######-------------|
MIDS raw=0.02 display=0.46 |#########-----------|
HIGH raw=0.00 display=0.08 |##------------------|
BEAT                  *

history
LOUD  ..::--==++***###%%@
BASS  ....::---====++....
MIDS  .......::::---==++.
HIGH  ...................
```

Allowed character ramp:

```text
 .:-=+*#%@
```

This ramp is ordered from quiet to loud. The space character is allowed for the
lowest level, but dots are easier to see in logs.

## Display Scaling

The chart should support adaptive visual scaling.

Each feature should track a recent peak value:

```text
display = raw / recent_peak
```

Then clamp:

```text
display = clamp(display, 0.0, 1.0)
```

This means a `bass` value of `0.03` can appear as a strong bar if recent bass
values are also small. The raw value must still be printed so the user knows the
real analyzer output.

## Peak Tracking

Use a short rolling history per feature.

Suggested history length:

```text
120 frames
```

At around 15 frames per second, that represents about 8 seconds.

For each feature:

- Store recent raw values in a fixed-size buffer.
- Compute the recent peak from that buffer.
- Use a minimum peak floor to avoid division by zero.

Suggested minimum peak:

```text
0.02
```

Example:

```text
recent_peak = max(max(history), 0.02)
display = raw / recent_peak
```

## Visual Gain

Support a configurable visual gain multiplier:

```text
display = (raw / recent_peak) * visual_gain
```

Suggested default:

```text
visual_gain = 1.25
```

Clamp after gain:

```text
display = clamp(display, 0.0, 1.0)
```

Add an optional CLI flag:

```sh
cargo run -- --chart --gain 1.5
```

If no `--gain` is provided, use the default.

## Temporal Smoothing

The chart should smooth display values to reduce flicker:

```text
smoothed_display = previous_display * hold + display * follow
```

Suggested values:

```text
hold = 0.70
follow = 0.30
```

Beat markers should not be smoothed. They should react immediately.

## Chart State

Create a visualization state type in the consumer app:

```rust
struct ChartState {
    loudness: TrackState,
    bass: TrackState,
    mids: TrackState,
    treble: TrackState,
    visual_gain: f32,
}

struct TrackState {
    history: VecDeque<f32>,
    display_value: f32,
}
```

The state should be owned by the render loop, not the audio callback.

## API Sketch

```rust
impl ChartState {
    fn new(config: ChartConfig) -> Self;

    fn update(&mut self, features: AudioFeatures) -> ChartFrame;
}

struct ChartConfig {
    history_len: usize,
    min_peak: f32,
    visual_gain: f32,
    smoothing: f32,
    bar_width: usize,
}

struct ChartFrame {
    loudness: ChartValue,
    bass: ChartValue,
    mids: ChartValue,
    treble: ChartValue,
    beat: bool,
    beat_strength: f32,
}

struct ChartValue {
    raw: f32,
    display: f32,
    history_line: String,
}
```

## Renderer Behavior

The renderer should:

- Clear the terminal and redraw in-place.
- Show raw and display values.
- Show horizontal bars.
- Show compact history lines.
- Show `*` when `beat=true`.
- Keep all output ASCII-only.
- Avoid printing unbounded lines.

Suggested widths:

```text
bar_width = 20
history_width = 48
```

## Bar Rendering

For a display value:

```text
filled = round(display * bar_width)
empty = bar_width - filled
```

Render:

```text
|########------------|
```

Use:

- `#` for filled
- `-` for empty

## History Rendering

Use the character ramp to render recent display values:

```text
 .:-=+*#%@
```

For each value:

```text
index = round(value * (ramp.len() - 1))
char = ramp[index]
```

The history line should show the most recent values from left to right.

If there are fewer values than `history_width`, left-pad with dots or spaces.

## CLI Flags

Add:

```text
--chart
--gain <number>
```

Existing flags should continue to work:

```text
--generated
--frames <number>
--help
```

Help output should explain:

```text
--chart       render adaptive ASCII chart
--gain N      visual-only chart gain, default 1.25
```

## Default Mode

If `--chart` is not passed, the app may keep the current simple bar renderer.

Recommended behavior:

- Existing simple renderer remains the default.
- `--chart` enables the adaptive chart.

This keeps the feature easy to compare against raw behavior.

## Acceptance Criteria

The feature is complete when:

- `cargo run -- --generated --chart --frames 40` exits successfully.
- The chart shows raw values and visually amplified display values.
- Small raw values like `0.01..0.04` produce visible chart movement.
- The chart uses ASCII-only characters.
- `--gain` changes only the display scale, not raw values.
- `--chart` works with microphone mode.
- The existing non-chart renderer still works.
- `cargo check` passes in `consumer_test`.
- `cargo clippy --all-targets -- -D warnings` passes in `consumer_test`.

## Implementation Steps

1. Add CLI parsing for `--chart` and `--gain`.
2. Add `ChartConfig`, `ChartState`, `TrackState`, `ChartFrame`, and
   `ChartValue`.
3. Move rendering choice into the render loop.
4. Keep analyzer output unchanged.
5. Implement adaptive peak scaling.
6. Implement display smoothing.
7. Implement ASCII bar rendering.
8. Implement ASCII history rendering.
9. Add generated-mode smoke command with `--chart --frames`.
10. Run formatting, check, clippy, and generated smoke test.

## Notes

This feature is for human feedback while tuning the library. It should make the
terminal feel alive even before the analyzer is fully calibrated.

Later, the same display scaling idea can inspire LED mapping, but LED mapping
should live in a separate layer.

