//! TR / EN interface strings.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Tr,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Tr => "tr",
            Lang::En => "en",
        }
    }
    pub fn from_code(c: &str) -> Option<Self> {
        match c {
            "tr" => Some(Lang::Tr),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
}

macro_rules! strings {
    ($($key:ident => $tr:expr, $en:expr;)*) => {
        #[allow(non_camel_case_types, dead_code)]
        #[derive(Clone, Copy, Debug)]
        pub enum S { $($key),* }
        pub fn t(lang: Lang, s: S) -> &'static str {
            match (s, lang) {
                $((S::$key, Lang::Tr) => $tr, (S::$key, Lang::En) => $en,)*
            }
        }
    };
}

strings! {
    champions_header => "ŞAMPİYONLAR", "CHAMPIONS";
    search_placeholder => "Şampiyon ara...", "Search champions...";
    skin_search => "Skin ara...", "Search skins...";
    select_champion => "Bir şampiyon seç", "Select a champion";
    select_champion_sub => "Ara, skin seç, Ekle → LTK", "Search, pick a skin, Add → LTK";
    favorite => "☆  Favori", "☆  Favorite";
    favorite_on => "★  Favori", "★  Favorite";
    settings => "Ayarlar", "Settings";
    ltk_checking => "●  LTK kontrol ediliyor...", "●  Checking LTK...";
    ltk_connected => "●  LTK Bağlı", "●  LTK Connected";
    ltk_offline => "●  LTK Kapalı", "●  LTK Offline";
    ready => "Hazır ✔  {n} şampiyon", "Ready ✔  {n} champions";
    loading_status => "Yükleniyor...", "Loading...";
    error_status => "Hata", "Error";
    pick_champion => "Soldan bir şampiyon seç", "Pick a champion on the left";
    no_skin_match => "Aramayla eşleşen skin yok", "No skins match the search";
    skins_count => "{n} skin", "{n} skins";
    add => "+  Ekle", "+  Add";
    sent => "✓  Gönderildi", "✓  Sent";
    add_to_ltk => "+  LTK'ya Ekle", "+  Add to LTK";
    close => "Kapat", "Close";
    color_packs => "Renk Paketleri", "Color Packs";
    default => "Varsayılan", "Default";
    still_loading => "Hala yükleniyor, bekle...", "Still loading, please wait...";
    sent_toast => "Skin LTK'ya gönderildi  ·  {label}", "Skin sent to LTK  ·  {label}";
    send_failed => "LTK'ya gönderilemedi", "Couldn't send to LTK";
    log_not_found => "Log dosyası bulunamadı", "Log file not found";
    open_restart_ltk => "LTK'yı Aç / Yeniden Başlat", "Open / Restart LTK";
    open_log => "Logu Aç", "Open log";
    open_data_dir => "Veri klasörünü aç", "Open data folder";
    ltk_download_page => "LTK indirme sayfası", "LTK download page";
    loading_data => "Veriler yükleniyor...", "Loading data...";
    preparing_list => "Liste hazırlanıyor...", "Preparing list...";
    first_launch => "İlk açılış, veriler indiriliyor...", "First launch, downloading data...";
    load_failed => "Yükleme başarısız", "Load failed";
    data_load_failed => "Veri yüklenemedi, internet bağlantını kontrol et", "Failed to load data, check your connection";
    retry => "Tekrar dene", "Retry";
    ltk_not_started => "LTK başlatılamadı, Ayarlar → LTK indirme sayfası", "LTK couldn't start, Settings → LTK download page";
    ltk_not_found_dl => "LTK bulunamadı, indirme sayfası açıldı", "LTK not found, download page opened";
    random => "🎲  Rastgele", "🎲  Random";
    random_hint => "Rastgele skin + renk seç (Ctrl+R)", "Pick a random skin + chroma (Ctrl+R)";
    random_pool_favs => "Favori şampiyonlardan rastgele", "Random from favorite champions";
    history => "🕘  Geçmiş", "🕘  History";
    history_title => "Gönderim Geçmişi", "Send History";
    history_empty => "Henüz LTK'ya skin göndermedin.", "You haven't sent any skins to LTK yet.";
    history_clear => "Geçmişi temizle", "Clear history";
    resend => "Tekrar gönder", "Send again";
    show => "Göster", "Show";
    fav_skin_hint => "Skini favorile, favoriler önce gelir", "Favorite this skin, favorites come first";
    ago_now => "az önce", "just now";
    ago_min => "{n} dk önce", "{n} min ago";
    ago_hour => "{n} sa önce", "{n} h ago";
    ago_day => "{n} gün önce", "{n} d ago";
    footer => "Halit Changer  ·  LTK Manager  ·  Rust", "Halit Changer  ·  LTK Manager  ·  Rust";
}
