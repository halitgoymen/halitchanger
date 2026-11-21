//! The egui interface.

use crate::data::{self, Champion, ChromaIndex, ChromaMeta, Config, GameData, HistoryEntry, Skin};
use crate::i18n::{t, Lang, S};
use crate::images::ImageLoader;
use crate::ltk;
use egui::{
    vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Id, Key, Margin, Modifiers,
    Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

// ------------------------------------------------------------------ theme

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}
const BG: Color32 = hex(0x07070c);
const BG2: Color32 = hex(0x0b0b12);
const PANEL: Color32 = hex(0x101018);
const CARD: Color32 = hex(0x161622);
const CARD_HOV: Color32 = hex(0x1c1c2c);
const CARD_SEL: Color32 = hex(0x1a1430);
const BORDER: Color32 = hex(0x262636);
const BORDER_HOV: Color32 = hex(0x5b4a8a);
const GOLD: Color32 = hex(0xf0c050);
const GOLD_DK: Color32 = hex(0xc8963c);
const PURPLE: Color32 = hex(0x8b5cf6);
const PURPLE_DK: Color32 = hex(0x6d28d9);
const PURPLE_HOV: Color32 = hex(0xa78bfa);
const PURPLE_DIM: Color32 = hex(0x3b2268);
const GREEN: Color32 = hex(0x22c55e);
const GREEN_DIM: Color32 = hex(0x0f2e1c);
const RED: Color32 = hex(0xef4444);
const RED_DIM: Color32 = hex(0x3a1212);
const TEXT: Color32 = hex(0xf2f0f8);
const TEXT2: Color32 = hex(0xc4c2ce);
const MUTED: Color32 = hex(0x6e6e84);
const ON_GOLD: Color32 = hex(0x1a1400);
const DEFAULT_DOT: Color32 = hex(0xd4d4de);

const CHAMP_ROW: f32 = 40.0;
const CARD_W: f32 = 232.0;
const CARD_H: f32 = 236.0;
const CARD_GAP_Y: f32 = 12.0;
const SKIN_IMG: Vec2 = vec2(216.0, 122.0);
const MAX_DOTS: usize = 10;

fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
fn reg(size: f32) -> FontId {
    FontId::proportional(size)
}

// ------------------------------------------------------------------ state

enum Msg {
    Loader(S),
    Data(GameData),
    Failed,
    Ltk(bool),
    Toast(S, bool),
    Sent(HistoryEntry, bool),
}

struct Toast {
    text: String,
    ok: bool,
    until: f64,
}

#[derive(Clone, Copy)]
struct Detail {
    skin_id: i64,
    chroma: Option<i64>,
}

#[derive(Clone)]
struct ChromaView {
    id: Option<i64>,
    name: String,
    color: Color32,
    preview: String,
}

pub struct HalitApp {
    lang: Lang,
    cfg: Config,
    chroma_index: ChromaIndex,
    data: Option<GameData>,
    ready: bool,
    load_failed: bool,
    loader_text: S,
    selected: Option<i64>,
    search: String,
    skin_search: String,
    card_chroma: HashMap<i64, Option<i64>>,
    sent_until: HashMap<i64, f64>,
    detail: Option<Detail>,
    show_settings: bool,
    show_history: bool,
    ltk: Option<bool>,
    next_probe: f64,
    probing: Arc<AtomicBool>,
    toasts: Vec<Toast>,
    imgs: ImageLoader,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    logo: Option<TextureHandle>,
    titlebar_done: bool,
}

impl HalitApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        setup_fonts(&ctx);
        setup_style(&ctx);
        data::ensure_dirs();
        data::log("=== Halit Changer (Rust) started ===");

        let cfg = Config::load();
        let lang = cfg
            .lang
            .as_deref()
            .and_then(Lang::from_code)
            .unwrap_or(if ltk::detect_turkish_ui() { Lang::Tr } else { Lang::En });
        let (tx, rx) = channel();

        let logo = image::load_from_memory(include_bytes!("../assets/logo.png")).ok().map(|img| {
            let img = img.resize(96, 96, image::imageops::FilterType::Lanczos3).to_rgba8();
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            );
            ctx.load_texture("logo", ci, egui::TextureOptions::LINEAR)
        });

        let app = Self {
            lang,
            cfg,
            chroma_index: ChromaIndex::load(),
            data: None,
            ready: false,
            load_failed: false,
            loader_text: S::loading_data,
            selected: None,
            search: String::new(),
            skin_search: String::new(),
            card_chroma: HashMap::new(),
            sent_until: HashMap::new(),
            detail: None,
            show_settings: false,
            show_history: false,
            ltk: None,
            next_probe: 0.0,
            probing: Arc::new(AtomicBool::new(false)),
            toasts: Vec::new(),
            imgs: ImageLoader::new(ctx.clone()),
            tx,
            rx,
            logo,
            titlebar_done: false,
        };
        app.spawn_loader(ctx.clone());
        app.spawn_ltk_autostart(ctx);
        app
    }

    fn tr(&self, s: S) -> &'static str {
        t(self.lang, s)
    }

    // -------------------------------------------------------------- background work

    fn spawn_loader(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let send = |m: Msg| {
                let _ = tx.send(m);
                ctx.request_repaint();
            };
            send(Msg::Loader(S::loading_data));
            if let Some(d) = data::load_game_data(false) {
                send(Msg::Loader(S::preparing_list));
                send(Msg::Data(d));
                // Refresh the cache quietly in the background.
                if let Some(d) = data::load_game_data(true) {
                    send(Msg::Data(d));
                }
            } else {
                send(Msg::Loader(S::first_launch));
                match data::load_game_data(true) {
                    Some(d) => send(Msg::Data(d)),
                    None => send(Msg::Failed),
                }
            }
            ltk::prepare_trusted_domain();
        });
    }

    fn spawn_ltk_autostart(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let running = if ltk::is_running() {
                data::log("LTK already running.");
                true
            } else if ltk::shell_open("ltk://") {
                data::log("LTK auto-started via protocol.");
                std::thread::sleep(std::time::Duration::from_millis(2500));
                ltk::is_running()
            } else {
                data::log("LTK not found.");
                let _ = tx.send(Msg::Toast(S::ltk_not_started, false));
                false
            };
            let _ = tx.send(Msg::Ltk(running));
            ctx.request_repaint();
        });
    }

    fn open_ltk(&self, ctx: &egui::Context) {
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            if ltk::shell_open("ltk://") {
                data::log("Opening LTK...");
                std::thread::sleep(std::time::Duration::from_millis(2000));
                let _ = tx.send(Msg::Ltk(ltk::is_running()));
            } else {
                data::log("LTK not found - download page opened.");
                ltk::shell_open(data::LTK_RELEASES);
                let _ = tx.send(Msg::Toast(S::ltk_not_found_dl, false));
                let _ = tx.send(Msg::Ltk(false));
            }
            ctx.request_repaint();
        });
    }

    fn poll_ltk(&mut self, ctx: &egui::Context, now: f64) {
        if now < self.next_probe || self.probing.swap(true, Ordering::SeqCst) {
            return;
        }
        let delay = if self.ltk == Some(true) { 5.0 } else { 2.0 };
        self.next_probe = now + delay;
        let tx = self.tx.clone();
        let probing = self.probing.clone();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Ltk(ltk::is_running()));
            probing.store(false, Ordering::SeqCst);
            c.request_repaint();
        });
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(delay));
    }

    fn drain(&mut self, now: f64) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Loader(s) => self.loader_text = s,
                Msg::Data(d) => {
                    if !self.ready {
                        data::log(&format!("Ready. {} champions loaded.", d.champions.len()));
                    }
                    self.data = Some(d);
                    self.ready = true;
                    self.load_failed = false;
                }
                Msg::Failed => {
                    data::log("ERROR: champion data unavailable");
                    self.load_failed = true;
                    self.loader_text = S::load_failed;
                }
                Msg::Ltk(r) => self.ltk = Some(r),
                Msg::Toast(s, ok) => self.toast(t(self.lang, s).to_string(), ok, now),
                Msg::Sent(entry, ok) => {
                    if ok {
                        data::log(&format!("  -> Sent to LTK: {}", entry.label()));
                        let msg = t(self.lang, S::sent_toast).replace("{label}", &entry.label());
                        self.toast(msg, true, now);
                        self.sent_until.insert(entry.skin_id, now + 1.6);
                        self.push_history(entry);
                    } else {
                        data::log("  X LTK send failed");
                        self.toast(t(self.lang, S::send_failed).to_string(), false, now);
                    }
                }
            }
        }
    }

    fn toast(&mut self, text: String, ok: bool, now: f64) {
        self.toasts.push(Toast { text, ok, until: now + 2.8 });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    // -------------------------------------------------------------- domain helpers

    fn champ(&self, id: i64) -> Option<&Champion> {
        self.data.as_ref()?.champions.iter().find(|c| c.id == id)
    }

    fn skin(&self, skin_id: i64) -> Option<&Skin> {
        self.data.as_ref()?.skins_by_champ.get(&(skin_id / 1000))?.iter().find(|s| s.id == skin_id)
    }

    fn ordered_champs(&self) -> Vec<&Champion> {
        let Some(d) = &self.data else { return vec![] };
        let term = self.search.trim().to_lowercase();
        let mut v: Vec<&Champion> = d
            .champions
            .iter()
            .filter(|c| term.is_empty() || c.name_lc.contains(&term) || c.alias_lc.contains(&term))
            .collect();
        v.sort_by_key(|c| !self.cfg.fav_champs.contains(&c.id));
        v
    }

    fn visible_skins(&self) -> Vec<Skin> {
        let (Some(d), Some(cid)) = (&self.data, self.selected) else { return vec![] };
        let term = self.skin_search.trim().to_lowercase();
        let mut v: Vec<Skin> = d
            .skins_by_champ
            .get(&cid)
            .map(|l| {
                l.iter()
                    .filter(|s| !s.is_base)
                    .filter(|s| term.is_empty() || s.name.to_lowercase().contains(&term))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|s| !self.cfg.fav_skins.contains(&s.id));
        v
    }

    /// Chromas that actually exist in the LeagueSkins repo (bundled id table),
    /// enriched with CDragon's localised name, real colour and preview.
    fn chromas(&self, skin_id: i64) -> Vec<ChromaView> {
        let meta = self.data.as_ref().map(|d| &d.chroma_meta);
        self.chroma_index
            .chromas(skin_id)
            .iter()
            .map(|c| {
                let m: Option<&ChromaMeta> = meta.and_then(|m| m.get(&c.id));
                let rgb = m.and_then(|m| m.rgb).unwrap_or_else(|| data::chroma_rgb(&c.name));
                let name = match (self.lang, m) {
                    (Lang::Tr, Some(m)) if !m.name.is_empty() => m.name.clone(),
                    _ => c.name.clone(),
                };
                ChromaView {
                    id: Some(c.id),
                    name,
                    color: Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                    preview: m.map(|m| m.preview.clone()).unwrap_or_default(),
                }
            })
            .collect()
    }

    fn select_champion(&mut self, id: i64) {
        if self.selected != Some(id) {
            self.selected = Some(id);
            self.skin_search.clear();
        }
    }

    fn persist(&self) {
        let mut cfg = self.cfg.clone();
        cfg.lang = Some(self.lang.code().into());
        cfg.save();
    }

    fn toggle_fav_champ(&mut self, id: i64) {
        if !self.cfg.fav_champs.remove(&id) {
            self.cfg.fav_champs.insert(id);
        }
        self.persist();
    }

    fn toggle_fav_skin(&mut self, id: i64) {
        if !self.cfg.fav_skins.remove(&id) {
            self.cfg.fav_skins.insert(id);
        }
        self.persist();
    }

    fn push_history(&mut self, e: HistoryEntry) {
        self.cfg
            .history
            .retain(|h| !(h.skin_id == e.skin_id && h.chroma_id == e.chroma_id));
        self.cfg.history.insert(0, e);
        self.cfg.history.truncate(data::HISTORY_MAX);
        self.persist();
    }

    fn send(&mut self, ctx: &egui::Context, skin_id: i64, chroma: Option<i64>, now: f64) {
        if !self.ready {
            self.toast(self.tr(S::still_loading).into(), false, now);
            return;
        }
        let champ_id = skin_id / 1000;
        let (Some(champ), Some(skin)) = (self.champ(champ_id), self.skin(skin_id)) else { return };
        let chroma_name = chroma.and_then(|cid| {
            self.chromas(skin_id).into_iter().find(|c| c.id == Some(cid)).map(|c| c.name)
        });
        let entry = HistoryEntry {
            champ_id,
            champ_name: champ.name.clone(),
            skin_id,
            skin_name: skin.name.clone(),
            chroma_id: chroma,
            chroma_name,
            ts: data::unix_now(),
        };
        self.send_entry(ctx, entry);
    }

    fn send_entry(&self, ctx: &egui::Context, mut entry: HistoryEntry) {
        entry.ts = data::unix_now();
        let url = data::fantome_url(entry.champ_id, entry.skin_id, entry.chroma_id);
        let link = ltk::install_link(&url, &format!("{} - {}", entry.champ_name, entry.label()));
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let ok = ltk::shell_open(&link);
            let _ = tx.send(Msg::Sent(entry, ok));
            ctx.request_repaint();
        });
    }

    /// 🎲 Random: a skin of the selected champion, or of a favourite/any champion,
    /// with a random chroma half of the time. Opens the detail view to confirm.
    fn random_pick(&mut self) {
        let Some(d) = &self.data else { return };
        let pool: Vec<i64> = match self.selected {
            Some(c) => vec![c],
            None => {
                let favs: Vec<i64> = d
                    .champions
                    .iter()
                    .filter(|c| self.cfg.fav_champs.contains(&c.id))
                    .map(|c| c.id)
                    .collect();
                if favs.is_empty() { d.champions.iter().map(|c| c.id).collect() } else { favs }
            }
        };
        for _ in 0..8 {
            let Some(&cid) = pool.get(fastrand::usize(..pool.len().max(1))) else { return };
            let skins: Vec<i64> = d
                .skins_by_champ
                .get(&cid)
                .map(|l| l.iter().filter(|s| !s.is_base).map(|s| s.id).collect())
                .unwrap_or_default();
            if skins.is_empty() {
                continue;
            }
            let mut sid = skins[fastrand::usize(..skins.len())];
            // Avoid re-rolling the exact skin already on screen.
            if skins.len() > 1 && self.detail.map(|x| x.skin_id) == Some(sid) {
                sid = skins[(skins.iter().position(|&s| s == sid).unwrap() + 1) % skins.len()];
            }
            let chromas = self.chroma_index.chromas(sid);
            let chroma = (!chromas.is_empty() && fastrand::bool())
                .then(|| chromas[fastrand::usize(..chromas.len())].id);
            self.select_champion(cid);
            self.card_chroma.insert(sid, chroma);
            self.detail = Some(Detail { skin_id: sid, chroma });
            return;
        }
    }

    // -------------------------------------------------------------- ui pieces

    fn header(&mut self, ui: &mut Ui, now: f64) {
        let ctx = ui.ctx().clone();
        ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            if let Some(logo) = &self.logo {
                ui.add(egui::Image::new((logo.id(), vec2(36.0, 36.0))));
                ui.add_space(6.0);
            }
            ui.label(RichText::new("HALIT").font(bold(19.0)).color(GOLD));
            ui.add_space(-4.0);
            ui.label(RichText::new(" CHANGER").font(bold(19.0)).color(PURPLE_HOV));
            ui.label(RichText::new("a1.2").font(reg(11.0)).color(MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                // language toggle
                for lang in [Lang::En, Lang::Tr] {
                    let on = self.lang == lang;
                    let b = egui::Button::new(
                        RichText::new(lang.code().to_uppercase()).font(bold(12.0)).color(if on { TEXT } else { MUTED }),
                    )
                    .fill(if on { PURPLE_DK } else { CARD })
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8)
                    .min_size(vec2(42.0, 30.0));
                    if ui.add(b).clicked() && !on {
                        self.lang = lang;
                        self.persist();
                    }
                }
                ui.add_space(6.0);
                if header_btn(ui, self.tr(S::settings)).clicked() {
                    self.show_settings = true;
                }
                let hist = format!("{}  {}", self.tr(S::history), self.cfg.history.len());
                if header_btn(ui, &hist).clicked() {
                    self.show_history = true;
                }
                if header_btn(ui, self.tr(S::random)).on_hover_text(self.tr(S::random_hint)).clicked() {
                    self.random_pick();
                }
                ui.add_space(6.0);
                // LTK pill
                let (txt, fg, bg) = match self.ltk {
                    None => (self.tr(S::ltk_checking), MUTED, CARD),
                    Some(true) => (self.tr(S::ltk_connected), GREEN, GREEN_DIM),
                    Some(false) => (self.tr(S::ltk_offline), RED, RED_DIM),
                };
                let pill = egui::Button::new(RichText::new(txt).font(bold(12.0)).color(fg))
                    .fill(bg)
                    .stroke(Stroke::new(1.0, if self.ltk.is_some() { fg } else { BORDER }))
                    .corner_radius(16)
                    .min_size(vec2(0.0, 30.0));
                if ui.add(pill).on_hover_text(self.tr(S::open_restart_ltk)).clicked() {
                    self.open_ltk(&ctx);
                }
                ui.add_space(10.0);
                let status = if self.ready {
                    let n = self.data.as_ref().map_or(0, |d| d.champions.len());
                    self.tr(S::ready).replace("{n}", &n.to_string())
                } else if self.load_failed {
                    self.tr(S::error_status).to_string()
                } else {
                    self.tr(S::loading_status).to_string()
                };
                ui.label(RichText::new(status).font(reg(12.0)).color(MUTED));
            });
        });
        let _ = now;
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(self.tr(S::champions_header)).font(bold(12.0)).color(MUTED));
        });
        ui.add_space(6.0);
        let search_id = Id::unique("champ_search");
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .id(search_id)
                    .hint_text(RichText::new(t(self.lang, S::search_placeholder)).color(MUTED))
                    .font(reg(14.0))
                    .margin(Margin::symmetric(10, 8))
                    .desired_width(ui.available_width() - 4.0),
            );
        });
        if ui.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::F)) {
            ui.memory_mut(|m| m.request_focus(search_id));
        }
        ui.add_space(8.0);

        let champs: Vec<(i64, String)> =
            self.ordered_champs().into_iter().map(|c| (c.id, c.name.clone())).collect();
        // Enter in the search box picks the first match.
        if ui.memory(|m| m.has_focus(search_id)) && ui.input(|i| i.key_pressed(Key::Enter)) {
            if let Some((id, _)) = champs.first() {
                self.select_champion(*id);
            }
        }
        let mut clicked = None;
        let mut fav_clicked = None;
        egui::ScrollArea::vertical()
            .id_salt("champ_list")
            .auto_shrink(false)
            .show_rows(ui, CHAMP_ROW, champs.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (id, name) in &champs[range] {
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, CHAMP_ROW), Sense::click());
                    let on = self.selected == Some(*id);
                    let row = rect.shrink2(vec2(4.0, 2.0));
                    let fill = if on {
                        CARD_SEL
                    } else if resp.hovered() {
                        CARD_HOV
                    } else {
                        PANEL
                    };
                    let p = ui.painter();
                    p.rect(row, 8, fill, Stroke::new(1.0, if on { PURPLE } else { fill }), StrokeKind::Inside);
                    let icon_rect = Rect::from_center_size(row.left_center() + vec2(18.0, 0.0), vec2(24.0, 24.0));
                    let px = (24.0 * ui.ctx().pixels_per_point()).round() as u32;
                    if let Some(tex) = self.imgs.get(&data::champ_icon_url(*id), px, px) {
                        egui::Image::new((tex.id(), icon_rect.size()))
                            .corner_radius(12)
                            .paint_at(ui, icon_rect);
                    } else {
                        ui.painter().circle_filled(icon_rect.center(), 12.0, CARD);
                    }
                    let p = ui.painter();
                    p.text(
                        row.left_center() + vec2(38.0, 0.0),
                        Align2::LEFT_CENTER,
                        name,
                        reg(14.0),
                        if on { TEXT } else { TEXT2 },
                    );
                    let fav = self.cfg.fav_champs.contains(id);
                    let star_rect = Rect::from_center_size(row.right_center() - vec2(14.0, 0.0), vec2(26.0, 26.0));
                    let star_hover = ui.rect_contains_pointer(star_rect);
                    p.text(
                        star_rect.center(),
                        Align2::CENTER_CENTER,
                        if fav { "★" } else { "☆" },
                        reg(15.0),
                        if fav { GOLD } else if star_hover { TEXT2 } else { MUTED },
                    );
                    if resp.clicked() {
                        if star_hover {
                            fav_clicked = Some(*id);
                        } else {
                            clicked = Some(*id);
                        }
                    }
                }
            });
        if let Some(id) = fav_clicked {
            self.toggle_fav_champ(id);
        }
        if let Some(id) = clicked {
            self.select_champion(id);
        }
    }

    fn center(&mut self, ui: &mut Ui, now: f64) {
        let ctx = ui.ctx().clone();
        let skins = self.visible_skins();
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            match self.selected.and_then(|id| self.champ(id).cloned()) {
                Some(champ) => {
                    let px = (40.0 * ui.ctx().pixels_per_point()).round() as u32;
                    if let Some(tex) = self.imgs.get(&data::champ_icon_url(champ.id), px, px) {
                        ui.add(egui::Image::new((tex.id(), vec2(40.0, 40.0))).corner_radius(20));
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&champ.name).font(bold(23.0)).color(TEXT));
                        let total = self
                            .data
                            .as_ref()
                            .and_then(|d| d.skins_by_champ.get(&champ.id))
                            .map_or(0, |l| l.iter().filter(|s| !s.is_base).count());
                        ui.label(
                            RichText::new(self.tr(S::skins_count).replace("{n}", &total.to_string()))
                                .font(reg(13.0))
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        let fav = self.cfg.fav_champs.contains(&champ.id);
                        let b = egui::Button::new(
                            RichText::new(self.tr(if fav { S::favorite_on } else { S::favorite }))
                                .font(reg(13.0))
                                .color(if fav { GOLD } else { TEXT2 }),
                        )
                        .fill(CARD)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(8)
                        .min_size(vec2(110.0, 32.0));
                        if ui.add(b).clicked() {
                            self.toggle_fav_champ(champ.id);
                        }
                        ui.add_space(8.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.skin_search)
                                .hint_text(RichText::new(t(self.lang, S::skin_search)).color(MUTED))
                                .font(reg(13.0))
                                .margin(Margin::symmetric(10, 7))
                                .desired_width(200.0),
                        );
                    });
                }
                None => {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.tr(S::select_champion)).font(bold(23.0)).color(TEXT));
                        ui.label(RichText::new(self.tr(S::select_champion_sub)).font(reg(13.0)).color(MUTED));
                    });
                }
            }
        });
        ui.add_space(10.0);

        if self.selected.is_none() || skins.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                let s = if self.selected.is_none() { S::pick_champion } else { S::no_skin_match };
                ui.label(RichText::new(self.tr(s)).font(reg(16.0)).color(MUTED));
            });
            return;
        }

        let avail = ui.available_width() - 12.0;
        let cols = (((avail - 8.0) / (CARD_W + 8.0)).floor() as usize).clamp(1, 6);
        let gap = ((avail - cols as f32 * CARD_W) / (cols as f32 + 1.0)).max(8.0);
        let rows = skins.len().div_ceil(cols);
        let mut actions: Vec<CardAction> = Vec::new();

        egui::ScrollArea::vertical()
            .id_salt(("skins", self.selected))
            .auto_shrink(false)
            .show_rows(ui, CARD_H + CARD_GAP_Y, rows, |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for r in range {
                    let (row_rect, _) =
                        ui.allocate_exact_size(vec2(avail, CARD_H + CARD_GAP_Y), Sense::hover());
                    for c in 0..cols {
                        let Some(skin) = skins.get(r * cols + c) else { break };
                        let x = row_rect.left() + gap + c as f32 * (CARD_W + gap);
                        let rect = Rect::from_min_size(egui::pos2(x, row_rect.top()), vec2(CARD_W, CARD_H));
                        if let Some(a) = self.skin_card(ui, rect, skin, now) {
                            actions.push(a);
                        }
                    }
                }
            });

        for a in actions {
            match a {
                CardAction::Open(sid) => {
                    let chroma = self.card_chroma.get(&sid).copied().flatten();
                    self.detail = Some(Detail { skin_id: sid, chroma });
                }
                CardAction::Chroma(sid, ch) => {
                    self.card_chroma.insert(sid, ch);
                }
                CardAction::Add(sid) => {
                    let chroma = self.card_chroma.get(&sid).copied().flatten();
                    self.send(&ctx, sid, chroma, now);
                }
                CardAction::Fav(sid) => self.toggle_fav_skin(sid),
            }
        }
    }

    fn skin_card(&mut self, ui: &mut Ui, rect: Rect, skin: &Skin, now: f64) -> Option<CardAction> {
        let mut action = None;
        let id = Id::unique(("card", skin.id));
        let hovered = ui.rect_contains_pointer(rect);
        ui.painter().rect(
            rect,
            10,
            if hovered { CARD_HOV } else { CARD },
            Stroke::new(1.0, if hovered { BORDER_HOV } else { BORDER }),
            StrokeKind::Inside,
        );

        // image
        let img_rect = Rect::from_min_size(rect.min + vec2(8.0, 8.0), SKIN_IMG);
        let ppp = ui.ctx().pixels_per_point();
        let url = if skin.tile.is_empty() { &skin.splash } else { &skin.tile };
        let img_resp = ui.interact(img_rect, id.with("img"), Sense::click());
        match self.imgs.get(url, (SKIN_IMG.x * ppp) as u32, (SKIN_IMG.y * ppp) as u32) {
            Some(tex) => {
                egui::Image::new((tex.id(), SKIN_IMG)).corner_radius(7).paint_at(ui, img_rect);
            }
            None => {
                ui.painter().rect_filled(img_rect, 7, BG);
            }
        }
        if img_resp.hovered() {
            ui.painter().rect_stroke(img_rect, 7, Stroke::new(1.0, PURPLE_HOV), StrokeKind::Inside);
        }
        if img_resp.clicked() {
            action = Some(CardAction::Open(skin.id));
        }

        // rarity tag + favourite star on the image
        if !skin.rarity.is_empty() {
            let galley = ui.painter().layout_no_wrap(skin.rarity.to_string(), bold(10.0), GOLD);
            let tag = Rect::from_min_size(img_rect.min + vec2(6.0, 6.0), galley.size() + vec2(10.0, 4.0));
            ui.painter().rect_filled(tag, 5, Color32::from_black_alpha(170));
            ui.painter().galley(tag.min + vec2(5.0, 2.0), galley, GOLD);
        }
        let fav = self.cfg.fav_skins.contains(&skin.id);
        let star = Rect::from_center_size(img_rect.right_top() + vec2(-15.0, 15.0), vec2(24.0, 24.0));
        let star_resp = ui.interact(star, id.with("star"), Sense::click());
        if fav || hovered {
            ui.painter().circle_filled(star.center(), 12.0, Color32::from_black_alpha(160));
            ui.painter().text(
                star.center(),
                Align2::CENTER_CENTER,
                if fav { "★" } else { "☆" },
                reg(14.0),
                if fav { GOLD } else if star_resp.hovered() { TEXT } else { TEXT2 },
            );
        }
        if star_resp.on_hover_text(self.tr(S::fav_skin_hint)).clicked() {
            action = Some(CardAction::Fav(skin.id));
        }

        // name (max 2 lines)
        let mut job = egui::text::LayoutJob::simple(skin.name.clone(), bold(13.0), TEXT, CARD_W - 18.0);
        job.wrap.max_rows = 2;
        let galley = ui.painter().layout_job(job);
        let name_pos = img_rect.left_bottom() + vec2(1.0, 6.0);
        ui.painter().galley(name_pos, galley, TEXT);

        // chroma dots
        let chromas = self.chromas(skin.id);
        let selected = self.card_chroma.get(&skin.id).copied().flatten();
        if !chromas.is_empty() {
            let y = rect.bottom() - 58.0;
            let mut x = rect.left() + 10.0;
            let dots = std::iter::once((None, DEFAULT_DOT, self.tr(S::default).to_string()))
                .chain(chromas.iter().take(MAX_DOTS).map(|c| (c.id, c.color, c.name.clone())));
            for (cid, color, name) in dots {
                let r = Rect::from_min_size(egui::pos2(x, y), vec2(16.0, 16.0));
                let resp = ui.interact(r, id.with(("dot", cid)), Sense::click()).on_hover_text(name);
                let on = selected == cid;
                ui.painter().circle_filled(r.center(), 7.0, color);
                if on || resp.hovered() {
                    ui.painter().circle_stroke(r.center(), 8.0, Stroke::new(2.0, if on { GOLD } else { TEXT2 }));
                }
                if resp.clicked() {
                    action = Some(CardAction::Chroma(skin.id, cid));
                }
                x += 19.0;
            }
            if chromas.len() > MAX_DOTS {
                ui.painter().text(
                    egui::pos2(x + 2.0, y + 8.0),
                    Align2::LEFT_CENTER,
                    format!("+{}", chromas.len() - MAX_DOTS),
                    reg(11.0),
                    MUTED,
                );
            }
        }

        // add button
        let btn_rect = Rect::from_min_max(
            egui::pos2(rect.left() + 8.0, rect.bottom() - 36.0),
            egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0),
        );
        let sent = self.sent_until.get(&skin.id).is_some_and(|&u| u > now);
        let label = if sent { self.tr(S::sent) } else { self.tr(S::add) };
        let btn = egui::Button::new(RichText::new(label).font(bold(13.0)).color(ON_GOLD))
            .fill(if sent { GREEN } else { GOLD_DK })
            .corner_radius(7);
        if ui.put(btn_rect, btn).clicked() {
            action = Some(CardAction::Add(skin.id));
        }
        if sent {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        action
    }

    fn detail_modal(&mut self, ctx: &egui::Context, now: f64) {
        let Some(det) = self.detail else { return };
        let Some(skin) = self.skin(det.skin_id).cloned() else {
            self.detail = None;
            return;
        };
        let champ_name = self.champ(det.skin_id / 1000).map(|c| c.name.clone()).unwrap_or_default();
        let chromas = self.chromas(skin.id);
        let mut close = false;
        let mut send = false;
        let mut pick: Option<Option<i64>> = None;
        let mut reroll = false;

        let resp = egui::Modal::new(Id::unique("detail"))
            .backdrop_color(Color32::from_black_alpha(170))
            .frame(Frame::new().fill(BG).stroke(Stroke::new(1.0, BORDER)).corner_radius(14).inner_margin(16))
            .show(ctx, |ui| {
                ui.set_width(720.0);
                let size = vec2(720.0, 320.0);
                let ppp = ui.ctx().pixels_per_point();
                let chosen = det.chroma.and_then(|cid| chromas.iter().find(|c| c.id == Some(cid)));
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let splash = if skin.splash.is_empty() { &skin.tile } else { &skin.splash };
                match self.imgs.get(splash, (size.x * ppp) as u32, (size.y * ppp) as u32) {
                    Some(tex) => egui::Image::new((tex.id(), size)).corner_radius(12).paint_at(ui, rect),
                    None => {
                        ui.painter().rect_filled(rect, 12, CARD);
                        ui.put(rect, egui::Spinner::new().color(PURPLE).size(28.0));
                    }
                }
                // chroma preview in the corner of the splash
                if let Some(c) = chosen.filter(|c| !c.preview.is_empty()) {
                    let pv = Rect::from_min_size(rect.right_bottom() - vec2(150.0, 150.0), vec2(138.0, 138.0));
                    ui.painter().rect_filled(pv.expand(3.0), 12, Color32::from_black_alpha(190));
                    if let Some(tex) = self.imgs.get(&c.preview, (138.0 * ppp) as u32, (138.0 * ppp) as u32) {
                        egui::Image::new((tex.id(), pv.size())).corner_radius(10).paint_at(ui, pv);
                    }
                    ui.painter().rect_stroke(pv.expand(3.0), 12, Stroke::new(2.0, c.color), StrokeKind::Outside);
                }

                ui.add_space(10.0);
                ui.label(RichText::new(&skin.name).font(bold(22.0)).color(GOLD));
                let mut bits = vec![champ_name.clone(), format!("ID {}", skin.id)];
                if !skin.rarity.is_empty() {
                    bits.push(skin.rarity.to_string());
                }
                ui.label(RichText::new(bits.join("  ·  ")).font(reg(13.0)).color(MUTED));

                if !chromas.is_empty() {
                    ui.add_space(10.0);
                    ui.label(RichText::new(self.tr(S::color_packs)).font(bold(13.0)).color(TEXT2));
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let all = std::iter::once((None, DEFAULT_DOT, self.tr(S::default).to_string()))
                                .chain(chromas.iter().map(|c| (c.id, c.color, c.name.clone())));
                            for (cid, color, name) in all {
                                let on = det.chroma == cid;
                                let b = egui::Button::new(RichText::new(format!("●  {name}")).font(bold(12.0)).color(TEXT))
                                    .fill(if on { PURPLE_DK } else { CARD })
                                    .stroke(Stroke::new(2.0, if on { GOLD } else { color }))
                                    .corner_radius(8)
                                    .min_size(vec2(0.0, 30.0));
                                if ui.add(b).clicked() {
                                    pick = Some(cid);
                                }
                            }
                        });
                    });
                }

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let add = egui::Button::new(RichText::new(self.tr(S::add_to_ltk)).font(bold(15.0)).color(ON_GOLD))
                        .fill(GOLD_DK)
                        .corner_radius(10)
                        .min_size(vec2(170.0, 40.0));
                    if ui.add(add).clicked() {
                        send = true;
                    }
                    let rnd = egui::Button::new(RichText::new(self.tr(S::random)).font(reg(14.0)).color(TEXT2))
                        .fill(CARD)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(10)
                        .min_size(vec2(120.0, 40.0));
                    if ui.add(rnd).on_hover_text(self.tr(S::random_hint)).clicked() {
                        reroll = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let b = egui::Button::new(RichText::new(self.tr(S::close)).font(reg(14.0)).color(TEXT2))
                            .fill(CARD)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(10)
                            .min_size(vec2(90.0, 40.0));
                        if ui.add(b).clicked() {
                            close = true;
                        }
                    });
                });
            });

        if let Some(cid) = pick {
            self.detail = Some(Detail { skin_id: det.skin_id, chroma: cid });
            self.card_chroma.insert(det.skin_id, cid);
        }
        if reroll {
            self.random_pick();
        }
        if send {
            let chroma = self.detail.map(|d| d.chroma).unwrap_or(det.chroma);
            self.send(ctx, det.skin_id, chroma, now);
            self.detail = None;
        } else if close || (resp.should_close() && !reroll) {
            self.detail = None;
        }
    }

    fn settings_modal(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut close = false;
        let resp = egui::Modal::new(Id::unique("settings"))
            .backdrop_color(Color32::from_black_alpha(150))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(12).inner_margin(18))
            .show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(RichText::new(self.tr(S::settings)).font(bold(17.0)).color(TEXT));
                ui.add_space(10.0);
                let wide = |ui: &mut Ui, text: &str| {
                    ui.add(
                        egui::Button::new(RichText::new(text).font(reg(14.0)).color(TEXT2))
                            .fill(CARD)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(8)
                            .min_size(vec2(320.0, 36.0)),
                    )
                };
                if wide(ui, self.tr(S::open_restart_ltk)).clicked() {
                    self.open_ltk(ctx);
                    close = true;
                }
                if wide(ui, self.tr(S::open_log)).clicked() {
                    let p = data::log_file();
                    if p.is_file() {
                        ltk::open_notepad(&p);
                    } else {
                        let now = ctx.input(|i| i.time);
                        self.toast(self.tr(S::log_not_found).into(), false, now);
                    }
                }
                if wide(ui, self.tr(S::open_data_dir)).clicked() {
                    ltk::shell_open(&data::data_dir().to_string_lossy());
                }
                if wide(ui, self.tr(S::ltk_download_page)).clicked() {
                    ltk::shell_open(data::LTK_RELEASES);
                }
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(self.tr(S::footer)).font(reg(11.0)).color(MUTED));
                });
            });
        if close || resp.should_close() {
            self.show_settings = false;
        }
    }

    fn history_modal(&mut self, ctx: &egui::Context, now: f64) {
        if !self.show_history {
            return;
        }
        let mut resend: Option<HistoryEntry> = None;
        let mut show: Option<HistoryEntry> = None;
        let mut clear = false;
        let mut close = false;
        let unix = data::unix_now();
        let resp = egui::Modal::new(Id::unique("history"))
            .backdrop_color(Color32::from_black_alpha(150))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(12).inner_margin(18))
            .show(ctx, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(self.tr(S::history_title)).font(bold(17.0)).color(TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(small_btn(self.tr(S::close))).clicked() {
                            close = true;
                        }
                        if !self.cfg.history.is_empty() && ui.add(small_btn(self.tr(S::history_clear))).clicked() {
                            clear = true;
                        }
                    });
                });
                ui.add_space(10.0);
                if self.cfg.history.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(self.tr(S::history_empty)).font(reg(14.0)).color(MUTED));
                    });
                    ui.add_space(20.0);
                    return;
                }
                let entries = self.cfg.history.clone();
                egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
                    for e in &entries {
                        Frame::new()
                            .fill(CARD)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(10)
                            .inner_margin(Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let px = (32.0 * ui.ctx().pixels_per_point()).round() as u32;
                                    if let Some(tex) = self.imgs.get(&data::champ_icon_url(e.champ_id), px, px) {
                                        ui.add(egui::Image::new((tex.id(), vec2(32.0, 32.0))).corner_radius(16));
                                    }
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(e.label()).font(bold(14.0)).color(TEXT));
                                        ui.label(
                                            RichText::new(format!("{}  ·  {}", e.champ_name, self.ago(unix, e.ts)))
                                                .font(reg(12.0))
                                                .color(MUTED),
                                        );
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let b = egui::Button::new(RichText::new(self.tr(S::resend)).font(bold(12.0)).color(ON_GOLD))
                                            .fill(GOLD_DK)
                                            .corner_radius(7)
                                            .min_size(vec2(0.0, 28.0));
                                        if ui.add(b).clicked() {
                                            resend = Some(e.clone());
                                        }
                                        if ui.add(small_btn(self.tr(S::show))).clicked() {
                                            show = Some(e.clone());
                                        }
                                    });
                                });
                            });
                        ui.add_space(6.0);
                    }
                });
            });
        if clear {
            self.cfg.history.clear();
            self.persist();
        }
        if let Some(e) = resend {
            self.send_entry(ctx, e);
        }
        if let Some(e) = show {
            self.select_champion(e.champ_id);
            self.card_chroma.insert(e.skin_id, e.chroma_id);
            self.detail = Some(Detail { skin_id: e.skin_id, chroma: e.chroma_id });
            close = true;
        }
        if close || resp.should_close() {
            self.show_history = false;
        }
        let _ = now;
    }

    fn ago(&self, now: u64, ts: u64) -> String {
        let d = now.saturating_sub(ts);
        let (s, n) = match d {
            0..60 => return self.tr(S::ago_now).into(),
            60..3600 => (S::ago_min, d / 60),
            3600..86400 => (S::ago_hour, d / 3600),
            _ => (S::ago_day, d / 86400),
        };
        self.tr(s).replace("{n}", &n.to_string())
    }

    fn loader_screen(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let card = Rect::from_center_size(rect.center(), vec2(380.0, 240.0));
        ui.painter().rect(card, 18, PANEL, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let mut retry = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(card.shrink(20.0)), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                if let Some(logo) = &self.logo {
                    ui.add(egui::Image::new((logo.id(), vec2(64.0, 64.0))));
                }
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 150.0).max(0.0) / 2.0);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(RichText::new("HALIT").font(bold(19.0)).color(GOLD));
                    ui.label(RichText::new(" CHANGER").font(bold(19.0)).color(PURPLE_HOV));
                });
                ui.add_space(14.0);
                if self.load_failed {
                    ui.label(RichText::new(self.tr(S::data_load_failed)).font(reg(13.0)).color(RED));
                    ui.add_space(8.0);
                    retry = ui.add(small_btn(self.tr(S::retry))).clicked();
                } else {
                    ui.add(egui::Spinner::new().color(PURPLE).size(26.0));
                    ui.add_space(8.0);
                    ui.label(RichText::new(self.tr(self.loader_text)).font(reg(13.0)).color(MUTED));
                }
            });
        });
        if retry {
            self.load_failed = false;
            self.loader_text = S::loading_data;
            self.spawn_loader(ui.ctx().clone());
        }
    }

    fn toasts(&mut self, ctx: &egui::Context, now: f64) {
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(Id::unique("toasts"))
            .anchor(Align2::RIGHT_TOP, vec2(-24.0, 72.0))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                for t in &self.toasts {
                    let (c, bg) = if t.ok { (GREEN, GREEN_DIM) } else { (RED, RED_DIM) };
                    let fade = ((t.until - now) / 0.3).clamp(0.0, 1.0) as f32;
                    Frame::new()
                        .fill(blend(CARD, bg, 0.5).gamma_multiply(fade))
                        .stroke(Stroke::new(1.0, c.gamma_multiply(fade)))
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_max_width(380.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(if t.ok { "✓" } else { "✕" }).font(bold(16.0)).color(c));
                                ui.label(RichText::new(&t.text).font(bold(13.0)).color(TEXT.gamma_multiply(fade)));
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

enum CardAction {
    Open(i64),
    Chroma(i64, Option<i64>),
    Add(i64),
    Fav(i64),
}

fn header_btn(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).font(reg(13.0)).color(TEXT2))
            .fill(CARD)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(8)
            .min_size(vec2(0.0, 30.0)),
    )
}

fn small_btn(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).font(reg(12.0)).color(TEXT2))
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(7)
        .min_size(vec2(0.0, 28.0))
}

fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

impl eframe::App for HalitApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        if !self.titlebar_done {
            self.titlebar_done = crate::dark_titlebar();
        }
        self.imgs.pump(&ctx);
        self.drain(now);
        self.poll_ltk(&ctx, now);

        let modal_open = self.detail.is_some() || self.show_settings || self.show_history;
        if self.ready && !modal_open && ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::R)) {
            self.random_pick();
        }

        egui::Panel::top("header")
            .exact_size(60.0)
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(2.0, PURPLE_DIM)))
            .show(ui, |ui| self.header(ui, now));

        if !self.ready {
            egui::CentralPanel::default()
                .frame(Frame::new().fill(BG))
                .show(ui, |ui| self.loader_screen(ui));
            self.toasts(&ctx, now);
            return;
        }

        egui::Panel::left("champions")
            .exact_size(256.0)
            .frame(Frame::new().fill(PANEL).inner_margin(Margin { left: 6, right: 4, top: 0, bottom: 8 }))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG2).inner_margin(Margin { left: 6, right: 6, top: 0, bottom: 6 }))
            .show(ui, |ui| self.center(ui, now));

        self.detail_modal(&ctx, now);
        self.settings_modal(&ctx);
        self.history_modal(&ctx, now);
        self.toasts(&ctx, now);
    }
}

// ------------------------------------------------------------------ fonts + style

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let mut load = |key: &str, file: &str| -> bool {
        match std::fs::read(dir.join(file)) {
            Ok(bytes) => {
                fonts.font_data.insert(key.into(), Arc::new(egui::FontData::from_owned(bytes)));
                true
            }
            Err(_) => false,
        }
    };
    let ui_font = load("segoe", "segoeui.ttf");
    let bold_font = load("segoe_bold", "segoeuib.ttf");
    let sym = load("segoe_sym", "seguisym.ttf");
    let emoji = load("segoe_emoji", "seguiemj.ttf");

    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    if ui_font {
        prop.insert(0, "segoe".into());
    }
    let mut fallbacks: Vec<String> = Vec::new();
    if sym {
        fallbacks.push("segoe_sym".into());
    }
    if emoji {
        fallbacks.push("segoe_emoji".into());
    }
    prop.extend(fallbacks.iter().cloned());
    let mut bold_family: Vec<String> = Vec::new();
    if bold_font {
        bold_family.push("segoe_bold".into());
    }
    bold_family.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
    fonts.families.insert(FontFamily::Name("bold".into()), bold_family);
    ctx.set_fonts(fonts);
}

fn setup_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.global_style_mut(|style| {
        let v = &mut style.visuals;
        v.panel_fill = BG2;
        v.window_fill = PANEL;
        v.extreme_bg_color = CARD;
        v.faint_bg_color = CARD;
        v.override_text_color = None;
        v.selection.bg_fill = PURPLE_DK;
        v.selection.stroke = Stroke::new(1.0, PURPLE_HOV);
        v.hyperlink_color = PURPLE_HOV;
        v.window_corner_radius = CornerRadius::same(12);
        let w = &mut v.widgets;
        w.inactive.bg_fill = CARD;
        w.inactive.weak_bg_fill = CARD;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        w.hovered.bg_fill = CARD_HOV;
        w.hovered.weak_bg_fill = CARD_HOV;
        w.hovered.bg_stroke = Stroke::new(1.0, BORDER_HOV);
        w.active.bg_fill = PURPLE_DK;
        w.active.weak_bg_fill = PURPLE_DK;
        w.active.bg_stroke = Stroke::new(1.0, PURPLE);
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.text_edit_bg_color = Some(CARD);
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(12.0, 5.0);
        style.spacing.scroll.bar_width = 8.0;
    });
}
