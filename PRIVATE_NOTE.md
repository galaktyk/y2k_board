
nit: when select multiple items and use TAB key to reset rotation to 0, item rotate but the multi select border is not rotate back
---
image not align with real location? try to reproduce

---

big refactor
remove the text element type internally and re-use the rectangle element type
text box now is just rect with transparent fill and 0px border when first created

text tool in the toolbar now create rect with transparent fill and 0px border



---

ro sprite player





---
thumbnail swtitch with zoom level is not reliable,
some image might bigger than another, 
I'm considering threshold on image size on screen like hires switch but doing that need CPU loop every image which is not goood,



--- 
500ms debounce ops