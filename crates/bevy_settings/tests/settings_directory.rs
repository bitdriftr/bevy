//! Tests for explicitly configured filesystem storage.

#![cfg(not(target_arch = "wasm32"))]

use bevy_app::App;
use bevy_ecs::{reflect::ReflectResource, resource::Resource, system::Command};
use bevy_reflect::{prelude::ReflectDefault, Reflect};
use bevy_settings::{
    ReflectSettingsGroup, SaveSettingsDeferred, SaveSettingsSync, SettingsDirectory, SettingsGroup,
    SettingsPlugin,
};
use bevy_tasks::{IoTaskPool, TaskPoolBuilder};
use bevy_time::{Real, Time};
use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

const APP_NAME: &str = "org.bevy.tests.settings_directory";

#[test]
fn loads_multiple_files_from_exact_directory_before_first_update() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new()?;
    fs::write(
        directory.0.join("settings.toml"),
        "[counter]\ncount = 41\n[display]\nenabled = true\n",
    )?;
    fs::write(directory.0.join("audio.toml"), "[audio]\nvolume = 0.25\n")?;

    let application = directory.application();

    assert_eq!(application.world().resource::<CounterSettings>().count, 41);
    assert!(application.world().resource::<DisplaySettings>().enabled);
    assert_eq!(application.world().resource::<AudioSettings>().volume, 0.25);

    Ok(())
}

#[test]
fn sync_save_keeps_directory_resolved_when_plugin_was_added() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new()?;
    let mut application = directory.application();
    let changed_directory = directory.0.join("changed");

    application.world_mut().increment_change_tick();
    application
        .world_mut()
        .resource_mut::<CounterSettings>()
        .count = 73;
    application
        .world_mut()
        .resource_mut::<DisplaySettings>()
        .enabled = true;
    application
        .world_mut()
        .resource_mut::<AudioSettings>()
        .volume = 0.5;
    application.insert_resource(SettingsDirectory(changed_directory.clone()));
    SaveSettingsSync::IfChanged.apply(application.world_mut());

    let settings: toml::Table = fs::read_to_string(directory.0.join("settings.toml"))?.parse()?;
    let audio: toml::Table = fs::read_to_string(directory.0.join("audio.toml"))?.parse()?;
    assert_eq!(settings["counter"]["count"].as_integer(), Some(73));
    assert_eq!(settings["display"]["enabled"].as_bool(), Some(true));
    assert_eq!(audio["audio"]["volume"].as_float(), Some(0.5));
    assert!(!directory.0.join(APP_NAME).exists());
    assert!(!changed_directory.exists());

    Ok(())
}

#[test]
fn deferred_save_uses_same_directory_after_delay() -> Result<(), Box<dyn Error>> {
    IoTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
    let directory = TestDirectory::new()?;
    let mut application = directory.application();

    application.update();
    application
        .world_mut()
        .resource_mut::<CounterSettings>()
        .count = 17;
    SaveSettingsDeferred(Duration::from_secs(2)).apply(application.world_mut());

    application
        .world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(1));
    application.update();
    assert!(!directory.0.join("settings.toml").exists());

    application
        .world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(1));
    application.update();

    let settings: toml::Table = fs::read_to_string(directory.0.join("settings.toml"))?.parse()?;
    assert_eq!(settings["counter"]["count"].as_integer(), Some(17));
    assert!(!directory.0.join(APP_NAME).exists());

    Ok(())
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Result<Self, std::io::Error> {
        static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "bevy-settings-directory-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn application(&self) -> App {
        let mut application = App::new();
        application
            .register_type::<CounterSettings>()
            .register_type::<DisplaySettings>()
            .register_type::<AudioSettings>()
            .insert_resource(Time::<Real>::default())
            .insert_resource(SettingsDirectory(self.0.clone()))
            .add_plugins(SettingsPlugin {
                app_name: APP_NAME.into(),
            });
        application
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Resource, SettingsGroup, Reflect, Default)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "counter")]
struct CounterSettings {
    count: u32,
}

#[derive(Resource, SettingsGroup, Reflect, Default)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "display")]
struct DisplaySettings {
    enabled: bool,
}

#[derive(Resource, SettingsGroup, Reflect, Default)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "audio", file = "audio")]
struct AudioSettings {
    volume: f32,
}
