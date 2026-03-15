#[repr(C)]
pub(super) struct ShapeUniforms {
    pub u_mvp: [[f32; 4]; 4],
    pub u_world_per_px: f32,
}

#[repr(C)]
pub(super) struct TextUniforms {
    pub u_mvp: [[f32; 4]; 4],
}

#[repr(C)]
pub(super) struct GridUniforms {
    pub u_inv_mvp: [[f32; 4]; 4],
    pub u_cell: f32,
}