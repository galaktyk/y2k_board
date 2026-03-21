"use strict";

(function () {
    const LOG_PREFIX = "[miniGalaktyk/fonts]";
    const CACHE_KEY = "app-fonts-v1";

    const FONT_FILES = Object.freeze({
        thai: "./fonts/NotoSansThai-Regular.ttf",
        arabic: "./fonts/NotoSansArabic-Regular.ttf",
        jp: "./fonts/NotoSansJP-Regular.otf",
        kr: "./fonts/NotoSansKR-Regular.otf",
        sc: "./fonts/NotoSansSC-Regular.otf",
        tc: "./fonts/NotoSansTC-Regular.otf",
        emoji: "./fonts/NotoColorEmoji.ttf",
        symbols: "./fonts/DejaVuSans.ttf",
        devanagari: "./fonts/NotoSansDevanagari-Regular.ttf"
    });

    // Edit this list to change which self-hosted fonts load during startup.
    const STARTUP_FONT_URLS = Object.freeze([
        FONT_FILES.thai,
        FONT_FILES.symbols,
        FONT_FILES.emoji,
        FONT_FILES.jp,
        FONT_FILES.sc,
        FONT_FILES.tc,
        FONT_FILES.kr,
        FONT_FILES.arabic,
        FONT_FILES.devanagari,
    ]);

    const alreadyLoaded = new Set();
    const inFlight = new Set();

    console.info(LOG_PREFIX, "font.js loaded");

    function supportsCacheApi() {
        return typeof caches !== "undefined" && typeof caches.open === "function";
    }

    async function cacheGetOrFetch(cache, url, init) {
        if (!cache) {
            console.debug(LOG_PREFIX, "fetching without Cache API", url);
            const response = await fetch(url, init || {});
            if (!response.ok) {
                throw new Error("fetch failed: " + url + " (" + response.status + ")");
            }
            return response;
        }

        let response = await cache.match(url);
        if (!response) {
            console.debug(LOG_PREFIX, "cache miss", url);
            response = await fetch(url, init || {});
            if (!response.ok) {
                throw new Error("fetch failed: " + url + " (" + response.status + ")");
            }
            await cache.put(url, response.clone());
        } else {
            console.debug(LOG_PREFIX, "cache hit", url);
        }
        return response;
    }

    function pushFontBytesToWasm(bytes, wasmExports, wasmMemory) {
        if (!wasmExports || typeof wasmExports.allocate_vec_u8 !== "function" || typeof wasmExports.mg_browser_font_loaded !== "function") {
            console.warn(LOG_PREFIX, "wasm exports unavailable for font push", {
                hasExports: !!wasmExports,
                hasAllocate: !!(wasmExports && wasmExports.allocate_vec_u8),
                hasCallback: !!(wasmExports && wasmExports.mg_browser_font_loaded),
            });
            return;
        }

        if (!wasmMemory || !wasmMemory.buffer) {
            console.warn(LOG_PREFIX, "wasm memory unavailable for font push");
            return;
        }

        const fontBytes = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes || 0);
        if (fontBytes.length === 0) {
            console.warn(LOG_PREFIX, "skipping empty font payload");
            return;
        }

        const ptr = wasmExports.allocate_vec_u8(fontBytes.length);
        const heap = new Uint8Array(wasmMemory.buffer, ptr, fontBytes.length);
        heap.set(fontBytes, 0);
        console.info(LOG_PREFIX, "pushing font bytes into wasm", { bytes: fontBytes.length });
        wasmExports.mg_browser_font_loaded(ptr, fontBytes.length);
    }

    async function fetchAndPushFont(fontUrl, cache, wasmExports, wasmMemory) {
        if (!fontUrl || alreadyLoaded.has(fontUrl) || inFlight.has(fontUrl)) {
            return;
        }

        inFlight.add(fontUrl);
        try {
            console.info(LOG_PREFIX, "loading startup font", fontUrl);
            const response = await cacheGetOrFetch(cache, fontUrl);
            const buffer = await response.arrayBuffer();
            pushFontBytesToWasm(buffer, wasmExports, wasmMemory);
            alreadyLoaded.add(fontUrl);
            console.info(LOG_PREFIX, "startup font loaded", { url: fontUrl, bytes: buffer.byteLength });
        } catch (error) {
            console.warn(LOG_PREFIX, "startup font load failed; leaving fallback glyphs", {
                url: fontUrl,
                error: error && error.message ? error.message : String(error),
            });
        } finally {
            inFlight.delete(fontUrl);
        }
    }

    async function bootstrapFonts(wasmExports, wasmMemory) {
        if (!wasmExports || !wasmMemory) {
            console.warn(LOG_PREFIX, "bootstrap skipped; wasm bridge unavailable");
            return;
        }

        const cache = supportsCacheApi() ? await caches.open(CACHE_KEY) : null;
        console.info(LOG_PREFIX, "bootstrapping startup fonts", STARTUP_FONT_URLS);
        await Promise.all(STARTUP_FONT_URLS.map(function (fontUrl) {
            return fetchAndPushFont(fontUrl, cache, wasmExports, wasmMemory);
        }));
    }

    async function clearFontCache() {
        if (supportsCacheApi()) {
            await caches.delete(CACHE_KEY);
        }
        alreadyLoaded.clear();
        inFlight.clear();
        console.info(LOG_PREFIX, "font cache cleared");
    }

    window.miniGalaktykFonts = {
        fontFiles: FONT_FILES,
        startupFontUrls: STARTUP_FONT_URLS.slice(),
        bootstrapFonts: bootstrapFonts,
        clearFontCache: clearFontCache,
    };
})();