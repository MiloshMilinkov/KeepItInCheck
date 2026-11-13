//! Rolls individual processes up into applications.
//!
//! Chrome, Edge, Discord, VS Code and every other Electron/Chromium app run as a swarm of
//! processes. We walk each process up its parent chain while the parent looks like "the same app"
//! (same exe name, a generic helper such as `msedgewebview2.exe`, or an exe inside the parent's
//! install folder). Launchers such as `explorer.exe` or a shell break the chain, so an app you
//! started from Explorer or a terminal is its own row.

use crate::model::ProcSample;
use crate::protect;
use std::collections::{HashMap, HashSet};

/// A set of processes that belong to one application.
#[derive(Debug, Clone)]
pub struct Group {
    /// Stable identity across ticks: lower-cased exe path of the root, or its name.
    pub key: String,
    /// Index (into the sample slice) of the root process used for name, path and icon.
    pub root: usize,
    /// Indices (into the sample slice) of every member, root included.
    pub members: Vec<usize>,
}

/// Lower-case directory part of an exe path, including the trailing separator.
fn exe_dir(exe: &str) -> Option<String> {
    let idx = exe.rfind(['\\', '/'])?;
    let dir = exe[..=idx].to_lowercase();
    // Never treat a shallow folder like `C:\` or `C:\Program Files\` as an install dir.
    let depth = dir.matches(['\\', '/']).count();
    (depth >= 3).then_some(dir)
}

fn same_app(child: &ProcSample, parent: &ProcSample) -> bool {
    if child.name.eq_ignore_ascii_case(&parent.name) || protect::is_helper(&child.name) {
        return true;
    }
    match (&child.exe, parent.exe.as_deref().and_then(exe_dir)) {
        (Some(c), Some(pdir)) => c.to_lowercase().starts_with(&pdir),
        _ => false,
    }
}

/// Index of the live parent of `procs[i]`, guarding against PID reuse: a parent must have
/// started no later than its child.
fn parent_of(procs: &[ProcSample], by_pid: &HashMap<u32, usize>, i: usize) -> Option<usize> {
    let p = &procs[i];
    if p.ppid == 0 || p.ppid == p.pid {
        return None;
    }
    let &j = by_pid.get(&p.ppid)?;
    (procs[j].start_time <= p.start_time).then_some(j)
}

/// PID -> index into the sample slice. Built once per tick and shared by the functions below.
pub type PidIndex = HashMap<u32, usize>;

pub fn pid_index(procs: &[ProcSample]) -> PidIndex {
    procs.iter().enumerate().map(|(i, p)| (p.pid, i)).collect()
}

/// Stable identity of an app: lower-cased exe path, or the process name when the path is hidden.
pub fn group_key(p: &ProcSample) -> String {
    p.exe.as_deref().unwrap_or(&p.name).to_lowercase()
}

/// Group processes into applications.
pub fn group(procs: &[ProcSample], by_pid: &PidIndex) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();

    for i in 0..procs.len() {
        let mut cur = i;
        // Bounded walk: PID reuse can in theory create cycles.
        for _ in 0..32 {
            let Some(parent) = parent_of(procs, by_pid, cur) else { break };
            let pp = &procs[parent];
            if protect::is_launcher(&pp.name) || protect::is_critical(&pp.name) {
                break;
            }
            if !same_app(&procs[cur], pp) {
                break;
            }
            cur = parent;
        }
        let key = group_key(&procs[cur]);
        match by_key.get(&key) {
            Some(&g) => groups[g].members.push(i),
            None => {
                by_key.insert(key.clone(), groups.len());
                groups.push(Group { key, root: cur, members: vec![i] });
            }
        }
    }

    // When several roots share a key (two separate Notepad instances), prefer the oldest one
    // as the representative so the name/icon do not flicker between ticks.
    for g in &mut groups {
        if let Some(&oldest) = g
            .members
            .iter()
            .filter(|&&m| group_key(&procs[m]) == g.key)
            .min_by_key(|&&m| (procs[m].start_time, procs[m].pid))
        {
            g.root = oldest;
        }
    }
    groups
}

/// KeepItInCheck's own process tree: `root_pid` plus descendants reached *only* through
/// WebView2 helper processes. Apps we start ourselves (Relaunch) are children of ours too, but
/// they are the user's apps and must not inherit our "self" protection.
pub fn own_tree(procs: &[ProcSample], by_pid: &PidIndex, root_pid: u32) -> HashSet<u32> {
    let mut out = HashSet::from([root_pid]);
    for i in 0..procs.len() {
        let mut cur = i;
        for _ in 0..16 {
            if !procs[cur].name.eq_ignore_ascii_case("msedgewebview2.exe") {
                break;
            }
            match parent_of(procs, by_pid, cur) {
                Some(p) if procs[p].pid == root_pid => {
                    out.insert(procs[i].pid);
                    break;
                }
                Some(p) => cur = p,
                None => break,
            }
        }
    }
    out
}

/// PIDs of `pid` and all of its live ancestors.
pub fn ancestors(procs: &[ProcSample], by_pid: &PidIndex, pid: u32) -> HashSet<u32> {
    let mut out = HashSet::new();
    let Some(&start) = by_pid.get(&pid) else { return out };
    let mut cur = start;
    out.insert(pid);
    for _ in 0..64 {
        match parent_of(procs, by_pid, cur) {
            Some(p) if out.insert(procs[p].pid) => cur = p,
            _ => break,
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn proc(pid: u32, ppid: u32, name: &str, exe: Option<&str>, start: u64) -> ProcSample {
        ProcSample {
            pid,
            ppid,
            name: name.into(),
            exe: exe.map(Into::into),
            start_time: start,
            ..Default::default()
        }
    }

    fn names(procs: &[ProcSample], g: &Group) -> Vec<u32> {
        let mut v: Vec<u32> = g.members.iter().map(|&m| procs[m].pid).collect();
        v.sort();
        v
    }

    #[test]
    fn chromium_children_roll_up_under_one_app() {
        let chrome = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
        let procs = vec![
            proc(10, 4, "explorer.exe", Some(r"C:\Windows\explorer.exe"), 1),
            proc(100, 10, "chrome.exe", Some(chrome), 5),
            proc(101, 100, "chrome.exe", Some(chrome), 6),
            proc(102, 100, "chrome.exe", Some(chrome), 7),
            proc(103, 100, "crashpad_handler.exe", None, 8),
        ];
        let groups = group(&procs, &pid_index(&procs));
        let g = groups.iter().find(|g| g.key == chrome.to_lowercase()).unwrap();
        assert_eq!(names(&procs, g), vec![100, 101, 102, 103]);
        assert_eq!(procs[g.root].pid, 100);
    }

    #[test]
    fn webview2_rolls_into_host_app() {
        let procs = vec![
            proc(10, 4, "explorer.exe", None, 1),
            proc(200, 10, "ms-teams.exe", Some(r"C:\Program Files\WindowsApps\Teams\ms-teams.exe"), 5),
            proc(201, 200, "msedgewebview2.exe", Some(r"C:\Program Files (x86)\Microsoft\EdgeWebView\msedgewebview2.exe"), 6),
            proc(202, 201, "msedgewebview2.exe", Some(r"C:\Program Files (x86)\Microsoft\EdgeWebView\msedgewebview2.exe"), 7),
        ];
        let groups = group(&procs, &pid_index(&procs));
        let teams = groups.iter().find(|g| g.key.ends_with("ms-teams.exe")).unwrap();
        assert_eq!(names(&procs, teams), vec![200, 201, 202]);
    }

    #[test]
    fn launchers_break_the_chain() {
        let procs = vec![
            proc(10, 4, "explorer.exe", Some(r"C:\Windows\explorer.exe"), 1),
            proc(20, 10, "notepad.exe", Some(r"C:\Windows\System32\notepad.exe"), 2),
            proc(30, 10, "pwsh.exe", Some(r"C:\Program Files\PowerShell\7\pwsh.exe"), 3),
            proc(31, 30, "node.exe", Some(r"C:\Program Files\nodejs\node.exe"), 4),
        ];
        let groups = group(&procs, &pid_index(&procs));
        assert_eq!(groups.len(), 4, "explorer, notepad, pwsh and node are separate apps");
    }

    #[test]
    fn install_folder_children_join_parent() {
        let procs = vec![
            proc(10, 4, "explorer.exe", None, 1),
            proc(40, 10, "steam.exe", Some(r"C:\Program Files (x86)\Steam\steam.exe"), 2),
            proc(41, 40, "steamwebhelper.exe", Some(r"C:\Program Files (x86)\Steam\bin\cef\steamwebhelper.exe"), 3),
        ];
        let groups = group(&procs, &pid_index(&procs));
        let steam = groups.iter().find(|g| g.key.ends_with("steam.exe")).unwrap();
        assert_eq!(names(&procs, steam), vec![40, 41]);
    }

    #[test]
    fn pid_reuse_does_not_link_unrelated_processes() {
        // Parent 50 started *after* the child, so it is a recycled PID, not the real parent.
        let procs = vec![
            proc(50, 4, "game.exe", Some(r"D:\Games\X\game.exe"), 100),
            proc(60, 50, "game.exe", Some(r"D:\Games\X\game.exe"), 10),
        ];
        let groups = group(&procs, &pid_index(&procs));
        let g = &groups[0];
        assert_eq!(g.members.len(), 2, "same exe path still shares a row");
        assert_eq!(procs[g.root].pid, 60, "oldest instance is the representative");
    }

    #[test]
    fn own_tree_covers_webview_helpers_but_not_relaunched_apps() {
        let procs = vec![
            proc(100, 10, "keepitincheck.exe", None, 1),
            proc(101, 100, "msedgewebview2.exe", None, 2),
            proc(102, 101, "msedgewebview2.exe", None, 3),
            proc(103, 100, "spotify.exe", None, 4), // relaunched by the user from our log
            proc(104, 103, "msedgewebview2.exe", None, 5), // that app's own WebView2
        ];
        assert_eq!(own_tree(&procs, &pid_index(&procs), 100), HashSet::from([100, 101, 102]));
    }

    #[test]
    fn ancestors_walk_up_the_chain() {
        let procs = vec![
            proc(1, 0, "a.exe", None, 1),
            proc(2, 1, "b.exe", None, 2),
            proc(3, 2, "c.exe", None, 3),
            proc(4, 1, "d.exe", None, 4),
            proc(5, 0, "e.exe", None, 5),
        ];
        let a = ancestors(&procs, &pid_index(&procs), 3);
        assert_eq!(a, HashSet::from([1, 2, 3]));
    }
}
