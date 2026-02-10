use anyhow::Ok;
use image::{DynamicImage, GrayImage};
use rust_droid::common::point::Point;

use crate::assets::apply_threshold;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UIPoint {
    pub x: u32,
    pub y: u32,
}

impl UIPoint {
    pub fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UISurface {
    pub top_left: UIPoint,
    pub bottom_right: UIPoint,
}

impl UISurface {
    pub fn new(top_left: UIPoint, bottom_right: UIPoint) -> Self {
        Self {
            top_left,
            bottom_right,
        }
    }

    pub fn random_point(&self) -> Point {
        let x = rand::random_range(self.top_left.x..=self.bottom_right.x);
        let y = rand::random_range(self.top_left.y..=self.bottom_right.y);
        Point::new(x, y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UIMask {
    name: &'static str,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl std::fmt::Display for UIMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl UIMask {

    pub const GEM_COLUMN: UIMask = UIMask {
        name: "GEM_COLUMN",
        x: 0,
        y: 0,
        width: 90,
        height: 695,
    };

    pub const GEM_CURRENCY: UIMask = UIMask {
        name: "GEM_CURRENCY",
        x: 25,
        y: 52,
        width: 50,
        height: 25,
    };

    pub const WAVE_COUNT: UIMask = UIMask {
        name: "WAVE_COUNT",
        x: 205,
        y: 433,
        width: 50,
        height: 17,
    };

    pub const BATTLE_END_SCREEN: UIMask = UIMask {
        name: "BATTLE_END_SCREEN",
        x: 16,
        y: 150,
        width: 287,
        height: 400,
    };

    pub fn crop(&self, img: &DynamicImage) -> DynamicImage {
        img.crop_imm(self.x, self.y, self.width, self.height)
    }

    pub fn apply(&self, img: &DynamicImage) -> anyhow::Result<GrayImage> {
        let cropped = self.crop(img);
        Ok(apply_threshold(&cropped)?)
    }

    pub fn to_point(&self, x: u32, y: u32) -> UIPoint {
        UIPoint {
            x: self.x + x,
            y: self.y + y,
        }
    }
}
