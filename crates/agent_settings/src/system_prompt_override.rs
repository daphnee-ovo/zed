//! User-global native Agent system prompt override support.

use std::{sync::Arc, time::Duration};

use anyhow::Context as _;
use fs::{Fs, PathEventKind};
use futures::StreamExt as _;
use gpui::{App, BorrowAppContext, Global, SharedString, Task};
use handlebars::Handlebars;

#[derive(Debug, Default, Clone)]
pub enum SystemPromptOverrideState {
    #[default]
    Loading,
    Empty,
    Loaded(SharedString),
    Error(SharedString),
}

impl SystemPromptOverrideState {
    pub fn source(&self) -> Option<&SharedString> {
        match self {
            Self::Loaded(source) => Some(source),
            Self::Loading | Self::Empty | Self::Error(_) => None,
        }
    }

    pub fn error(&self) -> Option<&SharedString> {
        match self {
            Self::Error(message) => Some(message),
            Self::Loading | Self::Empty | Self::Loaded(_) => None,
        }
    }
}

pub struct SystemPromptOverride {
    state: SystemPromptOverrideState,
    _watcher: Task<()>,
}

impl Global for SystemPromptOverride {}

impl SystemPromptOverride {
    pub fn global(cx: &App) -> Option<&Self> {
        cx.try_global::<Self>()
    }

    pub fn state(&self) -> &SystemPromptOverrideState {
        &self.state
    }

    pub fn source(&self) -> Option<&SharedString> {
        self.state.source()
    }

    pub fn error(&self) -> Option<&SharedString> {
        self.state.error()
    }
}

pub fn init(
    fs: Arc<dyn Fs>,
    cx: &mut App,
    on_change: impl Fn(&SystemPromptOverrideState, &mut App) + 'static,
) {
    let watcher = spawn_watcher(fs, cx, on_change);
    cx.set_global(SystemPromptOverride {
        state: SystemPromptOverrideState::Loading,
        _watcher: watcher,
    });
}

fn spawn_watcher(
    fs: Arc<dyn Fs>,
    cx: &mut App,
    on_change: impl Fn(&SystemPromptOverrideState, &mut App) + 'static,
) -> Task<()> {
    let path = paths::system_prompt_file().clone();

    cx.spawn(async move |cx| {
        let Some(config_dir) = path.parent() else {
            publish_state(
                SystemPromptOverrideState::Error(
                    format!("{} has no parent directory", path.display()).into(),
                ),
                &on_change,
                cx,
            );
            return;
        };
        let config_dir = config_dir.to_path_buf();
        let (mut events, _watcher) = fs.watch(&config_dir, Duration::from_millis(100)).await;

        publish_state(load_state(fs.as_ref(), &path).await, &on_change, cx);

        while let Some(events) = events.next().await {
            let should_reload = events.iter().any(|event| {
                event.path == path
                    || (event.path == config_dir
                        && matches!(event.kind, Some(PathEventKind::Rescan)))
            });
            if should_reload {
                publish_state(load_state(fs.as_ref(), &path).await, &on_change, cx);
            }
        }
    })
}

fn publish_state(
    state: SystemPromptOverrideState,
    on_change: &impl Fn(&SystemPromptOverrideState, &mut App),
    cx: &mut gpui::AsyncApp,
) {
    cx.update(|cx| {
        cx.update_global::<SystemPromptOverride, _>(|prompt, _| {
            prompt.state = state.clone();
        });
        on_change(&state, cx);
    });
}

async fn load_state(fs: &dyn Fs, path: &std::path::Path) -> SystemPromptOverrideState {
    match fs.load(path).await {
        Ok(source) if source.trim().is_empty() => SystemPromptOverrideState::Empty,
        Ok(source) => match validate_template(&source) {
            Ok(()) => SystemPromptOverrideState::Loaded(source.into()),
            Err(error) => SystemPromptOverrideState::Error(error.to_string().into()),
        },
        Err(error) => {
            if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
                if io_error.kind() == std::io::ErrorKind::NotFound {
                    return SystemPromptOverrideState::Empty;
                }
            }
            SystemPromptOverrideState::Error(format!("{error:#}").into())
        }
    }
}

fn validate_template(source: &str) -> anyhow::Result<()> {
    let mut handlebars = Handlebars::new();
    handlebars
        .register_template_string("system_prompt", source)
        .context("failed to parse Handlebars template")
}
