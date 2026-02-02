use image::DynamicImage;
use rust_droid::common::point::Point;

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
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl UIMask {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn x(&self) -> u32 {
        self.x
    }
    pub fn y(&self) -> u32 {
        self.y
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }

    pub const GEM_COLUMN: UIMask = UIMask {
        x: 0,
        y: 0,
        width: 100,
        height: 695,
    };

    // TODO: Update with real coordinates when available.
    pub const BATTLE_END_SCREEN: UIMask = UIMask {
        x: 609,
        y: 0,
        width: 295,
        height: 695,
    };

    pub fn gem_column() -> Self {
        UIMask::GEM_COLUMN
    }

    pub fn battle_end_screen() -> Self {
        UIMask::BATTLE_END_SCREEN
    }

    pub fn crop(&self, img: DynamicImage) -> DynamicImage {
        img.crop_imm(self.x, self.y, self.width, self.height)
    }

    pub fn to_point(&self, x: u32, y: u32) -> UIPoint {
        UIPoint {
            x: self.x + x,
            y: self.y + y,
        }
    }
}
