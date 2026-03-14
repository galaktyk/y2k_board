

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

