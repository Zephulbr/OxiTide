//! Cover fetch + palette cache for `analyze_cover` (spec §13: extraction
//! happens off the UI critical path, from already-decoded artwork bytes).
//! Fetch/decode failures propagate so the UI keeps its current colors instead
//! of resetting to brand teal (spec §5.2).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::palette::{extract_palette, Palette};

const ANALYSIS_SIZE: u32 = 160;
const EXTRACT_SIDE: u32 = 48;
/// Newest-N bound, mirroring `lyrics::CACHE_MAX_ENTRIES`; a palette entry is
/// tiny, but the unbounded map also made every save an O(n) full-file rewrite.
const PALETTE_MAX_ENTRIES: usize = 2_000;
/// Bump when `extract_palette`'s output changes: older files are discarded so
/// already-seen covers are refetched under the new rules.
const CACHE_VERSION: u32 = 4;

#[derive(Default, Serialize, Deserialize)]
struct DiskCache {
    #[serde(default)]
    version: u32,
    palettes: HashMap<String, PaletteDto>,
}

/// Borrowed view for serialization, so `save_disk` never clones the map.
#[derive(Serialize)]
struct DiskCacheRef<'a> {
    version: u32,
    palettes: &'a HashMap<String, PaletteDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteDto {
    pub primary: String,
    pub on_primary: String,
    pub secondary: String,
    pub tertiary: String,
    /// Added with the seek cap's backdrop wash; entries cached before it fail
    /// to parse once and the cache self-heals.
    pub backdrop: String,
    pub ceiling_l: f32,
    /// Unix seconds of the last fetch; pre-Phase-D entries default to 0 and
    /// prune first. Not part of the API surface.
    #[serde(default)]
    pub fetched_at: u64,
}

impl From<Palette> for PaletteDto {
    fn from(p: Palette) -> Self {
        Self {
            primary: p.primary,
            on_primary: p.on_primary,
            secondary: p.secondary,
            tertiary: p.tertiary,
            backdrop: p.backdrop,
            ceiling_l: p.ceiling_l,
            fetched_at: 0,
        }
    }
}
impl From<PaletteDto> for Palette {
    fn from(d: PaletteDto) -> Self {
        Self {
            primary: d.primary,
            on_primary: d.on_primary,
            secondary: d.secondary,
            tertiary: d.tertiary,
            backdrop: d.backdrop,
            ceiling_l: d.ceiling_l,
        }
    }
}

pub struct ArtworkService {
    http: reqwest::Client,
    base_url: String,
    memory: OnceLock<Mutex<HashMap<String, PaletteDto>>>,
    disk_path: PathBuf,
}

impl ArtworkService {
    pub fn new(http: reqwest::Client, base_url: String, disk_path: PathBuf) -> Self {
        if let Some(parent) = disk_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            memory: OnceLock::new(),
            disk_path,
        }
    }

    /// The palette map, read from disk on first use so startup never parses it.
    fn memory(&self) -> &Mutex<HashMap<String, PaletteDto>> {
        self.memory.get_or_init(|| {
            let memory = std::fs::read_to_string(&self.disk_path)
                .ok()
                .and_then(|text| serde_json::from_str::<DiskCache>(&text).ok())
                .filter(|cache| cache.version == CACHE_VERSION)
                .map(|cache| cache.palettes)
                .unwrap_or_default();
            Mutex::new(memory)
        })
    }

    fn url(base: &str, uuid: &str) -> String {
        format!("{base}/{}/{ANALYSIS_SIZE}x{ANALYSIS_SIZE}.jpg", uuid.replace('-', "/"))
    }

    /// `Err` means "no palette available right now" (empty uuid, HTTP error, or
    /// undecodable bytes): the caller keeps its current colors (spec §5.2).
    pub async fn palette(&self, uuid: &str) -> Result<Palette, String> {
        if uuid.is_empty() {
            return Err("empty cover uuid".into());
        }
        if let Some(hit) = self.memory().lock().unwrap().get(uuid) {
            return Ok(hit.clone().into());
        }
        let palette = match self.fetch(uuid).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("cover analysis failed for {uuid}: {e}");
                return Err(e);
            }
        };
        let mut dto = PaletteDto::from(palette.clone());
        dto.fetched_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.memory().lock().unwrap().insert(uuid.to_string(), dto);
        self.save_disk();
        Ok(palette)
    }

    async fn fetch(&self, uuid: &str) -> Result<Palette, String> {
        let bytes = self
            .http
            .get(Self::url(&self.base_url, uuid))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        let small = image::imageops::resize(
            &image.to_rgb8(),
            EXTRACT_SIDE,
            EXTRACT_SIDE,
            image::imageops::FilterType::Triangle,
        );
        let pixels: Vec<(u8, u8, u8)> = small.pixels().map(|p| (p[0], p[1], p[2])).collect();
        Ok(extract_palette(&pixels, EXTRACT_SIDE as usize, EXTRACT_SIDE as usize))
    }

    fn save_disk(&self) {
        let mut memory = self.memory().lock().unwrap();
        if memory.len() > PALETTE_MAX_ENTRIES {
            let mut entries: Vec<(String, PaletteDto)> = memory.drain().collect();
            entries.sort_by_key(|(_, dto)| std::cmp::Reverse(dto.fetched_at));
            entries.truncate(PALETTE_MAX_ENTRIES);
            memory.extend(entries);
        }
        let Ok(text) = serde_json::to_string_pretty(&DiskCacheRef { version: CACHE_VERSION, palettes: &memory }) else {
            return;
        };
        let tmp = self.disk_path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &self.disk_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::fallback_palette;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn png_bytes(color: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb(color));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn service(server: &MockServer, dir: &std::path::Path) -> ArtworkService {
        ArtworkService::new(reqwest::Client::new(), server.uri(), dir.join("palettes.json"))
    }

    fn temp_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "oxitide-artwork-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn palette_dto(primary: &str) -> PaletteDto {
        PaletteDto {
            primary: primary.into(),
            on_primary: "#000000".into(),
            secondary: "#111111".into(),
            tertiary: "#222222".into(),
            backdrop: "#333333".into(),
            ceiling_l: 0.5,
            fetched_at: 0,
        }
    }

    fn write_palettes(path: &std::path::Path, uuid: &str, dto: &PaletteDto) {
        let mut palettes = std::collections::HashMap::new();
        palettes.insert(uuid.to_string(), dto.clone());
        let text = serde_json::to_string(&DiskCache { version: CACHE_VERSION, palettes }).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// The constructor must not parse palettes.json; the first use loads what
    /// is on disk *at that moment*.
    #[tokio::test]
    async fn disk_palettes_load_lazily_on_first_use() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let dir = temp_dir();
        let path = dir.join("palettes.json");
        write_palettes(&path, "ab-cd", &palette_dto("#aaaaaa"));
        let service = service(&server, &dir);
        write_palettes(&path, "ab-cd", &palette_dto("#bbbbbb"));
        let palette = service.palette("ab-cd").await.unwrap();
        assert_eq!(palette.primary, "#bbbbbb");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn fetches_and_extracts_a_real_cover() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
            .expect(1)
            .mount(&server).await;
        let dir = temp_dir();
        let palette = service(&server, &dir).palette("ab-cd").await.unwrap();
        assert_ne!(palette.primary, fallback_palette().primary);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Review Focus: offline / 404 / garbage propagate so the UI keeps its
    /// current colors instead of resetting to brand teal (spec §5.2).
    #[tokio::test]
    async fn failures_propagate_to_keep_the_current_colors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server).await;
        let dir = temp_dir();
        let service = service(&server, &dir);
        assert!(service.palette("missing-uuid").await.is_err());
        assert!(service.palette("").await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_memory_cache_serves_repeats_without_http() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
            .expect(1) // second call must not hit the server
            .mount(&server).await;
        let dir = temp_dir();
        let service = service(&server, &dir);
        let first = service.palette("ab-cd").await.unwrap();
        let second = service.palette("ab-cd").await.unwrap();
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_disk_cache_survives_a_restart() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
            .expect(1)
            .mount(&server).await;
        let dir = temp_dir();
        let first = service(&server, &dir).palette("ab-cd").await.unwrap();
        drop(server); // server gone; a fresh service must read the disk cache
        let second = service(&MockServer::start().await, &dir).palette("ab-cd").await.unwrap();
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A palette written before versioning (no `version` field) must be
    /// discarded so a change in the extraction rules reaches covers seen
    /// before the change.
    #[tokio::test]
    async fn stale_cache_entries_are_discarded_on_version_mismatch() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
            .expect(1) // the stale entry must not suppress the refetch
            .mount(&server).await;
        let dir = temp_dir();
        std::fs::write(
            dir.join("palettes.json"),
            r##"{"palettes":{"ab-cd":{"primary":"#ffffff","on_primary":"#000000","secondary":"#111111","tertiary":"#222222","backdrop":"#333333","ceiling_l":0.1}}}"##,
        )
        .unwrap();
        let palette = service(&server, &dir).palette("ab-cd").await.unwrap();
        assert_ne!(palette.primary, "#ffffff");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn pruning_keeps_the_newest_palettes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let dir = temp_dir();
        let service = service(&server, &dir);
        {
            let mut memory = service.memory().lock().unwrap();
            for i in 0..(PALETTE_MAX_ENTRIES + 5) {
                let mut dto = palette_dto("#000000");
                dto.fetched_at = i as u64;
                memory.insert(format!("uuid-{i:05}"), dto);
            }
        }
        service.save_disk();

        let memory = service.memory().lock().unwrap();
        assert_eq!(memory.len(), PALETTE_MAX_ENTRIES);
        assert!(memory.contains_key("uuid-02004"), "the newest entry was pruned");
        assert!(!memory.contains_key("uuid-00000"), "the oldest entry survived");
        drop(memory);
        let text = std::fs::read_to_string(dir.join("palettes.json")).unwrap();
        let cache: DiskCache = serde_json::from_str(&text).unwrap();
        assert_eq!(cache.palettes.len(), PALETTE_MAX_ENTRIES);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn old_palette_files_without_fetched_at_still_parse() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;
        let dir = temp_dir();
        let path = dir.join("palettes.json");
        let text = format!(
            r##"{{"version":{CACHE_VERSION},"palettes":{{"ab-cd":{{"primary":"#aaaaaa","on_primary":"#000000","secondary":"#111111","tertiary":"#222222","backdrop":"#333333","ceiling_l":0.5}}}}}}"##
        );
        std::fs::write(&path, text).unwrap();
        let palette = service(&server, &dir).palette("ab-cd").await.unwrap();
        assert_eq!(palette.primary, "#aaaaaa");
        let _ = std::fs::remove_dir_all(dir);
    }
}
