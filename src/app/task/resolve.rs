use std::{
    path::Path,
    pin::{Pin, pin},
};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use reqwest::{ClientBuilder, header};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncWriteExt, BufWriter},
    sync::oneshot,
};
use url::Url;

use crate::app::{
    send::DownloadRequest,
    task::{Task, TaskInner, TaskResult},
};

pub async fn handle_task(task: Task) {
    match task.request {
        DownloadRequest::Normal { url } => {
            handle_normal_download(task.inner, url, task.reporter).await;
        }
    }
}

async fn handle_normal_download(
    task: TaskInner,
    url_str: String,
    reporter: oneshot::Sender<TaskResult>,
) {
    let url = match get_proper_url(&url_str) {
        Ok(u) => u,
        Err(e) => {
            // 解析失败时，发送一个未知URL的结果
            reporter
                .send(TaskResult::new_unknown_url(e.to_string()))
                .unwrap();
            return;
        }
    };

    // FIXME: 使用配置的路径
    let base_dirs = directories::BaseDirs::new().unwrap(); // 临时先用一个路径
    let download_dir = base_dirs.home_dir().join("Downloads");
    let _ = std::fs::DirBuilder::new().create(&download_dir);

    let client = match ClientBuilder::new().build() {
        Ok(c) => c,
        Err(e) => {
            reporter
                .send(TaskResult::new_failed_to_connection(e.to_string()))
                .unwrap();
            return;
        }
    };

    let stream = match get_download_head(&task, url, &client, &download_dir).await {
        Ok(s) => s,
        Err(e) => {
            reporter
                .send(TaskResult::new_failed_to_connection(e.to_string()))
                .unwrap();
            return;
        }
    };
    let mut stream = pin!(stream);

    let filepath = { task.state.lock().unwrap().filepath.clone() };
    let mut file = match create_download_file(&filepath).await {
        Ok(f) => f,
        Err(e) => {
            reporter
                .send(TaskResult::new_failed_to_create_file(e.to_string()))
                .unwrap();
            return;
        }
    };

    if let Some(reporter) =
        download_stream_to_file(&task, Pin::new(&mut stream), &mut file, reporter).await
    {
        reporter.send(TaskResult::new_finished()).unwrap();
    }
}

fn get_proper_url(url_str: &str) -> anyhow::Result<Url> {
    match Url::parse(url_str) {
        Ok(url) => Ok(url),
        Err(e) => {
            if e == url::ParseError::RelativeUrlWithoutBase {
                let fixed_url = format!("http://{}", url_str);
                match Url::parse(&fixed_url) {
                    Ok(url) => Ok(url),
                    Err(e) => Err(anyhow::anyhow!("Failed to parse URL: {}", e)),
                }
            } else {
                Err(anyhow::anyhow!("Failed to parse URL: {}", e))
            }
        }
    }
}

/// FILENAME 不是 PATH !!!
///
/// 规则为filename.ext -> filename(1).ext -> filename(2).ext ...
fn get_filename_no_duplicate(dir: &Path, filename: &str) -> String {
    let mut path = dir.join(filename);
    if !path.exists() {
        return filename.to_string();
    }

    let mut count: u64 = 1;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    loop {
        let new_filename = if let Some(ext) = &extension {
            format!("{}({}).{}", stem, count, ext)
        } else {
            format!("{}({})", stem, count)
        };
        path = dir.join(&new_filename);
        if !path.exists() {
            return new_filename;
        }
        count += 1;
    }
}

async fn get_download_head(
    task: &TaskInner,
    url: Url,
    client: &reqwest::Client,
    download_dir: &Path,
) -> anyhow::Result<impl Stream<Item = reqwest::Result<Bytes>>> {
    let response = client.get(url.clone()).send().await?;

    let head = response.headers();
    let content_length = head
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let accept_ranges = head
        .get(header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.eq_ignore_ascii_case("bytes"));
    let dest = {
        let opt_fname = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .and_then(|name| if name.is_empty() { None } else { Some(name) });
        let fname = opt_fname
            .or(response
                .url()
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .and_then(|name| if name.is_empty() { None } else { Some(name) }))
            .unwrap_or("tmp.bin");
        download_dir.join(get_filename_no_duplicate(download_dir, fname))
    };

    // TODO: handle Content-Disposition

    {
        let mut state = task.state.lock().unwrap();
        state.content_length = content_length;
        state.accept_ranges = accept_ranges;
        state.filepath = dest;
        state.url = Some(response.url().clone());
    }

    let stream = response.bytes_stream();

    Ok(stream)
}

async fn create_download_file(filepath: &Path) -> anyhow::Result<BufWriter<File>> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(filepath)
        .await?;
    Ok(BufWriter::new(file))
}

async fn download_stream_to_file(
    task: &TaskInner,
    mut stream: Pin<&mut impl Stream<Item = reqwest::Result<Bytes>>>,
    file: &mut BufWriter<File>,
    reporter: oneshot::Sender<TaskResult>,
) -> Option<oneshot::Sender<TaskResult>> {
    while let Some(chunk) = stream.next().await {
        let data = match chunk {
            Ok(d) => d,
            Err(e) => {
                reporter
                    .send(TaskResult::new_failed_to_download(e.to_string()))
                    .unwrap();
                return None;
            }
        };

        match file.write_all(&data).await {
            Ok(_) => {}
            Err(e) => {
                reporter
                    .send(TaskResult::new_failed_to_write(e.to_string()))
                    .unwrap();
                return None;
            }
        }

        {
            let mut state = task.state.lock().unwrap();
            state.downloaded += data.len() as u64;
        } // MutexGuard drop here
    }

    match file.flush().await {
        Ok(_) => {}
        Err(e) => {
            reporter
                .send(TaskResult::new_failed_to_write(e.to_string()))
                .unwrap();
            return None;
        }
    }
    Some(reporter)
}
