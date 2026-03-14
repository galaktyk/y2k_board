
File: src/input/handlers/mouse.rs
Confidence: 92%
Problem: During live drag (move, resize, rotate), element.pos, element.size, and element.rotation are mutated directly, but bump_text_generation() is never called. The L2 cache (cached_text_draw) is correctly invalidated via mark_elements_dirty(), which sets text_dirty = true. However, when build_visible_text_instances() runs, ensure_layout_cached() finds a generation match and returns the stale CachedLayout with the old world_min (from text_bounds() which depends on element.pos).

Impact:

Move: Text renders at the original position instead of following the element during drag.
Resize: Text wrapping uses the old element width, and world_min is wrong — text appears mispositioned and incorrectly wrapped.
Rotate: The cached world_min is stale (rotation affects text bounds indirectly through the origin calculation).
Note: The generation IS bumped correctly in the undo/redo path (board.rs:299 and board.rs:309), so the bug only manifests during the live drag, not after committing the operation.

Suggestion: Call element.bump_text_generation() after mutating pos/size/rotation in the drag handler:

// src/input/handlers/mouse.rs, inside the drag loop after each match arm
match state.drag_mode {
    DragMode::MoveSelected => {
        element.pos = orig_pos + state.move_delta;
        element.bump_text_generation();
    }
    DragMode::Rotating => {
        // ... existing rotation code ...
        element.rotation = orig_rot + angle_diff;
        element.bump_text_generation();
    }
    DragMode::ResizingHandle(dir) => {
        // ... existing resize code ...
        element.bump_text_generation();
    }
    DragMode::None => {}
}
Alternatively, for move-only operations where the buffer content doesn't change (only world_min shifts), you could store world_min separately and update it without re-shaping. But bumping the generation is the simplest correct fix.


---
File: src/text.rs
Confidence: 75%
Problem: active_edit.unwrap() is logically safe (guarded by is_active_edit which checks active_edit.is_some()), but bare unwrap() doesn't communicate the invariant to future readers. Since ActiveTextEdit is Copy, the as_ref() on line 127 doesn't consume it, making the unwrap() work — but this is subtle.

Suggestion: Use expect() or restructure:

let content = if is_active_edit {
    // SAFETY: is_active_edit is true only when active_edit.is_some()
    active_edit.expect("checked by is_active_edit").content
} else {
    element.text.as_ref().map(|text| text.content.as_str()).unwrap_or_default()
};

---


culling optimize
- if shape or text element  size is smaller than 20px, skip render text
wait this seems to do a loop check
should i do this in cpu or gpu?

---


is this html correct? i only see backscreen when open http://localhost:8080/ without any error in console

---

add multiple selection with drag over
 (show area rectangle while dragging)
when drag finished, show big selection rectangle (also with handles for resizing/rotating)
drag start anywhere inside that big rectangle will drag whole selection 


---

Then enable only what you need.

----

change default color for text
1. text in shapes -> black
2. own text element in board -> white
nit text bug
- space character become tofu square
- edit cursor not show when on left most edge of text

