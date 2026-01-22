use std::{
    path::Path,
    pin::{Pin, pin},
    time::Instant,
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
    sender::DownloadRequest,
    task::{SignalHandler, Task, TaskCommand, TaskInner, TaskResult},
};

pub async fn handle_task(task: Task) {
    match task.request {
        DownloadRequest::Normal { url } => {
            handle_normal_download(task.inner, url, task.handler).await;
        }
        DownloadRequest::Resume => {
            handle_resume_download(task.inner, task.handler).await;
        }
    }
}

async fn handle_normal_download(task: TaskInner, url_str: String, handler: SignalHandler) {
    let url = match get_proper_url(&url_str) {
        Ok(u) => u,
        Err(e) => {
            // 解析失败时，发送一个未知URL的结果
            handler
                .reporter
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
            handler
                .reporter
                .send(TaskResult::new_failed_to_connection(e.to_string()))
                .unwrap();
            return;
        }
    };

    let stream = match get_download_head(&task, url, &client, &download_dir).await {
        Ok(s) => s,
        Err(e) => {
            handler
                .reporter
                .send(TaskResult::new_failed_to_connection(e.to_string()))
                .unwrap();
            return;
        }
    };
    let stream = pin!(stream);

    let filepath = { task.state.lock().unwrap().filepath.clone() };
    let mut file = match create_download_file(&filepath).await {
        Ok(f) => f,
        Err(e) => {
            handler
                .reporter
                .send(TaskResult::new_failed_to_create_file(e.to_string()))
                .unwrap();
            return;
        }
    };

    if let Some(handler) = download_stream_to_file(&task, stream, &mut file, handler).await {
        handler.reporter.send(TaskResult::new_finished()).unwrap();
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

        // FIXME:
        // 由于当前会首先搜索目录下是否有同名文件，然后创建文件，存在这样一种情况，
        // 同时下载两个同名文件时，两者同时检测到没有同名文件，然后创建了同名文件，导致冲突。
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

async fn flush_file_buffer(
    file: &mut BufWriter<File>,
    reporter: oneshot::Sender<TaskResult>,
) -> Option<oneshot::Sender<TaskResult>> {
    match file.flush().await {
        Ok(_) => Some(reporter),
        Err(e) => {
            reporter
                .send(TaskResult::new_failed_to_write(e.to_string()))
                .unwrap();
            None
        }
    }
}

async fn download_stream_to_file(
    task: &TaskInner,
    mut stream: Pin<&mut impl Stream<Item = reqwest::Result<Bytes>>>,
    file: &mut BufWriter<File>,
    handler: SignalHandler,
) -> Option<SignalHandler> {
    let reporter = handler.reporter;
    let mut cmd_recv = handler.receiver;
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

        if let Err(e) = file.write_all(&data).await {
            reporter
                .send(TaskResult::new_failed_to_write(e.to_string()))
                .unwrap();
            return None;
        }

        {
            let mut state = task.state.lock().unwrap();
            state.downloaded += data.len() as u64;
        } // MutexGuard drop here

        // 监听指令（非异步）
        match cmd_recv.try_recv() {
            Ok(signal) => match signal {
                TaskCommand::Stop => {
                    let reporter = flush_file_buffer(file, reporter).await?;
                    reporter.send(TaskResult::new_interrupted()).unwrap();
                    return None;
                }
                TaskCommand::Abort => {
                    let reporter = flush_file_buffer(file, reporter).await?;
                    reporter.send(TaskResult::new_abort()).unwrap();
                    return None;
                }
            },
            Err(oneshot::error::TryRecvError::Closed) => {
                let _ = reporter.send(TaskResult::new_unknown_error(String::from(
                    "Command channel closed unexpectedly",
                )));
                // Command channel关闭了，我们无法保证report是否能够发送成功
                // 因此我们发送失败后直接忽略
                return None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
        }
    }

    let reporter = flush_file_buffer(file, reporter).await?;

    Some(SignalHandler::new(reporter, cmd_recv))
}

async fn resume_file(
    filepath: &Path,
    downloaded: u64,
    accept_range: bool,
) -> Result<BufWriter<File>, TaskResult> {
    if !accept_range {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(filepath)
            .await
            .map_err(|e| TaskResult::new_failed_to_resume_file(e.to_string()))?;
        return Ok(BufWriter::new(file));
    }

    let file = OpenOptions::new()
        .create(false)
        .append(true)
        .open(filepath)
        .await
        .map_err(|e| TaskResult::new_failed_to_resume_file(e.to_string()))?;

    if file
        .metadata()
        .await
        .map_err(|e| TaskResult::new_failed_to_resume_file(e.to_string()))?
        .len()
        < downloaded
    {
        return Err(TaskResult::new_file_corrupted(
            "File changed or failed to write since last download attempt".to_string(),
        ));
    }

    file.set_len(downloaded)
        .await
        .map_err(|e| TaskResult::new_failed_to_resume_file(e.to_string()))?;

    Ok(BufWriter::new(file))
}

async fn handle_resume_download(task: TaskInner, handler: SignalHandler) {
    let (url, filepath, accept_range, mut downloaded) = {
        let mut state_guard = task.state.lock().unwrap();
        let url = state_guard.url.clone().unwrap();
        let filepath = state_guard.filepath.clone();
        let accept_ranges = state_guard.accept_ranges;
        let mut downloaded = state_guard.downloaded;

        if !accept_ranges {
            state_guard.downloaded = 0;
            downloaded = 0;
        }

        state_guard.last_updated = Instant::now();
        state_guard.last_downloaded = downloaded;
        state_guard.last_speed = None;

        (url, filepath, accept_ranges, downloaded)
    }; // MutexGuard unlock here

    if !accept_range {
        downloaded = 0;
        task.state.lock().unwrap().downloaded = 0;
    }

    let mut file = match resume_file(&filepath, downloaded, accept_range).await {
        Ok(f) => f,
        Err(tr) => {
            handler.reporter.send(tr).unwrap();
            return;
        }
    };

    let client = match ClientBuilder::new().build() {
        Ok(c) => c,
        Err(e) => {
            handler
                .reporter
                .send(TaskResult::new_failed_to_resume_connection(e.to_string()))
                .unwrap();
            return;
        }
    };

    let stream =
        match get_resume_download_stream(&task, url, &client, downloaded, accept_range).await {
            Ok(s) => s,
            Err(e) => {
                handler
                    .reporter
                    .send(TaskResult::new_failed_to_resume_connection(e.to_string()))
                    .unwrap();
                return;
            }
        };
    let stream = pin!(stream);

    if let Some(handler) = download_stream_to_file(&task, stream, &mut file, handler).await {
        handler.reporter.send(TaskResult::new_finished()).unwrap();
    }
}

async fn get_resume_download_stream(
    task: &TaskInner,
    url: Url,
    client: &reqwest::Client,
    downloaded: u64,
    accept_range: bool,
) -> anyhow::Result<impl Stream<Item = reqwest::Result<Bytes>>> {
    if accept_range {
        let response = client
            .get(url)
            .header(
                header::RANGE,
                header::HeaderValue::from_str(&format!("bytes={}-", downloaded))?,
            )
            .send()
            .await?;

        let head = response.headers();
        let content_length = head
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        {
            let mut state = task.state.lock().unwrap();
            if let Some(len) = content_length {
                state.content_length = Some(len + downloaded);
            }
        }

        let stream = response.bytes_stream();
        return Ok(stream);
    }

    // vvv !ACCEPT_RANGES

    let response = client.get(url).send().await?;
    let head = response.headers();
    let content_length = head
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let accept_ranges = head
        .get(header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.eq_ignore_ascii_case("bytes"));

    {
        let mut state = task.state.lock().unwrap();
        state.content_length = content_length;
        state.accept_ranges = accept_ranges;
    }

    let stream = response.bytes_stream();
    Ok(stream)
}
