[CmdletBinding()]
Param(
  [Parameter(Mandatory=$true)]
  [ValidateSet("windows-2019", "ubuntu-18.04", "macos-10.15")]
  [string]$Runner,

  [Parameter(Mandatory=$true)]
  [string]$Version
)

$ErrorActionPreference = "Stop"

function Write-Output
{
    param ( [string]$name, [string]$value )

    Write-Host ("::set-output name=$name::$value")
}

function Get-Sem-Version
{
    param ( [string]$version )

    $index = $version.LastIndexOf("-")

    if ($index -eq -1) {
        return $version
    }


    $numbers = $version.Remove($index).Split("-")
    $numbers = @($numbers | ForEach-Object {
        if ($_ -eq "2020") {
            "20"
        } else {
            $_
        }
    })

    return [string]::Join(".", $numbers)
}

New-Item -Path . -Name "output" -ItemType "directory" -Force

switch($Runner)
{
  ubuntu-18.04
  {
    cargo install cargo-deb
    cargo deb --manifest-path=Cargo.toml --output=output
    $artifactName = ls output
    $finalName = "EventStoreDB.TUI-$Version-1.${Runner}_amd64.deb"
    Push-Location "output"
    Move-Item -Path $artifactName $finalName
    Write-Output "artifact_name" $finalName
    Write-Output "content_type" "application/vnd.debian.binary-package"
    Pop-Location
  }

  windows-2019
  {
    cargo build --bin esdb-tui --release
    Move-Item -Path (Join-Path "target" (Join-Path "release" "esdb-tui.exe")) (Join-Path "output" "esdb-tui.exe")
    Push-Location output
    $artifactName = "EventStoreDB.TUI-Windows-x64-$Version.zip"
    Write-Output "artifact_name" $artifactName
    Write-Output "content_type" "application/zip"
    Compress-Archive -Path "esdb-tui.exe" -DestinationPath $artifactName
    Pop-Location
  }

  macos-10.15
  {
    cargo build --bin esdb-tui --release

    $packageName = "EventStoreDB.TUI-OSX-$Version.pkg"
    $semVer = Get-Sem-Version $Version

    New-Item -Path . -Name "macbuild" -ItemType "directory"
    Copy-Item -Path "target/release/esdb-tui" "macbuild"

    pkgbuild --root macbuild --identifier com.eventstore.cloud.cli --ownership recommended --version $semVer --install-location /usr/local/bin "output/$packageName"

    Write-Output "artifact_name" $packageName
    Write-Output "content_type" "application/octet-stream"
  }
}