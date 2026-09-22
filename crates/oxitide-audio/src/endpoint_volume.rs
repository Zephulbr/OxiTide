use windows::core::HSTRING;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

use crate::error::{Error, Result};

/// Set the endpoint master volume scalar (0.0..=1.0) for one render device, or
/// the system default when `device_id` is `None`.
pub fn set_master_volume(device_id: Option<&str>, volume: f32) -> Result<()> {
    crate::output::platform::ensure_com()?;
    let volume = volume.clamp(0.0, 1.0);
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
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).map_err(to_error)?;
        endpoint
            .SetMasterVolumeLevelScalar(volume, std::ptr::null())
            .map_err(to_error)?;
    }
    Ok(())
}

fn to_error(e: windows::core::Error) -> Error {
    Error::Output(e.to_string())
}
