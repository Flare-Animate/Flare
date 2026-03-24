$branch = 'ci/auto-fix-checkout'
$retriggerInterval = 180 # seconds
$retriggerCooldown = 300 # seconds between pushes
$lastPush = Get-Date "2000-01-01"
Add-Content ci_retrigger.log "---- Auto-retrigger started: $(Get-Date -Format o) ----"
while ($true) {
  try {
    $ts = Get-Date -Format o
    Add-Content ci_retrigger.log "\n=== Check at $ts ==="
    $actionsPage = Invoke-WebRequest -Uri "https://github.com/Flare-Animate/Flare/actions?query=branch:$branch" -UseBasicParsing -ErrorAction Stop
    $body = $actionsPage.Content
    if ($body -match 'Failure') {
      Add-Content ci_retrigger.log "Failure found on Actions page at $ts"
      $now = Get-Date
      if (($now - $lastPush).TotalSeconds -gt $retriggerCooldown) {
        Add-Content ci_retrigger.log "Creating empty commit to retrigger CI..."
        git add -A
        git commit --allow-empty -m "CI: retrigger after failures $ts" | Out-Null
        git push origin $branch
        $lastPush = Get-Date
        Add-Content ci_retrigger.log "Pushed retrigger commit at $(Get-Date -Format o)"
      } else {
        Add-Content ci_retrigger.log "Skipping push due to cooldown; last push at $lastPush"
      }
    } else {
      Add-Content ci_retrigger.log "No failures detected at $ts"
    }
  } catch {
    Add-Content ci_retrigger.log "ERROR: $_"
  }
  Start-Sleep -Seconds $retriggerInterval
}
