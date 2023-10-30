#[derive(Clone, PartialEq, Debug)]
pub struct BoundingBox {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub left: Option<f32>,
    pub top: Option<f32>,
}
