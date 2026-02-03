use anyhow::{self, Context};
use env_logger;
use find_subimage::SubImageFinderState;
use image::DynamicImage;
use log::{debug, info};
use rust_droid::{Droid, DroidConfig};
mod assets;
mod ui;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use ui::UISurface;

use crate::assets::AssetTemplate;
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

fn prepare_screen(droid: &mut Droid) -> anyhow::Result<DynamicImage> {
    let now = chrono::Local::now();
    let filename = format!("screen_{}.png", now.format("%Y-%m-%dT%H-%M-%S"));
    let snapshot_path = get_snapshots_dir()?.join(Path::new(&filename));
    droid.snapshot(&snapshot_path)?;
    prune_snapshots(20)?;
    Ok(image::open(&snapshot_path)?)
}

fn apply_mask(img: &DynamicImage, mask: UIMask) -> anyhow::Result<(Vec<u8>, usize, usize)> {
    let cropped = mask.crop(img).to_luma8();
    let (width, height) = cropped.dimensions();
    let bytes = cropped.into_raw();
    Ok((bytes, width as usize, height as usize))
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

fn with_surface<F>(
    screen: &DynamicImage,
    template: &AssetTemplate,
    mask: UIMask,
    callback: F,
) -> anyhow::Result<bool>
where
    F: FnOnce(UISurface) -> anyhow::Result<()>,
{
    let (input_buf, input_w, input_h) = apply_mask(&screen, mask)?;

    // let backend = Backend::Scalar {
    //     threshold: 0.0,
    //     step_x: 1,
    //     step_y: 1,
    // };
    // let mut finder = SubImageFinderState::new().with_backend(backend);
    let mut finder = SubImageFinderState::new();

    let matches = finder.find_subimage_positions(
        (&input_buf, input_w as usize, input_h as usize),
        (
            &template.buf,
            template.width as usize,
            template.height as usize,
        ),
        1,
    );

    if let Some((x, y, _distance)) = matches.first() {
        let x = *x as u32;
        let y = *y as u32;
        let surface = UISurface::new(
            mask.to_point(x, y),
            mask.to_point(x + template.width, y + template.height),
        );
        debug!(
            "Surface matched: ({}, {}) to ({}, {})",
            surface.top_left.x, surface.top_left.y, surface.bottom_right.x, surface.bottom_right.y,
        );
        callback(surface)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    connect_waydroid()?;
    let mut droid = Droid::new(DroidConfig::default())?;
    let gems_template = assets::AssetTemplate::from_file("claim_gems.png")?;
    let retry_run_template = assets::AssetTemplate::from_file("retry_run.png")?;

    loop {
        let mut sleep_duration_secs = 60;
        let screen = prepare_screen(&mut droid)?;
        with_surface(&screen, &gems_template, UIMask::gem_column(), |surface| {
            droid.touch(surface.random_point().into()).execute()?;
            droid.sleep(Duration::from_millis(500));
            droid.touch(surface.random_point().into()).execute()?;
            info!("Gems claimed");
            sleep_duration_secs = 630;
            Ok(())
        })?;
        with_surface(
            &screen,
            &retry_run_template,
            UIMask::battle_end_screen(),
            |_| {
                info!("Game end screen found. what to do?");
                Ok(())
            },
        )?;
        droid.sleep(Duration::from_secs(sleep_duration_secs));
    }
}
