use actix_web::{web, App, HttpServer, HttpResponse, Error};
use actix_multipart::Multipart;
use actix_files::NamedFile;
use futures::{StreamExt, TryStreamExt};
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::Write;
use std::time::{SystemTime, Duration};
use chrono::{DateTime, Utc};
use tokio::time::interval;
use std::sync::Arc;
use serde::Serialize;

const MAX_FILE_SIZE: usize = 20 * 1024 * 1024; // 20MB
const FILE_EXPIRE_HOURS: u64 = 24;

#[derive(Serialize)]
pub struct UploadResponse {
    url: String,
    filename: String,
    size: u64,
}

pub struct FileServer {
    storage_path: PathBuf,
    base_url: String,
}

impl FileServer {
    pub fn new(storage_path: impl Into<PathBuf>, host: &str, port: u16) -> Self {
        let storage_path = storage_path.into();
        fs::create_dir_all(&storage_path).unwrap();
        
        Self {
            storage_path,
            base_url: format!("http://{}:{}", host, port),
        }
    }
    
    pub async fn start(self, port: u16) -> std::io::Result<()> {
        let storage_path = Arc::new(self.storage_path);
        let base_url = Arc::new(self.base_url);
        
        // 启动文件清理任务
        let cleanup_path = storage_path.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;
                Self::cleanup_old_files(&cleanup_path).await;
            }
        });
        
        println!("Starting file server on port {}", port);
        HttpServer::new(move || {
            let storage = storage_path.clone();
            let url = base_url.clone();
            
            App::new()
                .app_data(web::Data::new(storage))
                .app_data(web::Data::new(url))
                .service(web::resource("/upload").route(web::post().to(upload_handler)))
                .service(web::resource("/files/{filename}").route(web::get().to(download_handler)))
        })
        .bind(("0.0.0.0", port))?
        .run()
        .await
    }
    
    async fn cleanup_old_files(storage_path: &Path) {
        let cutoff = SystemTime::now() - Duration::from_secs(FILE_EXPIRE_HOURS * 3600);
        if let Ok(entries) = fs::read_dir(storage_path) {
            for entry in entries.filter_map(Result::ok) {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }
}

async fn upload_handler(
    mut payload: Multipart,
    storage: web::Data<PathBuf>,
    base_url: web::Data<String>,
) -> Result<HttpResponse, Error> {
    let mut total_size: usize = 0;
    
    if let Some(mut field) = payload.try_next().await? {
        let filename = field.content_disposition()
            .get_filename()
            .unwrap_or("unknown")
            .to_string();
            
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let unique_filename = format!("{}_{}", timestamp, filename);
        let filepath = storage.join(&unique_filename);
        
        // 创建文件
        let mut f = File::create(&filepath)?;
        
        // 写入文件内容
        while let Some(chunk) = field.try_next().await? {
            total_size += chunk.len();
            if total_size > MAX_FILE_SIZE {
                fs::remove_file(&filepath)?;
                return Ok(HttpResponse::BadRequest().body("File too large"));
            }
            f.write_all(&chunk)?;
        }
        
        // 获取文件大小
        let size = fs::metadata(&filepath)?.len();
        
        Ok(HttpResponse::Ok().json(UploadResponse {
            url: format!("{}/files/{}", base_url, unique_filename),
            filename,
            size,
        }))
    } else {
        Ok(HttpResponse::BadRequest().body("No file provided"))
    }
}

async fn download_handler(
    filename: web::Path<String>,
    storage: web::Data<PathBuf>,
) -> Result<NamedFile, Error> {
    let filepath = storage.join(filename.as_ref());
    Ok(NamedFile::open(filepath)?)
}
