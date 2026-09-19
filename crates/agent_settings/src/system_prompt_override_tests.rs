use std::{cell::RefCell, rc::Rc, sync::Arc};

use fs::{FakeFs, Fs as _};
use gpui::TestAppContext;

use crate::{SystemPromptOverride, SystemPromptOverrideState, init_system_prompt_override};

async fn init_test(
    cx: &mut TestAppContext,
) -> (Arc<FakeFs>, Rc<RefCell<Vec<SystemPromptOverrideState>>>) {
    cx.executor().allow_parking();
    let fs = FakeFs::new(cx.executor());
    let config_dir = paths::system_prompt_file()
        .parent()
        .expect("system prompt path should have a parent")
        .to_path_buf();
    fs.create_dir(&config_dir).await.expect("create config dir");

    let history = Rc::new(RefCell::new(Vec::new()));
    let history_clone = history.clone();
    cx.update(|cx| {
        init_system_prompt_override(fs.clone(), cx, move |state, _cx| {
            history_clone.borrow_mut().push(state.clone());
        });
    });
    (fs, history)
}

#[gpui::test]
async fn loads_valid_template(cx: &mut TestAppContext) {
    let (fs, history) = init_test(cx).await;
    fs.insert_file(
        paths::system_prompt_file(),
        b"Custom prompt for {{model_name}}".to_vec(),
    )
    .await;

    cx.run_until_parked();

    cx.update(|cx| {
        assert_eq!(
            SystemPromptOverride::global(cx)
                .and_then(|prompt| prompt.source())
                .map(AsRef::as_ref),
            Some("Custom prompt for {{model_name}}"),
        );
    });
    assert!(matches!(
        history.borrow().last(),
        Some(SystemPromptOverrideState::Loaded(_))
    ));
}

#[gpui::test]
async fn rejects_invalid_template(cx: &mut TestAppContext) {
    let (fs, history) = init_test(cx).await;
    fs.insert_file(paths::system_prompt_file(), b"{{#if model_name}}".to_vec())
        .await;

    cx.run_until_parked();

    cx.update(|cx| {
        let error = SystemPromptOverride::global(cx)
            .and_then(|prompt| prompt.error())
            .expect("invalid template should have an error");
        assert!(error.contains("failed to parse Handlebars template"));
    });
    assert!(matches!(
        history.borrow().last(),
        Some(SystemPromptOverrideState::Error(_))
    ));
}

#[gpui::test]
async fn removing_template_restores_built_in_state(cx: &mut TestAppContext) {
    let (fs, history) = init_test(cx).await;
    fs.insert_file(paths::system_prompt_file(), b"custom".to_vec())
        .await;
    cx.run_until_parked();

    fs.remove_file(paths::system_prompt_file(), Default::default())
        .await
        .expect("remove template");
    cx.run_until_parked();

    cx.update(|cx| {
        assert!(matches!(
            SystemPromptOverride::global(cx).map(|prompt| prompt.state()),
            Some(SystemPromptOverrideState::Empty)
        ));
    });
    assert!(matches!(
        history.borrow().last(),
        Some(SystemPromptOverrideState::Empty)
    ));
}
