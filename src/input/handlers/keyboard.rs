use crate::board::Board;
use crate::input::state::InputState;

pub fn on_key_down(
    state: &mut InputState,
    board: &mut Board,
    keycode: miniquad::KeyCode,
    modifiers: miniquad::KeyMods,
) {
    if state.active_text_id.is_some() {
        return;
    }

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
