//! FFI-обёртка TDLib tdjson.
//!
//! TDLib — библиотека на C++ с C API `td_json_client_*`: все запросы/ответы —
//! строки JSON. Здесь минимальная обёртка поверх libloading.
//!
//! Архитектура: один выделенный поток владельца клиента (TDLib клиент НЕ
//! Send-safe по своей природе — все вызовы идут через channel в этот поток).

use std::ffi::{CStr, CString, c_double, c_void};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc;

use libloading::Library;
use serde_json::{Value, json};

type FnCreate = unsafe extern "C" fn() -> *mut c_void;
type FnSend = unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char);
type FnReceive = unsafe extern "C" fn(*mut c_void, c_double) -> *const std::ffi::c_char;
type FnExecute =
    unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char) -> *const std::ffi::c_char;

enum Command {
    Send { method: String, params: Value },
    Execute { method: String, params: Value },
}

struct WorkerState {
    client: *mut c_void,
    send: FnSend,
    receive: FnReceive,
    execute: FnExecute,
}

// SAFETY: TdClient передаёт state только в выделенный worker-поток; доступ
// к сырым указателям изолирован внутри одного потока (движение — единожды).
unsafe impl Send for WorkerState {}

/// Ручка клиента: шлёт команды в worker-поток и ждёт ответов.
pub struct TdClient {
    tx: mpsc::Sender<Command>,
    next_extra: AtomicI64,
    /// Ответы на request(), отфильтрованные по @extra из общего потока апдейтов.
    pending: std::sync::Arc<PendingMap>,
    /// Приёмник произвольных апдейтов (updateNewMessage и т.п.).
    updates: mpsc::Receiver<Value>,
}

type PendingMap = std::sync::Mutex<std::collections::HashMap<i64, mpsc::Sender<Value>>>;

impl TdClient {
    /// Загружает libtdjson и запускает worker-поток.
    pub fn start(tdjson_path: &str) -> anyhow::Result<Self> {
        unsafe {
            let lib = Library::new(tdjson_path)?;
            let client = (lib.get::<FnCreate>(b"td_json_client_create")?)();
            // Символы нужно скопировать ДО перемещения	lib в Box
            let send: FnSend = *lib.get(b"td_json_client_send")?;
            let receive: FnReceive = *lib.get(b"td_json_client_receive")?;
            let execute: FnExecute = *lib.get(b"td_json_client_execute")?;
            let destroy_fn: &'static unsafe extern "C" fn(*mut c_void) =
                Box::leak(Box::new(*lib.get(b"td_json_client_destroy")?));

            let state = WorkerState {
                client,
                send,
                receive,
                execute,
            };
            // lib живёт до конца процесса (Box::leak) — tdjson символы остаются валидными
            let lib: &'static Library = Box::leak(Box::new(lib));

            let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
            let (upd_tx, upd_rx) = mpsc::channel::<Value>();
            let pending: std::sync::Arc<PendingMap> = std::sync::Arc::default();
            let pending_worker = pending.clone();

            let state = std::sync::Mutex::new(state);
            std::thread::Builder::new()
                .name("tdlib-worker".into())
                .spawn(move || {
                    worker_loop(state, cmd_rx, upd_tx, pending_worker, lib, destroy_fn)
                })?;

            Ok(Self {
                tx: cmd_tx,
                next_extra: AtomicI64::new(1),
                pending,
                updates: upd_rx,
            })
        }
    }

    fn next_extra(&self) -> i64 {
        self.next_extra.fetch_add(1, Ordering::Relaxed)
    }

    /// Отправить запрос без ожидания (fire-and-forget).
    pub fn send(&self, method: &str, params: &Value) -> anyhow::Result<()> {
        self.tx.send(Command::Send {
            method: method.into(),
            params: params.clone(),
        })?;
        Ok(())
    }

    /// Запрос-ответ: отправить, ждать ответ с совпавшим @extra,
    /// промежуточные апдейты уходят в общий поток.
    pub fn request(
        &self,
        method: &str,
        params: &Value,
        timeout_secs: f64,
    ) -> anyhow::Result<Value> {
        let extra = self.next_extra();
        let (resp_tx, resp_rx) = mpsc::channel();
        self.pending
            .lock()
            .expect("pending lock")
            .insert(extra, resp_tx);

        self.tx.send(Command::Send {
            method: method.into(),
            params: {
                let mut p = params.clone();
                if let Some(o) = p.as_object_mut() {
                    o.insert("@extra".into(), json!(extra));
                }
                p
            },
        })?;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout_secs);
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match resp_rx.recv_timeout(left) {
            Ok(v) => Ok(v),
            Err(_) => {
                self.pending.lock().expect("pending lock").remove(&extra);
                anyhow::bail!("TDLib request timeout/worker exit: {method}")
            }
        }
    }

    /// Синхронный execute (setLogVerbosityLevel и т.п.). Результат не нужен.
    pub fn execute(&self, method: &str, params: &Value) -> anyhow::Result<()> {
        self.tx.send(Command::Execute {
            method: method.into(),
            params: params.clone(),
        })?;
        Ok(())
    }

    /// Получить следующий апдейт (блокирует до arrival).
    pub fn recv_update(&self, timeout: std::time::Duration) -> Option<Value> {
        mpsc::Receiver::recv_timeout(&self.updates, timeout).ok()
    }
}

impl Drop for TdClient {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Send {
            method: "close".into(),
            params: json!({}),
        });
    }
}

fn worker_loop(
    state: std::sync::Mutex<WorkerState>,
    cmd_rx: mpsc::Receiver<Command>,
    upd_tx: mpsc::Sender<Value>,
    pending: std::sync::Arc<PendingMap>,
    _lib: &'static Library,
    destroy_fn: &'static unsafe extern "C" fn(*mut c_void),
) {
    loop {
        // 1) приём апдейтов из TDLib
        {
            let st = state.lock().expect("state lock");
            let WorkerState {
                client,
                receive,
                execute,
                send,
            } = &*st;
            unsafe {
                let raw = receive(*client, 0.05);
                if !raw.is_null() {
                    let s = CStr::from_ptr(raw).to_string_lossy().into_owned();
                    drop(st);
                    if let Ok(v) = serde_json::from_str::<Value>(&s) {
                        dispatch_update(v, &upd_tx, &pending);
                    }
                    continue;
                }
                // 2) исходящие команды
                match cmd_rx.try_recv() {
                    Ok(Command::Send { method, params }) => {
                        let mut full = json!({ "@type": method });
                        if let (Some(dst), Some(src)) = (full.as_object_mut(), params.as_object()) {
                            for (k, v) in src {
                                dst.insert(k.clone(), v.clone());
                            }
                        }
                        let c = CString::new(full.to_string()).expect("no nul");
                        send(*client, c.as_ptr());
                    }
                    Ok(Command::Execute { method, params }) => {
                        let mut full = params.clone();
                        if let Some(o) = full.as_object_mut() {
                            o.insert("@type".into(), json!(method));
                        }
                        let c = CString::new(full.to_string()).expect("no nul");
                        let _ = execute(*client, c.as_ptr());
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        destroy_fn(*client);
                        return;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
        }
    }
}

fn dispatch_update(v: Value, upd_tx: &mpsc::Sender<Value>, pending: &std::sync::Arc<PendingMap>) {
    if let Some(extra) = v.get("@extra").and_then(|e| e.as_i64()) {
        let sender = pending.lock().expect("pending lock").remove(&extra);
        if let Some(tx) = sender {
            let _ = tx.send(v);
            return;
        }
    }
    let ty = v.get("@type").and_then(Value::as_str).unwrap_or("");
    if ty.starts_with("update") {
        let _ = upd_tx.send(v);
    }
}
