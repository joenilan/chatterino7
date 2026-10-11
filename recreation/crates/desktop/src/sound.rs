//! Mention/highlight pings. One audio thread owns the output device; the
//! built-in tones are synthesized (no bundled audio files), or a .wav you choose.
use std::{
    path::PathBuf,
    sync::{OnceLock, mpsc},
    time::{Duration, Instant},
};

const RATE: u32 = 44_100;
/// Largest custom sound accepted (a ping, not a song).
const CUSTOM_LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum Tone {
    /// Two rising notes.
    Chime,
    /// One low, short note.
    Soft,
    Custom(PathBuf),
}
impl Tone {
    pub fn key(&self) -> String {
        match self {
            Tone::Chime => "chime".into(),
            Tone::Soft => "soft".into(),
            Tone::Custom(path) => path.display().to_string(),
        }
    }
    pub fn from_key(key: &str) -> Self {
        match key {
            "" | "chime" => Tone::Chime,
            "soft" => Tone::Soft,
            path => Tone::Custom(PathBuf::from(path)),
        }
    }
}

struct Request {
    tone: Tone,
    volume: f32,
}

fn note(samples: &mut Vec<f32>, start: f32, frequency: f32, length: f32, decay: f32) {
    let first = (start * RATE as f32) as usize;
    let count = (length * RATE as f32) as usize;
    if samples.len() < first + count {
        samples.resize(first + count, 0.);
    }
    for i in 0..count {
        let t = i as f32 / RATE as f32;
        // 4 ms attack avoids a click; exponential decay; a little second harmonic for warmth.
        let envelope = (t / 0.004).min(1.) * (-t * decay).exp();
        let phase = std::f32::consts::TAU * frequency * t;
        samples[first + i] += envelope * (phase.sin() * 0.8 + (phase * 2.).sin() * 0.2);
    }
}
pub fn synthesize(tone: &Tone) -> Vec<f32> {
    let mut samples = Vec::new();
    match tone {
        Tone::Soft => note(&mut samples, 0., 660., 0.35, 11.),
        _ => {
            note(&mut samples, 0., 880., 0.3, 12.);
            note(&mut samples, 0.11, 1318.5, 0.4, 9.);
        }
    }
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs())).max(1e-6);
    samples.iter().map(|s| s / peak * 0.5).collect()
}

fn worker(requests: mpsc::Receiver<Request>) {
    use rodio::{Decoder, OutputStream, Sink, buffer::SamplesBuffer};
    // Opened on first use and kept; reopened if the device disappears.
    let mut output: Option<(OutputStream, rodio::OutputStreamHandle)> = None;
    let mut sinks: Vec<Sink> = Vec::new();
    while let Ok(request) = requests.recv() {
        if output.is_none() {
            output = OutputStream::try_default().ok();
        }
        let Some((_, handle)) = &output else { continue };
        sinks.retain(|s| !s.empty());
        let Ok(sink) = Sink::try_new(handle) else {
            output = None;
            continue;
        };
        sink.set_volume(request.volume.clamp(0., 1.));
        match &request.tone {
            Tone::Custom(path) => {
                let bytes = std::fs::metadata(path)
                    .ok()
                    .filter(|m| m.len() <= CUSTOM_LIMIT)
                    .and_then(|_| std::fs::read(path).ok());
                match bytes.and_then(|b| Decoder::new_wav(std::io::Cursor::new(b)).ok()) {
                    Some(decoder) => sink.append(decoder),
                    // Unreadable file: fall back to the chime rather than staying silent.
                    None => sink.append(SamplesBuffer::new(1, RATE, synthesize(&Tone::Chime))),
                }
            }
            tone => sink.append(SamplesBuffer::new(1, RATE, synthesize(tone))),
        }
        sinks.push(sink);
    }
}

/// Play without blocking. Pings closer than 700 ms apart are merged into one.
pub fn play(tone: &Tone, volume: f32) {
    static SENDER: OnceLock<std::sync::Mutex<(mpsc::Sender<Request>, Option<Instant>)>> =
        OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("jawjack-sound".into())
            .spawn(move || worker(rx));
        std::sync::Mutex::new((tx, None))
    });
    let Ok(mut state) = sender.lock() else { return };
    if state
        .1
        .is_some_and(|last| last.elapsed() < Duration::from_millis(700))
    {
        return;
    }
    state.1 = Some(Instant::now());
    let _ = state.0.send(Request {
        tone: tone.clone(),
        volume,
    });
}

/// A custom sound must be a readable, reasonably small WAV file.
pub fn check_custom(path: &std::path::Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|_| "Can't read that file.".to_owned())?;
    if meta.len() > CUSTOM_LIMIT {
        return Err("Pick a shorter sound (4 MB at most).".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "Can't read that file.".to_owned())?;
    rodio::Decoder::new_wav(std::io::Cursor::new(bytes))
        .map(|_| ())
        .map_err(|_| "That isn't a WAV file Jawjack can play.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tones_are_short_and_bounded() {
        for tone in [Tone::Chime, Tone::Soft] {
            let samples = synthesize(&tone);
            assert!(samples.len() < RATE as usize, "under a second");
            assert!(samples.iter().all(|s| s.abs() <= 0.5001));
        }
        assert_eq!(Tone::from_key(&Tone::Soft.key()), Tone::Soft);
        assert_eq!(
            Tone::from_key("C:\\a.wav"),
            Tone::Custom("C:\\a.wav".into())
        );
    }
}
