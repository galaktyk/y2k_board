
image not align with real location? try to reproduce


---

add submenu to the screen right 

this will show when select creation tools or corresponding object is selected



### color swatch
*check palette.rs
basic color swatch ui like MS paint
three column grid color squares

since we have many element that can be colored, we can show 1 swatch and reuse it for different element
with tab ui like this
|A|▨|☐|
--------
☐☐☐
☐☐☐
☐☐☐
...

when user select the tab, the swatch will highlight border the color for that element 





## rectangle
- show text, fill, and border color swatch
- and "border width" slider

## ellipse
- show text, fill, and border color swatch
- and "border width" slider

## Line
- show line color swatch
- and "line width" slider


## select image
- don't show submenu


## select multiple
- if all the same type, show the corresponding submenu (I want this,  is this possible? )
- if not, only show what is common between them


** IF ANY OF THE ABOVE PERFORMANCE HEAVY PLEASE TELL ME FIRST, I CAN TRY TO OPTIMIZE OR CHANGE THE DESIGN,***


**
if the current code is not ready or missing some feature, also tell me first**





---

ro sprite player





---
thumbnail swtitch with zoom level is not reliable,
some image might bigger than another, 
I'm considering threshold on image size on screen like hires switch but doing that need CPU loop every image which is not goood,



---
auto clear all image in RAM every 1 min
(visible image is fine because it's in GPU)


----
add debug images
check many image
check many image zoom out
---