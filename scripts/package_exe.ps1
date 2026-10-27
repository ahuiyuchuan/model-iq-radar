#requires -Version 7.0
<#
.SYNOPSIS
校验最终 Windows x64 构建，仅复制一个版本化 EXE，供本地测试与 Release 共用。
.DESCRIPTION
作者：ahui。先执行 tauri build --no-bundle；本脚本不启动应用、不创建 ZIP、
不读取用户数据，不生成校验和附带文件。输出已有同名且不同内容时拒绝覆盖，
避免静默替换曾交付文件；需要新一轮测试时显式指定一个新的输出目录。
.PARAMETER SourcePath
已构建的 EXE 绝对路径；省略时读取项目默认 Cargo release 目录。
.PARAMETER OutputDirectory
仅用于保存本次 EXE 的目录；省略时使用 release-artifacts/v<版本>。
#>
param([string]$SourcePath, [string]$OutputDirectory)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$version = (Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $projectRoot 'package.json') | ConvertFrom-Json).version
if (-not $SourcePath) { $SourcePath = Join-Path $projectRoot 'src-tauri/target/release/model-probe.exe' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $projectRoot "release-artifacts/v$version" }
$source = Get-Item -LiteralPath $SourcePath
if ($source.VersionInfo.FileVersion -ne $version -or $source.VersionInfo.ProductVersion -ne $version) {
    throw 'EXE 文件/产品版本与工程版本不一致，请重新构建。'
}

# 检查 PE 架构，避免把错误目标或扩展名伪装成 EXE 的其他文件作为正式交付物。
$bytes = [IO.File]::ReadAllBytes($source.FullName)
if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) { throw 'EXE 的 DOS 头无效。' }
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
if ($peOffset -lt 0 -or $peOffset -gt $bytes.Length - 6 -or
    [BitConverter]::ToUInt32($bytes, $peOffset) -ne 0x4550 -or
    [BitConverter]::ToUInt16($bytes, $peOffset + 4) -ne 0x8664) { throw 'EXE 必须为 Windows x64 PE。' }

# 公开说明可以晚于编译；应用源码/清单不能晚于 EXE，以免误交付上轮构建。
$inputs = @('src', 'src-tauri/src', 'src-tauri/assets', 'src-tauri/icons', 'src-tauri/capabilities') | ForEach-Object {
    Get-ChildItem -LiteralPath (Join-Path $projectRoot $_) -File -Recurse
}
$inputs += @('package.json', 'package-lock.json', 'vite.config.ts', 'index.html',
    'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'src-tauri/build.rs') | ForEach-Object {
    Get-Item -LiteralPath (Join-Path $projectRoot $_)
}
if ($inputs | Where-Object LastWriteTimeUtc -gt $source.LastWriteTimeUtc) {
    throw '存在晚于 EXE 的应用输入，请重新构建后交付。'
}

# 只复制主程序；比较摘要保证输出与构建逐字节一致，已有不同文件时保留原文件并报错。
$null = New-Item -ItemType Directory -Path $OutputDirectory -Force
$destination = Join-Path $OutputDirectory "Model-IQ-Radar-v$version-Windows-x64.exe"
$sourceHash = (Get-FileHash -LiteralPath $source.FullName -Algorithm SHA256).Hash
if (Test-Path -LiteralPath $destination) {
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $sourceHash) {
        throw '输出目录已有不同内容的同名 EXE，请指定新的输出目录。'
    }
} else {
    Copy-Item -LiteralPath $source.FullName -Destination $destination
}
if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $sourceHash) { throw 'EXE 复制后摘要不符。' }
[pscustomobject]@{ Path = [IO.Path]::GetFullPath($destination); Version = $version; Bytes = $source.Length; SHA256 = $sourceHash.ToLowerInvariant() }
