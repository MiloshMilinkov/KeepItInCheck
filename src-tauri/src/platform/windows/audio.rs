//! Finds processes that are producing sound right now, via WASAPI audio sessions.
//!
//! A session counts as playing when it is Active *and* its peak meter is above silence, so apps
//! that merely hold an open audio stream (common for chat apps) are not treated as playing.

use std::collections::HashSet;
use std::time::{Duration, Instant};
use windows::core::Interface;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

/// Re-enumerate output devices this often (headphones plugged in, etc.).
const DEVICE_REFRESH: Duration = Duration::from_secs(60);
const SILENCE: f32 = 0.0005;

#[derive(Default)]
pub struct AudioProbe {
    managers: Vec<IAudioSessionManager2>,
    refreshed: Option<Instant>,
    com_ready: bool,
}

impl AudioProbe {
    fn refresh_devices(&mut self) {
        self.managers.clear();
        unsafe {
            if !self.com_ready {
                // S_FALSE (already initialised) is fine; a mode mismatch just means COM is up.
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                self.com_ready = true;
            }
            let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            else {
                return;
            };
            let Ok(devices) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else { return };
            let count = devices.GetCount().unwrap_or(0);
            for i in 0..count {
                if let Ok(dev) = devices.Item(i) {
                    if let Ok(m) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
                        self.managers.push(m);
                    }
                }
            }
        }
        self.refreshed = Some(Instant::now());
    }

    pub fn playing_pids(&mut self) -> HashSet<u32> {
        if self.refreshed.is_none_or(|t| t.elapsed() > DEVICE_REFRESH) {
            self.refresh_devices();
        }
        let mut out = HashSet::new();
        for m in &self.managers {
            unsafe {
                let Ok(sessions) = m.GetSessionEnumerator() else { continue };
                let n = sessions.GetCount().unwrap_or(0);
                for i in 0..n {
                    let Ok(ctl) = sessions.GetSession(i) else { continue };
                    if ctl.GetState().ok() != Some(AudioSessionStateActive) {
                        continue;
                    }
                    let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
                    let Ok(pid) = ctl2.GetProcessId() else { continue };
                    if pid == 0 {
                        continue;
                    }
                    let loud = ctl
                        .cast::<IAudioMeterInformation>()
                        .and_then(|meter| meter.GetPeakValue())
                        .map(|peak| peak > SILENCE)
                        .unwrap_or(true);
                    if loud {
                        out.insert(pid);
                    }
                }
            }
        }
        out
    }
}
