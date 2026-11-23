//! Frame-time budget on a 500-page document (parity view.large-document-performance): scrolling
//! continuously must keep the UI's own per-frame work far below a 60 fps frame. Rendering runs on
//! worker threads and is not part of the measurement.

use egui_kittest::Harness;
use localpdf_ui_egui::LocalPDFApp;

/// The one-minute load average, where the OS reports it (macOS `vm.loadavg`, Linux
/// `/proc/loadavg`). Windows has no load average; [`cpu_busy`] samples the CPU there.
#[cfg(not(windows))]
fn load_average() -> Option<f64> {
    if let Ok(s) = std::fs::read_to_string("/proc/loadavg") {
        return s.split_whitespace().next()?.parse().ok();
    }
    let out = std::process::Command::new("sysctl").args(["-n", "vm.loadavg"]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim_matches(|c: char| c == '{' || c == '}' || c.is_whitespace()).split_whitespace().next()?.parse().ok()
}

/// How much of the machine is busy, 0 to 1, measured over a short window of the CPU time counters (`GetSystemTimes`:
/// idle, kernel and user times, in 100 ns units; the kernel time includes the idle time). `None` when the counters
/// cannot be read or did not move.
#[cfg(windows)]
fn cpu_busy() -> Option<f64> {
    let sample = || {
        let (idle, kernel, user) = winsafe::GetSystemTimes().ok()?;
        Some([u64::from(idle), u64::from(kernel), u64::from(user)])
    };
    let before = sample()?;
    std::thread::sleep(CPU_SAMPLE);
    utilization(before, sample()?)
}

#[cfg(windows)]
const CPU_SAMPLE: std::time::Duration = std::time::Duration::from_millis(200);

/// The busy share of the machine between two `GetSystemTimes` samples (`idle`, `kernel`, `user`).
fn utilization(before: [u64; 3], after: [u64; 3]) -> Option<f64> {
    let delta = |i: usize| after[i].checked_sub(before[i]);
    let (idle, kernel, user) = (delta(0)?, delta(1)?, delta(2)?);
    let total = kernel + user;
    (total > 0).then(|| (total.saturating_sub(idle) as f64 / total as f64).clamp(0.0, 1.0))
}

/// A CPU busier than this share of the machine is another build running.
const BUSY: f64 = 0.5;

fn is_busy(share: f64) -> bool {
    share > BUSY
}

/// Frame budgets only mean something on a machine that isn't overcommitted: with the load average above twice the
/// core count (other builds running), skip and say so. Windows reports no load average, so there the guard samples
/// the CPU time counters instead.
fn overloaded() -> bool {
    #[cfg(windows)]
    return match cpu_busy() {
        Some(busy) if is_busy(busy) => {
            eprintln!("PERF skipped: the CPU was {:.0}% busy; frame budgets need a quiet machine", busy * 100.0);
            true
        }
        _ => false,
    };
    #[cfg(not(windows))]
    {
        let cores = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get) as f64;
        match load_average() {
            Some(l) if l > 2.0 * cores => {
                eprintln!("PERF skipped: load average {l:.0} on {cores} cores; frame budgets need a quiet machine");
                true
            }
            _ => false,
        }
    }
}

/// The budget tests take turns: each starts render workers, and measuring one while another
/// runs would charge it for the other's work on a small CI machine.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Generous for debug builds and shared CI machines; release builds are ~10× faster.
const BUDGET: std::time::Duration = std::time::Duration::from_millis(40);

/// The average frame over `frames` steps, best of up to three rounds: another process taking the
/// CPU only ever adds time, so the fastest round is the closest to the UI's own cost, and a real
/// regression is over budget in every round. Later rounds run only when a round is over budget.
fn best_average(label: &str, frames: usize, mut step: impl FnMut(usize)) -> std::time::Duration {
    let mut best = std::time::Duration::MAX;
    for round in 1..=3 {
        let start = std::time::Instant::now();
        let mut worst = std::time::Duration::ZERO;
        for f in 0..frames {
            let t = std::time::Instant::now();
            step(f);
            worst = worst.max(t.elapsed());
        }
        let avg = start.elapsed() / frames as u32;
        eprintln!("PERF {label} round {round}: avg frame {avg:?}, worst {worst:?}");
        best = best.min(avg);
        if best < BUDGET {
            break;
        }
    }
    best
}

/// `n` text pages with a few comments each, so panels and overlays have work to do.
fn big(n: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec!["<< /Type /Catalog /Pages 2 0 R >>".into()];
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 3 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} /MediaBox [0 0 612 792] >>", kids.join(" ")));
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    for i in 0..n {
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> /Annots [{} 0 R] >>",
            5 + 3 * i,
            6 + 3 * i
        ));
        let body: String =
            (0..40).map(|l| format!("BT /F1 10 Tf 72 {} Td (Page {} line {l} with some words to lay out) Tj ET\n", 720 - l * 15, i + 1)).collect();
        objs.push(format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len()));
        objs.push("<< /Type /Annot /Subtype /Square /Rect [72 72 144 144] /C [1 0 0] /T (Ada) /Contents (Check) >>".into());
    }
    pdf(objs)
}

fn pdf(objs: Vec<String>) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let x = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

/// Named roots, none with replies: the panel still looks up every root's thread.
fn named_comments(n: usize) -> Vec<u8> {
    let annots: Vec<String> = (0..n).map(|i| format!("{} 0 R", i + 4)).collect();
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Annots [{}] >>", annots.join(" ")),
    ];
    for i in 0..n {
        objs.push(format!(
            "<< /Type /Annot /Subtype /Square /Rect [72 72 144 144] /C [1 0 0] /T (Ada) /NM (root-{i}) /Contents (Review note {i}) >>"
        ));
    }
    pdf(objs)
}

#[test]
fn collapsed_comments_with_ten_thousand_named_roots_stay_within_the_frame_budget() {
    if overloaded() {
        return;
    }
    let _turn = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let bytes = named_comments(10_000);
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = LocalPDFApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("review.pdf", None, bytes).expect("opens");
        app.set_option("left", "closed").unwrap();
        app.set_option("panel", "comments").unwrap();
        app.views[0].comments.collapsed_pages = vec![0];
        app
    });
    h.run_steps(4);
    {
        let state = h.state();
        let info = &state.session.get(state.views[0].id).unwrap().info;
        assert_eq!(info.pages.len(), 1);
        assert_eq!(info.annotations.len(), 10_000);
        for a in &info.annotations {
            assert_eq!(a.name.as_deref(), Some(format!("root-{}", a.index).as_str()));
            assert!(a.in_reply_to.is_none());
            assert!(a.state.is_none());
        }
        assert_eq!(state.views[0].comments.collapsed_pages, [0]);
    }
    // Folding removes card layout cost, so this measures filtering and thread lookup work.
    let avg = best_average("collapsed named comments", 30, |_| h.step());
    assert!(avg < BUDGET, "collapsed comments: average frame {avg:?}");
}

#[test]
fn scrolling_a_500_page_document_stays_within_the_frame_budget() {
    if overloaded() {
        return;
    }
    let _turn = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let bytes = big(500);
    let t0 = std::time::Instant::now();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = LocalPDFApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("big.pdf", None, bytes).expect("opens");
        app
    });
    h.run_steps(4);
    eprintln!("PERF open {:?}", t0.elapsed());
    // Scroll through the document: 200 frames, jumping ~2 pages per frame.
    let avg = best_average("scroll", 200, |f| {
        h.state_mut().views[0].goto = Some(((f * 5) / 2 % 500, 0.3));
        h.step();
    });
    assert!(avg < BUDGET, "average frame {avg:?}");
}

#[test]
fn panels_with_hundreds_of_items_stay_within_the_frame_budget() {
    if overloaded() {
        return;
    }
    let _turn = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let bytes = big(500);
    for panel in ["comments", "pages", "fields"] {
        let b = bytes.clone();
        let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
            let mut app = LocalPDFApp::new();
            app.set_option("language", "en").unwrap();
            app.open_bytes("big.pdf", None, b).expect("opens");
            app.set_option("panel", panel).unwrap();
            app
        });
        h.run_steps(4);
        let avg = best_average(panel, 100, |f| {
            h.state_mut().views[0].goto = Some(((f * 5) % 500, 0.3));
            h.step();
        });
        assert!(avg < BUDGET, "{panel}: average frame {avg:?}");
    }
}

#[test]
fn the_busy_share_of_the_machine_comes_from_the_cpu_time_counters() {
    // Before: 100 idle, 300 kernel, 100 user. After: each grew by 100, so half of the elapsed time was idle.
    assert_eq!(utilization([100, 300, 100], [200, 400, 200]), Some(0.5));
    assert_eq!(utilization([100, 300, 100], [100, 300, 100]), None, "counters that did not move say nothing");
    assert_eq!(utilization([200, 400, 200], [100, 300, 100]), None, "counters going backwards say nothing");
}

#[test]
fn the_line_between_a_quiet_and_an_overcommitted_machine() {
    assert!(!is_busy(0.2));
    assert!(!is_busy(BUSY), "half the machine is still a quiet machine");
    assert!(is_busy(0.8));
}

#[cfg(windows)]
#[test]
fn the_windows_guard_samples_the_cpu_instead_of_a_load_average() {
    let busy = cpu_busy().expect("the CPU time counters are readable");
    assert!((0.0..=1.0).contains(&busy), "a share of the machine: {busy}");
}
