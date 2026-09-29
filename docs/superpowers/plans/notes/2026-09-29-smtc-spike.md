# SMTC spike findings (2026-09-29)

Throwaway probe: Task 0 of track 6a (Windows System Media Transport Controls).
Probe source: `crates/oxitide-app/examples/smtc_probe.rs` (deleted in Task 4).

- GetForWindow: success — `factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()` returned the interop, and `interop.GetForWindow(hwnd)` returned `SystemMediaTransportControls` with no panic/HRESULT.
- HWND construction that compiled: classic `RegisterClassW` + `CreateWindowExW` from the brief, with `GetModuleHandleW(None)` as the instance. No substitution (`GetConsoleWindow`/`GetDesktopWindow`) was needed.
- ButtonPressed delivery: deferred to Task 6 (requires a human at the desktop pressing media keys). The probe installed the `TypedEventHandler` successfully and pumped messages via its own `PeekMessageW`/`DispatchMessageW` loop, but no button was pressed during the automated run, so no thread id was observed.
- Thumbnail via CreateFromUri: deferred to Task 6 for rendering. Automated outcome: `Uri::CreateUri` + `RandomAccessStreamReference::CreateFromUri` both succeeded — stdout printed `THUMBNAIL: URI accepted` and `updater.Update()` did not error. Whether the flyout actually paints the remote image is deferred to Task 6. Fallback needed: unknown (defer render check).
- Windows version on the test machine: Windows 11 Pro 25H2, build 10.0.26200.
- Notes for Task 3: the brief's interop/HWND/thumbnail code compiled and ran unchanged under `windows` 0.62.2; no signature adjustments were required.

## Exact working code

`Cargo.toml` (added verbatim; workspace already pinned `windows` 0.62.2 transitively, no lockfile conflict):

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.62", features = [
    "Media",
    "Foundation",
    "Storage_Streams",
    "Win32_Foundation",
    "Win32_System_WinRT",
    "Win32_System_Com",
    "Win32_System_LibraryLoader",
    "Win32_UI_WindowsAndMessaging",
] }
```

Interop + HWND:

```rust
let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
let hinstance = GetModuleHandleW(None).expect("module handle");
let class = w!("OxiTideSmtcProbe");
let wc = WNDCLASSW {
    hInstance: hinstance.into(),
    lpszClassName: class,
    lpfnWndProc: Some(wndproc),
    ..Default::default()
};
let _ = RegisterClassW(&wc);
let hwnd = CreateWindowExW(
    Default::default(), class, w!("probe"), WS_OVERLAPPED,
    CW_USEDEFAULT, CW_USEDEFAULT, 400, 300,
    None, None, Some(hinstance.into()), None,
).expect("create window");

let interop: ISystemMediaTransportControlsInterop =
    factory::<SystemMediaTransportControls, _>().expect("interop factory");
let controls: SystemMediaTransportControls =
    interop.GetForWindow(hwnd).expect("GetForWindow");
```

Button handler (installed; delivery unverified):

```rust
let handler = TypedEventHandler::new(
    |_, args: windows::core::Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
        let args = args.ok()?;
        println!("BUTTON: {:?} on {:?}", args.Button()?, std::thread::current().id());
        Ok(())
    },
);
controls.ButtonPressed(&handler).unwrap();
```

Thumbnail (accepted the URI; render unverified):

```rust
let url = "https://resources.tidal.com/images/2fa8b7a6/9d5b/4a1e/9f0a/4a1e9f0a4a1e/320x320.jpg";
match Uri::CreateUri(&HSTRING::from(url)) {
    Ok(uri) => match RandomAccessStreamReference::CreateFromUri(&uri) {
        Ok(stream) => { updater.SetThumbnail(&stream).unwrap(); println!("THUMBNAIL: URI accepted"); }
        Err(e) => println!("THUMBNAIL: CreateFromUri failed: {e}"),
    },
    Err(e) => println!("THUMBNAIL: CreateUri failed: {e}"),
}
updater.Update().unwrap();
```

## Automated run output

```
$ cargo run -p oxitide-app --example smtc_probe
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.67s
     Running `target\debug\examples\smtc_probe.exe`
THUMBNAIL: URI accepted
SMTC probe ready: open the flyout and press media keys for 30s
```

(Process self-terminated after ~30s.)
