param([Parameter(Mandatory=$true)][string]$Directory,
      [Parameter(Mandatory=$true)][string]$Prefix,
      [string]$Theme = 'light', [switch]$Draft)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$process = Start-Process (Join-Path $Directory 'archaic.exe') -ArgumentList @('--temporary-session', '--fixture', '--theme', $Theme) -NoNewWindow -PassThru -RedirectStandardError "$Prefix-stderr.txt" -RedirectStandardOutput "$Prefix-stdout.txt"
try {
  $deadline = [DateTime]::UtcNow.AddSeconds(30)
  do {
    Start-Sleep -Milliseconds 250
    $process.Refresh()
    if ($process.HasExited) { throw "Archaic exited: $($process.ExitCode)" }
    $handle = $process.MainWindowHandle
  } while ($handle -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
  if ($handle -eq [IntPtr]::Zero) { throw 'No Archaic window' }
  Start-Sleep -Seconds 3
  $window = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
  if ($Draft) {
    $condition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
    $edits = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    $composer = $null
    $bottom = 0
    foreach ($edit in $edits) {
      $value = $null
      if ($edit.Current.IsOffscreen -or -not $edit.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$value) -or $value.Current.IsReadOnly) { continue }
      if ($edit.Current.BoundingRectangle.Bottom -gt $bottom) {
        $composer = $edit
        $bottom = $edit.Current.BoundingRectangle.Bottom
      }
    }
    if ($null -eq $composer) { throw 'No writable composer in accessibility tree' }
    $composer.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('A focused draft for checking the composer and send button.')
    $composer.SetFocus()
  } else {
    $condition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, 'Settings')
    $button = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
    if ($null -eq $button) { throw 'Settings action is missing from accessibility tree' }
    $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
  }
  Start-Sleep -Seconds 2
  $bounds = [Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bitmap = New-Object Drawing.Bitmap $bounds.Width, $bounds.Height
  $graphics = [Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.CopyFromScreen($bounds.Location, [Drawing.Point]::Empty, $bounds.Size)
    $bitmap.Save("$Prefix.png", [Drawing.Imaging.ImageFormat]::Png)
  } finally { $graphics.Dispose(); $bitmap.Dispose() }
} finally {
  if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
