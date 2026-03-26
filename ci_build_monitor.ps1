$repo='Flare-Animate/Flare'
$branch='ci/auto-fix-checkout'
Write-Host "Starting automatic CI monitor at $(Get-Date -Format o)"
while ($true) {
  $latest = gh run list --repo $repo --branch $branch --limit 1 --json databaseId,name,status,conclusion,headSha,workflowName | ConvertFrom-Json
  if (-not $latest) {
    Write-Host "No runs found yet. sleeping 30..."; Start-Sleep -Seconds 30; continue
  }
  $run = $latest[0]
  $line = "$(Get-Date -Format o) Run $($run.databaseId) $($run.workflowName) status=$($run.status) conclusion=$($run.conclusion) sha=$($run.headSha)"
  Write-Host $line
  if ($run.status -eq 'completed' -and $run.conclusion -eq 'success') {
    Write-Host "All good: build success. Exiting monitor."
    break
  }
  if ($run.status -eq 'completed' -and $run.conclusion -eq 'failure') {
    Write-Host "Latest run failed; inspect logs and possible fix required. Will keep polling."
  }
  Start-Sleep -Seconds 30
}
