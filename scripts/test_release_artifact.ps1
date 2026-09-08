param([Parameter(Mandatory = $true)][string]$Binary)

$ErrorActionPreference = "Stop"

$catalog = Join-Path ([System.IO.Path]::GetTempPath()) ("rocklake-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $catalog | Out-Null
try {
    & $Binary --version
    $version = & $Binary --version --output json | ConvertFrom-Json
    if (-not $version.version) { throw "version JSON did not contain version" }
    $doctor = & $Binary doctor --catalog (Join-Path $catalog "catalog") --output json | ConvertFrom-Json
    if (-not $doctor.ready) { throw "doctor reported not ready" }
} finally {
    Remove-Item -Recurse -Force $catalog -ErrorAction SilentlyContinue
}
