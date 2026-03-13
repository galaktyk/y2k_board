
# Collaborative Board (Miro-like) — Miniquad Architecture for desktop + web

## Goal

Build a collaborative whiteboard similar to **Miro**.

Performance target:

* **20k shapes**
* **144 Hz**
* smooth interaction while dragging/editing
* low CPU usage when idle

---

# Core Architecture

Unlike Bevy, **Miniquad has no ECS**, so the architecture becomes:

```
App State (CPU)
   ↓
Spatial Index
   ↓
Visible Elements
   ↓
Instance Buffer
   ↓
GPU Instanced Draw

```

The CPU stores **authoritative board state**, while the GPU is used only for rendering.

---

# Board Objects

Board elements are treated as **interactive particles**.

Each element contains:

* position
* size
* shape type
* metadata
* interaction state

Example structure:

```rust
struct Element {
    id: ElementId,
    shape: ShapeType,
    position: Vec2,
    size: Vec2,
    color: Color,
    selected: bool,
    metadata: Metadata,
}
```

Shape types:

```
Rectangle
Ellipse
Line
```

---

# Rendering Pipeline

Rendering uses **GPU instancing**.

```
Elements (CPU Vec<Element>)
       ↓
Visible elements (spatial culling)
       ↓
Instance buffer
       ↓
Single instanced draw call
```

Instance data example:

```rust
struct InstanceData {
    position: [f32; 2],
    size: [f32; 2],
    color: [f32; 4],
    shape_type: u32,
}
```

This allows **thousands of shapes rendered in one draw call**.

---

---

# Performance Techniques

### 1. Spatial Culling

512x512
Grid partitioning reduces objects sent to the GPU.

---

### 2. GPU Instancing

Render thousands of shapes using **one draw call**.

---

### 3. Dirty Rendering

Track when the board changes.

```
if dirty:
    update instance buffer
    render
else:
    skip expensive updates
```

---

### 4. Idle Freeze Mode
When no input occurs:
do not update anything to save battery.


---

# Board Operations

Board changes are represented as **operations**.

Ops follow a CRDT-friendly model for future collaboration.

Operation types:

```
ADD_ELEMENT
DELETE_ELEMENT
SET_PROPERTY
```


---

# Storage Model

### Snapshot File

```
snapshot.bin
```

Contains:

* full board state
* no history
* deleted elements removed

---



### Images
TBD

# Undo / Redo

Undo/Redo is implemented using the op0s.


# Future Online Collaboration

Not part of Sprint 1, but architecture should support it.
Design considerations:
Operations should be **minimal and semantic**.
Example:

```
drag element
```

Instead of sending many updates:

```
SET_POSITION x100
```

Send only:

```
SET_POSITION (on mouse release)
```

This reduces network traffic.

---


# Sprint 2
* ops CRDT system
* snapshot save/load



# TBD
* text rendering
* image uploading
* image rendering

* partial instance buffer updates
* networking
* collaborative editing

