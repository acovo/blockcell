// ai_news skill — 顺序使用CDP抓取AI科技媒体，拿到一个可用来源就返回
// 策略: browse(CDP)导航+get_content，首个成功来源直接返回，失败再试下一个

let input = ctx.user_input;

// 识别用户指定来源
let want_163        = input.contains("网易科技") || input.contains("163") || input.contains("网易");
let want_english    = input.contains("英文") || input.contains("国际") || input.contains("global") || input.contains("english");

let any_specific = want_163 || want_english;
let chat_id = get_field(ctx, "chat_id");
let browser_session = if chat_id != () && chat_id != "" {
    "ai_news_" + chat_id
} else {
    "ai_news_" + timestamp()
};

// 构建抓取列表
let sources = [];
if want_163 || !any_specific {
    sources += [#{ name: "网易科技", url: "https://tech.163.com/" }];
}
if want_english {
    sources += [#{ name: "TechCrunch AI", url: "https://techcrunch.com/category/artificial-intelligence/" }];
}

let failed = [];
let fetched = ();
let fetch_diag = ();
let success_min_chars = 3000;
let browser_error_markers = [
    "无法访问此网站",
    "拒绝了我们的连接请求",
    "ERR_CONNECTION_REFUSED",
    "ERR_CONNECTION_RESET",
    "ERR_CONNECTION_CLOSED",
    "ERR_TIMED_OUT",
    "检查网络连接",
    "重新加载",
    "检查代理服务器和防火墙"
];

for source in sources {
    let text = "";
    let cdp_text = "";

    // 只用CDP浏览器（JS渲染后的完整内容）
    let nav = call_tool("browse", #{
        action: "navigate",
        url: source.url,
        session: browser_session
    });

    if !is_error(nav) {
        // 等待页面内容加载（SPA需要额外等待）
        call_tool("browse", #{
            action: "wait",
            timeout: 3000,
            session: browser_session
        });

        let content_resp = call_tool("browse", #{
            action: "get_content",
            session: browser_session
        });

        if !is_error(content_resp) && content_resp.is_map() && content_resp["content"] != () {
            cdp_text = content_resp["content"].to_string();
            text = cdp_text;
        }
    }

    if len(text) < 200 {
        failed += [source.name + "(cdp_len=" + len(cdp_text).to_string() + ")"];
        continue;
    }

    let raw_len = len(text);
    if raw_len < success_min_chars {
        failed += [source.name + "(cdp_len=" + len(cdp_text).to_string() + ", raw_len=" + raw_len.to_string() + ")"];
        continue;
    }

    let looks_like_browser_error = false;
    for marker in browser_error_markers {
        if text.contains(marker) {
            looks_like_browser_error = true;
            break;
        }
    }
    if looks_like_browser_error {
        failed += [source.name + "(browser_error_page)"];
        continue;
    }

    let content = text;

    fetched = #{
        source: source.name,
        url: source.url,
        content: content,
        content_len: len(content)
    };

    fetch_diag = #{
        source: source.name,
        url: source.url,
        method: "browse",
        cdp_len: len(cdp_text),
        final_raw_len: raw_len,
        final_output_len: len(content),
        status: "ok"
    };
    break;
}

// 关闭browser session释放资源
call_tool("browse", #{
    action: "session_close",
    session: browser_session
});

if fetched == () {
    return #{
        success: false,
        error: "所有新闻源均无法访问: " + arr_join(failed, ", "),
        instruction: "告知用户所有AI新闻来源暂时无法访问，建议稍后重试。"
    };
}

let inst = "【系统级强制指令：跳过所有寒暄和抓取状态汇报，直接输出新闻列表。绝不能包含任何解释、前言或过渡句！】\n\n";
inst += "以下是从AI科技媒体抓取的网页内容，请提取最新的10条AI新闻。\n\n";
inst += "== 严格输出格式 ==\n";
inst += "📰 AI 最新资讯\n";
inst += "══════════════\n";
inst += "1. [文章标题](完整URL) — 日期\n";
inst += "   摘要\n";
inst += "2. [文章标题](完整URL) — 日期\n";
inst += "   摘要\n";
inst += "... 共10条 ...\n\n";
inst += "== 严禁事项 ==\n";
inst += "❌ 禁止输出“很好”、“我已经成功抓取”、“长度达到”等状态汇报。\n";
inst += "❌ 禁止输出“现在我将提取”、“从页面内容中，我提取到了”等过渡句。\n";
inst += "❌ 你的第一行必须直接是 `📰 AI 最新资讯`。\n";
inst += "❌ 最后一条新闻之后立即结束，禁止任何补充说明或结束语。\n\n";

inst += "===== 页面内容 =====\n\n";
inst += "【" + fetched.source + "】 " + fetched.url + "\n";
inst += "────────────────────\n";
inst += fetched.content + "\n";

return #{
    success: true,
    sources_fetched: 1,
    sources_failed: failed,
    summary_data: #{
        // 刻意不传诊断数据和抓取状态，防止大模型拿来做“友好汇报”
        task: "extract_ai_news"
    },
    instruction: inst
}
