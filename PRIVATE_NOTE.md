


use custom cursor for this app for these
default: assets/cursor/default_cursor.png
pointer: assets/cursor/pointer_cursor.png
sticky: assets/cursor/sticky_cursor.png
 (also wasm if possible what you think best practice)
---



fix touchpad

---




add support for sticky tool 

## Toolbar button:
-  assets/cursor/sticky_cursor.png position it before text tool

## Tool behavior:
1. when tool active change cursor image to sticky_cursor.png (can't do the drag create like other tool, just click to create)
2. when click on board, it will create a sticky note at the position

## sticky note element:
- it basically a rect element but with special parameters when created
- default have no border, background color is palette:yellow_pale, and text color is black, default size when create is 256x256px


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