//! The background sampler: one thread, one tick per interval.
//!
//! Each tick reads the process table (one syscall on Windows), window/audio facts, rolls
//! processes up into apps and updates the dead-weight history. While the panel is open it also
//! classifies every app and publishes a [`Snapshot`]; while collapsed it stops there, because
//! nothing would read the result. Polling is adaptive: fast while open, slow while collapsed,
//! stopped while paused. Authenticode checks run on a separate low-priority worker.

use crate::classifier::{self, Facts, Protection};
use crate::grouping::{self, Group};
use crate::history::{self, History, TickSignals};
use crate::model::{AppView, Class, ProcSample, ProcView, Pulse, Snapshot, HIGH_LOAD_PCT};
use crate::platform::{self, Platform, Signature};
use crate::protect;
use crate::settings::Settings;
use crate::util::{strip_exe, unix_millis, unix_secs};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};
use sysinfo::MemoryRefreshKind;
use tauri::{AppHandle, Emitter};

const SERVICES_EVERY: Duration = Duration::from_secs(30);
const SPARK_SECS: u64 = 60;

pub enum Msg {
    /// Run a tick now (panel opened, settings changed, cleanup needs fresh data).
    Wake,
    Quit,
}

/// State shared between the sampler, commands and window events.
pub struct Shared {
    pub settings: RwLock<Settings>,
    pub settings_path: PathBuf,
    pub latest: Mutex<Option<Arc<Snapshot>>>,
    /// Group key -> `(pid, start_time)` of its processes, from the latest full tick. Cleanup
    /// combines this with the matching [`AppView`] in `latest`.
    pub members: Mutex<Arc<HashMap<String, Vec<(u32, u64)>>>>,
    pub expanded: AtomicBool,
    pub paused: AtomicBool,
    pub cleanup_running: AtomicBool,
    /// Group key of the row expanded in the UI (gets per-process detail).
    pub detail_key: Mutex<Option<String>>,
    /// Exe path -> signed by Microsoft. Filled by the signature worker.
    signatures: Mutex<HashMap<String, bool>>,
    /// Set by `fresh_tick` so even a collapsed tick builds the full snapshot.
    want_full: AtomicBool,
    tick: (Mutex<u64>, Condvar),
    tx: Mutex<Option<Sender<Msg>>>,
}

impl Shared {
    pub fn new(settings: Settings, settings_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            settings: RwLock::new(settings),
            settings_path,
            latest: Mutex::new(None),
            members: Mutex::new(Arc::new(HashMap::new())),
            expanded: AtomicBool::new(true),
            paused: AtomicBool::new(false),
            cleanup_running: AtomicBool::new(false),
            detail_key: Mutex::new(None),
            signatures: Mutex::new(HashMap::new()),
            want_full: AtomicBool::new(false),
            tick: (Mutex::new(0), Condvar::new()),
            tx: Mutex::new(None),
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    /// The one way settings change: sanitize, persist and store under a single write lock, so a
    /// UI save and a drag-to-snap can never overwrite each other. Returns `(old, new)`.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) -> std::io::Result<(Settings, Settings)> {
        let mut guard = self.settings.write().unwrap();
        let old = guard.clone();
        let mut next = old.clone();
        f(&mut next);
        let next = next.sanitized();
        next.save(&self.settings_path)?;
        *guard = next.clone();
        drop(guard);
        self.wake();
        Ok((old, next))
    }

    pub fn wake(&self) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(Msg::Wake);
        }
    }

    pub fn quit(&self) {
        if let Some(tx) = self.tx.lock().unwrap().take() {
            let _ = tx.send(Msg::Quit);
        }
    }

    /// Run a full tick now and wait (up to `timeout`) until it has been published.
    pub fn fresh_tick(&self, timeout: Duration) {
        let start = *self.tick.0.lock().unwrap();
        self.want_full.store(true, Ordering::SeqCst);
        self.wake();
        let guard = self.tick.0.lock().unwrap();
        let _ = self.tick.1.wait_timeout_while(guard, timeout, |n| *n == start);
    }

    fn is_microsoft(&self, exe: &str) -> Option<bool> {
        self.signatures.lock().unwrap().get(exe).copied()
    }
}

/// Start the sampler and signature worker threads.
pub fn spawn(app: AppHandle, shared: Arc<Shared>) {
    let (tx, rx) = mpsc::channel();
    *shared.tx.lock().unwrap() = Some(tx);
    let (sig_tx, sig_rx) = mpsc::channel::<String>();

    let sh = shared.clone();
    std::thread::Builder::new()
        .name("kic-signatures".into())
        .spawn(move || signature_worker(sh, sig_rx))
        .expect("spawn signature worker");

    std::thread::Builder::new()
        .name("kic-sampler".into())
        .spawn(move || run(app, shared, rx, sig_tx))
        .expect("spawn sampler");
}

fn signature_worker(shared: Arc<Shared>, rx: Receiver<String>) {
    lower_thread_priority();
    let mut plat = platform::create();
    while let Ok(exe) = rx.recv() {
        let microsoft = plat.signature(&exe) == Signature::Microsoft;
        shared.signatures.lock().unwrap().insert(exe, microsoft);
        // Spread the work out: verification is CPU-heavy for large binaries.
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(windows)]
fn lower_thread_priority() {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN};
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}
#[cfg(not(windows))]
fn lower_thread_priority() {}

fn run(app: AppHandle, shared: Arc<Shared>, rx: Receiver<Msg>, sig_tx: Sender<String>) {
    let mut s = Sampler::new(sig_tx);
    loop {
        // A requested fresh tick (cleanup) runs even while paused, so it never acts on stale data.
        if !shared.paused.load(Ordering::Relaxed) || s.ticks == 0 || shared.want_full.load(Ordering::SeqCst) {
            s.tick(&app, &shared);
        }
        let interval = {
            let st = shared.settings.read().unwrap();
            if shared.paused.load(Ordering::Relaxed) {
                Duration::from_secs(3600)
            } else if shared.expanded.load(Ordering::Relaxed) {
                Duration::from_millis(st.refresh_visible_ms as u64)
            } else {
                Duration::from_millis(st.refresh_hidden_ms as u64)
            }
        };
        match rx.recv_timeout(interval) {
            Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Msg::Wake) | Err(RecvTimeoutError::Timeout) => {
                // Coalesce a burst of wake-ups into one tick.
                while let Ok(m) = rx.try_recv() {
                    if matches!(m, Msg::Quit) {
                        return;
                    }
                }
            }
        }
    }
}

/// Display name and vendor of an exe, resolved once.
struct ExeInfo {
    name: String,
    company: Option<String>,
}

struct Sampler {
    plat: Box<dyn Platform>,
    sys: Option<sysinfo::System>,
    cores: u32,
    prev_cpu: HashMap<u32, (u64, u64)>,
    prev_wall: Option<Instant>,
    exe_cache: HashMap<(u32, u64), Option<String>>,
    info_cache: HashMap<String, ExeInfo>,
    services: HashSet<u32>,
    services_at: Option<Instant>,
    history: History,
    spark: VecDeque<(u64, f32, f32)>,
    started: u64,
    last_external_fg: Option<u32>,
    sig_tx: Sender<String>,
    sig_requested: HashSet<String>,
    /// Last high-load state sent to the collapsed strip (None: send on the next collapsed tick).
    last_hot: Option<bool>,
    self_pid: u32,
    ticks: u64,
    /// Debug aid: `KIC_DUMP=path` writes every snapshot as JSON to that file.
    dump: Option<PathBuf>,
}

/// Per-tick facts shared by every group while building the view.
struct TickCtx<'a> {
    procs: &'a [ProcSample],
    windowed: &'a HashSet<u32>,
    self_tree: &'a HashSet<u32>,
    fg_chain: &'a HashSet<u32>,
    settings: &'a Settings,
    now: u64,
}

impl Sampler {
    fn new(sig_tx: Sender<String>) -> Self {
        let mut sys = sysinfo::System::new().ok();
        if let Some(sys) = sys.as_mut() {
            sys.refresh_cpu_usage();
        }
        let cores = sys
            .as_ref()
            .map(|s| s.cpus().len() as u32)
            .filter(|&n| n > 0)
            .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1));
        Self {
            plat: platform::create(),
            sys,
            cores,
            prev_cpu: HashMap::new(),
            prev_wall: None,
            exe_cache: HashMap::new(),
            info_cache: HashMap::new(),
            services: HashSet::new(),
            services_at: None,
            history: History::new(cores),
            spark: VecDeque::with_capacity(64),
            started: unix_secs(),
            last_external_fg: None,
            sig_tx,
            sig_requested: HashSet::new(),
            last_hot: None,
            self_pid: std::process::id(),
            ticks: 0,
            dump: std::env::var_os("KIC_DUMP").map(PathBuf::from),
        }
    }

    /// Process table with per-tick CPU %, from the platform fast path or `sysinfo`.
    fn read_processes(&mut self) -> Vec<ProcSample> {
        let mut procs = match self.plat.process_table() {
            Some(p) => p,
            None => self.sysinfo_processes(),
        };
        let now = Instant::now();
        let wall_ms = self.prev_wall.map(|t| now.duration_since(t).as_millis() as f64).unwrap_or(0.0);
        let mut next = HashMap::with_capacity(procs.len());
        for p in &mut procs {
            if let Some(&(st, prev)) = self.prev_cpu.get(&p.pid) {
                if st == p.start_time {
                    p.cpu = history::machine_pct(p.cpu_time_ms.saturating_sub(prev) as f64, wall_ms, self.cores);
                }
            }
            next.insert(p.pid, (p.start_time, p.cpu_time_ms));
            if p.exe.is_none() {
                let (pid, plat) = (p.pid, &mut self.plat);
                p.exe = self.exe_cache.entry((pid, p.start_time)).or_insert_with(|| plat.exe_path(pid)).clone();
            }
        }
        // Forget paths of processes that are gone.
        self.exe_cache.retain(|(pid, st), _| next.get(pid).is_some_and(|(s, _)| s == st));
        self.prev_cpu = next;
        self.prev_wall = Some(now);
        procs
    }

    fn sysinfo_processes(&mut self) -> Vec<ProcSample> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        let Some(sys) = self.sys.as_mut() else { return Vec::new() };
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_exe(UpdateKind::OnlyIfNotSet)
                .without_tasks(),
        );
        sys.processes()
            .values()
            .map(|p| {
                let d = p.disk_usage();
                ProcSample {
                    pid: p.pid().as_u32(),
                    ppid: p.parent().map(|x| x.as_u32()).unwrap_or(0),
                    name: p.name().to_string_lossy().into_owned(),
                    exe: p.exe().map(|e| e.to_string_lossy().into_owned()),
                    start_time: p.start_time(),
                    session_id: None,
                    cpu: 0.0,
                    cpu_time_ms: p.accumulated_cpu_time(),
                    mem: p.memory(),
                    io_bytes: d.total_read_bytes + d.total_written_bytes,
                }
            })
            .collect()
    }

    /// Readable name and vendor for an app, resolved once per exe.
    fn exe_info(&mut self, exe: Option<&str>, name: &str) -> (String, Option<String>) {
        let fallback = || protect::friendly_name(name).unwrap_or(strip_exe(name)).to_string();
        let Some(exe) = exe else { return (fallback(), None) };
        if let Some(i) = self.info_cache.get(exe) {
            return (i.name.clone(), i.company.clone());
        }
        let info = self.plat.file_info(exe);
        let pretty = info
            .description
            .as_deref()
            .filter(|d| d.len() <= 48 && !d.eq_ignore_ascii_case("electron"))
            .or(info.product.as_deref().filter(|p| !p.contains("Operating System")))
            .filter(|d| d.len() <= 48)
            .map(|d| strip_exe(d).to_string())
            .unwrap_or_else(fallback);
        let e = ExeInfo { name: pretty, company: info.company };
        let out = (e.name.clone(), e.company.clone());
        self.info_cache.insert(exe.to_string(), e);
        out
    }

    fn tick(&mut self, app: &AppHandle, shared: &Shared) {
        let settings = shared.settings();
        let t = &settings.thresholds;
        let now = unix_secs();

        // System-wide CPU and RAM from sysinfo (RAM only: swap figures are never shown).
        let (cpu, mem_used, mem_total) = match self.sys.as_mut() {
            Some(sys) => {
                sys.refresh_cpu_usage();
                sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
                let total = sys.total_memory();
                (sys.global_cpu_usage(), total.saturating_sub(sys.available_memory()), total)
            }
            None => (0.0, 0, 1),
        };
        let mem_pct = if mem_total > 0 { mem_used as f32 / mem_total as f32 * 100.0 } else { 0.0 };
        let now_ms = unix_millis();
        self.spark.push_back((now_ms, cpu, mem_pct));
        while self.spark.front().is_some_and(|(t, _, _)| now_ms.saturating_sub(*t) > SPARK_SECS * 1000) {
            self.spark.pop_front();
        }

        let expanded = shared.expanded.load(Ordering::Relaxed);
        let full = expanded || shared.want_full.swap(false, Ordering::SeqCst);
        if expanded {
            self.last_hot = None;
        } else {
            // The strip only changes colour under high load, so only tell it when that flips.
            let hot = cpu >= HIGH_LOAD_PCT || mem_pct >= HIGH_LOAD_PCT;
            if self.last_hot != Some(hot) {
                self.last_hot = Some(hot);
                let _ = app.emit("pulse", Pulse { hot });
            }
        }

        let procs = self.read_processes();
        let wins = self.plat.windows();
        let audio_pids = self.plat.audio_pids();
        if self.services_at.is_none_or(|t| t.elapsed() > SERVICES_EVERY) {
            self.services = self.plat.service_pids();
            self.services_at = Some(Instant::now());
        }

        let by_pid = grouping::pid_index(&procs);
        let groups = grouping::group(&procs, &by_pid);
        let self_tree = grouping::own_tree(&procs, &by_pid, self.self_pid);
        // Clicking our own panel makes us the foreground app; remember the user's real one.
        if let Some(fg) = wins.foreground_pid.filter(|p| !self_tree.contains(p)) {
            self.last_external_fg = Some(fg);
        }

        let mut group_of_pid: HashMap<u32, usize> = HashMap::with_capacity(procs.len());
        for (gi, g) in groups.iter().enumerate() {
            for &m in &g.members {
                group_of_pid.insert(procs[m].pid, gi);
            }
        }
        let fg_key = self
            .last_external_fg
            .and_then(|p| group_of_pid.get(&p))
            .map(|&gi| groups[gi].key.as_str());
        let audio_keys: HashSet<String> = audio_pids
            .iter()
            .filter_map(|p| group_of_pid.get(p))
            .map(|&gi| groups[gi].key.clone())
            .collect();
        self.history
            .update(now, &procs, &groups, &TickSignals { foreground_key: fg_key, audio_keys: &audio_keys }, t);

        if full {
            let fg_chain = self
                .last_external_fg
                .map(|p| grouping::ancestors(&procs, &by_pid, p))
                .unwrap_or_default();
            let ctx = TickCtx {
                procs: &procs,
                windowed: &wins.windowed_pids,
                self_tree: &self_tree,
                fg_chain: &fg_chain,
                settings: &settings,
                now,
            };
            self.publish(app, shared, &ctx, &groups, cpu, mem_used, mem_total, expanded);
        }

        self.ticks += 1;
        let mut n = shared.tick.0.lock().unwrap();
        *n += 1;
        shared.tick.1.notify_all();
    }

    /// Classify every app and publish the snapshot.
    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        app: &AppHandle,
        shared: &Shared,
        ctx: &TickCtx,
        groups: &[Group],
        cpu: f32,
        mem_used: u64,
        mem_total: u64,
        emit: bool,
    ) {
        let procs = ctx.procs;
        let t = &ctx.settings.thresholds;
        let detail_key = shared.detail_key.lock().unwrap().clone();
        let mut apps = Vec::with_capacity(groups.len());
        let mut members = HashMap::with_capacity(groups.len());
        let (mut self_cpu, mut self_mem) = (0f32, 0u64);
        for p in procs.iter().filter(|p| ctx.self_tree.contains(&p.pid)) {
            self_cpu += p.cpu;
            self_mem += p.mem;
        }

        for g in groups {
            let root = &procs[g.root];
            let (cpu, mem) = g
                .members
                .iter()
                .fold((0f32, 0u64), |(c, m), &i| (c + procs[i].cpu, m + procs[i].mem));
            let has_window = g.members.iter().any(|&i| ctx.windowed.contains(&procs[i].pid));
            let (name, company) = self.exe_info(root.exe.as_deref(), &root.name);
            let (protection, signature_pending) = self.protection(g, ctx, company.as_deref(), mem, shared);
            let excluded = ctx.settings.is_excluded(&root.name, root.exe.as_deref());
            let facts = Facts {
                mem,
                has_window,
                stats: self.history.stats(&g.key, ctx.now, t.window_secs()),
                protection,
                excluded,
                snoozed: ctx.settings.snoozed.get(&g.key).is_some_and(|&until| until > ctx.now),
                signature_pending,
                caution: protect::is_caution(&root.name),
            };
            let verdict = classifier::classify(&facts, t);

            let mut detail: Vec<ProcView> = Vec::new();
            if detail_key.as_deref() == Some(g.key.as_str()) {
                detail = g
                    .members
                    .iter()
                    .map(|&i| ProcView { pid: procs[i].pid, name: procs[i].name.clone(), cpu: procs[i].cpu, mem: procs[i].mem })
                    .collect();
                detail.sort_by_key(|p| std::cmp::Reverse(p.mem));
            }
            members.insert(g.key.clone(), g.members.iter().map(|&i| (procs[i].pid, procs[i].start_time)).collect());
            apps.push(AppView {
                key: g.key.clone(),
                name,
                exe: root.exe.clone(),
                cpu,
                mem,
                has_window,
                excluded,
                proc_count: g.members.len() as u32,
                procs: detail,
                verdict,
            });
        }
        apps.sort_by_key(|a| std::cmp::Reverse(a.mem));
        let reclaimable = apps.iter().filter(|a| a.verdict.class != Class::Keep).map(|a| a.mem).sum();

        let now_ms = unix_millis();
        let snap = Arc::new(Snapshot {
            ts: now_ms,
            cpu,
            mem_used,
            mem_total,
            cores: self.cores,
            process_count: procs.len() as u32,
            history: self
                .spark
                .iter()
                .map(|&(t, c, m)| [now_ms.saturating_sub(t) as f32 / 1000.0, c, m])
                .collect(),
            apps,
            reclaimable,
            observed_secs: ctx.now.saturating_sub(self.started),
            learning_secs: t.window_secs(),
            self_cpu,
            self_mem,
        });

        *shared.members.lock().unwrap() = Arc::new(members);
        *shared.latest.lock().unwrap() = Some(snap.clone());
        if emit {
            let _ = app.emit("snapshot", &*snap);
        }
        if let Some(path) = &self.dump {
            let _ = std::fs::write(path, serde_json::to_vec_pretty(&*snap).unwrap_or_default());
        }
    }

    /// Protected-list check. Returns the protection (if any) and whether we are still waiting
    /// for the Authenticode check (only asked for apps big enough to ever be suggested).
    fn protection(
        &mut self,
        g: &Group,
        ctx: &TickCtx,
        company: Option<&str>,
        mem: u64,
        shared: &Shared,
    ) -> (Option<Protection>, bool) {
        let members = || g.members.iter().map(|&i| &ctx.procs[i]);
        let root = &ctx.procs[g.root];
        let fixed = if members().any(|p| ctx.self_tree.contains(&p.pid)) {
            Some(Protection::SelfApp)
        } else if members().any(|p| protect::is_critical(&p.name)) {
            Some(Protection::Critical)
        } else if members().any(|p| p.session_id == Some(0)) {
            Some(Protection::SystemSession)
        } else if members().any(|p| self.services.contains(&p.pid)) {
            Some(Protection::Service)
        } else if members().any(|p| protect::is_security(&p.name, None)) || protect::is_security(&root.name, company) {
            Some(Protection::Security)
        } else if members().any(|p| ctx.fg_chain.contains(&p.pid)) {
            Some(Protection::ForegroundChain)
        } else if root.exe.is_none() {
            Some(Protection::Inaccessible)
        } else {
            None
        };
        if fixed.is_some() {
            return (fixed, false);
        }
        let Some(exe) = root.exe.as_deref() else { return (None, false) };
        if mem < ctx.settings.thresholds.min_ram_bytes() {
            return (None, false);
        }
        match shared.is_microsoft(exe) {
            Some(true) if ctx.settings.protect_all_microsoft || protect::is_in_windows_dir(exe) => {
                (Some(Protection::MicrosoftSigned), false)
            }
            Some(_) => (None, false),
            None => {
                if self.sig_requested.insert(exe.to_string()) {
                    let _ = self.sig_tx.send(exe.to_string());
                }
                (None, true)
            }
        }
    }
}
