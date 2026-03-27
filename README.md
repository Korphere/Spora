# Spora 🌿

[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Version](https://img.shields.io/github/v/release/Korphere/Spora)](https://github.com/Korphere/Spora/releases)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

**Spora** (スポラ) は、JavaおよびKotlinプロジェクトのための、軽量で宣言的なツールチェーンマネージャーです。
プロジェクトごとに最適なJDKベンダーとバージョンを自動的に取得し、開発環境のセットアップを自動化します。

## ✨ 特徴

- **Zero-Setup**: `spora.toml` を書くだけで、必要なJDKが自動的にダウンロード・配置されます。
- **マルチベンダー対応**: Temurin, Microsoft, Corretto, Oracle, Zulu 等をサポート。
- **カタログ同期**: GitHub上の最新カタログと同期し、常に最新のマイナーアップデートを追跡。
- **Rust製**: 爆速な動作と、最小限の依存関係。

## 🚀 インストール

[Releases](https://github.com/Korphere/Spora/releases) からお使いのOSに合ったバイナリをダウンロードし、パスの通った場所に配置してください。

## 📦 使い方

プロジェクトのルートに `spora.toml` を作成します。

```toml
[project]
name = "my-awesome-app"
lang = "java"

[runtime]
vendor = "temurin"
version = "21"
```
