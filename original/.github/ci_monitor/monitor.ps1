$log = "ci_monitor.log"
Add-Content $log "---- Monitor started: $(Get-Date -Format o) ----"
while ($true) {
  try {
    $ts = Get-Date -Format o
    Add-Content $log "`n=== Poll at $ts ==="

    Invoke-WebRequest -Uri 'https://github.com/Flare-Animate/Flare/pull/46' -UseBasicParsing -OutFile pr_page.html -ErrorAction Stop
    Select-String -Path pr_page.html -Pattern 'CI: ' -AllMatches | ForEach-Object { Add-Content $log $_.Line }

    Invoke-WebRequest -Uri 'https://github.com/Flare-Animate/Flare/actions?query=branch:ci/auto-fix-checkout' -UseBasicParsing -OutFile actions_page.html -ErrorAction Stop
    Select-String -Path actions_page.html -Pattern 'Failure|In progress|in progress|failed|success|ran' -AllMatches | ForEach-Object { Add-Content $log $_.Line }
  } catch {
    Add-Content $log "ERROR: $_"
  }
  Start-Sleep -Seconds 180
}
