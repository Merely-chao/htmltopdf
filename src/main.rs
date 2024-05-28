use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
    time::Duration,
};

use headless_chrome::{
    protocol::cdp::{
        Fetch::{RequestPattern, RequestStage},
        Target::CreateTarget,
    },
    types::PrintToPdfOptions,
    Browser, LaunchOptionsBuilder,
};
use log::info;
use ntex::{
    rt::spawn,
    web::{
        self, get,
        types::{Query, State},
    },
};
use serde::Deserialize;

#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

static G_COUNT: u8 = 120;

#[ntex::main]
async fn main() -> Result<(), Box<dyn Error>> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

   info!("启动");
    let  launch_options = LaunchOptionsBuilder::default()
    .sandbox(false)
    .headless(true)
    // .args(vec![OsStr::new("--no-startup-window"),OsStr::new("--no-pdf-header-footer")])
    .port(Some(8001))
    .idle_browser_timeout(Duration::MAX).build()?;

    info!("打开浏览器");
    let browser: Browser = Browser::new(launch_options)?;

    let use_state = Arc::new(Mutex::new(G_COUNT));

    let ref_browser = browser.clone();
    let ref_state = use_state.clone();

    spawn(async move {
        loop {

            //监听无法印报告请求，清理可能出现的tabs
            loop {
                ntex::time::sleep(Duration::from_secs(30)).await;
                match ref_state.lock() {
                    Ok(mut state) => {

                        info!("当前state{}",state);
                        if *state == 0 {
                            *state = 10;
                            break;
                        }
                        *state -= 1;
                    },
                    Err(e) => {
                        info!("获取状态值失败:{:?}", e);
                        continue;
                    }
                };
            }
            info!("开始清理tab");
            let _b = match ref_browser.get_tabs().lock() {
                Ok(v) => v,
                Err(e) => {
                    info!("清理tab失败，错误信息:{:?}", e);
                    continue;
                }
            };
            info!("清理前tab数量:{}", _b.len());
            _b.iter().for_each(|tab| {
                let _ = tab.close(true);
            });
            info!("清理后tab数量:{}", _b.len());
        }
    });

    info!("开始监听");

    web::HttpServer::new(move || {
        web::App::new()
            .state(browser.clone())
            .state(use_state.clone())
            .route("/pdf", get().to(pdf))
            .route("/img", get().to(img))
    })
    .bind(("0.0.0.0", 3000))?
    .run()
    .await?;

    Ok(())
}

pub async fn pdf(
    params: Query<HashMap<String, String>>,
    browser: State<Browser>,
    count_state: State<Arc<Mutex<u8>>>,
) -> Result<impl web::Responder, web::Error> {
    let mut state = match count_state.lock() {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "获取state失败，错误信息:{}",
                e.to_string()
            ))
            .into())
        }
    };
    *state = G_COUNT;

    drop(state);

    info!("进入");
    let url = match params.get("url") {
        Some(v) => v,
        None => return Err(web::error::ErrorInternalServerError("url是必须的:").into()),
    };
    ntex::time::sleep(Duration::from_nanos(1)).await;

    let tab = match browser.new_tab() {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "创建新tab失败，错误信息:{}",
                e.to_string()
            ))
            .into())
        }
    };

    let patterns: Vec<RequestPattern> = vec![
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

    ntex::time::sleep(Duration::from_nanos(1)).await;
    let _ = match tab.navigate_to(&url) {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "打开链接出错：{}",
                e.to_string()
            ))
            .into())
        }
    };

    ntex::time::sleep(Duration::from_nanos(1)).await;

    let tab = match tab.wait_until_navigated() {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "等待页面加载完成出错{}",
                e.to_string()
            ))
            .into())
        }
    };
    info!("等待界面加载结束");

    let mut p = PrintToPdfOptions::default();
    p.prefer_css_page_size = Some(true);
    ntex::time::sleep(Duration::from_nanos(1)).await;
    let data = match tab.print_to_pdf(Some(p)) {
        Ok(v) => v,
        Err(e) => {
            return Err(
                web::error::ErrorInternalServerError(format!("转pdf报错{}", e.to_string())).into(),
            )
        }
    };

    let _ = tab.close(true);

    Ok(web::HttpResponse::Ok()
        .content_type("application/pdf")
        .body(data))
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
    params: Query<ImgParams>,
    browser: State<Browser>,
    count_state: State<Arc<Mutex<u8>>>,
) -> Result<impl web::Responder, web::Error> {
    let mut state = match count_state.lock() {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "获取state失败，错误信息:{}",
                e.to_string()
            ))
            .into())
        }
    };
    *state = G_COUNT;

    drop(state);

    let target_options = CreateTarget {
        url: "about:blank".to_string(),
        width: params.width,
        height: params.height,
        browser_context_id: None,
        enable_begin_frame_control: None,
        new_window: None,
        background: params.background,
    };
    ntex::time::sleep(Duration::from_nanos(1)).await;
    let tab = match browser.new_tab_with_options(target_options) {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "创建新tab失败，错误信息：{}",
                e.to_string()
            ))
            .into())
        }
    };
    ntex::time::sleep(Duration::from_nanos(1)).await;
    let _ = match tab.navigate_to(&params.url) {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "打开链接出错：{}",
                e.to_string()
            ))
            .into())
        }
    };
    ntex::time::sleep(Duration::from_nanos(1)).await;
    let tab = match tab.wait_until_navigated() {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "等待页面加载完成出错{}",
                e.to_string()
            ))
            .into())
        }
    };
    ntex::time::sleep(Duration::from_nanos(1)).await;
    let data = match tab.capture_screenshot(params.format.clone(), None, None, true) {
        Ok(v) => v,
        Err(e) => {
            return Err(web::error::ErrorInternalServerError(format!(
                "截图出现错误：{}",
                e.to_string()
            ))
            .into())
        }
    };

    let _ = tab.close_with_unload();

    Ok(web::HttpResponse::Ok().body(data))
}
