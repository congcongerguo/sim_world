Add-Type -AssemblyName System.Drawing

$dir = "assets/kenney_tiny-town/Tiles"

function Analyze-Tile($path) {
    $img = [System.Drawing.Image]::FromFile($path)
    $bmp = New-Object System.Drawing.Bitmap $img
    $data = @{
        green = 0; brown = 0; blue = 0; red = 0; white = 0; black = 0; skin = 0; transparent = 0
        total = 0
    }
    for ($y = 0; $y -lt 16; $y++) {
        for ($x = 0; $x -lt 16; $x++) {
            $p = $bmp.GetPixel($x, $y)
            if ($p.A -eq 0) { $data.transparent++; continue }
            $data.total++
            # Green (vegetation)
            if ($p.G -gt 120 -and $p.G -gt $p.R + 10 -and $p.G -gt $p.B + 10) { $data.green++ }
            # Skin tones (warm, mid-brightness)
            elseif ($p.R -gt 150 -and $p.G -lt 150 -and $p.B -lt 130 -and $p.R - $p.G -gt 20) { $data.skin++ }
            # Blue/water
            elseif ($p.B -gt 120 -and $p.B -gt $p.R + 10 -and $p.B -gt $p.G + 10) { $data.blue++ }
            # Red
            elseif ($p.R -gt 180 -and $p.G -lt 100 -and $p.B -lt 100) { $data.red++ }
            # White/light
            elseif ($p.R -gt 200 -and $p.G -gt 200 -and $p.B -gt 200) { $data.white++ }
            # Brown/wood/dark
            elseif ($p.R -gt 80 -and $p.R -lt 200 -and $p.G -gt 40 -and $p.G -lt 150 -and $p.B -lt 100) { $data.brown++ }
            # Dark/black
            elseif ($p.R -lt 80 -and $p.G -lt 80 -and $p.B -lt 80) { $data.black++ }
        }
    }
    $bmp.Dispose()
    $img.Dispose()
    return $data
}

Write-Host "idx`tgrn`tbrn`tblu`tred`twht`tbkl`tskn`ttrn`tcategory"
for ($i = 0; $i -lt 132; $i++) {
    $path = "$dir/tile_$($i.ToString('0000')).png"
    if (!(Test-Path $path)) { continue }
    $d = Analyze-Tile $path
    $cat = if ($d.green -gt 50) { "veg" }
    elseif ($d.skin -gt 20) { "char" }
    elseif ($d.red -gt 30) { "roof" }
    elseif ($d.blue -gt 50) { "water" }
    elseif ($d.white -gt 100) { "white" }
    elseif ($d.brown -gt 50 -and $d.transparent -lt 50) { "brown" }
    elseif ($d.black -gt 80) { "dark" }
    elseif ($d.transparent -gt 200) { "sparse" }
    elseif ($d.total -eq 256 -and $d.green -gt 100) { "grass" }
    elseif ($d.total -eq 256) { "solid" }
    else { "mixed" }
    Write-Host "$i`t$($d.green)`t$($d.brown)`t$($d.blue)`t$($d.red)`t$($d.white)`t$($d.black)`t$($d.skin)`t$($d.transparent)`t$cat"
}
