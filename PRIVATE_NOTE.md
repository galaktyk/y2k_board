




is this CPU viewport culling too often on camera pan? 
my cpu% skyrocket when i pan camera, b
maybe we can just send all to GPU？ because GPU can throw away invisible elements, 
no need vbo update every frame, (it just 10MB onvram anyway)

(when i do multiple select move it 144hz, but when i pan camera it go down to20, because of cpu culling?)

* all elements text already in GPU VBO, 
- dont want to do heavy cpu stuff because it slower than let GPU draw it and throw away invisible text
- all character in text box or shape have single color, single size, (as now = good, can avoid loop on all characters and just check  box's text size)
- looking for Multi Draw Indirect — no upload, no copy, pure GPU?


---

---

Then enable only what you need.

----

