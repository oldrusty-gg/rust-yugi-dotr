//! Owned waveOut buffers for one intro followed by an explicit PCM loop region.
//! Shared by the native client and the Bevy preview; callers supply logging.
use crate::audio::Pcm16Wave;
use std::cell::UnsafeCell;
use std::ops::Range;
use std::sync::Arc;
use windows::core::PSTR;
use windows::Win32::Media::Audio::*;

const HEADER_BYTES: u32 = std::mem::size_of::<WAVEHDR>() as u32;

struct Header {
    // The driver mutates the header asynchronously. UnsafeCell prevents claiming
    // immutable Rust access, and Box keeps its address stable if App moves.
    storage: Option<Box<UnsafeCell<WAVEHDR>>>,
    prepared: bool,
}
impl Header {
    fn new(pcm: &Pcm16Wave, range: Range<usize>) -> Self {
        Self {
            storage: Some(Box::new(UnsafeCell::new(WAVEHDR {
                // waveOut only reads output PCM; its legacy API uses LPSTR.
                lpData: PSTR(pcm.data()[range.clone()].as_ptr().cast_mut()),
                dwBufferLength: range.len() as u32,
                ..Default::default()
            }))),
            prepared: false,
        }
    }
    fn pointer(&self) -> *mut WAVEHDR {
        self.storage.as_ref().unwrap().get()
    }
    unsafe fn prepare(&mut self, handle: HWAVEOUT) -> Result<(), String> {
        checked(
            waveOutPrepareHeader(handle, self.pointer(), HEADER_BYTES),
            "prepare",
        )?;
        self.prepared = true;
        Ok(())
    }
    unsafe fn unprepare(&mut self, handle: HWAVEOUT) -> u32 {
        if !self.prepared {
            return 0;
        }
        let result = waveOutUnprepareHeader(handle, self.pointer(), HEADER_BYTES);
        if result == 0 {
            self.prepared = false;
        }
        result
    }
    fn retain_for_driver(&mut self) {
        // On an exceptional driver cleanup failure, a bounded leak is safer than
        // releasing an allocation that the native device may still reference.
        if let Some(storage) = self.storage.take() {
            let _ = Box::into_raw(storage);
        }
    }
}

pub struct WavePlayer {
    record: fn(&str),
    handle: HWAVEOUT,
    pcm: Option<Arc<Pcm16Wave>>,
    intro: Option<Header>,
    repeated: Header,
}
impl WavePlayer {
    pub fn start(
        pcm: Arc<Pcm16Wave>,
        loop_start_frame: u32,
        record: fn(&str),
    ) -> Result<Self, String> {
        if crate::muted() {
            return Err("muted by ROSES_MUTE".into());
        }
        let regions = pcm.loop_regions(loop_start_frame)?;
        let intro = (!regions.intro.is_empty()).then(|| Header::new(&pcm, regions.intro));
        let repeated = Header::new(&pcm, regions.repeated);
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: pcm.channels(),
            nSamplesPerSec: pcm.sample_rate(),
            nAvgBytesPerSec: pcm.sample_rate() * u32::from(pcm.block_align()),
            nBlockAlign: pcm.block_align(),
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let mut handle = HWAVEOUT::default();
        unsafe {
            checked(
                waveOutOpen(
                    Some(&mut handle),
                    WAVE_MAPPER,
                    &format,
                    None,
                    None,
                    CALLBACK_NULL,
                ),
                "open",
            )?;
        }
        // From here every failure runs Drop before any buffer allocation dies.
        let mut player = Self {
            record,
            handle,
            pcm: Some(pcm),
            intro,
            repeated,
        };
        unsafe {
            checked(waveOutPause(handle), "pause before queueing")?;
            if let Some(intro) = &mut player.intro {
                intro.prepare(handle)?;
            }
            player.repeated.prepare(handle)?;
            // Prepare requires zero flags. Set loop flags only after preparing,
            // while the header is not yet queued or owned by the running driver.
            let repeated = player.repeated.pointer();
            (*repeated).dwFlags |= WHDR_BEGINLOOP | WHDR_ENDLOOP;
            (*repeated).dwLoops = u32::MAX;
            if let Some(intro) = &player.intro {
                checked(
                    waveOutWrite(handle, intro.pointer(), HEADER_BYTES),
                    "queue intro",
                )?;
            }
            checked(waveOutWrite(handle, repeated, HEADER_BYTES), "queue loop")?;
            checked(waveOutRestart(handle), "start")?;
        }
        (player.record)(&format!(
            "PCM intro/loop playback started at loop frame {loop_start_frame}"
        ));
        Ok(player)
    }
}
impl Drop for WavePlayer {
    fn drop(&mut self) {
        unsafe {
            let reset = waveOutReset(self.handle);
            let intro = self.intro.as_mut().map_or(0, |h| h.unprepare(self.handle));
            let repeated = self.repeated.unprepare(self.handle);
            let close = waveOutClose(self.handle);
            if intro != 0 || repeated != 0 || close != 0 {
                if let Some(header) = &mut self.intro {
                    header.retain_for_driver();
                }
                self.repeated.retain_for_driver();
                if let Some(pcm) = self.pcm.take() {
                    std::mem::forget(pcm);
                }
                (self.record)(&format!("PCM cleanup failed; retained device buffers: reset={reset} intro={intro} loop={repeated} close={close}"));
            } else {
                (self.record)(&format!(
                    "PCM playback stopped and buffers released (reset={reset})"
                ));
            }
        }
    }
}
fn checked(code: u32, operation: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!(
            "Native PCM playback could not {operation} (waveOut status {code})."
        ))
    }
}
