use std::convert::Infallible;
use std::marker::PhantomData;
use std::num::NonZero;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use arc_swap::{ArcSwap, Guard};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use winit::window::Icon;
use voxel_runtime::sync::Unparker;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub enum FullscreenMode {
    On,
    #[default]
    Off,
    Borderless,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Fov(NonZero<u8>); // although it is more than just `NonZero` it is a start

impl Fov {
    pub const MIN: Self = Self(NonZero::new(30).unwrap());
    pub const MAX: Self = Self(NonZero::new(120).unwrap());
    
    #[inline]
    pub const fn new(fov: u8) -> Option<Self> {
        if fov < Self::MIN.get() || fov > Self::MAX.get() { 
            return None;
        }
        
        Some(Self(NonZero::new(fov).unwrap()))
    }

    pub const fn new_saturating(fov: u8) -> Self {
        match Self::new(fov) {
            Some(fov) => fov,
            None if fov < 30 => Self::MIN,
            None => Self::MAX
        }
    }
    
    pub const fn get(&self) -> u8 {
        self.0.get()
    }
}

impl Serialize for Fov {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Serialize::serialize(&self.get(), serializer)
    }
}

impl<'de> Deserialize<'de> for Fov {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <u8 as Deserialize>::deserialize(deserializer).map(Fov::new_saturating)
    }
}

impl Default for Fov {
    fn default() -> Self {
        const { Self::new(45).unwrap() }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub enum Vsync {
    #[default]
    On,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct GameTitle(Box<str>);

impl Default for GameTitle {
    fn default() -> Self {
        GameTitle("Game of Voxels".into())
    }
}

impl Deref for GameTitle {
    type Target = str;
    
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}




#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct GameSettings {
    pub game_title: GameTitle,
    pub vsync: Vsync,
    pub fov: Fov,
    pub fullscreen: FullscreenMode,
}

struct GameSettingsHandleInner {
    data: ArcSwap<GameSettings>,
    modified: Unparker 
}

#[derive(Clone)]
pub struct GameSettingsHandle(Arc<GameSettingsHandleInner>);

pub struct LoadedSettings {
    guard: Guard<Arc<GameSettings>>,
    _marker: PhantomData<GameSettings>
}

impl Deref for LoadedSettings {
    type Target = GameSettings;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl LoadedSettings {
    fn load_full(self) -> Arc<GameSettings> {
        Guard::into_inner(self.guard)
    }
}


impl GameSettingsHandle {
    pub fn load(&self) -> LoadedSettings {
        LoadedSettings {
            guard: self.0.data.load(),
            _marker: PhantomData
        }
    }


    #[expect(dead_code, reason = "UI not implemented yet")]
    pub fn store(&self, settings: GameSettings) {
        if *self.load() != settings {
            self.0.data.store(Arc::new(settings));
            self.0.modified.unpark();
        }
    }
}

const SETTINGS_PATH: &str = "./settings.toml";

fn load_icon_inner() -> anyhow::Result<Icon> {
    let image = image::load_from_memory(include_bytes!("../assets/icon/voxel-engine256.png"))?.into_rgba8();
    let (width, height) = image.dimensions();

    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}

pub fn load_icon() -> Option<Icon> {
    load_icon_inner().inspect_err(|err| tracing::error!("unable to load game icon; {err}")).ok()
}

pub fn load() -> GameSettingsHandle {
    let game_settings = std::fs::read_to_string(SETTINGS_PATH)
        .ok()
        .and_then(|s| toml::from_str::<GameSettings>(&s).ok())
        .unwrap_or_default();
    
    let swap = ArcSwap::new(Arc::new(game_settings));
    let (mut parker, unparker) = voxel_runtime::sync::make_parker();
    
    let inner = GameSettingsHandleInner {
        data: swap,
        modified: unparker
    };
    
    let settings = GameSettingsHandle(Arc::new(inner));

    let settings_handle = Arc::downgrade(&settings.0);

    // spawn non async because these operations (serialization, file writing)
    // and this will live for a long time, don't put this in the blocking pool
    voxel_runtime::rt::spawn_long_lived(move || -> Option<Infallible> {
        let mut prev = {
            let handle = settings_handle.upgrade()?;
            handle.data.load_full()
        };

        let save = |settings: &GameSettings| {
            let bytes = toml::to_string_pretty(settings)
                .expect("should always be able to serialize");

            let res = std::fs::write(SETTINGS_PATH, bytes);
            if let Err(err) = res.as_ref() {
                tracing::error!("Failed to save settings; {err}")
            }

            res.is_err()
        };

        let mut last_save_err = save(&prev);

        loop {
            // join the execution poll and wait
            voxel_runtime::block_on(async {
                // Save only at most every 10 seconds
                voxel_runtime::time::sleep(Duration::from_secs(10)).await;
                parker.park().await;
                Some(())
            })?;

            
            let handle = GameSettingsHandle(settings_handle.upgrade()?);
            let current = handle.load();
            let changed = (*current) != *prev;

            if last_save_err || changed {
                match changed {
                    true => prev = current.load_full(),
                    false => drop(current)
                }

                last_save_err = save(&prev);
            }
        }
    });

    settings
}