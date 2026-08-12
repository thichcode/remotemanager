# Dismiss Dependabot alerts for thichcode/remotemanager
# Usage: $env:GITHUB_TOKEN="ghp_xxx"; .\dismiss-dependabot.ps1
# Get token at: https://github.com/settings/tokens (scope: repo:security_events)

param(
    [string]$Token = $env:GITHUB_TOKEN,
    [string]$Repo = "thichcode/remotemanager"
)

if (-not $Token) {
    Write-Error "Set \$env:GITHUB_TOKEN first. Get token: https://github.com/settings/tokens (scope: repo:security_events)"
    exit 1
}

$headers = @{
    "Accept"        = "application/vnd.github+json"
    "Authorization" = "Bearer $Token"
    "X-GitHub-Api-Version" = "2022-11-28"
}

$alerts = Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$Repo/dependabot/alerts?state=open"

$ignored = @(
    "RUSTSEC-2023-0071",  # rsa from russh: Marvin Attack
    "RUSTSEC-2022-0011",  # rust-crypto from rdp-rs: AES miscomputation
    "RUSTSEC-2022-0004",  # rustc-serialize from rdp-rs: JSON stack overflow
    "RUSTSEC-2020-0071"   # time 0.1.45 from rdp-rs: potential segfault
)

foreach ($alert in $alerts) {
    $advisoryId = $alert.security_advisory.ghsa_id ?? ""
    $vulnId = $alert.security_advisory.identifiers | Where-Object { $_.type -eq "rustsec" } | Select-Object -ExpandProperty value
    $vulnId = if (-not $vulnId) { $alert.security_advisory.identifiers | Where-Object { $_.type -eq "RHSA" -or $_.type -eq "CVE" } | Select-Object -ExpandProperty value } else { $vulnId }
    
    $shouldDismiss = $false
    if ($ignored -contains $vulnId) { $shouldDismiss = $true }
    if ($ignored -contains $advisoryId) { $shouldDismiss = $true }
    
    # Also match by summary text as fallback
    $summary = $alert.security_advisory.summary ?? ""
    if ($summary -match "Marvin Attack|AES miscomputation|stack overflow in rustc_serialize|segfault in the time crate") { $shouldDismiss = $true }
    
    if ($shouldDismiss) {
        $dismissUri = "https://api.github.com/repos/$Repo/dependabot/alerts/$($alert.number)"
        $body = @{ dismissed_reason = "wont_fix"; dismissed_comment = "Transitive dependency with no upstream fix. Suppressed in .cargo/audit.toml" } | ConvertTo-Json
        try {
            Invoke-RestMethod -Method Patch -Headers $headers -Uri $dismissUri -Body $body -ContentType "application/json"
            Write-Host "Dismissed alert #$($alert.number): $vulnId - $summary"
        } catch {
            Write-Warning "Failed to dismiss alert #$($alert.number): $($_.Exception.Message)"
        }
    } else {
        Write-Host "Keeping alert #$($alert.number): $vulnId - $summary"
    }
}