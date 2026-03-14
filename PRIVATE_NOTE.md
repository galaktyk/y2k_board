


---


text render optimize
- i want to stop render text if it is too small < 5px>
- all text already in GPU VBO, 
- dont want to do heavy cpu stuff because it slower than let GPU draw it and throw away invisible text
- all character in text box or shape have single color, single size, (as now = good, can avoid loop on all characters and just check  box's text size)
- looking for Multi Draw Indirect — no upload, no copy, pure GPU?


---


add multiple selection with drag over
 (show area rectangle while dragging)
when drag finished, show big selection rectangle (also with handles for resizing/rotating)
drag start anywhere inside that big rectangle will drag whole selection 


---

Then enable only what you need.

----

