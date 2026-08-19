# Run 1-hour segments of the resumable rusqlite soak until the 48-hour target is reached.
#
# Each segment is resumable; if this script is interrupted, run it again and it
# will continue from the state stored in local/soak-48h/soak-rusqlite.db.

$ErrorActionPreference = "Stop"

$workspace = Resolve-Path (Join-Path $PSScriptRoot "..")
$soakDir = Join-Path $workspace "local/soak-48h"
$dbPath = (Join-Path $soakDir "soak-rusqlite.db") -replace '\\', '/'

New-Item -ItemType Directory -Force -Path $soakDir | Out-Null
Set-Location $workspace

while ($true) {
    .\local\run-soak-segment.ps1

    $completed = python -c "import sqlite3; conn=sqlite3.connect('$dbPath'); c=conn.execute('SELECT completed FROM soak_state WHERE id=1').fetchone(); conn.close(); print(c[0] if c else 0)"
    if ($completed -eq "1") {
        Write-Host "Soak completed."
        break
    }
}
