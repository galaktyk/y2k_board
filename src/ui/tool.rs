#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Select,
    Rect,
    Ellipse,
    Line,
    Sticky,
    Text,
}
