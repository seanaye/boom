use serde::{Deserialize, Serialize};
use std::{cmp, ops::Sub};

pub trait Num: Sub + Eq + Ord + Copy {}

impl<T> Num for T where T: Sub + Eq + Ord + Copy {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point<T>
where
    T: Num,
{
    pub x: T,
    pub y: T,
}

impl<T> Point<T>
where
    T: Num,
{
    pub fn to_rect(&self, other: Point<T>) -> Rect<T> {
        let min_x = cmp::min(self.x, other.x);
        let min_y = cmp::min(self.y, other.y);
        let max_x = cmp::max(self.x, other.x);
        let max_y = cmp::max(self.y, other.y);

        let top_left = Point { x: min_x, y: min_y };
        let bottom_right = Point { x: max_x, y: max_y };
        Rect {
            top_left,
            bottom_right,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect<T>
where
    T: Num,
{
    top_left: Point<T>,
    bottom_right: Point<T>,
}

impl<T> Rect<T>
where
    T: Num,
{
    pub fn width(&self) -> <T as Sub>::Output {
        self.bottom_right.x - self.top_left.x
    }

    pub fn height(&self) -> <T as Sub>::Output {
        self.bottom_right.y - self.top_left.y
    }

    pub fn origin(&self) -> &Point<T> {
        &self.top_left
    }
}
