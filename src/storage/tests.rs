use super::*;
use crate::test_support::Scratch;

#[test]
fn decimal_units_are_bounded_and_precise() {
    for (bytes, label) in [
        (0, "0 B"),
        (999, "999 B"),
        (1000, "1.0 KB"),
        (12_345, "12.3 KB"),
        (1_000_000, "1.0 MB"),
        (2_500_000_000, "2.5 GB"),
        (u64::MAX, "18446744073.7 GB"),
    ] {
        assert_eq!(format_bytes(bytes), label);
    }
}
#[test]
fn disk_calculations_reserve_blocks_and_reject_invalid_output() -> Result<(), String> {
    let disk = disk::parse(
        "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/root 1000 800 150 84% /\n",
    )?;
    assert_eq!(
        (disk.total, disk.used, disk.available, disk.percent()),
        (1_024_000, 819_200, 153_600, 80)
    );
    assert!(disk::parse("header\n/dev/root 0 0 0 0% /\n").is_err());
    assert!(disk::parse("header\n/dev/root 100 200 0 100% /\n").is_err());
    assert!(disk::parse("header\n/dev/root 18446744073709551615 0 0 0% /\n").is_err());
    assert!(disk::parse("missing").is_err());
    assert_eq!(
        disk::parse("header\n/dev/root 100 99 -1 100% /Volumes/App Data\n")?.mount,
        "/Volumes/App Data"
    );
    Ok(())
}
#[test]
fn thresholds_include_exact_limits() {
    let mut disk = Disk {
        filesystem: "test".into(),
        mount: "/".into(),
        total: 10_000,
        used: 9000,
        available: 1000,
    };
    let threshold = LowSpace {
        bytes: 100,
        percent: 5,
    };
    assert!(!threshold.critical(&disk));
    disk.available = 500;
    assert!(threshold.critical(&disk));
    disk.available = 501;
    assert!(!threshold.critical(&disk));
    disk.total = 1000;
    disk.available = 100;
    assert!(threshold.critical(&disk));
}
#[test]
fn aggregation_sorting_empty_and_unknown_sizes() {
    let mut apps = vec![
        app("small", 1, false),
        app("unknown", 0, true),
        app("large", 100, false),
    ];
    accounting::sort(&mut apps);
    assert_eq!(
        apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["large", "small", "unknown"]
    );
    let total = accounting::aggregate(apps.iter().map(|a| &a.total));
    assert_eq!(total.bytes, 101);
    assert!(total.incomplete);
    assert_eq!(apps[2].total.label(), "Unavailable");
    assert_eq!(accounting::aggregate([].iter()), Size::default());
    accounting::sort(&mut []);
}
fn app(name: &str, bytes: u64, incomplete: bool) -> AppUsage {
    AppUsage {
        id: name.into(),
        name: name.into(),
        icon: None,
        parts: Default::default(),
        total: Size {
            bytes,
            incomplete,
            issue: None,
        },
    }
}
#[test]
fn missing_optional_locations_are_empty_but_missing_required_ones_are_unknown() -> Result<(), String>
{
    let scratch = Scratch::new().map_err(|e| e.to_string())?;
    let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let mut scanner = Scanner::new(&cancel);
    assert_eq!(
        scanner.measure(&root.join("missing"), true),
        Size::default()
    );
    assert!(scanner.measure(&root.join("missing"), false).incomplete);
    std::fs::write(root.join("file"), "data").map_err(|e| e.to_string())?;
    assert!(scanner.measure(&root.join("file/child"), false).incomplete);
    assert!(scanner.measure(Path::new("relative"), false).incomplete);
    assert!(scanner.measure(&root.join("../outside"), false).incomplete);
    Ok(())
}
#[cfg(unix)]
#[test]
fn symlinks_hardlinks_and_overlapping_roots_are_not_counted_twice() -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, symlink};
    let scratch = Scratch::new().map_err(|e| e.to_string())?;
    let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
    std::fs::create_dir(root.join("nested")).map_err(|e| e.to_string())?;
    std::fs::write(root.join("nested/file"), vec![1; 8192]).map_err(|e| e.to_string())?;
    std::fs::hard_link(root.join("nested/file"), root.join("alias")).map_err(|e| e.to_string())?;
    symlink(&root, root.join("nested/loop")).map_err(|e| e.to_string())?;
    symlink(root.join("missing"), root.join("broken")).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let mut scanner = Scanner::new(&cancel);
    let nested = scanner.measure(&root.join("nested"), false);
    assert!(!nested.incomplete);
    assert_eq!(scanner.measure(&root.join("nested/file"), false).bytes, 0);
    let rest = scanner.measure(&root, false);
    let mut whole_scanner = Scanner::new(&cancel);
    let whole = whole_scanner.measure(&root, false);
    assert_eq!(nested.bytes + rest.bytes, whole.bytes);
    let mut expected: u64 = 0;
    for name in ["", "nested", "nested/file", "nested/loop", "broken"] {
        let metadata = std::fs::symlink_metadata(root.join(name)).map_err(|e| e.to_string())?;
        expected += metadata.blocks() * 512;
    }
    assert_eq!(whole.bytes, expected);
    assert!(!whole.incomplete);
    assert!(
        Scanner::new(&cancel)
            .measure(&root.join("nested/loop/nested"), false)
            .incomplete
    );
    Ok(())
}
#[cfg(unix)]
#[test]
fn permission_failure_is_reported_without_panicking() -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new().map_err(|e| e.to_string())?;
    let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
    let unreadable = root.join("private");
    std::fs::create_dir(&unreadable).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o0))
        .map_err(|e| e.to_string())?;
    let can_read = std::fs::read_dir(&unreadable).is_ok();
    let cancel = AtomicBool::new(false);
    let result = Scanner::new(&cancel).measure(&root, false);
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    if !can_read {
        assert!(result.incomplete);
        assert!(result.issue.is_some());
    }
    // Even under a privileged runner, deterministic read failures remain covered.
    let mut size = Size::default();
    size.fail(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    assert!(size.incomplete);
    Ok(())
}
#[test]
fn cancelled_scans_return_partial_results_and_do_not_descend() -> Result<(), String> {
    let scratch = Scratch::new().map_err(|e| e.to_string())?;
    let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(true);
    let result = Scanner::new(&cancel).measure(&root, false);
    assert!(result.incomplete);
    assert_eq!(result.bytes, 0);
    Ok(())
}
#[test]
fn cache_refresh_and_cancellation_keep_one_worker_and_reject_stale_results() -> Result<(), String> {
    let revision = crate::app_center::storage_revision();
    let mut storage = Storage {
        completed: Some(Instant::now()),
        revision,
        ..Storage::default()
    };
    storage.enter();
    assert!(!storage.wanted);
    storage.refresh();
    assert!(storage.wanted);
    let (send, updates) = mpsc::sync_channel(2);
    let cancel = Arc::new(AtomicBool::new(false));
    storage.job = Some(Job {
        updates,
        cancel: Arc::clone(&cancel),
        revision,
        finished: false,
    });
    storage.wanted = false;
    storage.refresh();
    assert!(!storage.wanted);
    storage.close();
    assert!(cancel.load(Ordering::Relaxed));
    send.send(Update::Report(Ok(Report::default())))
        .map_err(|e| e.to_string())?;
    drop(send);
    storage.poll(false);
    assert!(!storage.busy());
    assert!(storage.report.is_none());
    storage.completed = Instant::now().checked_sub(CACHE_AGE);
    storage.enter();
    assert!(storage.wanted);
    storage.wanted = false;
    storage.revision = revision.wrapping_sub(1);
    storage.enter();
    assert!(storage.wanted);
    Ok(())
}

#[test]
fn slow_worker_poll_is_nonblocking_and_old_generations_are_discarded() -> Result<(), String> {
    let (send, updates) = mpsc::sync_channel(2);
    let revision = crate::app_center::storage_revision();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut storage = Storage {
        job: Some(Job {
            updates,
            cancel: Arc::clone(&cancel),
            revision,
            finished: false,
        }),
        ..Storage::default()
    };
    let start = Instant::now();
    assert!(!storage.poll(false));
    assert!(start.elapsed() < Duration::from_millis(100));
    storage
        .job
        .as_mut()
        .ok_or("the slow worker must stay tracked")?
        .revision = revision.wrapping_sub(1);
    send.send(Update::Report(Ok(Report::default())))
        .map_err(|e| e.to_string())?;
    drop(send);
    storage.poll(false);
    assert!(cancel.load(Ordering::Relaxed));
    assert!(storage.report.is_none());
    assert!(!storage.busy());
    Ok(())
}
#[test]
fn worker_failure_is_explicit_and_does_not_trigger_a_retry_loop() {
    let (send, updates) = mpsc::sync_channel(2);
    let revision = crate::app_center::storage_revision();
    let mut storage = Storage {
        job: Some(Job {
            updates,
            cancel: Arc::new(AtomicBool::new(false)),
            revision,
            finished: false,
        }),
        completed: Some(Instant::now()),
        revision,
        ..Storage::default()
    };
    drop(send);
    assert!(storage.poll(true));
    assert!(storage.error.is_some());
    assert!(!storage.busy());
    assert!(!storage.poll(true));
}
#[test]
fn directory_disappearance_during_traversal_returns_a_partial_result() -> Result<(), String> {
    let scratch = Scratch::new().map_err(|e| e.to_string())?;
    let root = scratch
        .0
        .canonicalize()
        .map_err(|e| e.to_string())?
        .join("removed");
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let mut removed = false;
    let issue = Scanner::new(&cancel).walk(&root, false, &mut |path, _| {
        removed = std::fs::remove_dir(path).is_ok();
    });
    assert!(removed, "the traversal must visit and remove the directory");
    assert!(issue.is_some());
    Ok(())
}
