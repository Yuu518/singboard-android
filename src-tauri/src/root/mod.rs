pub mod fs;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const SERVICE_HOME: &str = "/data/adb/singboard";

static ENABLED: AtomicBool = AtomicBool::new(false);
static SEQ: AtomicU64 = AtomicU64::new(0);
static SHELL: Mutex<Option<Shell>> = Mutex::new(None);
static DENIED_AT: Mutex<Option<Instant>> = Mutex::new(None);

const DENIED_COOLDOWN: Duration = Duration::from_secs(15);

struct Shell {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Shell {
    fn spawn() -> Result<Self, String> {
        Self::spawn_program("su")
    }

    fn spawn_program(program: &str) -> Result<Self, String> {
        let mut child = Command::new(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("无法启动 su: {e}"))?;
        let stdin = child.stdin.take().ok_or("无法连接 su 输入")?;
        let stdout = child.stdout.take().ok_or("无法连接 su 输出")?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn run(&mut self, script: &str) -> std::io::Result<(i32, Vec<u8>)> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default();
        let marker = format!(
            "__SINGBOARD_END_{}_{}__",
            SEQ.fetch_add(1, Ordering::Relaxed),
            nonce
        );
        let payload = format!("(\n{script}\n) </dev/null 2>&1\nprintf '\\n{marker}:%s\\n' \"$?\"\n");
        self.stdin.write_all(payload.as_bytes())?;
        self.stdin.flush()?;

        let mut output = Vec::new();
        loop {
            let mut line = Vec::new();
            if self.stdout.read_until(b'\n', &mut line)? == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            if let Some(rest) = line.strip_prefix(marker.as_bytes()) {
                let code = String::from_utf8_lossy(rest)
                    .trim_start_matches(':')
                    .trim()
                    .parse()
                    .unwrap_or(-1);
                if output.last() == Some(&b'\n') {
                    output.pop();
                }
                return Ok((code, output));
            }
            output.extend_from_slice(&line);
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        if let Ok(mut guard) = SHELL.lock() {
            *guard = None;
        }
    }
}

fn exec_raw(script: &str, respect_cooldown: bool) -> Result<(i32, Vec<u8>), String> {
    let mut guard = SHELL.lock().map_err(|_| "root shell 状态异常".to_string())?;
    if guard.is_none() {
        if respect_cooldown && in_cooldown() {
            return Err("未获得 root 权限".into());
        }
        match Shell::spawn() {
            Ok(shell) => *guard = Some(shell),
            Err(e) => {
                mark_denied();
                return Err(e);
            }
        }
    }
    let shell = guard.as_mut().expect("root shell");
    match shell.run(script) {
        Ok(result) => {
            clear_denied();
            Ok(result)
        }
        Err(e) => {
            *guard = None;
            mark_denied();
            if e.kind() == std::io::ErrorKind::UnexpectedEof
                || e.kind() == std::io::ErrorKind::BrokenPipe
            {
                Err("未获得 root 权限".into())
            } else {
                Err(format!("root shell 执行失败: {e}"))
            }
        }
    }
}

fn in_cooldown() -> bool {
    DENIED_AT
        .lock()
        .ok()
        .and_then(|at| *at)
        .is_some_and(|at| at.elapsed() < DENIED_COOLDOWN)
}

fn mark_denied() {
    if let Ok(mut at) = DENIED_AT.lock() {
        *at = Some(Instant::now());
    }
}

fn clear_denied() {
    if let Ok(mut at) = DENIED_AT.lock() {
        *at = None;
    }
}

pub fn exec(script: &str) -> Result<(i32, Vec<u8>), String> {
    if !enabled() {
        return Err("未启用 root 模式".into());
    }
    exec_raw(script, true)
}

pub fn run(script: &str) -> Result<String, String> {
    let (code, output) = exec(script)?;
    let text = String::from_utf8_lossy(&output).trim().to_string();
    if code == 0 {
        Ok(text)
    } else if text.is_empty() {
        Err(format!("命令执行失败（退出码 {code}）"))
    } else {
        Err(text)
    }
}

pub async fn run_async(script: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || run(&script))
        .await
        .map_err(|e| format!("任务执行失败: {e}"))?
}

pub fn probe() -> bool {
    matches!(exec_raw("id -u", false), Ok((0, out)) if String::from_utf8_lossy(&out).trim() == "0")
}

pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_protocol_keeps_output_exit_codes_and_session_state() {
        let Ok(mut shell) = Shell::spawn_program("sh") else {
            return;
        };
        let (code, out) = shell.run("echo first\nprintf 'no newline'").unwrap();
        assert_eq!(code, 0);
        assert_eq!(String::from_utf8_lossy(&out), "first\nno newline");

        let (code, out) = shell.run("echo failing >&2\nexit 3").unwrap();
        assert_eq!(code, 3);
        assert_eq!(String::from_utf8_lossy(&out), "failing\n");

        let (code, out) = shell.run("cat").unwrap();
        assert_eq!(code, 0);
        assert!(out.is_empty());

        let (code, out) = shell.run(&format!("printf '%s' {}", quote("it's quoted"))).unwrap();
        assert_eq!(code, 0);
        assert_eq!(String::from_utf8_lossy(&out), "it's quoted");
    }
}
