# Run one segment of the resumable 48-hour rusqlite soak test.
#
# The database and logs are kept in local/soak-48h/ inside the workspace, never
# on the system temp directory or the C: drive. If a previous soak database
# exists, the script resumes; otherwise it starts a fresh segment.
#
# Usage:
#   .\local\run-soak-segment.ps1                    # run one 6-hour segment
#   $env:RUPRIZZLE_SOAK_DURATION_SECONDS=3600; .\local\run-soak-segment.ps1

$ErrorActionPreference = "Stop"

$workspace = Resolve-Path (Join-Path $PSScriptRoot "..")
$soakDir = Join-Path $workspace "local/soak-48h"
$dbPath = Join-Path $soakDir "soak-rusqlite.db"

New-Item -ItemType Directory -Force -Path $soakDir | Out-Null

if (-not $env:RUPRIZZLE_SOAK_WORKERS) { $env:RUPRIZZLE_SOAK_WORKERS = "8" }
if (-not $env:RUPRIZZLE_SOAK_DURATION_SECONDS) { $env:RUPRIZZLE_SOAK_DURATION_SECONDS = "21600" }

$env:RUPRIZZLE_TEST_RUSQLITE = "1"
$env:RUST_BACKTRACE = "1"
$env:RUPRIZZLE_SOAK_LOG_DIR = $soakDir
$env:RUPRIZZLE_SOAK_DB_PATH = $dbPath

if (Test-Path $dbPath) {
    $env:RUPRIZZLE_SOAK_RESUME = "1"
    Write-Host "Resuming existing soak from $dbPath"
} else {
    $env:RUPRIZZLE_SOAK_RESUME = "0"
    Write-Host "Starting new soak at $dbPath"
}

Set-Location $workspace

cargo test -p ruprizzle --test soak_resumable --features "sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite" --release -- --ignored --exact soak_rusqlite_resumable_48h --nocapture
