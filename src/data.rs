//! Paths, config, CommunityDragon data and the bundled skin/chroma id table.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

pub const SKINS_RAW: &str = "https://raw.githubusercontent.com/Alban1911/LeagueSkins/main";
pub const DOWNLOAD_DOMAIN: &str = "raw.githubusercontent.com";
pub const LTK_RELEASES: &str = "https://github.com/LeagueToolkit/ltk-manager/releases/latest";

const CDRAGON: &str =
    "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global";

/// Real champion ids are < 1000. Since patch 16.23 CDragon also returns
/// same-named variants (e.g. 60001 "Jade_Annie") that showed up as skinless twins.
const MAX_CHAMP_ID: i64 = 1000;

pub const HISTORY_MAX: usize = 40;

static SKIN_IDS_JSON: &str = include_str!("../skin_ids.json");

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("HalitChanger")
}
pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}
pub fn img_cache_dir() -> PathBuf {
    cache_dir().join("img")
}
pub fn config_file() -> PathBuf {
    data_dir().join("config.json")
}
pub fn log_file() -> PathBuf {
    data_dir().join("halit.log")
}

pub fn ensure_dirs() {
    let _ = fs::create_dir_all(img_cache_dir());
}

pub fn log(msg: &str) {
    let now = local_hms();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_file()) {
        let _ = writeln!(f, "[{now}] {msg}");
    }
}

fn local_hms() -> String {
    // Local wall clock without pulling in a date crate.
    #[cfg(windows)]
    unsafe {
        let mut st = std::mem::zeroed::<windows_sys::Win32::Foundation::SYSTEMTIME>();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut st);
        return format!("{:02}:{:02}:{:02}", st.wHour, st.wMinute, st.wSecond);
    }
    #[allow(unreachable_code)]
    String::from("--:--:--")
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn http() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent("HalitChanger")
            .build()
            .into()
    })
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let mut resp = http().get(url).call().map_err(|e| e.to_string())?;
    resp.body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ config

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub champ_id: i64,
    pub champ_name: String,
    pub skin_id: i64,
    pub skin_name: String,
    #[serde(default)]
    pub chroma_id: Option<i64>,
    #[serde(default)]
    pub chroma_name: Option<String>,
    pub ts: u64,
}

impl HistoryEntry {
    pub fn label(&self) -> String {
        match &self.chroma_name {
            Some(c) => format!("{} ({c})", self.skin_name),
            None => self.skin_name.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default)]
    pub fav_champs: BTreeSet<i64>,
    #[serde(default)]
    pub fav_skins: BTreeSet<i64>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    /// Keys written by older versions are kept as-is.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Config {
    pub fn load() -> Self {
        fs::read_to_string(config_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let path = config_file();
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, s).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
    }
}

// ------------------------------------------------------------------ cdragon

#[derive(Clone, Debug)]
pub struct Champion {
    pub id: i64,
    pub name: String,
    pub name_lc: String,
    pub alias_lc: String,
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub id: i64,
    pub name: String,
    pub tile: String,
    pub splash: String,
    pub is_base: bool,
    pub rarity: &'static str,
}

#[derive(Clone, Debug)]
pub struct Chroma {
    pub id: i64,
    pub name: String,
}

/// Localised name, real colour and preview image of a chroma, from CDragon.
#[derive(Clone, Debug)]
pub struct ChromaMeta {
    pub name: String,
    pub rgb: Option<[u8; 3]>,
    pub preview: String,
}

pub struct GameData {
    pub champions: Vec<Champion>,
    pub skins_by_champ: HashMap<i64, Vec<Skin>>,
    pub chroma_meta: HashMap<i64, ChromaMeta>,
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// "Ay Canavarı Annie (Yakut)" -> "Yakut"
fn pack_name(full: &str) -> String {
    match (full.rfind(" ("), full.ends_with(')')) {
        (Some(p), true) => full[p + 2..full.len() - 1].to_string(),
        _ => full.to_string(),
    }
}

pub fn champions_url() -> String {
    format!("{CDRAGON}/tr_tr/v1/champion-summary.json")
}
pub fn skins_url() -> String {
    format!("{CDRAGON}/tr_tr/v1/skins.json")
}
pub fn champ_icon_url(cid: i64) -> String {
    format!("{CDRAGON}/default/v1/champion-icons/{cid}.png")
}

pub fn fantome_url(champ_id: i64, skin_id: i64, chroma_id: Option<i64>) -> String {
    match chroma_id {
        Some(c) => format!("{SKINS_RAW}/skins/{champ_id}/{skin_id}/{c}/{c}.fantome"),
        None => format!("{SKINS_RAW}/skins/{champ_id}/{skin_id}/{skin_id}.fantome"),
    }
}

fn cdragon_asset(game_path: &str) -> String {
    const PREFIX: &str = "/lol-game-data/assets/";
    if game_path.len() > PREFIX.len() && game_path[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        format!("{CDRAGON}/default/{}", game_path[PREFIX.len()..].to_lowercase())
    } else {
        String::new()
    }
}

fn nice_rarity(raw: &str) -> &'static str {
    match raw {
        "kEpic" => "Epic",
        "kLegendary" => "Legendary",
        "kMythic" => "Mythic",
        "kUltimate" => "Ultimate",
        "kTranscendent" => "Transcendent",
        "kExalted" => "Exalted",
        _ => "",
    }
}

/// Returns the cached JSON unless `force_network`; on network failure falls back to cache.
fn fetch_json_cached(url: &str, cache_name: &str, force_network: bool) -> Option<serde_json::Value> {
    let path = cache_dir().join(cache_name);
    let cached = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    if cached.is_some() && !force_network {
        return cached;
    }
    match get_bytes(url).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string())) {
        Ok(v) => {
            let tmp = path.with_extension("tmp");
            if let Ok(s) = serde_json::to_string(&v) {
                if fs::write(&tmp, s).is_ok() {
                    let _ = fs::rename(&tmp, &path);
                }
            }
            Some(v)
        }
        Err(e) => {
            log(&format!("fetch failed {url}: {e}"));
            cached
        }
    }
}

pub fn load_game_data(force_network: bool) -> Option<GameData> {
    let champs = fetch_json_cached(&champions_url(), "champions.json", force_network)?;
    let skins = fetch_json_cached(&skins_url(), "skins.json", force_network).unwrap_or_default();

    let mut champions: Vec<Champion> = champs
        .as_array()?
        .iter()
        .filter_map(|c| {
            let id = c.get("id")?.as_i64()?;
            if id <= 0 || id >= MAX_CHAMP_ID {
                return None;
            }
            let name = c.get("name")?.as_str()?.to_string();
            let alias = c.get("alias").and_then(|a| a.as_str()).unwrap_or(&name).to_string();
            Some(Champion {
                id,
                name_lc: name.to_lowercase(),
                alias_lc: alias.to_lowercase(),
                name,
            })
        })
        .collect();
    if champions.is_empty() {
        return None;
    }
    champions.sort_by(|a, b| a.name_lc.cmp(&b.name_lc));

    let mut by_champ: HashMap<i64, Vec<Skin>> = HashMap::new();
    let mut chroma_meta = HashMap::new();
    if let Some(map) = skins.as_object() {
        for (sid_str, s) in map {
            for ch in s.get("chromas").and_then(|c| c.as_array()).into_iter().flatten() {
                let Some(cid) = ch.get("id").and_then(|v| v.as_i64()) else { continue };
                let rgb = ch
                    .get("colors")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|c| c.as_str())
                    .and_then(parse_hex);
                chroma_meta.insert(cid, ChromaMeta {
                    name: pack_name(ch.get("name").and_then(|v| v.as_str()).unwrap_or("")),
                    rgb,
                    preview: cdragon_asset(ch.get("chromaPath").and_then(|v| v.as_str()).unwrap_or("")),
                });
            }
            let Ok(sid) = sid_str.parse::<i64>() else { continue };
            let str_of = |k: &str| s.get(k).and_then(|v| v.as_str()).unwrap_or("");
            let splash_path = {
                let u = str_of("uncenteredSplashPath");
                if u.is_empty() { str_of("splashPath") } else { u }
            };
            by_champ.entry(sid / 1000).or_default().push(Skin {
                id: sid,
                name: s.get("name").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(|| sid.to_string()),
                tile: cdragon_asset(str_of("tilePath")),
                splash: cdragon_asset(splash_path),
                is_base: s.get("isBase").and_then(|v| v.as_bool()).unwrap_or(false),
                rarity: nice_rarity(str_of("rarity")),
            });
        }
    }
    for list in by_champ.values_mut() {
        list.sort_by_key(|s| s.id);
    }
    Some(GameData { champions, skins_by_champ: by_champ, chroma_meta })
}

// ------------------------------------------------------------------ chromas

/// skin_ids.json: "1013": "Lunar Beast Annie", "1014": "Lunar Beast Annie (Ruby)"
pub struct ChromaIndex {
    id_to_en: HashMap<i64, String>,
    by_parent: HashMap<String, Vec<Chroma>>,
}

impl ChromaIndex {
    pub fn load() -> Self {
        let raw: HashMap<String, serde_json::Value> =
            serde_json::from_str(SKIN_IDS_JSON).unwrap_or_default();
        let mut id_to_en = HashMap::new();
        let mut by_parent: HashMap<String, Vec<Chroma>> = HashMap::new();
        for (sid_str, name) in raw {
            let (Ok(sid), Some(name)) = (sid_str.parse::<i64>(), name.as_str()) else { continue };
            if name.ends_with(')') {
                if let Some(p) = name.rfind(" (") {
                    let parent = &name[..p];
                    let pack = &name[p + 2..name.len() - 1];
                    if !parent.is_empty() && !pack.is_empty() {
                        by_parent
                            .entry(parent.to_string())
                            .or_default()
                            .push(Chroma { id: sid, name: pack.to_string() });
                    }
                }
            }
            id_to_en.insert(sid, name.to_string());
        }
        for list in by_parent.values_mut() {
            list.sort_by_key(|c| c.id);
        }
        Self { id_to_en, by_parent }
    }

    pub fn chromas(&self, skin_id: i64) -> &[Chroma] {
        self.id_to_en
            .get(&skin_id)
            .and_then(|en| self.by_parent.get(en))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

pub fn chroma_rgb(name: &str) -> [u8; 3] {
    const TABLE: &[(&str, u32)] = &[
        ("Ruby", 0xdc2626), ("Obsidian", 0x1a1a1a), ("Pearl", 0xf0e6d4),
        ("Sapphire", 0x2563eb), ("Emerald", 0x059669), ("Amethyst", 0x9333ea),
        ("Citrine", 0xeab308), ("Rose Quartz", 0xf472b6), ("Turquoise", 0x14b8a6),
        ("Aquamarine", 0x34d399), ("Tanzanite", 0x6366f1), ("Sandstone", 0xc2a068),
        ("Granite", 0x6b7280), ("Peridot", 0x84cc16), ("Catseye", 0xf59e0b),
        ("Rainbow", 0xa855f7), ("Meteorite", 0x475569), ("Paragon", 0xfacc15),
        ("Resolute", 0x0ea5e9), ("Elite", 0xd97706), ("Gilded", 0xf59e0b),
        ("Chrono", 0x06b6d4), ("Merc", 0x64748b), ("Underground", 0x57534e),
        ("Worlds Early Bird", 0xf97316), ("Vitality", 0x22d3ee), ("Nomad", 0xa16207),
        ("Disco", 0xc084fc), ("Ace", 0xf43f5e), ("Night Blossom", 0xbe185d),
        ("Dark Ritual", 0x4a1942), ("Unlocked", 0x10b981), ("Worthy", 0xf59e0b),
        ("Neon Facade", 0x00ff88), ("Hunter", 0x15803d), ("MSI 2024 Chaos", 0xe11d48),
        ("Strike Gold", 0xf59e0b), ("Strike Aqua", 0x06b6d4), ("Strike Crimson", 0xdc2626),
        ("Reckoning", 0xbe123c), ("Punk Navy", 0x1e3a5f), ("Punk Purple", 0x6b21a8),
        ("Punk Orange", 0xc2410c), ("Speckled", 0x78716c), ("Golden", 0xf59e0b),
        ("Golden Tiger", 0xf59e0b), ("Worlds 2019", 0xf59e0b), ("Wreathguard", 0x059669),
        ("Ghoulish", 0x15803d), ("K.O.", 0xef4444), ("Soccer Cup", 0x16a34a),
        ("Cerulean Club", 0x2563eb), ("Cursed", 0x7f1d1d), ("Tenfold Triumph", 0xf59e0b),
        ("Mythclimber", 0xa855f7), ("Vivid", 0xf43f5e), ("Inked", 0x1a1a1a),
        ("Lustrous", 0xf59e0b), ("Bronze", 0xcd7f32), ("Silver", 0xc0c0c0),
        ("Gold", 0xffd700), ("Platinum", 0xe5e4e2), ("Diamond", 0xb9f2ff),
        ("Master", 0x9333ea), ("Grandmaster", 0xef4444), ("Challenger", 0xf59e0b),
    ];
    if let Some((_, c)) = TABLE.iter().find(|(n, _)| *n == name) {
        return [(c >> 16) as u8, (c >> 8) as u8, *c as u8];
    }
    // Stable pseudo-colour for unknown pack names.
    let h = (fnv1a(name.as_bytes()) % 360) as u32;
    [
        ((h * 97) % 200 + 40) as u8,
        ((h * 57) % 180 + 40) as u8,
        ((h * 31) % 200 + 40) as u8,
    ]
}

pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chroma_index_parses_bundled_table() {
        let idx = ChromaIndex::load();
        assert!(idx.id_to_en.len() > 1000);
        assert!(idx.by_parent.values().any(|v| !v.is_empty()));
    }

    #[test]
    fn asset_path_maps_to_cdragon() {
        let u = cdragon_asset("/lol-game-data/assets/ASSETS/Characters/Annie/Skins/Skin01/Images/x.jpg");
        assert!(u.ends_with("/default/assets/characters/annie/skins/skin01/images/x.jpg"));
        assert_eq!(cdragon_asset("nope"), "");
    }

    #[test]
    fn hex_and_pack() {
        assert_eq!(parse_hex("#D33528"), Some([0xd3, 0x35, 0x28]));
        assert_eq!(pack_name("Ay Canavarı Annie (Yakut)"), "Yakut");
    }

    #[test]
    fn fantome_urls() {
        assert!(fantome_url(1, 1013, None).ends_with("/skins/1/1013/1013.fantome"));
        assert!(fantome_url(1, 1013, Some(1014)).ends_with("/skins/1/1013/1014/1014.fantome"));
    }

    #[test]
    fn config_keeps_unknown_keys() {
        let c: Config = serde_json::from_str(r#"{"lang":"tr","fav_champs":[1,2],"other":5}"#).unwrap();
        assert_eq!(c.fav_champs.len(), 2);
        let s = serde_json::to_string(&c).unwrap();
        assert!(s.contains("\"other\":5"));
    }
}
