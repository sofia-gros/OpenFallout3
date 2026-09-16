use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use wgpu;

struct AudioPipeReader {
    stdout: ChildStdout,
    stop: Arc<AtomicBool>,
    eof: Arc<AtomicBool>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl AudioPipeReader {
    fn run(mut self) {
        let mut chunk = vec![0u8; 4096];
        loop {
            if self.stop.load(Ordering::Relaxed) { break; }
            match self.stdout.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let mut buf = self.buffer.lock().unwrap();
                    for b in chunk[..n].chunks_exact(2) {
                        let s16 = i16::from_le_bytes([b[0], b[1]]);
                        buf.push_back(s16 as f32 / 32768.0);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        self.eof.store(true, Ordering::Release);
    }
}

struct PcmStreamSource {
    buffer: Arc<Mutex<VecDeque<f32>>>,
    eof: Arc<AtomicBool>,
}

impl Iterator for PcmStreamSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(sample) = self.buffer.lock().unwrap().pop_front() {
            return Some(sample);
        }
        if self.eof.load(Ordering::Acquire) {
            return None;
        }
        Some(0.0)
    }
}

impl rodio::Source for PcmStreamSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 2 }
    fn sample_rate(&self) -> u32 { 44100 }
    fn total_duration(&self) -> Option<Duration> { None }
}

struct VideoPipeReader {
    stdout: ChildStdout,
    stop: Arc<AtomicBool>,
    eof: Arc<AtomicBool>,
    frame_size: usize,
    buffer: Arc<Mutex<Option<Vec<u8>>>>,
}

impl VideoPipeReader {
    fn run(mut self) {
        let mut frame_buf = vec![0u8; self.frame_size];
        loop {
            if self.stop.load(Ordering::Relaxed) { break; }
            let mut bytes_read = 0;
            while bytes_read < self.frame_size {
                if self.stop.load(Ordering::Relaxed) { break; }
                match self.stdout.read(&mut frame_buf[bytes_read..]) {
                    Ok(0) => {
                        self.eof.store(true, Ordering::Release);
                        return;
                    }
                    Ok(n) => bytes_read += n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        self.eof.store(true, Ordering::Release);
                        return;
                    }
                }
            }
            if bytes_read == self.frame_size {
                let mut guard = self.buffer.lock().unwrap();
                *guard = Some(frame_buf.clone());
            }
        }
    }
}

pub struct BinkPlayer {
    _process: Option<Child>,
    _video_thread: Option<JoinHandle<()>>,
    video_stop: Arc<AtomicBool>,
    video_eof: Arc<AtomicBool>,
    video_buffer: Arc<Mutex<Option<Vec<u8>>>>,
    pub width: u32,
    pub height: u32,
    pub finished: bool,
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    _audio_process: Option<Child>,
    _audio_thread: Option<JoinHandle<()>>,
    audio_stop: Arc<AtomicBool>,
    audio_eof: Arc<AtomicBool>,
    audio_buffer: Arc<Mutex<VecDeque<f32>>>,
    audio_sink: Option<rodio::Sink>,
}

impl BinkPlayer {
    pub fn open(
        file_path: &str,
        target_width: u32,
        target_height: u32,
        device: &wgpu::Device,
        audio_handle: Option<rodio::OutputStreamHandle>,
    ) -> Option<Self> {
        let mut child = Command::new("ffmpeg")
            .args([
                "-re",
                "-loglevel", "quiet",
                "-i", file_path,
                "-an",
                "-f", "rawvideo",
                "-pix_fmt", "rgba",
                "-s", &format!("{}x{}", target_width, target_height),
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let stdout = child.stdout.take()?;

        let texture_size = wgpu::Extent3d {
            width: target_width,
            height: target_height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bink Video Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let video_stop = Arc::new(AtomicBool::new(false));
        let video_eof = Arc::new(AtomicBool::new(false));
        let video_buffer = Arc::new(Mutex::new(None));

        let frame_size = (target_width * target_height * 4) as usize;

        let reader = VideoPipeReader {
            stdout,
            stop: video_stop.clone(),
            eof: video_eof.clone(),
            frame_size,
            buffer: video_buffer.clone(),
        };
        let video_thread = thread::spawn(move || reader.run());

        let (audio_process, audio_thread, audio_stop, audio_eof, audio_buffer, audio_sink) =
            Self::open_audio(file_path, audio_handle);

        println!("[BinkPlayer] {} ({}x{}) のインウィンドウ再生を開始 (非同期)", file_path, target_width, target_height);

        Some(Self {
            _process: Some(child),
            _video_thread: Some(video_thread),
            video_stop,
            video_eof,
            video_buffer,
            width: target_width,
            height: target_height,
            finished: false,
            texture,
            texture_view,
            _audio_process: audio_process,
            _audio_thread: audio_thread,
            audio_stop,
            audio_eof,
            audio_buffer,
            audio_sink,
        })
    }

    #[allow(clippy::type_complexity)]
    fn open_audio(
        file_path: &str,
        audio_handle: Option<rodio::OutputStreamHandle>,
    ) -> (
        Option<Child>,
        Option<JoinHandle<()>>,
        Arc<AtomicBool>,
        Arc<AtomicBool>,
        Arc<Mutex<VecDeque<f32>>>,
        Option<rodio::Sink>,
    ) {
        let stop = Arc::new(AtomicBool::new(false));
        let eof = Arc::new(AtomicBool::new(false));
        let buffer: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));

        let handle = match audio_handle {
            Some(h) => h,
            None => return (None, None, stop, eof, buffer, None),
        };

        let mut child = match Command::new("ffmpeg")
            .args([
                "-re",
                "-loglevel", "quiet",
                "-i", file_path,
                "-vn",
                "-ac", "2",
                "-ar", "44100",
                "-f", "s16le",
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return (None, None, stop, eof, buffer, None),
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = child.kill();
                return (None, None, stop, eof, buffer, None);
            }
        };

        let source = PcmStreamSource {
            buffer: buffer.clone(),
            eof: eof.clone(),
        };
        let sink = match rodio::Sink::try_new(&handle) {
            Ok(s) => {
                s.append(source);
                Some(s)
            }
            Err(_) => {
                let _ = child.kill();
                return (None, None, stop, eof, buffer, None);
            }
        };

        let reader = AudioPipeReader {
            stdout,
            stop: stop.clone(),
            eof: eof.clone(),
            buffer: buffer.clone(),
        };
        let thread = thread::spawn(move || reader.run());

        (Some(child), Some(thread), stop, eof, buffer, sink)
    }

    pub fn advance_frame(&mut self, queue: &wgpu::Queue) -> bool {
        if self.finished {
            return false;
        }

        let mut frame_data = None;
        {
            let mut guard = self.video_buffer.lock().unwrap();
            if guard.is_some() {
                frame_data = guard.take();
            }
        }

        if let Some(data) = frame_data {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * 4),
                    rows_per_image: Some(self.height),
                },
                wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            );
            true
        } else {
            if self.video_eof.load(Ordering::Acquire) {
                self.finished = true;
                false
            } else {
                true // still playing, no new frame this tick
            }
        }
    }
}

impl Drop for BinkPlayer {
    fn drop(&mut self) {
        self.audio_stop.store(true, Ordering::Relaxed);
        self.video_stop.store(true, Ordering::Relaxed);
        
        if let Some(mut p) = self._audio_process.take() {
            let _ = p.kill();
        }
        if let Some(t) = self._audio_thread.take() {
            let _ = t.join();
        }
        
        if let Some(mut p) = self._process.take() {
            let _ = p.kill();
        }
        if let Some(t) = self._video_thread.take() {
            let _ = t.join();
        }
    }
}
