use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::AppHandle;

static SESSION_VALID: AtomicBool = AtomicBool::new(false);

pub fn set_session_valid(valid: bool) {
    SESSION_VALID.store(valid, Ordering::SeqCst);
}

pub fn is_session_valid() -> bool {
    SESSION_VALID.load(Ordering::SeqCst)
}

#[cfg(target_os = "windows")]
pub fn is_debugger_present() -> bool {
    use std::ffi::c_void;
    
    #[link(name = "kernel32")]
    extern "system" {
        fn IsDebuggerPresent() -> i32;
        fn CheckRemoteDebuggerPresent(hProcess: *mut c_void, pbDebuggerPresent: *mut i32) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
    }
    
    unsafe {
        // 检查本地调试器
        if IsDebuggerPresent() != 0 {
            return true;
        }
        
        // 检查远程调试器
        let mut is_remote_debugger: i32 = 0;
        let process = GetCurrentProcess();
        if CheckRemoteDebuggerPresent(process, &mut is_remote_debugger) != 0 {
            if is_remote_debugger != 0 {
                return true;
            }
        }
        
        false
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_debugger_present() -> bool {
    false
}

// 检测常见分析工具进程
#[cfg(target_os = "windows")]
fn check_analysis_tools() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let suspicious = [
        "ollydbg", "x64dbg", "x32dbg", "ida", "ida64", 
        "ghidra", "wireshark", "fiddler", "charles",
        "processhacker", "procmon", "procexp",
        "dnspy", "de4dot", "ilspy",
    ];
    
    if let Ok(output) = Command::new("tasklist")
        .creation_flags(CREATE_NO_WINDOW)
        .output() 
    {
        let list = String::from_utf8_lossy(&output.stdout).to_lowercase();
        for tool in suspicious {
            if list.contains(tool) {
                return true;
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn check_analysis_tools() -> bool {
    false
}

// 检测虚拟机环境
#[cfg(target_os = "windows")]
fn check_vm_environment() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    // 检查常见 VM 特征
    if let Ok(output) = Command::new("wmic")
        .args(["computersystem", "get", "model"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        let model = String::from_utf8_lossy(&output.stdout).to_lowercase();
        let vm_indicators = ["virtual", "vmware", "virtualbox", "qemu", "xen", "hyper-v"];
        for indicator in vm_indicators {
            if model.contains(indicator) {
                return true;
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn check_vm_environment() -> bool {
    false
}

// 时间检测（检测单步调试）
fn timing_check() -> bool {
    use std::time::Instant;
    
    let start = Instant::now();
    // 执行一些简单操作
    let mut dummy = 0u64;
    for i in 0..1000 {
        dummy = dummy.wrapping_add(i);
    }
    let elapsed = start.elapsed();
    
    // 如果执行时间异常长，可能正在被调试
    // 正常应该在 1ms 以内
    elapsed.as_millis() > 100
}

pub fn start_heartbeat_loop(_app_handle: AppHandle) {
    // 初始延迟，随机化启动时间
    let initial_delay = rand::random::<u64>() % 5000 + 1000;
    std::thread::sleep(Duration::from_millis(initial_delay));
    
    loop {
        // 随机化检查间隔 (30-90秒)
        let interval = rand::random::<u64>() % 60000 + 30000;
        std::thread::sleep(Duration::from_millis(interval));
        
        // Release 模式下进行安全检查
        #[cfg(not(debug_assertions))]
        {
            // 检查调试器
            if is_debugger_present() {
                // 不要直接退出，随机延迟后退出
                let delay = rand::random::<u64>() % 3000;
                std::thread::sleep(Duration::from_millis(delay));
                std::process::exit(0);
            }
            
            // 检查分析工具
            if check_analysis_tools() {
                let delay = rand::random::<u64>() % 5000;
                std::thread::sleep(Duration::from_millis(delay));
                std::process::exit(0);
            }
            
            // 时间检测
            if timing_check() {
                // 可能正在被单步调试
                let delay = rand::random::<u64>() % 2000;
                std::thread::sleep(Duration::from_millis(delay));
                std::process::exit(0);
            }
        }
        
        // 如果有活跃会话，发送心跳
        if is_session_valid() {
            // 心跳逻辑在 commands::heartbeat 中实现
        }
    }
}

pub fn verify_timestamp(timestamp: i64) -> bool {
    let now = chrono::Utc::now().timestamp();
    let diff = (now - timestamp).abs();
    // 允许 5 分钟的时间差
    diff < 300
}

pub fn generate_request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
