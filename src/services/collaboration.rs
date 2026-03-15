#![allow(dead_code)]

pub trait CollaborationService {
    fn connect(&mut self) -> Result<(), &'static str>;
    fn disconnect(&mut self);
}

#[derive(Default)]
pub struct NoopCollaborationService;

impl CollaborationService for NoopCollaborationService {
    fn connect(&mut self) -> Result<(), &'static str> {
        Err("collaboration service adapter is not implemented yet")
    }

    fn disconnect(&mut self) {}
}
