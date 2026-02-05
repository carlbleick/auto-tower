use anyhow::{self, Context};
use env_logger;
use image::DynamicImage;
use imageproc::template_matching::{MatchTemplateMethod, find_extremes, match_template};
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

const MIN_SCORE: f32 = 0.8;

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

fn with_surface(
    screen: &DynamicImage,
    template: &AssetTemplate,
    mask: UIMask,
) -> anyhow::Result<Option<UISurface>> {
    let input = mask.apply(&screen)?;
    input.save(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("after-threshold/screen.png"))?;

    let scores = match_template(
        &input,
        &template.image,
        MatchTemplateMethod::CrossCorrelationNormalized,
    );
    let extremes = find_extremes(&scores);
    let best_score = extremes.max_value;
    let best_location = extremes.max_value_location;

    if best_score >= MIN_SCORE {
        let (x, y) = best_location;
        let surface = UISurface::new(
            mask.to_point(x, y),
            mask.to_point(x + template.width, y + template.height),
        );
        debug!(
            "Surface matched: ({}, {}) to ({}, {})",
            surface.top_left.x, surface.top_left.y, surface.bottom_right.x, surface.bottom_right.y,
        );
        debug!(
            "MATCH -> x={}, y={}, score={:.4}",
            best_location.0, best_location.1, best_score
        );
        Ok(Some(surface))
    } else {
        debug!(
            "NO MATCH -> best x={}, y={}, score={:.4}",
            best_location.0, best_location.1, best_score
        );
        Ok(None)
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
        if let Some(surface) = with_surface(&screen, &gems_template, UIMask::gem_column())? {
            droid.touch(surface.random_point().into()).execute()?;
            droid.sleep(Duration::from_millis(500));
            droid.touch(surface.random_point().into()).execute()?;
            info!("Gems claimed");
            sleep_duration_secs = 630;
        } else if let Some(surface) =
            with_surface(&screen, &retry_run_template, UIMask::battle_end_screen())?
        {
            info!("Game end screen found. restarting run");
            droid.touch(surface.random_point().into()).execute()?;
            sleep_duration_secs = 30;
        }

        droid.sleep(Duration::from_secs(sleep_duration_secs));
    }
}
