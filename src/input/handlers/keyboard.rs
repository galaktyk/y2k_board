use crate::board::Board;
use crate::input::state::InputState;

pub fn on_key_down(
    _state: &mut InputState,
    board: &mut Board,
    keycode: miniquad::KeyCode,
    modifiers: miniquad::KeyMods,
) {
    match keycode {
        miniquad::KeyCode::Z if modifiers.ctrl => {
            if modifiers.shift {
                board.redo();
            } else {
                board.undo();
            }
        }
        miniquad::KeyCode::Y if modifiers.ctrl => {
            board.redo();
        }
        miniquad::KeyCode::Delete | miniquad::KeyCode::Backspace => {
            board.delete_selected();
        }
        miniquad::KeyCode::Space => {}
        _ => {}
    }
}