use std::collections::VecDeque;
use std::env;
use std::error::Error;
use std::f32::consts::PI;
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use yasts_audio::{Analyzer, AnalyzerConfig, AudioFeatures};

const FRAME_SIZE: usize = 1024;
const RENDER_INTERVAL: Duration = Duration::from_millis(66);
const DEFAULT_VISUAL_GAIN: f32 = 1.25;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = AppOptions::parse(&args)?;

    if options.help {
        print_help();
        return Ok(());
    }

    if options.generated {
        run_generated(options)
    } else {
        run_microphone(options)
    }
}

#[derive(Debug, Clone, Copy)]
struct AppOptions {
    generated: bool,
    chart: bool,
    help: bool,
    max_frames: Option<usize>,
    visual_gain: f32,
}

impl AppOptions {
    fn parse(args: &[String]) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            generated: args.iter().any(|arg| arg == "--generated"),
            chart: args.iter().any(|arg| arg == "--chart"),
            help: args.iter().any(|arg| arg == "--help" || arg == "-h"),
            max_frames: parse_optional_usize(args, "--frames")?,
            visual_gain: parse_optional_f32(args, "--gain")?.unwrap_or(DEFAULT_VISUAL_GAIN),
        })
    }
}

fn print_help() {
    println!("consumer-test");
    println!();
    println!("Usage:");
    println!("  cargo run                                  # use the default microphone");
    println!("  cargo run -- --chart                       # render adaptive ASCII chart");
    println!("  cargo run -- --generated                   # use synthetic audio instead");
    println!("  cargo run -- --generated --chart --frames 40");
    println!();
    println!("Options:");
    println!("  --generated     use synthetic audio instead of the microphone");
    println!("  --chart         render adaptive ASCII chart");
    println!("  --gain N        visual-only chart gain, default 1.25");
    println!("  --frames N      stop after rendering N frames");
}

fn parse_optional_usize(args: &[String], flag: &str) -> Result<Option<usize>, Box<dyn Error>> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };

    let value = args
        .get(index + 1)
        .ok_or_else(|| format!("{flag} requires a positive integer value"))?;
    let parsed = value.parse::<usize>()?;

    Ok(Some(parsed.max(1)))
}

fn parse_optional_f32(args: &[String], flag: &str) -> Result<Option<f32>, Box<dyn Error>> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };

    let value = args
        .get(index + 1)
        .ok_or_else(|| format!("{flag} requires a number"))?;
    let parsed = value.parse::<f32>()?;

    Ok(Some(parsed.max(0.0)))
}

fn run_microphone(options: AppOptions) -> Result<(), Box<dyn Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or(
        "No default input device found. On macOS, check System Settings -> Privacy & Security -> Microphone.",
    )?;
    let supported_config = device.default_input_config()?;
    let sample_rate_hz = supported_config.sample_rate().0;
    let channels = supported_config.channels() as usize;

    println!("Input device: {}", device.name()?);
    println!(
        "Input config: {} Hz, {} channel(s), {:?}",
        sample_rate_hz,
        channels,
        supported_config.sample_format()
    );
    println!("Listening. Press Ctrl+C to stop.");
    println!();

    let (features_tx, features_rx) = mpsc::channel();
    let config = supported_config.config();
    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => {
            build_input_stream::<f32>(&device, &config, features_tx, sample_rate_hz, channels)?
        }
        SampleFormat::I16 => {
            build_input_stream::<i16>(&device, &config, features_tx, sample_rate_hz, channels)?
        }
        SampleFormat::U16 => {
            build_input_stream::<u16>(&device, &config, features_tx, sample_rate_hz, channels)?
        }
        other => return Err(format!("Unsupported input sample format: {other:?}").into()),
    };

    stream.play().map_err(|err| {
        format!(
            "Could not start the microphone stream: {err}. On macOS, enable microphone access for Terminal or your IDE in System Settings -> Privacy & Security -> Microphone."
        )
    })?;

    render_loop(features_rx, options)
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    features_tx: Sender<AudioFeatures>,
    sample_rate_hz: u32,
    channels: usize,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::Sample + cpal::SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let mut analyzer = Analyzer::new(AnalyzerConfig {
        sample_rate_hz,
        frame_size: FRAME_SIZE,
        smoothing: 0.65,
        beat_sensitivity: 0.5,
    });
    let mut frame = Vec::with_capacity(FRAME_SIZE);

    let err_fn = |err| {
        eprintln!(
            "Microphone stream error: {err}. On macOS, check System Settings -> Privacy & Security -> Microphone."
        );
    };

    device.build_input_stream(
        config,
        move |data: &[T], _| {
            for chunk in data.chunks(channels.max(1)) {
                let mono = chunk
                    .iter()
                    .map(|sample| sample_to_f32(sample))
                    .sum::<f32>()
                    / chunk.len().max(1) as f32;
                frame.push(mono);

                if frame.len() == FRAME_SIZE {
                    let features = analyzer.process_frame(&frame);
                    let _ = features_tx.send(features);
                    frame.clear();
                }
            }
        },
        err_fn,
        None,
    )
}

fn sample_to_f32<T>(sample: &T) -> f32
where
    T: cpal::Sample,
    f32: cpal::FromSample<T>,
{
    (*sample).to_sample::<f32>().clamp(-1.0, 1.0)
}

fn run_generated(options: AppOptions) -> Result<(), Box<dyn Error>> {
    println!("Generated audio mode. Press Ctrl+C to stop.");
    println!();

    let sample_rate_hz = 44_100;
    let mut analyzer = Analyzer::new(AnalyzerConfig {
        sample_rate_hz,
        frame_size: FRAME_SIZE,
        smoothing: 0.65,
        beat_sensitivity: 0.0,
    });
    let mut frame_index = 0usize;
    let mut rendered = 0usize;
    let mut renderer = Renderer::new(options);

    loop {
        let samples = generated_frame(frame_index, sample_rate_hz);
        frame_index += 1;
        let features = analyzer.process_frame(&samples);
        renderer.render(features)?;
        rendered += 1;

        if options.max_frames.is_some_and(|limit| rendered >= limit) {
            return Ok(());
        }

        thread::sleep(RENDER_INTERVAL);
    }
}

fn generated_frame(frame_index: usize, sample_rate_hz: u32) -> Vec<f32> {
    let start_sample = frame_index * FRAME_SIZE;
    let cycle_position = frame_index % 24;
    let pulse = if (8..=9).contains(&cycle_position) {
        1.0
    } else {
        0.02
    };

    (0..FRAME_SIZE)
        .map(|offset| {
            let index = start_sample + offset;
            let time = index as f32 / sample_rate_hz as f32;

            let bass = (2.0 * PI * 120.0 * time).sin() * pulse;
            let mids = (2.0 * PI * 880.0 * time).sin() * 0.18;
            let treble = (2.0 * PI * 6_400.0 * time).sin() * 0.08;

            (bass + mids + treble).clamp(-1.0, 1.0)
        })
        .collect()
}

fn render_loop(
    features_rx: Receiver<AudioFeatures>,
    options: AppOptions,
) -> Result<(), Box<dyn Error>> {
    let mut last_render = Instant::now() - RENDER_INTERVAL;
    let mut rendered = 0usize;
    let mut renderer = Renderer::new(options);

    for features in features_rx {
        if last_render.elapsed() >= RENDER_INTERVAL {
            renderer.render(features)?;
            last_render = Instant::now();
            rendered += 1;

            if options.max_frames.is_some_and(|limit| rendered >= limit) {
                return Ok(());
            }
        }
    }

    Ok(())
}

enum Renderer {
    Simple,
    Chart(Box<ChartState>),
}

impl Renderer {
    fn new(options: AppOptions) -> Self {
        if options.chart {
            Self::Chart(Box::new(ChartState::new(ChartConfig {
                history_len: 120,
                min_peak: 0.02,
                visual_gain: options.visual_gain,
                smoothing: 0.70,
                bar_width: 20,
                history_width: 48,
            })))
        } else {
            Self::Simple
        }
    }

    fn render(&mut self, features: AudioFeatures) -> io::Result<()> {
        match self {
            Self::Simple => render_simple(features),
            Self::Chart(chart) => render_chart(chart.update(features), chart.config()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ChartConfig {
    history_len: usize,
    min_peak: f32,
    visual_gain: f32,
    smoothing: f32,
    bar_width: usize,
    history_width: usize,
}

#[derive(Debug)]
struct ChartState {
    loudness: TrackState,
    bass: TrackState,
    mids: TrackState,
    treble: TrackState,
    config: ChartConfig,
}

impl ChartState {
    fn new(config: ChartConfig) -> Self {
        Self {
            loudness: TrackState::new(config),
            bass: TrackState::new(config),
            mids: TrackState::new(config),
            treble: TrackState::new(config),
            config,
        }
    }

    fn config(&self) -> ChartConfig {
        self.config
    }

    fn update(&mut self, features: AudioFeatures) -> ChartFrame {
        ChartFrame {
            loudness: self.loudness.update(features.loudness),
            bass: self.bass.update(features.bass),
            mids: self.mids.update(features.mids),
            treble: self.treble.update(features.treble),
            beat: features.beat,
            beat_strength: features.beat_strength,
        }
    }
}

#[derive(Debug)]
struct TrackState {
    raw_history: VecDeque<f32>,
    display_history: VecDeque<f32>,
    display_value: f32,
    config: ChartConfig,
}

impl TrackState {
    fn new(config: ChartConfig) -> Self {
        Self {
            raw_history: VecDeque::with_capacity(config.history_len),
            display_history: VecDeque::with_capacity(config.history_len),
            display_value: 0.0,
            config,
        }
    }

    fn update(&mut self, raw: f32) -> ChartValue {
        let raw = raw.clamp(0.0, 1.0);
        push_limited(&mut self.raw_history, raw, self.config.history_len);

        let peak = self
            .raw_history
            .iter()
            .copied()
            .fold(self.config.min_peak, f32::max);
        let target_display = ((raw / peak) * self.config.visual_gain).clamp(0.0, 1.0);
        let hold = self.config.smoothing.clamp(0.0, 1.0);
        let follow = 1.0 - hold;

        self.display_value =
            ((self.display_value * hold) + (target_display * follow)).clamp(0.0, 1.0);
        push_limited(
            &mut self.display_history,
            self.display_value,
            self.config.history_len,
        );

        ChartValue {
            raw,
            display: self.display_value,
            history_line: history_line(&self.display_history, self.config.history_width),
        }
    }
}

#[derive(Debug, Clone)]
struct ChartFrame {
    loudness: ChartValue,
    bass: ChartValue,
    mids: ChartValue,
    treble: ChartValue,
    beat: bool,
    beat_strength: f32,
}

#[derive(Debug, Clone)]
struct ChartValue {
    raw: f32,
    display: f32,
    history_line: String,
}

fn push_limited(history: &mut VecDeque<f32>, value: f32, limit: usize) {
    history.push_back(value);
    while history.len() > limit {
        history.pop_front();
    }
}

fn render_simple(features: AudioFeatures) -> io::Result<()> {
    print!("\x1B[2J\x1B[H");
    println!("YASTS MacBook consumer test");
    println!();
    println!("LOUD  {} {:.2}", bar(features.loudness), features.loudness);
    println!("BASS  {} {:.2}", bar(features.bass), features.bass);
    println!("MIDS  {} {:.2}", bar(features.mids), features.mids);
    println!("HIGH  {} {:.2}", bar(features.treble), features.treble);
    println!(
        "BEAT  {} {:.2}",
        if features.beat { "*" } else { " " },
        features.beat_strength
    );
    println!();
    println!(
        "raw: loudness={:.2} bass={:.2} mids={:.2} treble={:.2} beat={} beat_strength={:.2}",
        features.loudness,
        features.bass,
        features.mids,
        features.treble,
        features.beat,
        features.beat_strength
    );
    println!();
    println!("Press Ctrl+C to stop.");
    io::stdout().flush()
}

fn render_chart(frame: ChartFrame, config: ChartConfig) -> io::Result<()> {
    print!("\x1B[2J\x1B[H");
    println!("YASTS ASCII CHART");
    println!();
    print_chart_row("LOUD", &frame.loudness, config.bar_width);
    print_chart_row("BASS", &frame.bass, config.bar_width);
    print_chart_row("MIDS", &frame.mids, config.bar_width);
    print_chart_row("HIGH", &frame.treble, config.bar_width);
    println!(
        "BEAT                  {} strength={:.2}",
        if frame.beat { "*" } else { " " },
        frame.beat_strength
    );
    println!();
    println!("history");
    println!("LOUD  {}", frame.loudness.history_line);
    println!("BASS  {}", frame.bass.history_line);
    println!("MIDS  {}", frame.mids.history_line);
    println!("HIGH  {}", frame.treble.history_line);
    println!();
    println!("Press Ctrl+C to stop.");
    io::stdout().flush()
}

fn print_chart_row(label: &str, value: &ChartValue, width: usize) {
    println!(
        "{label:<4} raw={:.2} display={:.2} |{}|",
        value.raw,
        value.display,
        chart_bar(value.display, width)
    );
}

fn bar(value: f32) -> String {
    let width = 20usize;
    let filled = (value.clamp(0.0, 1.0) * width as f32).round() as usize;
    let empty = width - filled;

    format!("{}{}", "#".repeat(filled), ".".repeat(empty))
}

fn chart_bar(value: f32, width: usize) -> String {
    let filled = (value.clamp(0.0, 1.0) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);

    format!("{}{}", "#".repeat(filled), "-".repeat(empty))
}

fn history_line(history: &VecDeque<f32>, width: usize) -> String {
    const RAMP: &[u8] = b" .:-=+*#%@";
    let mut line = String::with_capacity(width);
    let missing = width.saturating_sub(history.len());

    line.push_str(&".".repeat(missing));

    for value in history.iter().skip(history.len().saturating_sub(width)) {
        let index = (value.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32).round() as usize;
        line.push(RAMP[index] as char);
    }

    line
}
