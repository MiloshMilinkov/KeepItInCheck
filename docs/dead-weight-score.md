# How the dead-weight score works

KeepItInCheck flags apps that are running but adding nothing. This page explains exactly how an
app ends up as **Safe to close**, **Probably safe** or **Keep**, and which setting moves which
line. Every rule below lives in `src-tauri/src/classifier.rs` and is covered by unit tests
(`cargo test --manifest-path src-tauri/Cargo.toml`).

All thresholds are in **Settings → Dead-weight thresholds** and are stored in
`%APPDATA%\com.milinkov.keepitincheck\settings.json`.

## 1. What counts as one "app"

Processes are rolled up into applications first. A process joins its parent's app when it has
the same exe name (Chrome, Edge, Discord and other Electron apps), when it is a generic helper
(`msedgewebview2.exe`, `crashpad_handler.exe`), or when its exe sits inside the parent's install
folder (`steamwebhelper.exe` under `Steam\`). Launchers such as Explorer, a terminal or
`svchost.exe` break the chain, so an app you started from them is its own row. CPU, RAM and I/O
are summed across the whole group.

RAM is the **private working set**, the same number Task Manager shows. It is the memory that is
actually returned when the app exits.

## 2. Protected: never suggested, never closed

Checked before anything else. Any one of these makes the app **Keep**, with a shield icon:

| Rule | How it is detected |
|---|---|
| KeepItInCheck itself | our PID and every descendant (its WebView2 helpers) |
| Core Windows process | name list in `protect.rs` (`csrss`, `lsass`, `dwm`, `explorer`, `svchost`, …) |
| System session | the process runs in session 0, where services and the kernel live |
| Windows service | PID hosts a running service (Service Control Manager, refreshed every 30 s) |
| Security software | known antivirus exe names, or a security vendor in the file's CompanyName |
| Microsoft-signed | Authenticode signature (embedded or catalog) issued to Microsoft. By default this protects **every** Microsoft-signed app; turn off *Protect all Microsoft-signed apps* to protect only those inside the Windows folder |
| In use | the app owns, or is a parent of, the window you are using right now |
| Can't be inspected | we cannot read its path (it runs elevated), so we cannot reason about it |

Signature checks never touch the network (revocation checks off, cache-only URL retrieval) and
run on a low-priority background thread. Until the check finishes the app shows
"Checking publisher signature…" and is not suggested.

## 3. Not dead weight: Keep

Then every one of these gates must pass, otherwise the app is **Keep** with the reason shown:

| Gate | Default | Setting |
|---|---|---|
| On your never-touch list or snoozed | – | Never touch / Snooze |
| RAM at least the minimum | **50 MB** | Ignore apps under |
| Watched for at least 2 minutes | 2 min | fixed |
| No audio in the idle window | 10 min | Averaged over |
| Average CPU below the idle line | **0.3 %** of the whole machine | Idle CPU below |
| Average disk + network I/O negligible | **20 KB/s** | Negligible I/O below |
| Has no window **or** its window has not been in the foreground long enough | **30 min** | Window unused for |

Notes:

- **Averages are exact.** Every 15 s the app's cumulative CPU time and I/O byte counters are
  checkpointed. The average over the window is the difference divided by elapsed time, so it is
  correct whether the panel polled every second or every 5 seconds.
- **CPU % is of the whole machine**, like Task Manager: 0.3 % on a 16-thread CPU is about 5 % of
  one core.
- **I/O** comes from the Windows I/O counters (read + write + other transfer bytes), which
  include both file and network traffic.
- **Audio** counts only when a session is active *and* its peak meter is above silence, so an app
  that merely keeps an audio device open does not count as playing.
- **Focus**: clicking into the KeepItInCheck panel never counts as using another app; the app
  remembers your last real foreground window.

## 4. The score (0–100)

An app that passed every gate gets a score. Higher means more clearly dead weight.

| Part | Max | Full points when |
|---|---|---|
| CPU | 30 | average CPU is 0; falls linearly to 0 at the idle line |
| Window | 25 | no window at all. A windowed app gets 15 at the unfocused limit, rising to 25 at twice the limit |
| RAM | 20 | 1 GB or more (log scale from the minimum RAM up to 1 GB) |
| I/O | 15 | no I/O; falls linearly to 0 at the I/O limit |
| Audio | 10 | always (playing audio already made it Keep) |

While the app has been watched for less than the full idle window, the score is multiplied by
the covered fraction (never below 0.5), so early guesses score lower.

## 5. Safe to close vs. Probably safe

An app is **Safe to close** (teal, pre-checked in Clean up) only when **all** of these hold:

1. it has **no window** (so there is nothing on screen with unsaved work),
2. it was watched for the **full idle window** (10 min by default),
3. it is not in the **cautious** list (sync clients, VPNs, password managers, remote access,
   game launchers, IDEs and terminals; see `CAUTION` in `protect.rs`),
4. its score is at least **Safe-to-close score** (default **65**).

Everything else that passed the gates is **Probably safe** (amber, unchecked in Clean up).

## 6. The reason text

Every suggestion explains itself, for example:

- `Idle for 47 min, no window, using 312 MB`
- `Idle for 52 min, window unused for 52 min, using 318 MB`
- `Mostly idle (0.12% CPU avg), no window, using 96 MB`: the average is low but there were
  short bursts of activity inside the window
- `Idle for 5 min so far, …`: still inside the first idle window
- `… · may sync or work in the background`: a cautious-list app

## 7. Tuning cheat sheet

| You see | Change |
|---|---|
| Too many suggestions | lower *Idle CPU below* (e.g. 0.1 %), raise *Ignore apps under* (e.g. 150 MB), raise *Averaged over* (e.g. 20 min) |
| Too few suggestions | raise *Idle CPU below* (e.g. 0.5 %), lower *Window unused for* (e.g. 15 min) |
| Background apps stuck at "Probably safe" | lower *Safe-to-close score* (e.g. 55) |
| A specific app keeps coming back | open its row and choose *Never touch this app*, or snooze it for 24 h |
| Want Edge / Teams / OneDrive considered | turn off *Protect all Microsoft-signed apps* |

## 8. What happens when you clean up

1. The selection is re-checked against fresh data. Anything that became Keep (you switched to
   it, it started playing audio) is skipped and listed with the reason.
2. Each app gets a normal close request: `WM_CLOSE`, the same message as the title-bar ×.
   Background apps get it on their hidden windows too.
3. Up to **5 seconds** for everything to exit.
4. Still running and **no window**: ended with `TerminateProcess`, after checking the PID still
   belongs to the same process (start time match).
   Still running **with a window** (or it opened one, like a save prompt): **left running**,
   with an explanation. Force-closing it is a separate per-app button with its own confirmation.
5. RAM freed is measured from the system's available memory before and after, and every result
   is written to the cleanup log (`cleanup-log.json`), with Relaunch for apps whose path is known.

Dry run (in Clean up, or as the default in Settings) does steps 1 and reports what would happen,
without sending anything.
