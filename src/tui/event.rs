//! 事件系统
//!
//! 统一处理来自不同源的事件:
//! - 键盘输入
//! - PTY 输出
//! - 窗口大小变化
//! - 定时器 tick

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::io::Read;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// TUI 事件类型
#[derive(Debug)]
pub enum Event {
    /// 键盘事件
    Key(KeyEvent),

    /// 鼠标事件
    Mouse(MouseEvent),

    /// PTY 输出事件
    PtyOutput(Vec<u8>),

    /// PTY 进程退出
    PtyExit(Option<u32>),

    /// 窗口大小变化
    Resize(u16, u16),

    /// 定时器 tick（用于渲染刷新）
    Tick,

    /// 错误事件
    Error(String),
}

/// 事件处理器
pub struct EventHandler {
    /// 事件接收端
    rx: mpsc::Receiver<Event>,

    /// 事件发送端 (用于 PTY 线程)
    tx: mpsc::Sender<Event>,
}

impl EventHandler {
    /// 创建新的事件处理器并启动事件循环
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel();

        // 启动终端事件监听线程
        let terminal_tx = tx.clone();
        thread::spawn(move || {
            loop {
                // 使用 poll 来实现非阻塞的事件检测
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if terminal_tx.send(Event::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            if terminal_tx.send(Event::Mouse(mouse)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            if terminal_tx.send(Event::Resize(w, h)).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {} // 忽略其他事件
                        Err(e) => {
                            let _ = terminal_tx.send(Event::Error(e.to_string()));
                            break;
                        }
                    }
                } else {
                    // 超时，发送 Tick 事件
                    if terminal_tx.send(Event::Tick).is_err() {
                        break;
                    }
                }
            }
        });

        Self { rx, tx }
    }

    /// 获取下一个事件 (阻塞)
    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.rx.recv()
    }

    /// 尝试获取下一个事件 (非阻塞)
    pub fn try_next(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }

    /// 获取发送端克隆（用于 PTY 输出线程）
    pub fn sender(&self) -> mpsc::Sender<Event> {
        self.tx.clone()
    }

    /// 启动 PTY 读取线程
    pub fn start_pty_reader<R: Read + Send + 'static>(&self, mut reader: R) {
        let tx = self.tx.clone();

        thread::spawn(move || {
            let mut buffer = [0u8; 4096];

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        // EOF - 进程退出
                        let _ = tx.send(Event::PtyExit(None));
                        break;
                    }
                    Ok(n) => {
                        let data = buffer[..n].to_vec();
                        if tx.send(Event::PtyOutput(data)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Error(format!("PTY read error: {}", e)));
                        break;
                    }
                }
            }
        });
    }
}
