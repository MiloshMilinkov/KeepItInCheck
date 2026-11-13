//! Static knowledge about processes that must never be suggested or closed.
//!
//! These name lists are the first, cheapest line of defence. The sampler adds dynamic checks on
//! top (session 0, Windows services, Microsoft Authenticode signatures, the foreground chain and
//! KeepItInCheck's own process tree). Names are lower-case exe file names.

/// Core Windows processes. Closing any of these can log you out, blank the screen or bugcheck.
pub const CRITICAL: &[&str] = &[
    "system",
    "system idle process",
    "idle",
    "registry",
    "memory compression",
    "secure system",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
    "svchost.exe",
    "dwm.exe",
    "explorer.exe",
    "fontdrvhost.exe",
    "sihost.exe",
    "ctfmon.exe",
    "taskhostw.exe",
    "runtimebroker.exe",
    "dllhost.exe",
    "conhost.exe",
    "audiodg.exe",
    "spoolsv.exe",
    "wudfhost.exe",
    "dashost.exe",
    "wmiprvse.exe",
    "lockapp.exe",
    "logonui.exe",
    "userinit.exe",
    "searchindexer.exe",
    "searchhost.exe",
    "searchapp.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "shellhost.exe",
    "textinputhost.exe",
    "applicationframehost.exe",
    "systemsettings.exe",
    "smartscreen.exe",
    "securityhealthservice.exe",
    "securityhealthsystray.exe",
    "sgrmbroker.exe",
    "msmpeng.exe",
    "nissrv.exe",
    "mpdefendercoreservice.exe",
    "mpcopyaccelerator.exe",
    "wlanext.exe",
    "unsecapp.exe",
    "rundll32.exe",
    "consent.exe",
    "werfault.exe",
    "wermgr.exe",
    "useroobebroker.exe",
    "credentialuibroker.exe",
    "windowsinternal.composableshell.experiences.textinput.inputapp.exe",
    "crossdeviceresume.exe",
    "widgetservice.exe",
];

/// Executables of well-known antivirus / endpoint-security products.
pub const SECURITY_EXES: &[&str] = &[
    "msmpeng.exe",
    "msseces.exe",
    "avp.exe",
    "avpui.exe",
    "avastsvc.exe",
    "avastui.exe",
    "avgsvc.exe",
    "avgui.exe",
    "bdagent.exe",
    "bdservicehost.exe",
    "vsserv.exe",
    "mcshield.exe",
    "mfemms.exe",
    "mcuicnt.exe",
    "ccsvchst.exe",
    "nortonsecurity.exe",
    "ns.exe",
    "ekrn.exe",
    "egui.exe",
    "mbam.exe",
    "mbamservice.exe",
    "mbamtray.exe",
    "sentinelagent.exe",
    "sentinelui.exe",
    "csfalconservice.exe",
    "csfalconcontainer.exe",
    "savservice.exe",
    "sophosui.exe",
    "sophoshealth.exe",
    "coreserviceshell.exe",
    "pccntmon.exe",
    "wrsa.exe",
    "fsav.exe",
    "fshoster32.exe",
    "avguard.exe",
    "avira.systray.exe",
    "psanhost.exe",
    "cmdagent.exe",
    "cis.exe",
    "a2service.exe",
    "zatray.exe",
    "hitmanpro.exe",
    "cyserver.exe",
    "elastic-endpoint.exe",
];

/// Substrings of company names (from file version info) that mark security software.
pub const SECURITY_VENDORS: &[&str] = &[
    "kaspersky",
    "bitdefender",
    "eset",
    "mcafee",
    "norton",
    "gen digital",
    "symantec",
    "avast",
    "avg technologies",
    "malwarebytes",
    "sophos",
    "crowdstrike",
    "sentinelone",
    "trend micro",
    "webroot",
    "f-secure",
    "withsecure",
    "avira",
    "panda security",
    "comodo",
    "emsisoft",
    "palo alto networks",
    "cylance",
    "zonealarm",
    "check point",
];

/// Apps that may look idle but often do important quiet work (sync, VPN, backup, password
/// managers, remote access). They are capped at "Probably safe" so a human always reviews them.
pub const CAUTION: &[&str] = &[
    "onedrive.exe",
    "dropbox.exe",
    "googledrivefs.exe",
    "icloudservices.exe",
    "icloudDrive.exe",
    "megasync.exe",
    "nextcloud.exe",
    "syncthing.exe",
    "backblaze.exe",
    "bzbui.exe",
    "1password.exe",
    "bitwarden.exe",
    "keepass.exe",
    "keepassxc.exe",
    "dashlane.exe",
    "lastpass.exe",
    "nordvpn.exe",
    "expressvpn.exe",
    "protonvpn.exe",
    "openvpn-gui.exe",
    "wireguard.exe",
    "tailscale-ipn.exe",
    "teamviewer.exe",
    "anydesk.exe",
    "rustdesk.exe",
    "parsecd.exe",
    "obs64.exe",
    "steam.exe",
    "epicgameslauncher.exe",
    "battle.net.exe",
    "docker desktop.exe",
    "com.docker.backend.exe",
    "wsl.exe",
    "wslservice.exe",
    "vmware.exe",
    "virtualbox.exe",
    "code.exe",
    "devenv.exe",
    "windowsterminal.exe",
];

/// Generic helper hosts that belong to whichever app launched them.
pub const HELPERS: &[&str] = &[
    "msedgewebview2.exe",
    "crashpad_handler.exe",
    "cefsharp.browsersubprocess.exe",
    "qtwebengineprocess.exe",
];

/// Parents that are launchers, not owners: a child of these is its own app.
pub const LAUNCHERS: &[&str] = &[
    "explorer.exe",
    "svchost.exe",
    "services.exe",
    "wininit.exe",
    "winlogon.exe",
    "userinit.exe",
    "sihost.exe",
    "taskhostw.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "windowsterminal.exe",
    "openconsole.exe",
    "conhost.exe",
    "runtimebroker.exe",
    "dllhost.exe",
    "rundll32.exe",
];

/// Readable names for system processes whose version info we usually cannot read (elevated).
const FRIENDLY: &[(&str, &str)] = &[
    ("msmpeng.exe", "Microsoft Defender Antivirus"),
    ("svchost.exe", "Service Host"),
    ("dwm.exe", "Desktop Window Manager"),
    ("searchindexer.exe", "Windows Search Indexer"),
    ("csrss.exe", "Client Server Runtime"),
    ("lsass.exe", "Local Security Authority"),
    ("services.exe", "Services and Controller"),
    ("wininit.exe", "Windows Start-Up"),
    ("winlogon.exe", "Windows Logon"),
    ("smss.exe", "Session Manager"),
    ("audiodg.exe", "Windows Audio Device Graph"),
    ("spoolsv.exe", "Print Spooler"),
    ("nvdisplay.container.exe", "NVIDIA Display Container"),
    ("nvcontainer.exe", "NVIDIA Container"),
    ("officeclicktorun.exe", "Microsoft Office Click-to-Run"),
    ("wmiprvse.exe", "WMI Provider Host"),
    ("fontdrvhost.exe", "Font Driver Host"),
    ("dllhost.exe", "COM Surrogate"),
    ("conhost.exe", "Console Window Host"),
];

pub fn friendly_name(exe_name: &str) -> Option<&'static str> {
    FRIENDLY.iter().find(|(n, _)| n.eq_ignore_ascii_case(exe_name)).map(|(_, f)| *f)
}

fn has(list: &[&str], name: &str) -> bool {
    list.iter().any(|n| n.eq_ignore_ascii_case(name))
}

pub fn is_critical(name: &str) -> bool {
    has(CRITICAL, name)
}

pub fn is_security(name: &str, company: Option<&str>) -> bool {
    if has(SECURITY_EXES, name) {
        return true;
    }
    company.is_some_and(|c| {
        let c = c.to_lowercase();
        SECURITY_VENDORS.iter().any(|v| c.contains(v))
    })
}

pub fn is_caution(name: &str) -> bool {
    has(CAUTION, name)
}

pub fn is_helper(name: &str) -> bool {
    has(HELPERS, name)
}

pub fn is_launcher(name: &str) -> bool {
    has(LAUNCHERS, name)
}

/// True for binaries that live inside the Windows directory (e.g. `C:\Windows\System32`).
pub fn is_in_windows_dir(exe: &str) -> bool {
    static PREFIX: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let prefix = PREFIX.get_or_init(|| {
        let windir = std::env::var("WINDIR")
            .or_else(|_| std::env::var("SystemRoot"))
            .unwrap_or_else(|_| "c:\\windows".into())
            .to_lowercase();
        windir.trim_end_matches('\\').to_string() + "\\"
    });
    exe.to_lowercase().starts_with(prefix.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_case_insensitive() {
        assert!(is_critical("LSASS.EXE"));
        assert!(is_critical("explorer.exe"));
        assert!(!is_critical("chrome.exe"));
        assert!(is_security("MsMpEng.exe", None));
        assert!(is_security("whatever.exe", Some("Bitdefender SRL")));
        assert!(!is_security("spotify.exe", Some("Spotify AB")));
        assert!(is_caution("OneDrive.exe"));
        assert!(is_helper("msedgewebview2.exe"));
    }

    #[test]
    fn windows_dir_detection() {
        assert!(is_in_windows_dir("C:\\Windows\\System32\\notepad.exe"));
        assert!(!is_in_windows_dir("C:\\Program Files\\App\\app.exe"));
        assert!(!is_in_windows_dir("C:\\WindowsApps\\x.exe"));
    }
}
