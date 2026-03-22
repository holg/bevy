$duration = 300
  $env:RUST_LOG = "bevy_diagnostic=info,info"

  foreach ($name in @("bistro_bench", "bistro_bench-fix")) {
      Write-Host "`n=== Running $name for $($duration/60) minutes ===" -ForegroundColor Cyan

      $proc = Start-Process -FilePath ".\$name.exe" -RedirectStandardError "$name.log" -PassThru

      # Sample memory every 5 seconds
      $ramSamples = @()
      $vramSamples = @()
      $timer = [Diagnostics.Stopwatch]::StartNew()

      while ($timer.Elapsed.TotalSeconds -lt $duration -and !$proc.HasExited) {
          Start-Sleep -Seconds 5

          # Process RAM (working set)
          $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
          if ($p) { $ramSamples += $p.WorkingSet64 / 1MB }

          # GPU VRAM (NVIDIA dedicated memory via performance counter)
          $gpu = Get-Counter '\GPU Process Memory(pid_*_luid_*)\Dedicated Usage' -ErrorAction SilentlyContinue
          if ($gpu) {
              $pidCounters = $gpu.CounterSamples | Where-Object {
                  $_.InstanceName -match "pid_$($proc.Id)_"
              }
              if ($pidCounters) {
                  $vramSamples += ($pidCounters | Measure-Object -Property CookedValue -Sum).Sum / 1MB
              }
          }
      }

      Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
      Start-Sleep -Seconds 2

      # Parse FPS
      $avgs = Select-String -Path "$name.log" -Pattern "fps\s+:.*avg\s+([\d.]+)" |
          ForEach-Object { [double]$_.Matches.Groups[1].Value }
      $steady = $avgs | Select-Object -Skip 2

      Write-Host "  --- FPS ---"
      if ($steady.Count -gt 0) {
          $stats = $steady | Measure-Object -Average -Minimum -Maximum
          Write-Host "  Samples: $($stats.Count)"
          Write-Host "  Avg:     $([math]::Round($stats.Average, 1))"
          Write-Host "  Min:     $([math]::Round($stats.Minimum, 1))"
          Write-Host "  Max:     $([math]::Round($stats.Maximum, 1))"
      }

      Write-Host "  --- RAM (Working Set) ---"
      if ($ramSamples.Count -gt 0) {
          $rs = $ramSamples | Measure-Object -Average -Maximum
          Write-Host "  Avg:     $([math]::Round($rs.Average)) MB"
          Write-Host "  Peak:    $([math]::Round($rs.Maximum)) MB"
      }

      Write-Host "  --- VRAM (Dedicated GPU) ---"
      if ($vramSamples.Count -gt 0) {
          $vs = $vramSamples | Measure-Object -Average -Maximum
          Write-Host "  Avg:     $([math]::Round($vs.Average)) MB"
          Write-Host "  Peak:    $([math]::Round($vs.Maximum)) MB"
      } else {
          Write-Host "  Not available (needs NVIDIA GPU perf counters)"
      }
  }

  Write-Host "`n=== Done ===" -ForegroundColor Cyan