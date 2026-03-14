#include "image_cache.h"
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <filesystem>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
namespace {

static Image kEmptyImage{};  // sentinel returned when load fails

std::size_t TexBytes(int w, int h) {
    return static_cast<std::size_t>(w) * static_cast<std::size_t>(h) * 4;
}

} // namespace

// ---------------------------------------------------------------------------
// LOD filename helpers
// ---------------------------------------------------------------------------

std::string ImageManager::LodSmallName(const std::string& base) {
    const auto dot = base.rfind('.');
    const std::string stem = (dot == std::string::npos) ? base : base.substr(0, dot);
    if (stem.size() >= 2 && stem.substr(stem.size() - 2) == "_s") return base;  // already small
    if (dot == std::string::npos) return base + "_s";
    return stem + "_s" + base.substr(dot);
}

// ---------------------------------------------------------------------------
// Init / Shutdown
// ---------------------------------------------------------------------------

void ImageManager::Init(const std::string& imagesDir) {
    imagesDir_ = imagesDir;
    atlasRT_ = LoadRenderTexture(kAtlasSize, kAtlasSize);
    BeginTextureMode(atlasRT_);
    ClearBackground(BLANK);
    EndTextureMode();

    bgRunning_ = true;
    bgThread_  = std::thread([this] { BgWorker(); });
}

void ImageManager::Shutdown() {
    // Signal and join the background thread
    {
        std::lock_guard<std::mutex> lk(workMutex_);
        bgRunning_ = false;
    }
    workCv_.notify_all();
    if (bgThread_.joinable()) bgThread_.join();

    // Discard any leftover staged images (no GPU context available here)
    {
        std::lock_guard<std::mutex> lk(stageMutex_);
        for (auto& s : stagingQueue_)    UnloadImage(s.img);
        stagingQueue_.clear();
        for (auto& s : hiresStagingQueue_) UnloadImage(s.img);
        hiresStagingQueue_.clear();
    }

    for (auto& e : ramList_) UnloadImage(e.img);
    ramList_.clear();
    ramIndex_.clear();
    ramUsedBytes_ = 0;

    for (auto& e : gpuList_) UnloadTexture(e.tex);
    gpuList_.clear();
    gpuIndex_.clear();
    gpuUsedBytes_ = 0;

    filenameToSlot_.clear();
    slotOwner_.fill(std::string{});
    atlasNextSlot_ = 0;
    enqueuedForBg_.clear();
    enqueuedHiRes_.clear();
    hiResAvailable_.clear();
    hiResChecked_.clear();

    UnloadRenderTexture(atlasRT_);
    atlasRT_ = RenderTexture2D{};
}

// ---------------------------------------------------------------------------
// RAM LRU
// ---------------------------------------------------------------------------

void ImageManager::EvictRAMIfNeeded(std::size_t newBytes) {
    while (!ramList_.empty() && ramUsedBytes_ + newBytes > kMaxRamBytes) {
        auto& tail = ramList_.back();
        ramIndex_.erase(tail.key);
        ramUsedBytes_ -= tail.bytes;
        UnloadImage(tail.img);
        ramList_.pop_back();
    }
}

Image& ImageManager::GetOrLoadRAM(const std::string& lodPath) {
    auto it = ramIndex_.find(lodPath);
    if (it != ramIndex_.end()) {
        // Promote to MRU position
        ramList_.splice(ramList_.begin(), ramList_, it->second);
        return ramList_.front().img;
    }

    const std::string fullPath = (std::filesystem::path(imagesDir_) / lodPath).string();
    Image img = LoadImage(fullPath.c_str());

    if (img.data == nullptr) {
        TraceLog(LOG_WARNING, "IMAGE CACHE: failed to load '%s'", fullPath.c_str());
        return kEmptyImage;
    }
    ImageFormat(&img, PIXELFORMAT_UNCOMPRESSED_R8G8B8A8);

    const std::size_t bytes = TexBytes(img.width, img.height);
    EvictRAMIfNeeded(bytes);
    ramList_.push_front({lodPath, img, bytes});
    ramIndex_[lodPath] = ramList_.begin();
    ramUsedBytes_ += bytes;
    return ramList_.front().img;
}

// ---------------------------------------------------------------------------
// GPU LRU
// ---------------------------------------------------------------------------

void ImageManager::EvictGPUIfNeeded(std::size_t newBytes) {
    while (!gpuList_.empty() && gpuUsedBytes_ + newBytes > kMaxGpuBytes) {
        auto& tail = gpuList_.back();
        // Allow re-enqueueing a hi-res job if the texture gets evicted
        enqueuedHiRes_.erase(tail.key);
        gpuIndex_.erase(tail.key);
        gpuUsedBytes_ -= tail.bytes;
        UnloadTexture(tail.tex);
        gpuList_.pop_back();
        TraceLog(LOG_INFO, "GPU EVICT: evicted '%s' to free %.1f MB", tail.key.c_str(), static_cast<float>(tail.bytes) / (1024.0f * 1024.0f));
    }
}

Texture2D ImageManager::GetOrUploadGPU(const std::string& lodPath) {
    auto it = gpuIndex_.find(lodPath);
    if (it != gpuIndex_.end()) {
        gpuList_.splice(gpuList_.begin(), gpuList_, it->second);
        return gpuList_.front().tex;
    }

    Image& img = GetOrLoadRAM(lodPath);
    if (img.data == nullptr || img.width == 0) {
        return Texture2D{};
    }

    const std::size_t bytes = TexBytes(img.width, img.height);
    EvictGPUIfNeeded(bytes);

    Texture2D tex = LoadTextureFromImage(img);
    if (!IsTextureValid(tex)) {
        return Texture2D{};
    }
    GenTextureMipmaps(&tex);
    SetTextureFilter(tex, TEXTURE_FILTER_TRILINEAR);

    gpuList_.push_front({lodPath, tex, bytes});
    gpuIndex_[lodPath] = gpuList_.begin();
    gpuUsedBytes_ += bytes;
    return tex;
}

// ---------------------------------------------------------------------------
// Background thumbnail builder
// ---------------------------------------------------------------------------

void ImageManager::BgWorker() {
    while (true) {
        WorkItem item;
        {
            std::unique_lock<std::mutex> lk(workMutex_);
            workCv_.wait(lk, [this] { return !workQueue_.empty() || !bgRunning_; });
            if (!bgRunning_ && workQueue_.empty()) break;
            if (workQueue_.empty()) continue;
            item = std::move(workQueue_.back());
            workQueue_.pop_back();
        }

        if (item.type == WorkItem::Type::Thumb) {
            // Prefer loading the 512 LOD (faster I/O); fall back to full-res.
            const std::string smallPath = (std::filesystem::path(imagesDir_) / LodSmallName(item.filename)).string();
            const std::string basePath  = (std::filesystem::path(imagesDir_) / item.filename).string();
            Image img = LoadImage(smallPath.c_str());
            if (img.data == nullptr) img = LoadImage(basePath.c_str());
            if (img.data == nullptr || img.width == 0) {
                TraceLog(LOG_WARNING, "IMAGE BG: failed to load thumb for '%s'", item.filename.c_str());
                continue;
            }
            ImageFormat(&img, PIXELFORMAT_UNCOMPRESSED_R8G8B8A8);
            {
                std::lock_guard<std::mutex> lk(stageMutex_);
                stagingQueue_.push_back({item.filename, img});
                hasStagedWork_ = true;
            }
        } else {
            // HiRes: load full-res, clamp to 2048.
            const std::string fullPath = (std::filesystem::path(imagesDir_) / item.filename).string();
            Image img = LoadImage(fullPath.c_str());
            if (img.data == nullptr || img.width == 0) {
                TraceLog(LOG_WARNING, "IMAGE BG: failed to load hires '%s'", item.filename.c_str());
                continue;
            }
            ImageFormat(&img, PIXELFORMAT_UNCOMPRESSED_R8G8B8A8);
            constexpr int kMax = 2048;
            if (img.width > kMax || img.height > kMax) {
                const float sc = std::min(
                    static_cast<float>(kMax) / static_cast<float>(img.width),
                    static_cast<float>(kMax) / static_cast<float>(img.height));
                ImageResize(&img, static_cast<int>(img.width * sc), static_cast<int>(img.height * sc));
            }
            {
                std::lock_guard<std::mutex> lk(stageMutex_);
                hiresStagingQueue_.push_back({item.filename, img});
                hasStagedHiRes_ = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Atlas (stamp + GPU upload, all on main thread)
// ---------------------------------------------------------------------------

bool ImageManager::FlushAtlas() {
    std::vector<StagedThumb> batch;
    {
        std::lock_guard<std::mutex> lk(stageMutex_);
        std::swap(batch, stagingQueue_);
        hasStagedWork_ = false;
    }

    // Upload source textures, assign slots, collect draw jobs.
    struct DrawJob { int col; int row; Texture2D srcTex; };
    std::vector<DrawJob> jobs;
    jobs.reserve(batch.size());

    for (auto& s : batch) {
        if (filenameToSlot_.count(s.baseFilename)) {
            UnloadImage(s.img);
            continue;
        }

        Texture2D srcTex = LoadTextureFromImage(s.img);
        UnloadImage(s.img);
        if (!IsTextureValid(srcTex)) {
            TraceLog(LOG_WARNING, "IMAGE ATLAS: failed to upload texture for '%s'", s.baseFilename.c_str());
            continue;
        }
        // Let GPU pick the best mip when downsampling 512→64
        GenTextureMipmaps(&srcTex);
        SetTextureFilter(srcTex, TEXTURE_FILTER_TRILINEAR);

        const int slot = atlasNextSlot_;
        const int col  = slot % kSlotsPerRow;
        const int row  = slot / kSlotsPerRow;

        const std::string& prevOwner = slotOwner_[static_cast<std::size_t>(slot)];
        if (!prevOwner.empty()) {
            filenameToSlot_.erase(prevOwner);
            enqueuedForBg_.erase(prevOwner);
            TraceLog(LOG_INFO, "IMAGE ATLAS: slot %d reused, evicted '%s'", slot, prevOwner.c_str());
        }

        slotOwner_[static_cast<std::size_t>(slot)] = s.baseFilename;
        filenameToSlot_[s.baseFilename]             = slot;
        atlasNextSlot_ = (atlasNextSlot_ + 1) % kAtlasSlots;
        if (atlasNextSlot_ == 0)
            TraceLog(LOG_INFO, "IMAGE ATLAS: ring buffer wrapped, overwriting from slot 0");

        jobs.push_back({col, row, srcTex});
    }

    if (jobs.empty()) return false;

    // GPU-blit all thumbnails into the atlas render texture in one pass.
    // Negative src height compensates for the render-texture Y-flip so that
    // GetDrawInfo can address slots with the same (col*64, row*64) coordinates.
    BeginTextureMode(atlasRT_);
    for (auto& j : jobs) {
        const Rectangle src{0.0f, 0.0f,
            static_cast<float>(j.srcTex.width),
            -static_cast<float>(j.srcTex.height)};
        const Rectangle dst{
            static_cast<float>(j.col * kThumbSize),
            static_cast<float>(j.row * kThumbSize),
            static_cast<float>(kThumbSize),
            static_cast<float>(kThumbSize)
        };
        DrawTexturePro(j.srcTex, src, dst, Vector2{0.0f, 0.0f}, 0.0f, WHITE);
    }
    EndTextureMode();

    for (auto& j : jobs) UnloadTexture(j.srcTex);
    return true;
}

bool ImageManager::HasStagedWork() const {
    return hasStagedWork_ || hasStagedHiRes_;
}

void ImageManager::SeedHiRes(const std::string& baseFilename, Image img) {
    if (gpuIndex_.count(baseFilename)) {
        UnloadImage(img);
        return;
    }
    const std::size_t bytes = TexBytes(img.width, img.height);
    EvictGPUIfNeeded(bytes);
    Texture2D tex = LoadTextureFromImage(img);
    UnloadImage(img);
    if (!IsTextureValid(tex)) return;
    SetTextureFilter(tex, TEXTURE_FILTER_BILINEAR);
    gpuList_.push_front({baseFilename, tex, bytes});
    gpuIndex_[baseFilename] = gpuList_.begin();
    gpuUsedBytes_ += bytes;
    // Mark as known-available and already-served so bg won't re-enqueue
    hiResAvailable_.insert(baseFilename);
    hiResChecked_.insert(baseFilename);
    enqueuedHiRes_.insert(baseFilename);
}

bool ImageManager::FlushHiRes() {
    std::vector<StagedHiRes> batch;
    {
        std::lock_guard<std::mutex> lk(stageMutex_);
        std::swap(batch, hiresStagingQueue_);
        hasStagedHiRes_ = false;
    }
    bool anyUploaded = false;
    for (auto& s : batch) {
        if (gpuIndex_.count(s.baseFilename)) {
            // Already uploaded via SeedHiRes at paste time
            UnloadImage(s.img);
            continue;
        }
        const std::size_t bytes = TexBytes(s.img.width, s.img.height);
        EvictGPUIfNeeded(bytes);
        Texture2D tex = LoadTextureFromImage(s.img);
        UnloadImage(s.img);
        if (!IsTextureValid(tex)) continue;
        SetTextureFilter(tex, TEXTURE_FILTER_BILINEAR);
        gpuList_.push_front({s.baseFilename, tex, bytes});
        gpuIndex_[s.baseFilename] = gpuList_.begin();
        gpuUsedBytes_ += bytes;
        anyUploaded = true;
    }
    return anyUploaded;
}

void ImageManager::MaybeEnqueueHiRes(const std::string& baseFilename) {
    // Small-only files (element->text is already "*_s.jpg") never have a hires companion
    if (LodSmallName(baseFilename) == baseFilename) return;

    if (enqueuedHiRes_.count(baseFilename)) return;

    // One-time disk check for images loaded from a saved board (not freshly pasted)
    if (!hiResChecked_.count(baseFilename)) {
        hiResChecked_.insert(baseFilename);
        if (std::filesystem::exists(std::filesystem::path(imagesDir_) / baseFilename)) {
            hiResAvailable_.insert(baseFilename);
        } else {
            TraceLog(LOG_WARNING, "IMAGE HIRES: no hires file for '%s'", baseFilename.c_str());
            return;
        }
    }

    if (!hiResAvailable_.count(baseFilename)) return;

    enqueuedHiRes_.insert(baseFilename);
    std::lock_guard<std::mutex> lk(workMutex_);
    // push_back = processed first (we pop_back), giving hires priority over thumbs
    workQueue_.push_back({WorkItem::Type::HiRes, baseFilename});
    workCv_.notify_one();
}

void ImageManager::SeedRAM(const std::string& baseFilename, Image img) {
    // If already cached (shouldn't happen for a brand-new file, but be safe), free and overwrite.
    auto it = ramIndex_.find(baseFilename);
    if (it != ramIndex_.end()) {
        ramUsedBytes_ -= it->second->bytes;
        UnloadImage(it->second->img);
        ramList_.erase(it->second);
        ramIndex_.erase(it);
    }
    const std::size_t bytes = TexBytes(img.width, img.height);
    EvictRAMIfNeeded(bytes);
    ramList_.push_front({baseFilename, img, bytes});
    ramIndex_[baseFilename] = ramList_.begin();
    ramUsedBytes_ += bytes;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

DrawInfo ImageManager::GetDrawInfo(const std::string& baseFilename, float zoom,
                                    float elemScreenW, float elemScreenH,
                                    int screenW, int screenH) {
    DrawInfo result{};

    if (zoom < kThumbZoomThreshold) {
        // Atlas lookup — stamped earlier in PreloadRAM/FlushAtlas
        auto slotIt = filenameToSlot_.find(baseFilename);
        if (slotIt == filenameToSlot_.end()) return result;
        const int slot = slotIt->second;
        const int col  = slot % kSlotsPerRow;
        const int row  = slot / kSlotsPerRow;
        result.texture = atlasRT_.texture;
        result.srcRect = Rectangle{
            static_cast<float>(col * kThumbSize),
            static_cast<float>(kAtlasSize - (row + 1) * kThumbSize),
            static_cast<float>(kThumbSize),
            static_cast<float>(kThumbSize)
        };
        result.valid = IsTextureValid(atlasRT_.texture);
        return result;
    }

    // Hi-res branch: either dimension fills ≥ 80 % of the screen
    const bool wantHiRes = (elemScreenW > static_cast<float>(screenW) * kHiResScreenFrac ||
                             elemScreenH > static_cast<float>(screenH) * kHiResScreenFrac);
    if (wantHiRes) {
        auto hiIt = gpuIndex_.find(baseFilename);
        if (hiIt != gpuIndex_.end()) {
            gpuList_.splice(gpuList_.begin(), gpuList_, hiIt->second);
            const Texture2D& tex = gpuList_.front().tex;
            result.texture = tex;
            result.srcRect = Rectangle{0.0f, 0.0f,
                static_cast<float>(tex.width), static_cast<float>(tex.height)};
            result.valid = IsTextureValid(tex);
            return result;
        }
        // Not ready yet — kick off background load and fall through to 512 LOD
        MaybeEnqueueHiRes(baseFilename);
    }

    // 512 LOD via GPU LRU (backed by RAM LRU)
    Texture2D tex = GetOrUploadGPU(LodSmallName(baseFilename));
    if (!IsTextureValid(tex)) return result;
    result.texture = tex;
    result.srcRect = Rectangle{0.0f, 0.0f,
        static_cast<float>(tex.width), static_cast<float>(tex.height)};
    result.valid = true;
    return result;
}

void ImageManager::PreloadRAM(const std::vector<std::string>& baseFilenames, float zoom) {
    // Always enqueue for the background atlas builder regardless of zoom,
    // so thumbnails are ready whenever the user zooms out.
    std::vector<std::string> toEnqueue;
    for (const auto& fn : baseFilenames) {
        if (filenameToSlot_.count(fn) == 0 && enqueuedForBg_.count(fn) == 0) {
            enqueuedForBg_.insert(fn);
            toEnqueue.push_back(fn);
        }
    }
    if (!toEnqueue.empty()) {
        std::lock_guard<std::mutex> lk(workMutex_);
        // push_front = lowest priority (popped last, after any pending HiRes jobs)
        for (auto& fn : toEnqueue) workQueue_.push_front({WorkItem::Type::Thumb, std::move(fn)});
        workCv_.notify_all();
    }

    // Additionally, eagerly load the 512 LOD into RAM when zoomed in.
    if (zoom >= kThumbZoomThreshold) {
        for (const auto& fn : baseFilenames) {
            GetOrLoadRAM(LodSmallName(fn));
        }
    }
}

int ImageManager::GetRAMCount() const {
    return static_cast<int>(ramList_.size());
}

std::size_t ImageManager::GetRAMBytes() const {
    return ramUsedBytes_;
}

int ImageManager::GetAtlasCount() const {
    return static_cast<int>(filenameToSlot_.size());
}

std::size_t ImageManager::GetVRAMBytes() const {
    return gpuUsedBytes_;
}
