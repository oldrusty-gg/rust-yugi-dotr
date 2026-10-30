//! The mixer's output: a waveOut stream at 48 kHz stereo fed by a thread
//! that mixes 20 ms (one field) per buffer into four rotating buffers.
use crate::sound_mix::{Mixer, OUTPUT_RATE};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use windows::Win32::Media::Audio::*;

const BUFFERS: usize = 4;
const FRAMES: usize = (OUTPUT_RATE / 50) as usize;
const HEADER_BYTES: u32 = std::mem::size_of::<WAVEHDR>() as u32;

pub struct SoundOut {
    mixer: Arc<Mutex<Mixer>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SoundOut {
    /// Open the default output and start mixing.
    pub fn start() -> Result<Self, String> {
        if crate::muted() {
            return Err("muted by ROSES_MUTE".into());
        }
        let mixer = Arc::new(Mutex::new(Mixer::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (ready, started) = mpsc::channel();
        let (m, s) = (mixer.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("sound out".into())
            .spawn(move || run(m, s, ready))
            .map_err(|e| e.to_string())?;
        started.recv().map_err(|e| e.to_string())??;
        Ok(Self { mixer, stop, thread: Some(thread) })
    }

    pub fn mixer(&self) -> Arc<Mutex<Mixer>> {
        self.mixer.clone()
    }
}

impl Drop for SoundOut {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(mixer: Arc<Mutex<Mixer>>, stop: Arc<AtomicBool>, ready: mpsc::Sender<Result<(), String>>) {
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 2,
        nSamplesPerSec: OUTPUT_RATE,
        nAvgBytesPerSec: OUTPUT_RATE * 4,
        nBlockAlign: 4,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    let mut handle = HWAVEOUT::default();
    let opened = unsafe { waveOutOpen(Some(&mut handle), WAVE_MAPPER, &format, None, None, CALLBACK_NULL) };
    if opened != 0 {
        let _ = ready.send(Err(format!("waveOutOpen failed ({opened})")));
        return;
    }
    let mut buffers: Vec<Box<[i16]>> = (0..BUFFERS).map(|_| vec![0i16; FRAMES * 2].into_boxed_slice()).collect();
    let mut headers: Vec<Box<WAVEHDR>> = buffers
        .iter_mut()
        .map(|b| {
            Box::new(WAVEHDR {
                lpData: windows::core::PSTR(b.as_mut_ptr().cast()),
                dwBufferLength: (FRAMES * 4) as u32,
                ..Default::default()
            })
        })
        .collect();
    unsafe {
        for h in &mut headers {
            waveOutPrepareHeader(handle, &mut **h, HEADER_BYTES);
        }
    }
    let _ = ready.send(Ok(()));
    let mut queued = [false; BUFFERS];
    while !stop.load(Ordering::SeqCst) {
        for i in 0..BUFFERS {
            let done = headers[i].dwFlags & WHDR_DONE != 0;
            if queued[i] && !done {
                continue;
            }
            if let Ok(mut m) = mixer.lock() {
                m.mix(&mut buffers[i]);
            }
            unsafe {
                headers[i].dwFlags &= !WHDR_DONE;
                waveOutWrite(handle, &mut *headers[i], HEADER_BYTES);
            }
            queued[i] = true;
        }
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    unsafe {
        waveOutReset(handle);
        for h in &mut headers {
            waveOutUnprepareHeader(handle, &mut **h, HEADER_BYTES);
        }
        waveOutClose(handle);
    }
}
