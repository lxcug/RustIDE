Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-Cmd([string]$Name) {
  $cmd = Get-Command $Name -ErrorAction SilentlyContinue
  if ($null -eq $cmd) {
    [pscustomobject]@{ Name = $Name; Found = $false; Path = "" }
  } else {
    [pscustomobject]@{ Name = $Name; Found = $true; Path = $cmd.Source }
  }
}

$checks = @(
  "git",
  "rustc",
  "cargo",
  "rg",
  "clangd",
  "node",
  "pyright-langserver",
  "python"
) | ForEach-Object { Test-Cmd $_ }

$checks | Format-Table -AutoSize

if ($checks | Where-Object { -not $_.Found }) {
  Write-Host ""
  Write-Host "Missing prerequisites detected. Install suggestions:" -ForegroundColor Yellow
  Write-Host "  - Rust: https://rustup.rs/"
  Write-Host "  - ripgrep (rg): https://github.com/BurntSushi/ripgrep/releases"
  Write-Host "  - clangd: install LLVM/clang tools or use your package manager"
  Write-Host "  - pyright: npm i -g pyright"
  Write-Host "  - pylsp (optional): python -m pip install 'python-lsp-server[all]'"
  exit 1
}

Write-Host ""
Write-Host "All prerequisites found." -ForegroundColor Green

