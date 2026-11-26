//! Sound (spec 1.1 §6): Core Audio — output devices, sessions of programs on the default output, the default
//! microphone. Read every 2 s while a panel with `mixer` (or a `mic` / `audioOut` button) is active; changes
//! come through `Actions::audio`. The default output is set with IPolicyConfig (undocumented, as in SoundSwitch
//! and EarTrumpet). COM must be initialised on the calling thread.

use crate::actions::AudioOp;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Sessions that are silent for longer than this are not shown.
pub const RECENT: Duration = Duration::from_secs(30);

/// `audioOut`: the device after `current` in `devices` that is connected now (`active`), round the list.
/// A current device outside the list -> the first connected one. None when nothing else is connected.
pub fn next_output(devices: &[String], current: Option<&str>, active: &[String]) -> Option<String> {
    if devices.is_empty() {
        return None;
    }
    let start = current.and_then(|c| devices.iter().position(|d| d == c)).map(|i| i + 1).unwrap_or(0);
    (0..devices.len())
        .map(|k| &devices[(start + k) % devices.len()])
        .find(|d| Some(d.as_str()) != current && active.iter().any(|a| a == *d))
        .cloned()
}

#[cfg(windows)]
mod win {
    use windows::core::{Interface, GUID, HRESULT, PCWSTR, PWSTR};
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCommunications, eConsole, eMultimedia, eRender, AudioSessionStateActive, EDataFlow, ERole, IAudioSessionControl2,
        IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};

    /// PKEY_Device_FriendlyName
    const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };

    pub type R<T> = Result<T, String>;

    fn e(err: windows::core::Error) -> String {
        format!("звук: {}", err.message())
    }

    /// PWSTR from COM (CoTaskMemAlloc) -> String, freed.
    unsafe fn take(p: PWSTR) -> String {
        if p.is_null() {
            return String::new();
        }
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }

    pub fn enumerator() -> R<IMMDeviceEnumerator> {
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(e)
    }

    pub fn id(d: &IMMDevice) -> String {
        unsafe { d.GetId().map(|p| take(p)).unwrap_or_default() }
    }

    pub fn name(d: &IMMDevice) -> String {
        unsafe {
            let Ok(store) = d.OpenPropertyStore(STGM_READ) else { return String::new() };
            let Ok(mut v) = store.GetValue(&FRIENDLY_NAME) else { return String::new() };
            let s = PropVariantToStringAlloc(&v).map(|p| take(p)).unwrap_or_default();
            let _ = PropVariantClear(&mut v);
            s
        }
    }

    /// Active devices of a direction: (id, name).
    pub fn devices(en: &IMMDeviceEnumerator, flow: EDataFlow) -> R<Vec<(String, String)>> {
        unsafe {
            let c = en.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE).map_err(e)?;
            let n = c.GetCount().map_err(e)?;
            Ok((0..n).filter_map(|i| c.Item(i).ok()).map(|d| (id(&d), name(&d))).collect())
        }
    }

    pub fn default_device(en: &IMMDeviceEnumerator, flow: EDataFlow) -> Option<IMMDevice> {
        unsafe { en.GetDefaultAudioEndpoint(flow, eConsole).ok() }
    }

    pub fn endpoint_volume(d: &IMMDevice) -> R<IAudioEndpointVolume> {
        unsafe { d.Activate(CLSCTX_ALL, None) }.map_err(e)
    }

    /// Sessions of the default output: (pid, display name, active now, volume, muted, ISimpleAudioVolume).
    pub fn sessions(en: &IMMDeviceEnumerator) -> R<Vec<(u32, String, bool, f32, bool, ISimpleAudioVolume)>> {
        let Some(dev) = default_device(en, eRender) else { return Ok(Vec::new()) };
        unsafe {
            let mgr: IAudioSessionManager2 = dev.Activate(CLSCTX_ALL, None).map_err(e)?;
            let list = mgr.GetSessionEnumerator().map_err(e)?;
            let n = list.GetCount().map_err(e)?;
            let mut out = Vec::new();
            for i in 0..n {
                let Ok(c) = list.GetSession(i) else { continue };
                let Ok(c2) = c.cast::<IAudioSessionControl2>() else { continue };
                let Ok(vol) = c.cast::<ISimpleAudioVolume>() else { continue };
                let pid = c2.GetProcessId().unwrap_or(0);
                let system = c2.IsSystemSoundsSession() == HRESULT(0);
                let pid = if system { 0 } else { pid };
                let active = c.GetState().map(|s| s == AudioSessionStateActive).unwrap_or(false);
                let display = c.GetDisplayName().map(|p| take(p)).unwrap_or_default();
                let level = vol.GetMasterVolume().unwrap_or(0.0);
                let muted = vol.GetMute().map(|b| b.as_bool()).unwrap_or(false);
                out.push((pid, display, active, level, muted, vol));
            }
            Ok(out)
        }
    }

    #[repr(C)]
    struct PolicyConfigVtbl {
        base: windows::core::IUnknown_Vtbl,
        // GetMixFormat .. SetPropertyValue: not used
        _unused: [usize; 10],
        set_default_endpoint: unsafe extern "system" fn(*mut core::ffi::c_void, PCWSTR, ERole) -> HRESULT,
    }

    const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

    /// Default output for console, multimedia and communications.
    pub fn set_default_output(device_id: &str) -> R<()> {
        unsafe {
            let unk: windows::core::IUnknown = CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL).map_err(e)?;
            let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
            unk.query(&IID_POLICY_CONFIG, &mut p).ok().map_err(e)?;
            // owns the reference QueryInterface added
            let pc = windows::core::IUnknown::from_raw(p);
            let vtbl = *(p as *const *const PolicyConfigVtbl);
            let id: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
            for role in [eConsole, eMultimedia, eCommunications] {
                ((*vtbl).set_default_endpoint)(pc.as_raw(), PCWSTR(id.as_ptr()), role).ok().map_err(e)?;
            }
            Ok(())
        }
    }
}

/// Sessions are shown while active and for `RECENT` after; exe names are cached per pid.
#[derive(Default)]
pub struct Audio {
    last_active: HashMap<u32, Instant>,
    names: HashMap<u32, String>,
}

#[cfg(windows)]
impl Audio {
    pub fn new() -> Audio {
        Audio::default()
    }

    /// `audio` in /api/v1/state.
    pub fn sample(&mut self) -> Result<Value, String> {
        use windows::Win32::Media::Audio::{eCapture, eRender};
        let en = win::enumerator()?;
        let outputs = win::devices(&en, eRender)?;
        let output = win::default_device(&en, eRender).map(|d| {
            let id = win::id(&d);
            let name = outputs.iter().find(|(i, _)| *i == id).map(|(_, n)| n.clone()).unwrap_or_else(|| win::name(&d));
            json!({"id": id, "name": name})
        });
        let mic = win::default_device(&en, eCapture).map(|d| {
            let muted = win::endpoint_volume(&d).ok().and_then(|v| unsafe { v.GetMute() }.ok()).map(|b| b.as_bool()).unwrap_or(false);
            json!({"name": win::name(&d), "muted": muted})
        });
        let now = Instant::now();
        let mut apps: Vec<(u32, String, String, f32, bool)> = Vec::new();
        for (pid, display, active, level, muted, _) in win::sessions(&en)? {
            if active {
                self.last_active.insert(pid, now);
            }
            if !self.last_active.get(&pid).is_some_and(|t| now.duration_since(*t) < RECENT) {
                continue;
            }
            if let Some(a) = apps.iter_mut().find(|a| a.0 == pid) {
                a.4 &= muted; // several sessions of one program: muted only when all are
                continue;
            }
            let name = if pid == 0 {
                "system".to_string()
            } else {
                self.names
                    .entry(pid)
                    .or_insert_with(|| {
                        let p = crate::platform::exe_path(pid).unwrap_or_default();
                        let n = crate::platform::file_name(&p);
                        n.strip_suffix(".exe").or_else(|| n.strip_suffix(".EXE")).unwrap_or(n).to_string()
                    })
                    .clone()
            };
            let title = if pid == 0 {
                "Системные звуки".to_string()
            } else if display.is_empty() || display.starts_with('@') {
                name.clone()
            } else {
                display
            };
            apps.push((pid, name, title, level, muted));
        }
        self.last_active.retain(|_, t| now.duration_since(*t) < RECENT);
        self.names.retain(|pid, _| self.last_active.contains_key(pid));
        let apps: Vec<Value> = apps
            .into_iter()
            .map(|(pid, name, title, level, muted)| {
                json!({"pid": pid, "name": name, "title": title, "volume": (level as f64 * 100.0).round() / 100.0, "muted": muted})
            })
            .collect();
        let outputs: Vec<Value> = outputs.into_iter().map(|(id, name)| json!({"id": id, "name": name})).collect();
        Ok(json!({"output": output, "outputs": outputs, "mic": mic, "apps": apps}))
    }
}

/// GET /admin/audio/devices: `{"outputs": [{id, name, default}], "inputs": [...]}`.
#[cfg(windows)]
pub fn devices() -> Result<Value, String> {
    use windows::Win32::Media::Audio::{eCapture, eRender};
    let en = win::enumerator()?;
    let list = |flow| -> Result<Vec<Value>, String> {
        let def = win::default_device(&en, flow).map(|d| win::id(&d));
        Ok(win::devices(&en, flow)?
            .into_iter()
            .map(|(id, name)| {
                let is_default = def.as_deref() == Some(id.as_str());
                json!({"id": id, "name": name, "default": is_default})
            })
            .collect())
    };
    Ok(json!({"outputs": list(eRender)?, "inputs": list(eCapture)?}))
}

/// Applies a change (the real side of `Actions::audio`).
#[cfg(windows)]
pub fn apply(op: &AudioOp) -> Result<(), String> {
    use windows::Win32::Media::Audio::{eCapture, eRender};
    let en = win::enumerator()?;
    let mic = || -> Result<_, String> {
        let d = win::default_device(&en, eCapture).ok_or("микрофон не найден")?;
        win::endpoint_volume(&d)
    };
    let set_err = |x: windows::core::Error| format!("звук: {}", x.message());
    match op {
        AudioOp::Output(id) => {
            if !win::devices(&en, eRender)?.iter().any(|(i, _)| i == id) {
                return Err("устройство вывода не подключено".into());
            }
            win::set_default_output(id)
        }
        AudioOp::NextOutput(list) => {
            let current = win::default_device(&en, eRender).map(|d| win::id(&d));
            let active: Vec<String> = win::devices(&en, eRender)?.into_iter().map(|(i, _)| i).collect();
            match next_output(list, current.as_deref(), &active) {
                Some(id) => win::set_default_output(&id),
                None if current.as_ref().is_some_and(|c| list.contains(c)) => Ok(()), // the only connected one is already on
                None => Err("устройства из списка не подключены".into()),
            }
        }
        AudioOp::Volume { pid, value } => {
            let list: Vec<_> = win::sessions(&en)?.into_iter().filter(|s| s.0 == *pid).collect();
            if list.is_empty() {
                return Err("программа не найдена".into());
            }
            for s in list {
                unsafe { s.5.SetMasterVolume(value.clamp(0.0, 1.0), std::ptr::null()) }.map_err(set_err)?;
            }
            Ok(())
        }
        AudioOp::AppMute { pid, muted } => {
            let list: Vec<_> = win::sessions(&en)?.into_iter().filter(|s| s.0 == *pid).collect();
            if list.is_empty() {
                return Err("программа не найдена".into());
            }
            for s in list {
                unsafe { s.5.SetMute(*muted, std::ptr::null()) }.map_err(set_err)?;
            }
            Ok(())
        }
        AudioOp::Mic(m) => unsafe { mic()?.SetMute(*m, std::ptr::null()) }.map_err(set_err),
        AudioOp::MicToggle => {
            let v = mic()?;
            unsafe {
                let m = v.GetMute().map_err(set_err)?.as_bool();
                v.SetMute(!m, std::ptr::null()).map_err(set_err)
            }
        }
    }
}
