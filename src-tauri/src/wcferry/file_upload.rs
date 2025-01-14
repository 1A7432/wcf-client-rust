use std::path::Path;
use reqwest::multipart::{Form, Part};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub async fn upload_file(file_path: &Path, file_server_url: &str) -> Result<(String, String, u64), Box<dyn std::error::Error>> {
    // 读取文件
    let mut file = File::open(file_path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    
    // 获取文件名和大小
    let file_name = file_path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_size = buffer.len() as u64;
    
    // 创建 multipart form
    let form = Form::new()
        .part("file", Part::bytes(buffer)
            .file_name(file_name.clone())
            .mime_str("application/octet-stream")?);
    
    // 发送请求
    let client = reqwest::Client::new();
    let response = client.post(&format!("{}/upload", file_server_url))
        .multipart(form)
        .send()
        .await?;
    
    // 解析响应
    let result: serde_json::Value = response.json().await?;
    let file_url = result["url"].as_str()
        .ok_or("Missing file URL in response")?
        .to_string();
        
    Ok((file_url, file_name, file_size))
}

pub async fn upload_image(file_path: &Path, file_server_url: &str) -> Result<(String, String, String, u64), Box<dyn std::error::Error>> {
    let (file_url, file_name, file_size) = upload_file(file_path, file_server_url).await?;
    
    // 对于图片，我们使用相同的URL作为缩略图
    let thumb_url = file_url.clone();
    
    Ok((file_url, thumb_url, file_name, file_size))
}
