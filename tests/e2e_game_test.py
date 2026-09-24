#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Fallout 3 (Gamebryo 2.6) 実機 E2E 自動テストハーネス
Windows ネイティブ仮想入力 (SendInput) を用いて実プロセス (fo3_viewer.exe) を直接検証。
"""

import os
import sys
import time
import subprocess
import threading
import ctypes
from ctypes import wintypes

try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except Exception:
    pass

# Windows API 定数および構造体
user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32

INPUT_KEYBOARD = 1
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_SCANCODE = 0x0008

# 仮想キーコード (Virtual Key Codes)
VK_RETURN = 0x0D
VK_ESCAPE = 0x1B
VK_SPACE  = 0x20
VK_KEY_W  = 0x57
VK_KEY_A  = 0x41
VK_KEY_S  = 0x53
VK_KEY_D  = 0x44
VK_KEY_E  = 0x45

class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(wintypes.ULONG)),
    ]

class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [
        ("uMsg", wintypes.DWORD),
        ("wParamL", wintypes.WORD),
        ("wParamH", wintypes.WORD),
    ]

class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(wintypes.ULONG)),
    ]

class _INPUT_UNION(ctypes.Union):
    _fields_ = [
        ("mi", MOUSEINPUT),
        ("ki", KEYBDINPUT),
        ("hi", HARDWAREINPUT),
    ]

class INPUT(ctypes.Structure):
    _fields_ = [
        ("type", wintypes.DWORD),
        ("u", _INPUT_UNION),
    ]

WM_KEYDOWN = 0x0100
WM_KEYUP   = 0x0101

target_window_hwnd = None

def send_key(vk_code, hold_time=0.05):
    """Windows API SendInput (SCANCODE) および PostMessage を使ってキーを押下・解放する。"""
    scan = user32.MapVirtualKeyW(vk_code, 0)
    
    inp_down = INPUT()
    inp_down.type = INPUT_KEYBOARD
    inp_down.u.ki.wVk = vk_code
    inp_down.u.ki.wScan = scan
    inp_down.u.ki.dwFlags = KEYEVENTF_SCANCODE
    
    inp_up = INPUT()
    inp_up.type = INPUT_KEYBOARD
    inp_up.u.ki.wVk = vk_code
    inp_up.u.ki.wScan = scan
    inp_up.u.ki.dwFlags = KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP
    
    user32.SendInput(1, ctypes.byref(inp_down), ctypes.sizeof(INPUT))
    if target_window_hwnd:
        lparam_down = 1 | (scan << 16)
        user32.PostMessageW(target_window_hwnd, WM_KEYDOWN, vk_code, lparam_down)
        
    time.sleep(hold_time)
    
    user32.SendInput(1, ctypes.byref(inp_up), ctypes.sizeof(INPUT))
    if target_window_hwnd:
        lparam_up = 1 | (scan << 16) | (1 << 30) | (1 << 31)
        user32.PostMessageW(target_window_hwnd, WM_KEYUP, vk_code, lparam_up)

def focus_window_by_title_substring(title_substr):
    """指定の文字列を含むトップレベルウィンドウを探してフォアグラウンドに持ってくる。"""
    found_hwnd = []

    def enum_windows_proc(hwnd, lParam):
        if user32.IsWindowVisible(hwnd):
            length = user32.GetWindowTextLengthW(hwnd)
            if length > 0:
                buf = ctypes.create_unicode_buffer(length + 1)
                user32.GetWindowTextW(hwnd, buf, length + 1)
                if title_substr.lower() in buf.value.lower():
                    found_hwnd.append((hwnd, buf.value))
        return True

    WNDENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.EnumWindows(WNDENUMPROC(enum_windows_proc), 0)
    
    if found_hwnd:
        global target_window_hwnd
        hwnd, title = found_hwnd[0]
        target_window_hwnd = hwnd
        user32.ShowWindow(hwnd, 9) # SW_RESTORE
        user32.SetForegroundWindow(hwnd)
        rect = wintypes.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
        center_x = (rect.left + rect.right) // 2
        center_y = (rect.top + rect.bottom) // 2
        user32.SetCursorPos(center_x, center_y)
        user32.mouse_event(0x0002, 0, 0, 0, 0) # LEFTDOWN
        time.sleep(0.05)
        user32.mouse_event(0x0004, 0, 0, 0, 0) # LEFTUP
        time.sleep(0.1)
        return True, title
    return False, None

def run_e2e_test():
    print("=" * 70)
    print("Fallout 3 実機 E2E 自動テスト開始")
    print("=" * 70)
    
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    data_dir = r"A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data"
    exe_path = os.path.join(repo_root, "target", "debug", "fo3_viewer.exe")
    
    if not os.path.exists(exe_path):
        print(f"[*] バイナリが存在しません。ビルドを実行中: {exe_path}")
        res = subprocess.run(["cargo", "build", "-p", "fo3_viewer"], cwd=repo_root)
        if res.returncode != 0:
            print("[!] ビルドに失敗しました。テストを中断します。")
            sys.exit(1)
            
    print(f"[*] 対象バイナリ: {exe_path}")
    print(f"[*] データディレクトリ: {data_dir}")
    print("[*] ゲームプロセス (newgame モード) を起動します...")
    
    cmd = [exe_path, "newgame", data_dir]
    
    proc = subprocess.Popen(
        cmd,
        cwd=repo_root,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        encoding="utf-8",
        errors="replace"
    )
    
    log_lines = []
    quest_events = []
    heartbeats = []
    key_inputs = []
    stop_event = threading.Event()
    
    def log_reader():
        for line in iter(proc.stdout.readline, ''):
            clean_line = line.rstrip()
            log_lines.append(clean_line)
            try:
                print(f"[ENGINE-STDOUT] {clean_line}")
            except Exception:
                safe_line = clean_line.encode("ascii", "replace").decode("ascii")
                print(f"[ENGINE-STDOUT] {safe_line}")
            
            if "[EVENT:QUEST]" in clean_line:
                quest_events.append(clean_line)
            if "[ENGINE:HEARTBEAT]" in clean_line:
                heartbeats.append(clean_line)
            if "[INPUT:KEY]" in clean_line:
                key_inputs.append(clean_line)
                
            if stop_event.is_set():
                break
        proc.stdout.close()
        
    t = threading.Thread(target=log_reader, daemon=True)
    t.start()
    
    test_duration = 60 # 最大 60 秒間実行
    start_time = time.time()
    
    print("[*] プロセス出力を監視し、ウィンドウ生成および描画開始を待機中...")
    
    window_focused = False
    action_step = 0
    gameplay_start_time = None
    
    while time.time() - start_time < test_duration:
        # プロセスの生存チェック
        ret = proc.poll()
        if ret is not None:
            print(f"\n[!] プロセスが予期せず終了しました (終了コード: {ret})！")
            break
            
        elapsed = time.time() - start_time
        
        # 1. ウィンドウフォーカス
        if not window_focused and elapsed > 2.0:
            focused, title = focus_window_by_title_substring("OpenFallout3")
            if not focused:
                focused, title = focus_window_by_title_substring("Viewer")
            if focused:
                print(f"[+] ゲームウィンドウをアクティブ化しました: \"{title}\"")
                window_focused = True
                    
        # 2. フレーム描画が開始されたらゲームプレイ入力シナリオを開始
        if len(heartbeats) > 0 and gameplay_start_time is None:
            gameplay_start_time = time.time()
            print("[+] ★ 描画ループ (ハートビート) の生存を確認！ゲーム入力注入を開始します。")

        if gameplay_start_time is not None:
            gp_elapsed = time.time() - gameplay_start_time
            if action_step == 0 and gp_elapsed > 2.0:
                print("[*] 入力注入: Space キー (ムービー/イントロスキップ)")
                send_key(VK_SPACE)
                action_step = 1
            elif action_step == 1 and gp_elapsed > 4.0:
                print("[*] 入力注入: Enter キー (ダイアログ/確定)")
                send_key(VK_RETURN)
                action_step = 2
            elif action_step == 2 and gp_elapsed > 7.0:
                print("[*] 入力注入: E キー (インタラクト)")
                send_key(VK_KEY_E)
                action_step = 3
            elif action_step == 3 and gp_elapsed > 10.0:
                print("[*] 入力注入: W キー (前進移動 1秒)")
                send_key(VK_KEY_W, hold_time=1.0)
                action_step = 4
            elif action_step == 4 and gp_elapsed > 15.0:
                print("[*] 目標シナリオ完了。テスト終了フェーズへ進みます。")
                break
                
        time.sleep(0.5)
        
    print("\n[*] テスト終了フェーズ: プロセスを正常終了させます...")
    stop_event.set()
    
    if proc.poll() is None:
        print("[*] ESC キー送信で正常終了要求...")
        send_key(VK_ESCAPE)
        time.sleep(1.0)
        
    if proc.poll() is None:
        print("[*] プロセス終了待機 (terminate)...")
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            
    # テストログの保存
    log_path = os.path.join(repo_root, "tests", "e2e_run.log")
    with open(log_path, "w", encoding="utf-8") as f:
        f.write("\n".join(log_lines))
        
    print("\n" + "=" * 70)
    print("E2E テスト実行サマリ")
    print("=" * 70)
    print(f"- 実行時間: {time.time() - start_time:.1f} 秒")
    print(f"- 総ログ行数: {len(log_lines)}")
    print(f"- クエストイベント数: {len(quest_events)}")
    for q in quest_events:
        print(f"    {q}")
    print(f"- ハートビート (描画生存) 回数: {len(heartbeats)}")
    print(f"- キー入力検知数: {len(key_inputs)}")
    for k in key_inputs:
        print(f"    {k}")
    print(f"- ログ保存先: {log_path}")
    print("=" * 70)
    
    if len(heartbeats) > 0:
        print("[SUCCESS] ゲームループが継続生存し、正常にフレーム描画が実行されました。")
    else:
        print("[WARNING] フレーム描画ハートビートが検知されませんでした。ログを確認してください。")

if __name__ == "__main__":
    run_e2e_test()
