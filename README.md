# Spora 🌿

![SporaLogo](https://github.com/Korphere/Spora/tree/main/resources/logo.png "SporaLogo")

[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Version](https://img.shields.io/github/v/release/Korphere/Spora)](https://github.com/Korphere/Spora/releases)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

**Spora** is a lightweight toolchain manager and build tool for projects using JVM languages.
It automatically retrieves the optimal JDK vendor and version for each project and automates the setup of the development environment.

## ✨ Features

- **Zero-Setup**: Simply write a `spora.toml` file, and the required JDKs will be automatically downloaded and installed.
- **Multi-Vendor Support**: Supports Temurin, Microsoft, Corretto, Oracle, Zulu, Liberica, SAP, GraalVM CE, and more.
- **Catalog Synchronization**: Synchronizes with the latest catalog on GitHub to always track the latest minor updates.
- **Built with Rust**: Blazing-fast performance and minimal dependencies.

## 🚀 Installation

### Download and place the binary

Download the binary for your OS from [Releases](https://github.com/Korphere/Spora/releases) and place it in a directory on your PATH.

### Install using curl

```shell
curl --proto ‘=https’ --tlsv1.2 -LsSf https://github.com/Korphere/Spora/releases/download/<version>/spora-installer.sh | sh
```

### Installing with PowerShell

```powershell
powershell -ExecutionPolicy Bypass -c “irm https://github.com/Korphere/Spora/releases/download/<version>/spora-installer.ps1 | iex”
```

## 📦 How to Use

Create a `spora.toml` file in the project root.

```toml
[project]
name = “my-awesome-app”
lang = “java”
[runtime]
vendor = “temurin”
version = “21”
```
