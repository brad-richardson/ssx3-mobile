//! Platform-neutral ownership of the engine, storage and application state.
use ssx3_engine::{Audio, Engine, EngineConfig, Settings};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct MobileSettings {
    path: PathBuf,
    values: Arc<RwLock<BTreeMap<String, String>>>,
}

impl MobileSettings {
    pub fn load(save_root: &Path) -> Self {
        let path = save_root.join("ssx3-settings.cfg");
        let mut values = BTreeMap::new();
        if let Ok(contents) = std::fs::read_to_string(&path) {
            for line in contents.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let (key, value) = (key.trim(), value.trim());
                if key.starts_with("SSX3_") && !key.is_empty() {
                    values.insert(key.to_owned(), value.to_owned());
                }
            }
        }
        Self {
            path,
            values: Arc::new(RwLock::new(values)),
        }
    }

    pub fn set(&self, key: &str, value: &str) -> std::io::Result<()> {
        if !key.starts_with("SSX3_") {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        self.values
            .write()
            .unwrap()
            .insert(key.to_owned(), value.to_owned());
        let state = self.values.read().unwrap();
        let mut text = String::from("# SSX 3 mobile settings\n");
        for (k, v) in state.iter().filter(|(_, v)| !v.is_empty()) {
            text.push_str(&format!("{k} = {v}\n"));
        }
        let tmp = self.path.with_extension("cfg.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, &self.path)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        std::env::var(key)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| {
                self.values
                    .read()
                    .unwrap()
                    .get(key)
                    .filter(|v| !v.is_empty())
                    .cloned()
            })
            .or_else(|| match key {
                "SSX3_RENDER" => Some("native".into()),
                "SSX3_RENDER_SIZE" => Some("1920x1080".into()),
                "SSX3_RESOLUTION" => Some("100".into()),
                "SSX3_ASPECT" => Some("16:9".into()),
                "SSX3_CATCHUP" => Some("1".into()),
                _ => None,
            })
    }
}

impl Settings for MobileSettings {
    fn get(&self, key: &str) -> Option<String> {
        self.get(key)
    }
}

pub struct MobileCore {
    engine: Engine,
    settings: MobileSettings,
    started: std::time::Instant,
    active: bool,
    audible: bool,
}

impl MobileCore {
    pub fn boot(data_root: PathBuf, save_root: PathBuf) -> Self {
        // Both roots are explicit; the engine's one-time path selection cannot
        // silently place the memory card in a relative `saves` directory.
        let settings = MobileSettings::load(&save_root);
        let mut config = EngineConfig::new(Box::new(settings.clone()));
        config.data_root = Some(data_root);
        config.save_root = Some(save_root);
        config.find_pack = true;
        config.skip_movies = true;
        config.audio = Audio::Pull;
        let mut engine = Engine::boot(config);
        engine.set_catch_up(settings.get("SSX3_CATCHUP").as_deref() != Some("0"));
        engine.pause();
        Self {
            engine,
            settings,
            started: std::time::Instant::now(),
            active: false,
            audible: false,
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }
    pub fn settings(&self) -> &MobileSettings {
        &self.settings
    }

    pub fn activity(&mut self, active: bool, audible: bool) {
        if self.active != active {
            self.active = active;
            if active {
                self.engine.resume();
            } else {
                self.engine.set_pad(0, 0);
                self.engine.set_sticks(0, [0x80; 4]);
                self.engine.pause();
            }
        }
        if self.audible != audible {
            self.audible = audible;
            if audible {
                self.engine.audio_resume();
            } else {
                self.engine.audio_suspend();
            }
        }
    }

    pub fn advance(&mut self) -> ssx3_engine::Advance {
        self.engine.advance(self.started.elapsed().as_secs_f64())
    }

    pub fn pace(&mut self) -> ssx3_engine::Pace {
        self.engine.pace(self.started.elapsed().as_secs_f64())
    }

    pub fn step(&mut self, buttons: u16, sticks: [u8; 4]) -> bool {
        self.set_pad(buttons, sticks);
        self.engine.step()
    }

    pub fn set_display_hz(&mut self, hz: f64) -> bool {
        self.engine.set_display_hz(hz)
    }
    pub fn set_surface_size(&mut self, w: u32, h: u32) {
        self.engine.set_surface_size(w, h);
    }
    pub fn set_pad(&mut self, buttons: u16, sticks: [u8; 4]) {
        self.engine.set_pad(0, buttons);
        self.engine.set_sticks(0, sticks);
    }
    pub fn set_setting(&mut self, key: &str, value: &str) -> std::io::Result<()> {
        self.settings.set(key, value)?;
        self.engine.setting_changed(key);
        Ok(())
    }
    pub fn shutdown(&mut self) {
        self.engine.shutdown();
    }
}
