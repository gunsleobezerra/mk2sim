# Gera os ícones do app (PNG 32/128/256 + icon.ico multi-resolução).
# Uso: powershell -ExecutionPolicy Bypass -File src-tauri/tools/make-icons.ps1
param(
    [string]$OutDir = (Join-Path $PSScriptRoot '..\icons')
)

Add-Type -AssemblyName System.Drawing

$OutDir = [System.IO.Path]::GetFullPath($OutDir)
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function New-LogoBitmap {
    param([int]$Size)

    $bmp = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $g.Clear([System.Drawing.Color]::Transparent)

    $inset = [Math]::Max(1, [int]($Size * 0.03))
    $d = $Size - (2 * $inset)

    $bgBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        (New-Object System.Drawing.Rectangle($inset, $inset, $d, $d)),
        [System.Drawing.Color]::FromArgb(255, 34, 34, 38),
        [System.Drawing.Color]::FromArgb(255, 10, 10, 12),
        90.0)
    $g.FillEllipse($bgBrush, $inset, $inset, $d, $d)

    $ringW = [Math]::Max(1.0, $Size * 0.085)
    $ringInset = [int]($Size * 0.14)
    $ringD = $Size - (2 * $ringInset)
    $ringPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 255, 75, 62), $ringW)
    $g.DrawEllipse($ringPen, $ringInset, $ringInset, $ringD, $ringD)

    if ($Size -ge 48) {
        $fontSize = [float]($Size * 0.30)
        $font = New-Object System.Drawing.Font('Segoe UI', $fontSize, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
        $fmt = New-Object System.Drawing.StringFormat
        $fmt.Alignment = [System.Drawing.StringAlignment]::Center
        $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
        $textRect = New-Object System.Drawing.RectangleF(0, 0, $Size, $Size)
        $textBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 240, 240, 240))
        $g.DrawString('505', $font, $textBrush, $textRect, $fmt)
        $textBrush.Dispose()
        $font.Dispose()
    }

    $ringPen.Dispose()
    $bgBrush.Dispose()
    $g.Dispose()
    return $bmp
}

function Get-IconDib {
    param([System.Drawing.Bitmap]$Bitmap)

    $w = $Bitmap.Width
    $h = $Bitmap.Height
    $rect = New-Object System.Drawing.Rectangle(0, 0, $w, $h)
    $lock = $Bitmap.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $stride = [Math]::Abs($lock.Stride)
    $raw = New-Object byte[] ($stride * $h)
    [System.Runtime.InteropServices.Marshal]::Copy($lock.Scan0, $raw, 0, $raw.Length)
    $Bitmap.UnlockBits($lock)

    $maskStride = [int]([Math]::Floor(($w + 31) / 32) * 4)
    $xorSize = $w * 4 * $h
    $maskSize = $maskStride * $h

    $ms = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter($ms)

    # BITMAPINFOHEADER
    $bw.Write([int]40)
    $bw.Write([int]$w)
    $bw.Write([int]($h * 2))
    $bw.Write([int16]1)
    $bw.Write([int16]32)
    $bw.Write([int]0)
    $bw.Write([int]($xorSize + $maskSize))
    $bw.Write([int]0)
    $bw.Write([int]0)
    $bw.Write([int]0)
    $bw.Write([int]0)

    # XOR bitmap, bottom-up
    for ($y = $h - 1; $y -ge 0; $y--) {
        $bw.Write($raw, ($y * $stride), ($w * 4))
    }

    # AND mask (todos opacos; alpha cuida da transparência)
    $bw.Write((New-Object byte[] $maskSize), 0, $maskSize)
    $bw.Flush()
    $bytes = $ms.ToArray()
    $bw.Dispose()
    $ms.Dispose()
    return $bytes
}

$sizes = @(16, 32, 48, 256)
$images = @()
foreach ($s in $sizes) {
    $bmp = New-LogoBitmap -Size $s
    if ($s -eq 32) { $bmp.Save((Join-Path $OutDir '32x32.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    if ($s -eq 128) { $bmp.Save((Join-Path $OutDir '128x128.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    $images += [pscustomobject]@{ Size = $s; Data = (Get-IconDib -Bitmap $bmp) }
    $bmp.Dispose()
}

# PNG avulsos exigidos pelo tauri.conf.json
foreach ($p in @(@(32, '32x32.png'), @(128, '128x128.png'), @(256, '128x128@2x.png'))) {
    $bmp = New-LogoBitmap -Size $p[0]
    $bmp.Save((Join-Path $OutDir $p[1]), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

# ICO
$icoPath = Join-Path $OutDir 'icon.ico'
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([int16]0)
$bw.Write([int16]1)
$bw.Write([int16]$images.Count)
$offset = 6 + (16 * $images.Count)
foreach ($img in $images) {
    $dim = if ($img.Size -ge 256) { 0 } else { $img.Size }
    $bw.Write([byte]$dim)
    $bw.Write([byte]$dim)
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([int16]1)
    $bw.Write([int16]32)
    $bw.Write([int]$img.Data.Length)
    $bw.Write([int]$offset)
    $offset += $img.Data.Length
}
foreach ($img in $images) {
    $bw.Write($img.Data, 0, $img.Data.Length)
}
$bw.Flush()
$bw.Dispose()
$fs.Dispose()

Get-ChildItem $OutDir -File | ForEach-Object { "{0}  {1} bytes" -f $_.Name, $_.Length }
