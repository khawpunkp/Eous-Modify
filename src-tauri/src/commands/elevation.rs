//! Whether Eous is running as administrator, and how to restart it as one.
//!
//! Only the in-game reload needs this, and it needs it absolutely. Zenless Zone Zero runs elevated,
//! and Windows' UIPI silently discards synthetic input aimed at a higher-integrity process — while
//! `SendInput` still reports success. So a normal-privilege Eous queues a reload, sends F10, sees no
//! error, and nothing happens in the game. The failure is invisible from the inside, which is why the
//! app has to check its own elevation up front rather than wait for something to go wrong.

/// Why a `runas` launch didn't happen.
///
/// The caller supplies the wording: declining the prompt for the game is a different sentence from
/// declining it for Eous itself.
#[cfg(windows)]
pub enum RunAsError {
    /// The user dismissed the UAC prompt.
    Cancelled,
    /// `ShellExecuteW`'s status code, which is anything at or below 32.
    Failed(isize),
}

/// Launches `path` through `ShellExecuteW`'s `runas` verb, which raises the UAC prompt.
///
/// A non-elevated process can't spawn an elevated child any other way on Windows — hence the shell
/// round-trip rather than a plain `Command`. Used both to start the game when it demands elevation
/// and to restart Eous itself.
#[cfg(windows)]
pub fn run_as_admin(path: &str, args: &[&str]) -> Result<(), RunAsError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
    }

    let path_wide = wide(path);
    let verb_wide = wide("runas");

    // Kept alive for the duration of the call; ShellExecuteW takes one parameter string, not a list.
    let params = args.join(" ");
    let params_wide = wide(&params);
    let params_ptr = if params.is_empty() { PCWSTR::null() } else { PCWSTR(params_wide.as_ptr()) };

    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            PCWSTR(verb_wide.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            params_ptr,
            None,
            SW_SHOWNORMAL,
        )
    };

    // The returned pseudo-HINSTANCE is really a status: anything above 32 means it launched,
    // anything else is an error code.
    let code = result.0 as isize;
    if code > 32 {
        return Ok(());
    }
    if code as i32 == ERROR_CANCELLED.0 as i32 {
        return Err(RunAsError::Cancelled);
    }
    Err(RunAsError::Failed(code))
}

/// Whether this process holds an elevated token.
#[cfg(windows)]
pub fn running_as_admin() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let queried = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
        .is_ok();

        CloseHandle(token).ok();
        queried && elevation.TokenIsElevated != 0
    }
}

/// Elevation is a Windows concept, and so is the reload that wants it. Reporting "already elevated"
/// elsewhere keeps the UI from offering a restart that could not fix anything.
#[cfg(not(windows))]
pub fn running_as_admin() -> bool {
    true
}

/// Starts a second copy of Eous with an elevated token. The caller is responsible for closing this
/// one — two instances sharing the database is not a state to linger in.
#[cfg(windows)]
pub fn restart_elevated() -> Result<(), String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("Could not locate Eous on disk: {e}"))?;

    match run_as_admin(&exe.to_string_lossy(), &[]) {
        Ok(()) => Ok(()),
        Err(RunAsError::Cancelled) => {
            Err("Administrator permission was declined.".to_string())
        }
        Err(RunAsError::Failed(code)) => Err(format!(
            "Windows would not restart Eous as administrator (ShellExecuteW error {code})."
        )),
    }
}

#[cfg(not(windows))]
pub fn restart_elevated() -> Result<(), String> {
    Err("This platform has nothing to restart into.".to_string())
}

/// Whether the reload can reach the game. Read on the Settings page to decide whether to offer the
/// restart below.
#[tauri::command]
pub fn is_elevated() -> bool {
    running_as_admin()
}

/// Restarts Eous as administrator at the user's request, closing this instance once the elevated one
/// is on its way.
#[tauri::command]
pub fn relaunch_as_admin(app_handle: tauri::AppHandle) -> Result<(), String> {
    restart_elevated()?;
    app_handle.exit(0);
    Ok(())
}
