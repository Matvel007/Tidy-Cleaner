<div align="center">

<img src="resources/icons/logo.png" alt="Tidy Cleaner Logo" width="128" height="128" />

# Tidy Cleaner

**Modern, ultra-fast, and premium Linux system manager & cleaner built in Rust with Slint.**

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![Slint UI](https://img.shields.io/badge/Slint-1.17-blue.svg?logo=slint)](https://slint.dev/)
[![Platform](https://img.shields.io/badge/Platform-Linux-green.svg?logo=linux)](https://kernel.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Translations](https://img.shields.io/badge/i18n-English%20%7C%20Русский-purple.svg)](#localization)

[Русская версия (README_ru.md)](README_ru.md)

</div>

---

## 🌟 Overview

**Tidy Cleaner** is a modern, lightweight, and safe alternative to system cleaners for Linux. Combining real-time hardware telemetry, disk management, and intelligent cleaning workflows, it provides a native frameless desktop interface without sacrificing system security or performance.

<div align="center">

![Tidy Cleaner Dashboard](resources/screenshots/dashboard.png)

</div>

---

## ✨ Features

- ⚡ **Real-Time Telemetry** — Circular gauges for CPU load, GPU utilization (NVIDIA & integrated), and RAM usage.
- 🌡️ **Thermal Monitoring** — Live CPU & GPU thermal indicators with adaptive status indicators.
- 💾 **Multi-Disk Analytics** — Individual disk cards with live capacity, free space tracking, and file system details (Btrfs, ext4, etc.).
- 💻 **System Info Overview** — Terminal-inspired compact overview showing OS, Host, Kernel version, and system Uptime.
- 🧹 **Intelligent Cleanup** *(In Development)* — Fast & Full scanning routines with risk levels (Safe, Warning, Dangerous) and dry-run user review.
- 📦 **Applications Manager** *(In Development)* — Unified management for Pacman, AUR, Flatpak, and desktop entries.
- 🚀 **Startup Manager** *(In Development)* — Inspect, enable, disable, and add autostart applications conforming to Freedesktop standards.
- 🎨 **Premium Frameless UI** — Custom draggable titlebar, butter-smooth sidebar animations, and adaptive dark/light themes.
- 🌍 **Strict Localization** — External XML localization architecture supporting English and Russian seamlessly.

---

## 🛠️ Tech Stack

- **Language:** [Rust](https://www.rust-lang.org/) (Strict memory safety, Tokio async runtime)
- **GUI Toolkit:** [Slint](https://slint.dev/) (Declarative, fluid animations, native Linux rendering)
- **Telemetry:** `sysinfo`, `nvidia-smi` integration
- **Architecture:** Zero monolithic files, modular design with services and safety checks

---

## 🚀 Building & Running

### Prerequisites

Ensure you have Rust and Cargo installed:

```bash
# Arch Linux / CachyOS / Manjaro
sudo pacman -S rust cargo

# Debian / Ubuntu
sudo apt update && sudo apt install cargo
```

### Run in Development Mode

```bash
cargo run
```

### Build Optimized Release Binary

```bash
cargo build --release
```

The compiled binary will be available at `target/release/tidy-cleaner`.

---

## 🧪 Testing

Run unit tests, integration tests, and localization validation:

```bash
cargo test
```

---

## ❤️ Made with AI

> Программа делается с помощью **Gemini 3.7 Flash** и **DeepSeek v4 Pro** ❤️

---

## 📄 License

Distributed under the **MIT License**. See `LICENSE` for more information.
