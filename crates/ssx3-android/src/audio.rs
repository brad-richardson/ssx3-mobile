//! Android-owned output stream. The engine produces 48 kHz stereo i16 into
//! its public pull ring; cpal owns the platform callback on this thread.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ssx3_engine::ssx3_iop::audio;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

enum Command {
    Active(bool),
    Quit,
}

pub struct AudioOwner {
    commands: mpsc::Sender<Command>,
    callbacks: Arc<AtomicU64>,
    underrun: Arc<AtomicU64>,
}

impl AudioOwner {
    pub fn new() -> Self {
        let (commands, rx) = mpsc::channel();
        let callbacks = Arc::new(AtomicU64::new(0));
        let underrun = Arc::new(AtomicU64::new(0));
        let cb = callbacks.clone();
        let gaps = underrun.clone();
        std::thread::Builder::new()
            .name("ssx3-mobile-audio".into())
            .spawn(move || owner(rx, cb, gaps))
            .expect("audio owner thread");
        Self {
            commands,
            callbacks,
            underrun,
        }
    }

    pub fn request(&self, active: bool) {
        let _ = self.commands.send(Command::Active(active));
    }

    pub fn counters(&self) -> (u64, u64) {
        (
            self.callbacks.load(Ordering::Relaxed),
            self.underrun.load(Ordering::Relaxed),
        )
    }
}

impl Drop for AudioOwner {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Quit);
    }
}

fn owner(rx: mpsc::Receiver<Command>, callbacks: Arc<AtomicU64>, underrun: Arc<AtomicU64>) {
    let failed = Arc::new(AtomicBool::new(false));
    let mut stream: Option<cpal::Stream> = None;
    let mut active = false;
    let mut retry_at = Instant::now();
    let mut backoff = Duration::from_millis(500);
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Command::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Ok(Command::Active(value)) => {
                if active != value {
                    active = value;
                    stream = None; // Drop closes the Android stream on background.
                    retry_at = Instant::now();
                    backoff = Duration::from_millis(500);
                    failed.store(false, Ordering::Release);
                    eprintln!("[mobile-audio] active={active}");
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !active {
            continue;
        }
        if failed.swap(false, Ordering::AcqRel) {
            stream = None;
            retry_at = Instant::now() + backoff;
            eprintln!(
                "[mobile-audio] stream error; reopening after {} ms",
                backoff.as_millis()
            );
            backoff = (backoff * 2).min(Duration::from_secs(8));
        }
        if stream.is_none() && Instant::now() >= retry_at {
            match open_stream(failed.clone(), callbacks.clone(), underrun.clone()) {
                Ok(s) => {
                    stream = Some(s);
                    backoff = Duration::from_millis(500);
                }
                Err(e) => {
                    eprintln!("[mobile-audio] open failed: {e}");
                    retry_at = Instant::now() + backoff;
                    backoff = (backoff * 2).min(Duration::from_secs(8));
                }
            }
        }
    }
}

fn open_stream(
    failed: Arc<AtomicBool>,
    callbacks: Arc<AtomicU64>,
    underrun: Arc<AtomicU64>,
) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no default output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let channels = supported.channels() as usize;
    let device_rate = supported.sample_rate();
    let config = cpal::StreamConfig {
        channels: supported.channels(),
        sample_rate: device_rate,
        buffer_size: cpal::BufferSize::Default,
    };
    eprintln!(
        "[mobile-audio] open {} Hz {} channels {:?}",
        device_rate,
        channels,
        supported.sample_format()
    );

    let mut samples: VecDeque<[i16; 2]> = VecDeque::with_capacity(2048);
    let mut scratch = [0i16; 2048];
    let mut current = [0i16; 2];
    let mut next = [0i16; 2];
    let mut phase = 0.0f64;
    let step = audio::RATE as f64 / device_rate as f64;
    let mut fill = move |out: &mut dyn FnMut(usize, f32), frames: usize| {
        let needed = (frames as f64 * step).ceil() as usize + 2;
        while samples.len() < needed {
            let n = audio::pull_into(&mut scratch);
            if n == 0 {
                break;
            }
            for stereo in scratch[..n * 2].chunks_exact(2) {
                samples.push_back([stereo[0], stereo[1]]);
            }
        }
        for frame in 0..frames {
            phase += step;
            while phase >= 1.0 {
                phase -= 1.0;
                current = next;
                match samples.pop_front() {
                    Some(v) => next = v,
                    None => {
                        next = [0; 2];
                        underrun.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            let t = phase as f32;
            for ch in 0..channels {
                let side = ch.min(1);
                let v = current[side] as f32 * (1.0 - t) + next[side] as f32 * t;
                out(frame * channels + ch, v / 32768.0);
            }
        }
        callbacks.fetch_add(1, Ordering::Relaxed);
    };
    let on_error = move |e| {
        eprintln!("[mobile-audio] stream callback error: {e}");
        failed.store(true, Ordering::Release);
    };
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _| {
                let frames = data.len() / channels;
                fill(&mut |i, v| data[i] = v, frames)
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _| {
                let frames = data.len() / channels;
                fill(&mut |i, v| data[i] = (v * 32767.0) as i16, frames)
            },
            on_error,
            None,
        ),
        f => return Err(format!("unsupported output format {f:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}
