use prompt_store::ProjectContext;

use crate::{SystemPromptTemplate, Templates};

fn context<'a>(project: &'a ProjectContext) -> SystemPromptTemplate<'a> {
    SystemPromptTemplate {
        project,
        available_tools: vec!["terminal".into()],
        model_name: Some("test-model".to_string()),
        date: "2026-09-18".to_string(),
        user_agents_md: Some("personal instructions".into()),
        sandboxing: true,
        is_linux: false,
        is_windows: false,
    }
}

#[test]
fn renders_system_prompt_override_with_runtime_context() {
    let project = ProjectContext::default();
    let rendered = Templates::new()
        .render_system_prompt(
            &context(&project),
            Some("Override for {{model_name}} using {{#each available_tools}}{{this}}{{/each}}"),
        )
        .expect("render system prompt override");

    assert_eq!(rendered, "Override for test-model using terminal");
}

#[test]
fn rejects_system_prompt_override_with_unknown_context() {
    let project = ProjectContext::default();
    let error = Templates::new()
        .render_system_prompt(&context(&project), Some("{{unknown_context}}"))
        .expect_err("strict rendering should reject unknown context");

    assert!(error.to_string().contains("unknown_context"));
}
