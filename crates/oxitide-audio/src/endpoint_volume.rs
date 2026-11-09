use windows::core::HSTRING;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    ENDPOINT_HARDWARE_SUPPORT_VOLUME,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

use crate::error::{Error, Result};

/// One render device's endpoint (master) volume, kept open by the thread that
/// feeds the device so it can step the volume every period.
pub struct EndpointVolume {
    endpoint: IAudioEndpointVolume,
}

impl EndpointVolume {
    /// The endpoint volume of `device_id`, or of the system default when
    /// `None`. The caller's thread must have COM initialized.
    pub fn open(device_id: Option<&str>) -> Result<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(to_error)?;
            let device: IMMDevice = match device_id {
                Some(id) => {
                    let id = HSTRING::from(id);
                    enumerator.GetDevice(&id).map_err(to_error)?
                }
                None => enumerator
                    .GetDefaultAudioEndpoint(eRender, eMultimedia)
                    .map_err(to_error)?,
            };
            let endpoint: IAudioEndpointVolume =
                device.Activate(CLSCTX_ALL, None).map_err(to_error)?;
            Ok(Self { endpoint })
        }
    }

    /// Whether the device applies the volume in hardware. An exclusive
    /// stream bypasses the audio engine, so without it the endpoint volume
    /// would have no effect and the volume must be applied in software.
    pub fn has_hardware_volume(&self) -> bool {
        unsafe { self.endpoint.QueryHardwareSupport() }
            .is_ok_and(|mask| mask & ENDPOINT_HARDWARE_SUPPORT_VOLUME != 0)
    }

    /// Set the volume as a slider position (0.0..=1.0); Windows applies its
    /// own audio taper to it.
    pub fn set(&self, position: f32) -> Result<()> {
        unsafe {
            self.endpoint
                .SetMasterVolumeLevelScalar(position.clamp(0.0, 1.0), std::ptr::null())
                .map_err(to_error)
        }
    }
}

fn to_error(e: windows::core::Error) -> Error {
    Error::Output(e.to_string())
}
