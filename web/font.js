"use strict";

(function () {
    const LOG_PREFIX = "[miniGalaktyk/fonts]";
    const CACHE_KEY = "app-fonts-v1";

    const FONT_CSS = {
        emoji: "./NotoColorEmoji.ttf",
        emoji_text: "./NotoColorEmoji.ttf",
        thai: "https://fonts.googleapis.com/css2?family=Noto+Sans+Thai:wght@400;700",
        arabic: "https://fonts.googleapis.com/css2?family=Noto+Sans+Arabic:wght@400;700",
        devanagari: "https://fonts.googleapis.com/css2?family=Noto+Sans+Devanagari:wght@400;700",
        jp: "https://fonts.googleapis.com/css2?family=Noto+Sans+JP:wght@400;700",
        sc: "https://fonts.googleapis.com/css2?family=Noto+Sans+SC:wght@400;700",
        tc: "https://fonts.googleapis.com/css2?family=Noto+Sans+TC:wght@400;700",
        symbols: "./DejaVuSans.ttf",
        symbols2: "./DejaVuSans.ttf",
    };

    const LANG_DETECT = [
        {
            lang: "thai",
            test: function (cp) {
                return cp >= 0x0E00 && cp <= 0x0E7F;
            },
        },
        {
            lang: "arabic",
            test: function (cp) {
                return (cp >= 0x0600 && cp <= 0x06FF)
                    || (cp >= 0x0750 && cp <= 0x077F)
                    || (cp >= 0x08A0 && cp <= 0x08FF)
                    || (cp >= 0xFB50 && cp <= 0xFDFF)
                    || (cp >= 0xFE70 && cp <= 0xFEFF);
            },
        },
        {
            lang: "devanagari",
            test: function (cp) {
                return cp >= 0x0900 && cp <= 0x097F;
            },
        },
    ];

    const HAN_START = 0x4E00;
    const HAN_END = 0x9FFF;
    const ARROWS_START = 0x2190;
    const ARROWS_END = 0x21FF;
    const TECHNICAL_START = 0x2300;
    const TECHNICAL_END = 0x23FF;
    const GEOMETRIC_START = 0x25A0;
    const GEOMETRIC_END = 0x25FF;
    const MISC_START = 0x2600;
    const MISC_END = 0x27BF;
    const HIRAGANA_START = 0x3040;
    const HIRAGANA_END = 0x309F;
    const KATAKANA_START = 0x30A0;
    const KATAKANA_END = 0x30FF;
    const EMOJI_START = 0x1F300;
    const EMOJI_END = 0x1FAFF;
    const VS15 = 0xFE0E;
    const VS16 = 0xFE0F;

    const alreadyLoaded = new Set();
    const inFlight = new Set();
    const cssCache = new Map();

    console.info(LOG_PREFIX, "font.js loaded");

    function previewText(text) {
        if (!text) {
            return "";
        }

        return text.length > 32 ? text.slice(0, 32) + "..." : text;
    }

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

    function parseFontFaces(css) {
        const faces = [];
        const matches = css.matchAll(/@font-face\s*\{([^}]+)\}/g);
        for (const match of matches) {
            const block = match[1];
            const srcMatch = block.match(/url\(([^)]+\.(?:woff2|ttf|otf))\)/i);
            const rangeMatch = block.match(/unicode-range:\s*([^;]+)/i);
            if (!srcMatch) {
                continue;
            }

            faces.push({
                src: srcMatch[1].replace(/^['"]|['"]$/g, ""),
                range: rangeMatch ? rangeMatch[1].trim() : null,
            });
        }
        return faces;
    }

    function subsetNeeded(codepoints, rangeString, lang) {
        if (!rangeString) {
            return true;
        }

        if (lang === "emoji" || lang === "emoji_text" || lang === "symbols" || lang === "symbols2") {
            return true;
        }

        const parts = rangeString.split(",");
        for (const part of parts) {
            const normalized = part.trim().replace(/^U\+/i, "");
            if (!normalized) {
                continue;
            }

            if (normalized.indexOf("-") >= 0) {
                const bounds = normalized.split("-");
                const lo = parseInt(bounds[0], 16);
                const hi = parseInt(bounds[1], 16);
                if (codepoints.some(function (cp) { return cp >= lo && cp <= hi; })) {
                    return true;
                }
                continue;
            }

            if (normalized.indexOf("?") >= 0) {
                const lo = parseInt(normalized.replace(/\?/g, "0"), 16);
                const hi = parseInt(normalized.replace(/\?/g, "F"), 16);
                if (codepoints.some(function (cp) { return cp >= lo && cp <= hi; })) {
                    return true;
                }
                continue;
            }

            const cp = parseInt(normalized, 16);
            if (codepoints.indexOf(cp) >= 0) {
                return true;
            }
        }

        return false;
    }

    function defaultHanLanguage() {
        const locale = String((navigator.languages && navigator.languages[0]) || navigator.language || "").toLowerCase();
        if (locale.indexOf("zh-tw") >= 0 || locale.indexOf("zh-hk") >= 0 || locale.indexOf("zh-mo") >= 0 || locale.indexOf("hant") >= 0) {
            return "tc";
        }
        return "sc";
    }

    function detectLanguages(text) {
        const codepoints = Array.from(text, function (char) {
            return char.codePointAt(0);
        });
        const needed = new Set();
        let hasHan = false;
        let hasKana = false;
        let hasEmoji = false;
        let hasSymbol = false;

        for (const descriptor of LANG_DETECT) {
            if (codepoints.some(descriptor.test)) {
                needed.add(descriptor.lang);
            }
        }

        for (const cp of codepoints) {
            if (cp >= HAN_START && cp <= HAN_END) {
                hasHan = true;
            }
            if ((cp >= HIRAGANA_START && cp <= HIRAGANA_END) || (cp >= KATAKANA_START && cp <= KATAKANA_END)) {
                hasKana = true;
            }
            if ((cp >= 0x1F300 && cp <= 0x1FAFF) || (cp >= 0x2600 && cp <= 0x27BF) || cp === VS15 || cp === VS16) {
                hasEmoji = true;
            }
            if ((cp >= ARROWS_START && cp <= ARROWS_END)
                || (cp >= TECHNICAL_START && cp <= TECHNICAL_END)
                || (cp >= GEOMETRIC_START && cp <= GEOMETRIC_END)
                || (cp >= MISC_START && cp <= MISC_END)
                || (cp >= 0x2000 && cp <= 0x2BFF)) {
                hasSymbol = true;
            }
        }

        if (hasEmoji) {
            needed.add("emoji_text"); // only load monochrome to ensure no COLRv1 renderer bugs
        }

        if (hasSymbol) {
            needed.add("symbols");
            needed.add("symbols2");
        }

        if (hasKana) {
            needed.add("jp");
        } else if (hasHan) {
            needed.add(defaultHanLanguage());
        }

        return {
            codepoints: codepoints,
            langs: Array.from(needed),
        };
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

    async function fetchFontSubsetsForText(text, wasmExports, wasmMemory) {
        if (!text || text.length === 0) {
            return;
        }

        const detection = detectLanguages(text);
        if (detection.langs.length === 0) {
            return;
        }

        const cache = supportsCacheApi() ? await caches.open(CACHE_KEY) : null;
        const codepoints = detection.codepoints;
        console.debug(LOG_PREFIX, "using cache api", !!cache);

        await Promise.all(detection.langs.map(async function (lang) {
            const cssUrl = FONT_CSS[lang];
            if (!cssUrl) {
                console.warn(LOG_PREFIX, "missing CSS URL for language", lang);
                return;
            }

            if (cssUrl.endsWith(".ttf") || cssUrl.endsWith(".otf") || cssUrl.endsWith(".woff") || cssUrl.endsWith(".woff2")) {
                if (alreadyLoaded.has(cssUrl) || inFlight.has(cssUrl)) return;
                inFlight.add(cssUrl);
                try {
                    console.info(LOG_PREFIX, "fetching direct font file", cssUrl);
                    const response = await cacheGetOrFetch(cache, cssUrl);
                    const buffer = await response.arrayBuffer();
                    pushFontBytesToWasm(buffer, wasmExports, wasmMemory);
                    alreadyLoaded.add(cssUrl);
                    console.info(LOG_PREFIX, "font file loaded", cssUrl);
                } catch (e) {
                    console.error(LOG_PREFIX, "failed to fetch font file", cssUrl, e);
                } finally {
                    inFlight.delete(cssUrl);
                }
                return;
            }

            let faces = cssCache.get(cssUrl);
            if (!faces) {
                console.info(LOG_PREFIX, "loading CSS", { lang: lang, url: cssUrl });
                const cssResponse = await cacheGetOrFetch(cache, cssUrl, {
                    headers: { Accept: "text/css" },
                });
                const css = await cssResponse.text();
                faces = parseFontFaces(css);
                cssCache.set(cssUrl, faces);
            }

            const neededFaces = faces.filter(function (face) {
                return !alreadyLoaded.has(face.src)
                    && !inFlight.has(face.src)
                    && subsetNeeded(codepoints, face.range, lang);
            });

            console.debug(LOG_PREFIX, "language analysis", {
                lang: lang,
                totalFaces: faces.length,
                neededFaces: neededFaces.length,
            });

            await Promise.all(neededFaces.map(async function (face) {
                inFlight.add(face.src);
                try {
                    console.info(LOG_PREFIX, "fetching font subset", { lang: lang, url: face.src });
                    const response = await cacheGetOrFetch(cache, face.src);
                    const buffer = await response.arrayBuffer();
                    alreadyLoaded.add(face.src);
                    pushFontBytesToWasm(new Uint8Array(buffer), wasmExports, wasmMemory);
                    console.info(LOG_PREFIX, "font subset loaded", { lang: lang, url: face.src, bytes: buffer.byteLength });
                } finally {
                    inFlight.delete(face.src);
                }
            }));
        }));
    }

    async function requestFontsForText(text, wasmExports, wasmMemory) {
        // Run as background task to avoid blocking main UI loop.
        // fetchFontSubsetsForText is already async and handles its own state.
        setTimeout(async function () {
            try {
                await fetchFontSubsetsForText(text, wasmExports, wasmMemory);
            } catch (error) {
                console.warn("Font subset fetch failed:", error && error.message ? error.message : error);
            }
        }, 0);
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
        requestFontsForText: requestFontsForText,
        clearFontCache: clearFontCache,
    };
})();