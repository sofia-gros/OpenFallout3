use fo3_esm::types::FormId;
use glam::Vec3;

#[derive(Clone, Debug)]
pub struct NavPath {
    pub points: Vec<Vec3>,
    pub nodes: Vec<(FormId, u16)>,
}
