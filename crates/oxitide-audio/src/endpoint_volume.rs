use windows::core::HSTRING;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    ENDPOINT_HARDWARE_SUPPORT_VOLUME,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

use crate::error::{Error, Result};

/// One render device's endpoint (master) volume. Only queried: the app never
/// sets the system volume (an exclusive open checks whether the device has
/// hardware volume, see `output::ExclusiveVolume`).
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
                None => enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).map_err(to_error)?,
            };
            let endpoint: IAudioEndpointVolume =
                device.Activate(CLSCTX_ALL, None).map_err(to_error)?;
            Ok(Self { endpoint })
        }
    }

    /// Whether the device applies the volume in hardware. An exclusive
    /// stream bypasses the audio engine, so only then does the system volume
    /// still apply to it.
    pub fn has_hardware_volume(&self) -> bool {
        unsafe { self.endpoint.QueryHardwareSupport() }
            .is_ok_and(|mask| mask & ENDPOINT_HARDWARE_SUPPORT_VOLUME != 0)
    }
}

fn to_error(e: windows::core::Error) -> Error {
    Error::Output(e.to_string())
}
