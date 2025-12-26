//! ATM Tray Updater
//! 用于替换主程序 EXE 并重启
//! 
//! 使用方法: updater.exe <目标exe路径> <新版本exe路径>

use std::env;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 3 {
        eprintln!("用法: updater.exe <目标exe路径> <新版本exe路径>");
        eprintln!("示例: updater.exe C:\\app\\atm-tray.exe C:\\app\\update.exe");
        std::process::exit(1);
    }
    
    let target_exe = &args[1];
    let new_exe = &args[2];
    
    println!("ATM Tray Updater");
    println!("================");
    println!("目标文件: {}", target_exe);
    println!("新版本: {}", new_exe);
    
    // 等待主程序退出
    println!("\n等待主程序退出...");
    for i in 0..10 {
        thread::sleep(Duration::from_millis(500));
        
        // 尝试打开目标文件，如果能打开说明主程序已退出
        if let Ok(_) = fs::OpenOptions::new()
            .write(true)
            .open(target_exe)
        {
            println!("主程序已退出");
            break;
        }
        
        if i == 9 {
            println!("等待超时，强制继续...");
        }
    }
    
    // 备份旧版本
    let backup_path = format!("{}.bak", target_exe);
    println!("\n备份旧版本到: {}", backup_path);
    if let Err(e) = fs::copy(target_exe, &backup_path) {
        eprintln!("警告: 备份失败: {}", e);
    }
    
    // 替换文件
    println!("替换文件...");
    match fs::copy(new_exe, target_exe) {
        Ok(_) => {
            println!("文件替换成功!");
            
            // 删除下载的临时文件
            if let Err(e) = fs::remove_file(new_exe) {
                eprintln!("警告: 删除临时文件失败: {}", e);
            }
            
            // 启动新版本
            println!("\n启动新版本...");
            thread::sleep(Duration::from_millis(500));
            
            match Command::new(target_exe).spawn() {
                Ok(_) => {
                    println!("新版本已启动!");
                }
                Err(e) => {
                    eprintln!("启动失败: {}", e);
                    eprintln!("请手动启动程序: {}", target_exe);
                    
                    // 等待用户看到错误信息
                    thread::sleep(Duration::from_secs(5));
                }
            }
        }
        Err(e) => {
            eprintln!("文件替换失败: {}", e);
            
            // 尝试恢复备份
            if fs::metadata(&backup_path).is_ok() {
                println!("尝试恢复备份...");
                if let Err(e) = fs::copy(&backup_path, target_exe) {
                    eprintln!("恢复失败: {}", e);
                } else {
                    println!("已恢复到旧版本");
                }
            }
            
            // 等待用户看到错误信息
            thread::sleep(Duration::from_secs(5));
        }
    }
}
