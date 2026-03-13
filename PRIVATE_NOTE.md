BIG FEATURE: Text rendering
use cosmic text to render and text-edit text



1. add new tool "Text" box, create similar to rect but with text rendering inside and transparent background
2. for rect and ellipse, add ability to add text rendering inside the shape by double clicking the shape, this will create a text element as a child of the shape, and the text will be rendered inside the shape with word wrap and clipping to the shape bounds
(edit in place, no separate editor window)





so  cosmic text output bitmap with correct vowel like "ดื่มด่ำ 好き　你好"
i add all text visible to atlas with existing culling system
send atlas to GPU  element use atlas uv to draw



## how to render
have pre alloc this on GPU
text_atlas:  1024×1024 R8   = 1MB   — all text glyphs
emoji_atlas: 1024×1024 RGBA = 4MB   — emoji
use update_texture_part to update only part of the atlas when new text is added, this is fast and efficient


---
