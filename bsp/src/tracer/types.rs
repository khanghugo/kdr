use crate::Plane;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub enum HullType {
    Point = 0,
    Stand = 1,
    Monster = 2,
    Duck = 3,
}

#[derive(Debug)]
pub struct TraceResult {
    pub all_solid: bool,
    pub start_solid: bool,
    pub in_open: bool,
    pub in_water: bool,
    pub fraction: f32,
    pub end_pos: glam::Vec3,
    pub plane: Plane,
}

impl Default for TraceResult {
    fn default() -> Self {
        Self {
            all_solid: true,
            start_solid: Default::default(),
            in_open: Default::default(),
            in_water: Default::default(),
            fraction: 1.,
            end_pos: glam::Vec3::ZERO,
            plane: Default::default(),
        }
    }
}
