# -*- coding: utf-8 -*-
"""
Halit Changer - League of Legends Skin Tarayici
Skinler : https://github.com/Alban1911/LeagueSkins
Motor   : LTK Manager (https://github.com/LeagueToolkit/ltk-manager)
          Skinler ltk://install protokolu ile LTK'ya gonderilir; indirme,
          kurulum ve oyuna uygulama isini LTK yapar.
"""

import os
import re
import sys
import json
import time
import ctypes
import datetime
import threading
import webbrowser
import subprocess
import urllib.parse
import tkinter as tk
from concurrent.futures import ThreadPoolExecutor
from io import BytesIO

import requests
from requests.adapters import HTTPAdapter
import customtkinter as ctk
from PIL import Image, ImageDraw, ImageTk

# ------------------------------------------------------------------ sabitler

def app_dir():
    if getattr(sys, "frozen", False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.abspath(__file__))


def resource(rel):
    base = getattr(sys, "_MEIPASS", app_dir())
    return os.path.join(base, rel)


APP_NAME = "Halit Changer"
DATA_DIR = os.path.join(os.environ.get("LOCALAPPDATA", os.path.expanduser("~")), "HalitChanger")
CACHE_DIR = os.path.join(DATA_DIR, "cache")
IMG_CACHE_DIR = os.path.join(CACHE_DIR, "img")
CONFIG_FILE = os.path.join(DATA_DIR, "config.json")
META_FILE = os.path.join(CACHE_DIR, "meta.json")
LOG_FILE = os.path.join(DATA_DIR, "halit.log")
SKIN_IDS_FILE = resource("skin_ids.json")

LOGO_PATH = resource(os.path.join("assets", "logo.png"))
ICON_PATH = resource(os.path.join("assets", "icon.ico"))

SKINS_REPO = "Alban1911/LeagueSkins"
SKINS_RAW = f"https://raw.githubusercontent.com/{SKINS_REPO}/main"
GITHUB_REPO_API = f"https://api.github.com/repos/{SKINS_REPO}"
DOWNLOAD_DOMAIN = "raw.githubusercontent.com"

LTK_SETTINGS = os.path.join(os.environ.get("APPDATA", os.path.expanduser("~")),
                            "dev.leaguetoolkit.manager", "settings.json")

# LTK Manager is a Tauri app. Protocol handler is often an 8.3 name
# (LTK-MA~1.EXE), not ltk.exe. Patcher host also counts as running.
_LTK_EXE_HINTS = (
    "ltk-manager",
    "ltk manager",
    "ltk.exe",
    "ltk-ma~",
    "ltk_patcher",
    "cslol-host",
    "cslol_host",
)

CDRAGON = "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global"
CHAMPIONS_URL = f"{CDRAGON}/tr_tr/v1/champion-summary.json"
SKINS_URL = f"{CDRAGON}/tr_tr/v1/skins.json"
CHAMP_ICON_URL = f"{CDRAGON}/default/v1/champion-icons/{{cid}}.png"

# ------------------------------------------------------------------ tema

CLR_BG         = "#07070c"
CLR_BG2        = "#0b0b12"
CLR_PANEL      = "#101018"
CLR_PANEL2     = "#14141e"
CLR_CARD       = "#161622"
CLR_CARD_HOV   = "#1c1c2c"
CLR_CARD_SEL   = "#1a1430"
CLR_BORDER     = "#262636"
CLR_BORDER_HOV = "#5b4a8a"
CLR_GOLD       = "#f0c050"
CLR_GOLD_DK    = "#c8963c"
CLR_GOLD_DIM   = "#6b5420"
CLR_PURPLE     = "#8b5cf6"
CLR_PURPLE_DK  = "#6d28d9"
CLR_PURPLE_HOV = "#a78bfa"
CLR_PURPLE_DIM = "#3b2268"
CLR_GREEN      = "#22c55e"
CLR_GREEN_DIM  = "#0f2e1c"
CLR_RED        = "#ef4444"
CLR_RED_DIM    = "#3a1212"
CLR_TEXT       = "#f2f0f8"
CLR_TEXT2      = "#c4c2ce"
CLR_MUTED      = "#6e6e84"
CLR_STAR       = "#f0c050"

NO_WINDOW = subprocess.CREATE_NO_WINDOW if hasattr(subprocess, "CREATE_NO_WINDOW") else 0

TH32CS_SNAPPROCESS = 0x00000002


class _PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", ctypes.c_ulong),
        ("cntUsage", ctypes.c_ulong),
        ("th32ProcessID", ctypes.c_ulong),
        ("th32DefaultHeapID", ctypes.c_size_t),
        ("th32ModuleID", ctypes.c_ulong),
        ("cntThreads", ctypes.c_ulong),
        ("th32ParentProcessID", ctypes.c_ulong),
        ("pcPriClassBase", ctypes.c_long),
        ("dwFlags", ctypes.c_ulong),
        ("szExeFile", ctypes.c_wchar * 260),
    ]


def _ltk_protocol_basenames():
    names = set()
    try:
        import winreg
        for hive in (winreg.HKEY_CURRENT_USER, winreg.HKEY_LOCAL_MACHINE):
            try:
                with winreg.OpenKey(hive, r"Software\Classes\ltk\shell\open\command") as key:
                    cmd = str(winreg.QueryValueEx(key, None)[0] or "")
            except OSError:
                continue
            m = re.search(r'"([^"]+\.exe)"', cmd, re.I) or re.search(r'(\S+\.exe)', cmd, re.I)
            if not m:
                continue
            path = m.group(1)
            names.add(os.path.basename(path).lower())
            try:
                buf = ctypes.create_unicode_buffer(32768)
                n = ctypes.windll.kernel32.GetLongPathNameW(path, buf, 32768)
                if n:
                    names.add(os.path.basename(buf.value).lower())
            except Exception:
                pass
    except Exception:
        pass
    return names


def _running_exe_names():
    names = []
    kernel32 = ctypes.windll.kernel32
    kernel32.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
    kernel32.CreateToolhelp32Snapshot.argtypes = [ctypes.c_ulong, ctypes.c_ulong]
    kernel32.Process32FirstW.argtypes = [ctypes.c_void_p, ctypes.POINTER(_PROCESSENTRY32W)]
    kernel32.Process32FirstW.restype = ctypes.c_int
    kernel32.Process32NextW.argtypes = [ctypes.c_void_p, ctypes.POINTER(_PROCESSENTRY32W)]
    kernel32.Process32NextW.restype = ctypes.c_int
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    snap = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if not snap or snap == ctypes.c_void_p(-1).value:
        return names
    try:
        pe = _PROCESSENTRY32W()
        pe.dwSize = ctypes.sizeof(_PROCESSENTRY32W)
        if not kernel32.Process32FirstW(snap, ctypes.byref(pe)):
            return names
        while True:
            names.append(pe.szExeFile)
            if not kernel32.Process32NextW(snap, ctypes.byref(pe)):
                break
    finally:
        kernel32.CloseHandle(snap)
    return names


def _is_ltk_exe_name(name, protocol_names=None):
    n = (name or "").lower()
    if not n:
        return False
    if protocol_names and n in protocol_names:
        return True
    return any(h in n for h in _LTK_EXE_HINTS)

CHROMA_COLORS = {
    "Ruby": "#dc2626", "Obsidian": "#1a1a1a", "Pearl": "#f0e6d4",
    "Sapphire": "#2563eb", "Emerald": "#059669", "Amethyst": "#9333ea",
    "Citrine": "#eab308", "Rose Quartz": "#f472b6", "Turquoise": "#14b8a6",
    "Aquamarine": "#34d399", "Tanzanite": "#6366f1", "Sandstone": "#c2a068",
    "Granite": "#6b7280", "Peridot": "#84cc16", "Catseye": "#f59e0b",
    "Rainbow": "#a855f7", "Meteorite": "#475569", "Paragon": "#facc15",
    "Resolute": "#0ea5e9", "Elite": "#d97706", "Gilded": "#f59e0b",
    "Chrono": "#06b6d4", "Merc": "#64748b", "Underground": "#57534e",
    "Worlds Early Bird": "#f97316", "Vitality": "#22d3ee", "Nomad": "#a16207",
    "Disco": "#c084fc", "Ace": "#f43f5e", "Night Blossom": "#be185d",
    "Dark Ritual": "#4a1942", "Unlocked": "#10b981", "Worthy": "#f59e0b",
    "Neon Facade": "#00ff88", "Hunter": "#15803d", "MSI 2024 Chaos": "#e11d48",
    "Strike Gold": "#f59e0b", "Strike Aqua": "#06b6d4", "Strike Crimson": "#dc2626",
    "Reckoning": "#be123c", "Punk Navy": "#1e3a5f", "Punk Purple": "#6b21a8",
    "Punk Orange": "#c2410c", "Speckled": "#78716c", "Golden": "#f59e0b",
    "Golden Tiger": "#f59e0b", "Worlds 2019": "#f59e0b", "Wreathguard": "#059669",
    "Ghoulish": "#15803d", "K.O.": "#ef4444", "Soccer Cup": "#16a34a",
    "Cerulean Club": "#2563eb", "Cursed": "#7f1d1d", "Tenfold Triumph": "#f59e0b",
    "Mythclimber": "#a855f7", "Vivid": "#f43f5e", "Inked": "#1a1a1a",
    "Lustrous": "#f59e0b", "Bronze": "#cd7f32", "Silver": "#c0c0c0",
    "Gold": "#ffd700", "Platinum": "#e5e4e2", "Diamond": "#b9f2ff",
    "Master": "#9333ea", "Grandmaster": "#ef4444", "Challenger": "#f59e0b",
}

STRINGS = {
    "champions_header": {"tr": "ŞAMPİYONLAR", "en": "CHAMPIONS"},
    "search_placeholder": {"tr": "  Şampiyon ara...", "en": "  Search champions..."},
    "select_champion": {"tr": "Bir şampiyon seç", "en": "Select a champion"},
    "select_champion_sub": {"tr": "Ara, skin seç, Add → LTK", "en": "Search, pick a skin, Add → LTK"},
    "favorite": {"tr": "☆  Favori", "en": "☆  Favorite"},
    "favorite_on": {"tr": "★  Favori", "en": "★  Favorite"},
    "settings": {"tr": "Ayarlar", "en": "Settings"},
    "ltk_checking": {"tr": "  ●  LTK kontrol ediliyor...  ", "en": "  ●  LTK Checking...  "},
    "ltk_connected": {"tr": "  ●  LTK Bağlı  ", "en": "  ●  LTK Connected  "},
    "ltk_offline": {"tr": "  ●  LTK Kapalı  ", "en": "  ●  LTK Offline  "},
    "ready": {"tr": "Hazır  ✔   {n} şampiyon", "en": "Ready  ✔   {n} champions"},
    "loading_status": {"tr": "Yükleniyor...", "en": "Loading..."},
    "error_status": {"tr": "Hata", "en": "Error"},
    "pick_champion": {"tr": "Bir şampiyon seç", "en": "Pick a champion"},
    "skins_count": {"tr": "{n} skin", "en": "{n} skins"},
    "add": {"tr": "+  Ekle", "en": "+  Add"},
    "sending": {"tr": "Gönderiliyor...", "en": "Sending..."},
    "sent": {"tr": "Gönderildi", "en": "Sent"},
    "add_to_ltk": {"tr": "+  LTK'ya Ekle", "en": "+  Add to LTK"},
    "close": {"tr": "Kapat", "en": "Close"},
    "color_packs": {"tr": "Renk Paketleri", "en": "Color Packs"},
    "default": {"tr": "Varsayılan", "en": "Default"},
    "loading_img": {"tr": "Yükleniyor...", "en": "Loading..."},
    "still_loading": {"tr": "Hâlâ yükleniyor, bekle...", "en": "Still loading, please wait..."},
    "sent_toast": {"tr": "✓  Skin LTK'ya gönderildi  ·  {label}", "en": "✓  Skin sent to LTK  ·  {label}"},
    "send_failed": {"tr": "LTK'ya gönderilemedi", "en": "Couldn't send to LTK"},
    "log_not_found": {"tr": "Log dosyası bulunamadı", "en": "Log file not found"},
    "open_restart_ltk": {"tr": "LTK'yi Aç / Yeniden Başlat", "en": "Open / Restart LTK"},
    "open_log": {"tr": "Logu Aç", "en": "Open log"},
    "ltk_download_page": {"tr": "LTK indirme sayfası", "en": "LTK download page"},
    "loading_data": {"tr": "Veriler yükleniyor...", "en": "Loading data..."},
    "preparing_list": {"tr": "Liste hazırlanıyor...", "en": "Preparing list..."},
    "first_launch": {"tr": "İlk açılış — veriler indiriliyor...", "en": "First launch — downloading data..."},
    "load_failed": {"tr": "Yükleme başarısız", "en": "Load failed"},
    "data_load_failed": {"tr": "Veri yüklenemedi: {e}", "en": "Failed to load data: {e}"},
    "ltk_not_started": {"tr": "LTK başlatılamadı — Settings → LTK download", "en": "LTK couldn't start — Settings → LTK download"},
    "ltk_conn_not_found": {"tr": "LTK bağlantısı bulunamadı", "en": "LTK connection not found"},
    "ltk_not_found_dl": {"tr": "LTK bulunamadı — indirme sayfası açıldı", "en": "LTK not found — download page opened"},
}

ctk.set_appearance_mode("dark")
ctk.set_default_color_theme("dark-blue")

SESSION = requests.Session()
SESSION.headers["User-Agent"] = "HalitChanger"
_adapter = HTTPAdapter(pool_connections=12, pool_maxsize=12, max_retries=0)
SESSION.mount("https://", _adapter)
SESSION.mount("http://", _adapter)

CHAMP_ROW_H = 40
SKIN_BATCH = 6
IMG_MAX_MEM = 220


def ensure_dirs():
    for d in (DATA_DIR, CACHE_DIR, IMG_CACHE_DIR):
        os.makedirs(d, exist_ok=True)


def load_config():
    try:
        with open(CONFIG_FILE, "r", encoding="utf-8") as f:
            data = json.load(f)
            if isinstance(data, dict):
                return data
    except Exception:
        pass
    return {}


def save_config(cfg):
    try:
        with open(CONFIG_FILE, "w", encoding="utf-8") as f:
            json.dump(cfg, f, ensure_ascii=False, indent=2)
    except OSError:
        pass


def load_skin_ids():
    try:
        with open(SKIN_IDS_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def fetch_json_cached(url, cache_name, force_network=False):
    cache_path = os.path.join(CACHE_DIR, cache_name)
    cached = None
    if os.path.isfile(cache_path):
        try:
            with open(cache_path, "r", encoding="utf-8") as f:
                cached = json.load(f)
        except Exception:
            cached = None
    if cached is not None and not force_network:
        return cached
    try:
        r = SESSION.get(url, timeout=20)
        r.raise_for_status()
        data = r.json()
        tmp = cache_path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False)
        os.replace(tmp, cache_path)
        return data
    except Exception:
        return cached or {}


def cdragon_url(game_path):
    m = re.match(r"/lol-game-data/assets/(.+)", game_path or "", re.I)
    return f"{CDRAGON}/default/{m.group(1).lower()}" if m else ""


def chroma_hex(name):
    if name in CHROMA_COLORS:
        return CHROMA_COLORS[name]
    h = abs(hash(name)) % 360
    return f"#{(h * 97) % 200 + 40:02x}{(h * 57) % 180 + 40:02x}{(h * 31) % 200 + 40:02x}"


def index_chromas(skin_ids):
    """skin_ids.json: '1013': 'Lunar Beast Annie', '1014': 'Lunar Beast Annie (Ruby)'"""
    id_to_name = {}
    by_parent = {}
    for sid_str, name in (skin_ids or {}).items():
        try:
            sid = int(sid_str)
        except (TypeError, ValueError):
            continue
        if not isinstance(name, str):
            continue
        id_to_name[sid] = name
        if " (" in name and name.endswith(")"):
            parent = name[:name.rfind(" (")]
            pack = name[name.rfind(" (") + 2:-1]
            if parent and pack:
                by_parent.setdefault(parent, []).append({
                    "id": sid, "name": pack, "full": name,
                })
    return id_to_name, by_parent


# ------------------------------------------------------------------ goruntuler

class ImageLoader:
    def __init__(self, root):
        self.root = root
        self.pool = ThreadPoolExecutor(max_workers=6)
        self.mem = {}
        self._seq = {}

    def get_async(self, url, size, callback, corner=0, photo=False):
        if not url:
            return
        key = (url, size, corner, photo)
        cached = self.mem.get(key)
        if cached is not None:
            callback(cached)
            return
        token = self._seq.get(key, 0) + 1
        self._seq[key] = token
        self.pool.submit(self._work, url, size, callback, corner, key, token, photo)
        try:
            for t in self.pool._threads:
                t.daemon = True
        except Exception:
            pass

    def _paths(self, url, size, corner):
        stem = re.sub(r"[^a-z0-9]", "_", url.lower())[-80:]
        raw = os.path.join(IMG_CACHE_DIR, stem + ".png")
        fitted = os.path.join(IMG_CACHE_DIR, f"{stem}_{size[0]}x{size[1]}_r{corner}.png")
        return raw, fitted

    def _work(self, url, size, callback, corner, key, token, photo):
        try:
            raw, fitted = self._paths(url, size, corner)
            if os.path.isfile(fitted):
                pil = Image.open(fitted).convert("RGBA")
            else:
                if os.path.isfile(raw):
                    pil = Image.open(raw).convert("RGBA")
                else:
                    r = SESSION.get(url, timeout=12)
                    r.raise_for_status()
                    pil = Image.open(BytesIO(r.content)).convert("RGBA")
                    try:
                        pil.save(raw)
                    except Exception:
                        pass
                pil = self._fit(pil, size)
                if corner:
                    pil = self._round(pil, corner)
                try:
                    pil.save(fitted)
                except Exception:
                    pass
            self.root.after(0, lambda: self._done(pil, size, callback, key, token, photo))
        except Exception:
            pass

    def _done(self, pil, size, callback, key, token, photo):
        if self._seq.get(key) != token:
            return
        try:
            if not self.root.winfo_exists():
                return
            img = ImageTk.PhotoImage(pil) if photo else ctk.CTkImage(
                light_image=pil, dark_image=pil, size=size)
            self.mem[key] = img
            if len(self.mem) > IMG_MAX_MEM:
                try:
                    self.mem.pop(next(iter(self.mem)))
                except Exception:
                    pass
            callback(img)
        except Exception:
            pass

    @staticmethod
    def _fit(pil, size):
        tw, th = size
        w, h = pil.size
        scale = max(tw / max(w, 1), th / max(h, 1))
        nw, nh = max(1, int(w * scale)), max(1, int(h * scale))
        pil = pil.resize((nw, nh), Image.BILINEAR)
        w, h = pil.size
        left, top = (w - tw) // 2, (h - th) // 2
        return pil.crop((left, top, left + tw, top + th))

    @staticmethod
    def _round(pil, radius):
        mask = Image.new("L", pil.size, 0)
        d = ImageDraw.Draw(mask)
        d.rounded_rectangle([0, 0, pil.size[0] - 1, pil.size[1] - 1], radius=radius, fill=255)
        out = pil.copy()
        out.putalpha(mask)
        return out


# ------------------------------------------------------------------ toast

class Toast(ctk.CTkToplevel):
    def __init__(self, parent, message, success=True, duration=2600):
        super().__init__(parent)
        self.overrideredirect(True)
        self.attributes("-topmost", True)
        self.configure(fg_color=CLR_BG)
        color = CLR_GREEN if success else CLR_RED
        bg = CLR_GREEN_DIM if success else CLR_RED_DIM
        icon = "\u2713" if success else "\u2715"

        wrap = ctk.CTkFrame(self, fg_color=bg, corner_radius=14, border_width=1, border_color=color)
        wrap.pack(padx=0, pady=0)
        inner = ctk.CTkFrame(wrap, fg_color=CLR_CARD, corner_radius=13)
        inner.pack(padx=1, pady=1, ipadx=14, ipady=8)
        ctk.CTkLabel(inner, text=icon, font=ctk.CTkFont(size=16, weight="bold"),
                     text_color=color).pack(side="left", padx=(4, 8))
        ctk.CTkLabel(inner, text=message, font=ctk.CTkFont(size=13, weight="bold"),
                     text_color=CLR_TEXT, wraplength=340, justify="left").pack(side="left", padx=(0, 4))
        self.update_idletasks()
        try:
            px = parent.winfo_rootx()
            py = parent.winfo_rooty()
            pw = parent.winfo_width()
            self.geometry(f"+{px + pw - self.winfo_width() - 24}+{py + 56}")
        except Exception:
            pass
        self.after(duration, self.destroy)


# ------------------------------------------------------------------ skin karti

class ChampCanvas(tk.Frame):
    ROW = 40

    def __init__(self, master, app):
        super().__init__(master, bg=CLR_PANEL, highlightthickness=0)
        self.app = app
        self.items = []
        self.photos = {}
        self._asked = set()
        self._draw_job = None
        self.canvas = tk.Canvas(self, bg=CLR_PANEL, highlightthickness=0, bd=0)
        self.sb = ctk.CTkScrollbar(self, command=self._yview, width=12,
                                   fg_color=CLR_PANEL, button_color=CLR_BORDER,
                                   button_hover_color=CLR_MUTED)
        self.canvas.pack(side="left", fill="both", expand=True)
        self.sb.pack(side="right", fill="y")
        self.canvas.configure(yscrollcommand=self._on_sb)
        self.canvas.bind("<Configure>", lambda e: self._schedule_draw())
        self.canvas.bind("<MouseWheel>", self._wheel)
        self.canvas.bind("<Button-1>", self._click)
        self.bind("<Enter>", lambda e: self.canvas.focus_set())

    def _on_sb(self, first, last):
        self.sb.set(first, last)
        self._schedule_draw()

    def _yview(self, *args):
        self.canvas.yview(*args)
        self._schedule_draw()

    def _wheel(self, e):
        self.canvas.yview_scroll(int(-e.delta / 120), "units")
        self._schedule_draw()
        return "break"

    def set_items(self, items, keep_scroll=False):
        y = self.canvas.canvasy(0) if keep_scroll else 0
        self.items = items
        h = max(len(items) * self.ROW, 1)
        self.canvas.configure(scrollregion=(0, 0, 220, h))
        if not keep_scroll:
            self.canvas.yview_moveto(0)
        else:
            self.canvas.yview_moveto(min(1.0, y / max(h, 1)))
        self._draw()

    def redraw(self):
        self._draw()

    def _schedule_draw(self):
        if self._draw_job:
            self.after_cancel(self._draw_job)
        self._draw_job = self.after(8, self._draw)

    def _visible(self):
        h = max(self.canvas.winfo_height(), 1)
        y0 = self.canvas.canvasy(0)
        i0 = max(0, int(y0 // self.ROW) - 1)
        i1 = min(len(self.items), int((y0 + h) // self.ROW) + 2)
        return i0, i1

    def _draw(self):
        self._draw_job = None
        self.canvas.delete("row")
        if not self.items:
            return
        i0, i1 = self._visible()
        w = max(self.canvas.winfo_width(), 40)
        sel = self.app.selected_champ["id"] if self.app.selected_champ else None
        for i in range(i0, i1):
            c = self.items[i]
            y = i * self.ROW
            on = c["id"] == sel
            bg = CLR_CARD_SEL if on else CLR_PANEL
            self.canvas.create_rectangle(
                6, y + 2, w - 4, y + self.ROW - 2,
                fill=bg, outline=CLR_PURPLE if on else bg, width=1, tags="row")
            ph = self.photos.get(c["id"])
            if ph:
                self.canvas.create_image(22, y + self.ROW // 2, image=ph, tags="row")
            else:
                self._ask_icon(c)
            self.canvas.create_text(
                40, y + self.ROW // 2, text=c["name"], anchor="w",
                fill=CLR_TEXT if on else CLR_TEXT2,
                font=("Segoe UI", 10), tags="row")
            fav = c["id"] in self.app.fav_champs
            self.canvas.create_text(
                w - 14, y + self.ROW // 2,
                text="\u2605" if fav else "\u2606",
                fill=CLR_STAR if fav else CLR_MUTED,
                font=("Segoe UI", 11), tags="row")

    def _ask_icon(self, champ):
        cid = champ["id"]
        if cid in self._asked:
            return
        self._asked.add(cid)
        self.app.imgs.get_async(
            CHAMP_ICON_URL.format(cid=cid), (22, 22),
            lambda img, i=cid: self._got_icon(i, img),
            corner=11, photo=True)

    def _got_icon(self, cid, img):
        self.photos[cid] = img
        self._schedule_draw()

    def _click(self, e):
        if not self.items:
            return
        i = int(self.canvas.canvasy(e.y) // self.ROW)
        if i < 0 or i >= len(self.items):
            return
        champ = self.items[i]
        w = self.canvas.winfo_width()
        if e.x >= w - 28:
            self.app._toggle_fav_champ_id(champ["id"])
        else:
            self.app._select_champion(champ)


SKIN_IMG = (216, 122)
SKIN_CARD_W = 232
SKIN_CARD_H = 228


class SkinSlot(tk.Frame):
    def __init__(self, master, app):
        super().__init__(master, bg=CLR_CARD, highlightbackground=CLR_BORDER,
                         highlightthickness=1, width=SKIN_CARD_W, height=SKIN_CARD_H)
        self.pack_propagate(False)
        self.app = app
        self.skin = None
        self.bound_id = None
        self.selected_chroma = None
        self._busy = False
        self._photo = None
        self._token = 0
        self.img = tk.Label(self, bg=CLR_BG, width=SKIN_IMG[0], height=SKIN_IMG[1], bd=0)
        self.img.pack(padx=7, pady=(7, 4))
        self.img.bind("<Button-1>", lambda e: self.skin and self.app._open_detail(self.skin, self))
        self.name = tk.Label(self, bg=CLR_CARD, fg=CLR_TEXT, font=("Segoe UI", 9, "bold"),
                             wraplength=200, justify="left", anchor="w")
        self.name.pack(fill="x", padx=8)
        self.chroma = tk.Canvas(self, height=18, bg=CLR_CARD, highlightthickness=0, bd=0)
        self.chroma.pack(fill="x", padx=6, pady=(2, 0))
        self.chroma.bind("<Button-1>", self._chroma_click)
        self.add_btn = tk.Button(
            self, text=app.t("add"), bg=CLR_GOLD_DK, fg="#1a1400", relief="flat",
            font=("Segoe UI", 9, "bold"), cursor="hand2", command=self._add,
            activebackground=CLR_GOLD, activeforeground="#1a1400")
        self.add_btn.pack(fill="x", padx=8, pady=(6, 8), ipady=3)
        self.bind("<Enter>", lambda e: self.configure(highlightbackground=CLR_BORDER_HOV))
        self.bind("<Leave>", lambda e: self.configure(highlightbackground=CLR_BORDER))
        for w in (self, self.img, self.name, self.chroma, self.add_btn):
            w.bind("<MouseWheel>", self._wheel)

    def _wheel(self, e):
        self.app.skin_grid._wheel(e)
        return "break"

    def bind_skin(self, skin):
        if self.bound_id == skin["id"] and self.skin is skin:
            return
        self.skin = skin
        self.bound_id = skin["id"]
        self.selected_chroma = None
        self._busy = False
        self._token += 1
        token = self._token
        self.name.configure(text=skin["name"])
        self.add_btn.configure(text=self.app.t("add"), state="normal")
        self.img.configure(image="", text="")
        self._photo = None
        chromas = self.app._get_chromas(skin)
        if chromas:
            self.chroma.pack(fill="x", padx=6, pady=(2, 0))
            self._paint_chromas(chromas)
        else:
            self.chroma.pack_forget()
        url = skin.get("tile") or skin.get("splash")
        self.app.imgs.get_async(
            url, SKIN_IMG,
            lambda img, t=token: self._set_img(img, t),
            corner=0, photo=True)

    def _set_img(self, img, token):
        if token != self._token or not self.winfo_exists():
            return
        self._photo = img
        self.img.configure(image=img)

    def _paint_chromas(self, chromas):
        self.chroma.delete("all")
        self._hits = []
        x = 4
        for ch in [None] + chromas[:10]:
            color = "#d4d4de" if ch is None else chroma_hex(ch["name"])
            on = (ch is None and self.selected_chroma is None) or (
                ch and self.selected_chroma and ch["id"] == self.selected_chroma["id"])
            self.chroma.create_oval(x, 2, x + 14, 16, fill=color,
                                    outline=CLR_GOLD if on else color, width=2)
            self._hits.append((x, x + 16, ch))
            x += 18

    def _chroma_click(self, e):
        if not self.skin:
            return
        for x0, x1, ch in getattr(self, "_hits", []):
            if x0 <= e.x <= x1:
                self.selected_chroma = ch
                self._paint_chromas(self.app._get_chromas(self.skin))
                break

    def _select_chroma(self, chroma):
        self.selected_chroma = chroma
        if self.skin:
            chromas = self.app._get_chromas(self.skin)
            if chromas:
                self._paint_chromas(chromas)

    def _add(self):
        if self._busy or not self.skin:
            return
        self._busy = True
        self.add_btn.configure(text=self.app.t("sending"), state="disabled")
        self.app._send_to_ltk(self.skin, self.selected_chroma, card=self)

    def send_done(self, ok):
        self._busy = False
        try:
            self.add_btn.configure(text=self.app.t("add") if not ok else self.app.t("sent"), state="normal")
            if ok:
                self.after(1400, lambda: self.add_btn.winfo_exists() and self.add_btn.configure(text=self.app.t("add")))
        except Exception:
            pass


class VirtualSkinGrid(tk.Frame):
    def __init__(self, master, app):
        super().__init__(master, bg=CLR_BG2, highlightthickness=0)
        self.app = app
        self.skins = []
        self.cols = 4
        self.slots = []
        self._ids = []
        self._draw_job = None
        self.canvas = tk.Canvas(self, bg=CLR_BG2, highlightthickness=0, bd=0)
        self.sb = ctk.CTkScrollbar(self, command=self._yview, width=12,
                                   fg_color=CLR_BG2, button_color=CLR_BORDER,
                                   button_hover_color=CLR_MUTED)
        self.canvas.pack(side="left", fill="both", expand=True)
        self.sb.pack(side="right", fill="y")
        self.canvas.configure(yscrollcommand=self._on_sb)
        self.canvas.bind("<Configure>", lambda e: self._schedule())
        self.canvas.bind("<MouseWheel>", self._wheel)
        self.bind("<Enter>", lambda e: self.canvas.focus_set())

    def _on_sb(self, first, last):
        self.sb.set(first, last)
        self._schedule()

    def _yview(self, *args):
        self.canvas.yview(*args)
        self._schedule()

    def _wheel(self, e):
        self.canvas.yview_scroll(int(-e.delta / 120), "units")
        self._schedule()
        return "break"

    def set_cols(self, cols):
        cols = max(2, min(6, cols))
        if cols != self.cols:
            self.cols = cols
            self.canvas.yview_moveto(0)
            self._refresh_region()
            self._draw()

    def set_skins(self, skins):
        self.skins = skins
        self.canvas.yview_moveto(0)
        self._refresh_region()
        self._draw()

    def _refresh_region(self):
        rows = max(1, (len(self.skins) + self.cols - 1) // self.cols) if self.skins else 1
        self.canvas.configure(scrollregion=(0, 0, 800, rows * SKIN_CARD_H + 16))

    def _schedule(self):
        if self._draw_job:
            self.after_cancel(self._draw_job)
        self._draw_job = self.after(10, self._draw)

    def _ensure(self, n):
        while len(self.slots) < n:
            slot = SkinSlot(self.canvas, self.app)
            wid = self.canvas.create_window(0, 0, window=slot, anchor="nw", state="hidden")
            self.slots.append(slot)
            self._ids.append(wid)

    def _draw(self):
        self._draw_job = None
        cw = max(self.canvas.winfo_width(), SKIN_CARD_W)
        ch = max(self.canvas.winfo_height(), 1)
        if not self.skins:
            for wid in self._ids:
                self.canvas.itemconfigure(wid, state="hidden")
            self.canvas.delete("empty")
            self.canvas.create_text(
                cw // 2, 80, text=self.app.t("pick_champion"), fill=CLR_MUTED,
                font=("Segoe UI", 14), tags="empty")
            return
        self.canvas.delete("empty")
        y0 = self.canvas.canvasy(0)
        row0 = max(0, int(y0 // SKIN_CARD_H) - 1)
        vis_rows = int(ch // SKIN_CARD_H) + 3
        need = vis_rows * self.cols
        self._ensure(need)
        gap = max(8, (cw - self.cols * SKIN_CARD_W) // (self.cols + 1))
        used = 0
        for r in range(vis_rows):
            for c in range(self.cols):
                idx = (row0 + r) * self.cols + c
                if used >= len(self.slots):
                    break
                wid = self._ids[used]
                slot = self.slots[used]
                used += 1
                if idx >= len(self.skins):
                    self.canvas.itemconfigure(wid, state="hidden")
                    continue
                x = gap + c * (SKIN_CARD_W + gap)
                y = (row0 + r) * SKIN_CARD_H + 8
                self.canvas.itemconfigure(wid, state="normal")
                self.canvas.coords(wid, x, y)
                slot.bind_skin(self.skins[idx])
        for i in range(used, len(self._ids)):
            self.canvas.itemconfigure(self._ids[i], state="hidden")



# ------------------------------------------------------------------ ana uygulama

class HalitChanger(ctk.CTk):
    def __init__(self):
        super().__init__()
        self.title(APP_NAME)
        self.geometry("1380x860")
        self.minsize(980, 640)
        self.configure(fg_color=CLR_BG)
        try:
            if os.path.isfile(ICON_PATH):
                self.iconbitmap(ICON_PATH)
        except Exception:
            pass

        self.cfg = load_config()
        self.lang = self.cfg.get("lang") or self._detect_lang()
        self.fav_champs = {int(x) for x in (self.cfg.get("fav_champs") or [])}
        self.fav_skins = {int(x) for x in (self.cfg.get("fav_skins") or [])}
        self.skin_ids = load_skin_ids()
        self.id_to_en, self.chromas_by_parent = index_chromas(self.skin_ids)
        self.champions = []
        self.skins_by_champ = {}
        self.selected_champ = None
        self.url_cache = {}
        self.imgs = ImageLoader(self)
        self.ready = False
        self._champ_rows = []
        self.ltk_running = False
        self._ltk_running_next = None
        self._ltk_toast = None
        self._ltk_protocol_names = None
        self._skin_cols = 4
        self._current_skins = []
        self._grid_host = None
        self._settings_win = None
        self._resize_job = None
        self._loader = None
        self._champ_queue = []

        self._build_ui()
        self._build_loader()
        self.protocol("WM_DELETE_WINDOW", self._on_close)
        self.after(80, self._apply_dark_titlebar)
        threading.Thread(target=self._setup, daemon=True).start()
        self.after(280, self._auto_start_ltk)
        self.after(400, self._poll_ltk_status)

    # -------------------------------------------------------------- windows chrome

    def _hwnd(self):
        self.update_idletasks()
        try:
            return ctypes.windll.user32.GetParent(self.winfo_id())
        except Exception:
            return 0

    def _apply_dark_titlebar(self):
        try:
            hwnd = self._hwnd()
            if not hwnd:
                return
            value = ctypes.c_int(1)
            for attr in (20, 19):
                ctypes.windll.dwmapi.DwmSetWindowAttribute(
                    hwnd, attr, ctypes.byref(value), ctypes.sizeof(value))
            caption = ctypes.c_int(0x000C0A07)
            text = ctypes.c_int(0x00F8F0F2)
            border = ctypes.c_int(0x0068223B)
            ctypes.windll.dwmapi.DwmSetWindowAttribute(hwnd, 35, ctypes.byref(caption), 4)
            ctypes.windll.dwmapi.DwmSetWindowAttribute(hwnd, 36, ctypes.byref(text), 4)
            ctypes.windll.dwmapi.DwmSetWindowAttribute(hwnd, 34, ctypes.byref(border), 4)
        except Exception:
            pass

    def _on_close(self):
        try:
            self.imgs.pool.shutdown(wait=False, cancel_futures=True)
        except Exception:
            pass
        try:
            self.quit()
        except Exception:
            pass
        self.destroy()

    # -------------------------------------------------------------- dil

    @staticmethod
    def _detect_lang():
        try:
            langid = ctypes.windll.kernel32.GetUserDefaultUILanguage()
            if (langid & 0xFF) == 0x1A:  # LANG_TURKISH
                return "tr"
        except Exception:
            pass
        return "en"

    def t(self, key, **kw):
        entry = STRINGS.get(key, {})
        text = entry.get(self.lang) or entry.get("en") or key
        return text.format(**kw) if kw else text

    def _set_lang(self, code):
        if code not in ("tr", "en") or code == self.lang:
            return
        self.lang = code
        self.cfg["lang"] = code
        save_config(self.cfg)
        self._refresh_lang_ui()

    def _refresh_lang_ui(self):
        try:
            self.lang_switch.set(self.lang.upper())
        except Exception:
            pass
        try:
            self.side_title.configure(text=self.t("champions_header"))
        except Exception:
            pass
        try:
            self.search_entry.configure(placeholder_text=self.t("search_placeholder"))
        except Exception:
            pass
        try:
            self.settings_btn.configure(text=self.t("settings"))
        except Exception:
            pass
        try:
            if self.selected_champ:
                n = len(self._current_skins)
                self.skin_count_lbl.configure(text=self.t("skins_count", n=n))
            else:
                self.champ_title.configure(text=self.t("select_champion"))
                self.skin_count_lbl.configure(text=self.t("select_champion_sub"))
        except Exception:
            pass
        self._sync_champ_fav_btn()
        try:
            self._apply_ltk_badge(self.ltk_running)
        except Exception:
            pass
        try:
            if self.ready:
                self.set_status(self.t("ready", n=len(self.champions)))
            else:
                self.set_status(self.t("loading_status"))
        except Exception:
            pass
        try:
            if self._current_skins:
                self.skin_grid.set_skins(self._current_skins)
            else:
                self.skin_grid._draw()
        except Exception:
            pass

    # -------------------------------------------------------------- UI

    def _build_ui(self):
        self.grid_columnconfigure(1, weight=1)
        self.grid_rowconfigure(2, weight=1)
        self._build_header()
        self._build_sidebar()
        self._build_center()

    def _build_loader(self):
        self._loader = ctk.CTkFrame(self, fg_color=CLR_BG, corner_radius=0)
        self._loader.place(relx=0, rely=0, relwidth=1, relheight=1)
        card = ctk.CTkFrame(self._loader, fg_color=CLR_PANEL, corner_radius=18,
                            border_width=1, border_color=CLR_BORDER, width=360, height=220)
        card.place(relx=0.5, rely=0.5, anchor="center")
        card.pack_propagate(False)

        if os.path.isfile(LOGO_PATH):
            try:
                pil = Image.open(LOGO_PATH).convert("RGBA")
                pil.thumbnail((64, 64), Image.BILINEAR)
                img = ctk.CTkImage(light_image=pil, dark_image=pil, size=pil.size)
                logo = ctk.CTkLabel(card, text="", image=img)
                logo.pack(pady=(22, 4))
                logo._ref = img
            except Exception:
                pass

        name = ctk.CTkFrame(card, fg_color="transparent")
        name.pack()
        ctk.CTkLabel(name, text="HALIT", font=ctk.CTkFont(size=18, weight="bold"),
                     text_color=CLR_GOLD).pack(side="left")
        ctk.CTkLabel(name, text=" CHANGER", font=ctk.CTkFont(size=18, weight="bold"),
                     text_color=CLR_PURPLE_HOV).pack(side="left")

        self._loader_bar = ctk.CTkProgressBar(
            card, width=220, height=4, corner_radius=2,
            fg_color=CLR_CARD, progress_color=CLR_PURPLE,
            mode="indeterminate")
        self._loader_bar.pack(pady=(16, 8))
        self._loader_bar.start()

        self._loader_label = ctk.CTkLabel(
            card, text=self.t("loading_data"), font=ctk.CTkFont(size=12), text_color=CLR_MUTED)
        self._loader_label.pack(pady=(0, 8))
        self._loader.lift()

    def _set_loader(self, text):
        def apply():
            try:
                if self._loader and self._loader_label.winfo_exists():
                    self._loader_label.configure(text=text)
            except Exception:
                pass
        self.after(0, apply)

    def _hide_loader(self):
        def hide():
            loader = self._loader
            if not loader:
                return
            try:
                self._loader_bar.stop()
            except Exception:
                pass
            try:
                loader.place_forget()
                loader.destroy()
            except Exception:
                pass
            self._loader = None
        self.after(0, hide)

    def _build_header(self):
        header = ctk.CTkFrame(self, corner_radius=0, fg_color=CLR_PANEL, height=58)
        header.grid(row=0, column=0, columnspan=2, sticky="ew")
        header.grid_propagate(False)

        left = ctk.CTkFrame(header, fg_color="transparent")
        left.pack(side="left", padx=14, fill="y")

        if os.path.isfile(LOGO_PATH):
            try:
                pil = Image.open(LOGO_PATH).convert("RGBA")
                pil.thumbnail((40, 40), Image.LANCZOS)
                logo_img = ctk.CTkImage(light_image=pil, dark_image=pil, size=pil.size)
                lbl = ctk.CTkLabel(left, text="", image=logo_img, width=40)
                lbl.pack(side="left", padx=(0, 10))
                lbl._ref = logo_img
            except Exception:
                pass

        name_frame = ctk.CTkFrame(left, fg_color="transparent")
        name_frame.pack(side="left")
        t1 = ctk.CTkLabel(name_frame, text="HALIT", font=ctk.CTkFont(size=18, weight="bold"),
                          text_color=CLR_GOLD)
        t2 = ctk.CTkLabel(name_frame, text=" CHANGER", font=ctk.CTkFont(size=18, weight="bold"),
                          text_color=CLR_PURPLE_HOV)
        t1.pack(side="left")
        t2.pack(side="left")

        right = ctk.CTkFrame(header, fg_color="transparent")
        right.pack(side="right", padx=(0, 8), fill="y")

        self.ltk_pill = ctk.CTkFrame(right, fg_color=CLR_CARD, corner_radius=16,
                                     border_width=1, border_color=CLR_BORDER)
        self.ltk_pill.pack(side="left", padx=(0, 8), pady=14)
        self.ltk_badge = ctk.CTkLabel(
            self.ltk_pill, text=self.t("ltk_checking"),
            font=ctk.CTkFont(size=11, weight="bold"), text_color=CLR_MUTED)
        self.ltk_badge.pack(padx=4, pady=4)
        self.ltk_badge.bind("<Button-1>", lambda e: self._open_ltk(force=True))
        self.ltk_pill.bind("<Button-1>", lambda e: self._open_ltk(force=True))

        self.settings_btn = ctk.CTkButton(
            right, text=self.t("settings"), width=88, height=30, corner_radius=8,
            fg_color=CLR_CARD, hover_color=CLR_CARD_HOV,
            border_color=CLR_BORDER, border_width=1,
            font=ctk.CTkFont(size=12),
            command=self._open_settings)
        self.settings_btn.pack(side="left", pady=14)

        self.lang_switch = ctk.CTkSegmentedButton(
            right, values=["TR", "EN"], width=100, height=30, corner_radius=8,
            fg_color=CLR_CARD, selected_color=CLR_PURPLE_DK, selected_hover_color=CLR_PURPLE,
            unselected_color=CLR_CARD, unselected_hover_color=CLR_CARD_HOV,
            font=ctk.CTkFont(size=11, weight="bold"),
            command=lambda v: self._set_lang(v.lower()))
        self.lang_switch.set(self.lang.upper())
        self.lang_switch.pack(side="left", padx=(0, 8), pady=14)

        self.status_label = ctk.CTkLabel(right, text=self.t("loading_status"),
                                         font=ctk.CTkFont(size=11), text_color=CLR_MUTED)
        self.status_label.pack(side="left", padx=(12, 4), pady=14)

        accent = ctk.CTkFrame(self, height=2, fg_color=CLR_PURPLE_DIM, corner_radius=0)
        accent.grid(row=1, column=0, columnspan=2, sticky="ew")
        accent.grid_propagate(False)

    def _build_sidebar(self):
        side = ctk.CTkFrame(self, width=248, fg_color=CLR_PANEL, corner_radius=0)
        side.grid(row=2, column=0, sticky="nsw")
        side.grid_rowconfigure(2, weight=1)
        side.grid_propagate(False)
        side.grid_columnconfigure(0, weight=1)

        self.side_title = ctk.CTkLabel(side, text=self.t("champions_header"),
                                       font=ctk.CTkFont(size=11, weight="bold"),
                                       text_color=CLR_MUTED)
        self.side_title.grid(row=0, column=0, padx=16, pady=(14, 6), sticky="w")

        self.search_var = ctk.StringVar()
        self.search_var.trace_add("write", lambda *a: self._filter_champions())
        self.search_entry = ctk.CTkEntry(
            side, textvariable=self.search_var, height=36, corner_radius=10,
            placeholder_text=self.t("search_placeholder"),
            placeholder_text_color=CLR_MUTED,
            fg_color=CLR_CARD, border_color=CLR_BORDER, border_width=1,
            font=ctk.CTkFont(size=13))
        self.search_entry.grid(row=1, column=0, padx=12, pady=(0, 8), sticky="ew")

        self.champ_list = ChampCanvas(side, self)
        self.champ_list.grid(row=2, column=0, sticky="nsew", padx=(6, 4), pady=(0, 10))

    def _build_center(self):
        center = ctk.CTkFrame(self, fg_color=CLR_BG2, corner_radius=0)
        center.grid(row=2, column=1, sticky="nsew")
        center.grid_rowconfigure(1, weight=1)
        center.grid_columnconfigure(0, weight=1)
        self._center = center
        center.bind("<Configure>", self._on_center_resize)

        head = ctk.CTkFrame(center, fg_color="transparent")
        head.grid(row=0, column=0, sticky="ew", padx=22, pady=(16, 6))

        self.champ_icon_lbl = ctk.CTkLabel(head, text="", width=40, height=40)
        self.champ_icon_lbl.pack(side="left", padx=(0, 10))

        titles = ctk.CTkFrame(head, fg_color="transparent")
        titles.pack(side="left", fill="y")
        self.champ_title = ctk.CTkLabel(titles, text=self.t("select_champion"),
                                        font=ctk.CTkFont(size=22, weight="bold"),
                                        text_color=CLR_TEXT)
        self.champ_title.pack(anchor="w")
        self.skin_count_lbl = ctk.CTkLabel(titles, text=self.t("select_champion_sub"),
                                           font=ctk.CTkFont(size=12), text_color=CLR_MUTED)
        self.skin_count_lbl.pack(anchor="w")

        self.champ_fav_btn = ctk.CTkButton(
            head, text=self.t("favorite"), width=110, height=32, corner_radius=8,
            fg_color=CLR_CARD, hover_color=CLR_CARD_HOV, border_width=1, border_color=CLR_BORDER,
            font=ctk.CTkFont(size=12), command=self._toggle_fav_champ)
        self.champ_fav_btn.pack(side="right")
        self.champ_fav_btn.pack_forget()

        self.skin_grid = VirtualSkinGrid(center, self)
        self.skin_grid.grid(row=1, column=0, sticky="nsew", padx=14, pady=(0, 12))

    # -------------------------------------------------------------- toast + log

    def toast(self, msg, success=True):
        self.after(0, lambda: Toast(self, msg, success=success))

    def log(self, msg):
        try:
            with open(LOG_FILE, "a", encoding="utf-8") as f:
                f.write(f"[{datetime.datetime.now():%H:%M:%S}] {msg}\n")
        except OSError:
            pass

    def set_status(self, text):
        self.after(0, lambda: self.status_label.configure(text=text) if self.status_label.winfo_exists() else None)

    def _open_log(self):
        if os.path.isfile(LOG_FILE):
            subprocess.Popen(["notepad", LOG_FILE], creationflags=NO_WINDOW)
        else:
            self.toast(self.t("log_not_found"), success=False)

    def _open_settings(self):
        if self._settings_win and self._settings_win.winfo_exists():
            self._settings_win.lift()
            return
        win = ctk.CTkToplevel(self)
        self._settings_win = win
        win.title(self.t("settings"))
        win.geometry("340x280")
        win.configure(fg_color=CLR_BG)
        win.resizable(False, False)
        win.attributes("-topmost", True)
        win.transient(self)
        try:
            if os.path.isfile(ICON_PATH):
                win.after(200, lambda: win.iconbitmap(ICON_PATH))
        except Exception:
            pass
        pad = ctk.CTkFrame(win, fg_color=CLR_PANEL, corner_radius=12)
        pad.pack(fill="both", expand=True, padx=16, pady=16)
        ctk.CTkLabel(pad, text=self.t("settings"), font=ctk.CTkFont(size=16, weight="bold"),
                     text_color=CLR_TEXT).pack(anchor="w", padx=16, pady=(14, 10))
        for text, cmd in (
            (self.t("open_restart_ltk"), lambda: self._open_ltk(force=True)),
            (self.t("open_log"), self._open_log),
            (self.t("ltk_download_page"), lambda: webbrowser.open(
                "https://github.com/LeagueToolkit/ltk-manager/releases/latest")),
        ):
            ctk.CTkButton(pad, text=text, height=36, corner_radius=8,
                          fg_color=CLR_CARD, hover_color=CLR_CARD_HOV,
                          border_width=1, border_color=CLR_BORDER,
                          font=ctk.CTkFont(size=13), command=cmd).pack(fill="x", padx=16, pady=4)
        ctk.CTkLabel(pad, text="Halit Changer  \u00b7  LTK Manager",
                     font=ctk.CTkFont(size=11), text_color=CLR_MUTED).pack(pady=(12, 8))

    # -------------------------------------------------------------- LTK

    def _prepare_ltk(self):
        try:
            if not os.path.isfile(LTK_SETTINGS):
                return
            with open(LTK_SETTINGS, "r", encoding="utf-8") as f:
                data = json.load(f)
            domains = data.get("trustedDomains", [])
            if DOWNLOAD_DOMAIN not in domains:
                domains.append(DOWNLOAD_DOMAIN)
                data["trustedDomains"] = domains
                with open(LTK_SETTINGS, "w", encoding="utf-8") as f:
                    json.dump(data, f, ensure_ascii=False, indent=2)
                self.log(f"Trusted domain added: {DOWNLOAD_DOMAIN}")
        except Exception as e:
            self.log(f"LTK settings update failed: {e}")

    def _check_ltk_running(self):
        try:
            protocol = getattr(self, "_ltk_protocol_names", None)
            if protocol is None:
                protocol = _ltk_protocol_basenames()
                self._ltk_protocol_names = protocol
            names = _running_exe_names()
            if names:
                return any(_is_ltk_exe_name(n, protocol) for n in names)
            r = subprocess.run(
                ["tasklist", "/fo", "csv", "/nh"],
                capture_output=True, text=True, timeout=8, creationflags=NO_WINDOW
            )
            out = (r.stdout or "").lower()
            if any(h in out for h in _LTK_EXE_HINTS):
                return True
            return any(n in out for n in protocol)
        except Exception:
            return False

    def _auto_start_ltk(self):
        def work():
            try:
                if self._check_ltk_running():
                    self.log("LTK already running.")
                    self._ltk_running_next = True
                    return
                try:
                    os.startfile("ltk://")
                    self.log("LTK auto-started via protocol.")
                    time.sleep(2.5)
                    self._ltk_running_next = self._check_ltk_running()
                except Exception:
                    self.log("LTK not found.")
                    self._ltk_running_next = False
                    self._ltk_toast = (self.t("ltk_not_started"), False)
            except Exception as e:
                self.log(f"LTK auto-start failed: {e}")
                self._ltk_running_next = False
                self._ltk_toast = (self.t("ltk_conn_not_found"), False)
        threading.Thread(target=work, daemon=True).start()

    def _apply_ltk_badge(self, running):
        self.ltk_running = bool(running)
        try:
            if running:
                self.ltk_badge.configure(text=self.t("ltk_connected"), text_color=CLR_GREEN)
                self.ltk_pill.configure(fg_color=CLR_GREEN_DIM, border_color=CLR_GREEN)
            else:
                self.ltk_badge.configure(text=self.t("ltk_offline"), text_color=CLR_RED)
                self.ltk_pill.configure(fg_color=CLR_RED_DIM, border_color=CLR_RED)
        except Exception:
            pass

    def _poll_ltk_status(self):
        if not self.winfo_exists():
            return
        toast = getattr(self, "_ltk_toast", None)
        if toast:
            self._ltk_toast = None
            msg, ok = toast
            self.toast(msg, success=ok)
        flag = getattr(self, "_ltk_running_next", None)
        if flag is not None:
            self._apply_ltk_badge(flag)
        else:
            self._ltk_running_next = self._check_ltk_running()
            self._apply_ltk_badge(self._ltk_running_next)

        def probe():
            self._ltk_running_next = self._check_ltk_running()

        threading.Thread(target=probe, daemon=True).start()
        delay = 2000 if not getattr(self, "ltk_running", False) else 5000
        self.after(delay, self._poll_ltk_status)

    def _open_ltk(self, force=False):
        def work():
            running = self._check_ltk_running()
            if running and not force:
                self._ltk_running_next = True
                return
            try:
                os.startfile("ltk://")
                self.log("Opening LTK...")
                time.sleep(2.0)
                self._ltk_running_next = self._check_ltk_running()
            except Exception:
                self.log("LTK not found - download page opened.")
                webbrowser.open("https://github.com/LeagueToolkit/ltk-manager/releases/latest")
                self._ltk_toast = (self.t("ltk_not_found_dl"), False)
                self._ltk_running_next = False
        threading.Thread(target=work, daemon=True).start()

    # -------------------------------------------------------------- setup

    def _setup(self):
        ensure_dirs()
        self.log("=== Halit Changer started ===")
        try:
            self._set_loader(self.t("loading_data"))
            self.set_status(self.t("loading_status"))
            cached = self._load_data(force_network=False)
            if cached:
                self._set_loader(self.t("preparing_list"))
                self.after(0, self._render_champions)
                threading.Thread(target=self._refresh_data, daemon=True).start()
            else:
                self._set_loader(self.t("first_launch"))
                self._load_data(force_network=True)
                self._set_loader(self.t("preparing_list"))
                self.after(0, self._render_champions)
            self._prepare_ltk()
            self.ready = True
            self.set_status(self.t("ready", n=len(self.champions)))
            self.log(f"Ready. {len(self.champions)} champions loaded.")
        except Exception as e:
            self.set_status(self.t("error_status"))
            self.log(f"ERROR: {e}")
            self._set_loader(self.t("load_failed"))
            self._hide_loader()
            self.toast(self.t("data_load_failed", e=e), success=False)

    def _refresh_data(self):
        try:
            self._load_data(force_network=True)
        except Exception as e:
            self.log(f"Background refresh failed: {e}")

    @staticmethod
    def _nice_rarity(raw):
        return {"kEpic": "Epic", "kLegendary": "Legendary", "kMythic": "Mythic",
                "kUltimate": "Ultimate", "kTranscendent": "Transcendent",
                "kExalted": "Exalted"}.get(raw or "", "")

    def _load_data(self, force_network=False):
        champs = fetch_json_cached(CHAMPIONS_URL, "champions.json", force_network=force_network)
        skins = fetch_json_cached(SKINS_URL, "skins.json", force_network=force_network)
        if not champs:
            if force_network:
                raise RuntimeError("Champion data unavailable")
            return False
        self.champions = sorted(
            [{"id": c["id"], "name": c["name"], "alias": c.get("alias", c["name"])}
             for c in champs if c.get("id", 0) > 0], key=lambda c: c["name"].lower())
        by_champ = {}
        for sid_str, s in (skins or {}).items():
            try:
                sid = int(sid_str)
            except (TypeError, ValueError):
                continue
            by_champ.setdefault(sid // 1000, []).append({
                "id": sid, "name": s.get("name", str(sid)),
                "tile": cdragon_url(s.get("tilePath", "")),
                "splash": cdragon_url(s.get("uncenteredSplashPath") or s.get("splashPath", "")),
                "is_base": s.get("isBase", False),
                "rarity": self._nice_rarity(s.get("rarity")),
            })
        for lst in by_champ.values():
            lst.sort(key=lambda x: x["id"])
        self.skins_by_champ = by_champ
        return True

    # -------------------------------------------------------------- sampiyonlar

    def _ordered_champs(self):
        term = (self.search_var.get() or "").lower().strip()
        items = sorted(
            self.champions,
            key=lambda c: (0 if c["id"] in self.fav_champs else 1, c["name"].lower()))
        if not term:
            return items
        return [c for c in items if term in c["name"].lower() or term in c.get("alias", "").lower()]

    def _render_champions(self):
        self.champ_list.set_items(self._ordered_champs())
        self._hide_loader()

    def _filter_champions(self):
        if not self.champions:
            return
        self.champ_list.set_items(self._ordered_champs())

    def _highlight_champ(self):
        self.champ_list.redraw()

    def _select_champion(self, champ):
        if self.selected_champ and self.selected_champ["id"] == champ["id"]:
            self._highlight_champ()
            return
        self.selected_champ = champ
        self.champ_title.configure(text=champ["name"])
        self.champ_fav_btn.pack(side="right")
        self._sync_champ_fav_btn()
        self._highlight_champ()
        self.imgs.get_async(
            CHAMP_ICON_URL.format(cid=champ["id"]), (36, 36),
            lambda img: self.champ_icon_lbl.winfo_exists() and self.champ_icon_lbl.configure(image=img),
            corner=18)
        skins = [s for s in self.skins_by_champ.get(champ["id"], []) if not s.get("is_base")]
        self._current_skins = skins
        self.skin_count_lbl.configure(text=self.t("skins_count", n=len(skins)))
        self.skin_grid.set_cols(self._skin_cols)
        self.skin_grid.set_skins(skins)

    def _sync_champ_fav_btn(self):
        if not self.selected_champ:
            return
        on = self.selected_champ["id"] in self.fav_champs
        self.champ_fav_btn.configure(
            text=self.t("favorite_on") if on else self.t("favorite"),
            text_color=CLR_STAR if on else CLR_TEXT2)

    def _persist_favs(self):
        self.cfg["fav_champs"] = sorted(self.fav_champs)
        self.cfg["fav_skins"] = sorted(self.fav_skins)
        save_config(self.cfg)

    def _toggle_fav_champ(self):
        if self.selected_champ:
            self._toggle_fav_champ_id(self.selected_champ["id"])

    def _toggle_fav_champ_id(self, cid):
        if cid in self.fav_champs:
            self.fav_champs.discard(cid)
        else:
            self.fav_champs.add(cid)
        self._persist_favs()
        self._sync_champ_fav_btn()
        self.champ_list.set_items(self._ordered_champs(), keep_scroll=True)

    def _toggle_fav_skin(self, sid):
        if sid in self.fav_skins:
            self.fav_skins.discard(sid)
        else:
            self.fav_skins.add(sid)
        self._persist_favs()

    # -------------------------------------------------------------- skin grid

    def _on_center_resize(self, event):
        if event.widget is not self._center:
            return
        cols = max(2, min(6, max(event.width - 40, 400) // 250))
        self._skin_cols = cols
        if self._resize_job:
            self.after_cancel(self._resize_job)
        self._resize_job = self.after(80, lambda: self.skin_grid.set_cols(cols))

    def _get_chromas(self, skin):
        en = self.id_to_en.get(skin["id"])
        if not en:
            return []
        return list(self.chromas_by_parent.get(en, []))

    # -------------------------------------------------------------- gonder

    def _resolve_url(self, champ_id, skin_id, chroma_id=None):
        if chroma_id:
            return f"{SKINS_RAW}/skins/{champ_id}/{skin_id}/{chroma_id}/{chroma_id}.fantome"
        return f"{SKINS_RAW}/skins/{champ_id}/{skin_id}/{skin_id}.fantome"

    def _send_to_ltk(self, skin, chroma=None, card=None):
        if not self.ready:
            self.toast(self.t("still_loading"), success=False)
            if card:
                card.send_done(False)
            return
        if not self.selected_champ:
            if card:
                card.send_done(False)
            return
        champ = self.selected_champ
        chroma_id = chroma["id"] if chroma else None
        pack_name = chroma["name"] if chroma else None
        label = skin["name"] + (f" ({pack_name})" if pack_name else "")

        def work():
            url = self._resolve_url(champ["id"], skin["id"], chroma_id)
            name = urllib.parse.quote(f"{champ['name']} - {label}")
            deep = (f"ltk://install?url={urllib.parse.quote(url, safe='')}"
                    f"&name={name}&author=LeagueSkins&source=HalitChanger")
            try:
                os.startfile(deep)
                self.log(f"  -> Sent to LTK: {label}")
                self.toast(self.t("sent_toast", label=label), success=True)
                if card:
                    self.after(0, lambda: card.send_done(True))
            except OSError:
                self.log("  X LTK send failed")
                self.toast(self.t("send_failed"), success=False)
                if card:
                    self.after(0, lambda: card.send_done(False))

        threading.Thread(target=work, daemon=True).start()

    # -------------------------------------------------------------- detay

    def _open_detail(self, skin, card=None):
        champ = self.selected_champ
        if not champ:
            return
        win = ctk.CTkToplevel(self)
        win.title(skin["name"])
        win.geometry("720x640")
        win.configure(fg_color=CLR_BG)
        win.minsize(620, 520)
        win.transient(self)
        try:
            if os.path.isfile(ICON_PATH):
                win.after(200, lambda: win.iconbitmap(ICON_PATH))
        except Exception:
            pass

        splash = ctk.CTkLabel(win, text=self.t("loading_img"), width=688, height=300,
                              fg_color=CLR_CARD, corner_radius=12, text_color=CLR_MUTED,
                              font=ctk.CTkFont(size=14))
        splash.pack(padx=16, pady=(16, 8))
        self.imgs.get_async(
            skin.get("splash") or skin.get("tile"), (688, 300),
            lambda img: splash.winfo_exists() and splash.configure(image=img, text=""),
            corner=12)

        info = ctk.CTkFrame(win, fg_color="transparent")
        info.pack(fill="x", padx=20)
        ctk.CTkLabel(info, text=skin["name"], font=ctk.CTkFont(size=20, weight="bold"),
                     text_color=CLR_GOLD).pack(anchor="w")
        bits = [champ["name"], f"ID {skin['id']}"]
        if skin.get("rarity"):
            bits.append(skin["rarity"])
        ctk.CTkLabel(info, text="  \u00b7  ".join(bits),
                     font=ctk.CTkFont(size=12), text_color=CLR_MUTED).pack(anchor="w")

        chromas = self._get_chromas(skin)
        selected = {"v": card.selected_chroma if card else None}

        if chromas:
            ctk.CTkLabel(win, text=self.t("color_packs"), font=ctk.CTkFont(size=12, weight="bold"),
                         text_color=CLR_TEXT2).pack(anchor="w", padx=20, pady=(12, 4))
            ch_frame = ctk.CTkScrollableFrame(win, fg_color="transparent", height=92,
                                              scrollbar_fg_color=CLR_BG,
                                              scrollbar_button_color=CLR_BORDER)
            ch_frame.pack(fill="x", padx=16, pady=(0, 4))
            chips = []

            def paint():
                for ch, btn, color in chips:
                    on = (ch is None and selected["v"] is None) or (
                        ch and selected["v"] and ch["id"] == selected["v"]["id"])
                    btn.configure(
                        fg_color=CLR_PURPLE_DK if on else CLR_CARD,
                        border_color=CLR_GOLD if on else color)

            def pick(ch):
                selected["v"] = ch
                if card:
                    card._select_chroma(ch)
                paint()

            def add_chip(ch, label, color):
                btn = ctk.CTkButton(
                    ch_frame, text=label, height=32, corner_radius=8,
                    fg_color=CLR_CARD, hover_color=CLR_CARD_HOV,
                    border_color=color, border_width=2,
                    font=ctk.CTkFont(size=11, weight="bold"),
                    command=lambda c=ch: pick(c))
                btn.pack(side="left", padx=(0, 6), pady=4)
                chips.append((ch, btn, color))

            add_chip(None, self.t("default"), "#d4d4de")
            for ch in chromas:
                add_chip(ch, ch["name"], chroma_hex(ch["name"]))
            paint()

        bar = ctk.CTkFrame(win, fg_color="transparent")
        bar.pack(fill="x", padx=20, pady=(10, 16))

        def send_and_close():
            self._send_to_ltk(skin, selected["v"], card=card)
            win.destroy()

        ctk.CTkButton(bar, text=self.t("add_to_ltk"), height=40, corner_radius=10,
                      fg_color=CLR_GOLD_DK, hover_color=CLR_GOLD, text_color="#1a1400",
                      font=ctk.CTkFont(size=14, weight="bold"),
                      command=send_and_close).pack(side="left")
        ctk.CTkButton(bar, text=self.t("close"), height=40, width=90, corner_radius=10,
                      fg_color=CLR_CARD, hover_color=CLR_CARD_HOV,
                      border_width=1, border_color=CLR_BORDER,
                      font=ctk.CTkFont(size=13),
                      command=win.destroy).pack(side="right")
        win.bind("<Escape>", lambda e: win.destroy())


if __name__ == "__main__":
    ensure_dirs()
    app = HalitChanger()
    app.mainloop()
