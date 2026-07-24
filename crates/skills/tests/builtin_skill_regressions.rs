use blockcell_core::Error;
use blockcell_skills::dispatcher::SkillDispatcher;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn skill_script(skill: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join("skills").join(skill).join("SKILL.rhai")).unwrap()
}

fn context(value: Value) -> HashMap<String, Value> {
    HashMap::from([
        ("ctx".to_string(), value.clone()),
        ("context".to_string(), value),
    ])
}

#[test]
fn app_control_routes_open_file_to_the_open_dialog() {
    let calls = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let recorded = calls.clone();
    SkillDispatcher::new()
        .execute_sync(
            &skill_script("app_control"),
            "打开文件",
            context(json!({"app":"Windsurf", "path":"/tmp/report.md"})),
            move |name, params| {
                recorded.lock().unwrap().push((name.to_string(), params));
                Ok(json!({"status":"ok"}))
            },
        )
        .unwrap();
    let calls = calls.lock().unwrap();
    assert!(calls.iter().any(|(name, params)| {
        name == "app_control" && params["action"] == "press_key" && params["text"] == "cmd+o"
    }));
}

#[test]
fn app_control_routes_menu_intent_to_click_menu() {
    let calls = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let recorded = calls.clone();
    SkillDispatcher::new()
        .execute_sync(
            &skill_script("app_control"),
            "打开文件菜单",
            context(json!({"app":"Windsurf", "menu":"File > Open"})),
            move |name, params| {
                recorded.lock().unwrap().push((name.to_string(), params));
                Ok(json!({"status":"ok"}))
            },
        )
        .unwrap();
    assert!(calls.lock().unwrap().iter().any(|(name, params)| {
        name == "app_control" && params["action"] == "click_menu" && params["text"] == "File > Open"
    }));
}

#[test]
fn app_control_does_not_report_success_when_screenshot_fallback_fails() {
    let result = SkillDispatcher::new()
        .execute_sync(
            &skill_script("app_control"),
            "截图 Windsurf",
            context(json!({"app":"Windsurf"})),
            |_, _| Err(Error::Tool("capture failed".into())),
        )
        .unwrap();
    assert_eq!(result.output["success"], false);
}

#[test]
fn camera_failure_falls_back_to_screen_capture_instead_of_retrying_camera() {
    let calls = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let recorded = calls.clone();
    let result = SkillDispatcher::new()
        .execute_sync(
            &skill_script("camera"),
            "拍照",
            context(json!({"message":"拍照"})),
            move |name, params| {
                recorded
                    .lock()
                    .unwrap()
                    .push((name.to_string(), params.clone()));
                if name == "camera_capture" && params["action"] == "capture" {
                    Err(Error::Tool("camera unavailable".into()))
                } else {
                    Ok(json!({"status":"ok", "path":"media/fallback.png"}))
                }
            },
        )
        .unwrap();
    let calls = calls.lock().unwrap();
    assert!(calls.iter().any(|(name, _)| name == "exec"));
    assert_eq!(result.output["degraded"], true);
}

fn run_ai_news(chat_id: &str) -> (Vec<(String, Value)>, Value) {
    let calls = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let recorded = calls.clone();
    let result = SkillDispatcher::new()
        .execute_sync(
            &skill_script("ai_news"),
            "只看英文 AI 新闻",
            context(json!({"user_input":"只看英文 AI 新闻", "chat_id":chat_id})),
            move |name, params| {
                recorded
                    .lock()
                    .unwrap()
                    .push((name.to_string(), params.clone()));
                match params["action"].as_str() {
                    Some("get_content") => Ok(json!({"content":"x".repeat(3500)})),
                    _ => Ok(json!({"status":"ok"})),
                }
            },
        )
        .unwrap();
    let calls = calls.lock().unwrap().clone();
    (calls, result.output)
}

#[test]
fn ai_news_honors_an_explicit_english_source_request() {
    let (calls, _) = run_ai_news("chat-a");
    let first_navigation = calls
        .iter()
        .find(|(_, params)| params["action"] == "navigate")
        .unwrap();
    assert!(first_navigation.1["url"]
        .as_str()
        .unwrap()
        .contains("techcrunch.com"));
}

#[test]
fn ai_news_uses_a_separate_browser_session_for_each_chat() {
    let (first, _) = run_ai_news("chat-a");
    let (second, _) = run_ai_news("chat-b");
    let session = |calls: &[(String, Value)]| {
        calls
            .iter()
            .find_map(|(_, params)| params.get("session").and_then(Value::as_str))
            .unwrap()
            .to_string()
    };
    assert_ne!(session(&first), session(&second));
}
