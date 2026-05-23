Add-Type -AssemblyName System.Drawing

# Create a grid of all tiles
$tileSize = 16
$cols = 12
$rows = 11
$gap = 2
$padding = 4

$totalW = $padding * 2 + $cols * ($tileSize + $gap) - $gap
$totalH = $padding * 2 + $rows * ($tileSize + $gap) - $gap

$bmp = New-Object System.Drawing.Bitmap $totalW, $totalH
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::FromArgb(0, 0, 0, 0))

$dir = "assets/kenney_tiny-town/Tiles"

for ($i = 0; $i -lt 132; $i++) {
    $path = "$dir/tile_$($i.ToString('0000')).png"
    $col = $i % $cols
    $row = [Math]::Floor($i / $cols)
    $x = $padding + $col * ($tileSize + $gap)
    $y = $padding + $row * ($tileSize + $gap)

    if (Test-Path $path) {
        $img = [System.Drawing.Image]::FromFile($path)
        $g.DrawImage($img, $x, $y, $tileSize, $tileSize)
        $img.Dispose()
    }

    # Draw index number
    $g.DrawString($i.ToString(), [System.Drawing.Font]::new("Arial", 4), [System.Drawing.Brushes]::White, $x, $y + 12)
}

$outPath = "assets/kenney_tiny-town/tile_reference.png"
$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Host "Created $outPath ($totalW x $totalH)"
