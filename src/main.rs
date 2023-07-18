use std::{collections::HashMap, error::Error, ffi::OsStr, net::SocketAddr, time::Duration};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Router,
};
use headless_chrome::{
    protocol::cdp::Target::CreateTarget, types::PrintToPdfOptions, Browser, LaunchOptions,
};
use serde::Deserialize;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut launch_options = LaunchOptions::default();
    launch_options.headless = true;

    launch_options.args.push(OsStr::new("--no-startup-window"));
    launch_options
        .args
        .push(OsStr::new("--no-pdf-header-footer"));
    launch_options.idle_browser_timeout = Duration::MAX; //永不断开
    let browser: Browser = Browser::new(launch_options)?;

    let app = Router::new()
        .route("/pdf", get(pdf))
        .route("/img", get(img))
        .with_state(browser);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

pub async fn pdf(
    Query(params): Query<HashMap<String, String>>,
    State(browser): State<Browser>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let url = match params.get("url") {
        Some(v) => v,
        None => return Err((StatusCode::UNPROCESSABLE_ENTITY, "url是必须的".to_string())),
    };
    let tab = match browser.new_tab() {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("创建新tab失败，错误信息：{}", e.to_string()),
            ))
        }
    };

    let _ = match tab.navigate_to(&url) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("打开链接出错：{}", e.to_string()),
            ))
        }
    };

    let tab = match tab.wait_until_navigated() {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("等待页面加载完成出错{}", e.to_string()),
            ))
        }
    };

    let mut p = PrintToPdfOptions::default();
    p.prefer_css_page_size = Some(true);

    let data = match tab.print_to_pdf(Some(p)) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("转pdf报错{}", e.to_string()),
            ))
        }
    };

    let _ = tab.close_with_unload();
    Ok(data)
}

#[derive(Deserialize)]
pub struct ImgParams {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
    format: headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption,
    background: Option<bool>,
}

pub async fn img(
    Query(params): Query<ImgParams>,
    State(browser): State<Browser>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let target_options = CreateTarget {
        url: "about:blank".to_string(),
        width: params.width,
        height: params.height,
        browser_context_id: None,
        enable_begin_frame_control: None,
        new_window: None,
        background: params.background,
    };

    let tab = match browser.new_tab_with_options(target_options) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("创建新tab失败，错误信息：{}", e.to_string()),
            ))
        }
    };

    let _ = match tab.navigate_to(&params.url) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("打开链接出错：{}", e.to_string()),
            ))
        }
    };

    let tab = match tab.wait_until_navigated() {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("等待页面加载完成出错{}", e.to_string()),
            ))
        }
    };

    let data = match  tab
    .capture_screenshot(params.format, None, None, true) {
        Ok(v)=>v,
        Err(e)=> return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("截图出现错误：{}", e.to_string()),
        ))
    }; 

    let _ = tab.close_with_unload();
    Ok(data)
}
