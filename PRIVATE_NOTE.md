

current app using text layout cache?
layout float16? uint? since textbox is not gonna be that big

---
fix touchpad

---
arrow snapping
current implement tation have two behaviour,
1. when create it preview snap to element edge
2. when drag it's preview not snap but when release it snap to element edge

make it use same behaviour by
1. line drag preview snap BUT! only snap when near 12px to edge


---

nit on line and arrow renderering
1. dont make line end round, just leave it as simple line
2. add antialias (simple one)
3. arrow head size should fix with the line, current is fix pixel so when zoom out it looks weird



---

fix ellipse border

---

add support for text size adjustment for rect, ellipse

text size change on subtool with two button "A" and "a" for increase and decrease text size, beside it show the current text size in px

how text size works:
just use the same atlas just scale it up and down, on the whole rect


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