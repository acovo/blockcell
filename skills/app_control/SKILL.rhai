// 通用应用控制技能编排脚本
// 根据用户意图执行不同的应用控制流程

let user_input = if ctx != () {
    let value = get_field(ctx, "user_input");
    if value != () && value != "" { value } else { user_input }
} else { user_input };
let target_app = if ctx != () {
    let value = get_field(ctx, "app");
    if value != () && value != "" { value } else { "Windsurf" }
} else { "Windsurf" };
let target_path = if ctx != () { get_field(ctx, "path") } else { () };
let menu_path = if ctx != () { get_field(ctx, "menu") } else { () };

// 判断用户意图
let intent = "screenshot"; // 默认截图

if user_input.contains("列出") || user_input.contains("什么应用") || user_input.contains("list") {
    intent = "list_apps";
} else if user_input.contains("菜单") || user_input.contains("menu") {
    intent = "menu";
} else if user_input.contains("打开文件") || user_input.contains("open file") {
    intent = "open_file";
} else if user_input.contains("截图") || user_input.contains("看看") || user_input.contains("screenshot") {
    intent = "screenshot";
} else if user_input.contains("UI") || user_input.contains("界面") || user_input.contains("read") {
    intent = "read_ui";
}

if intent == "list_apps" {
    let result = call_tool("app_control", #{
        "action": "list_apps"
    });
    if is_error(result) {
        log_warn("列出应用失败: " + to_json(result));
        set_output("❌ 无法列出运行中的应用");
    } else {
        set_output_json(result);
    }
} else if intent == "screenshot" {
    // 截图流程
    let result = call_tool("app_control", #{
        "action": "screenshot",
        "app": target_app
    });
    if is_error(result) {
        log_warn("截图失败，尝试降级: " + to_json(result));
        // 降级: 全屏截图
        let fallback_path = "media/app_screenshot_" + timestamp() + ".png";
        let fallback = call_tool("exec", #{
            "command": "mkdir -p media && screencapture -x " + fallback_path
        });
        if is_error(fallback) || (get_field(fallback, "exit_code") != () && get_field(fallback, "exit_code") != 0) {
            set_output(#{ success: false, error: "窗口截图和全屏截图均失败", original_error: result });
        } else {
            set_output(#{ success: true, degraded: true, screenshot_path: fallback_path });
        }
    } else {
        // 同时读取 UI 树
        let ui = call_tool("app_control", #{
            "action": "read_ui",
            "app": target_app,
            "depth": 2
        });
        let output = #{
            "screenshot": result,
            "ui_tree": ui
        };
        set_output_json(output);
    }
} else if intent == "open_file" {
    let activated = call_tool("app_control", #{ action: "activate", app: target_app });
    let opened = if is_error(activated) { activated } else {
        call_tool("app_control", #{ action: "press_key", app: target_app, text: "cmd+o" })
    };
    if is_error(opened) {
        set_output(#{ success: false, error: get_field(opened, "error") });
    } else if target_path != () && target_path != "" {
        let typed = call_tool("app_control", #{ action: "type", app: target_app, text: target_path });
        let confirmed = if is_error(typed) { typed } else {
            call_tool("app_control", #{ action: "press_key", app: target_app, text: "return" })
        };
        set_output_json(confirmed);
    } else {
        set_output_json(opened);
    }
} else if intent == "menu" {
    if menu_path == () || menu_path == "" {
        set_output(#{ success: false, error: "缺少菜单路径" });
    } else {
        let result = call_tool("app_control", #{ action: "click_menu", app: target_app, text: menu_path });
        set_output_json(result);
    }
} else if intent == "read_ui" {
    let result = call_tool("app_control", #{
        "action": "read_ui",
        "app": target_app,
        "depth": 3
    });
    if is_error(result) {
        // 降级: 减少深度
        log_warn("UI 读取失败，降低深度重试");
        let retry = call_tool("app_control", #{
            "action": "read_ui",
            "app": target_app,
            "depth": 1
        });
        set_output_json(retry);
    } else {
        set_output_json(result);
    }
} else {
    // 默认: 截图
    let result = call_tool("app_control", #{
        "action": "screenshot",
        "app": target_app
    });
    set_output_json(result);
}
