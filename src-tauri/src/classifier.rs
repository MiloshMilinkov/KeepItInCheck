//! The dead-weight classifier: pure, deterministic, unit-tested.
//!
//! Input is everything we know about one app group ([`Facts`]); output is a [`Verdict`] with a
//! class (Safe / Probable / Keep), a 0-100 score and a plain-English reason. The rules are
//! documented for users in `docs/dead-weight-score.md`; keep the two in sync.

use crate::history::GroupStats;
use crate::model::{Class, Verdict};
use crate::settings::Thresholds;

/// Below this much observation we do not suggest anything at all.
pub const MIN_OBSERVE_SECS: u64 = 120;

/// Why an app is on the protected list. Protected apps are never suggested or closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// KeepItInCheck itself or one of its WebView2 helpers.
    SelfApp,
    /// Core Windows process (csrss, lsass, dwm, explorer, ...).
    Critical,
    /// Runs in session 0, where services and the kernel live.
    SystemSession,
    /// Hosts a Windows service.
    Service,
    /// Antivirus or endpoint security.
    Security,
    /// Authenticode-signed by Microsoft.
    MicrosoftSigned,
    /// Owns or is a parent of the window you are using right now.
    ForegroundChain,
    /// We cannot read its path (elevated or access denied), so we cannot reason about it.
    Inaccessible,
}

impl Protection {
    pub fn label(self) -> &'static str {
        match self {
            Protection::SelfApp => "Protected: KeepItInCheck itself",
            Protection::Critical => "Protected: core Windows process",
            Protection::SystemSession => "Protected: runs as a system process",
            Protection::Service => "Protected: Windows service",
            Protection::Security => "Protected: security software",
            Protection::MicrosoftSigned => "Protected: Microsoft-signed component",
            Protection::ForegroundChain => "In use: hosts the window you're using",
            Protection::Inaccessible => "Protected: runs elevated, can't be inspected",
        }
    }
}

/// Everything the classifier knows about one app group.
#[derive(Debug, Clone)]
pub struct Facts {
    pub mem: u64,
    pub has_window: bool,
    pub stats: GroupStats,
    pub protection: Option<Protection>,
    pub excluded: bool,
    pub snoozed: bool,
    /// Signature check still running; we wait rather than guess.
    pub signature_pending: bool,
    /// Sync/VPN/password-manager style app: never above "Probably safe".
    pub caution: bool,
}

fn keep(reason: impl Into<String>, protected: bool) -> Verdict {
    Verdict { class: Class::Keep, score: 0, reason: reason.into(), protected }
}

pub fn classify(f: &Facts, t: &Thresholds) -> Verdict {
    if let Some(p) = f.protection {
        return keep(p.label(), true);
    }
    if f.excluded {
        return keep("On your never-touch list", false);
    }
    if f.snoozed {
        return keep("Suggestions snoozed", false);
    }
    if f.signature_pending {
        return keep("Checking publisher signature…", false);
    }

    let min_ram = t.min_ram_bytes();
    if f.mem < min_ram {
        return keep(format!("Small: {} (under {} MB)", fmt_mem(f.mem), t.min_ram_mb), false);
    }

    let s = &f.stats;
    let window = t.window_secs();
    if s.observed_secs < MIN_OBSERVE_SECS || s.covered_secs < 60 {
        return keep(format!("Watching… ({} so far)", fmt_dur(s.observed_secs)), false);
    }
    if s.since_audio.is_some_and(|a| a < window) {
        return keep("Played audio recently", false);
    }
    if s.avg_cpu_pct >= t.idle_cpu_pct {
        return keep(format!("Active: {:.1}% CPU on average", s.avg_cpu_pct), false);
    }
    let max_io = t.max_io_bps();
    if s.io_bps >= max_io && s.io_bps > 0.0 {
        return keep(format!("Busy: {}/s disk or network", fmt_mem(s.io_bps as u64)), false);
    }

    let unfocused_limit = t.unfocused_secs();
    // Never seen in the foreground while observed: it has been unfocused at least that long.
    let unfocused = s.since_foreground.unwrap_or(s.observed_secs);
    if f.has_window && unfocused < unfocused_limit {
        return match s.since_foreground {
            Some(secs) => keep(format!("In use: focused {} ago", fmt_dur(secs)), false),
            None => keep(
                format!(
                    "Has a window: watching focus ({} of {} min)",
                    unfocused / 60,
                    t.unfocused_mins
                ),
                false,
            ),
        };
    }

    // Every hard gate passed: this app is dead weight. Now decide how confident we are.
    let score = score(f, t, unfocused, unfocused_limit, window);
    let full_window = s.covered_secs + crate::history::CHECKPOINT_SECS >= window;
    let class = if !f.has_window && full_window && !f.caution && score >= t.safe_score {
        Class::Safe
    } else {
        Class::Probable
    };

    Verdict { class, score, reason: reason(f, unfocused, full_window), protected: false }
}

/// 0-100. CPU 30, window 25, RAM 20, I/O 15, audio 10, scaled down while still learning.
fn score(f: &Facts, t: &Thresholds, unfocused: u64, unfocused_limit: u64, window: u64) -> u8 {
    let s = &f.stats;
    let cpu = 30.0 * (1.0 - (s.avg_cpu_pct / t.idle_cpu_pct) as f64).clamp(0.0, 1.0);
    let max_io = t.max_io_bps();
    let io = if max_io <= 0.0 {
        15.0
    } else {
        15.0 * (1.0 - s.io_bps / max_io).clamp(0.0, 1.0)
    };
    let win = if !f.has_window {
        25.0
    } else {
        // 15 at the unfocused limit, rising to 25 at twice the limit.
        let extra = (unfocused.saturating_sub(unfocused_limit)) as f64 / unfocused_limit.max(1) as f64;
        15.0 + 10.0 * extra.clamp(0.0, 1.0)
    };
    let audio = 10.0;
    let min_mb = (t.min_ram_mb.max(1)) as f64;
    let mem_mb = f.mem as f64 / (1024.0 * 1024.0);
    let ram = if min_mb >= 1024.0 {
        20.0
    } else {
        20.0 * ((mem_mb / min_mb).ln() / (1024.0 / min_mb).ln()).clamp(0.0, 1.0)
    };
    let confidence = (s.covered_secs as f64 / window.max(1) as f64).clamp(0.5, 1.0);
    ((cpu + io + win + audio + ram) * confidence).round().clamp(0.0, 100.0) as u8
}

fn reason(f: &Facts, unfocused: u64, full_window: bool) -> String {
    let s = &f.stats;
    let mut parts = Vec::with_capacity(4);
    let idle_for = s.idle_secs.min(s.observed_secs);
    if idle_for + 60 >= s.covered_secs {
        let so_far = if full_window { "" } else { " so far" };
        parts.push(format!("Idle for {}{so_far}", fmt_dur(idle_for)));
    } else {
        parts.push(format!("Mostly idle ({:.2}% CPU avg)", s.avg_cpu_pct));
    }
    if f.has_window {
        parts.push(format!("window unused for {}", fmt_dur(unfocused)));
    } else {
        parts.push("no window".into());
    }
    parts.push(format!("using {}", fmt_mem(f.mem)));
    let mut out = parts.join(", ");
    if f.caution {
        out.push_str(" · may sync or work in the background");
    }
    out
}

/// "45 s", "47 min", "2 h 5 min".
pub fn fmt_dur(secs: u64) -> String {
    if secs < 60 {
        format!("{secs} s")
    } else if secs < 3600 {
        format!("{} min", secs / 60)
    } else {
        let (h, m) = (secs / 3600, (secs % 3600) / 60);
        if m == 0 { format!("{h} h") } else { format!("{h} h {m} min") }
    }
}

/// "312 MB", "1.4 GB".
pub fn fmt_mem(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else if mb >= 1.0 {
        format!("{} MB", mb.round() as u64)
    } else {
        format!("{} KB", (bytes as f64 / 1024.0).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    /// A background app that has done nothing for 47 minutes.
    fn idle_background() -> Facts {
        Facts {
            mem: 312 * MB,
            has_window: false,
            stats: GroupStats {
                avg_cpu_pct: 0.01,
                io_bps: 0.0,
                covered_secs: 600,
                observed_secs: 47 * 60,
                idle_secs: 47 * 60,
                since_foreground: None,
                since_audio: None,
            },
            protection: None,
            excluded: false,
            snoozed: false,
            signature_pending: false,
            caution: false,
        }
    }

    fn t() -> Thresholds {
        Thresholds::default()
    }

    // ---- Protected ---------------------------------------------------------------------------

    #[test]
    fn every_protection_reason_is_keep_and_protected() {
        for p in [
            Protection::SelfApp,
            Protection::Critical,
            Protection::SystemSession,
            Protection::Service,
            Protection::Security,
            Protection::MicrosoftSigned,
            Protection::ForegroundChain,
            Protection::Inaccessible,
        ] {
            let mut f = idle_background();
            f.protection = Some(p);
            let v = classify(&f, &t());
            assert_eq!(v.class, Class::Keep, "{p:?}");
            assert!(v.protected, "{p:?}");
            assert_eq!(v.reason, p.label());
        }
    }

    #[test]
    fn protection_beats_a_perfect_dead_weight_profile() {
        let mut f = idle_background();
        f.mem = 4096 * MB;
        f.protection = Some(Protection::Critical);
        assert_eq!(classify(&f, &t()).class, Class::Keep);
    }

    #[test]
    fn exclusion_and_snooze_keep_without_protecting() {
        let mut f = idle_background();
        f.excluded = true;
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Keep);
        assert!(!v.protected);

        let mut f = idle_background();
        f.snoozed = true;
        assert_eq!(classify(&f, &t()).class, Class::Keep);
    }

    #[test]
    fn waits_for_signature_check() {
        let mut f = idle_background();
        f.signature_pending = true;
        assert_eq!(classify(&f, &t()).class, Class::Keep);
    }

    // ---- Safe --------------------------------------------------------------------------------

    #[test]
    fn idle_background_app_is_safe_with_plain_reason() {
        let v = classify(&idle_background(), &t());
        assert_eq!(v.class, Class::Safe);
        assert_eq!(v.reason, "Idle for 47 min, no window, using 312 MB");
        assert!(v.score >= t().safe_score, "score {}", v.score);
    }

    #[test]
    fn huge_idle_app_scores_higher_than_small_one() {
        let mut big = idle_background();
        big.mem = 2048 * MB;
        let mut small = idle_background();
        small.mem = 60 * MB;
        assert!(classify(&big, &t()).score > classify(&small, &t()).score);
    }

    // ---- Probably safe -----------------------------------------------------------------------

    #[test]
    fn windowed_app_unfocused_for_long_is_only_probable() {
        let mut f = idle_background();
        f.has_window = true;
        f.stats.since_foreground = Some(52 * 60);
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Probable);
        assert_eq!(v.reason, "Idle for 47 min, window unused for 52 min, using 312 MB");
    }

    #[test]
    fn caution_apps_are_capped_at_probable() {
        let mut f = idle_background();
        f.caution = true;
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Probable);
        assert!(v.reason.ends_with("may sync or work in the background"));
    }

    #[test]
    fn partial_observation_is_probable_and_says_so() {
        let mut f = idle_background();
        f.stats.observed_secs = 5 * 60;
        f.stats.idle_secs = 5 * 60;
        f.stats.covered_secs = 5 * 60;
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Probable);
        assert!(v.reason.starts_with("Idle for 5 min so far"), "{}", v.reason);
    }

    #[test]
    fn borderline_cpu_and_small_ram_scores_low_and_is_probable() {
        let mut f = idle_background();
        f.mem = 55 * MB;
        f.stats.avg_cpu_pct = 0.29;
        f.stats.io_bps = 19.0 * 1024.0;
        f.stats.idle_secs = 30;
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Probable);
        assert!(v.score < t().safe_score);
        assert!(v.reason.starts_with("Mostly idle"), "{}", v.reason);
    }

    // ---- Keep (not flagged) ------------------------------------------------------------------

    #[test]
    fn small_apps_are_ignored() {
        let mut f = idle_background();
        f.mem = 20 * MB;
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Keep);
        assert!(v.reason.starts_with("Small"));
    }

    #[test]
    fn busy_cpu_is_kept() {
        let mut f = idle_background();
        f.stats.avg_cpu_pct = 1.4;
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Keep);
        assert_eq!(v.reason, "Active: 1.4% CPU on average");
    }

    #[test]
    fn io_activity_is_kept() {
        let mut f = idle_background();
        f.stats.io_bps = 500.0 * 1024.0;
        assert_eq!(classify(&f, &t()).class, Class::Keep);
    }

    #[test]
    fn audio_is_kept() {
        let mut f = idle_background();
        f.stats.since_audio = Some(90);
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Keep);
        assert_eq!(v.reason, "Played audio recently");
    }

    #[test]
    fn recently_focused_window_is_kept() {
        let mut f = idle_background();
        f.has_window = true;
        f.stats.since_foreground = Some(3 * 60);
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Keep);
        assert_eq!(v.reason, "In use: focused 3 min ago");
    }

    #[test]
    fn windowed_app_needs_the_full_unfocused_period() {
        let mut f = idle_background();
        f.has_window = true;
        f.stats.observed_secs = 12 * 60;
        let v = classify(&f, &t());
        assert_eq!(v.class, Class::Keep);
        assert!(v.reason.contains("watching focus (12 of 30 min)"), "{}", v.reason);
    }

    #[test]
    fn nothing_is_suggested_in_the_first_two_minutes() {
        let mut f = idle_background();
        f.stats.observed_secs = 60;
        assert_eq!(classify(&f, &t()).class, Class::Keep);
    }

    #[test]
    fn thresholds_are_respected() {
        let mut f = idle_background();
        f.stats.avg_cpu_pct = 0.5;
        assert_eq!(classify(&f, &t()).class, Class::Keep);
        let mut loose = t();
        loose.idle_cpu_pct = 1.0;
        assert_ne!(classify(&f, &loose).class, Class::Keep);
    }

    #[test]
    fn formatting() {
        assert_eq!(fmt_dur(45), "45 s");
        assert_eq!(fmt_dur(47 * 60), "47 min");
        assert_eq!(fmt_dur(2 * 3600 + 5 * 60), "2 h 5 min");
        assert_eq!(fmt_mem(312 * MB), "312 MB");
        assert_eq!(fmt_mem(1536 * MB), "1.5 GB");
    }
}
