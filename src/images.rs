//! Background image loader: download → cover-fit → disk cache → egui texture.

use crate::data::{self, fnv1a};
use egui::{ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

const WORKERS: usize = 6;
const MAX_TEXTURES: usize = 320;

struct Job {
    key: String,
    url: String,
    w: u32,
    h: u32,
}

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
}

pub struct ImageLoader {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    rx: Receiver<(String, Option<ColorImage>)>,
    textures: HashMap<String, (TextureHandle, u64)>,
    pending: HashSet<String>,
    failed: HashSet<String>,
    frame: u64,
}

impl ImageLoader {
    pub fn new(ctx: egui::Context) -> Self {
        let queue: Arc<(Mutex<Queue>, Condvar)> = Arc::default();
        let (tx, rx) = channel();
        for _ in 0..WORKERS {
            let q = queue.clone();
            let tx: Sender<_> = tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let job = {
                    let (lock, cv) = &*q;
                    let mut g = lock.lock().unwrap();
                    loop {
                        // Newest first: what the user scrolled to most recently.
                        if let Some(j) = g.jobs.pop_back() {
                            break j;
                        }
                        g = cv.wait(g).unwrap();
                    }
                };
                let img = load(&job.url, job.w, job.h);
                if tx.send((job.key, img)).is_err() {
                    return;
                }
                ctx.request_repaint();
            });
        }
        Self {
            queue,
            rx,
            textures: HashMap::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
            frame: 0,
        }
    }

    /// Call once per frame: uploads finished images and evicts old textures.
    pub fn pump(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        while let Ok((key, img)) = self.rx.try_recv() {
            self.pending.remove(&key);
            match img {
                Some(img) => {
                    let tex = ctx.load_texture(&key, img, TextureOptions::LINEAR);
                    self.textures.insert(key, (tex, self.frame));
                }
                None => {
                    self.failed.insert(key);
                }
            }
        }
        if self.textures.len() > MAX_TEXTURES {
            let mut ages: Vec<(u64, String)> =
                self.textures.iter().map(|(k, (_, f))| (*f, k.clone())).collect();
            ages.sort();
            for (_, k) in ages.into_iter().take(self.textures.len() - MAX_TEXTURES) {
                self.textures.remove(&k);
            }
        }
    }

    /// Texture if ready; otherwise schedules a load and returns None.
    pub fn get(&mut self, url: &str, w: u32, h: u32) -> Option<TextureHandle> {
        if url.is_empty() {
            return None;
        }
        let key = format!("{url}@{w}x{h}");
        if let Some((tex, used)) = self.textures.get_mut(&key) {
            *used = self.frame;
            return Some(tex.clone());
        }
        if self.failed.contains(&key) || self.pending.contains(&key) {
            return None;
        }
        self.pending.insert(key.clone());
        let (lock, cv) = &*self.queue;
        lock.lock().unwrap().jobs.push_back(Job { key, url: url.to_string(), w, h });
        cv.notify_one();
        None
    }
}

fn load(url: &str, w: u32, h: u32) -> Option<ColorImage> {
    let stem = format!("{:016x}", fnv1a(url.as_bytes()));
    let dir = data::img_cache_dir();
    let fitted_path = dir.join(format!("{stem}_{w}x{h}.png"));
    let rgba = if let Ok(img) = image::open(&fitted_path) {
        img.to_rgba8()
    } else {
        let raw_path = dir.join(format!("{stem}.bin"));
        let bytes = match std::fs::read(&raw_path) {
            Ok(b) => b,
            Err(_) => {
                let b = data::get_bytes(url).ok()?;
                let _ = std::fs::write(&raw_path, &b);
                b
            }
        };
        let img = image::load_from_memory(&bytes).ok()?;
        let fitted = cover_fit(&img.to_rgba8(), w, h);
        let _ = fitted.save(&fitted_path);
        fitted
    };
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

fn cover_fit(src: &image::RgbaImage, tw: u32, th: u32) -> image::RgbaImage {
    let (w, h) = (src.width().max(1) as f32, src.height().max(1) as f32);
    let scale = (tw as f32 / w).max(th as f32 / h);
    let nw = ((w * scale) as u32).max(tw);
    let nh = ((h * scale) as u32).max(th);
    let resized = image::imageops::resize(src, nw, nh, image::imageops::FilterType::Triangle);
    let left = (nw - tw) / 2;
    let top = (nh - th) / 2;
    image::imageops::crop_imm(&resized, left, top, tw, th).to_image()
}
