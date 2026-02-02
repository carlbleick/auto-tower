use anyhow::{self, Context};
use env_logger;
use find_subimage::SubImageFinderState;
use log::{debug, info, warn};
use rust_droid::{Droid, DroidConfig};
mod ui;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use ui::UISurface;

use crate::ui::UIMask;

fn get_snapshots_dir() -> anyhow::Result<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("snapshots");
    fs::create_dir_all(&dir).context("failed to create snapshots directory")?;
    Ok(dir)
}

fn prune_snapshots(limit: usize) -> anyhow::Result<()> {
    let dir = get_snapshots_dir()?;
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = fs::read_dir(&dir)
        .context("failed to read snapshots directory")?
        .filter_map(|res| res.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.is_file() && path.extension().map(|ext| ext == "png").unwrap_or(false) {
                match e.metadata().and_then(|m| m.modified()) {
                    Ok(modified) => Some((path, modified)),
                    Err(_) => None,
                }
            } else {
                None
            }
        })
        .collect();

    entries.sort_by(|a, b| a.1.cmp(&b.1));

    while entries.len() > limit {
        if let Some((path, _)) = entries.first().cloned() {
            debug!("Removing old snapshot: '{}'", path.display());
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove old snapshot '{}':", path.display()))?;
            entries.remove(0);
        } else {
            break;
        }
    }

    Ok(())
}

fn prepare_screen(droid: &mut Droid) -> anyhow::Result<(UIMask, PathBuf)> {
    let now = chrono::Local::now();
    let filename = format!("screen_{}.png", now.format("%Y-%m-%dT%H-%M-%S"));
    let snapshot_path = get_snapshots_dir()?.join(Path::new(&filename));
    debug!(
        "Taking a snapshot and saving to '{}'...",
        snapshot_path.display()
    );
    droid.snapshot(&snapshot_path)?;
    let mask = UIMask::gem_column();
    let img = mask.crop(image::open(&snapshot_path)?);
    img.save(&snapshot_path)?;
    prune_snapshots(20)?;
    Ok((mask, snapshot_path))
}

fn connect_waydroid() -> anyhow::Result<()> {
    let output = Command::new("waydroid")
        .args(["adb", "connect"])
        .output()
        .context("failed to execute 'waydroid adb connect'")?;

    debug!(
        "waydroid adb connect output: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Command::new("adb")
        .args(["shell", "wm", "size", "319x695"])
        .output()
        .context("failed to set size via adb")?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    connect_waydroid()?;
    let mut droid = Droid::new(DroidConfig::default())?;

    let template_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/assets/claim_gems.png");
    let template = image::open(template_path)?.to_luma8();
    let (tpl_w, tpl_h) = template.dimensions();
    let template_buf = template.into_raw();

    // let backend = Backend::Scalar {
    //     threshold: 0.0,
    //     step_x: 1,
    //     step_y: 1,
    // };
    // let mut finder = SubImageFinderState::new().with_backend(backend);
    let mut finder = SubImageFinderState::new();

    loop {
        let (window_mask, input_path) = prepare_screen(&mut droid)?;
        let input = image::open(input_path)?.to_luma8();
        let (input_w, input_h) = input.dimensions();
        let input_buf = input.into_raw();

        let matches = finder.find_subimage_positions(
            (&input_buf, input_w as usize, input_h as usize),
            (&template_buf, tpl_w as usize, tpl_h as usize),
            1,
        );

        if let Some((x, y, _distance)) = matches.first() {
            let x = *x;
            let y = *y;
            let surface = UISurface::new(
                window_mask.to_point(x as u32, y as u32),
                window_mask.to_point(x as u32 + tpl_w, y as u32 + tpl_h),
            );
            debug!(
                "Claim gems bounding box: ({},{}) to ({},{})",
                surface.top_left.x,
                surface.top_left.y,
                surface.bottom_right.x,
                surface.bottom_right.y,
            );
            droid.touch(surface.random_point().into()).execute()?;
            droid.sleep(Duration::from_millis(500));
            droid.touch(surface.random_point().into()).execute()?;

            info!("Gems claimed");
            droid.sleep(Duration::from_mins(10));
        } else {
            warn!("Template not found, trying again in 1 minute");
            droid.sleep(Duration::from_mins(1));
        }
    }
}
