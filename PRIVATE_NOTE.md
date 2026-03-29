
fix touchpad

---




---
implement  curve line feature
The connector curve technique in short:

**shape (box, ellipse, etc.) face normals → tangent constraints → cubic Bézier**

1. **Face normal** — each box edge has a direction vector (right = `(1,0)`, top = `(0,-1)`, etc.)
2. **Auto control points** — `C1 = exitPoint + (normal × offset)`, `C2 = entryPoint + (inwardNormal × offset)` — computed invisibly
3. **Perpendicular departure** — because C1 is placed along the normal, the curve always leaves the box at 90° to the face
4. **Single handle abstraction** — instead of exposing C1 and C2 raw, one midpoint handle shifts both symmetrically
5. **Result** — a cubic Bézier `P0 → C1 → C2 → P3` that looks intentional from any layout

The key insight is that **the normal determines the tangent**, and the tangent is what makes the curve feel like it "belongs" to the box rather than just floating between two points.

if have qeustion ask me, (this app really serious about optimization, so I want to make sure the implementation is correct and efficient)


---

add support for text size adjustment for rect, ellipse, text box, and sticky

text size change on subtool with two button big "A" and (little bit) smaller "A" for increase and decrease text size, beside it show the current text size in px e.g.
[A] [A] 24px

how text size works:
just use the same atlas just scale it up and down, on every text in the whole container
i don't care if text will be blurry


---

gdrive adapter for web version and desktop version,

---
better ime preview inline




---

---
laser tool
---



add frame tool

---
thumbnail swtitch with zoom level is not reliable,
some image might bigger than another, 
I'm considering threshold on image size on screen like hires switch but doing that need CPU loop every image which is not goood,



--- 
for collab
500ms debounce ops
need binary packer?



-------
license

cursor https://www.void1gaming.com/free-basic-cursor-pack
font: W95FA.otf, notosans, deja vu sans