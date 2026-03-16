pub const MAX_SHAPE_INSTANCES: usize = 100_000;
pub const MAX_TEXT_INSTANCES: usize = 200_000;
pub const MAX_IMAGE_INSTANCES: usize = 8_192;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InstanceData {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [u8; 4],
    pub rotation: f32,
    pub alpha: u8,
    pub shape_type: u8,
    pub stroke_width: u8,
    pub _pad: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TextInstanceData {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_min: [u16; 2],
    pub uv_max: [u16; 2],
    pub origin: [i16; 2],
    pub color: [u8; 4],
    pub rotation: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ImageInstanceData {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_min: [u16; 2],
    pub uv_max: [u16; 2],
    pub origin: [i16; 2],
    pub color: [u8; 4],
    pub rotation: f32,
}

#[derive(Clone, Copy)]
pub struct PreparedImageDraw {
    pub texture: miniquad::TextureId,
    pub instance: ImageInstanceData,
}

impl InstanceData {
    pub fn new(
        pos: [f32; 2],
        size: [f32; 2],
        rotation: f32,
        color_f32: [f32; 4],
        shape_type: f32,
        alpha_f32: f32,
    ) -> Self {
        Self {
            pos,
            size,
            color: [
                (color_f32[0] * 255.0) as u8,
                (color_f32[1] * 255.0) as u8,
                (color_f32[2] * 255.0) as u8,
                (color_f32[3] * 255.0) as u8,
            ],
            rotation,
            alpha: (alpha_f32 * 255.0) as u8,
            shape_type: shape_type as u8,
            stroke_width: 1,
            _pad: 0,
        }
    }

    pub fn with_stroke_width(mut self, stroke_width: u8) -> Self {
        self.stroke_width = stroke_width;
        self
    }
}

impl TextInstanceData {
    pub fn new(
        pos: [f32; 2],
        size: [f32; 2],
        origin: [f32; 2],
        rotation: f32,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        color_f32: [f32; 4],
    ) -> Self {
        Self {
            pos,
            size,
            uv_min: [(uv_min[0] * 65535.0) as u16, (uv_min[1] * 65535.0) as u16],
            uv_max: [(uv_max[0] * 65535.0) as u16, (uv_max[1] * 65535.0) as u16],
            origin: [origin[0] as i16, origin[1] as i16],
            color: [
                (color_f32[0] * 255.0) as u8,
                (color_f32[1] * 255.0) as u8,
                (color_f32[2] * 255.0) as u8,
                (color_f32[3] * 255.0) as u8,
            ],
            rotation,
        }
    }
}

impl ImageInstanceData {
    pub fn new(
        pos: [f32; 2],
        size: [f32; 2],
        origin: [f32; 2],
        rotation: f32,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        color_f32: [f32; 4],
    ) -> Self {
        Self {
            pos,
            size,
            uv_min: [(uv_min[0] * 65535.0) as u16, (uv_min[1] * 65535.0) as u16],
            uv_max: [(uv_max[0] * 65535.0) as u16, (uv_max[1] * 65535.0) as u16],
            origin: [origin[0] as i16, origin[1] as i16],
            color: [
                (color_f32[0] * 255.0) as u8,
                (color_f32[1] * 255.0) as u8,
                (color_f32[2] * 255.0) as u8,
                (color_f32[3] * 255.0) as u8,
            ],
            rotation,
        }
    }
}