<#
.SYNOPSIS
    Downloads Whisper weights in the Candle layout expected by karaoke-cli.

.DESCRIPTION
    Fetches config.json, tokenizer.json and model.safetensors from a
    Hugging Face openai/whisper-* repository into one directory, which is what
    the CLI expects for --whisper_model_dir. The CLI itself never downloads
    anything; this script is an explicit, one-off convenience.

.PARAMETER Model
    Whisper size: tiny, base, small, medium, large-v1..v3, or an *.en variant.

.PARAMETER Destination
    Target directory. Defaults to .\whisper-<model> next to this script.

.PARAMETER Force
    Re-download files that already exist.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\fetch-whisper.ps1

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\fetch-whisper.ps1 -Model small -Destination D:\whisper-small
#>
[CmdletBinding()]
param(
    [ValidateSet('tiny', 'tiny.en', 'base', 'base.en', 'small', 'small.en', 'medium', 'medium.en', 'large-v1', 'large-v2', 'large-v3')]
    [string]$Model = 'base',

    [string]$Destination = (Join-Path $PSScriptRoot "whisper-$Model"),

    [switch]$Force
)

$ErrorActionPreference = 'Stop'

if ($PSVersionTable.PSVersion.Major -lt 6) {
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
}

function Get-File {
    param([string]$Uri, [string]$Path)
    if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
        & curl.exe -L --fail --retry 3 --retry-delay 5 -o "$Path" "$Uri"
        if ($LASTEXITCODE -ne 0) { throw "curl failed ($LASTEXITCODE) for $Uri" }
    }
    else {
        Invoke-WebRequest -Uri $Uri -OutFile $Path -UseBasicParsing
    }
}

$repo = "openai/whisper-$Model"
$files = 'config.json', 'tokenizer.json', 'model.safetensors'

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Write-Host "repo:        $repo"
Write-Host "destination: $Destination"

foreach ($name in $files) {
    $target = Join-Path $Destination $name
    if ((Test-Path $target) -and -not $Force) {
        Write-Host "skip  $name (already present; use -Force to replace)"
        continue
    }

    $url = "https://huggingface.co/$repo/resolve/main/$name"
    Write-Host "get   $name"
    try {
        Get-File -Uri $url -Path "$target.part"
        Move-Item -Force "$target.part" $target
    }
    finally {
        if (Test-Path "$target.part") { Remove-Item -Force "$target.part" }
    }
}

# Sanity checks: the JSON files parse and the weights are a real binary blob,
# so an HTML/JSON error page from the CDN is caught here instead of at load.
Get-Content -Raw (Join-Path $Destination 'config.json') | ConvertFrom-Json | Out-Null
Get-Content -Raw (Join-Path $Destination 'tokenizer.json') | ConvertFrom-Json | Out-Null

$weights = Get-Item (Join-Path $Destination 'model.safetensors')
if ($weights.Length -lt 1MB) {
    throw "model.safetensors is only $($weights.Length) bytes - not a checkpoint"
}

Get-ChildItem $Destination |
    Select-Object Name, @{ n = 'MB'; e = { [math]::Round($_.Length / 1MB, 1) } } |
    Format-Table -AutoSize

Write-Host ''
Write-Host 'Done. Use it with:'
Write-Host "  karaoke-cli.exe song.mp3 --model_filename <model>.safetensors --config <profile>.yaml ``"
Write-Host "    --output_dir out --transcribe --whisper_model_dir `"$Destination`" --language ru"
