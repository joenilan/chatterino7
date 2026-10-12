param([string]$Process = "chat-workbench", [int]$X = 760, [int]$Y = 180, [int]$W = 1920, [int]$Height = 1080, [switch]$List, [int]$Id = 0, [string]$Title = "")
Add-Type @"
using System; using System.Runtime.InteropServices;
public class Stage {
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hh, uint f);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out R r, int s);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
[DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
public struct R { public int L,T,Rr,B; } }
"@
[Stage]::SetProcessDPIAware() | Out-Null
$procs = Get-Process $Process -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0
if ($Id) { $procs = $procs | Where-Object Id -eq $Id }
if ($Title) { $procs = $procs | Where-Object MainWindowTitle -like "*$Title*" }
if ($List) { $procs | ForEach-Object { "$($_.Id) '$($_.MainWindowTitle)'" }; return }
$p = $procs | Select-Object -First 1
if (-not $p) { "no window for $Process"; return }
$hwnd = $p.MainWindowHandle
[Stage]::ShowWindow($hwnd, 9) | Out-Null  # restore if minimised/maximised
# Size so the visible frame (without the invisible resize border) is exactly W x H at X,Y.
$wr = New-Object Stage+R; [Stage]::GetWindowRect($hwnd, [ref]$wr) | Out-Null
$fr = New-Object Stage+R; [Stage]::DwmGetWindowAttribute($hwnd, 9, [ref]$fr, 16) | Out-Null
$l = $fr.L - $wr.L; $t = $fr.T - $wr.T; $r = $wr.Rr - $fr.Rr; $b = $wr.B - $fr.B
[Stage]::SetWindowPos($hwnd, [IntPtr]::Zero, $X - $l, $Y - $t, $W + $l + $r, $Height + $t + $b, 0x0040) | Out-Null
# Alt tap lets SetForegroundWindow succeed from a background process.
[Stage]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [Stage]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[Stage]::SetForegroundWindow($hwnd) | Out-Null
"$Process '$($p.MainWindowTitle)' placed at $X,$Y ${W}x$Height"



