use anyhow::{self, Context};
use env_logger;
use image::DynamicImage;
use imageproc::template_matching::{MatchTemplateMethod, find_extremes, match_template};
use log::{debug, info};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
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

const MIN_SCORE: f32 = 0.75;

fn file_path(path: &str) -> PathBuf {
    let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    abs_path.push(path);
    abs_path
}

fn get_snapshots_dir() -> anyhow::Result<PathBuf> {
    let dir = file_path("snapshots");
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
    input.save(file_path("debugging-imgs/screen.png"))?;

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

fn get_ocr_text(engine: &OcrEngine, mask: UIMask, img: &DynamicImage) -> anyhow::Result<String> {
    let masked = mask.crop(&img);
    masked.save(file_path(&format!("debugging-imgs/ocr-{}.png", mask)))?;
    let img_rgb8 = masked.into_rgb8();
    let img_source = ImageSource::from_bytes(img_rgb8.as_raw(), img_rgb8.dimensions())?;
    let ocr_input = engine.prepare_input(img_source)?;
    engine.get_text(&ocr_input)
}

fn print_stats(engine: &OcrEngine, img: &DynamicImage) -> anyhow::Result<()> {
    let gem_currency = get_ocr_text(engine, UIMask::GEM_CURRENCY, img)?;
    let wave_count = get_ocr_text(engine, UIMask::WAVE_COUNT, img)?;
    info!("GEMS: {}", gem_currency);
    info!("WAVE: {}", wave_count);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    connect_waydroid()?;
    let mut droid = Droid::new(DroidConfig::default())?;
    let gems_template = assets::AssetTemplate::from_file("claim_gems.png")?;
    let retry_run_template = assets::AssetTemplate::from_file("retry_run.png")?;

    let detection_model_path = file_path("ocr-models/text-detection.rten");
    let rec_model_path = file_path("ocr-models/text-recognition.rten");

    let detection_model = Model::load_file(detection_model_path)?;
    let recognition_model = Model::load_file(rec_model_path)?;

    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })?;

    loop {
        let mut sleep_duration_secs = 60;
        let screen = prepare_screen(&mut droid)?;
        print_stats(&engine, &screen)?;
        if let Some(surface) = with_surface(&screen, &gems_template, UIMask::GEM_COLUMN)? {
            droid.touch(surface.random_point().into()).execute()?;
            droid.sleep(Duration::from_millis(500));
            droid.touch(surface.random_point().into()).execute()?;
            info!("Gems claimed");
            sleep_duration_secs = 630;
        } else if let Some(surface) =
            with_surface(&screen, &retry_run_template, UIMask::BATTLE_END_SCREEN)?
        {
            info!("Game end screen found. restarting run");
            droid.touch(surface.random_point().into()).execute()?;
            sleep_duration_secs = 30;
        }

        droid.sleep(Duration::from_secs(sleep_duration_secs));
    }
}
