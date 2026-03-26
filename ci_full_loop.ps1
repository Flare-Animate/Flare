$repo='Flare-Animate/Flare'
$branch='ci/auto-fix-checkout'
while ($true) {
  $runs = gh run list --repo $repo --branch $branch --limit 1 --json databaseId,name,status,conclusion,headSha,workflowName,createdAt | ConvertFrom-Json
  if (-not $runs) { Write-Host "No run yet"; Start-Sleep 10; continue }
  $run=$runs[0]
  Write-Host "$(Get-Date -Format o) - latest run $($run.databaseId) $($run.workflowName) status=$($run.status) conclusion=$($run.conclusion)"
  if ($run.status -eq 'in_progress' -or $run.status -eq 'queued') { Start-Sleep 10; continue }
  if ($run.status -eq 'completed' -and $run.conclusion -eq 'success') {
    Write-Host "SUCCESS: all workflows passed on run $($run.databaseId). Fetching logs now..."
    gh run view $($run.databaseId) --repo $repo --log > run-$($run.databaseId)-log.txt
    break
  }
  if ($run.status -eq 'completed' -and $run.conclusion -eq 'failure') {
    Write-Host "FAILURE detected on run $($run.databaseId). Fetching artifact logs and job details..."
    gh run view $($run.databaseId) --repo $repo --json jobs > run-$($run.databaseId)-jobs.json
    gh run view $($run.databaseId) --repo $repo --log > run-$($run.databaseId)-log.txt

    $jobs = gh api repos/$repo/actions/runs/$($run.databaseId)/jobs --jq '.jobs[] | select(.conclusion=="failure") | {id:.id,name:.name,html_url:.html_url}'
    if ($jobs) { Write-Host "Failed jobs:\n$jobs" } else { Write-Host "No failed jobs in API payload; maybe workflow file issue." }

    # Placeholder for auto-fix (manual intervention required for actual source fixes)
    Write-Host "(No code fix auto-applied by script: manual inspection required. Sleep and retry.)"
    Start-Sleep 10
    continue
  }
  Start-Sleep 10
}
