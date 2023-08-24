use std::{collections::HashMap, error::Error, ffi::OsStr, net::SocketAddr, time::Duration, sync::Arc, path::PathBuf};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Router,
};
use headless_chrome::{
    protocol::cdp::{Target::CreateTarget, Fetch::{RequestPattern, RequestStage, events::RequestPausedEvent}}, types::PrintToPdfOptions, Browser, LaunchOptions, browser::{transport::{Transport, SessionId}, tab::RequestPausedDecision}, LaunchOptionsBuilder
};
use log::info;
use serde::Deserialize;
use tokio::task::yield_now;

#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

   info!("启动");
    let  launch_options = LaunchOptionsBuilder::default()
    .sandbox(false)
    .headless(true)
    .args(vec![OsStr::new("--no-startup-window"),OsStr::new("--no-pdf-header-footer")])
    .port(Some(8001))
    .idle_browser_timeout(Duration::MAX).build()?;

    info!("打开浏览器");
    let browser: Browser = Browser::new(launch_options)?;

    let app = Router::new()
        .route("/pdf", get(pdf))
        .route("/img", get(img))
        .with_state(browser);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("开始监听");
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

    let patterns = vec![
        RequestPattern {
            url_pattern: None,
            resource_Type: None,
            request_stage: Some(RequestStage::Response),
        },
        RequestPattern {
            url_pattern: None,
            resource_Type: None,
            request_stage: Some(RequestStage::Request),
        },
    ];
    let _ = tab.enable_fetch(Some(&patterns), None);

    let _ = tab.enable_request_interception(Arc::new(
        move |_transport: Arc<Transport>, _session_id: SessionId, _intercepted: RequestPausedEvent| {
        // println!("进来了");
           
                RequestPausedDecision::Continue(None)
           
        },
    ));

    let _ = match tab.navigate_to(&url) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("打开链接出错：{}", e.to_string()),
            ))
        }
    };
    yield_now().await;
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
    yield_now().await;
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
