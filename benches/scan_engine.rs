use std::fs;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use repo_radar::{ScanConfig, scan};

fn main() {
    let fixture = create_fixture();
    let iterations = 100;
    let started = Instant::now();

    for _ in 0..iterations {
        scan(&fixture, &ScanConfig::default()).expect("benchmark scan should succeed");
    }

    let elapsed = started.elapsed();
    println!(
        "scan_engine: {iterations} scans in {elapsed:?} ({:?}/scan)",
        elapsed / iterations
    );
    let _ = fs::remove_dir_all(fixture);
}

fn create_fixture() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("repo-radar-bench-{suffix}"));
    fs::create_dir(&root).expect("benchmark root should be created");

    for directory in ["src", "tests", "docs"] {
        fs::create_dir(root.join(directory)).expect("benchmark directory should be created");
    }
    for index in 0..100 {
        let directory = match index % 3 {
            0 => "src",
            1 => "tests",
            _ => "docs",
        };
        let extension = match index % 2 {
            0 => "rs",
            _ => "md",
        };
        fs::write(
            root.join(directory)
                .join(format!("file-{index}.{extension}")),
            vec![b'x'; 128],
        )
        .expect("benchmark file should be created");
    }
    fs::write(root.join("README"), b"benchmark").expect("benchmark file should be created");
    fs::write(root.join("empty"), b"").expect("benchmark file should be created");
    fs::create_dir(root.join("target")).expect("ignored directory should be created");
    fs::write(root.join("target/ignored.rs"), b"ignored").expect("ignored file should be created");

    root
}
