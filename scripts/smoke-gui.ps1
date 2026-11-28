# Conferencia da interface GPUI: sobe o hub de desenvolvimento num diretorio temporario e numa
# porta propria (o hub instalado ocupa 7777/7778), espera, confere que o processo segue vivo e
# que o log nao tem ERROR/panicked; depois, sem falhar, tenta salvar uma captura da janela.
# Escuta so em 127.0.0.1 (HUBFINANCE_BIND): em 0.0.0.0 o Firewall do Windows abre um alerta.
param(
    [int]$Port = 47777,
    [switch]$Demo,
    # Valor de HUBFINANCE_DEMO (ex.: componentes); implica -Demo. Sem ele, -Demo usa grafite.
    [string]$DemoName = '',
    [switch]$Light,
    [switch]$Background,
    [int]$Width = 0,
    [int]$Height = 0
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

if (Get-NetTCPConnection -LocalPort $Port -ErrorAction SilentlyContinue) {
    Write-Error "porta $Port ocupada; use -Port"
    exit 1
}

cargo build
if ($LASTEXITCODE -ne 0) { exit 1 }

$data = Join-Path ([IO.Path]::GetTempPath()) ("hub-smoke-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $data | Out-Null
if ($Light) {
    Set-Content -Path (Join-Path $data 'ui.json') -Value '{"theme":"light"}' -Encoding ascii
}

$env:HUBFINANCE_UI = 'gpui'
$env:HUBFINANCE_DATA_DIR = $data
$env:HUBFINANCE_PORT = "$Port"
$env:HUBFINANCE_BIND = '127.0.0.1'
if ($DemoName) { $env:HUBFINANCE_DEMO = $DemoName } elseif ($Demo) { $env:HUBFINANCE_DEMO = 'grafite' } else { Remove-Item Env:HUBFINANCE_DEMO -ErrorAction SilentlyContinue }

$exe = Join-Path (Get-Location) 'target\debug\hub_finance.exe'
$argList = @()
if ($Background) { $argList += '--background' }
$proc = if ($argList.Count) { Start-Process -FilePath $exe -ArgumentList $argList -PassThru } else { Start-Process -FilePath $exe -PassThru }
Start-Sleep -Seconds 8

$failed = $false
$proc.Refresh()
if ($proc.HasExited) {
    Write-Host "processo encerrou (codigo $($proc.ExitCode))"
    $failed = $true
}

$logs = Join-Path $data 'logs'
if (Test-Path $logs) {
    $bad = Get-ChildItem $logs -Filter '*.log' | Select-String -CaseSensitive -Pattern 'ERROR|panicked'
    if ($bad) {
        $bad | Select-Object -First 5 | ForEach-Object { Write-Host $_.Line }
        $failed = $true
    }
}

if ($Background -and -not $failed) {
    if ($proc.MainWindowHandle -ne 0) {
        Write-Host 'havia janela com --background'
        $failed = $true
    }
}

if (-not $failed -and -not $Background) {
    try {
        Add-Type -AssemblyName System.Drawing
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SmokeWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hgt, bool repaint);
}
'@
        $proc.Refresh()
        $h = $proc.MainWindowHandle
        if ($h -eq 0) { throw 'sem janela' }
        if ($Width -gt 0 -or $Height -gt 0) {
            $cur = New-Object SmokeWin+RECT
            [void][SmokeWin]::GetWindowRect($h, [ref]$cur)
            $nw = if ($Width -gt 0) { $Width } else { $cur.Right - $cur.Left }
            $nh = if ($Height -gt 0) { $Height } else { 900 }
            [void][SmokeWin]::MoveWindow($h, 50, 50, $nw, $nh, $true)
            Start-Sleep -Seconds 1
        }
        [void][SmokeWin]::SetForegroundWindow($h)
        Start-Sleep -Milliseconds 800
        $r = New-Object SmokeWin+RECT
        # Limites visiveis (DWMWA_EXTENDED_FRAME_BOUNDS = 9): GetWindowRect inclui ~8 px de borda invisivel.
        if ([SmokeWin]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) -ne 0) {
            [void][SmokeWin]::GetWindowRect($h, [ref]$r)
        }
        $w = $r.Right - $r.Left
        $hg = $r.Bottom - $r.Top
        $bmp = New-Object System.Drawing.Bitmap $w, $hg
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
        $g.Dispose()
        $out = Join-Path ([IO.Path]::GetTempPath()) 'hub-smoke.png'
        $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
        $px = $bmp.GetPixel(20, 60)
        $bmp.Dispose()
        if (($px.R + $px.G + $px.B) -eq 0) {
            Write-Host 'captura indisponivel (imagem preta; sessao bloqueada?)'
        } else {
            Write-Host ('pixel #{0:X2}{1:X2}{2:X2}' -f $px.R, $px.G, $px.B)
            Write-Host "captura em $out"
        }
    } catch {
        Write-Host "captura indisponivel: $($_.Exception.Message)"
    }
}

if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
if ($failed) { exit 1 }
Write-Host 'ok'
