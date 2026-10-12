# Demo actions: glide the real cursor to a point in the demo Jawjack window,
# then click through the app's control channel. Window-local coordinates.
param([string]$Do, [int]$X = 0, [int]$Y = 0, [string]$Text = "", [string]$Key = "", [int]$Ms = 600)
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices;
public class Cur { [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y); [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
"@
[Cur]::SetProcessDPIAware() | Out-Null
$cc = Join-Path $PSScriptRoot 'cc.ps1'
$OX = 920; $OY = 270  # demo window's top-left on screen
function Glide([int]$tx, [int]$ty, [int]$ms) {
    $p = [System.Windows.Forms.Cursor]::Position
    $steps = [Math]::Max(12, [int]($ms / 16))
    for ($i = 1; $i -le $steps; $i++) {
        $t = $i / $steps; $e = 1 - [Math]::Pow(1 - $t, 3)  # ease-out
        [Cur]::SetCursorPos([int]($p.X + ($tx - $p.X) * $e), [int]($p.Y + ($ty - $p.Y) * $e)) | Out-Null
        Start-Sleep -Milliseconds 16
    }
}
function Ptr($kind) { & $cc -Json "{`"method`":`"pointer`",`"kind`":`"$kind`",`"x`":$X,`"y`":$Y,`"button`":`"left`"}" | Out-Null }
switch ($Do) {
    'move'  { Glide ($OX + $X) ($OY + $Y) $Ms; Ptr 'move' }
    'click' { Glide ($OX + $X) ($OY + $Y) $Ms; Ptr 'move'; Start-Sleep -Milliseconds 180; Ptr 'down'; Start-Sleep -Milliseconds 90; Ptr 'up' }
    'type'  { foreach ($ch in $Text.ToCharArray()) { $j = ("$ch" | ConvertTo-Json); & $cc -Json "{`"method`":`"text`",`"text`":$j}" | Out-Null; Start-Sleep -Milliseconds 35 } }
    'key'   { & $cc -Json "{`"method`":`"key`",`"key`":`"$Key`"}" | Out-Null }
    'screen'{ Glide $X $Y $Ms }  # screen coordinates, cursor only
}

# Real mouse click at screen coordinates (for windows outside Jawjack).
if ($Do -eq 'osclick') {
    Add-Type @"
using System; using System.Runtime.InteropServices;
public class Btn { [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e); }
"@
    Glide $X $Y $Ms; Start-Sleep -Milliseconds 200
    [Btn]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80; [Btn]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}
