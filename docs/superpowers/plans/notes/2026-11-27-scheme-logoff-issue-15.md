# reg.exe error on sign-out or restart (issue #15)

**Status:** Fixed on `release/1.x` (`44eb4531`), ships in v1.1.11; owner smoke passed
2026-11-27 on a local build of `a355ffd6`. Open: the reporter's confirmation.

Issue: https://github.com/Zephulbr/OxiTide/issues/15 (Windows 11 Home: signing out or
restarting with OxiTide open shows a "reg.exe - Application Error" dialog).

## Cause

`scheme.rs` hands `tidal://` back to the official TIDAL app on exit (spec §5.2). Up to
1.1.10 it did so by spawning `reg.exe` (`reg import` of a `.reg` backup, or `reg delete`)
from `run_exit_work`. A sign-out or restart also ends the app through `RunEvent::Exit`, but
by then the session is closing and Windows cannot start a new console process: it fails DLL
initialisation (0xc0000142), which is the dialog. The restore never ran, so on machines with
the official app installed OxiTide kept its `tidal://` handler and `tidal-scheme.reg` stayed
behind.

## Fix

- Detection, backup and restore use the registry API in-process (`RegGetValueW`,
  `RegCopyTreeW`, `RegDeleteTreeW`; the `Win32_System_Registry` feature of `windows`).
  Nothing is spawned on exit.
- The backup is a registry copy at `HKCU\Software\OxiTide\tidal-scheme-backup`, not a file.
  `RegCopyTreeW` copies security descriptors too, so the destination is opened with
  `KEY_ALL_ACCESS` (`KEY_WRITE` lacks `WRITE_DAC`: error 5).
- Migration: if a `tidal-scheme.reg` from an older build exists while the key points at
  OxiTide and no registry backup does, it is `reg import`ed once at startup (where starting a
  process works), deleted, and the snapshot then backs the official handler up into the
  registry. A failed import keeps the file.
- A `tidal` key whose `shell\open\command` is missing or unreadable now counts as another
  app's (backed up and restored), never as absent (deleted on exit).
- Test: `backs_up_and_restores_a_handler_in_process` runs the round trip on a scratch HKCU
  key and removes it on drop.

## Owner smoke

On a release-identity build of this branch, with the official TIDAL app installed (it owns
`tidal://` before OxiTide starts):

- [x] Start OxiTide, then sign out of Windows (and once: restart). No "reg.exe" dialog.
- [x] Signed back in, before starting OxiTide:
      `reg query HKCU\Software\Classes\tidal\shell\open\command /ve` names TIDAL's exe, and
      `HKCU\Software\OxiTide\tidal-scheme-backup` is gone.
- [x] Quit OxiTide from the tray: the same two checks.
- [x] Sign in to TIDAL in OxiTide (the paste-back redirect) still works while it runs.
- [x] Migration, if a 1.1.10 sign-out left `tidal-scheme.reg` in the config folder (next to
      `config.toml`): start the new build; the file is gone and the official handler comes
      back on the next quit.
