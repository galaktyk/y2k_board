




# Image Streaming
(migrate from cpp please check ref/image_cache.cpp for reference)


have 3 locations for images:
1. disk: original images file
2. RAM LRU cache 128mb
3. GPU 
3.1 Thumbnail atlas 1024x1024 for <64x64 images (max 256 images in 1024x1024 atlas) pre alloc
3.2 hires (>512px) (also LRU cap max 64MB)





## Step 1: insert image
### A. from image button top bar
1. open file dialog, open file, decode, resize <2048 -> RAM (LRU cache 128mb)



### B. from paste
1. decode, resize <2048 -> RAM (LRU cache 128mb)





## Step 2. save backup to disk for later (libwebp ultra lossy)

2.1 save as jpg format in disk (images/xxx.webp)

2.2 if image size > 512 (consider this hires)
save another image in disk (images/xxx_hires.webp)



## Step 3. render
1. Culling If image in view, if already in GPU, render
2. If not in GPU, check RAM cache
3. If not in RAM cache, load from disk
4. decode, upload to GPU with mipmap



## Special case hires image >512px
normally it follow step 3 but if it hires and we are looking at it whith width or height > screen 80% then we can load the hires version from RAM or disk and upload to GPU for better quality



## Special case for thumbnail 
we steal mipmap in GPU -> atlas GPU copy from image 512 to our pre alloc atlas 
when Zoom < 0.2 render all iamge as thumbnail atlas

if atlas not exist in GPU show placeholder thumbnail (gray box)



Lifecycle example:
```
user pans board
↓
spatial query finds tiles
↓
if GPU missing
↓
check RAM cache
↓
if missing
↓
load from file
↓
decode → upload to GPU
```

#### Atlas
for 64x64 thumbnails, can fit 256 images in a 1024x1024 atlas texture
when user pastes image, add to atlas as size < 64x64, store UV and width/height for rendering
when render, use UV to sample from atlas 
save atlas to disk when saveing snapshot

